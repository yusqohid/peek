use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Local, TimeZone};
use git2::{Repository, Sort};

use crate::model::git::{CommitInfo, Contributor, GitStats};

/// Analyze the git repository at `path`. Returns `None` if the directory is not
/// a git repository or if git operations fail.
pub fn analyze_git(path: &Path) -> Option<GitStats> {
    let repo = Repository::open(path).ok()?;
    let mut revwalk = repo.revwalk().ok()?;
    revwalk.push_head().ok()?;
    revwalk.set_sorting(Sort::TIME).ok()?;

    let current_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    let now = Local::now();
    let days_tracked = 364; // 52 weeks * 7 days
    let mut daily_activity = vec![0usize; days_tracked];

    let mut total_commits = 0usize;
    let mut commits_last_30_days = 0usize;
    let mut recent_commits = Vec::new();
    let mut author_counts: HashMap<String, usize> = HashMap::new();
    let mut first_commit_info: Option<CommitInfo> = None;

    for oid_result in revwalk {
        let oid = match oid_result {
            Ok(oid) => oid,
            Err(_) => continue,
        };

        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        total_commits += 1;

        let commit_seconds = commit.time().seconds();
        let commit_time: DateTime<Local> = match Local.timestamp_opt(commit_seconds, 0) {
            chrono::LocalResult::Single(dt) => dt,
            _ => Local::now(),
        };

        let author_name = commit.author().name().unwrap_or("Unknown").to_string();
        *author_counts.entry(author_name.clone()).or_default() += 1;

        let short_hash = oid.to_string();
        let short_hash = if short_hash.len() >= 7 {
            short_hash[..7].to_string()
        } else {
            short_hash
        };

        let message = commit.summary().unwrap_or("").to_string();

        let info = CommitInfo {
            hash: short_hash,
            author: author_name,
            message,
            timestamp: commit_time,
        };

        if first_commit_info.is_none() {
            first_commit_info = Some(info.clone());
        }

        if recent_commits.len() < 15 {
            recent_commits.push(info);
        }

        // Daily activity for the last 52 weeks (364 days).
        let days_ago = (now.date_naive() - commit_time.date_naive()).num_days();
        if days_ago >= 0 && (days_ago as usize) < days_tracked {
            let idx = (days_tracked - 1) - (days_ago as usize);
            daily_activity[idx] += 1;
        }

        if (0..=30).contains(&days_ago) {
            commits_last_30_days += 1;
        }
    }

    if total_commits == 0 {
        return None;
    }

    // Sort contributors by commit count descending.
    let mut top_contributors: Vec<Contributor> = author_counts
        .into_iter()
        .map(|(name, commit_count)| Contributor { name, commit_count })
        .collect();
    top_contributors.sort_by_key(|c| std::cmp::Reverse(c.commit_count));
    top_contributors.truncate(5);

    Some(GitStats {
        total_commits,
        commits_last_30_days,
        last_commit: first_commit_info,
        recent_commits,
        top_contributors,
        daily_activity,
        current_branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_analyze_git_current_repo() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let stats = analyze_git(&manifest_dir);
        assert!(stats.is_some(), "Current repository should have git stats");
        let stats = stats.unwrap();
        assert!(stats.total_commits >= 1, "Should have at least 1 commit");
        assert!(stats.last_commit.is_some(), "Should have a last commit");
        assert!(
            !stats.top_contributors.is_empty(),
            "Should have top contributors"
        );
    }
}
