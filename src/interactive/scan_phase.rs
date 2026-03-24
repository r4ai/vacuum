use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::adapter::CleanTarget;
use crate::scanner::{ScanConfig, scan_streaming};

// ─── Background message ───────────────────────────────────────────────────────

pub enum ScanMsg {
    Found(CleanTarget),
    Done,
}

// ─── State ────────────────────────────────────────────────────────────────────

pub struct ScanningState {
    pub rx: mpsc::Receiver<ScanMsg>,
    pub found: Vec<CleanTarget>,
    pub start: Instant,
    pub done: bool,
}

impl ScanningState {
    /// Spawn background scan thread and return the state.
    pub fn new(root: PathBuf, cfg: ScanConfig) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = scan_streaming(&root, &cfg, &mut |target| {
                let _ = tx.send(ScanMsg::Found(target));
            });
            let _ = tx.send(ScanMsg::Done);
        });
        Self {
            rx,
            found: Vec::new(),
            start: Instant::now(),
            done: false,
        }
    }

    /// Drain all pending messages from the background thread.
    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ScanMsg::Found(t) => self.found.push(t),
                ScanMsg::Done => self.done = true,
            }
        }
    }
}

// ─── Spinner animation ────────────────────────────────────────────────────────

static SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner_frame(elapsed_ms: u128) -> &'static str {
    let idx = (elapsed_ms / 80) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

// ─── Rendering ───────────────────────────────────────────────────────────────

pub fn render_scanning(frame: &mut ratatui::Frame, state: &ScanningState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // body
            Constraint::Length(1), // hint
        ])
        .split(area);

    render_scan_header(frame, chunks[0]);
    render_scan_body(frame, state, chunks[1]);
    render_scan_hint(frame, chunks[2]);
}

fn render_scan_header(frame: &mut ratatui::Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header = ratatui::widgets::Paragraph::new(Line::from(vec![
        Span::styled(
            "vacuum",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — Scanning..."),
    ]))
    .block(block);
    frame.render_widget(header, area);
}

fn render_scan_body(frame: &mut ratatui::Frame, state: &ScanningState, area: Rect) {
    let elapsed = state.start.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let elapsed_s = elapsed.as_secs_f64();

    let spinner = if state.done {
        "✓"
    } else {
        spinner_frame(elapsed_ms)
    };

    let status_style = if state.done {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    };

    let status_text = if state.done { "Done" } else { "Scanning" };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let body_inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::styled(format!("{spinner} {status_text}"), status_style),
            Span::styled(
                format!("  {:.1}s", elapsed_s),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Found: "),
            Span::styled(
                format!("{}", state.found.len()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" targets"),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), body_inner);
}

fn render_scan_hint(frame: &mut ratatui::Frame, area: Rect) {
    let hint = Line::from(vec![
        Span::styled("[q/Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]);
    frame.render_widget(Paragraph::new(hint), area);
}
