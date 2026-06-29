//! Pattern Guesser — identify the next element in a sequence.

use std::collections::HashSet;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::{seq::SliceRandom, Rng};
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

/// Number of rounds played per session.
const ROUNDS: usize = 6;

// ── Round data ────────────────────────────────────────────────────────────────

/// All data for one pattern round.
struct Round {
    /// The 4 displayed sequence elements.
    shown: Vec<String>,
    /// The correct 5th element.
    answer: String,
    /// 4 shuffled answer options (correct + 3 distractors).
    options: Vec<String>,
    /// 0-based index of the correct option within `options`.
    correct_idx: usize,
}

// ── Pattern generators ────────────────────────────────────────────────────────

/// Arithmetic progression: `start, start+step, start+2·step, …`
fn arithmetic_round(rng: &mut impl Rng) -> Round {
    let start: i64 = rng.gen_range(1..=20);
    let step: i64 = rng.gen_range(2..=9);
    let terms: Vec<i64> = (0..5).map(|i| start + i * step).collect();
    let shown = terms[..4].iter().map(|n| n.to_string()).collect();
    let answer = terms[4].to_string();
    let t = terms[4];
    assemble_round(
        shown,
        answer,
        vec![
            (t + step).to_string(),
            (t - step).to_string(),
            (t + step * 2).to_string(),
            (t - 1).to_string(),
            (t + 1).to_string(),
        ],
        rng,
    )
}

/// Geometric progression: `start, start·r, start·r², …`
fn geometric_round(rng: &mut impl Rng) -> Round {
    let start: i64 = rng.gen_range(1..=4);
    let ratio: i64 = rng.gen_range(2..=4);
    let terms: Vec<i64> = (0..5).map(|i| start * ratio.pow(i as u32)).collect();
    let shown = terms[..4].iter().map(|n| n.to_string()).collect();
    let answer = terms[4].to_string();
    let t = terms[4];
    assemble_round(
        shown,
        answer,
        vec![
            (t * ratio).to_string(),
            terms[3].to_string(),
            (t + terms[3]).to_string(),
            (t + 1).to_string(),
            (t - 1).to_string(),
        ],
        rng,
    )
}

/// Repeating symbol cycle, e.g. `A, B, C, A, ? → B`.
fn repeating_round(rng: &mut impl Rng) -> Round {
    let cycle_len: usize = rng.gen_range(3usize..=4);
    let start = rng.gen_range(0u8..=20) + b'A';
    let cycle: Vec<String> = (0..cycle_len)
        .map(|i| char::from(start + i as u8).to_string())
        .collect();
    let shown: Vec<String> = (0..4).map(|i| cycle[i % cycle_len].clone()).collect();
    let answer = cycle[4 % cycle_len].clone();
    let ch = answer.chars().next().unwrap() as u8;
    let mut distractors: Vec<String> = cycle.iter().filter(|s| **s != answer).cloned().collect();
    if ch > b'A' {
        distractors.push(char::from(ch - 1).to_string());
    }
    if ch < b'Z' {
        distractors.push(char::from(ch + 1).to_string());
    }
    assemble_round(shown, answer, distractors, rng)
}

/// ASCII growing shape: `., .., ..., …, ? → .....`
fn growing_round(rng: &mut impl Rng) -> Round {
    let ch: char = *['.', 'o', '#', '+'].choose(rng).unwrap();
    let offset: usize = rng.gen_range(1usize..=3);
    let shown: Vec<String> = (0..4).map(|i| ch.to_string().repeat(offset + i)).collect();
    let alen = offset + 4;
    let answer = ch.to_string().repeat(alen);
    assemble_round(
        shown,
        answer,
        vec![
            ch.to_string().repeat(alen + 1),
            ch.to_string().repeat(alen.saturating_sub(1)),
            ch.to_string().repeat(alen + 2),
            ch.to_string().repeat(alen.saturating_sub(2).max(1)),
        ],
        rng,
    )
}

/// Alternating two values: `a, b, a, b, ? → a`.
fn alternating_round(rng: &mut impl Rng) -> Round {
    let a: i64 = rng.gen_range(1..=9);
    let b: i64 = loop {
        let v = rng.gen_range(1i64..=20);
        if v != a {
            break v;
        }
    };
    let shown = vec![a.to_string(), b.to_string(), a.to_string(), b.to_string()];
    let answer = a.to_string();
    assemble_round(
        shown,
        answer,
        vec![
            b.to_string(),
            (a + 1).to_string(),
            (a - 1).to_string(),
            (b + 1).to_string(),
            (a + b).to_string(),
        ],
        rng,
    )
}

/// Fibonacci-style: each term is the sum of the two before it.
fn fibonacci_round(rng: &mut impl Rng) -> Round {
    let a: i64 = rng.gen_range(1..=5);
    let b: i64 = rng.gen_range(1..=8);
    let terms = [a, b, a + b, a + 2 * b, 2 * a + 3 * b];
    let shown = terms[..4].iter().map(|n| n.to_string()).collect();
    let answer = terms[4].to_string();
    let t = terms[4];
    assemble_round(
        shown,
        answer,
        vec![
            (t + 1).to_string(),
            (t - 1).to_string(),
            (t + terms[3]).to_string(),
            (t - terms[3]).to_string(),
            (t + terms[2]).to_string(),
        ],
        rng,
    )
}

