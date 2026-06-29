//! Estimation Challenge — guess real-world numerical quantities and earn points
//! based on how close your estimate is to the correct answer.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    common::rng::new_rng,
    games::{Game, GameResult},
    terminal::Term,
};

/// Questions asked per game session.
const QUESTIONS_PER_SESSION: usize = 5;

/// Maximum points awarded for a single question (answer within 10 %).
const MAX_POINTS: u32 = 100;

/// Raw question data embedded from `assets/estimation_questions.txt` at compile time.
const QUESTIONS_DATA: &str = include_str!("../../assets/estimation_questions.txt");

// ── Question model ────────────────────────────────────────────────────────────

/// A single estimation question parsed from the data file.
struct Question {
    /// Text displayed to the player.
    text: String,
    /// Numeric correct answer used for scoring.
    answer: f64,
    /// Human-readable answer string shown after the player confirms (e.g. `"8,953"`).
    display: String,
    /// Unit label appended after `display` on the result screen.
    unit: String,
}

/// Parse [`QUESTIONS_DATA`] into a `Vec<Question>`.
///
/// Each non-blank, non-comment line must have four pipe-separated fields:
/// `text|numeric_answer|display_string|unit`.
fn load_questions() -> Vec<Question> {
    QUESTIONS_DATA
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .filter_map(|line| {
            let mut parts = line.splitn(4, '|');
            let text = parts.next()?.trim().to_string();
            let answer: f64 = parts.next()?.trim().parse().ok()?;
            let display = parts.next()?.trim().to_string();
            let unit = parts.next()?.trim().to_string();
            Some(Question {
                text,
                answer,
                display,
                unit,
            })
        })
        .collect()
}

// ── Game phases ───────────────────────────────────────────────────────────────

/// Tracks which screen the game is currently showing.
enum Phase {
    /// Player is typing their estimate.
    Answering,
    /// Player confirmed; showing the correct answer and points earned before continuing.
    ShowingResult {
        /// The exact string the player submitted.
        submitted: String,
        /// Points awarded for this question.
        points: u32,
    },
    /// All questions answered; showing the final score summary.
    Summary,
}

// ── Session state ─────────────────────────────────────────────────────────────

/// All mutable state for one game session.
struct EstimationState {
    /// The 5 questions randomly selected for this session.
    questions: Vec<Question>,
    /// Zero-based index of the current question.
    current: usize,
    /// Text the player has typed so far.
    input: String,
    /// Points earned for each answered question (grows as the session progresses).
    scores: Vec<u32>,
    /// Current phase of the game.
    phase: Phase,
}

impl EstimationState {
    /// Initialise a fresh session: parse the question bank and pick 5 at random.
    fn new() -> Self {
        let mut rng = new_rng();
        let mut bank = load_questions();
        bank.shuffle(&mut rng);
        bank.truncate(QUESTIONS_PER_SESSION);
        Self {
            questions: bank,
            current: 0,
            input: String::new(),
            scores: Vec::new(),
            phase: Phase::Answering,
        }
    }

    /// Returns the question currently on screen.
    fn current_question(&self) -> &Question {
        &self.questions[self.current]
    }

    /// Parse the current input, record the score, and transition to `ShowingResult`.
    fn confirm(&mut self) {
        let q = self.current_question();
        let submitted = self.input.trim().to_string();
        let points = submitted
            .parse::<f64>()
            .map_or(0, |v| score_estimate(v, q.answer));
        self.scores.push(points);
        self.phase = Phase::ShowingResult { submitted, points };
    }

    /// Advance to the next question, or to the summary screen when all are done.
    fn advance(&mut self) {
        self.current += 1;
        self.input.clear();
        if self.current >= QUESTIONS_PER_SESSION {
            self.phase = Phase::Summary;
        } else {
            self.phase = Phase::Answering;
        }
    }

    /// Sum of all scores collected so far.
    fn total_score(&self) -> u32 {
        self.scores.iter().sum()
    }
}

// ── Scoring ───────────────────────────────────────────────────────────────────

/// Compute the point value for `answer` relative to `correct`.
///
/// | Relative error | Points |
/// |---|---|
/// | ≤ 10 % | 100 |
/// | ≤ 25 % | 75 |
/// | ≤ 50 % | 50 |
/// | ≤ 100 % | 25 |
/// | > 100 % | 0 |
fn score_estimate(answer: f64, correct: f64) -> u32 {
    if correct == 0.0 {
        return if answer == 0.0 { MAX_POINTS } else { 0 };
    }
    let ratio = (answer / correct - 1.0).abs();
    if ratio <= 0.10 {
        MAX_POINTS
    } else if ratio <= 0.25 {
        75
    } else if ratio <= 0.50 {
        50
    } else if ratio <= 1.00 {
        25
    } else {
        0
    }
}

