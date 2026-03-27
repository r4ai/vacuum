mod common;

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
