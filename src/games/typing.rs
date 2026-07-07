//! Typing Speed mini-test game.

use std::time::Instant;

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

/// Sentence data embedded from `assets/typing_sentences.txt` at compile time.
const SENTENCES_DATA: &str = include_str!("../../assets/typing_sentences.txt");

/// Returns an iterator over the non-empty sentences in the embedded data.
fn sentences() -> impl Iterator<Item = &'static str> {
    SENTENCES_DATA.lines().filter(|l| !l.is_empty())
}

/// All mutable state for one typing-speed session.
struct TypingState {
    target: Vec<char>,
    input: Vec<char>,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
}

impl TypingState {
    /// Pick a random sentence and initialise a fresh session.
    fn new() -> Self {
        let list: Vec<&str> = sentences().collect();
        let mut rng = new_rng();
        let sentence = list
            .choose(&mut rng)
            .copied()
            .unwrap_or("The quick brown fox jumps over the lazy dog in the park.");
        Self {
            target: sentence.chars().collect(),
            input: Vec::new(),
            started_at: None,
            finished_at: None,
        }
    }

    /// Append a typed character, starting the timer on the very first keypress.
    fn push(&mut self, c: char) {
        if self.is_done() {
            return;
        }
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        if self.input.len() < self.target.len() {
            self.input.push(c);
            if self.input.len() == self.target.len() {
                self.finished_at = Some(Instant::now());
            }
        }
    }

    /// Remove the last typed character (backspace).
    fn pop(&mut self) {
        if !self.is_done() {
            self.input.pop();
        }
    }

    fn is_done(&self) -> bool {
        self.finished_at.is_some()
    }

    /// Seconds elapsed from first keypress to completion (or to now while still typing).
    fn elapsed_secs(&self) -> f64 {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => (end - start).as_secs_f64(),
            (Some(start), None) => start.elapsed().as_secs_f64(),
            _ => 0.0,
        }
    }

    /// Words per minute using the standard 5-characters-per-word definition.
    fn wpm(&self) -> f64 {
        let secs = self.elapsed_secs();
        if secs <= 0.0 {
            return 0.0;
        }
        let words = self.target.len() as f64 / 5.0;
        words / (secs / 60.0)
    }

    /// Percentage of typed characters that match the target at the same position.
    fn accuracy(&self) -> f64 {
        if self.input.is_empty() {
            return 100.0;
        }
        let correct = self
            .input
            .iter()
            .zip(self.target.iter())
            .filter(|(a, b)| a == b)
            .count();
        correct as f64 / self.input.len() as f64 * 100.0
    }

    fn rating(&self) -> &'static str {
        let wpm = self.wpm();
        if wpm >= 80.0 {
            "Excellent"
        } else if wpm >= 60.0 {
            "Good"
        } else if wpm >= 40.0 {
            "Average"
        } else {
            "Keep practicing"
        }
    }
}

