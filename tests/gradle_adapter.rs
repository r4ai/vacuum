mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_gradle_artifacts() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("build.gradle"), b"plugins { id 'java' }");
    common::make_dir(&root.join(".gradle").join("8.0"));
    common::make_dir(&root.join("build").join("classes"));
    common::make_file(
        &root.join("build").join("classes").join("App.class"),
        b"cafebabe",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join(".gradle").exists(), ".gradle/ should be deleted");
    assert!(!root.join("build").exists(), "build/ should be deleted");
    assert!(
        root.join("build.gradle").exists(),
        "build.gradle should be preserved"
    );
}