/// Colour feedback for a point value.
fn points_color(points: u32) -> Color {
    match points {
        100 => Color::Green,
        75 => Color::Cyan,
        50 => Color::Yellow,
        25 => Color::LightRed,
        _ => Color::Red,
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Draw the active-question screen (answering phase or result phase).
fn render_question(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    state: &EstimationState,
) {
    let q = state.current_question();

    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1), // [1] question counter
        Constraint::Length(1), // [2] gap
        Constraint::Length(3), // [3] question text (wraps up to 3 lines)
        Constraint::Length(1), // [4] gap
        Constraint::Length(1), // [5] "Your answer: …"
        Constraint::Length(1), // [6] "Correct: …"  (blank while answering)
        Constraint::Length(1), // [7] "Points: …"   (blank while answering)
        Constraint::Length(1), // [8] gap
        Constraint::Length(1), // [9] help text
        Constraint::Fill(1),
    ])
    .split(area);

    // [1] Question counter
    frame.render_widget(
        Paragraph::new(format!(
            "Question {} / {}",
            state.current + 1,
            QUESTIONS_PER_SESSION
        ))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center),
        v[1],
    );

    // [3] Question text with automatic line-wrapping
    frame.render_widget(
        Paragraph::new(q.text.as_str())
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        v[3],
    );

    // [5–9] Phase-specific content
    match &state.phase {
        Phase::Answering => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("Your answer:  "),
                    Span::styled(
                        format!("{}|", state.input),
                        Style::default().fg(Color::Yellow),
                    ),
                ]))
                .alignment(Alignment::Center),
                v[5],
            );
            // v[6] and v[7] are intentionally left blank
            frame.render_widget(
                Paragraph::new("Type a number and press Enter  •  Esc to return to menu")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center),
                v[9],
            );
        }
        Phase::ShowingResult { submitted, points } => {
            let pts = *points;
            let col = points_color(pts);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("Your answer:  "),
                    Span::styled(submitted.as_str(), Style::default().fg(Color::White)),
                ]))
                .alignment(Alignment::Center),
                v[5],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("Correct:      "),
                    Span::styled(
                        format!("{} {}", q.display, q.unit),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Center),
                v[6],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("Points:       "),
                    Span::styled(
                        format!("{pts} / {MAX_POINTS}"),
                        Style::default().fg(col).add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Center),
                v[7],
            );
            frame.render_widget(
                Paragraph::new("Press Enter for the next question  •  Esc to return to menu")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center),
                v[9],
            );
        }
        Phase::Summary => {} // handled by render_summary
    }
}

/// Draw the end-of-game summary screen.
fn render_summary(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    state: &EstimationState,
) {
    let total = state.total_score();
    let max_total = (QUESTIONS_PER_SESSION as u32) * MAX_POINTS;

    let total_color = if total >= 400 {
        Color::Green
    } else if total >= 200 {
        Color::Yellow
    } else {
        Color::Red
    };

    // Fixed layout: header + 5 score lines + divider + total + help
    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1), // [1]  "Game Complete!"
        Constraint::Length(1), // [2]  gap
        Constraint::Length(1), // [3]  Q1
        Constraint::Length(1), // [4]  Q2
        Constraint::Length(1), // [5]  Q3
        Constraint::Length(1), // [6]  Q4
        Constraint::Length(1), // [7]  Q5
        Constraint::Length(1), // [8]  gap
        Constraint::Length(1), // [9]  total
        Constraint::Length(1), // [10] gap
        Constraint::Length(1), // [11] help
        Constraint::Fill(1),
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new("Game Complete!")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center),
        v[1],
    );

    for (i, (q, &pts)) in state.questions.iter().zip(state.scores.iter()).enumerate() {
        // Truncate to 40 characters (char-safe) and append ellipsis if needed
        let truncated: String = q.text.chars().take(40).collect();
        let label = if q.text.chars().count() > 40 {
            format!("{truncated}...")
        } else {
            truncated
        };
        let col = points_color(pts);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(format!("Q{}: {} — ", i + 1, label)),
                Span::styled(
                    format!("{pts} pts"),
                    Style::default().fg(col).add_modifier(Modifier::BOLD),
                ),
            ]))
            .alignment(Alignment::Center),
            v[3 + i],
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("Total: "),
            Span::styled(
                format!("{total} / {max_total}"),
                Style::default()
                    .fg(total_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center),
        v[9],
    );

    frame.render_widget(
        Paragraph::new("Press Q to return to the menu")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        v[11],
    );
}

