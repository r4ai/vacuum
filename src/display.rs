use bytesize::ByteSize;
use owo_colors::{OwoColorize, Stream};
use std::path::Path;

use crate::adapter::CleanTarget;

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
        format!("({})", ByteSize(total)).if_supports_color(Stream::Stdout, |t| t.bright_black()),
    );

    for t in targets {
        let rel = t.path.strip_prefix(root).unwrap_or(&t.path);
        println!(
            "  {}  {}  {}",
            format!("[{}]", t.adapter).if_supports_color(Stream::Stdout, |s| s.cyan()),
            rel.display(),
            format!("({})", ByteSize(t.size))
                .if_supports_color(Stream::Stdout, |s| s.bright_black()),
        );
    }

    println!();
}

pub fn print_cancelled() {
    println!(
        "{}",
        "Cancelled — nothing deleted.".if_supports_color(Stream::Stdout, |t| t.yellow())
    );
}

/// Print the final report after TUI-based deletion completes (safe mode).
pub fn print_final_report(freed: u64, errors: &[(String, String)], dry_run: bool) {
    if dry_run {
        println!(
            "{}",
            "Dry run complete — nothing was deleted."
                .if_supports_color(Stream::Stdout, |t| t.yellow())
        );
        return;
    }

    println!(
        "\n{} Freed {}",
        "✓".if_supports_color(Stream::Stdout, |t| t.green()),
        ByteSize(freed).if_supports_color(Stream::Stdout, |t| t.green()),
    );

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
