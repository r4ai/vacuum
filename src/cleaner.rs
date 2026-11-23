use anyhow::Context as _;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;

use crate::adapter::{CleanTarget, EntryKind};

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
///
/// Time passes between the scan and the removal, above all in safe mode.
/// A target whose type changed since the scan, such as a directory
/// replaced by a symlink, is not removed.
fn remove(target: &CleanTarget, method: DeleteMethod) -> anyhow::Result<()> {
    let path = &target.path;
    let Some(kind) = EntryKind::at(path) else {
        anyhow::bail!("No longer exists: {}", path.display());
    };
    if kind != target.kind {
        anyhow::bail!(
            "Skipped, changed since the scan (was {:?}, now {kind:?}): {}",
            target.kind,
            path.display()
        );
    }
    match method {
        DeleteMethod::Trash => trash::delete(path)
            .with_context(|| format!("Failed to move to trash: {}", path.display())),
        // `kind` does not follow symlinks, so a symlink is removed as a file.
        DeleteMethod::Remove if kind == EntryKind::Dir => fs::remove_dir_all(path)
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

/// The outcome of removing targets.
#[derive(Debug, Default)]
pub struct DeleteReport {
    /// Bytes of the targets removed.
    pub freed: u64,
    /// `(path, error)` for each target that could not be removed.
    pub errors: Vec<(String, String)>,
}

/// Remove `targets` without printing anything. `after_each` runs after
/// each target, removed or not.
pub fn remove_all(
    targets: &[CleanTarget],
    method: DeleteMethod,
    mut after_each: impl FnMut(&CleanTarget),
) -> DeleteReport {
    let mut report = DeleteReport::default();
    for target in targets {
        match remove(target, method) {
            Ok(()) => report.freed += target.size,
            Err(e) => report
                .errors
                .push((target.path.display().to_string(), format!("{e:#}"))),
        }
        after_each(target);
    }
    report
}

/// Delete the given targets, showing a progress bar.
/// In dry-run mode, deletes and prints nothing.
pub fn delete_targets(
    targets: &[CleanTarget],
    dry_run: bool,
    method: DeleteMethod,
    quiet: bool,
) -> DeleteReport {
    // The caller lists the targets, so a dry run has nothing to add.
    if dry_run {
        return DeleteReport::default();
    }

    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        ProgressBar::new(targets.len() as u64)
    };
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len}  {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let report = remove_all(targets, method, |target| {
        pb.set_message(format!("{}", target.path.display()));
        pb.inc(1);
    });
    pb.finish_and_clear();

    if !report.errors.is_empty() {
        eprintln!("Errors during deletion:");
        for (path, err) in &report.errors {
            eprintln!("  {path}: {err}");
        }
    }
    report
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
            kind: EntryKind::File,
        };
        let report = delete_targets(&[missing], false, DeleteMethod::Remove, true);
        assert_eq!(report.errors.len(), 1);
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
            kind: EntryKind::File,
        };
        let report = delete_targets(&[target], false, DeleteMethod::Remove, true);
        assert!(report.errors.is_empty());
        assert_eq!(report.freed, 1);
        assert!(!path.exists());
    }
}

#[cfg(all(test, unix))]
mod verify_tests {
    use super::*;
    use crate::adapter::EntryKind;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn dir_target(path: std::path::PathBuf) -> CleanTarget {
        CleanTarget {
            tracked: false,
            path,
            adapter: "test",
            description: "test".into(),
            size: 0,
            kind: EntryKind::Dir,
        }
    }

    #[test]
    fn a_directory_replaced_by_a_symlink_is_not_removed() {
        let dir = TempDir::new().unwrap();
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::write(elsewhere.join("precious"), b"x").unwrap();
        let target = dir.path().join("node_modules");
        symlink(&elsewhere, &target).unwrap();

        for method in [DeleteMethod::Remove, DeleteMethod::Trash] {
            let report = remove_all(&[dir_target(target.clone())], method, |_| {});
            assert_eq!(report.errors.len(), 1, "{method:?}");
            assert!(report.errors[0].1.contains("changed since the scan"));
        }
        assert!(target.is_symlink());
        assert!(elsewhere.join("precious").exists());
    }

    #[test]
    fn a_directory_replaced_by_a_file_is_not_removed() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("build");
        fs::write(&target, b"x").unwrap();

        let report = remove_all(&[dir_target(target.clone())], DeleteMethod::Remove, |_| {});

        assert_eq!(report.errors.len(), 1);
        assert!(target.exists());
    }

    #[test]
    fn a_scanned_symlink_is_removed_without_following_it() {
        let dir = TempDir::new().unwrap();
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::write(elsewhere.join("precious"), b"x").unwrap();
        let link = dir.path().join("link");
        symlink(&elsewhere, &link).unwrap();
        let target = CleanTarget {
            kind: EntryKind::Symlink,
            ..dir_target(link.clone())
        };

        let report = remove_all(&[target], DeleteMethod::Remove, |_| {});

        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(!link.exists() && !link.is_symlink());
        assert!(elsewhere.join("precious").exists());
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
