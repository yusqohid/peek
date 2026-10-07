use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use crate::analyzer::todo_scanner::TodoStats;
use crate::model::git::GitStats;
use crate::model::stats::CodeStats;

/// Fingerprint used to decide whether cached analysis is still valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub marker_modified: Option<DateTime<Local>>,
    pub git_head: Option<String>,
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
) -> CacheKey {
    CacheKey {
        marker_modified,
        git_head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_requires_matching_key() {
        let mut cache = AnalysisCache::default();
        let path = PathBuf::from("/proj");
        let key = cache_key_for(None, Some("abc".to_string()));
        cache.insert(path.clone(), key.clone(), None, None, None, None);

        assert!(cache.get(&path, &key).is_some());
        let other = cache_key_for(None, Some("def".to_string()));
        assert!(cache.get(&path, &other).is_none());
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
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
