use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::scanner::ScanConfig;

// ─── Adapter metadata ────────────────────────────────────────────────────────

struct AdapterEntry {
    name: &'static str,
    description: &'static str,
    is_safe: bool,
    get: fn(&ScanConfig) -> bool,
    set: fn(&mut ScanConfig, bool),
}

static ALL_ADAPTERS: &[AdapterEntry] = &[
    AdapterEntry {
        name: "node",
        description: "node_modules/",
        is_safe: true,
        get: |c| c.node,
        set: |c, v| c.node = v,
    },
    AdapterEntry {
        name: "cargo",
        description: "target/",
        is_safe: true,
        get: |c| c.cargo,
        set: |c, v| c.cargo = v,
    },
    AdapterEntry {
        name: "python",
        description: "__pycache__/, .venv/, venv/, dist/, build/, .eggs/, *.pyc",
        is_safe: true,
        get: |c| c.python,
        set: |c, v| c.python = v,
    },
    AdapterEntry {
        name: "go",
        description: "vendor/",
        is_safe: true,
        get: |c| c.go,
        set: |c, v| c.go = v,
    },
    AdapterEntry {
        name: "gradle",
        description: ".gradle/, build/",
        is_safe: true,
        get: |c| c.gradle,
        set: |c, v| c.gradle = v,
    },
    AdapterEntry {
        name: "maven",
        description: "target/",
        is_safe: true,
        get: |c| c.maven,
        set: |c, v| c.maven = v,
    },
    AdapterEntry {
        name: "ocaml",
        description: "_build/, _opam/",
        is_safe: true,
        get: |c| c.ocaml,
        set: |c, v| c.ocaml = v,
    },
    AdapterEntry {
        name: "gitignore",
        description: ".gitignore-matched paths",
        is_safe: false,
        get: |c| c.gitignore,
        set: |c, v| c.gitignore = v,
    },
];

// ─── State ────────────────────────────────────────────────────────────────────

pub struct AdapterSelectionState {
    pub cfg: ScanConfig,
    cursor: usize,
    list_state: ListState,
}

impl AdapterSelectionState {
    /// Build state whose selection starts from `cfg` (the CLI flags).
    pub fn new(cfg: ScanConfig) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            cfg,
            cursor: 0,
            list_state,
        }
    }

    pub fn any_selected(&self) -> bool {
        ALL_ADAPTERS.iter().any(|e| (e.get)(&self.cfg))
    }

    fn move_up(&mut self) {
        let len = ALL_ADAPTERS.len();
        self.cursor = if self.cursor == 0 {
            len - 1
        } else {
            self.cursor - 1
        };
        self.list_state.select(Some(self.cursor));
    }

    fn move_down(&mut self) {
        let len = ALL_ADAPTERS.len();
        self.cursor = if self.cursor + 1 >= len {
            0
        } else {
            self.cursor + 1
        };
        self.list_state.select(Some(self.cursor));
    }

    fn toggle(&mut self) {
        if let Some(entry) = ALL_ADAPTERS.get(self.cursor) {
            let cur = (entry.get)(&self.cfg);
            (entry.set)(&mut self.cfg, !cur);
        }
    }
}

// ─── Rendering ───────────────────────────────────────────────────────────────

