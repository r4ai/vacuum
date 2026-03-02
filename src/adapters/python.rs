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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn pycache_with_py_file_in_same_dir_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::create_dir(dir.path().join("__pycache__")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join("__pycache__")));
    }

    #[test]
    fn pycache_with_pyproject_toml_in_ancestor_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]").unwrap();
        let subdir = dir.path().join("src").join("pkg");
        fs::create_dir_all(&subdir).unwrap();
        fs::create_dir(subdir.join("__pycache__")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == subdir.join("__pycache__")));
    }

    #[test]
    fn pycache_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("__pycache__")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(
            targets.iter().all(|t| !t.path.ends_with("__pycache__")),
            "Should not detect __pycache__ without Python context"
        );
    }

    #[test]
    fn pyc_file_detected() {
        let dir = TempDir::new().unwrap();
        let pyc = dir.path().join("main.pyc");
        fs::write(&pyc, b"\x00\x00\x00\x00").unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == pyc));
    }

    #[test]
    fn venv_with_requirements_txt_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask==2.0.0").unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join(".venv")));
    }

    #[test]
    fn plain_venv_with_requirements_txt_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "django").unwrap();
        fs::create_dir(dir.path().join("venv")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join("venv")));
    }

    #[test]
    fn venv_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().all(|t| t.path != dir.path().join(".venv")));
    }

    #[test]
    fn dist_with_setup_py_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("setup.py"), "from setuptools import setup").unwrap();
        fs::create_dir(dir.path().join("dist")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join("dist")));
    }

    #[test]
    fn build_with_setup_cfg_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("setup.cfg"), "[metadata]").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join("build")));
    }

    #[test]
    fn eggs_with_pyproject_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]").unwrap();
        fs::create_dir(dir.path().join(".eggs")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join(".eggs")));
    }

    #[test]
    fn contents_of_detected_venv_not_listed_separately() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        let venv = dir.path().join(".venv");
        fs::create_dir_all(venv.join("lib")).unwrap();
        fs::create_dir(venv.join("__pycache__")).unwrap();

        let targets = PythonAdapter.scan(dir.path()).unwrap();
        let venv_count = targets.iter().filter(|t| t.path == dir.path().join(".venv")).count();
        let inner_pycache_count = targets
            .iter()
            .filter(|t| t.path == venv.join("__pycache__"))
            .count();
        assert_eq!(venv_count, 1);
        assert_eq!(inner_pycache_count, 0);
    }
}
