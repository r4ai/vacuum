use anyhow::Context as _;
use clap::{CommandFactory as _, Parser as _};

mod adapter;
mod adapters;
mod cleaner;
mod cli;
mod display;
mod interactive;
mod scanner;

use cli::{Cli, Mode};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Shell completion generation — exits immediately
    if let Some(shell) = cli.generate_completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "vacuum", &mut std::io::stdout());
        return Ok(());
    }

    // Resolve the scan root
    let root = cli
        .path
        .canonicalize()
        .with_context(|| format!("Cannot access path: {}", cli.path.display()))?;

    if !root.is_dir() {
        anyhow::bail!("{} is not a directory", root.display());
    }

    // Build enabled adapters
    let adapters = scanner::build_adapters(&cli);

    if adapters.is_empty() {
        println!("All adapters are disabled. Nothing to do.");
        return Ok(());
    }

    // Scan
    eprintln!("Scanning {} ...", root.display());
    eprintln!("Active adapters:");
    for adapter in &adapters {
        eprintln!(
            "  [{tag}] {name}  — {desc}",
            tag = if adapter.is_safe() {
                "safe"
            } else {
                "dangerous"
            },
            name = adapter.name(),
            desc = adapter.description(),
        );
    }
    eprintln!();
    let targets = scanner::scan_enabled(&root, &cli)?;

    // Display results
    display::print_targets(&targets, &root);

    if targets.is_empty() {
        return Ok(());
    }

    // Dry run short-circuit
    if cli.dry_run {
        cleaner::delete_targets(&targets, true)?;
        return Ok(());
    }

    // Mode-specific behavior
    match cli.mode {
        Mode::Safe => {
            let chosen = interactive::select_targets(&targets, &root)?;
            if chosen.is_empty() {
                display::print_cancelled();
            } else {
                cleaner::delete_targets(&chosen, false)?;
            }
        }
        Mode::Auto => {
            cleaner::delete_targets(&targets, false)?;
        }
    }

    Ok(())
}
