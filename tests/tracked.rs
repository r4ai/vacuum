mod common;

use std::path::Path;
use tempfile::TempDir;

/// A Go project whose `vendor/` is committed, and a Node project whose
/// `node_modules/` is not.
fn setup(root: &Path) {
    common::make_file(&root.join("app").join("go.mod"), b"module app\n");
    common::make_file(
        &root.join("app").join("vendor").join("lib.go"),
        b"package lib",
    );
    common::make_file(&root.join("web").join("package.json"), b"{}");
    common::make_file(&root.join(".gitignore"), b"node_modules/\n");
    common::git(root, &["init", "-q"]);
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-q", "-m", "init"]);
    common::make_file(&root.join("web").join("node_modules").join("a.js"), b"");
}

/// A relative path as vacuum prints it, with the platform's separator.
fn shown(dir: &str, name: &str) -> String {
    Path::new(dir).join(name).display().to_string()
}

fn vacuum(root: &Path, args: &[&str]) -> std::process::Output {
    let output = common::vacuum()
        .arg(root)
        .args(["--mode", "auto"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn tracked_targets_are_excluded_by_default() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    vacuum(root, &[]);

    assert!(
        root.join("app").join("vendor").exists(),
        "committed vendor/ must be preserved"
    );
    assert!(!root.join("web").join("node_modules").exists());
}

#[test]
fn include_tracked_lists_and_marks_tracked_targets() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    let output = vacuum(root, &["--include-tracked", "--dry-run", "--no-size"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    let vendor_line = stdout
        .lines()
        .find(|l| l.contains(&*shown("app", "vendor")))
        .unwrap_or_else(|| panic!("vendor not listed: {stdout}"));
    assert!(vendor_line.contains("[tracked]"), "{vendor_line}");
    let node_line = stdout
        .lines()
        .find(|l| l.contains(&*shown("web", "node_modules")))
        .unwrap_or_else(|| panic!("node_modules not listed: {stdout}"));
    assert!(!node_line.contains("[tracked]"), "{node_line}");
}

#[test]
fn include_tracked_deletes_tracked_targets() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    vacuum(root, &["--include-tracked"]);

    assert!(!root.join("app").join("vendor").exists());
}

#[test]
fn targets_outside_a_repository_are_kept_as_candidates() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("go.mod"), b"module app\n");
    common::make_file(&root.join("vendor").join("lib.go"), b"package lib");

    vacuum(root, &[]);

    assert!(!root.join("vendor").exists());
}
