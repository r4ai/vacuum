mod common;

use std::process::Command;
use tempfile::TempDir;

fn json(args: &[&str], root: &std::path::Path) -> serde_json::Value {
    let output = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--json"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stdout)))
}

fn node_project(root: &std::path::Path) {
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"12345");
}

#[test]
fn json_lists_targets_without_deleting_on_dry_run() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    node_project(root);

    let value = json(&["--dry-run"], root);

    let targets = value["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0]["adapter"], "node");
    assert_eq!(targets[0]["size"], 5);
    assert_eq!(
        targets[0]["path"],
        // vacuum prints canonical paths (e.g. /private/var on macOS).
        root.canonicalize()
            .unwrap()
            .join("node_modules")
            .display()
            .to_string()
    );
    assert_eq!(value["total_size"], 5);
    assert_eq!(value["result"]["dry_run"], true);
    assert_eq!(value["result"]["freed"], 0);
    assert!(root.join("node_modules").exists());
}

#[test]
fn json_reports_deletion_result() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    node_project(root);

    let value = json(&[], root);

    assert_eq!(value["result"]["dry_run"], false);
    assert_eq!(value["result"]["freed"], 5);
    assert_eq!(value["result"]["errors"].as_array().unwrap().len(), 0);
    assert!(!root.join("node_modules").exists());
}

#[test]
fn json_with_no_targets_is_still_json() {
    let dir = TempDir::new().unwrap();
    let value = json(&[], dir.path());
    assert_eq!(value["targets"].as_array().unwrap().len(), 0);
}

#[test]
fn json_requires_auto_mode() {
    let dir = TempDir::new().unwrap();
    let output = Command::new(common::vacuum_bin())
        .arg(dir.path())
        .arg("--json")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("--json"), "{stderr}");
}

#[cfg(target_os = "linux")]
#[test]
fn json_reports_trashed_bytes_apart_from_freed_bytes() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    node_project(&root);
    let data_home = dir.path().join("data");
    std::fs::create_dir_all(&data_home).unwrap();

    let output = Command::new(common::vacuum_bin())
        .arg(&root)
        .args(["--mode", "auto", "--json", "--trash"])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["method"], "trash");
    assert_eq!(value["result"]["freed"], 0);
    assert_eq!(value["result"]["trashed"], 5);
}
