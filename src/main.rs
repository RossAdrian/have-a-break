//! Entry point: main menu loop.

mod common;
mod games;
mod terminal;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use games::{Game, GameResult};
use terminal::TerminalHandle;

/// Display name for each game, in menu order.
const GAME_NAMES: &[&str] = &[
    "Snake",
    "Graph Coloring",
    "Tower of Hanoi",
    "Pattern Guesser",
    "Sliding Puzzle",
    "2048",
    "Estimation Challenge",
    "Typing Speed",
    "Hangman",
];

/// Dispatches to the game at `idx`, constructing a fresh instance each time.
fn run_game(idx: usize, terminal: &mut terminal::Term) -> Result<GameResult> {
    match idx {
        0 => games::snake::Snake.run(terminal),
        1 => games::graph_coloring::GraphColoring.run(terminal),
        2 => games::hanoi::Hanoi.run(terminal),
        3 => games::pattern::Pattern.run(terminal),
        4 => games::sliding::Sliding.run(terminal),
        5 => games::game_2048::Game2048.run(terminal),
        6 => games::estimation::Estimation.run(terminal),
        7 => games::typing::Typing.run(terminal),
        8 => games::hangman::Hangman.run(terminal),
        _ => unreachable!(),
    }
}

fn main() -> Result<()> {
    let mut handle = TerminalHandle::new()?;
    let mut selected: usize = 0;

    loop {
        handle.terminal.draw(|frame| {
            let area = frame.area();

            // Vertical: top padding | list | help line | bottom padding
            let vchunks = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(GAME_NAMES.len() as u16 + 2), // +2 for borders
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(area);

            // Horizontal: side padding | centred column | side padding
            let hchunks = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Max(42),
                Constraint::Fill(1),
            ])
            .split(vchunks[1]);

            let items: Vec<ListItem> = GAME_NAMES
                .iter()
                .map(|name| ListItem::new(*name))
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(" Have a Break ")
                        .borders(Borders::ALL),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            let mut state = ListState::default();
            state.select(Some(selected));

            frame.render_stateful_widget(list, hchunks[1], &mut state);

            let help = Paragraph::new(Line::from("↑↓ navigate  •  Enter select  •  Q quit"))
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(help, vchunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if selected < GAME_NAMES.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    let result = run_game(selected, &mut handle.terminal)?;
                    if result == GameResult::Quit {
                        return Ok(());
                    }
                    // GameResult::BackToMenu — just loop back to the menu
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                _ => {}
            }
        }
    }
}
