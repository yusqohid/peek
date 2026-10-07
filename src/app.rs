use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::cache::AnalysisCache;
use crate::config::AppConfig;
use crate::model::github::GitHubData;
use crate::model::project::ProjectInfo;
use crate::worker::{GithubEvent, ScanEvent, run_github_fetch, run_scan};

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
    pub scan_rx: Option<std::sync::mpsc::Receiver<ScanEvent>>,
    pub github_rx: Option<std::sync::mpsc::Receiver<GithubEvent>>,
    pub cache: AnalysisCache,
    pub last_scan: Option<std::time::Instant>,
    pub search_query: String,
    pub is_searching: bool,
    pub sort_reversed: bool,
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
            scan_rx: None,
            github_rx: None,
            cache: AnalysisCache::default(),
            last_scan: None,
            search_query: String::new(),
            is_searching: false,
            sort_reversed: false,
        }
    }

    /// Width below which the project table switches to a compact layout.
    pub const COMPACT_WIDTH: u16 = 80;

    pub fn use_compact_layout(width: u16) -> bool {
        width < Self::COMPACT_WIDTH
    }

    /// Whether an automatic refresh is due. Manual scans update the timer.
    pub fn should_auto_refresh(&self, now: std::time::Instant) -> bool {
        let interval = self.config.general.refresh_interval_secs;
        if interval == 0 || self.is_loading || self.scan_rx.is_some() {
            return false;
        }
        match self.last_scan {
            Some(last) => now.duration_since(last).as_secs() >= interval,
            None => false,
        }
    }

    /// Start a background scan; returns immediately without blocking the UI.
    ///
    /// Validation failures are applied synchronously. A previous in-flight
    /// scan is abandoned by replacing its channel; its late messages are
    /// ignored once the receiver is dropped. The detail view is preserved so
    /// auto-refresh never yanks the user out of what they are reading —
    /// callers that want a fresh list (manual `r`) clear it themselves.
    pub fn start_scan(&mut self) {
        let scan_dir = self.config.resolved_scan_directory();
        if let Err(message) = check_scan_directory(&scan_dir) {
            self.projects.clear();
            self.selected_project = 0;
            self.is_loading = false;
            self.scan_rx = None;
            self.scan_error = Some(message.clone());
            self.status_message = message;
            return;
        }

        self.is_loading = true;
        self.scan_error = None;
        self.status_message = format!("Starting scan in {}…", scan_dir.display());
        self.last_scan = Some(std::time::Instant::now());

        let (tx, rx) = std::sync::mpsc::channel();
        self.scan_rx = Some(rx);
        let depth = self.config.general.scan_depth;
        let analysis = self.config.analysis.clone();
        let cache = self.cache.clone();
        std::thread::spawn(move || run_scan(scan_dir, depth, analysis, cache, tx));
    }

    /// Drain finished background work without blocking. Call once per frame.
    pub fn poll_background(&mut self) {
        let mut finished: Option<(Vec<ProjectInfo>, AnalysisCache, usize)> = None;
        let mut scan_failed: Option<String> = None;
        let mut scan_disconnected = false;

        if let Some(rx) = self.scan_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(ScanEvent::Started { total }) => {
                        self.status_message = format!("Scanning… found {total} candidates");
                    }
                    Ok(ScanEvent::ProjectDone { index, total, name }) => {
                        self.status_message = format!("Analyzing [{index}/{total}] {name}…");
                    }
                    Ok(ScanEvent::Finished {
                        projects,
                        cache,
                        cached,
                    }) => {
                        finished = Some((projects, cache, cached));
                        break;
                    }
                    Ok(ScanEvent::Failed { message }) => {
                        scan_failed = Some(message);
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        scan_disconnected = true;
                        break;
                    }
                }
            }
        }

        if let Some((projects, cache, cached)) = finished {
            self.projects = projects;
            self.cache = cache;
            self.apply_sort();
            if let Some(idx) = self.detail_project
                && idx >= self.visible_projects().len()
            {
                self.detail_project = None;
            }
            self.is_loading = false;
            self.scan_rx = None;
            self.scan_error = None;
            self.last_scan = Some(std::time::Instant::now());
            if self.projects.is_empty() {
                let dir = self.config.resolved_scan_directory();
                self.status_message = format!("No projects found in {}", dir.display());
            } else {
                let active = self.visible_projects().len();
                self.status_message = if cached > 0 {
                    format!("Found {active} projects ({cached} cached)")
                } else {
                    format!("Found {active} projects")
                };
            }
        } else if let Some(message) = scan_failed {
            self.projects.clear();
            self.selected_project = 0;
            self.is_loading = false;
            self.scan_rx = None;
            self.scan_error = Some(message.clone());
            self.status_message = message;
        } else if scan_disconnected && self.is_loading {
            self.is_loading = false;
            self.scan_rx = None;
            self.scan_error = Some("Background scan stopped unexpectedly.".to_string());
            self.status_message = "Background scan stopped unexpectedly.".to_string();
        }

        if let Some(rx) = self.github_rx.as_ref() {
            match rx.try_recv() {
                Ok(GithubEvent::Success(data)) => {
                    self.github_data = *data;
                    self.github_rx = None;
                }
                Ok(GithubEvent::Failed(message)) => {
                    self.github_data.error_message = Some(message);
                    self.github_data.is_loading = false;
                    self.github_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.github_data.is_loading = false;
                    self.github_rx = None;
                }
            }
        }
    }

    /// Start a background GitHub fetch; returns immediately.
    pub fn start_github_fetch(&mut self) {
        let username = self.config.github.username.trim().to_string();
        if username.is_empty() {
            return;
        }

        self.github_data.is_loading = true;
        self.github_data.error_message = None;
        let (tx, rx) = std::sync::mpsc::channel();
        self.github_rx = Some(rx);
        let token = self.config.github.resolved_token();
        std::thread::spawn(move || run_github_fetch(username, token, tx));
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

    /// Projects filtered by the ignored flag and the search query.
    pub fn visible_projects(&self) -> Vec<&ProjectInfo> {
        let query = self.search_query.to_lowercase();
        self.projects
            .iter()
            .filter(|p| self.show_ignored || !p.ignored)
            .filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query))
            .collect()
    }

    /// Keep the selection inside the visible list after filtering.
    fn clamp_selection(&mut self) {
        let len = self.visible_projects().len();
        self.selected_project = self.selected_project.min(len.saturating_sub(1));
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
                self.detail_project = None;
                self.start_scan();
                self.start_github_fetch();
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
        // Search mode captures most keys as query text.
        if self.is_searching {
            match key.code {
                KeyCode::Esc => {
                    self.search_query.clear();
                    self.is_searching = false;
                    self.clamp_selection();
                    self.status_message = "Search cleared".to_string();
                }
                KeyCode::Enter => {
                    self.is_searching = false;
                    self.clamp_selection();
                    let n = self.visible_projects().len();
                    self.status_message = if self.search_query.is_empty() {
                        "Search cleared".to_string()
                    } else {
                        format!("Filter '{}' — {n} match(es)", self.search_query)
                    };
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.clamp_selection();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.clamp_selection();
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('/') => {
                self.is_searching = true;
                self.status_message = "Type to filter, Enter to apply, Esc to clear".to_string();
                return;
            }
            KeyCode::Char('d') => {
                self.sort_reversed = !self.sort_reversed;
                self.apply_sort();
                self.status_message = format!(
                    "Sorted by {} ({})",
                    self.sort_order.label(),
                    self.direction_label()
                );
                return;
            }
            _ => {}
        }

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
                self.status_message = format!(
                    "Sorted by {} ({})",
                    self.sort_order.label(),
                    self.direction_label()
                );
            }
            KeyCode::Char('i') => {
                self.show_ignored = !self.show_ignored;
                self.clamp_selection();
                self.status_message = if self.show_ignored {
                    "Showing ignored projects".to_string()
                } else {
                    "Hiding ignored projects".to_string()
                };
            }
            _ => {}
        }
    }

    /// Human direction for the current sort: Name defaults to asc, metrics to desc.
    pub fn direction_label(&self) -> &'static str {
        let default_asc = self.sort_order == SortOrder::Name;
        if self.sort_reversed == default_asc {
            "desc"
        } else {
            "asc"
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
        if self.sort_reversed {
            self.projects.reverse();
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

    /// Drive a real background scan to completion (bounded wait for tests).
    fn run_scan_to_completion(app: &mut App) {
        for _ in 0..500 {
            app.poll_background();
            if app.scan_rx.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("background scan did not finish in time");
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

        app.start_scan();

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

        app.start_scan();

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

        app.start_scan();
        run_scan_to_completion(&mut app);

        assert!(app.projects.is_empty());
        assert!(app.scan_error.is_none());
        assert!(!app.is_loading);
        assert!(app.status_message.contains("No projects found"));
    }

    #[test]
    fn poll_applies_finished_scan_and_sorts() {
        let mut app = App::new(AppConfig::default());
        app.is_loading = true;
        let (tx, rx) = std::sync::mpsc::channel();
        app.scan_rx = Some(rx);
        tx.send(crate::worker::ScanEvent::Started { total: 2 })
            .expect("send should work");
        tx.send(crate::worker::ScanEvent::ProjectDone {
            index: 1,
            total: 2,
            name: "beta".to_string(),
        })
        .expect("send should work");
        tx.send(crate::worker::ScanEvent::Finished {
            projects: vec![
                project_with_stats("beta", 10, 1, 1),
                project_with_stats("alpha", 10, 1, 1),
            ],
            cache: crate::cache::AnalysisCache::default(),
            cached: 0,
        })
        .expect("send should work");

        app.poll_background();

        assert_eq!(sorted_names(&app), vec!["alpha", "beta"]);
        assert!(!app.is_loading);
        assert!(app.scan_rx.is_none());
        assert!(app.scan_error.is_none());
        assert!(app.status_message.contains("Found 2 projects"));
    }

    #[test]
    fn background_finish_preserves_detail_view() {
        let mut app = App::new(AppConfig::default());
        app.is_loading = true;
        app.detail_project = Some(0);
        let (tx, rx) = std::sync::mpsc::channel();
        app.scan_rx = Some(rx);
        tx.send(crate::worker::ScanEvent::Finished {
            projects: vec![project_with_stats("alpha", 10, 1, 1)],
            cache: crate::cache::AnalysisCache::default(),
            cached: 0,
        })
        .expect("send should work");

        app.poll_background();

        assert_eq!(app.detail_project, Some(0));
    }

    #[test]
    fn background_finish_drops_out_of_range_detail() {
        let mut app = App::new(AppConfig::default());
        app.is_loading = true;
        app.detail_project = Some(5);
        let (tx, rx) = std::sync::mpsc::channel();
        app.scan_rx = Some(rx);
        tx.send(crate::worker::ScanEvent::Finished {
            projects: vec![project_with_stats("alpha", 10, 1, 1)],
            cache: crate::cache::AnalysisCache::default(),
            cached: 0,
        })
        .expect("send should work");

        app.poll_background();

        assert!(app.detail_project.is_none());
    }

    #[test]
    fn start_scan_preserves_detail_view() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        let mut app = app_with_scan_dir(&dir.path().to_string_lossy());
        app.detail_project = Some(0);

        app.start_scan();

        assert_eq!(app.detail_project, Some(0));
        // Drain the worker so the test never leaks a thread.
        for _ in 0..500 {
            app.poll_background();
            if app.scan_rx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn poll_applies_failed_scan_as_error() {
        let mut app = App::new(AppConfig::default());
        app.is_loading = true;
        let (tx, rx) = std::sync::mpsc::channel();
        app.scan_rx = Some(rx);
        tx.send(crate::worker::ScanEvent::Failed {
            message: "boom".to_string(),
        })
        .expect("send should work");

        app.poll_background();

        assert!(app.projects.is_empty());
        assert!(!app.is_loading);
        assert_eq!(app.scan_error.as_deref(), Some("boom"));
    }

    #[test]
    fn poll_applies_github_success_and_failure() {
        let mut app = App::new(AppConfig::default());
        let (tx, rx) = std::sync::mpsc::channel();
        app.github_rx = Some(rx);
        tx.send(crate::worker::GithubEvent::Failed("offline".to_string()))
            .expect("send should work");

        app.poll_background();

        assert_eq!(app.github_data.error_message.as_deref(), Some("offline"));
        assert!(!app.github_data.is_loading);
        assert!(app.github_rx.is_none());
    }

    #[test]
    fn start_scan_rejects_missing_directory_without_spawning() {
        let mut app = app_with_scan_dir("/definitely/not/here/peek-test");
        app.start_scan();

        assert!(app.scan_rx.is_none());
        assert!(!app.is_loading);
        assert!(app.scan_error.is_some());
    }

    #[test]
    fn auto_refresh_respects_interval_and_loading() {
        let mut config = AppConfig::default();
        config.general.refresh_interval_secs = 60;
        let mut app = App::new(config);
        // Never scanned -> no auto refresh yet.
        assert!(!app.should_auto_refresh(std::time::Instant::now()));

        app.last_scan = Some(std::time::Instant::now() - std::time::Duration::from_secs(61));
        assert!(app.should_auto_refresh(std::time::Instant::now()));

        app.is_loading = true;
        assert!(!app.should_auto_refresh(std::time::Instant::now()));
        app.is_loading = false;

        app.config.general.refresh_interval_secs = 0;
        assert!(!app.should_auto_refresh(std::time::Instant::now()));
    }

    fn app_with_named_projects() -> App {
        let mut app = App::new(AppConfig::default());
        app.active_tab = ActiveTab::Projects;
        app.projects = vec![
            project_with_stats("alpha", 100, 5, 10),
            project_with_stats("beta", 500, 1, 1),
            project_with_stats("gamma", 200, 20, 5),
        ];
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn search_filters_by_name_and_clamps_selection() {
        let mut app = app_with_named_projects();
        app.selected_project = 2;

        press(&mut app, KeyCode::Char('/'));
        assert!(app.is_searching);
        for c in "alp".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.search_query, "alp");

        let visible: Vec<_> = app
            .visible_projects()
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(visible, vec!["alpha"]);
        assert_eq!(app.selected_project, 0);

        press(&mut app, KeyCode::Enter);
        assert!(!app.is_searching);
        assert_eq!(app.visible_projects().len(), 1);

        press(&mut app, KeyCode::Esc);
        // Esc outside search mode is ignored; query stays until search Esc.
        assert_eq!(app.search_query, "alp");
    }

    #[test]
    fn search_escape_clears_query() {
        let mut app = app_with_named_projects();
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('b'));
        assert_eq!(app.visible_projects().len(), 1);

        press(&mut app, KeyCode::Esc);
        assert!(!app.is_searching);
        assert!(app.search_query.is_empty());
        assert_eq!(app.visible_projects().len(), 3);
    }

    #[test]
    fn direction_toggle_reverses_order() {
        let mut app = app_with_named_projects();
        assert_eq!(app.direction_label(), "asc");

        press(&mut app, KeyCode::Char('d'));
        assert!(app.sort_reversed);
        assert_eq!(app.direction_label(), "desc");
        assert_eq!(sorted_names(&app), vec!["gamma", "beta", "alpha"]);

        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.sort_order, SortOrder::Loc);
        // Loc default is desc; reversed makes it asc.
        assert_eq!(app.direction_label(), "asc");
        assert_eq!(sorted_names(&app), vec!["alpha", "gamma", "beta"]);
    }

    #[test]
    fn compact_layout_threshold() {
        assert!(App::use_compact_layout(79));
        assert!(!App::use_compact_layout(80));
        assert!(!App::use_compact_layout(120));
    }
}
