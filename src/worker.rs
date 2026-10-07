use std::path::PathBuf;
use std::sync::mpsc::Sender;

use crate::analyzer::{
    code_stats, git_analyzer, github_client::GitHubClient, scanner, todo_scanner,
};
use crate::cache::{AnalysisCache, cache_key_for, git_head_oid, newest_source_mtime};
use crate::config::AnalysisConfig;
use crate::model::github::GitHubData;
use crate::model::project::ProjectInfo;

/// Progress events sent from the scan worker thread to the UI thread.
#[derive(Debug)]
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
        cache: AnalysisCache,
        cached: usize,
    },
    #[allow(dead_code)]
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
pub fn run_scan(
    scan_dir: PathBuf,
    depth: usize,
    analysis: AnalysisConfig,
    cache: AnalysisCache,
    tx: Sender<ScanEvent>,
) {
    let projects = scanner::scan_projects(&scan_dir, depth, &analysis);
    let total = projects.len();
    let _ = tx.send(ScanEvent::Started { total });

    let mut projects = projects;
    let mut cache = cache;
    let mut cached = 0usize;
    for (i, project) in projects.iter_mut().enumerate() {
        if project.ignored {
            continue;
        }
        let _ = tx.send(ScanEvent::ProjectDone {
            index: i + 1,
            total,
            name: project.name.clone(),
        });

        let marker_modified = project.last_modified;
        let git_head = git_head_oid(&project.path);
        let source_modified = newest_source_mtime(&project.path, &analysis.exclude_dirs);
        let key = cache_key_for(marker_modified, git_head, source_modified);
        if let Some(hit) = cache.get(&project.path, &key) {
            project.code_stats = hit.code_stats.clone();
            project.git_stats = hit.git_stats.clone();
            project.todo_stats = hit.todo_stats.clone();
            project.last_modified = hit.last_modified;
            cached += 1;
            continue;
        }

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
        cache.insert(
            project.path.clone(),
            key,
            project.code_stats.clone(),
            project.git_stats.clone(),
            project.todo_stats.clone(),
            project.last_modified,
        );
    }

    let _ = tx.send(ScanEvent::Finished {
        projects,
        cache,
        cached,
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AnalysisConfig;

    fn drain(
        tx_rx: std::sync::mpsc::Receiver<ScanEvent>,
    ) -> (Vec<ProjectInfo>, AnalysisCache, usize) {
        let mut finished = None;
        for ev in tx_rx {
            if let ScanEvent::Finished {
                projects,
                cache,
                cached,
            } = ev
            {
                finished = Some((projects, cache, cached));
                break;
            }
        }
        finished.expect("Finished should be sent")
    }

    #[test]
    fn second_scan_reuses_cache_for_unchanged_project() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        let proj = dir.path().join("demo");
        std::fs::create_dir_all(&proj).expect("proj should be created");
        std::fs::write(proj.join("Cargo.toml"), "[package]\nname=\"demo\"")
            .expect("marker should be written");
        std::fs::write(proj.join("main.rs"), "fn main() {}\n").expect("source should be written");
        let analysis = AnalysisConfig {
            exclude_dirs: vec![],
            ignored_projects: vec![],
        };

        let (tx1, rx1) = std::sync::mpsc::channel();
        run_scan(
            dir.path().to_path_buf(),
            3,
            analysis.clone(),
            AnalysisCache::default(),
            tx1,
        );
        let (projects1, cache1, cached1) = drain(rx1);
        assert_eq!(projects1.len(), 1);
        assert_eq!(cached1, 0);
        assert_eq!(cache1.len(), 1);

        let (tx2, rx2) = std::sync::mpsc::channel();
        run_scan(dir.path().to_path_buf(), 3, analysis, cache1, tx2);
        let (projects2, cache2, cached2) = drain(rx2);
        assert_eq!(projects2.len(), 1);
        assert_eq!(cached2, 1);
        assert_eq!(cache2.len(), 1);
        assert_eq!(
            projects2[0].code_stats.as_ref().map(|s| s.code_lines),
            projects1[0].code_stats.as_ref().map(|s| s.code_lines)
        );
    }

    #[test]
    fn edited_source_invalidates_cache_and_updates_stats() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        let proj = dir.path().join("demo");
        std::fs::create_dir_all(&proj).expect("proj should be created");
        std::fs::write(proj.join("Cargo.toml"), "[package]\nname=\"demo\"")
            .expect("marker should be written");
        std::fs::write(proj.join("main.rs"), "fn main() {}\n").expect("source should be written");
        let analysis = AnalysisConfig {
            exclude_dirs: vec![],
            ignored_projects: vec![],
        };

        let (tx1, rx1) = std::sync::mpsc::channel();
        run_scan(
            dir.path().to_path_buf(),
            3,
            analysis.clone(),
            AnalysisCache::default(),
            tx1,
        );
        let (projects1, cache1, _) = drain(rx1);
        let loc_before = projects1[0]
            .code_stats
            .as_ref()
            .expect("stats should exist")
            .code_lines;

        // Edit source without touching the marker or committing.
        let extra = "fn f() {}\n".repeat(100);
        std::fs::write(proj.join("main.rs"), format!("fn main() {{}}\n{extra}"))
            .expect("edit should be written");

        let (tx2, rx2) = std::sync::mpsc::channel();
        run_scan(dir.path().to_path_buf(), 3, analysis, cache1, tx2);
        let (projects2, _, cached2) = drain(rx2);
        let loc_after = projects2[0]
            .code_stats
            .as_ref()
            .expect("stats should exist")
            .code_lines;

        assert_eq!(cached2, 0, "edited source must miss the cache");
        assert!(
            loc_after > loc_before,
            "{loc_after} should exceed {loc_before}"
        );
    }
}
