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

/// A `~/.claude` with session data and user-authored config.
fn make_claude_home(home: &Path) -> std::path::PathBuf {
    let claude = home.join(".claude");
    common::make_file(
        &claude.join("projects").join("-home-u-app").join("s.jsonl"),
        b"{}",
    );
    common::make_file(&claude.join("history.jsonl"), b"{}");
    common::make_file(&claude.join("settings.json"), b"{}");
    common::make_file(&claude.join("CLAUDE.md"), b"keep");
    common::make_file(&claude.join("agents").join("reviewer.md"), b"keep");
    for dir in [
        "debug",
        "shell-snapshots",
        "file-history",
        "paste-cache",
        "image-cache",
        "session-env",
        "todos",
        "statsig",
        "logs",
    ] {
        common::make_file(&claude.join(dir).join("x"), b"data");
    }
    claude
}

#[test]
fn session_data_in_the_config_dir_is_deleted() {
    let dir = TempDir::new().unwrap();
    let claude = make_claude_home(dir.path());

    vacuum(dir.path(), &["--claude-code"]);

    for dir in [
        "debug",
        "shell-snapshots",
        "file-history",
        "paste-cache",
        "image-cache",
        "session-env",
        "todos",
        "statsig",
        "logs",
    ] {
        assert!(!claude.join(dir).exists(), "{dir}/ should be deleted");
    }
}

#[test]
fn transcripts_and_config_are_kept() {
    let dir = TempDir::new().unwrap();
    let claude = make_claude_home(dir.path());

    vacuum(dir.path(), &["--claude-code"]);

    assert!(
        claude
            .join("projects")
            .join("-home-u-app")
            .join("s.jsonl")
            .exists()
    );
    assert!(claude.join("history.jsonl").exists());
    assert!(claude.join("settings.json").exists());
    assert!(claude.join("CLAUDE.md").exists());
    assert!(claude.join("agents").join("reviewer.md").exists());
}

#[test]
fn disabled_by_default() {
    let dir = TempDir::new().unwrap();
    let claude = make_claude_home(dir.path());

    vacuum(dir.path(), &[]);

    assert!(claude.join("debug").exists());
}

#[test]
fn project_claude_dir_is_kept() {
    // A project's `.claude/` holds shared config, not session data.
    let dir = TempDir::new().unwrap();
    let claude = dir.path().join("app").join(".claude");
    common::make_file(&claude.join("settings.json"), b"{}");
    common::make_file(&claude.join("logs").join("notes.md"), b"keep");

    vacuum(dir.path(), &["--claude-code"]);

    assert!(claude.join("logs").join("notes.md").exists());
}
