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

/// Run vacuum in auto + dry-run mode with gitignore enabled; return stdout.
fn dry_run_with_gitignore(root: &std::path::Path) -> String {
    let output = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--gitignore", "--dry-run", "--no-size"])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn descendant_of_gitignore_target_is_not_listed() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".gitignore"), b"pkgs/\n");
    common::make_file(&root.join("pkgs").join("app").join("package.json"), b"{}");
    common::make_dir(&root.join("pkgs").join("app").join("node_modules"));

    let stdout = dry_run_with_gitignore(root);

    assert!(stdout.contains("1 target(s) found"), "stdout: {stdout}");
    assert!(stdout.contains("[gitignore]  pkgs"), "stdout: {stdout}");
    assert!(!stdout.contains("node_modules"), "stdout: {stdout}");
}

#[test]
fn gitignore_match_inside_adapter_target_is_not_listed() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".gitignore"), b"*.log\n");
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(
        &root.join("node_modules").join("dep").join("debug.log"),
        b"log",
    );

    let stdout = dry_run_with_gitignore(root);

    assert!(stdout.contains("1 target(s) found"), "stdout: {stdout}");
    assert!(stdout.contains("[node]  node_modules"), "stdout: {stdout}");
    assert!(!stdout.contains("debug.log"), "stdout: {stdout}");
}

#[test]
fn nested_targets_are_deleted_without_errors() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".gitignore"), b"pkgs/\n");
    common::make_file(&root.join("pkgs").join("app").join("package.json"), b"{}");
    common::make_file(
        &root
            .join("pkgs")
            .join("app")
            .join("node_modules")
            .join("a.js"),
        b"",
    );

    let output = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--gitignore"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(!stderr.contains("Failed"), "stderr: {stderr}");
    assert!(!root.join("pkgs").exists());
}
