mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[test]
fn gitignore_adapter_deletes_matched_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".gitignore"), b"*.log\n");
    common::make_file(&root.join("app.log"), b"log content");
    common::make_file(&root.join("app.py"), b"# not ignored");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args([
            "--mode",
            "auto",
            "--gitignore",
            "--node=false",
            "--cargo=false",
            "--python=false",
            "--go=false",
            "--gradle=false",
            "--maven=false",
        ])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !root.join("app.log").exists(),
        "app.log should be deleted (matches *.log)"
    );
    assert!(root.join("app.py").exists(), "app.py should be preserved");
}

const GITIGNORE_ONLY: &[&str] = &[
    "--mode",
    "auto",
    "--gitignore",
    "--node=false",
    "--cargo=false",
    "--python=false",
    "--go=false",
    "--gradle=false",
    "--maven=false",
    "--ocaml=false",
];

/// Create a Git repository at `root` with the given `.gitignore`,
/// then add a real worktree at `worktree_rel`. Returns the worktree path.
fn setup_repo_with_worktree(root: &Path, gitignore: &str, worktree_rel: &str) -> PathBuf {
    common::make_file(&root.join(".gitignore"), gitignore.as_bytes());
    common::make_file(&root.join("main.rs"), b"fn main() {}");
    common::git(root, &["init", "-q"]);
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-q", "-m", "init"]);
    common::git(
        root,
        &["worktree", "add", "-q", "-b", "feature", worktree_rel],
    );
    root.join(worktree_rel)
}

fn run_vacuum(root: &Path, args: &[&str]) {
    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

/// Run the gitignore adapter on a repo whose worktree lives in an ignored
/// directory, and check that only ignored files inside the worktree are removed.
fn assert_worktree_preserved(gitignore: &str, worktree_rel: &str) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let worktree = setup_repo_with_worktree(root, gitignore, worktree_rel);
    common::make_file(&worktree.join("debug.log"), b"log");
    common::make_file(&worktree.join("wip.rs"), b"// uncommitted work");
    common::make_file(&root.join("app.log"), b"log");

    run_vacuum(root, GITIGNORE_ONLY);

    assert!(!root.join("app.log").exists(), "app.log should be deleted");
    assert!(
        worktree.join(".git").exists(),
        "worktree should be preserved"
    );
    assert!(
        worktree.join("main.rs").exists(),
        "committed files in worktree should be preserved"
    );
    assert!(
        worktree.join("wip.rs").exists(),
        "uncommitted work in worktree should be preserved"
    );
    assert!(
        !worktree.join("debug.log").exists(),
        "debug.log in worktree should be deleted by the worktree's .gitignore"
    );

    // Git still recognizes the worktree.
    let output = Command::new("git")
        .current_dir(&worktree)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(output.status.success(), "worktree should remain usable");
}

#[test]
fn gitignore_adapter_preserves_worktree_in_ignored_dir() {
    assert_worktree_preserved(".worktrees/\n*.log\n", ".worktrees/feature");
}

#[test]
fn gitignore_adapter_preserves_claude_code_worktree() {
    assert_worktree_preserved(".claude/worktrees/\n*.log\n", ".claude/worktrees/feature");
}

#[test]
fn gitignore_adapter_preserves_claude_code_worktree_when_claude_dir_ignored() {
    assert_worktree_preserved(".claude/\n*.log\n", ".claude/worktrees/feature");
}

#[test]
fn gitignore_adapter_deletes_ignored_siblings_of_worktree() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let worktree = setup_repo_with_worktree(root, ".claude/\n", ".claude/worktrees/feature");
    common::make_file(&root.join(".claude").join("settings.local.json"), b"{}");

    run_vacuum(root, GITIGNORE_ONLY);

    assert!(
        !root.join(".claude").join("settings.local.json").exists(),
        "ignored file next to the worktree should be deleted"
    );
    assert!(
        worktree.join("main.rs").exists(),
        "worktree should be preserved"
    );
}

#[test]
fn default_adapters_clean_inside_claude_code_worktree() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let worktree =
        setup_repo_with_worktree(root, ".claude/worktrees/\n", ".claude/worktrees/feature");
    common::make_file(&worktree.join("package.json"), b"{}");
    common::make_file(&worktree.join("node_modules").join("a.js"), b"");
    common::make_file(&worktree.join("Cargo.toml"), b"[package]");
    common::make_file(&worktree.join("target").join("debug").join("app"), b"");

    run_vacuum(root, &["--mode", "auto"]);

    assert!(!worktree.join("node_modules").exists());
    assert!(!worktree.join("target").exists());
    assert!(worktree.join("package.json").exists());
    assert!(worktree.join("main.rs").exists());
}
