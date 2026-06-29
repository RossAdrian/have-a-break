//! Hangman game.

use std::collections::HashSet;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    common::rng::new_rng,
    games::{Game, GameResult},
    terminal::Term,
};

/// Maximum number of wrong guesses before the player loses.
const MAX_WRONG: u8 = 6;

/// Classic hangman ASCII art, one entry per wrong-guess count (0–6).
const STAGES: [&str; 7] = [
    "  +---+\n  |   |\n      |\n      |\n      |\n      |\n=========",
    "  +---+\n  |   |\n  O   |\n      |\n      |\n      |\n=========",
    "  +---+\n  |   |\n  O   |\n  |   |\n      |\n      |\n=========",
    "  +---+\n  |   |\n  O   |\n /|   |\n      |\n      |\n=========",
    "  +---+\n  |   |\n  O   |\n /|\\  |\n      |\n      |\n=========",
    "  +---+\n  |   |\n  O   |\n /|\\  |\n /    |\n      |\n=========",
    "  +---+\n  |   |\n  O   |\n /|\\  |\n / \\  |\n      |\n=========",
];

/// All game data for one session of Hangman.
struct HangmanState {
    word: Vec<char>,
    guessed: HashSet<char>,
    wrong: u8,
}

impl HangmanState {
    /// Pick a random word from the embedded word list and start a fresh session.
    fn new() -> Self {
        let word_list: Vec<&str> = crate::common::words::words().collect();
        let mut rng = new_rng();
        let word = word_list
            .choose(&mut rng)
            .unwrap_or(&"hangman")
            .to_ascii_uppercase()
            .chars()
            .collect();
        Self {
            word,
            guessed: HashSet::new(),
            wrong: 0,
        }
    }

    /// Record a guess. Returns `false` if the letter was already guessed (no-op).
    fn guess(&mut self, c: char) -> bool {
        if self.guessed.insert(c) {
            if !self.word.contains(&c) {
                self.wrong += 1;
            }
            true
        } else {
            false
        }
    }

    fn is_won(&self) -> bool {
        self.word.iter().all(|c| self.guessed.contains(c))
    }

    fn is_lost(&self) -> bool {
        self.wrong >= MAX_WRONG
    }

    /// Returns the word with unguessed letters replaced by `_`.
    /// When `reveal_all` is true every letter is shown (used on loss).
    fn display_word(&self, reveal_all: bool) -> String {
        let chars: String = self
            .word
            .iter()
            .flat_map(|c| {
                let shown = if reveal_all || self.guessed.contains(c) {
                    *c
                } else {
                    '_'
                };
                [shown, ' ']
            })
            .collect();
        chars.trim_end().to_string()
    }
}

/// Hangman game entry point.
pub struct Hangman;

impl Game for Hangman {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = HangmanState::new();

        loop {
            let won = state.is_won();
            let lost = state.is_lost();
            let game_over = won || lost;

            terminal.draw(|frame| {
                let area = frame.area();

                let block = Block::default().title(" Hangman ").borders(Borders::ALL);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                // Vertical bands: fill | category | gap | art | gap | word | gap |
                //                 alpha×2 | gap | counter | gap | status | fill
                let v = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1), // [1] category
                    Constraint::Length(1), // [2] gap
                    Constraint::Length(7), // [3] ASCII art
                    Constraint::Length(1), // [4] gap
                    Constraint::Length(1), // [5] word
                    Constraint::Length(1), // [6] gap
                    Constraint::Length(1), // [7] alphabet row 1
                    Constraint::Length(1), // [8] alphabet row 2
                    Constraint::Length(1), // [9] gap
                    Constraint::Length(1), // [10] wrong counter
                    Constraint::Length(1), // [11] gap
                    Constraint::Length(1), // [12] status
                    Constraint::Fill(1),
                ])
                .split(inner);

                // Narrow centre column so text doesn't stretch across wide terminals.
                let centre = |area| {
                    Layout::horizontal([
                        Constraint::Fill(1),
                        Constraint::Max(52),
                        Constraint::Fill(1),
                    ])
                    .split(area)[1]
                };

