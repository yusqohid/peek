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
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn counts_markers_in_source_files_and_skips_excluded_directories() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("peek-todo-scanner-{}-{unique}", std::process::id()));
        fs::create_dir_all(root.join("excluded")).expect("fixture directory should be created");
        fs::write(
            root.join("main.rs"),
            "// TODO: implement\n// FIXME: repair\n// HACK: temporary\n// BUG: tracked\n",
        )
        .expect("fixture source should be written");
        fs::write(root.join("notes.txt"), "TODO: non-source file")
            .expect("fixture note should be written");
        fs::write(root.join("excluded/ignored.rs"), "// TODO: excluded")
            .expect("excluded source should be written");

        let stats = scan_todos(&root, &["excluded".to_string()]);

        assert_eq!(stats.todo_count, 1);
        assert_eq!(stats.fixme_count, 1);
        assert_eq!(stats.hack_count, 1);
        assert_eq!(stats.bug_count, 1);
        assert_eq!(stats.total(), 4);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }
}
