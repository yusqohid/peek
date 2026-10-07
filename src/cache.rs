use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use walkdir::WalkDir;

use crate::analyzer::todo_scanner::TodoStats;
use crate::model::git::GitStats;
use crate::model::stats::CodeStats;

/// Fingerprint used to decide whether cached analysis is still valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub marker_modified: Option<DateTime<Local>>,
    pub git_head: Option<String>,
    pub source_modified: Option<SystemTime>,
}

/// Cached per-project analysis results.
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub key: CacheKey,
    pub code_stats: Option<CodeStats>,
    pub git_stats: Option<GitStats>,
    pub todo_stats: Option<TodoStats>,
    pub last_modified: Option<DateTime<Local>>,
}

/// In-memory cache of per-project analysis, keyed by project path.
#[derive(Debug, Clone, Default)]
pub struct AnalysisCache {
    entries: HashMap<PathBuf, CachedEntry>,
}

impl AnalysisCache {
    pub fn get(&self, path: &Path, key: &CacheKey) -> Option<&CachedEntry> {
        self.entries.get(path).filter(|e| &e.key == key)
    }

    pub fn insert(
        &mut self,
        path: PathBuf,
        key: CacheKey,
        code_stats: Option<CodeStats>,
        git_stats: Option<GitStats>,
        todo_stats: Option<TodoStats>,
        last_modified: Option<DateTime<Local>>,
    ) {
        self.entries.insert(
            path,
            CachedEntry {
                key,
                code_stats,
                git_stats,
                todo_stats,
                last_modified,
            },
        );
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Cheap HEAD lookup without walking history. Returns `None` for non-repos.
pub fn git_head_oid(path: &Path) -> Option<String> {
    let repo = git2::Repository::open(path).ok()?;
    let head = repo.head().ok()?;
    let oid = head.target()?;
    Some(oid.to_string())
}

pub fn cache_key_for(
    marker_modified: Option<DateTime<Local>>,
    git_head: Option<String>,
    source_modified: Option<SystemTime>,
) -> CacheKey {
    CacheKey {
        marker_modified,
        git_head,
        source_modified,
    }
}

/// Newest file modification time under `path`, skipping hidden and excluded
/// directories.
///
/// Metadata-only walk: far cheaper than re-running tokei/git analysis, so the
/// cache still pays off on large workspaces when nothing changed.
pub fn newest_source_mtime(path: &Path, exclude_dirs: &[String]) -> Option<SystemTime> {
    WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && !exclude_dirs.iter().any(|ex| ex == &name)
        })
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_requires_matching_key() {
        let mut cache = AnalysisCache::default();
        let path = PathBuf::from("/proj");
        let key = cache_key_for(None, Some("abc".to_string()), None);
        cache.insert(path.clone(), key.clone(), None, None, None, None);

        assert!(cache.get(&path, &key).is_some());
        let other = cache_key_for(None, Some("def".to_string()), None);
        assert!(cache.get(&path, &other).is_none());
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
    }

    #[test]
    fn source_mtime_change_invalidates_key() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n")
            .expect("fixture should be written");

        let before = newest_source_mtime(dir.path(), &[]).expect("mtime should exist");
        // A new file is guaranteed a newer (or equal) mtime; the key must
        // differ once content is added.
        std::fs::write(dir.path().join("lib.rs"), "pub fn f() {}\n")
            .expect("fixture should be written");
        let after = newest_source_mtime(dir.path(), &[]).expect("mtime should exist");

        assert!(after >= before);
        let a = cache_key_for(None, None, Some(before));
        let b = cache_key_for(None, None, Some(after));
        // Keys differ unless the filesystem timestamp granularity collided;
        // at minimum the lookup helper must observe the new file.
        if before != after {
            assert_ne!(a, b);
        }

        // Excluded directories are ignored.
        std::fs::create_dir_all(dir.path().join("target")).expect("dir should be created");
        std::fs::write(dir.path().join("target").join("x"), "newer")
            .expect("fixture should be written");
        let excluded = newest_source_mtime(dir.path(), &["target".to_string()]);
        assert_eq!(excluded, Some(after));
    }

    #[test]
    fn git_head_returns_none_for_non_repo() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        assert!(git_head_oid(dir.path()).is_none());
    }
}
