use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct OcamlAdapter;

fn has_ocaml_context(dir: &Path) -> bool {
    dir.join("dune-project").exists()
}

impl Adapter for OcamlAdapter {
    fn name(&self) -> &'static str {
        "ocaml"
    }

    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        let mut targets = Vec::new();
        let mut iter = WalkDir::new(root).follow_links(false).into_iter();

        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();

            if !entry.file_type().is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy();
            if name != "_build" {
                continue;
            }

            let parent = match path.parent() {
                Some(p) => p,
                None => continue,
            };
            if !has_ocaml_context(parent) {
                continue;
            }

            let size = compute_dir_size(path);
            targets.push(CleanTarget {
                path: path.to_path_buf(),
                adapter: self.name(),
                description: "OCaml/dune build artifacts (_build/)".into(),
                size,
            });
            iter.skip_current_dir();
        }

        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn build_dir_with_dune_project_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("dune-project"), "(lang dune 3.0)").unwrap();
        fs::create_dir(dir.path().join("_build")).unwrap();

        let targets = OcamlAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("_build"));
        assert_eq!(targets[0].adapter, "ocaml");
    }

    #[test]
    fn build_dir_without_dune_project_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("_build")).unwrap();

        let targets = OcamlAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn multiple_projects_all_detected() {
        let dir = TempDir::new().unwrap();
        let project_a = dir.path().join("project_a");
        let project_b = dir.path().join("project_b");
        fs::create_dir_all(&project_a).unwrap();
        fs::create_dir_all(&project_b).unwrap();
        fs::write(project_a.join("dune-project"), "(lang dune 3.0)").unwrap();
        fs::create_dir(project_a.join("_build")).unwrap();
        fs::write(project_b.join("dune-project"), "(lang dune 3.0)").unwrap();
        fs::create_dir(project_b.join("_build")).unwrap();

        let targets = OcamlAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn nested_build_dir_not_rescanned() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("dune-project"), "(lang dune 3.0)").unwrap();
        fs::create_dir_all(dir.path().join("_build").join("_build")).unwrap();

        let targets = OcamlAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("_build"));
    }
}
