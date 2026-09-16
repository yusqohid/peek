use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Top-level configuration loaded from `config.toml`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub display: DisplayConfig,

    #[serde(default)]
    pub analysis: AnalysisConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralConfig {
    /// Root directory to scan for projects.
    #[serde(default = "default_scan_dir")]
    pub scan_directory: String,

    /// Maximum depth when walking directories looking for project markers.
    #[serde(default = "default_scan_depth")]
    pub scan_depth: usize,

    /// Auto-refresh interval in seconds.
    #[serde(default = "default_refresh")]
    #[allow(dead_code)]
    pub refresh_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    /// Which tab to show on startup.
    #[serde(default = "default_tab")]
    pub default_tab: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisConfig {
    /// Directory names excluded from code analysis (e.g. node_modules, target).
    #[serde(default = "default_exclude_dirs")]
    pub exclude_dirs: Vec<String>,

    /// Project directory names (or relative paths) to ignore completely.
    #[serde(default)]
    pub ignored_projects: Vec<String>,
}

// ── defaults ──

fn default_scan_dir() -> String {
    "~/Dev".to_string()
}
fn default_scan_depth() -> usize {
    3
}
fn default_refresh() -> u64 {
    300
}
fn default_tab() -> String {
    "dashboard".to_string()
}
fn default_exclude_dirs() -> Vec<String> {
    [
        "node_modules",
        "target",
        "dist",
        "build",
        ".git",
        "vendor",
        "__pycache__",
        ".venv",
        "venv",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

// ── impl ──

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            scan_directory: default_scan_dir(),
            scan_depth: default_scan_depth(),
            refresh_interval_secs: default_refresh(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            default_tab: default_tab(),
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            exclude_dirs: default_exclude_dirs(),
            ignored_projects: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Load config from the given path, falling back to defaults for missing
    /// fields.  If the file does not exist a fully-default config is returned.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path).context("Failed to read config file")?;
        let config: Self = toml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    /// Resolve `~` to the actual home directory.
    pub fn resolved_scan_directory(&self) -> PathBuf {
        let raw = &self.general.scan_directory;
        if let Some(stripped) = raw.strip_prefix("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(stripped);
        }
        PathBuf::from(raw)
    }
}
