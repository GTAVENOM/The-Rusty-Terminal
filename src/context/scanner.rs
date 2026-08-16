//! Static project-context scanner.
//!
//! On demand (Ctrl+K, tab open), checks the working directory and up to 2
//! parent directories for well-known project markers. File-presence
//! detection ONLY: no live process scanning, no port scanning, no polling.

use std::path::Path;

/// How many parent directories to check above the cwd.
const PARENT_LEVELS: usize = 2;

/// Markers we look for, in the order they are reported.
const MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "requirements.txt",
    "Cargo.toml",
    "docker-compose.yml",
    "docker-compose.yaml",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectContext {
    /// (marker name, relative location: 0 = cwd, 1 = parent, 2 = grandparent)
    pub markers: Vec<(String, usize)>,
}

impl ProjectContext {
    /// Scan `cwd` and up to 2 parents for project markers.
    pub fn scan(cwd: &Path) -> Self {
        let mut markers = Vec::new();
        let mut dir = Some(cwd.to_path_buf());
        for level in 0..=PARENT_LEVELS {
            let Some(d) = &dir else { break };
            for marker in MARKERS {
                if d.join(marker).exists() {
                    // Report each marker once, from the nearest level.
                    if !markers.iter().any(|(m, _)| m == marker) {
                        markers.push((marker.to_string(), level));
                    }
                }
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
        Self { markers }
    }

    /// Marker names with location hints, for the LLM context block:
    /// `[".git (cwd)", "Cargo.toml (parent)"]`.
    pub fn marker_names(&self) -> Vec<String> {
        self.markers
            .iter()
            .map(|(name, level)| {
                let loc = match level {
                    0 => "cwd",
                    1 => "parent",
                    _ => "grandparent",
                };
                format!("{name} ({loc})")
            })
            .collect()
    }

    pub fn has_git(&self) -> bool {
        self.markers.iter().any(|(m, _)| m == ".git")
    }

    pub fn has_docker_compose(&self) -> bool {
        self.markers
            .iter()
            .any(|(m, _)| m.starts_with("docker-compose"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_markers_in_cwd_and_parents() {
        let root = std::env::temp_dir().join(format!(
            "rusty_scanner_test_{}",
            std::process::id()
        ));
        let child = root.join("a").join("b");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(child.join("docker-compose.yml"), "services:")
            .unwrap();

        let ctx = ProjectContext::scan(&child);
        assert!(ctx.has_docker_compose());
        // Cargo.toml is 2 levels up: found at grandparent level.
        assert!(ctx
            .markers
            .iter()
            .any(|(m, level)| m == "Cargo.toml" && *level == 2));
        assert!(!ctx.has_git());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn empty_dir_has_no_markers() {
        let dir = std::env::temp_dir().join(format!(
            "rusty_scanner_empty_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // Note: temp dir parents could theoretically contain markers;
        // scan a deep nested path to isolate.
        let deep = dir.join("x").join("y");
        std::fs::create_dir_all(&deep).unwrap();
        let ctx = ProjectContext::scan(&deep);
        // Only assert on markers *at* cwd level (parents are system dirs).
        assert!(!ctx.markers.iter().any(|(_, level)| *level == 0));
        std::fs::remove_dir_all(&dir).ok();
    }
}
