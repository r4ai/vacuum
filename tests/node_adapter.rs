mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_dir(&root.join("node_modules").join("dep"));
    common::make_file(
        &root.join("node_modules").join("dep").join("index.js"),
        b"// code",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !root.join("node_modules").exists(),
        "node_modules should be deleted"
    );
    assert!(
        root.join("package.json").exists(),
        "package.json should be preserved"
    );
}

#[test]
fn dry_run_does_not_delete_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_dir(&root.join("node_modules").join("dep"));
    common::make_file(
        &root.join("node_modules").join("dep").join("index.js"),
        b"// code",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--dry-run"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        root.join("node_modules").exists(),
        "node_modules should survive dry-run"
    );
}

#[test]
fn node_adapter_disabled_skips_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_dir(&root.join("node_modules").join("dep"));

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--node=false"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        root.join("node_modules").exists(),
        "node_modules should be preserved when adapter is disabled"
    );
}

#[test]
fn auto_mode_with_no_size_deletes_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_dir(&root.join("node_modules").join("dep"));
    common::make_file(
        &root.join("node_modules").join("dep").join("index.js"),
        b"// code",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--no-size"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !root.join("node_modules").exists(),
        "node_modules should be deleted"
    );
    assert!(
        root.join("package.json").exists(),
        "package.json should be preserved"
    );
}