pub fn render_adapter_selection(frame: &mut ratatui::Frame, state: &mut AdapterSelectionState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // list
            Constraint::Length(1), // hint
        ])
        .split(area);

    // Header
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "vacuum",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — Select adapters to scan"),
    ]))
    .block(block);
    frame.render_widget(header, chunks[0]);

    // Adapter list
    let items: Vec<ListItem> = ALL_ADAPTERS
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_cursor = i == state.cursor;
            let enabled = (entry.get)(&state.cfg);
            let check = if enabled { "✓" } else { "✗" };
            let check_style = if enabled {
                Style::default().fg(Color::Green)
            } else if is_cursor {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let safe_badge = if entry.is_safe {
                Span::styled(" [safe]", Style::default().fg(Color::Green))
            } else {
                Span::styled(" [custom]", Style::default().fg(Color::Yellow))
            };

            ListItem::new(Line::from(vec![
                Span::styled(check, check_style),
                Span::raw(" "),
                Span::styled(
                    format!("{:<10}", entry.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(entry.description, Style::default().fg(Color::Gray)),
                safe_badge,
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(list, chunks[1], &mut state.list_state);

    // Hint
    let k = |s| Span::styled(s, Style::default().fg(Color::Yellow));
    let t = |s: &'static str| Span::raw(s);
    let hint = Line::from(vec![
        k(" [↑↓/jk]"),
        t(" Move  "),
        k("[Space]"),
        t(" Toggle  "),
        Span::styled(
            "[Enter]",
            Style::default().fg(if state.any_selected() {
                Color::Green
            } else {
                Color::DarkGray
            }),
        ),
        t(" Scan  "),
        Span::styled("[q/Esc]", Style::default().fg(Color::Red)),
        t(" Quit"),
    ]);
    frame.render_widget(Paragraph::new(hint), chunks[2]);
}

// ─── Event handling ───────────────────────────────────────────────────────────

pub enum AdapterSelectionResult {
    Continue,
    Confirm,
    Quit,
}

pub fn handle_adapter_selection_key(
    state: &mut AdapterSelectionState,
    key: KeyEvent,
) -> AdapterSelectionResult {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => AdapterSelectionResult::Quit,
        KeyCode::Enter => {
            if state.any_selected() {
                AdapterSelectionResult::Confirm
            } else {
                AdapterSelectionResult::Continue
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            AdapterSelectionResult::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            AdapterSelectionResult::Continue
        }
        KeyCode::Char(' ') => {
            state.toggle();
            AdapterSelectionResult::Continue
        }
        _ => AdapterSelectionResult::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render the adapter selection screen and return its rows as text.
    fn rendered_rows(cfg: ScanConfig) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
        let mut state = AdapterSelectionState::new(cfg);
        terminal
            .draw(|frame| render_adapter_selection(frame, &mut state))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    fn row_of<'a>(rows: &'a [String], adapter: &str) -> &'a str {
        let label = format!(" {adapter:<10}");
        rows.iter()
            .find(|row| row.contains(&label))
            .unwrap_or_else(|| panic!("no row for {adapter}: {rows:#?}"))
    }

    fn scan_config(node: bool, gitignore: bool) -> ScanConfig {
        ScanConfig {
            no_size: false,
            node,
            cargo: true,
            python: true,
            go: true,
            gradle: true,
            maven: true,
            ocaml: true,
            gitignore,
        }
    }

    fn is_checked(row: &str) -> bool {
        row.contains('✓')
    }

    #[test]
    fn initial_state_reflects_cli_flags() {
        let rows = rendered_rows(scan_config(false, true));
        assert!(!is_checked(row_of(&rows, "node")), "node should be off");
        assert!(
            is_checked(row_of(&rows, "gitignore")),
            "gitignore should be on"
        );
        assert!(is_checked(row_of(&rows, "cargo")), "cargo should be on");
    }

    #[test]
    fn descriptions_match_actual_targets() {
        let rows = rendered_rows(scan_config(true, false));
        let expected = [
            ("node", "node_modules/"),
            ("cargo", "target/"),
            (
                "python",
                "__pycache__/, .venv/, venv/, dist/, build/, .eggs/, *.pyc",
            ),
            ("go", "vendor/"),
            ("gradle", ".gradle/, build/"),
            ("maven", "target/"),
            ("ocaml", "_build/, _opam/"),
            ("gitignore", ".gitignore-matched paths"),
        ];
        for (adapter, description) in expected {
            let row = row_of(&rows, adapter);
            assert!(
                row.contains(description),
                "{adapter}: expected {description:?} in {row:?}"
            );
        }
    }
}
