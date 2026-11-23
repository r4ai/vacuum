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

const DATA: [&str; 5] = [
    "browser_recordings",
    "brain",
    "conversations",
    "implicit",
    "code_tracker",
];

fn make_app_data(dir: &Path) {
    common::make_file(&dir.join("mcp_config.json"), b"{}");
    for name in DATA {
        common::make_file(&dir.join(name).join("x"), b"data");
    }
}

#[test]
fn agent_data_of_the_ide_and_the_cli_is_deleted() {
    let dir = TempDir::new().unwrap();
    let gemini = dir.path().join(".gemini");
    make_app_data(&gemini.join("antigravity"));
    make_app_data(&gemini.join("antigravity-cli"));

    vacuum(dir.path(), &["--antigravity"]);

    for app in ["antigravity", "antigravity-cli"] {
        for name in DATA {
            assert!(
                !gemini.join(app).join(name).exists(),
                "{app}/{name}/ should be deleted"
            );
        }
        assert!(gemini.join(app).join("mcp_config.json").exists());
    }
}

#[test]
fn disabled_by_default() {
    let dir = TempDir::new().unwrap();
    let app = dir.path().join(".gemini").join("antigravity");
    make_app_data(&app);

    vacuum(dir.path(), &[]);

    assert!(app.join("brain").exists());
}

#[test]
fn same_names_outside_the_app_data_dir_are_kept() {
    let dir = TempDir::new().unwrap();
    let app = dir.path().join("antigravity");
    make_app_data(&app);

    vacuum(dir.path(), &["--antigravity"]);

    assert!(app.join("brain").exists());
}
