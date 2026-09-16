use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// GitHub user profile summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub public_repos: usize,
    pub followers: usize,
    pub following: usize,
    pub html_url: String,
}

/// An individual GitHub activity event (from /users/{username}/events).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEvent {
    pub id: String,
    pub event_type: String,
    pub repo_name: String,
    pub created_at: DateTime<Local>,
    pub summary: String,
}

impl GitHubEvent {
    pub fn icon(&self) -> &'static str {
        match self.event_type.as_str() {
            "PushEvent" => "🚀 Push",
            "PullRequestEvent" => "🔀 PR",
            "IssuesEvent" => "⚠️ Issue",
            "IssueCommentEvent" => "💬 Comment",
            "WatchEvent" => "⭐ Star",
            "ForkEvent" => "🍴 Fork",
            "CreateEvent" => "✨ Create",
            "DeleteEvent" => "🗑️ Delete",
            "ReleaseEvent" => "🏷️ Release",
            _ => "📌 Event",
        }
    }

    pub fn time_ago(&self) -> String {
        let elapsed = Local::now().signed_duration_since(self.created_at);
        if elapsed.num_minutes() < 1 {
            "just now".to_string()
        } else if elapsed.num_hours() < 1 {
            format!("{}m ago", elapsed.num_minutes())
        } else if elapsed.num_days() < 1 {
            format!("{}h ago", elapsed.num_hours())
        } else if elapsed.num_weeks() < 1 {
            format!("{}d ago", elapsed.num_days())
        } else {
            format!("{}w ago", elapsed.num_weeks())
        }
    }
}

/// A repository owned or contributed to by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub name: String,
    pub description: Option<String>,
    pub stars: usize,
    pub forks: usize,
    pub open_issues: usize,
    pub language: Option<String>,
    pub html_url: String,
    pub updated_at: Option<DateTime<Local>>,
}

/// Aggregated state for the GitHub activity view.
#[derive(Debug, Clone, Default)]
pub struct GitHubData {
    pub user: Option<GitHubUser>,
    pub events: Vec<GitHubEvent>,
    pub repos: Vec<GitHubRepo>,
    #[allow(dead_code)]
    pub last_fetched: Option<DateTime<Local>>,
    #[allow(dead_code)]
    pub is_loading: bool,
    pub error_message: Option<String>,
}
