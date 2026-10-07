use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::analyzer::{
    code_stats, git_analyzer, github_client::GitHubClient, scanner, todo_scanner,
};
use crate::config::AppConfig;
use crate::model::github::GitHubData;
use crate::model::project::ProjectInfo;

/// Which tab / view is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    Projects,
    GitHub,
    Help,
}

impl ActiveTab {
    pub const ALL: [Self; 4] = [Self::Dashboard, Self::Projects, Self::GitHub, Self::Help];

    pub fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Projects => 1,
            Self::GitHub => 2,
            Self::Help => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Dashboard,
            1 => Self::Projects,
            2 => Self::GitHub,
            _ => Self::Help,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Projects => "Projects",
            Self::GitHub => "GitHub",
            Self::Help => "Help",
        }
    }
}

/// Sort order for the project list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Name,
    Loc,
    Commits,
    LastModified,
}

impl SortOrder {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Loc => "LOC",
            Self::Commits => "Commits",
            Self::LastModified => "Recent",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Loc,
            Self::Loc => Self::Commits,
            Self::Commits => Self::LastModified,
            Self::LastModified => Self::Name,
        }
    }
}

/// Validate that `scan_dir` can be scanned; return a user-facing message on failure.
fn check_scan_directory(scan_dir: &std::path::Path) -> Result<(), String> {
    if !scan_dir.exists() {
        return Err(format!(
            "Scan directory does not exist: {}. Check general.scan_directory in config.",
            scan_dir.display()
        ));
    }
    if !scan_dir.is_dir() {
        return Err(format!(
            "Scan path is not a directory: {}.",
            scan_dir.display()
        ));
    }
    if let Err(e) = std::fs::read_dir(scan_dir) {
        return Err(format!(
            "Cannot read scan directory {}: {e}. Check permissions.",
            scan_dir.display()
        ));
    }
    Ok(())
}

