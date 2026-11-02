#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;

pub fn vacuum_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vacuum")
}

/// A command that runs vacuum without the developer's config file.
pub fn vacuum() -> Command {
    let mut command = Command::new(vacuum_bin());
    command.env("XDG_CONFIG_HOME", "/nonexistent/vacuum-test-config");
    command
}

pub fn make_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

pub fn make_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

/// Run `git` in `dir` with a fixed identity, panicking on failure.
pub fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=vacuum",
            "-c",
            "user.email=vacuum@example.com",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}
