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

fn make_gemini_home(home: &Path) -> std::path::PathBuf {
    let gemini = home.join(".gemini");
    common::make_file(&gemini.join("settings.json"), b"{}");
    common::make_file(&gemini.join("GEMINI.md"), b"keep");
    common::make_file(
        &gemini
            .join("tmp")
            .join("my-app")
            .join("chats")
            .join("session.jsonl"),
        b"{}",
    );
    gemini
}

#[test]
fn tmp_is_deleted_and_config_is_kept() {
    let dir = TempDir::new().unwrap();
    let gemini = make_gemini_home(dir.path());

    vacuum(dir.path(), &["--gemini-cli"]);

    assert!(!gemini.join("tmp").exists(), "tmp/ should be deleted");
    assert!(gemini.join("settings.json").exists());
    assert!(gemini.join("GEMINI.md").exists());
}

#[test]
fn disabled_by_default() {
    let dir = TempDir::new().unwrap();
    let gemini = make_gemini_home(dir.path());

    vacuum(dir.path(), &[]);

    assert!(gemini.join("tmp").exists());
}

#[test]
fn tmp_outside_the_config_dir_is_kept() {
    let dir = TempDir::new().unwrap();
    common::make_file(&dir.path().join("app").join("tmp").join("x"), b"keep");

    vacuum(dir.path(), &["--gemini-cli"]);

    assert!(dir.path().join("app").join("tmp").join("x").exists());
}
