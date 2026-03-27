mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_maven_target() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("pom.xml"), b"<project/>");
    common::make_dir(&root.join("target").join("classes"));
    common::make_file(
        &root.join("target").join("classes").join("App.class"),
        b"cafebabe",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("target").exists(), "target/ should be deleted");
    assert!(root.join("pom.xml").exists(), "pom.xml should be preserved");
}
