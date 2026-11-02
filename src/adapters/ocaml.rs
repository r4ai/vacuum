use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct OcamlAdapter;

const OCAML_TARGETS: &[&str] = &["_build", "_opam"];
const OCAML_CONTEXT_FILES: &[&str] = &["dune-project", "dune-workspace"];

/// Whether `dir` is the root of a dune project or holds an `.opam` file.
fn has_ocaml_context(ctx: &mut Context, dir: &Path) -> bool {
    OCAML_CONTEXT_FILES.iter().any(|f| ctx.has_file(dir, f)) || ctx.has_extension(dir, "opam")
}

impl Adapter for OcamlAdapter {
    fn name(&self) -> &'static str {
        "ocaml"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (OCAML_TARGETS.contains(&name) && has_ocaml_context(ctx, parent))
            .then(|| format!("OCaml build artifact ({name}/)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn build_dir_with_dune_project_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("dune-project"), "(lang dune 3.0)").unwrap();
        fs::create_dir(dir.path().join("_build")).unwrap();

        let targets = scan_with(OcamlAdapter, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("_build"));
        assert_eq!(targets[0].adapter, "ocaml");
    }

    #[test]
    fn build_dir_without_dune_project_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("_build")).unwrap();

        let targets = scan_with(OcamlAdapter, dir.path());
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

        let targets = scan_with(OcamlAdapter, dir.path());
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn nested_build_dir_not_rescanned() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("dune-project"), "(lang dune 3.0)").unwrap();
        fs::create_dir_all(dir.path().join("_build").join("_build")).unwrap();

        let targets = scan_with(OcamlAdapter, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("_build"));
    }

    #[test]
    fn opam_dir_with_dune_workspace_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("dune-workspace"), "(lang dune 3.0)").unwrap();
        fs::create_dir(dir.path().join("_opam")).unwrap();

        let targets = scan_with(OcamlAdapter, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("_opam"));
    }

    #[test]
    fn opam_dir_with_opam_file_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("sample.opam"), "opam-version: \"2.0\"").unwrap();
        fs::create_dir(dir.path().join("_opam")).unwrap();

        let targets = scan_with(OcamlAdapter, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("_opam"));
    }

    #[test]
    fn opam_dir_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("_opam")).unwrap();

        let targets = scan_with(OcamlAdapter, dir.path());
        assert!(targets.is_empty());
    }
}
