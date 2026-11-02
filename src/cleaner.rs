use anyhow::Context as _;
use bytesize::ByteSize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;

use crate::adapter::CleanTarget;

// ─── Removal ──────────────────────────────────────────────────────────────────

/// How targets are removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteMethod {
    /// Delete permanently.
    Remove,
    /// Move to the OS trash, so the target can be restored.
    Trash,
}

impl DeleteMethod {
    /// How to report the bytes handled. Trashed bytes still occupy disk
    /// space, so they are not reported as freed.
    pub fn done_label(self) -> &'static str {
        match self {
            DeleteMethod::Remove => "Freed",
            DeleteMethod::Trash => "Moved to trash",
        }
    }
}

/// Remove one target with `method`.
fn remove(target: &CleanTarget, method: DeleteMethod) -> anyhow::Result<()> {
    let path = &target.path;
    match method {
        DeleteMethod::Trash => trash::delete(path)
            .with_context(|| format!("Failed to move to trash: {}", path.display())),
        DeleteMethod::Remove if path.is_dir() => fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove directory: {}", path.display())),
        DeleteMethod::Remove => fs::remove_file(path)
            .with_context(|| format!("Failed to remove file: {}", path.display())),
    }
}

// ─── TUI-compatible deletion (background thread) ─────────────────────────────

/// Progress event sent from `delete_with_progress` to the TUI thread.
#[derive(Debug)]
pub enum DeleteMsg {
    Progress {
        path: String,
        freed: u64,
        done: usize,
    },
    Done {
        freed: u64,
        errors: Vec<(String, String)>,
    },
}

/// Delete targets and stream progress to the caller via `tx`.
/// Designed to run inside `std::thread::spawn`.
/// On `dry_run = true` the files are not deleted; progress events are still sent.
pub fn delete_with_progress(
    targets: Vec<CleanTarget>,
    dry_run: bool,
    method: DeleteMethod,
    tx: std::sync::mpsc::Sender<DeleteMsg>,
) {
    let mut freed: u64 = 0;
    let mut errors: Vec<(String, String)> = Vec::new();

    for (i, target) in targets.iter().enumerate() {
        let path_str = target.path.display().to_string();

        if !dry_run {
            let result = remove(target, method);

            match result {
                Ok(()) => freed += target.size,
                Err(e) => errors.push((path_str.clone(), e.to_string())),
            }
        }

        let _ = tx.send(DeleteMsg::Progress {
            path: path_str,
            freed,
            done: i + 1,
        });
    }

    let _ = tx.send(DeleteMsg::Done { freed, errors });
}

// ─── Classic indicatif-based deletion (used for auto mode) ───────────────────

/// Delete the given targets, showing a progress bar.
/// In dry-run mode, only prints what would be deleted.
pub fn delete_targets(
    targets: &[CleanTarget],
    dry_run: bool,
    method: DeleteMethod,
) -> anyhow::Result<()> {
    if targets.is_empty() {
        println!("Nothing to delete.");
        return Ok(());
    }

    if dry_run {
        println!("Dry run — the following would be deleted:");
        let total: u64 = targets.iter().map(|t| t.size).sum();
        for t in targets {
            println!(
                "  {}  ({})",
                t.path.display(),
                ByteSize(t.size).display().si()
            );
        }
        println!("\nTotal: {}", ByteSize(total).display().si());
        return Ok(());
    }

    let pb = ProgressBar::new(targets.len() as u64);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len}  {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let mut errors: Vec<(String, anyhow::Error)> = Vec::new();
    let mut freed: u64 = 0;

    for target in targets {
        pb.set_message(format!("{}", target.path.display()));

        let result = remove(target, method);

        match result {
            Ok(()) => freed += target.size,
            Err(e) => errors.push((target.path.display().to_string(), e)),
        }
        pb.inc(1);
    }

    pb.finish_with_message(format!(
        "Done. {} {}",
        method.done_label(),
        ByteSize(freed).display().si()
    ));

    if !errors.is_empty() {
        eprintln!("\nErrors during deletion:");
        for (path, err) in &errors {
            eprintln!("  {path}: {err}");
        }
        anyhow::bail!("failed to delete {} target(s)", errors.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn delete_targets_fails_when_a_target_cannot_be_deleted() {
        let dir = TempDir::new().unwrap();
        let missing = CleanTarget {
            tracked: false,
            path: dir.path().join("missing"),
            adapter: "test",
            description: "test".into(),
            size: 0,
        };
        assert!(delete_targets(&[missing], false, DeleteMethod::Remove).is_err());
    }

    #[test]
    fn delete_targets_succeeds_when_all_targets_are_deleted() {
        let dir = TempDir::new().unwrap();
        let path: PathBuf = dir.path().join("file");
        fs::write(&path, b"x").unwrap();
        let target = CleanTarget {
            tracked: false,
            path: path.clone(),
            adapter: "test",
            description: "test".into(),
            size: 1,
        };
        assert!(delete_targets(&[target], false, DeleteMethod::Remove).is_ok());
        assert!(!path.exists());
    }
}

#[cfg(test)]
mod method_tests {
    use super::*;

    #[test]
    fn trash_is_not_reported_as_freed() {
        assert_eq!(DeleteMethod::Remove.done_label(), "Freed");
        assert_eq!(DeleteMethod::Trash.done_label(), "Moved to trash");
    }
}