/// Shuffle a correct answer together with 3 unique distractors chosen from `pool`.
fn assemble_round(
    shown: Vec<String>,
    answer: String,
    pool: Vec<String>,
    rng: &mut impl Rng,
) -> Round {
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(answer.clone());
    let mut distractors: Vec<String> = pool
        .into_iter()
        .filter(|d| seen.insert(d.clone()))
        .take(3)
        .collect();
    // Safety padding — only reached when the pool has too few unique entries.
    let mut n = 1i64;
    while distractors.len() < 3 {
        let s = format!("{n}?");
        if seen.insert(s.clone()) {
            distractors.push(s);
        }
        n += 1;
    }
    let mut options: Vec<String> = std::iter::once(answer.clone()).chain(distractors).collect();
    options.shuffle(rng);
    let correct_idx = options.iter().position(|o| *o == answer).unwrap();
    Round {
        shown,
        answer,
        options,
        correct_idx,
    }
}

/// Pick one of the 6 pattern types uniformly at random.
fn random_round(rng: &mut impl Rng) -> Round {
    match rng.gen_range(0u8..6) {
        0 => arithmetic_round(rng),
        1 => geometric_round(rng),
        2 => repeating_round(rng),
        3 => growing_round(rng),
        4 => alternating_round(rng),
        _ => fibonacci_round(rng),
    }
}

// ── Game phases ───────────────────────────────────────────────────────────────

enum Phase {
    Answering,
    ShowingResult { chosen_idx: usize, correct: bool },
    Summary,
}

// ── Session state ─────────────────────────────────────────────────────────────

struct PatternState {
    rounds: Vec<Round>,
    current: usize,
    score: u32,
    phase: Phase,
}

impl PatternState {
    fn new() -> Self {
        let mut rng = new_rng();
        let rounds = (0..ROUNDS).map(|_| random_round(&mut rng)).collect();
        Self {
            rounds,
            current: 0,
            score: 0,
            phase: Phase::Answering,
        }
    }

    fn current_round(&self) -> &Round {
        &self.rounds[self.current]
    }

    fn choose(&mut self, idx: usize) {
        let correct = idx == self.rounds[self.current].correct_idx;
        if correct {
            self.score += 1;
        }
        self.phase = Phase::ShowingResult {
            chosen_idx: idx,
            correct,
        };
    }

    fn advance(&mut self) {
        self.current += 1;
        if self.current >= ROUNDS {
            self.phase = Phase::Summary;
        } else {
            self.phase = Phase::Answering;
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render_round(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    state: &PatternState,
) {
    let round = state.current_round();

    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1), // [1]  round counter
        Constraint::Length(1), // [2]  gap
        Constraint::Length(1), // [3]  sequence
        Constraint::Length(1), // [4]  gap
        Constraint::Length(1), // [5]  option 1
        Constraint::Length(1), // [6]  option 2
        Constraint::Length(1), // [7]  option 3
        Constraint::Length(1), // [8]  option 4
        Constraint::Length(1), // [9]  gap
        Constraint::Length(1), // [10] feedback
        Constraint::Length(1), // [11] gap
        Constraint::Length(1), // [12] help
        Constraint::Fill(1),
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new(format!("Round {} / {}", state.current + 1, ROUNDS))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        v[1],
    );

    let seq_str = {
        let mut parts: Vec<String> = round.shown.clone();
        parts.push("?".to_string());
        parts.join(", ")
    };
    frame.render_widget(
        Paragraph::new(seq_str)
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        v[3],
    );

    for (i, opt) in round.options.iter().enumerate() {
        let style = match &state.phase {
            Phase::Answering => Style::default().fg(Color::Yellow),
            Phase::ShowingResult { chosen_idx, correct } => {
                if round.correct_idx == i {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if *chosen_idx == i && !correct {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            }
            Phase::Summary => Style::default().fg(Color::DarkGray),
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{})", i + 1), style),
                Span::raw("  "),
                Span::styled(opt.as_str(), style),
            ]))
            .alignment(Alignment::Center),
            v[5 + i],
        );
    }

    let feedback: Option<(String, Color)> = match &state.phase {
        Phase::Answering => None,
        Phase::ShowingResult { correct, .. } => {
            if *correct {
                Some(("Correct!".to_string(), Color::Green))
            } else {
                Some((
                    format!("Wrong — the answer was: {}", round.answer),
                    Color::Red,
                ))
            }
        }
        Phase::Summary => None,
    };
    if let Some((text, color)) = feedback {
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center),
            v[10],
        );
    }

    let help = match &state.phase {
        Phase::Answering => "Press 1–4 to choose  •  Esc to return to menu",
        Phase::ShowingResult { .. } => "Press Enter for the next round  •  Esc to return to menu",
        Phase::Summary => "",
    };
    frame.render_widget(
        Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        v[12],
    );
}

