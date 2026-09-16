use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::AnalysisConfig;
use crate::model::project::{ProjectInfo, ProjectType};

/// Marker files that identify a project root and its type.
const PROJECT_MARKERS: &[(&str, ProjectType)] = &[
    ("Cargo.toml", ProjectType::Rust),
    ("package.json", ProjectType::NodeJs),
    ("pyproject.toml", ProjectType::Python),
    ("setup.py", ProjectType::Python),
    ("go.mod", ProjectType::Go),
    ("pom.xml", ProjectType::Java),
    ("build.gradle", ProjectType::Java),
    ("build.gradle.kts", ProjectType::Java),
    ("*.csproj", ProjectType::CSharp),
    ("Gemfile", ProjectType::Ruby),
    ("composer.json", ProjectType::Php),
];

/// Scan `root` for project directories up to `max_depth`.
///
/// A directory is considered a project if it contains one of the known marker
/// files.  Projects whose name (or relative path) appears in
/// `config.ignored_projects` are marked as `ignored = true` but still included
/// in the returned list so the UI can display them.
pub fn scan_projects(root: &Path, max_depth: usize, config: &AnalysisConfig) -> Vec<ProjectInfo> {
    let mut projects: Vec<ProjectInfo> = Vec::new();

    for entry in WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden directories and common noise directories.
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && !config.exclude_dirs.contains(&name.to_string())
        })
        .flatten()
    {
        if !entry.file_type().is_dir() {
            continue;
        }

        let dir_path = entry.path();

        if let Some((marker, ptype)) = detect_project_type(dir_path) {
            // Avoid registering a sub-project that lives inside an
            // already-detected project (e.g. workspace members).
            let dominated = projects.iter().any(|p| dir_path.starts_with(&p.path));
            if dominated {
                continue;
            }

            let name = dir_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| marker.to_string());

            let rel_path = dir_path
                .strip_prefix(root)
                .unwrap_or(dir_path)
                .to_string_lossy()
                .to_string();

            let ignored = config.ignored_projects.contains(&name)
                || config.ignored_projects.contains(&rel_path);

            let mut info = ProjectInfo::new(name, dir_path.to_path_buf(), ptype.clone());
            info.ignored = ignored;

            // Try to get a last-modified timestamp from the marker file.
            if let Ok(meta) = std::fs::metadata(dir_path.join(marker))
                && let Ok(modified) = meta.modified()
            {
                info.last_modified = Some(modified.into());
            }

            projects.push(info);
        }
    }

    // Sort by name alphabetically by default.
    projects.sort_by_key(|a| a.name.to_lowercase());
    projects
}

/// Check whether `dir` contains one of the known project marker files and
/// return the marker filename together with the detected project type.
fn detect_project_type(dir: &Path) -> Option<(&'static str, &'static ProjectType)> {
    for (marker, ptype) in PROJECT_MARKERS {
        if let Some(ext) = marker.strip_prefix('*') {
            // Glob pattern (e.g. *.csproj) — check if any file matches.
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().ends_with(ext) {
                        return Some((marker, ptype));
                    }
                }
            }
        } else if dir.join(marker).exists() {
            return Some((marker, ptype));
        }
    }
    None
}

/// Convenience: resolve a path that may start with `~`.
#[allow(dead_code)]
pub fn resolve_path(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(stripped);
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_current_directory() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config = AnalysisConfig {
            exclude_dirs: vec!["target".to_string()],
            ignored_projects: vec![],
        };
        let projects = scan_projects(&manifest_dir, 2, &config);
        assert!(!projects.is_empty(), "Should detect current project");
        assert_eq!(projects[0].project_type, ProjectType::Rust);
        assert!(!projects[0].ignored);
    }

    #[test]
    fn test_ignored_project() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let name = manifest_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let config = AnalysisConfig {
            exclude_dirs: vec!["target".to_string()],
            ignored_projects: vec![name],
        };
        let projects = scan_projects(&manifest_dir, 2, &config);
        assert!(!projects.is_empty());
        assert!(projects[0].ignored, "Project should be marked as ignored");
    }
}
