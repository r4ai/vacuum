use std::path::Path;
use bytesize::ByteSize;
use owo_colors::{OwoColorize, Stream};

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
        format!("({})", ByteSize(total))
            .if_supports_color(Stream::Stdout, |t| t.bright_black()),
    );

    for t in targets {
        let rel = t.path.strip_prefix(root).unwrap_or(&t.path);
        println!(
            "  {}  {}  {}",
            format!("[{}]", t.adapter)
                .if_supports_color(Stream::Stdout, |s| s.cyan()),
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
