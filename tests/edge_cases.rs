mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn no_targets_exits_cleanly() {
    let dir = TempDir::new().unwrap();
    common::make_file(&dir.path().join("hello.txt"), b"hello");

    let status = Command::new(common::vacuum_bin())
        .arg(dir.path())
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
}

#[test]
fn all_adapters_disabled_exits_cleanly() {
    let dir = TempDir::new().unwrap();
    common::make_file(&dir.path().join("package.json"), b"{}");
    common::make_dir(&dir.path().join("node_modules"));

    let output = Command::new(common::vacuum_bin())
        .arg(dir.path())
        .args([
            "--mode",
            "auto",
            "--node=false",
            "--cargo=false",
            "--python=false",
            "--go=false",
            "--gradle=false",
            "--maven=false",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("All adapters are disabled"),
        "Expected message about disabled adapters, got: {stdout}"
    );
    assert!(
        dir.path().join("node_modules").exists(),
        "node_modules should be untouched"
    );
}

#[test]
fn invalid_path_exits_with_error() {
    let status = Command::new(common::vacuum_bin())
        .arg("/nonexistent/path/that/does/not/exist/vacuum_test")
        .status()
        .unwrap();

    assert!(!status.success(), "Should fail for a non-existent path");
}

#[test]
fn file_path_instead_of_dir_exits_with_error() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("file.txt");
    common::make_file(&file, b"content");

    let status = Command::new(common::vacuum_bin())
        .arg(&file)
        .status()
        .unwrap();

    assert!(
        !status.success(),
        "Should fail when path is a file, not a directory"
    );
}
