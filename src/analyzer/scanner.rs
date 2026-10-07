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

        if let Some((marker_path, ptype)) = detect_project_type(dir_path) {
            // Avoid registering a sub-project that lives inside an
            // already-detected project (e.g. workspace members).
            let dominated = projects.iter().any(|p| dir_path.starts_with(&p.path));
            if dominated {
                continue;
            }

            let name = dir_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| marker_path.display().to_string());

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
            if let Ok(meta) = std::fs::metadata(&marker_path)
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
/// return the marker path together with the detected project type.
fn detect_project_type(dir: &Path) -> Option<(PathBuf, &'static ProjectType)> {
    for (marker, ptype) in PROJECT_MARKERS {
        if let Some(ext) = marker.strip_prefix('*') {
            // Glob pattern (e.g. *.csproj) — check if any file matches.
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().ends_with(ext) {
                        return Some((e.path(), ptype));
                    }
                }
            }
        } else if dir.join(marker).exists() {
            return Some((dir.join(marker), ptype));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tempdir() -> TempDir {
        tempfile::Builder::new()
            .prefix("peek-test-")
            .tempdir()
            .expect("tempdir should be created")
    }

    fn test_config() -> AnalysisConfig {
        AnalysisConfig {
            exclude_dirs: vec!["target".to_string()],
            ignored_projects: vec![],
        }
    }

    fn create_project(root: &Path, name: &str, marker: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).expect("fixture directory should be created");
        fs::write(dir.join(marker), "marker").expect("marker file should be written");
        dir
    }

    #[test]
    fn detects_all_known_marker_types() {
        let tmp = tempdir();
        let markers = vec![
            ("rust-app", "Cargo.toml", ProjectType::Rust),
            ("node-app", "package.json", ProjectType::NodeJs),
            ("py-app", "pyproject.toml", ProjectType::Python),
            ("py-setup", "setup.py", ProjectType::Python),
            ("go-app", "go.mod", ProjectType::Go),
            ("java-maven", "pom.xml", ProjectType::Java),
            ("java-gradle", "build.gradle", ProjectType::Java),
            ("csharp-app", "example.csproj", ProjectType::CSharp),
            ("ruby-app", "Gemfile", ProjectType::Ruby),
            ("php-app", "composer.json", ProjectType::Php),
        ];
        for (name, marker, _) in &markers {
            create_project(tmp.path(), name, marker);
        }

        let projects = scan_projects(tmp.path(), 2, &test_config());

        assert_eq!(projects.len(), markers.len());
        for (name, _, expected) in &markers {
            let found = projects
                .iter()
                .find(|p| &p.name == name)
                .unwrap_or_else(|| panic!("project {name} should be detected"));
            assert_eq!(&found.project_type, expected);
        }
    }

    #[test]
    fn respects_max_depth() {
        let tmp = tempdir();
        create_project(tmp.path(), "shallow", "Cargo.toml");
        let nested = tmp.path().join("a").join("b");
        fs::create_dir_all(&nested).expect("nested dir should be created");
        fs::write(nested.join("Cargo.toml"), "marker").expect("marker should be written");

        let shallow_scan = scan_projects(tmp.path(), 1, &test_config());
        assert!(shallow_scan.iter().any(|p| p.name == "shallow"));
        assert!(!shallow_scan.iter().any(|p| p.name == "b"));

        let deep_scan = scan_projects(tmp.path(), 3, &test_config());
        assert!(deep_scan.iter().any(|p| p.name == "b"));
    }

    #[test]
    fn respects_exclude_dirs() {
        let tmp = tempdir();
        let inner = tmp.path().join("vendor").join("inner");
        fs::create_dir_all(&inner).expect("inner dir should be created");
        fs::write(inner.join("package.json"), "{}").expect("marker should be written");
        create_project(tmp.path(), "visible", "Cargo.toml");

        let config = AnalysisConfig {
            exclude_dirs: vec!["vendor".to_string()],
            ignored_projects: vec![],
        };
        let projects = scan_projects(tmp.path(), 4, &config);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "visible");
    }

    #[test]
    fn marks_ignored_by_name_and_relative_path() {
        let tmp = tempdir();
        create_project(tmp.path(), "keep", "Cargo.toml");
        create_project(tmp.path(), "skip-by-name", "Cargo.toml");
        let nested_proj = tmp.path().join("group").join("skip-by-path");
        fs::create_dir_all(&nested_proj).expect("nested project should be created");
        fs::write(nested_proj.join("go.mod"), "module x").expect("marker should be written");

        let config = AnalysisConfig {
            exclude_dirs: vec![],
            ignored_projects: vec!["skip-by-name".to_string(), "group/skip-by-path".to_string()],
        };
        let projects = scan_projects(tmp.path(), 4, &config);

        assert_eq!(projects.len(), 3);
        let by_name = projects.iter().find(|p| p.name == "skip-by-name").unwrap();
        assert!(by_name.ignored);
        let by_path = projects.iter().find(|p| p.name == "skip-by-path").unwrap();
        assert!(by_path.ignored);
        let keep = projects.iter().find(|p| p.name == "keep").unwrap();
        assert!(!keep.ignored);
    }

    #[test]
    fn skips_nested_workspace_member() {
        let tmp = tempdir();
        let outer = create_project(tmp.path(), "workspace", "Cargo.toml");
        let member = outer.join("member");
        fs::create_dir_all(&member).expect("member dir should be created");
        fs::write(member.join("Cargo.toml"), "marker").expect("marker should be written");

        let projects = scan_projects(tmp.path(), 4, &test_config());

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "workspace");
    }

    #[test]
    fn sorts_projects_case_insensitively() {
        let tmp = tempdir();
        for name in ["zeta", "Alpha", "mike"] {
            create_project(tmp.path(), name, "Cargo.toml");
        }

        let projects = scan_projects(tmp.path(), 2, &test_config());
        let names: Vec<_> = projects.iter().map(|p| p.name.as_str()).collect();

        assert_eq!(names, vec!["Alpha", "mike", "zeta"]);
    }

    #[test]
    fn detects_csharp_project_and_uses_the_matched_marker_timestamp() {
        let tmp = tempdir();
        let project_dir = tmp.path().join("example");
        fs::create_dir_all(&project_dir).expect("fixture directory should be created");
        fs::write(project_dir.join("example.csproj"), "<Project />")
            .expect("C# marker should be written");

        let projects = scan_projects(tmp.path(), 2, &AnalysisConfig::default());

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_type, ProjectType::CSharp);
        assert!(projects[0].last_modified.is_some());
    }
}