fn render_summary(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    state: &PatternState,
) {
    let score = state.score;
    let total = ROUNDS as u32;
    let color = if score >= 5 {
        Color::Green
    } else if score >= 3 {
        Color::Yellow
    } else {
        Color::Red
    };

    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1), // [1] title
        Constraint::Length(1), // [2] gap
        Constraint::Length(1), // [3] score
        Constraint::Length(1), // [4] gap
        Constraint::Length(1), // [5] help
        Constraint::Fill(1),
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new("Game Complete!")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        v[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("Score: "),
            Span::styled(
                format!("{score} / {total}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center),
        v[3],
    );

    frame.render_widget(
        Paragraph::new("Press Q to return to the menu")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        v[5],
    );
}

// ── Game entry point ──────────────────────────────────────────────────────────

/// Pattern Guesser game.
pub struct Pattern;

impl Game for Pattern {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = PatternState::new();

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let block = Block::default()
                    .title(" Pattern Guesser ")
                    .borders(Borders::ALL);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                let center = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Max(56),
                    Constraint::Fill(1),
                ])
                .split(inner)[1];

                if matches!(state.phase, Phase::Summary) {
                    render_summary(frame, center, &state);
                } else {
                    render_round(frame, center, &state);
                }
            })?;

            let is_answering = matches!(state.phase, Phase::Answering);
            let is_result = matches!(state.phase, Phase::ShowingResult { .. });
            let is_summary = matches!(state.phase, Phase::Summary);

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('1') if is_answering => state.choose(0),
                    KeyCode::Char('2') if is_answering => state.choose(1),
                    KeyCode::Char('3') if is_answering => state.choose(2),
                    KeyCode::Char('4') if is_answering => state.choose(3),
                    KeyCode::Enter if is_result => state.advance(),
                    KeyCode::Char('q') | KeyCode::Char('Q') if is_summary => {
                        return Ok(GameResult::BackToMenu);
                    }
                    KeyCode::Esc => return Ok(GameResult::BackToMenu),
                    _ => {}
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    fn check_round(r: &Round) {
        assert_eq!(r.shown.len(), 4, "shown must have 4 elements");
        assert_eq!(r.options.len(), 4, "options must have 4 choices");
        assert!(r.correct_idx < 4, "correct_idx must be in bounds");
        assert_eq!(
            r.options[r.correct_idx], r.answer,
            "options[correct_idx] must equal answer"
        );
        let unique: HashSet<&String> = r.options.iter().collect();
        assert_eq!(unique.len(), 4, "all 4 options must be distinct");
    }

    #[test]
    fn arithmetic_round_is_valid() {
        let mut rng = thread_rng();
        for _ in 0..30 {
            check_round(&arithmetic_round(&mut rng));
        }
    }

    #[test]
    fn geometric_round_is_valid() {
        let mut rng = thread_rng();
        for _ in 0..30 {
            check_round(&geometric_round(&mut rng));
        }
    }

    #[test]
    fn repeating_round_is_valid() {
        let mut rng = thread_rng();
        for _ in 0..30 {
            check_round(&repeating_round(&mut rng));
        }
    }

    #[test]
    fn growing_round_is_valid() {
        let mut rng = thread_rng();
        for _ in 0..30 {
            check_round(&growing_round(&mut rng));
        }
    }

    #[test]
    fn alternating_round_is_valid() {
        let mut rng = thread_rng();
        for _ in 0..30 {
            check_round(&alternating_round(&mut rng));
        }
    }

    #[test]
    fn fibonacci_round_is_valid() {
        let mut rng = thread_rng();
        for _ in 0..30 {
            check_round(&fibonacci_round(&mut rng));
        }
    }

    #[test]
    fn session_has_correct_round_count() {
        let state = PatternState::new();
        assert_eq!(state.rounds.len(), ROUNDS);
    }

    #[test]
    fn correct_choice_increments_score() {
        let mut state = PatternState::new();
        let idx = state.rounds[0].correct_idx;
        state.choose(idx);
        assert_eq!(state.score, 1);
    }

    #[test]
    fn wrong_choice_does_not_increment_score() {
        let mut state = PatternState::new();
        let correct = state.rounds[0].correct_idx;
        let wrong = (correct + 1) % 4;
        state.choose(wrong);
        assert_eq!(state.score, 0);
    }

    #[test]
    fn advance_moves_to_summary_after_last_round() {
        let mut state = PatternState::new();
        state.current = ROUNDS - 1;
        state.phase = Phase::ShowingResult {
            chosen_idx: 0,
            correct: true,
        };
        state.advance();
        assert!(matches!(state.phase, Phase::Summary));
    }

    #[test]
    fn arithmetic_sequence_is_correct() {
        let terms: Vec<i64> = (0..5).map(|i| 2 + i * 3).collect();
        assert_eq!(terms, vec![2, 5, 8, 11, 14]);
    }

    #[test]
    fn fibonacci_sequence_is_correct() {
        let (a, b) = (1i64, 2i64);
        let terms = [a, b, a + b, a + 2 * b, 2 * a + 3 * b];
        assert_eq!(terms, [1, 2, 3, 5, 8]);
    }
}
