use std::io;
use std::path::Path;

use anyhow::Context as _;
use bytesize::ByteSize;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{execute, ExecutableCommand as _};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Terminal;

use crate::adapter::CleanTarget;

struct App<'a> {
    targets: &'a [CleanTarget],
    root: &'a Path,
    selected: Vec<bool>,
    table_state: TableState,
}

impl<'a> App<'a> {
    fn new(targets: &'a [CleanTarget], root: &'a Path) -> Self {
        let mut table_state = TableState::default();
        if !targets.is_empty() {
            table_state.select(Some(0));
        }
        Self {
            targets,
            root,
            selected: vec![true; targets.len()],
            table_state,
        }
    }

    fn cursor(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    fn move_up(&mut self) {
        let cur = self.cursor();
        let next = if cur == 0 {
            self.targets.len().saturating_sub(1)
        } else {
            cur - 1
        };
        self.table_state.select(Some(next));
    }

    fn move_down(&mut self) {
        let cur = self.cursor();
        let next = if cur + 1 >= self.targets.len() {
            0
        } else {
            cur + 1
        };
        self.table_state.select(Some(next));
    }

    fn toggle(&mut self) {
        let cur = self.cursor();
        if let Some(v) = self.selected.get_mut(cur) {
            *v = !*v;
        }
    }

    fn select_all(&mut self) {
        self.selected.iter_mut().for_each(|v| *v = true);
    }

    fn select_none(&mut self) {
        self.selected.iter_mut().for_each(|v| *v = false);
    }

    fn total_size(&self) -> u64 {
        self.targets.iter().map(|t| t.size).sum()
    }

    fn selected_size(&self) -> u64 {
        self.targets
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, t)| t.size)
            .sum()
    }

    fn selected_count(&self) -> usize {
        self.selected.iter().filter(|&&v| v).count()
    }

    fn chosen_targets(&self) -> Vec<CleanTarget> {
        self.targets
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, t)| t.clone())
            .collect()
    }
}

fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // table
            Constraint::Length(2), // stats
            Constraint::Length(1), // keybind hint
        ])
        .split(area);

    // --- Header ---
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header_text = Paragraph::new(Line::from(vec![
        Span::styled("vacuum", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" — Select items to delete"),
    ]))
    .block(header_block);
    frame.render_widget(header_text, chunks[0]);

    // --- Table ---
    let header_cells = ["", "Adapter", "Path", "Size", "Description"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header_row = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app
        .targets
        .iter()
        .zip(&app.selected)
        .enumerate()
        .map(|(i, (t, &sel))| {
            let is_cursor = app.table_state.selected() == Some(i);
            let check = if sel { "✓" } else { "✗" };
            let check_style = if sel {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let rel = t.path.strip_prefix(app.root).unwrap_or(&t.path);
            let row_style = if is_cursor {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(check).style(check_style),
                Cell::from(t.adapter),
                Cell::from(rel.display().to_string()),
                Cell::from(ByteSize(t.size).to_string()).style(Style::default().fg(Color::Cyan)),
                Cell::from(t.description.as_str()),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(header_row)
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(Style::default());
    frame.render_stateful_widget(table, chunks[1], &mut app.table_state);

    // --- Stats ---
    let sel_count = app.selected_count();
    let total_count = app.targets.len();
    let sel_size = ByteSize(app.selected_size());
    let total_size = ByteSize(app.total_size());
    let stats = Paragraph::new(Line::from(vec![
        Span::raw(" Selected: "),
        Span::styled(
            format!("{sel_size}"),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" / {total_size}   ")),
        Span::styled(
            format!("{sel_count}"),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" / {total_count} items")),
    ]));
    frame.render_widget(stats, chunks[2]);

    // --- Keybind hint ---
    let hint = Paragraph::new(Line::from(vec![
        Span::styled(" [↑↓/jk]", Style::default().fg(Color::Yellow)),
        Span::raw(" Move  "),
        Span::styled("[Space]", Style::default().fg(Color::Yellow)),
        Span::raw(" Toggle  "),
        Span::styled("[a]", Style::default().fg(Color::Yellow)),
        Span::raw(" All  "),
        Span::styled("[n]", Style::default().fg(Color::Yellow)),
        Span::raw(" None  "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Confirm  "),
        Span::styled("[q/Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Quit"),
    ]));
    frame.render_widget(hint, chunks[3]);
}

/// Present an interactive multi-select table and return the targets
/// the user chose for deletion.
pub fn select_targets(targets: &[CleanTarget], root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
    if targets.is_empty() {
        return Ok(vec![]);
    }

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    let mut app = App::new(targets, root);
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode().context("Failed to disable raw mode")?;
    terminal
        .backend_mut()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<Vec<CleanTarget>> {
    loop {
        terminal.draw(|f| render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(vec![]),
                KeyCode::Enter => return Ok(app.chosen_targets()),
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Char(' ') => app.toggle(),
                KeyCode::Char('a') => app.select_all(),
                KeyCode::Char('n') => app.select_none(),
                _ => {}
            }
        }
    }
}
