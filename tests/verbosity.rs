mod common;

use std::path::Path;
use std::process::Output;
use tempfile::TempDir;

fn node_project(root: &Path) {
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"12345");
}

fn vacuum(root: &Path, args: &[&str]) -> Output {
    let output = common::vacuum().arg(root).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn auto_mode_prints_a_final_result() {
    let dir = TempDir::new().unwrap();
    node_project(dir.path());

    let output = vacuum(dir.path(), &["--mode", "auto"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Freed 5 B"), "{stdout}");
}

#[test]
fn quiet_prints_only_the_final_result() {
    let dir = TempDir::new().unwrap();
    node_project(dir.path());

    let output = vacuum(dir.path(), &["--mode", "auto", "--quiet"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stdout.trim(), "Freed 5 B (1 target)", "{stdout}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(!dir.path().join("node_modules").exists());
}

#[test]
fn quiet_dry_run_prints_only_the_total() {
    let dir = TempDir::new().unwrap();
    node_project(dir.path());

    let output = vacuum(dir.path(), &["--mode", "auto", "--quiet", "--dry-run"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "Would free 5 B (1 target)", "{stdout}");
    assert!(dir.path().join("node_modules").exists());
}

#[test]
fn verbose_explains_skipped_targets() {
    let dir = TempDir::new().unwrap();
    node_project(dir.path());

    let output = vacuum(
        dir.path(),
        &["--mode", "list", "--verbose", "--min-size", "1MB"],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("skip"), "{stderr}");
    assert!(stderr.contains("node_modules"), "{stderr}");
    assert!(stderr.contains("--min-size"), "{stderr}");
}

#[test]
fn quiet_and_verbose_conflict() {
    let dir = TempDir::new().unwrap();
    let output = common::vacuum()
        .arg(dir.path())
        .args(["--mode", "list", "--quiet", "--verbose"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
