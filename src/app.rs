use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::analyzer::{code_stats, git_analyzer, scanner};
use crate::config::AppConfig;
use crate::model::project::ProjectInfo;

/// Which tab / view is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    Projects,
    Help,
}

impl ActiveTab {
    pub const ALL: [Self; 3] = [Self::Dashboard, Self::Projects, Self::Help];

    pub fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Projects => 1,
            Self::Help => 2,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Dashboard,
            1 => Self::Projects,
            _ => Self::Help,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Projects => "Projects",
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
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let default_tab = match config.display.default_tab.as_str() {
            "projects" => ActiveTab::Projects,
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
        }
    }

    /// Run the initial project scan, code analysis, and git analysis.
    pub fn scan_and_analyze(&mut self) {
        self.is_loading = true;
        self.status_message = "Scanning projects…".to_string();

        let scan_dir = self.config.resolved_scan_directory();
        let depth = self.config.general.scan_depth;

        self.projects = scanner::scan_projects(&scan_dir, depth, &self.config.analysis);

        // Analyze code stats and git history for each non-ignored project.
        let total = self.projects.len();
        for (i, project) in self.projects.iter_mut().enumerate() {
            if project.ignored {
                continue;
            }
            self.status_message = format!("Analyzing [{}/{}] {}…", i + 1, total, project.name);
            project.code_stats = Some(code_stats::analyze(&project.path, &self.config.analysis));
            project.git_stats = git_analyzer::analyze_git(&project.path);
            if let Some(git) = &project.git_stats
                && let Some(last) = &git.last_commit
            {
                project.last_modified = Some(last.timestamp);
            }
        }

        self.apply_sort();
        self.is_loading = false;
        let active = self.visible_projects().len();
        self.status_message = format!("Found {active} projects");
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
            KeyCode::Char('3') | KeyCode::Char('?') => {
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
