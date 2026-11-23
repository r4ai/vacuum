//! Paths that cannot be read are reported, not silently skipped.
#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A project with a target and a directory nobody may read. `None` when
/// permissions do not apply, as for root.
fn tree_with_unreadable_dir() -> Option<(TempDir, PathBuf)> {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"x");
    let locked = root.join("locked");
    common::make_file(&locked.join("secret"), b"x");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        unlock(&locked);
        return None;
    }
    Some((dir, locked))
}

/// Let `TempDir` clean up.
fn unlock(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn list_mode_reports_unreadable_paths() {
    let Some((dir, locked)) = tree_with_unreadable_dir() else {
        eprintln!("skipped: running with permission to read everything");
        return;
    };

    let output = common::vacuum()
        .arg(dir.path())
        .args(["--mode", "list", "--gitignore"])
        .output()
        .unwrap();
    unlock(&locked);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("node_modules"), "{stdout}");
    assert!(stderr.contains("Could not read 1 path"), "{stderr}");
    assert_eq!(
        stderr.matches("locked").count(),
        1,
        "reported once: {stderr}"
    );
}

#[test]
fn json_lists_unreadable_paths() {
    let Some((dir, locked)) = tree_with_unreadable_dir() else {
        eprintln!("skipped: running with permission to read everything");
        return;
    };

    let output = common::vacuum()
        .arg(dir.path())
        .args(["--mode", "list", "--json"])
        .output()
        .unwrap();
    unlock(&locked);

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let skipped = value["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{value}");
    assert!(skipped[0]["path"].as_str().unwrap().ends_with("locked"));
    assert!(!skipped[0]["error"].as_str().unwrap().is_empty());
}

#[test]
fn json_has_no_skipped_paths_in_a_readable_tree() {
    let dir = TempDir::new().unwrap();
    common::make_file(&dir.path().join("package.json"), b"{}");

    let output = common::vacuum()
        .arg(dir.path())
        .args(["--mode", "list", "--json"])
        .output()
        .unwrap();

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["skipped"], serde_json::json!([]));
}
