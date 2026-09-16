use std::time::Duration;

use chrono::{DateTime, Local};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;

use crate::model::github::{GitHubData, GitHubEvent, GitHubRepo, GitHubUser};

/// HTTP client for querying the public GitHub REST API.
pub struct GitHubClient {
    client: reqwest::Client,
    username: String,
}

impl GitHubClient {
    pub fn new(username: String, token: Option<String>) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("rust-project-analyzer-tui/0.3.0"),
        );
        headers.insert(
            "Accept",
            HeaderValue::from_static("application/vnd.github+json"),
        );

        if let Some(tok) = token {
            let auth_val = format!("Bearer {tok}");
            if let Ok(val) = HeaderValue::from_str(&auth_val) {
                headers.insert(AUTHORIZATION, val);
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

        Ok(Self { client, username })
    }

    /// Fetch user profile, recent activity events, and top updated repos.
    pub async fn fetch_all(&self) -> Result<GitHubData, String> {
        let user = self.fetch_user().await?;
        let events = self.fetch_events().await.unwrap_or_default();
        let repos = self.fetch_repos().await.unwrap_or_default();

        Ok(GitHubData {
            user: Some(user),
            events,
            repos,
            last_fetched: Some(Local::now()),
            is_loading: false,
            error_message: None,
        })
    }

    async fn fetch_user(&self) -> Result<GitHubUser, String> {
        let url = format!("https://api.github.com/users/{}", self.username);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error fetching user: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(format!("GitHub user '{}' not found", self.username));
        } else if status == reqwest::StatusCode::FORBIDDEN {
            return Err(
                "GitHub API rate limit exceeded. Set GITHUB_TOKEN in env or config.toml"
                    .to_string(),
            );
        } else if !status.is_success() {
            return Err(format!("GitHub API error: HTTP {}", status));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse user JSON: {e}"))?;

        Ok(GitHubUser {
            login: json["login"].as_str().unwrap_or(&self.username).to_string(),
            name: json["name"].as_str().map(String::from),
            bio: json["bio"].as_str().map(String::from),
            public_repos: json["public_repos"].as_u64().unwrap_or(0) as usize,
            followers: json["followers"].as_u64().unwrap_or(0) as usize,
            following: json["following"].as_u64().unwrap_or(0) as usize,
            html_url: json["html_url"]
                .as_str()
                .unwrap_or("https://github.com")
                .to_string(),
        })
    }

    async fn fetch_events(&self) -> Result<Vec<GitHubEvent>, String> {
        let url = format!(
            "https://api.github.com/users/{}/events/public?per_page=25",
            self.username
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error fetching events: {e}"))?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let items: Vec<Value> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse events JSON: {e}"))?;

        let mut events = Vec::new();
        for item in items {
            let id = item["id"].as_str().unwrap_or("").to_string();
            let event_type = item["type"].as_str().unwrap_or("Event").to_string();
            let repo_name = item["repo"]["name"].as_str().unwrap_or("").to_string();

            let created_str = item["created_at"].as_str().unwrap_or("");
            let created_at: DateTime<Local> = DateTime::parse_from_rfc3339(created_str)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(|_| Local::now());

            // Extract brief human summary from payload
            let summary = match event_type.as_str() {
                "PushEvent" => {
                    let commits = item["payload"]["commits"].as_array();
                    if let Some(list) = commits {
                        if let Some(first) = list.first() {
                            let msg = first["message"]
                                .as_str()
                                .unwrap_or("")
                                .lines()
                                .next()
                                .unwrap_or("");
                            format!("Pushed {} commit(s): {}", list.len(), msg)
                        } else {
                            "Pushed commits".to_string()
                        }
                    } else {
                        "Pushed commits".to_string()
                    }
                }
                "PullRequestEvent" => {
                    let action = item["payload"]["action"].as_str().unwrap_or("action");
                    let title = item["payload"]["pull_request"]["title"]
                        .as_str()
                        .unwrap_or("");
                    format!("{action} PR: {title}")
                }
                "IssuesEvent" => {
                    let action = item["payload"]["action"].as_str().unwrap_or("action");
                    let title = item["payload"]["issue"]["title"].as_str().unwrap_or("");
                    format!("{action} issue: {title}")
                }
                "IssueCommentEvent" => {
                    let title = item["payload"]["issue"]["title"].as_str().unwrap_or("");
                    format!(
                        "Commented on #{}: {}",
                        item["payload"]["issue"]["number"], title
                    )
                }
                "WatchEvent" => "Starred repository".to_string(),
                "ForkEvent" => "Forked repository".to_string(),
                "CreateEvent" => {
                    let ref_type = item["payload"]["ref_type"].as_str().unwrap_or("item");
                    let ref_name = item["payload"]["ref"].as_str().unwrap_or("");
                    format!("Created {ref_type} {ref_name}")
                }
                _ => event_type.clone(),
            };

            events.push(GitHubEvent {
                id,
                event_type,
                repo_name,
                created_at,
                summary,
            });
        }

        Ok(events)
    }

    async fn fetch_repos(&self) -> Result<Vec<GitHubRepo>, String> {
        let url = format!(
            "https://api.github.com/users/{}/repos?sort=pushed&per_page=8",
            self.username
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error fetching repos: {e}"))?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let items: Vec<Value> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse repos JSON: {e}"))?;

        let mut repos = Vec::new();
        for item in items {
            let name = item["name"].as_str().unwrap_or("").to_string();
            let description = item["description"].as_str().map(String::from);
            let stars = item["stargazers_count"].as_u64().unwrap_or(0) as usize;
            let forks = item["forks_count"].as_u64().unwrap_or(0) as usize;
            let open_issues = item["open_issues_count"].as_u64().unwrap_or(0) as usize;
            let language = item["language"].as_str().map(String::from);
            let html_url = item["html_url"].as_str().unwrap_or("").to_string();

            let updated_str = item["pushed_at"].as_str().unwrap_or("");
            let updated_at = DateTime::parse_from_rfc3339(updated_str)
                .ok()
                .map(|dt| dt.with_timezone(&Local));

            repos.push(GitHubRepo {
                name,
                description,
                stars,
                forks,
                open_issues,
                language,
                html_url,
                updated_at,
            });
        }

        Ok(repos)
    }
}
