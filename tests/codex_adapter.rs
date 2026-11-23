mod common;

use std::path::Path;
use tempfile::TempDir;

fn vacuum(root: &Path, args: &[&str]) {
    let status = common::vacuum()
        .arg(root)
        .args(["--mode", "auto"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

/// A `~/.codex` with session data, config and a managed worktree.
fn make_codex_home(home: &Path) -> std::path::PathBuf {
    let codex = home.join(".codex");
    common::make_file(&codex.join("auth.json"), b"{}");
    common::make_file(&codex.join("config.toml"), b"");
    common::make_file(&codex.join("history.jsonl"), b"{}");
    common::make_file(&codex.join("AGENTS.md"), b"keep");
    common::make_file(
        &codex
            .join("sessions")
            .join("2026")
            .join("11")
            .join("01")
            .join("rollout-a.jsonl"),
        b"{}",
    );
    common::make_file(
        &codex.join("archived_sessions").join("rollout-b.jsonl"),
        b"{}",
    );
    common::make_file(&codex.join("log").join("codex-tui.log"), b"log");
    common::make_file(&codex.join("shell_snapshots").join("s.sh"), b"env");
    common::make_file(
        &codex
            .join("worktrees")
            .join("ab12")
            .join("app")
            .join("main.rs"),
        b"keep",
    );
    codex
}

#[test]
fn sessions_logs_and_snapshots_are_deleted() {
    let dir = TempDir::new().unwrap();
    let codex = make_codex_home(dir.path());

    vacuum(dir.path(), &["--codex"]);

    for dir in ["sessions", "archived_sessions", "log", "shell_snapshots"] {
        assert!(!codex.join(dir).exists(), "{dir}/ should be deleted");
    }
}

#[test]
fn config_and_worktrees_are_kept() {
    let dir = TempDir::new().unwrap();
    let codex = make_codex_home(dir.path());

    vacuum(dir.path(), &["--codex"]);

    assert!(codex.join("auth.json").exists());
    assert!(codex.join("config.toml").exists());
    assert!(codex.join("history.jsonl").exists());
    assert!(codex.join("AGENTS.md").exists());
    assert!(
        codex
            .join("worktrees")
            .join("ab12")
            .join("app")
            .join("main.rs")
            .exists(),
        "worktrees may hold uncommitted work"
    );
}

#[test]
fn disabled_by_default() {
    let dir = TempDir::new().unwrap();
    let codex = make_codex_home(dir.path());

    vacuum(dir.path(), &[]);

    assert!(codex.join("sessions").exists());
}

#[test]
fn project_codex_dir_is_kept() {
    // A project's `.codex/` holds shared config, not session data.
    let dir = TempDir::new().unwrap();
    let codex = dir.path().join("app").join(".codex");
    common::make_file(&codex.join("config.toml"), b"");
    common::make_file(&codex.join("log").join("notes.md"), b"keep");

    vacuum(dir.path(), &["--codex"]);

    assert!(codex.join("log").join("notes.md").exists());
}
