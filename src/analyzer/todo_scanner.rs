use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Statistics on technical debt markers (TODO, FIXME, etc.) in a codebase.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoStats {
    pub todo_count: usize,
    pub fixme_count: usize,
    pub hack_count: usize,
    pub bug_count: usize,
}

impl TodoStats {
    #[allow(dead_code)]
    pub fn total(&self) -> usize {
        self.todo_count + self.fixme_count + self.hack_count + self.bug_count
    }
}

/// Known programming file extensions to scan for TODOs/FIXMEs.
const SCANNABLE_EXTENSIONS: &[&str] = &[
    "rs", "js", "ts", "jsx", "tsx", "py", "go", "java", "c", "cpp", "h", "hpp", "cs", "rb", "php",
    "swift", "kt", "scala", "dart", "sh", "bash", "zsh", "lua", "zig",
];

/// Scan source files in `root` for TODO, FIXME, HACK, and BUG comments.
pub fn scan_todos(root: &Path, exclude_dirs: &[String]) -> TodoStats {
    let mut stats = TodoStats::default();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && !exclude_dirs.iter().any(|ex| ex == &name)
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !SCANNABLE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        // Limit file read size (skip generated or huge files > 1MB)
        if let Ok(metadata) = entry.metadata()
            && metadata.len() > 1_000_000
        {
            continue;
        }

        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for line_res in reader.lines() {
                let Ok(line) = line_res else { break };
                let upper = line.to_uppercase();

                if upper.contains("TODO:") || upper.contains("TODO ") {
                    stats.todo_count += 1;
                } else if upper.contains("FIXME:") || upper.contains("FIXME ") {
                    stats.fixme_count += 1;
                } else if upper.contains("HACK:") || upper.contains("HACK ") {
                    stats.hack_count += 1;
                } else if upper.contains("BUG:") || upper.contains("BUG ") {
                    stats.bug_count += 1;
                }
            }
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_scan_todos_current_repo() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let stats = scan_todos(&manifest_dir, &["target".to_string()]);
        // Validates that the scanner runs without crashing
        assert!(stats.total() >= 0);
    }
}
