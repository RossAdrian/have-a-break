//! Game trait and shared game types.

use anyhow::Result;

use crate::terminal::Term;

pub mod estimation;
pub mod game_2048;
pub mod graph_coloring;
pub mod hangman;
pub mod hanoi;
pub mod pattern;
pub mod sliding;
pub mod snake;
pub mod typing;

/// Outcome returned when a game session ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    /// The player chose to quit the application entirely.
    Quit,
    /// The game ended; return to the main menu.
    BackToMenu,
}

/// Common interface implemented by every game.
pub trait Game {
    /// Run the game to completion and return the outcome.
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_result_variants_are_distinct() {
        assert_ne!(GameResult::Quit, GameResult::BackToMenu);
    }
}
