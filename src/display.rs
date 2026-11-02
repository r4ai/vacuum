use bytesize::ByteSize;
use owo_colors::{OwoColorize, Stream};
use std::path::Path;

use crate::adapter::CleanTarget;
use crate::cleaner::DeleteMethod;

/// Print a summary table of all discovered targets.
pub fn print_targets(targets: &[CleanTarget], root: &Path) {
    if targets.is_empty() {
        println!(
            "{}",
            "No cleanup targets found.".if_supports_color(Stream::Stdout, |t| t.green())
        );
        return;
    }

    let total: u64 = targets.iter().map(|t| t.size).sum();

    println!(
        "\n{} {} target(s) found  {}\n",
        "→".if_supports_color(Stream::Stdout, |t| t.cyan()),
        targets
            .len()
            .if_supports_color(Stream::Stdout, |t| t.yellow()),
        format!("({})", ByteSize(total).display().si())
            .if_supports_color(Stream::Stdout, |t| t.bright_black()),
    );

    for t in targets {
        let rel = t.path.strip_prefix(root).unwrap_or(&t.path);
        let tracked = if t.tracked { "  [tracked]" } else { "" };
        println!(
            "  {}  {}  {}{}",
            format!("[{}]", t.adapter).if_supports_color(Stream::Stdout, |s| s.cyan()),
            rel.display(),
            format!("({})", ByteSize(t.size).display().si())
                .if_supports_color(Stream::Stdout, |s| s.bright_black()),
            tracked.if_supports_color(Stream::Stdout, |s| s.yellow()),
        );
    }

    println!();
}

/// Print the one-line result of a list or auto run.
/// `freed` and `removed` are ignored on a dry run.
pub fn print_summary(
    targets: &[CleanTarget],
    freed: u64,
    removed: usize,
    dry_run: bool,
    method: DeleteMethod,
) {
    println!("{}", summary_line(targets, freed, removed, dry_run, method));
}

fn summary_line(
    targets: &[CleanTarget],
    freed: u64,
    removed: usize,
    dry_run: bool,
    method: DeleteMethod,
) -> String {
    let count = |n: usize| format!("{n} target{}", if n == 1 { "" } else { "s" });
    if dry_run {
        let total: u64 = targets.iter().map(|t| t.size).sum();
        format!(
            "Would free {} ({})",
            ByteSize(total).display().si(),
            count(targets.len())
        )
    } else {
        format!(
            "{} {} ({})",
            method.done_label(),
            ByteSize(freed).display().si(),
            count(removed)
        )
    }
}

/// Per adapter: name, number of removed targets and their bytes, largest
/// first. Targets listed in `failed` (by path) were not removed.
pub fn adapter_breakdown(
    targets: &[CleanTarget],
    failed: &[(String, String)],
) -> Vec<(&'static str, usize, u64)> {
    let mut rows: Vec<(&'static str, usize, u64)> = Vec::new();
    for t in targets {
        let path = t.path.display().to_string();
        if failed.iter().any(|(p, _)| *p == path) {
            continue;
        }
        match rows.iter_mut().find(|(name, _, _)| *name == t.adapter) {
            Some(row) => {
                row.1 += 1;
                row.2 += t.size;
            }
            None => rows.push((t.adapter, 1, t.size)),
        }
    }
    rows.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(b.0)));
    rows
}

/// One aligned line per breakdown row.
fn breakdown_lines(rows: &[(&str, usize, u64)]) -> Vec<String> {
    let name_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
    let count_width = rows
        .iter()
        .map(|r| r.1.to_string().len())
        .max()
        .unwrap_or(0);
    rows.iter()
        .map(|(name, count, bytes)| {
            let noun = if *count == 1 { "item" } else { "items" };
            format!(
                "  {name:<name_width$}  {count:>count_width$} {noun:<5}  {}",
                ByteSize(*bytes).display().si()
            )
        })
        .collect()
}

/// Print how many targets each adapter removed and how much they held.
pub fn print_breakdown(targets: &[CleanTarget], failed: &[(String, String)]) {
    for line in breakdown_lines(&adapter_breakdown(targets, failed)) {
        println!("{line}");
    }
}

pub fn print_cancelled() {
    println!(
        "{}",
        "Cancelled — nothing deleted.".if_supports_color(Stream::Stdout, |t| t.yellow())
    );
}

/// Print the final report after TUI-based deletion completes (safe mode).
pub fn print_final_report(
    freed: u64,
    errors: &[(String, String)],
    targets: &[CleanTarget],
    dry_run: bool,
    method: DeleteMethod,
) {
    if dry_run {
        println!(
            "{}",
            "Dry run complete — nothing was deleted."
                .if_supports_color(Stream::Stdout, |t| t.yellow())
        );
        return;
    }

    println!(
        "\n{} {} {}",
        "✓".if_supports_color(Stream::Stdout, |t| t.green()),
        method.done_label(),
        ByteSize(freed)
            .display()
            .si()
            .if_supports_color(Stream::Stdout, |t| t.green()),
    );
    print_breakdown(targets, errors);

    if !errors.is_empty() {
        println!(
            "\n{} {} error(s) during deletion:",
            "⚠".if_supports_color(Stream::Stdout, |t| t.yellow()),
            errors
                .len()
                .if_supports_color(Stream::Stdout, |t| t.yellow()),
        );
        for (path, err) in errors {
            println!("  {}: {}", path, err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn target(path: &str, adapter: &'static str, size: u64) -> CleanTarget {
        CleanTarget {
            path: PathBuf::from(path),
            adapter,
            description: String::new(),
            size,
            tracked: false,
        }
    }

    #[test]
    fn breakdown_groups_removed_targets_by_adapter() {
        let targets = [
            target("/a/node_modules", "node", 100),
            target("/b/node_modules", "node", 50),
            target("/c/target", "cargo", 500),
            target("/d/target", "cargo", 7),
        ];
        let failed = [("/d/target".to_string(), "denied".to_string())];

        let rows = adapter_breakdown(&targets, &failed);

        assert_eq!(rows, vec![("cargo", 1, 500), ("node", 2, 150)]);
    }

    #[test]
    fn breakdown_rows_are_aligned() {
        let rows = [("cargo", 1, 500), ("python", 12, 1_500_000)];
        assert_eq!(
            breakdown_lines(&rows),
            vec!["  cargo    1 item   500 B", "  python  12 items  1.5 MB"]
        );
    }
}
