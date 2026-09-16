use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Short summary of a single git commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub author: String,
    pub message: String,
    pub timestamp: DateTime<Local>,
}

impl CommitInfo {
    /// Human-friendly "time ago" string.
    pub fn time_ago(&self) -> String {
        let elapsed = Local::now().signed_duration_since(self.timestamp);
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

/// Contributor summary for a git repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    pub name: String,
    pub commit_count: usize,
}

/// Comprehensive git statistics for a project repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStats {
    /// Total number of commits in the current branch history.
    pub total_commits: usize,

    /// Commits made in the last 30 days.
    pub commits_last_30_days: usize,

    /// Most recent commit (HEAD).
    pub last_commit: Option<CommitInfo>,

    /// Up to 15 recent commits in reverse chronological order.
    pub recent_commits: Vec<CommitInfo>,

    /// Top contributors by commit count.
    pub top_contributors: Vec<Contributor>,

    /// Commit counts per day for the last 52 weeks (364 days).
    /// Index 0 is 364 days ago, index 363 is today.
    pub daily_activity: Vec<usize>,

    /// Active git branch name, if known.
    pub current_branch: Option<String>,
}

impl GitStats {
    /// Total commits in the past 52 weeks.
    #[allow(dead_code)]
    pub fn commits_last_52_weeks(&self) -> usize {
        self.daily_activity.iter().sum()
    }
}