// ── Game entry point ──────────────────────────────────────────────────────────

/// Estimation Challenge game entry point.
pub struct Estimation;

impl Game for Estimation {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = EstimationState::new();

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let block = Block::default()
                    .title(" Estimation Challenge ")
                    .borders(Borders::ALL);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                // Narrow central column so text stays readable on wide terminals.
                let center = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Max(72),
                    Constraint::Fill(1),
                ])
                .split(inner)[1];

                if matches!(state.phase, Phase::Summary) {
                    render_summary(frame, center, &state);
                } else {
                    render_question(frame, center, &state);
                }
            })?;

            // Derive boolean flags so event handling never holds a reference into `state`.
            let is_answering = matches!(state.phase, Phase::Answering);
            let is_result = matches!(state.phase, Phase::ShowingResult { .. });
            let is_summary = matches!(state.phase, Phase::Summary);

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char(c)
                        if is_answering && (c.is_ascii_digit() || c == '.') =>
                    {
                        state.input.push(c);
                    }
                    KeyCode::Backspace if is_answering => {
                        state.input.pop();
                    }
                    KeyCode::Enter if is_answering && !state.input.trim().is_empty() => {
                        state.confirm();
                    }
                    KeyCode::Enter if is_result => {
                        state.advance();
                    }
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

    #[test]
    fn data_file_has_enough_questions() {
        assert!(
            load_questions().len() >= 20,
            "expected ≥20 questions in the data file, found {}",
            load_questions().len()
        );
    }

    #[test]
    fn all_questions_parse_cleanly() {
        for q in load_questions() {
            assert!(!q.text.is_empty(), "question text must not be empty");
            assert!(q.answer.is_finite(), "answer must be a finite number");
            assert!(!q.display.is_empty(), "display string must not be empty");
        }
    }

    #[test]
    fn score_exact_match() {
        assert_eq!(score_estimate(100.0, 100.0), 100);
    }

    #[test]
    fn score_within_10_percent() {
        assert_eq!(score_estimate(109.0, 100.0), 100);
        assert_eq!(score_estimate(91.0, 100.0), 100);
        assert_eq!(score_estimate(109.9, 100.0), 100);
    }

    #[test]
    fn score_within_25_percent() {
        assert_eq!(score_estimate(120.0, 100.0), 75);
        assert_eq!(score_estimate(80.0, 100.0), 75);
    }

    #[test]
    fn score_within_50_percent() {
        assert_eq!(score_estimate(140.0, 100.0), 50);
        assert_eq!(score_estimate(60.0, 100.0), 50);
    }

    #[test]
    fn score_within_100_percent() {
        assert_eq!(score_estimate(180.0, 100.0), 25);
        assert_eq!(score_estimate(30.0, 100.0), 25);
    }

    #[test]
    fn score_beyond_100_percent() {
        assert_eq!(score_estimate(210.0, 100.0), 0);
    }

    #[test]
    fn score_zero_correct_edge_case() {
        assert_eq!(score_estimate(0.0, 0.0), 100);
        assert_eq!(score_estimate(5.0, 0.0), 0);
    }

    #[test]
    fn score_unparseable_input_gives_zero() {
        let points = "not_a_number"
            .parse::<f64>()
            .map_or(0, |v| score_estimate(v, 100.0));
        assert_eq!(points, 0);
    }

    #[test]
    fn session_picks_five_questions() {
        let state = EstimationState::new();
        assert_eq!(state.questions.len(), QUESTIONS_PER_SESSION);
    }

    #[test]
    fn total_score_sums_correctly() {
        let mut state = EstimationState::new();
        state.scores = vec![100, 75, 50, 25, 0];
        assert_eq!(state.total_score(), 250);
    }

    #[test]
    fn max_possible_total_score() {
        let mut state = EstimationState::new();
        state.scores = vec![MAX_POINTS; QUESTIONS_PER_SESSION];
        assert_eq!(
            state.total_score(),
            MAX_POINTS * QUESTIONS_PER_SESSION as u32
        );
    }

    #[test]
    fn advance_transitions_to_summary_after_last_question() {
        let mut state = EstimationState::new();
        state.current = QUESTIONS_PER_SESSION - 1;
        state.scores = vec![100; QUESTIONS_PER_SESSION - 1];
        state.scores.push(75);
        state.phase = Phase::ShowingResult {
            submitted: "42".into(),
            points: 75,
        };
        state.advance();
        assert!(matches!(state.phase, Phase::Summary));
    }
}
