use anyhow::Context as _;
use clap::{CommandFactory as _, Parser as _};
use std::process::ExitCode;

mod adapter;
mod adapters;
mod cleaner;
mod cli;
mod display;
mod interactive;
mod scanner;

use cli::{Cli, Mode};
use interactive::TuiResult;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Print the error chain without a backtrace, even when
            // `RUST_BACKTRACE` is set: these are user-facing errors.
            eprintln!("Error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
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

    match cli.mode {
        // ── Safe mode: full TUI workflow ──────────────────────────────────────
        Mode::Safe => match interactive::run_tui_flow(&root, &cli)? {
            TuiResult::Cancelled => display::print_cancelled(),
            TuiResult::Completed { freed, errors } => {
                display::print_final_report(freed, &errors, cli.dry_run);
            }
        },

        // ── Auto mode: non-interactive scan + delete ─────────────────────────
        Mode::Auto => {
            let adapters = scanner::build_adapters(&cli);
            if adapters.is_empty() {
                println!("All adapters are disabled. Nothing to do.");
                return Ok(());
            }

            eprintln!("Scanning {} ...", root.display());
            let targets = scanner::scan_enabled(&root, &cli)?;

            display::print_targets(&targets, &root);

            if targets.is_empty() {
                return Ok(());
            }

            let dry_run = cli.dry_run;
            cleaner::delete_targets(&targets, dry_run)?;
        }
    }

    Ok(())
}
