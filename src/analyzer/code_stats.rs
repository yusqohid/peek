use std::path::Path;

use crate::config::AnalysisConfig;
use crate::model::stats::{CodeStats, LanguageStat};

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

    let mut lang_stats: Vec<LanguageStat> = languages
        .iter()
        .filter(|(_, lang)| lang.code > 0 || lang.comments > 0)
        .map(|(lang_type, lang)| {
            let reports = &lang.children;
            let file_count = lang.reports.len() + reports.values().map(|v| v.len()).sum::<usize>();

            LanguageStat {
                name: lang_type.to_string(),
                code_lines: lang.code,
                comment_lines: lang.comments,
                blank_lines: lang.blanks,
                file_count,
            }
        })
        .collect();

    // Sort by code lines descending so the primary language comes first.
    lang_stats.sort_by_key(|a| std::cmp::Reverse(a.code_lines));

    let code_lines: usize = lang_stats.iter().map(|l| l.code_lines).sum();
    let comment_lines: usize = lang_stats.iter().map(|l| l.comment_lines).sum();
    let blank_lines: usize = lang_stats.iter().map(|l| l.blank_lines).sum();
    let file_count: usize = lang_stats.iter().map(|l| l.file_count).sum();

    // Estimate total bytes from the source reports.
    let total_bytes: u64 = languages
        .values()
        .flat_map(|lang| lang.reports.iter())
        .filter_map(|report| std::fs::metadata(&report.name).ok().map(|m| m.len()))
        .sum();

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
    }
}
