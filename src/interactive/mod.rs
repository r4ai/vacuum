use std::io::{self, IsTerminal as _};
use std::path::Path;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::cli::Cli;
use crate::scanner::ScanConfig;

mod adapter_select;
mod delete_phase;
mod scan_phase;
mod target_select;
pub mod types;

use adapter_select::{
    AdapterSelectionResult, AdapterSelectionState, handle_adapter_selection_key,
    render_adapter_selection,
};
use delete_phase::{DeletingState, render_deleting};
use scan_phase::{ScanningState, render_scanning};
use target_select::{
    App, handle_confirm_key, handle_detail_key, handle_help_key, handle_normal_key,
    handle_search_key, handle_visual_key, render,
};
use types::{ActionResult, Mode};

// ─── Public result type ───────────────────────────────────────────────────────

pub enum TuiResult {
    Completed {
        freed: u64,
        errors: Vec<(String, String)>,
    },
    Cancelled,
}

// ─── Phase state machine ──────────────────────────────────────────────────────

enum Phase {
    AdapterSelection(AdapterSelectionState),
    Scanning(ScanningState),
    TargetSelection(App),
    Deleting(DeletingState),
}

// ─── Main event loop ──────────────────────────────────────────────────────────

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    root: &Path,
    cli: &Cli,
) -> anyhow::Result<TuiResult> {
    let mut phase = Phase::AdapterSelection(AdapterSelectionState::new(ScanConfig::from(cli)));

    loop {
        // Render current phase
        terminal.draw(|frame| match &mut phase {
            Phase::AdapterSelection(s) => render_adapter_selection(frame, s),
            Phase::Scanning(s) => render_scanning(frame, s),
            Phase::TargetSelection(app) => render(frame, app),
            Phase::Deleting(s) => render_deleting(frame, s),
        })?;

        // Drain background thread messages (non-blocking)
        match &mut phase {
            Phase::Scanning(s) => {
                s.drain();
                // Auto-transition when scan completes
                if s.done {
                    let targets = std::mem::take(&mut s.found);
                    if targets.is_empty() {
                        return Ok(TuiResult::Completed {
                            freed: 0,
                            errors: vec![],
                        });
                    }
                    phase = Phase::TargetSelection(App::new(targets, root.to_path_buf()));
                    continue;
                }
            }
            Phase::Deleting(s) => {
                let was_finished = s.finished;
                s.drain();
                // Auto-exit after Done state has been rendered at least once
                if was_finished && s.finished {
                    let freed = s.freed;
                    let errors = std::mem::take(&mut s.errors);
                    return Ok(TuiResult::Completed { freed, errors });
                }
            }
            _ => {}
        }

        // Poll events with a short timeout so spinners animate
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Mouse(mouse) => {
                if let Phase::TargetSelection(app) = &mut phase
                    && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                {
                    target_select::handle_mouse(app, mouse.column, mouse.row, mouse.kind);
                }
            }

            Event::Key(key) if key.kind == KeyEventKind::Press => {
                // Raw mode turns Ctrl+C into a key event instead of SIGINT.
                // Treat it as cancel in every phase. While deletion is in
                // progress it is ignored, like every other key.
                if is_interrupt(key) {
                    match &mut phase {
                        Phase::Deleting(state) if !state.finished => continue,
                        Phase::Deleting(state) => {
                            let freed = state.freed;
                            let errors = std::mem::take(&mut state.errors);
                            return Ok(TuiResult::Completed { freed, errors });
                        }
                        _ => return Ok(TuiResult::Cancelled),
                    }
                }

                match &mut phase {
                    // ── Adapter selection ─────────────────────────────────
                    Phase::AdapterSelection(state) => {
                        match handle_adapter_selection_key(state, key) {
                            AdapterSelectionResult::Quit => return Ok(TuiResult::Cancelled),
                            AdapterSelectionResult::Confirm => {
                                let cfg = state.cfg.clone();
                                phase =
                                    Phase::Scanning(ScanningState::new(root.to_path_buf(), cfg));
                            }
                            AdapterSelectionResult::Continue => {}
                        }
                    }

                    // ── Scanning ──────────────────────────────────────────
                    Phase::Scanning(_) => {
                        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                            return Ok(TuiResult::Cancelled);
                        }
                    }

                    // ── Target selection ──────────────────────────────────
                    Phase::TargetSelection(app) => {
                        let result = match app.mode {
                            Mode::Normal => handle_normal_key(app, key),
                            Mode::Visual => handle_visual_key(app, key),
                            Mode::Search => {
                                handle_search_key(app, key);
                                ActionResult::Continue
                            }
                            Mode::Help => handle_help_key(app, key),
                            Mode::Detail => handle_detail_key(app, key),
                            Mode::ConfirmDelete => handle_confirm_key(app, key),
                        };
                        match result {
                            ActionResult::Quit => return Ok(TuiResult::Cancelled),
                            ActionResult::Confirm => {
                                let chosen = app.chosen_targets();
                                if chosen.is_empty() {
                                    return Ok(TuiResult::Completed {
                                        freed: 0,
                                        errors: vec![],
                                    });
                                }
                                phase = Phase::Deleting(DeletingState::new(
                                    chosen,
                                    cli.dry_run,
                                    cli.delete_method(),
                                ));
                            }
                            ActionResult::Continue => {}
                        }
                    }

                    // ── Deleting ──────────────────────────────────────────
                    Phase::Deleting(state) => {
                        if state.finished
                            && matches!(
                                key.code,
                                KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc
                            )
                        {
                            let freed = state.freed;
                            let errors = std::mem::take(&mut state.errors);
                            return Ok(TuiResult::Completed { freed, errors });
                        }
                        // Ignore all keys while deletion is in progress
                    }
                }
            }

            _ => {}
        }
    }
}

/// Whether `key` is Ctrl+C.
fn is_interrupt(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

// ─── Terminal restoration ─────────────────────────────────────────────────────

/// Whether the terminal is in raw mode and the alternate screen.
static TUI_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Leave raw mode and the alternate screen, and show the cursor again.
/// Does nothing if the TUI is not active.
fn restore_terminal() {
    if !TUI_ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
}

/// Restore the terminal before the panic message is printed, so that a
/// panic on the main thread does not leave the terminal in raw mode.
fn install_panic_hook() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if std::thread::current().name() == Some("main") {
                restore_terminal();
            }
            original(info);
        }));
    });
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run the full safe-mode TUI workflow:
/// adapter selection → scanning → target selection → deletion.
pub fn run_tui_flow(root: &Path, cli: &Cli) -> anyhow::Result<TuiResult> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!(
            "safe mode requires an interactive terminal.\n\
             Use `--mode auto --dry-run` to list targets, \
             or `--mode auto` to delete them without prompts."
        );
    }

    install_panic_hook();

    enable_raw_mode().context("Failed to enable raw mode")?;
    TUI_ACTIVE.store(true, Ordering::SeqCst);
    let mut stdout = io::stdout();
    let entered = execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen");
    let result = entered.and_then(|()| {
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;
        run_loop(&mut terminal, root, cli)
    });

    // Always restore terminal, even on error
    restore_terminal();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_is_an_interrupt() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(is_interrupt(key));
    }

    #[test]
    fn plain_c_is_not_an_interrupt() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert!(!is_interrupt(key));
    }
}
