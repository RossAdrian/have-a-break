//! Tower of Hanoi game stub.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Alignment,
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    games::{Game, GameResult},
    terminal::Term,
};

/// Tower of Hanoi game (coming soon).
pub struct Hanoi;

impl Game for Hanoi {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        loop {
            terminal.draw(|frame| {
                let para = Paragraph::new("Coming soon — press Q to return.")
                    .block(Block::default().title(" Tower of Hanoi ").borders(Borders::ALL))
                    .alignment(Alignment::Center);
                frame.render_widget(para, frame.area());
            })?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        return Ok(GameResult::BackToMenu)
                    }
                    _ => {}
                }
            }
        }
    }
}
