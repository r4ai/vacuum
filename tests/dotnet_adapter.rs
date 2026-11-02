mod common;

use std::process::Command;
use tempfile::TempDir;

fn vacuum(root: &std::path::Path) {
    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn bin_and_obj_next_to_a_project_file_are_deleted() {
    for project in ["App.csproj", "App.fsproj", "App.vbproj"] {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        common::make_file(&root.join(project), b"<Project/>");
        common::make_file(&root.join("bin").join("Debug").join("App.dll"), b"");
        common::make_file(&root.join("obj").join("project.assets.json"), b"");

        vacuum(root);

        assert!(!root.join("bin").exists(), "bin/ next to {project}");
        assert!(!root.join("obj").exists(), "obj/ next to {project}");
        assert!(root.join(project).exists());
    }
}

/// `bin/` is a common name; without a project file it must be kept.
#[test]
fn bin_without_a_project_file_is_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("bin").join("deploy.sh"), b"#!/bin/sh");
    common::make_file(&root.join("sub").join("App.csproj"), b"<Project/>");

    vacuum(root);

    assert!(root.join("bin").join("deploy.sh").exists());
}
