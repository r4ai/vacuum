mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_handles_mixed_project() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Node sub-project
    common::make_file(&root.join("frontend").join("package.json"), b"{}");
    common::make_dir(&root.join("frontend").join("node_modules").join("dep"));
    common::make_file(
        &root
            .join("frontend")
            .join("node_modules")
            .join("dep")
            .join("index.js"),
        b"",
    );

    // Rust sub-project
    common::make_file(
        &root.join("backend").join("Cargo.toml"),
        b"[package]\nname=\"backend\"\nversion=\"0.1.0\"\nedition=\"2021\"",
    );
    common::make_dir(&root.join("backend").join("target").join("debug"));
    common::make_file(
        &root
            .join("backend")
            .join("target")
            .join("debug")
            .join("app"),
        b"binary",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !root.join("frontend").join("node_modules").exists(),
        "node_modules should be deleted"
    );
    assert!(
        !root.join("backend").join("target").exists(),
        "target/ should be deleted"
    );
    assert!(
        root.join("frontend").join("package.json").exists(),
        "package.json should be preserved"
    );
    assert!(
        root.join("backend").join("Cargo.toml").exists(),
        "Cargo.toml should be preserved"
    );
}
