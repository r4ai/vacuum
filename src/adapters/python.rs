use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct PythonAdapter;

const PYTHON_DIR_TARGETS: &[&str] = &["__pycache__", ".venv", "venv", "dist", "build", ".eggs"];
const PYTHON_CONTEXT_FILES: &[&str] = &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"];

fn has_python_context(dir: &Path) -> bool {
    PYTHON_CONTEXT_FILES.iter().any(|f| dir.join(f).exists())
}

impl Adapter for PythonAdapter {
    fn name(&self) -> &'static str {
        "python"
    }

    fn description(&self) -> &str {
        "Python build artifacts and virtual environments (__pycache__/, .venv/, dist/, etc.)"
    }

    fn is_safe(&self) -> bool {
        true
    }

    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        let mut targets = Vec::new();
        let mut skip_prefixes: Vec<std::path::PathBuf> = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if skip_prefixes.iter().any(|p| path.starts_with(p)) {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy();

            if entry.file_type().is_dir() {
                let name = file_name.as_ref();
                if !PYTHON_DIR_TARGETS.contains(&name) {
                    continue;
                }

                // For __pycache__, just check some parent up to 3 levels has python context
                // For venv-like dirs, check the same directory has python context
                let parent = match path.parent() {
                    Some(p) => p,
                    None => continue,
                };

                let has_context = if name == "__pycache__" {
                    // __pycache__ appears next to .py files; check up the tree
                    let mut p = parent;
                    let mut found = false;
                    for _ in 0..4 {
                        if has_python_context(p) {
                            found = true;
                            break;
                        }
                        match p.parent() {
                            Some(pp) => p = pp,
                            None => break,
                        }
                    }
                    // Also accept if there are .py files in same directory
                    found || parent.read_dir().is_ok_and(|mut d| {
                        d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|ext| ext == "py")))
                    })
                } else {
                    has_python_context(parent)
                };

                if !has_context {
                    continue;
                }

                let size = compute_dir_size(path);
                targets.push(CleanTarget {
                    path: path.to_path_buf(),
                    adapter: self.name(),
                    description: format!("Python build artifact ({name}/)"),
                    size,
                });
                skip_prefixes.push(path.to_path_buf());
            } else if entry.file_type().is_file() {
                // *.pyc files
                if !file_name.ends_with(".pyc") {
                    continue;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                targets.push(CleanTarget {
                    path: path.to_path_buf(),
                    adapter: self.name(),
                    description: "Python bytecode (.pyc)".into(),
                    size,
                });
            }
        }

        Ok(targets)
    }
}
