use std::path::PathBuf;
use std::sync::mpsc::Sender;

use crate::analyzer::{
    code_stats, git_analyzer, github_client::GitHubClient, scanner, todo_scanner,
};
use crate::config::AnalysisConfig;
use crate::model::github::GitHubData;
use crate::model::project::ProjectInfo;

/// Progress events sent from the scan worker thread to the UI thread.
#[derive(Debug)]
#[allow(dead_code)]
pub enum ScanEvent {
    Started {
        total: usize,
    },
    ProjectDone {
        index: usize,
        total: usize,
        name: String,
    },
    Finished {
        projects: Vec<ProjectInfo>,
    },
    Failed {
        message: String,
    },
}

/// Result sent from the GitHub worker thread to the UI thread.
#[derive(Debug)]
pub enum GithubEvent {
    Success(GitHubData),
    Failed(String),
}

/// Blocking scan + per-project analysis run on a worker thread.
///
/// Sends `Started`, per-project `ProjectDone`, and a terminal `Finished`.
/// Any panic-worthy input should be validated before spawning; failures inside
/// are reported as `Failed` instead of panicking the worker.
pub fn run_scan(scan_dir: PathBuf, depth: usize, analysis: AnalysisConfig, tx: Sender<ScanEvent>) {
    let projects = scanner::scan_projects(&scan_dir, depth, &analysis);
    let total = projects.len();
    let _ = tx.send(ScanEvent::Started { total });

    let mut projects = projects;
    for (i, project) in projects.iter_mut().enumerate() {
        if project.ignored {
            continue;
        }
        let _ = tx.send(ScanEvent::ProjectDone {
            index: i + 1,
            total,
            name: project.name.clone(),
        });
        project.code_stats = Some(code_stats::analyze(&project.path, &analysis));
        project.git_stats = git_analyzer::analyze_git(&project.path);
        project.todo_stats = Some(todo_scanner::scan_todos(
            &project.path,
            &analysis.exclude_dirs,
        ));
        if let Some(git) = &project.git_stats
            && let Some(last) = &git.last_commit
        {
            project.last_modified = Some(last.timestamp);
        }
    }

    let _ = tx.send(ScanEvent::Finished { projects });
}

/// Blocking GitHub fetch run on a worker thread (builds its own runtime).
pub fn run_github_fetch(username: String, token: Option<String>, tx: Sender<GithubEvent>) {
    let event = match GitHubClient::new(username, token) {
        Ok(client) => match tokio::runtime::Runtime::new() {
            Ok(rt) => match rt.block_on(client.fetch_all()) {
                Ok(data) => GithubEvent::Success(data),
                Err(e) => GithubEvent::Failed(e),
            },
            Err(e) => GithubEvent::Failed(format!("Failed to start async runtime: {e}")),
        },
        Err(e) => GithubEvent::Failed(e),
    };
    let _ = tx.send(event);
}
