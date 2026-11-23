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

fn make_repo(root: &Path) {
    common::make_dir(&root.join(".git"));
    common::make_file(&root.join(".aider.chat.history.md"), b"# chat");
    common::make_file(&root.join(".aider.input.history"), b"> hi");
    common::make_file(&root.join(".aider.llm.history"), b"raw");
    common::make_file(&root.join(".aider.tags.cache.v4").join("cache.db"), b"db");
    common::make_file(&root.join(".aider.conf.yml"), b"model: x");
    common::make_file(&root.join("main.py"), b"keep");
}

#[test]
fn history_and_tags_cache_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_repo(root);

    vacuum(root, &["--aider"]);

    assert!(!root.join(".aider.chat.history.md").exists());
    assert!(!root.join(".aider.input.history").exists());
    assert!(!root.join(".aider.llm.history").exists());
    assert!(!root.join(".aider.tags.cache.v4").exists());
}

#[test]
fn config_and_sources_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_repo(root);

    vacuum(root, &["--aider"]);

    assert!(
        root.join(".aider.conf.yml").exists(),
        "config should be kept"
    );
    assert!(root.join("main.py").exists());
}

#[test]
fn disabled_by_default() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_repo(root);

    vacuum(root, &[]);

    assert!(root.join(".aider.chat.history.md").exists());
}
