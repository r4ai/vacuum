mod common;

use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

/// Set the modification time of every file under `dir` to `days` ago.
fn age_files(dir: &Path, days: u64) {
    let time = SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60);
    for entry in walkdir(dir) {
        File::options()
            .write(true)
            .open(&entry)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walkdir(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// `old` was last touched 60 days ago, `new` just now.
fn setup(root: &Path) {
    for name in ["old", "new"] {
        common::make_file(&root.join(name).join("package.json"), b"{}");
        common::make_file(&root.join(name).join("index.js"), b"");
        common::make_file(&root.join(name).join("node_modules").join("a.js"), b"");
    }
    age_files(&root.join("old"), 60);
    // A fresh artifact must not make the old project look active.
    common::make_file(&root.join("old").join("node_modules").join("b.js"), b"");
}

fn vacuum(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn older_than_keeps_recently_modified_projects() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    let output = vacuum(root, &["--older-than", "30d"]);

    assert!(output.status.success());
    assert!(!root.join("old").join("node_modules").exists());
    assert!(root.join("new").join("node_modules").exists());
}

#[test]
fn older_than_accepts_hours_and_weeks() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    assert!(
        vacuum(root, &["--older-than", "12w", "--dry-run"])
            .status
            .success()
    );
    assert!(
        vacuum(root, &["--older-than", "48h", "--dry-run"])
            .status
            .success()
    );
}

#[test]
fn invalid_older_than_is_rejected() {
    let dir = TempDir::new().unwrap();
    let output = vacuum(dir.path(), &["--older-than", "soon"]);
    assert!(!output.status.success());
}
