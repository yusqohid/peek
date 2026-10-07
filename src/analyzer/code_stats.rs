use std::path::Path;

use crate::config::AnalysisConfig;
use crate::model::stats::{CodeStats, LanguageStat};

/// Non-programming languages and data formats to exclude from developer statistics.
const NON_PROGRAMMING_LANGUAGES: &[&str] = &[
    "html",
    "css",
    "scss",
    "sass",
    "less",
    "json",
    "json5",
    "jsonc",
    "yaml",
    "yml",
    "toml",
    "markdown",
    "xml",
    "svg",
    "text",
    "plain text",
    "csv",
    "tsv",
    "ini",
    "properties",
    "gitignore",
    "gitattributes",
    "dockerignore",
    "rst",
    "asciidoc",
    "tex",
    "latex",
    "patch",
    "diff",
];

/// Returns true if the language is a markup, style, data, or documentation format.
pub fn is_ignored_language(name: &str) -> bool {
    let lower = name.to_lowercase();
    NON_PROGRAMMING_LANGUAGES
        .iter()
        .any(|&ignored| lower == ignored || lower.starts_with(ignored))
}

/// Run `tokei` against the given project directory and return aggregated code
/// statistics.
pub fn analyze(project_path: &Path, config: &AnalysisConfig) -> CodeStats {
    let excluded: Vec<&str> = config.exclude_dirs.iter().map(String::as_str).collect();

    let tokei_config = tokei::Config {
        hidden: Some(false),
        ..tokei::Config::default()
    };

    let mut languages = tokei::Languages::new();
    languages.get_statistics(&[project_path], &excluded, &tokei_config);

    // Keep only programming languages; every aggregate below — including
    // `total_bytes` — is computed from this same set so the numbers agree.
    let mut lang_stats: Vec<LanguageStat> = Vec::new();
    let mut total_bytes: u64 = 0;
    for (lang_type, lang) in languages.iter() {
        let name = lang_type.to_string();
        if is_ignored_language(&name) || (lang.code == 0 && lang.comments == 0) {
            continue;
        }
        let reports = &lang.children;
        let file_count = lang.reports.len() + reports.values().map(|v| v.len()).sum::<usize>();
        total_bytes += lang
            .reports
            .iter()
            .filter_map(|report| std::fs::metadata(&report.name).ok().map(|m| m.len()))
            .sum::<u64>();

        lang_stats.push(LanguageStat {
            name,
            code_lines: lang.code,
            comment_lines: lang.comments,
            blank_lines: lang.blanks,
            file_count,
        });
    }

    // Sort by code lines descending so the primary language comes first.
    lang_stats.sort_by_key(|a| std::cmp::Reverse(a.code_lines));

    let code_lines: usize = lang_stats.iter().map(|l| l.code_lines).sum();
    let comment_lines: usize = lang_stats.iter().map(|l| l.comment_lines).sum();
    let blank_lines: usize = lang_stats.iter().map(|l| l.blank_lines).sum();
    let file_count: usize = lang_stats.iter().map(|l| l.file_count).sum();

    CodeStats {
        code_lines,
        comment_lines,
        blank_lines,
        file_count,
        languages: lang_stats,
        total_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_analyze_code_stats() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config = AnalysisConfig {
            exclude_dirs: vec!["target".to_string()],
            ignored_projects: vec![],
        };
        let stats = analyze(&manifest_dir, &config);
        assert!(stats.code_lines > 0, "Should have code lines");
        assert!(stats.file_count > 0, "Should have files");
        assert_eq!(stats.primary_language(), "Rust");

        // Verify that markup/data formats are not in the language statistics
        for lang in &stats.languages {
            assert!(
                !is_ignored_language(&lang.name),
                "Language {} should have been filtered out",
                lang.name
            );
        }
    }

    #[test]
    fn total_bytes_only_counts_programming_languages() {
        let dir = tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created");
        let rs_content = "fn main() {\n    println!(\"hi\");\n}\n";
        fs::write(dir.path().join("main.rs"), rs_content).expect("rs should be written");
        let json_content = serde_json::json!({"key": "value", "n": 1}).to_string();
        fs::write(dir.path().join("data.json"), &json_content).expect("json should be written");
        let config = AnalysisConfig {
            exclude_dirs: vec![],
            ignored_projects: vec![],
        };

        let stats = analyze(dir.path(), &config);

        assert!(stats.languages.iter().all(|l| l.name == "Rust"));
        let rs_len = std::fs::metadata(dir.path().join("main.rs"))
            .expect("metadata should exist")
            .len();
        assert_eq!(stats.total_bytes, rs_len);
    }

    #[test]
    fn test_is_ignored_language() {
        assert!(is_ignored_language("HTML"));
        assert!(is_ignored_language("html"));
        assert!(is_ignored_language("CSS"));
        assert!(is_ignored_language("JSON"));
        assert!(is_ignored_language("TOML"));
        assert!(is_ignored_language("Markdown"));
        assert!(is_ignored_language("YAML"));
        assert!(is_ignored_language("SVG"));
        assert!(is_ignored_language("XML"));

        assert!(!is_ignored_language("Rust"));
        assert!(!is_ignored_language("Python"));
        assert!(!is_ignored_language("JavaScript"));
        assert!(!is_ignored_language("TypeScript"));
        assert!(!is_ignored_language("Go"));
        assert!(!is_ignored_language("C"));
        assert!(!is_ignored_language("C++"));
    }
}
