mod common;

use std::path::Path;
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

    let status = common::vacuum()
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

    let output = common::vacuum()
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

    let output = common::vacuum()
        .args([&work, &oss])
        .args(["--mode", "auto", "--dry-run", "--no-size"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    for project in ["work", "oss"] {
        let shown = std::path::Path::new(project).join("node_modules");
        assert!(stdout.contains(&*shown.to_string_lossy()), "{stdout}");
    }
}