                // Category hint
                let category = Paragraph::new("Category: Common English Word")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Cyan));
                frame.render_widget(category, centre(v[1]));

                // ASCII art — red on loss
                let art_color = if lost { Color::Red } else { Color::White };
                let art = Paragraph::new(STAGES[state.wrong as usize])
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(art_color));
                frame.render_widget(art, centre(v[3]));

                // Blanked word — green on win, revealed on loss
                let word_str = state.display_word(lost);
                let word_color = if won { Color::Green } else { Color::White };
                let word_para = Paragraph::new(word_str)
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(word_color)
                            .add_modifier(Modifier::BOLD),
                    );
                frame.render_widget(word_para, centre(v[5]));

                // Alphabet: guessed-correct = green, guessed-wrong = dark, untouched = white
                let alphabet: Vec<char> = ('A'..='Z').collect();
                let make_alpha_row = |letters: &[char]| -> Line {
                    letters
                        .iter()
                        .map(|c| {
                            let guessed = state.guessed.contains(c);
                            let in_word = state.word.contains(c);
                            let style = match (guessed, in_word) {
                                (true, true) => Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                                (true, false) => Style::default().fg(Color::DarkGray),
                                _ => Style::default().fg(Color::White),
                            };
                            Span::styled(format!("{c} "), style)
                        })
                        .collect::<Vec<_>>()
                        .into()
                };

                frame.render_widget(
                    Paragraph::new(make_alpha_row(&alphabet[..13])).alignment(Alignment::Center),
                    centre(v[7]),
                );
                frame.render_widget(
                    Paragraph::new(make_alpha_row(&alphabet[13..])).alignment(Alignment::Center),
                    centre(v[8]),
                );

                // Wrong-guess counter with colour that shifts as lives decrease
                let remaining = MAX_WRONG - state.wrong;
                let counter_color = if remaining <= 1 {
                    Color::Red
                } else if remaining <= 3 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                let counter = Paragraph::new(format!(
                    "Wrong: {}/{}  •  Lives remaining: {}",
                    state.wrong, MAX_WRONG, remaining
                ))
                .alignment(Alignment::Center)
                .style(Style::default().fg(counter_color));
                frame.render_widget(counter, centre(v[10]));

                // Bottom status / result line
                let status: Paragraph = if won {
                    Paragraph::new("You won!  Press Esc to return to menu.")
                        .alignment(Alignment::Center)
                        .style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                } else if lost {
                    Paragraph::new(format!(
                        "Game over!  The word was: {}   Press Q to return.",
                        state.word.iter().collect::<String>()
                    ))
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Paragraph::new("Type a letter to guess  •  Esc to return to menu")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray))
                };
                frame.render_widget(status, centre(v[12]));
            })?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Esc => return Ok(GameResult::BackToMenu),
                    KeyCode::Char('q') | KeyCode::Char('Q') if game_over => return Ok(GameResult::BackToMenu),

                    KeyCode::Char(c) if !game_over && c.is_alphabetic() => {
                        state.guess(c.to_ascii_uppercase());
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_word(word: &str) -> HangmanState {
        HangmanState {
            word: word.chars().collect(),
            guessed: HashSet::new(),
            wrong: 0,
        }
    }

    #[test]
    fn new_game_starts_clean() {
        let s = HangmanState::new();
        assert_eq!(s.wrong, 0);
        assert!(s.guessed.is_empty());
        assert!(!s.word.is_empty());
    }

    #[test]
    fn correct_guess_does_not_count_as_wrong() {
        let mut s = state_with_word("HELLO");
        s.guess('H');
        assert_eq!(s.wrong, 0);
        assert!(s.guessed.contains(&'H'));
    }

    #[test]
    fn wrong_guess_increments_counter() {
        let mut s = state_with_word("HELLO");
        s.guess('Z');
        assert_eq!(s.wrong, 1);
    }

    #[test]
    fn duplicate_guess_is_ignored() {
        let mut s = state_with_word("HELLO");
        let first = s.guess('Z');
        let second = s.guess('Z');
        assert!(first);
        assert!(!second);
        assert_eq!(s.wrong, 1);
    }

    #[test]
    fn win_condition_all_letters_guessed() {
        let mut s = state_with_word("HI");
        s.guess('H');
        s.guess('I');
        assert!(s.is_won());
        assert!(!s.is_lost());
    }

    #[test]
    fn lose_condition_max_wrong_reached() {
        let mut s = state_with_word("Z");
        for c in ['A', 'B', 'C', 'D', 'E', 'F'] {
            s.guess(c);
        }
        assert!(s.is_lost());
        assert!(!s.is_won());
    }

    #[test]
    fn display_word_hides_unguessed_letters() {
        let mut s = state_with_word("HELLO");
        s.guess('H');
        assert_eq!(s.display_word(false), "H _ _ _ _");
    }

    #[test]
    fn display_word_reveals_all_on_loss() {
        let s = state_with_word("HELLO");
        assert_eq!(s.display_word(true), "H E L L O");
    }

    #[test]
    fn display_word_fully_revealed_on_win() {
        let mut s = state_with_word("CAT");
        for c in ['C', 'A', 'T'] {
            s.guess(c);
        }
        assert_eq!(s.display_word(false), "C A T");
        assert!(s.is_won());
    }

    #[test]
    fn word_list_has_enough_words() {
        let count = crate::common::words::words().count();
        assert!(count >= 200, "expected ≥200 words, got {count}");
    }

    #[test]
    fn all_stage_indices_are_valid() {
        for i in 0..=MAX_WRONG {
            assert!(!STAGES[i as usize].is_empty());
        }
    }
}
