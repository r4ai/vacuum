use anyhow::Context as _;
use bytesize::ByteSize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;

use crate::adapter::CleanTarget;

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
    tx: std::sync::mpsc::Sender<DeleteMsg>,
) {
    let mut freed: u64 = 0;
    let mut errors: Vec<(String, String)> = Vec::new();

    for (i, target) in targets.iter().enumerate() {
        let path_str = target.path.display().to_string();

        if !dry_run {
            let result = if target.path.is_dir() {
                fs::remove_dir_all(&target.path).with_context(|| {
                    format!("Failed to remove directory: {}", target.path.display())
                })
            } else {
                fs::remove_file(&target.path)
                    .with_context(|| format!("Failed to remove file: {}", target.path.display()))
            };

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
pub fn delete_targets(targets: &[CleanTarget], dry_run: bool) -> anyhow::Result<()> {
    if targets.is_empty() {
        println!("Nothing to delete.");
        return Ok(());
    }

    if dry_run {
        println!("Dry run — the following would be deleted:");
        let total: u64 = targets.iter().map(|t| t.size).sum();
        for t in targets {
            println!("  {}  ({})", t.path.display(), ByteSize(t.size));
        }
        println!("\nTotal: {}", ByteSize(total));
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

        let result = if target.path.is_dir() {
            fs::remove_dir_all(&target.path)
                .with_context(|| format!("Failed to remove directory: {}", target.path.display()))
        } else {
            fs::remove_file(&target.path)
                .with_context(|| format!("Failed to remove file: {}", target.path.display()))
        };

        match result {
            Ok(()) => freed += target.size,
            Err(e) => errors.push((target.path.display().to_string(), e)),
        }
        pb.inc(1);
    }

    pb.finish_with_message(format!("Done. Freed {}", ByteSize(freed)));

    if !errors.is_empty() {
        eprintln!("\nErrors during deletion:");
        for (path, err) in &errors {
            eprintln!("  {path}: {err}");
        }
    }

    Ok(())
}
