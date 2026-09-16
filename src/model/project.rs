use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::git::GitStats;
use super::stats::CodeStats;

/// Type of project detected by scanning for marker files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    NodeJs,
    Python,
    Go,
    Java,
    CSharp,
    Ruby,
    Php,
    Unknown,
}

impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let icon = match self {
            Self::Rust => "🦀 Rust",
            Self::NodeJs => "📦 Node.js",
            Self::Python => "🐍 Python",
            Self::Go => "🐹 Go",
            Self::Java => "☕ Java",
            Self::CSharp => "🔷 C#",
            Self::Ruby => "💎 Ruby",
            Self::Php => "🐘 PHP",
            Self::Unknown => "❓ Unknown",
        };
        write!(f, "{icon}")
    }
}

impl ProjectType {
    /// Short emoji icon for compact table display.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Rust => "🦀",
            Self::NodeJs => "📦",
            Self::Python => "🐍",
            Self::Go => "🐹",
            Self::Java => "☕",
            Self::CSharp => "🔷",
            Self::Ruby => "💎",
            Self::Php => "🐘",
            Self::Unknown => "❓",
        }
    }
}

/// All the information we know about a single detected project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Human-readable project name (directory basename).
    pub name: String,

    /// Absolute path to the project root.
    pub path: PathBuf,

    /// Detected project type.
    pub project_type: ProjectType,

    /// Code statistics (populated after analysis).
    pub code_stats: Option<CodeStats>,

    /// Git statistics (populated if the project is a git repo).
    pub git_stats: Option<GitStats>,

    /// When the project was last modified (from filesystem or git).
    pub last_modified: Option<DateTime<Local>>,

    /// Whether this project is ignored/disabled by the user.
    pub ignored: bool,
}

impl ProjectInfo {
    pub fn new(name: String, path: PathBuf, project_type: ProjectType) -> Self {
        Self {
            name,
            path,
            project_type,
            code_stats: None,
            git_stats: None,
            last_modified: None,
            ignored: false,
        }
    }

    /// Total lines of code, or 0 if not yet analyzed.
    pub fn total_loc(&self) -> usize {
        self.code_stats.as_ref().map_or(0, |s| s.code_lines)
    }

    /// Total git commits, or 0 if not a git repo or not yet analyzed.
    pub fn total_commits(&self) -> usize {
        self.git_stats.as_ref().map_or(0, |g| g.total_commits)
    }

    /// Human-friendly "time ago" string for last_modified.
    pub fn last_activity_display(&self) -> String {
        let Some(dt) = self.last_modified else {
            return "unknown".to_string();
        };
        let elapsed = Local::now().signed_duration_since(dt);
        if elapsed.num_minutes() < 1 {
            "just now".to_string()
        } else if elapsed.num_hours() < 1 {
            format!("{}m ago", elapsed.num_minutes())
        } else if elapsed.num_days() < 1 {
            format!("{}h ago", elapsed.num_hours())
        } else if elapsed.num_weeks() < 1 {
            format!("{}d ago", elapsed.num_days())
        } else if elapsed.num_weeks() < 5 {
            format!("{}w ago", elapsed.num_weeks())
        } else {
            format!("{}mo ago", elapsed.num_days() / 30)
        }
    }
}
