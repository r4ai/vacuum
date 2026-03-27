mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_build_dir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("dune-project"), b"(lang dune 3.0)");
    common::make_dir(&root.join("_build").join("default"));
    common::make_file(
        &root.join("_build").join("default").join("main.cmo"),
        b"\x00",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("_build").exists(), "_build/ should be deleted");
    assert!(
        root.join("dune-project").exists(),
        "dune-project should be preserved"
    );
}

#[test]
fn build_dir_without_dune_project_not_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_dir(&root.join("_build"));

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join("_build").exists(), "_build/ should NOT be deleted");
}

#[test]
fn ocaml_flag_false_skips_build_dir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("dune-project"), b"(lang dune 3.0)");
    common::make_dir(&root.join("_build"));

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--ocaml=false"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        root.join("_build").exists(),
        "_build/ should NOT be deleted when --ocaml=false"
    );
}
