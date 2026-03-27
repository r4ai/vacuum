mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_go_vendor() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("go.mod"), b"module example.com/app\n\ngo 1.21");
    common::make_dir(&root.join("vendor").join("github.com").join("pkg"));
    common::make_file(
        &root
            .join("vendor")
            .join("github.com")
            .join("pkg")
            .join("lib.go"),
        b"package pkg",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("vendor").exists(), "vendor/ should be deleted");
    assert!(root.join("go.mod").exists(), "go.mod should be preserved");
}
