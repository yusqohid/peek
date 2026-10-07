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

/// Scan source files in `root` for TODO, FIXME, HACK, and BUG markers.
///
/// Contract: plain-text matching — markers count anywhere in a scannable
/// source file, not only inside comments. Matching is on whole words, so
/// `debug:` does not count as BUG.
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
                let Ok(line) = line_res else { continue };
                match classify_line(&line) {
                    Some(Marker::Todo) => stats.todo_count += 1,
                    Some(Marker::Fixme) => stats.fixme_count += 1,
                    Some(Marker::Hack) => stats.hack_count += 1,
                    Some(Marker::Bug) => stats.bug_count += 1,
                    None => {}
                }
            }
        }
    }

    stats
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Todo,
    Fixme,
    Hack,
    Bug,
}

/// First marker whose whole word appears in `line` (case-insensitive).
///
/// Splitting on non-alphanumeric characters gives word boundaries, so
/// `debug:` yields the token `DEBUG` — not `BUG`.
fn classify_line(line: &str) -> Option<Marker> {
    let upper = line.to_uppercase();
    let mut found = None;
    for token in upper.split(|c: char| !c.is_alphanumeric()) {
        let marker = match token {
            "TODO" => Marker::Todo,
            "FIXME" => Marker::Fixme,
            "HACK" => Marker::Hack,
            "BUG" => Marker::Bug,
            _ => continue,
        };
        // Keep the same priority as before: TODO > FIXME > HACK > BUG.
        let rank = marker as u8;
        if found.is_none_or(|m: Marker| rank < m as u8) {
            found = Some(marker);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created")
    }

    #[test]
    fn counts_markers_in_source_files_and_skips_excluded_directories() {
        let dir = tempdir();
        let root = dir.path();
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

        let stats = scan_todos(root, &["excluded".to_string()]);

        assert_eq!(stats.todo_count, 1);
        assert_eq!(stats.fixme_count, 1);
        assert_eq!(stats.hack_count, 1);
        assert_eq!(stats.bug_count, 1);
        assert_eq!(stats.total(), 4);
    }

    #[test]
    fn debug_line_is_not_counted_as_bug() {
        assert_eq!(classify_line("// debug: verbose logging"), None);
        assert_eq!(classify_line("log::debug!(\"hi\")"), None);
        assert_eq!(classify_line("// BUG: tracked"), Some(Marker::Bug));
        assert_eq!(classify_line("// TODO implement"), Some(Marker::Todo));
        assert_eq!(classify_line("// FIXME"), Some(Marker::Fixme));
    }

    #[test]
    fn invalid_utf8_lines_do_not_stop_the_scan() {
        use std::io::Write;
        let dir = tempdir();
        let root = dir.path();
        // Line 1: invalid UTF-8, line 2: real marker, line 3: debug: (no match).
        let mut bytes = vec![0xff, 0xfe, b'\n'];
        bytes.extend_from_slice(b"// TODO: after bad line\n// debug: noise\n");
        let mut file = fs::File::create(root.join("bad.rs")).expect("fixture should be created");
        file.write_all(&bytes).expect("fixture should be written");

        let stats = scan_todos(root, &[]);

        assert_eq!(stats.todo_count, 1);
        assert_eq!(stats.bug_count, 0);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn skips_files_larger_than_one_megabyte() {
        let dir = tempdir();
        let root = dir.path();
        let mut big = String::from("// TODO: too big\n");
        big.push_str(&"x".repeat(1_000_001));
        fs::write(root.join("big.rs"), big).expect("large fixture should be written");
        fs::write(root.join("small.rs"), "// TODO: small\n")
            .expect("small fixture should be written");

        let stats = scan_todos(root, &[]);

        assert_eq!(stats.todo_count, 1);
        assert_eq!(stats.total(), 1);
    }
}