/// Top-level application state (the "Model" in TEA).
pub struct App {
    pub active_tab: ActiveTab,
    pub projects: Vec<ProjectInfo>,
    pub selected_project: usize,
    pub detail_project: Option<usize>,
    pub sort_order: SortOrder,
    pub show_ignored: bool,
    pub is_loading: bool,
    pub should_quit: bool,
    pub config: AppConfig,
    pub status_message: String,
    pub scan_error: Option<String>,
    pub github_data: GitHubData,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let default_tab = match config.display.default_tab.as_str() {
            "projects" => ActiveTab::Projects,
            "github" => ActiveTab::GitHub,
            "help" => ActiveTab::Help,
            _ => ActiveTab::Dashboard,
        };
        Self {
            active_tab: default_tab,
            projects: Vec::new(),
            selected_project: 0,
            detail_project: None,
            sort_order: SortOrder::Name,
            show_ignored: false,
            is_loading: false,
            should_quit: false,
            config,
            status_message: String::new(),
            scan_error: None,
            github_data: GitHubData::default(),
        }
    }

    /// Run the initial project scan, code analysis, git analysis, and technical debt scan.
    pub fn scan_and_analyze(&mut self) {
        self.is_loading = true;
        self.scan_error = None;
        self.status_message = "Scanning projects…".to_string();

        let scan_dir = self.config.resolved_scan_directory();
        if let Err(message) = check_scan_directory(&scan_dir) {
            self.projects.clear();
            self.selected_project = 0;
            self.is_loading = false;
            self.scan_error = Some(message.clone());
            self.status_message = message;
            return;
        }
        let depth = self.config.general.scan_depth;

        self.projects = scanner::scan_projects(&scan_dir, depth, &self.config.analysis);

        // Analyze code stats, git history, and todo markers for each non-ignored project.
        let total = self.projects.len();
        for (i, project) in self.projects.iter_mut().enumerate() {
            if project.ignored {
                continue;
            }
            self.status_message = format!("Analyzing [{}/{}] {}…", i + 1, total, project.name);
            project.code_stats = Some(code_stats::analyze(&project.path, &self.config.analysis));
            project.git_stats = git_analyzer::analyze_git(&project.path);
            project.todo_stats = Some(todo_scanner::scan_todos(
                &project.path,
                &self.config.analysis.exclude_dirs,
            ));
            if let Some(git) = &project.git_stats
                && let Some(last) = &git.last_commit
            {
                project.last_modified = Some(last.timestamp);
            }
        }

        self.apply_sort();
        self.is_loading = false;
        let active = self.visible_projects().len();
        if self.projects.is_empty() {
            self.status_message = format!("No projects found in {}", scan_dir.display());
        } else {
            self.status_message = format!("Found {active} projects");
        }
    }

    /// Fetch remote GitHub data if username is configured.
    pub fn fetch_github(&mut self) {
        let username = self.config.github.username.trim().to_string();
        if username.is_empty() {
            return;
        }

        self.github_data.is_loading = true;
        let token = self.config.github.resolved_token();
        match GitHubClient::new(username, token) {
            Ok(client) => {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    match rt.block_on(client.fetch_all()) {
                        Ok(data) => {
                            self.github_data = data;
                        }
                        Err(e) => {
                            self.github_data.error_message = Some(e);
                            self.github_data.is_loading = false;
                        }
                    }
                }
            }
            Err(e) => {
                self.github_data.error_message = Some(e);
                self.github_data.is_loading = false;
            }
        }
    }

    /// Count projects with no commits in the last 30 days.
    pub fn stale_projects_count(&self) -> usize {
        self.projects
            .iter()
            .filter(|p| !p.ignored)
            .filter(|p| {
                if let Some(git) = &p.git_stats {
                    git.commits_last_30_days == 0
                } else if let Some(last) = p.last_modified {
                    let elapsed = chrono::Local::now().signed_duration_since(last);
                    elapsed.num_days() > 30
                } else {
                    false
                }
            })
            .count()
    }

    /// Total TODO markers found across all active projects.
    pub fn total_todos(&self) -> usize {
        self.projects
            .iter()
            .filter(|p| !p.ignored)
            .filter_map(|p| p.todo_stats.as_ref())
            .map(|t| t.todo_count)
            .sum()
    }

    /// Total FIXME / BUG markers found across all active projects.
    pub fn total_fixmes(&self) -> usize {
        self.projects
            .iter()
            .filter(|p| !p.ignored)
            .filter_map(|p| p.todo_stats.as_ref())
            .map(|t| t.fixme_count + t.bug_count)
            .sum()
    }

    /// Projects filtered by the ignored flag.
    pub fn visible_projects(&self) -> Vec<&ProjectInfo> {
        self.projects
            .iter()
            .filter(|p| self.show_ignored || !p.ignored)
            .collect()
    }

    /// Aggregated totals across all visible (non-ignored) projects.
    pub fn total_loc(&self) -> usize {
        self.projects
            .iter()
            .filter(|p| !p.ignored)
            .map(|p| p.total_loc())
            .sum()
    }

    pub fn total_files(&self) -> usize {
        self.projects
            .iter()
            .filter(|p| !p.ignored)
            .filter_map(|p| p.code_stats.as_ref())
            .map(|s| s.file_count)
            .sum()
    }

    /// Number of distinct languages across all projects.
    pub fn unique_languages(&self) -> usize {
        let mut langs = std::collections::HashSet::new();
        for p in &self.projects {
            if p.ignored {
                continue;
            }
            if let Some(stats) = &p.code_stats {
                for l in &stats.languages {
                    langs.insert(l.name.clone());
                }
            }
        }
        langs.len()
    }

    /// Total commits across all visible (non-ignored) projects.
    pub fn total_commits(&self) -> usize {
        self.projects
            .iter()
            .filter(|p| !p.ignored)
            .map(|p| p.total_commits())
            .sum()
    }

    /// Combined daily activity for the last 52 weeks (364 days) across all projects.
    pub fn combined_activity_52_weeks(&self) -> Vec<usize> {
        let mut combined = vec![0usize; 364];
        for p in &self.projects {
            if p.ignored {
                continue;
            }
            if let Some(git) = &p.git_stats {
                for (i, &count) in git.daily_activity.iter().enumerate() {
                    if i < 364 {
                        combined[i] += count;
                    }
                }
            }
        }
        combined
    }

    // ── key handling ────────────────────────────────────────────

    pub fn handle_key(&mut self, key: KeyEvent) {
        // If in detail view, Esc / Backspace / q returns to project list.
        if self.detail_project.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => {
                    self.detail_project = None;
                    return;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_quit = true;
                    return;
                }
                _ => return,
            }
        }

        // Global bindings.
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('1') => {
                self.active_tab = ActiveTab::Dashboard;
                return;
            }
            KeyCode::Char('2') => {
                self.active_tab = ActiveTab::Projects;
                return;
            }
            KeyCode::Char('3') => {
                self.active_tab = ActiveTab::GitHub;
                return;
            }
            KeyCode::Char('4') | KeyCode::Char('?') => {
                self.active_tab = ActiveTab::Help;
                return;
            }
            KeyCode::Tab => {
                let next = (self.active_tab.index() + 1) % ActiveTab::ALL.len();
                self.active_tab = ActiveTab::from_index(next);
                return;
            }
            KeyCode::BackTab => {
                let prev = if self.active_tab.index() == 0 {
                    ActiveTab::ALL.len() - 1
                } else {
                    self.active_tab.index() - 1
                };
                self.active_tab = ActiveTab::from_index(prev);
                return;
            }
            KeyCode::Char('r') => {
                self.scan_and_analyze();
                self.fetch_github();
                return;
            }
            _ => {}
        }

        // Tab-specific bindings.
        if self.active_tab == ActiveTab::Projects {
            self.handle_project_list_key(key);
        }
    }

    fn handle_project_list_key(&mut self, key: KeyEvent) {
        let visible_len = self.visible_projects().len();
        if visible_len == 0 {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                self.detail_project = Some(self.selected_project);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected_project = (self.selected_project + 1).min(visible_len - 1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_project = self.selected_project.saturating_sub(1);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected_project = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected_project = visible_len.saturating_sub(1);
            }
            KeyCode::Char('s') => {
                self.sort_order = self.sort_order.next();
                self.apply_sort();
                self.status_message = format!("Sorted by {}", self.sort_order.label());
            }
            KeyCode::Char('i') => {
                self.show_ignored = !self.show_ignored;
                self.selected_project = self
                    .selected_project
                    .min(self.visible_projects().len().saturating_sub(1));
                self.status_message = if self.show_ignored {
                    "Showing ignored projects".to_string()
                } else {
                    "Hiding ignored projects".to_string()
                };
            }
            _ => {}
        }
    }

    fn apply_sort(&mut self) {
        match self.sort_order {
            SortOrder::Name => {
                self.projects.sort_by_key(|a| a.name.to_lowercase());
            }
            SortOrder::Loc => {
                self.projects
                    .sort_by_key(|a| std::cmp::Reverse(a.total_loc()));
            }
            SortOrder::Commits => {
                self.projects
                    .sort_by_key(|a| std::cmp::Reverse(a.total_commits()));
            }
            SortOrder::LastModified => {
                self.projects
                    .sort_by_key(|a| std::cmp::Reverse(a.last_modified));
            }
        }
        // Reset selection after re-sorting.
        self.selected_project = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Local;

    use super::*;
    use crate::model::git::{CommitInfo, GitStats};
    use crate::model::project::ProjectType;
    use crate::model::stats::CodeStats;

    fn project_with_stats(name: &str, loc: usize, commits: usize, days_ago: i64) -> ProjectInfo {
        let mut project = ProjectInfo::new(
            name.to_string(),
            PathBuf::from(format!("/{name}")),
            ProjectType::Rust,
        );
        project.code_stats = Some(CodeStats {
            code_lines: loc,
            comment_lines: 0,
            blank_lines: 0,
            file_count: 1,
            languages: vec![],
            total_bytes: 0,
        });
        let timestamp = Local::now() - chrono::Duration::days(days_ago);
        project.git_stats = Some(GitStats {
            total_commits: commits,
            commits_last_30_days: 0,
            last_commit: Some(CommitInfo {
                hash: "abc".to_string(),
                author: "tester".to_string(),
                message: "test".to_string(),
                timestamp,
            }),
            recent_commits: vec![],
            top_contributors: vec![],
            daily_activity: vec![0; 364],
            current_branch: None,
        });
        project.last_modified = Some(timestamp);
        project
    }

    fn sorted_names(app: &App) -> Vec<String> {
        app.projects.iter().map(|p| p.name.clone()).collect()
    }

    fn press_sort(app: &mut App) {
        app.active_tab = ActiveTab::Projects;
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    }

    #[test]
    fn hiding_ignored_projects_clamps_the_selected_row() {
        let mut app = App::new(AppConfig::default());
        app.active_tab = ActiveTab::Projects;
        app.projects = vec![
            ProjectInfo::new(
                "active".to_string(),
                PathBuf::from("/active"),
                ProjectType::Rust,
            ),
            ProjectInfo {
                ignored: true,
                ..ProjectInfo::new(
                    "ignored".to_string(),
                    PathBuf::from("/ignored"),
                    ProjectType::Rust,
                )
            },
        ];
        app.show_ignored = true;
        app.selected_project = 1;

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));

        assert!(!app.show_ignored);
        assert_eq!(app.visible_projects().len(), 1);
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn sort_cycles_through_loc_commits_recent_and_name() {
        let mut app = App::new(AppConfig::default());
        app.projects = vec![
            project_with_stats("alpha", 100, 5, 10),
            project_with_stats("beta", 500, 1, 1),
            project_with_stats("gamma", 200, 20, 5),
        ];

        assert_eq!(app.sort_order, SortOrder::Name);
        press_sort(&mut app);
        assert_eq!(app.sort_order, SortOrder::Loc);
        assert_eq!(sorted_names(&app), vec!["beta", "gamma", "alpha"]);
        assert_eq!(app.selected_project, 0);

        press_sort(&mut app);
        assert_eq!(app.sort_order, SortOrder::Commits);
        assert_eq!(sorted_names(&app), vec!["gamma", "alpha", "beta"]);

        press_sort(&mut app);
        assert_eq!(app.sort_order, SortOrder::LastModified);
        assert_eq!(sorted_names(&app), vec!["beta", "gamma", "alpha"]);

        press_sort(&mut app);
        assert_eq!(app.sort_order, SortOrder::Name);
        assert_eq!(sorted_names(&app), vec!["alpha", "beta", "gamma"]);
    }

    fn app_with_scan_dir(scan_directory: &str) -> App {
        let mut config = AppConfig::default();
        config.general.scan_directory = scan_directory.to_string();
        config.general.scan_depth = 2;
        App::new(config)
    }

    #[test]
    fn missing_scan_directory_sets_scan_error() {
        let missing = std::env::temp_dir().join(format!(
            "peek-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should work")
                .as_nanos()
        ));
        let mut app = app_with_scan_dir(&missing.to_string_lossy());

        app.scan_and_analyze();

        assert!(app.projects.is_empty());
        assert_eq!(app.selected_project, 0);
        assert!(!app.is_loading);
        let err = app.scan_error.expect("scan_error should be set");
        assert!(err.contains("does not exist"), "unexpected: {err}");
        assert!(app.status_message.contains("does not exist"));
    }

    #[test]
    fn file_as_scan_directory_sets_scan_error() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        let file = dir.path().join("not-a-dir.toml");
        std::fs::write(&file, "x").expect("file should be written");
        let mut app = app_with_scan_dir(&file.to_string_lossy());

        app.scan_and_analyze();

        assert!(app.projects.is_empty());
        let err = app.scan_error.expect("scan_error should be set");
        assert!(err.contains("not a directory"), "unexpected: {err}");
    }

    #[test]
    fn empty_scan_directory_clears_error_and_reports_no_projects() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        let mut app = app_with_scan_dir(&dir.path().to_string_lossy());
        app.scan_error = Some("stale".to_string());

        app.scan_and_analyze();

        assert!(app.projects.is_empty());
        assert!(app.scan_error.is_none());
        assert!(!app.is_loading);
        assert!(app.status_message.contains("No projects found"));
    }
}