/// Builds a coloured line overlaying the typed input on the target sentence.
///
/// - Green bold: typed and correct.
/// - Red bold: typed and wrong.
/// - Yellow underlined: next character to type (cursor position).
/// - Dark gray: not yet reached.
fn typed_line(target: &[char], input: &[char]) -> Line<'static> {
    target
        .iter()
        .enumerate()
        .map(|(i, &tc)| match input.get(i) {
            Some(&ic) if ic == tc => Span::styled(
                tc.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Some(&ic) => Span::styled(
                ic.to_string(),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            None if i == input.len() => Span::styled(
                tc.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            None => Span::styled(tc.to_string(), Style::default().fg(Color::DarkGray)),
        })
        .collect::<Vec<_>>()
        .into()
}

/// Typing Speed game entry point.
pub struct Typing;

impl Game for Typing {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = TypingState::new();

        loop {
            let done = state.is_done();

            terminal.draw(|frame| {
                let area = frame.area();

                let outer = Block::default()
                    .title(" Typing Speed ")
                    .borders(Borders::ALL);
                let inner = outer.inner(area);
                frame.render_widget(outer, area);

                let v = Layout::vertical([
                    Constraint::Fill(1),   // [0] top padding
                    Constraint::Length(1), // [1] instruction
                    Constraint::Length(1), // [2] gap
                    Constraint::Length(1), // [3] target sentence
                    Constraint::Length(1), // [4] typed line
                    Constraint::Length(1), // [5] gap
                    Constraint::Length(1), // [6] progress bar
                    Constraint::Length(1), // [7] gap
                    Constraint::Length(2), // [8] result stats (2 lines)
                    Constraint::Fill(1),   // [9] bottom padding
                    Constraint::Length(1), // [10] help hint
                ])
                .split(inner);

                let centre = |a| {
                    Layout::horizontal([
                        Constraint::Fill(1),
                        Constraint::Max(72),
                        Constraint::Fill(1),
                    ])
                    .split(a)[1]
                };

                // Instruction line
                let (instr_text, instr_style) = if done {
                    (
                        "Test complete!",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if state.started_at.is_none() {
                    (
                        "Timer starts on your first keypress — type the sentence below",
                        Style::default().fg(Color::Cyan),
                    )
                } else {
                    ("Keep going!", Style::default().fg(Color::Cyan))
                };
                frame.render_widget(
                    Paragraph::new(instr_text)
                        .alignment(Alignment::Center)
                        .style(instr_style),
                    centre(v[1]),
                );

                // Target sentence (static white reference)
                let target_str: String = state.target.iter().collect();
                frame.render_widget(
                    Paragraph::new(target_str).style(Style::default().fg(Color::White)),
                    centre(v[3]),
                );

                // Typed line with per-character colour feedback
                frame.render_widget(
                    Paragraph::new(typed_line(&state.target, &state.input)),
                    centre(v[4]),
                );

                // Progress bar + live timer
                let bar_width = 30usize;
                let filled = if state.target.is_empty() {
                    bar_width
                } else {
                    bar_width * state.input.len() / state.target.len()
                };
                let bar = format!(
                    "[{}{}] {}/{}  {:.2}s",
                    "█".repeat(filled),
                    "░".repeat(bar_width - filled),
                    state.input.len(),
                    state.target.len(),
                    state.elapsed_secs(),
                );
                frame.render_widget(
                    Paragraph::new(bar)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray)),
                    centre(v[6]),
                );

                // Results panel — only rendered after the test ends
                if done {
                    let wpm = state.wpm();
                    let acc = state.accuracy();
                    let secs = state.elapsed_secs();
                    let rating = state.rating();
                    let rating_color = match rating {
                        "Excellent" => Color::Green,
                        "Good" => Color::Cyan,
                        "Average" => Color::Yellow,
                        _ => Color::Red,
                    };

                    let stats = Line::from(vec![
                        Span::styled("Time: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{secs:.2}s"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("    "),
                        Span::styled("WPM: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{wpm:.1}"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("    "),
                        Span::styled("Accuracy: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{acc:.1}%"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]);
                    let rating_line = Line::from(Span::styled(
                        rating,
                        Style::default()
                            .fg(rating_color)
                            .add_modifier(Modifier::BOLD),
                    ));

                    frame.render_widget(
                        Paragraph::new(vec![stats, rating_line]).alignment(Alignment::Center),
                        centre(v[8]),
                    );
                }

                // Help hint
                let help = if done {
                    "Q — return to menu"
                } else {
                    "Backspace — delete  •  Esc — menu"
                };
                frame.render_widget(
                    Paragraph::new(help)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray)),
                    v[10],
                );
            })?;

            if event::poll(std::time::Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Esc => return Ok(GameResult::BackToMenu),
                    KeyCode::Char('q') | KeyCode::Char('Q') if done => {
                        return Ok(GameResult::BackToMenu);
                    }
                    KeyCode::Backspace if !done => state.pop(),
                    KeyCode::Char(c) if !done => state.push(c),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn make_state(target: &str) -> TypingState {
        TypingState {
            target: target.chars().collect(),
            input: Vec::new(),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn sentence_count_meets_minimum() {
        let count = sentences().count();
        assert!(count >= 30, "expected ≥30 sentences, got {count}");
    }

    #[test]
    fn all_sentences_have_acceptable_word_count() {
        for s in sentences() {
            let words = s.split_whitespace().count();
            assert!(
                (10..=15).contains(&words),
                "sentence has {words} words (need 10–15): {s}"
            );
        }
    }

    #[test]
    fn all_sentences_fit_in_70_chars() {
        for s in sentences() {
            assert!(
                s.len() <= 70,
                "sentence is {} chars (limit 70): {s}",
                s.len()
            );
        }
    }

    #[test]
    fn new_state_starts_empty() {
        let s = TypingState::new();
        assert!(s.input.is_empty());
        assert!(s.started_at.is_none());
        assert!(!s.is_done());
    }

    #[test]
    fn first_keypress_starts_timer() {
        let mut s = make_state("hello");
        assert!(s.started_at.is_none());
        s.push('h');
        assert!(s.started_at.is_some());
    }

    #[test]
    fn game_ends_when_sentence_length_reached() {
        let mut s = make_state("hi");
        s.push('h');
        assert!(!s.is_done());
        s.push('i');
        assert!(s.is_done());
    }

    #[test]
    fn no_input_accepted_after_done() {
        let mut s = make_state("hi");
        s.push('h');
        s.push('i');
        let len = s.input.len();
        s.push('x');
        assert_eq!(s.input.len(), len);
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut s = make_state("hello");
        s.push('h');
        s.push('e');
        s.pop();
        assert_eq!(s.input, vec!['h']);
    }

    #[test]
    fn backspace_on_empty_input_is_safe() {
        let mut s = make_state("hello");
        s.pop();
        assert!(s.input.is_empty());
    }

    #[test]
    fn accuracy_all_correct() {
        let mut s = make_state("abc");
        for c in ['a', 'b', 'c'] {
            s.push(c);
        }
        assert!((s.accuracy() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn accuracy_all_wrong() {
        let mut s = make_state("abc");
        for c in ['x', 'y', 'z'] {
            s.push(c);
        }
        assert!(s.accuracy() < f64::EPSILON);
    }

    #[test]
    fn accuracy_half_correct() {
        let mut s = make_state("abcd");
        for c in ['a', 'b', 'x', 'y'] {
            s.push(c);
        }
        assert!((s.accuracy() - 50.0).abs() < 0.001);
    }

    #[test]
    fn wpm_is_zero_before_typing_starts() {
        let s = make_state("hello world");
        assert_eq!(s.wpm() as u32, 0);
    }

    #[test]
    fn rating_boundaries() {
        let now = Instant::now();
        // 100-char target → words = 100/5 = 20
        let target: Vec<char> = "a".repeat(100).chars().collect();

        // 15 s → WPM = 20 / 0.25 = 80 → Excellent (boundary)
        let s = TypingState {
            target: target.clone(),
            input: target.clone(),
            started_at: Some(now - Duration::from_secs(15)),
            finished_at: Some(now),
        };
        assert_eq!(s.rating(), "Excellent");

        // 20 s → WPM = 20 / (1/3) = 60 → Good (boundary)
        let s = TypingState {
            target: target.clone(),
            input: target.clone(),
            started_at: Some(now - Duration::from_secs(20)),
            finished_at: Some(now),
        };
        assert_eq!(s.rating(), "Good");

        // 30 s → WPM = 20 / 0.5 = 40 → Average (boundary)
        let s = TypingState {
            target: target.clone(),
            input: target.clone(),
            started_at: Some(now - Duration::from_secs(30)),
            finished_at: Some(now),
        };
        assert_eq!(s.rating(), "Average");

        // 60 s → WPM = 20 / 1.0 = 20 → Keep practicing
        let s = TypingState {
            target: target.clone(),
            input: target.clone(),
            started_at: Some(now - Duration::from_secs(60)),
            finished_at: Some(now),
        };
        assert_eq!(s.rating(), "Keep practicing");
    }
}
