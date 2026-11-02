mod common;

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn node_project(dir: &Path) {
    common::make_file(&dir.join("package.json"), b"{}");
    common::make_file(&dir.join("node_modules").join("a.js"), b"");
}

#[test]
fn multiple_paths_are_all_scanned() {
    let dir = TempDir::new().unwrap();
    let work = dir.path().join("work");
    let oss = dir.path().join("oss");
    let other = dir.path().join("other");
    for p in [&work, &oss, &other] {
        node_project(p);
    }

    let status = Command::new(common::vacuum_bin())
        .args([&work, &oss])
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!work.join("node_modules").exists());
    assert!(!oss.join("node_modules").exists());
    assert!(other.join("node_modules").exists(), "unlisted path is kept");
}

#[test]
fn overlapping_paths_list_each_target_once() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    node_project(&root.join("app"));

    let output = Command::new(common::vacuum_bin())
        .arg(root)
        .arg(root.join("app"))
        .args(["--mode", "auto", "--dry-run", "--no-size"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("1 target(s) found"), "{stdout}");
}

#[test]
fn paths_are_shown_relative_to_their_common_ancestor() {
    let dir = TempDir::new().unwrap();
    let work = dir.path().join("work");
    let oss = dir.path().join("oss");
    node_project(&work);
    node_project(&oss);

    let output = Command::new(common::vacuum_bin())
        .args([&work, &oss])
        .args(["--mode", "auto", "--dry-run", "--no-size"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("work/node_modules"), "{stdout}");
    assert!(stdout.contains("oss/node_modules"), "{stdout}");
}
