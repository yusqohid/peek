use serde::{Deserialize, Serialize};

/// Per-language code breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStat {
    pub name: String,
    pub code_lines: usize,
    pub comment_lines: usize,
    pub blank_lines: usize,
    pub file_count: usize,
}

impl LanguageStat {
    #[allow(dead_code)]
    pub fn total_lines(&self) -> usize {
        self.code_lines + self.comment_lines + self.blank_lines
    }
}

/// Aggregated code statistics for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeStats {
    /// Total lines of actual code (excluding comments and blanks).
    pub code_lines: usize,

    /// Total comment lines.
    pub comment_lines: usize,

    /// Total blank lines.
    pub blank_lines: usize,

    /// Total number of source files analyzed.
    pub file_count: usize,

    /// Breakdown by programming language, sorted by code_lines descending.
    pub languages: Vec<LanguageStat>,

    /// Total project size on disk in bytes (source files only).
    pub total_bytes: u64,
}

impl CodeStats {
    #[allow(dead_code)]
    pub fn total_lines(&self) -> usize {
        self.code_lines + self.comment_lines + self.blank_lines
    }

    /// Ratio of comments to code (0.0 – 1.0+). Returns 0.0 when there is no code.
    #[allow(dead_code)]
    pub fn comment_ratio(&self) -> f64 {
        if self.code_lines == 0 {
            return 0.0;
        }
        self.comment_lines as f64 / self.code_lines as f64
    }

    /// The dominant language by lines of code, or "N/A".
    pub fn primary_language(&self) -> &str {
        self.languages
            .first()
            .map(|l| l.name.as_str())
            .unwrap_or("N/A")
    }
}
