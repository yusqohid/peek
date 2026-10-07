use std::time::Duration;

use chrono::{DateTime, Local};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;

use crate::model::github::{GitHubData, GitHubEvent, GitHubRepo, GitHubUser};

/// HTTP client for querying the public GitHub REST API.
pub struct GitHubClient {
    client: reqwest::Client,
    username: String,
    base_url: String,
}

const DEFAULT_BASE_URL: &str = "https://api.github.com";

impl GitHubClient {
    pub fn new(username: String, token: Option<String>) -> Result<Self, String> {
        Self::with_base_url(username, token, DEFAULT_BASE_URL.to_string())
    }

    pub(crate) fn with_base_url(
        username: String,
        token: Option<String>,
        base_url: String,
    ) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("peek-tui/", env!("CARGO_PKG_VERSION"))),
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

        Ok(Self {
            client,
            username,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
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

    pub(crate) async fn fetch_user(&self) -> Result<GitHubUser, String> {
        let url = format!("{}/users/{}", self.base_url, self.username);
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

        Ok(parse_user(&json, &self.username))
    }

    pub(crate) async fn fetch_events(&self) -> Result<Vec<GitHubEvent>, String> {
        let url = format!(
            "{}/users/{}/events/public?per_page=25",
            self.base_url, self.username
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

        Ok(parse_events(&items))
    }

    pub(crate) async fn fetch_repos(&self) -> Result<Vec<GitHubRepo>, String> {
        let url = format!(
            "{}/users/{}/repos?sort=pushed&per_page=8",
            self.base_url, self.username
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

        Ok(parse_repos(&items))
    }
}

pub(crate) fn parse_user(json: &Value, fallback_login: &str) -> GitHubUser {
    GitHubUser {
        login: json["login"].as_str().unwrap_or(fallback_login).to_string(),
        name: json["name"].as_str().map(String::from),
        bio: json["bio"].as_str().map(String::from),
        public_repos: json["public_repos"].as_u64().unwrap_or(0) as usize,
        followers: json["followers"].as_u64().unwrap_or(0) as usize,
        following: json["following"].as_u64().unwrap_or(0) as usize,
        html_url: json["html_url"]
            .as_str()
            .unwrap_or("https://github.com")
            .to_string(),
    }
}

pub(crate) fn parse_events(items: &[Value]) -> Vec<GitHubEvent> {
    items.iter().map(parse_event).collect()
}

pub(crate) fn parse_event(item: &Value) -> GitHubEvent {
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

    GitHubEvent {
        id,
        event_type,
        repo_name,
        created_at,
        summary,
    }
}

pub(crate) fn parse_repos(items: &[Value]) -> Vec<GitHubRepo> {
    items.iter().map(parse_repo).collect()
}

pub(crate) fn parse_repo(item: &Value) -> GitHubRepo {
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

    GitHubRepo {
        name,
        description,
        stars,
        forks,
        open_issues,
        language,
        html_url,
        updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    fn reason_phrase(status: u16) -> &'static str {
        match status {
            200 => "OK",
            404 => "Not Found",
            403 => "Forbidden",
            500 => "Internal Server Error",
            _ => "Error",
        }
    }

    /// Serve exactly `responses` HTTP requests; route by request path.
    fn start_mock(
        user: (u16, &str),
        events: (u16, &str),
        repos: (u16, &str),
        expected_requests: usize,
    ) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock should bind");
        let addr = listener.local_addr().expect("mock should have addr");
        let user = (user.0, user.1.to_string());
        let events = (events.0, events.1.to_string());
        let repos = (repos.0, repos.1.to_string());

        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(expected_requests) {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let mut buf = vec![0u8; 8192];
                let Ok(n) = stream.read(&mut buf) else {
                    break;
                };
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = request
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .to_string();

                let (status, body) = if path.contains("/events/") {
                    &events
                } else if path.contains("/repos") {
                    &repos
                } else {
                    &user
                };
                let response = format!(
                    "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    reason_phrase(*status),
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        (format!("http://{addr}"), handle)
    }

    fn client_with(base_url: &str) -> GitHubClient {
        GitHubClient::with_base_url("octocat".to_string(), None, base_url.to_string())
            .expect("client should build")
    }

    const USER_JSON: &str = r#"{"login":"octocat","name":"The Octocat","bio":"hi","public_repos":2,"followers":3,"following":1,"html_url":"https://github.com/octocat"}"#;
    const EVENTS_JSON: &str = r#"[{"id":"1","type":"PushEvent","repo":{"name":"octocat/hello"},"created_at":"2024-01-02T03:04:05Z","payload":{"commits":[{"message":"first line\nsecond"}]}}]"#;
    const REPOS_JSON: &str = r#"[{"name":"hello","description":"demo","stargazers_count":5,"forks_count":1,"open_issues_count":2,"language":"Rust","html_url":"https://github.com/octocat/hello","pushed_at":"2024-01-03T00:00:00Z"}]"#;

    #[tokio::test]
    async fn fetches_user_events_and_repos_from_mock_server() {
        let (base, handle) = start_mock((200, USER_JSON), (200, EVENTS_JSON), (200, REPOS_JSON), 3);
        let client = client_with(&base);

        let data = client.fetch_all().await.expect("fetch_all should succeed");

        assert_eq!(data.user.expect("user should exist").login, "octocat");
        assert_eq!(data.events.len(), 1);
        assert_eq!(data.events[0].repo_name, "octocat/hello");
        assert!(data.events[0].summary.contains("Pushed 1 commit(s)"));
        assert_eq!(data.repos.len(), 1);
        assert_eq!(data.repos[0].name, "hello");
        assert_eq!(data.repos[0].stars, 5);

        handle.join().expect("mock should finish");
    }

    #[tokio::test]
    async fn returns_not_found_when_user_missing() {
        let (base, handle) = start_mock(
            (404, r#"{"message":"Not Found"}"#),
            (200, "[]"),
            (200, "[]"),
            1,
        );
        let client = client_with(&base);

        let err = client.fetch_user().await.expect_err("should be not found");

        assert!(err.contains("not found"), "unexpected error: {err}");
        handle.join().expect("mock should finish");
    }

    #[tokio::test]
    async fn returns_rate_limit_hint_on_forbidden() {
        let (base, handle) = start_mock(
            (403, r#"{"message":"API rate limit exceeded"}"#),
            (200, "[]"),
            (200, "[]"),
            1,
        );
        let client = client_with(&base);

        let err = client
            .fetch_user()
            .await
            .expect_err("should be rate limited");

        assert!(err.contains("rate limit"), "unexpected error: {err}");
        handle.join().expect("mock should finish");
    }

    #[tokio::test]
    async fn returns_empty_events_and_repos_when_server_errors() {
        let (base, handle) = start_mock((200, USER_JSON), (500, "boom"), (500, "boom"), 3);
        let client = client_with(&base);

        let data = client.fetch_all().await.expect("fetch_all should succeed");

        assert!(data.events.is_empty());
        assert!(data.repos.is_empty());
        handle.join().expect("mock should finish");
    }

    #[test]
    fn parses_incomplete_json_with_defaults() {
        let user = parse_user(&serde_json::json!({}), "fallback");
        assert_eq!(user.login, "fallback");
        assert_eq!(user.public_repos, 0);
        assert_eq!(user.html_url, "https://github.com");

        let events = parse_events(&[serde_json::json!({})]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "Event");
        assert_eq!(events[0].summary, "Event");

        let repos = parse_repos(&[serde_json::json!({"name": "x"})]);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].stars, 0);
        assert!(repos[0].updated_at.is_none());
    }
}
