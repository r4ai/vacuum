mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_cargo_target() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(
        &root.join("Cargo.toml"),
        b"[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"",
    );
    common::make_dir(&root.join("target").join("debug"));
    common::make_file(&root.join("target").join("debug").join("app"), b"binary");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("target").exists(), "target/ should be deleted");
    assert!(
        root.join("Cargo.toml").exists(),
        "Cargo.toml should be preserved"
    );
}
