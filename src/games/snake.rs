//! Snake game.

use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::{
    common::rng::new_rng,
    games::{Game, GameResult},
    terminal::Term,
};

/// Width and height of the play area in cells.
const GRID_SIZE: i16 = 20;
/// Starting tick interval in milliseconds.
const INIT_TICK_MS: u64 = 250;
/// Fastest allowed tick interval in milliseconds.
const MIN_TICK_MS: u64 = 80;
/// Rendered grid width: 2 terminal columns per cell.
const GRID_COLS: u16 = GRID_SIZE as u16 * 2;
/// Rendered grid height: 1 terminal row per cell.
const GRID_ROWS: u16 = GRID_SIZE as u16;
/// Grid width including the 1-column wall border on each side.
const GRID_BOX_COLS: u16 = GRID_COLS + 2;
/// Grid height including the 1-row wall border on top and bottom.
const GRID_BOX_ROWS: u16 = GRID_ROWS + 2;

/// Movement direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    /// `true` when `other` is exactly opposite to `self`.
    fn is_opposite(self, other: Dir) -> bool {
        matches!(
            (self, other),
            (Dir::Up, Dir::Down)
                | (Dir::Down, Dir::Up)
                | (Dir::Left, Dir::Right)
                | (Dir::Right, Dir::Left)
        )
    }

    /// Grid offset `(dx, dy)` for one step in this direction.
    fn delta(self) -> (i16, i16) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }
}

/// Visual phase of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Playing,
    GameOver,
}

/// All mutable state for one Snake session.
struct SnakeState {
    /// Body segments, `front` = head.
    body: VecDeque<(i16, i16)>,
    /// Direction the snake is currently travelling.
    dir: Dir,
    /// Buffered direction change applied on the next tick.
    next_dir: Dir,
    /// Current food position.
    food: (i16, i16),
    score: u32,
    /// Milliseconds per tick; decreases with score.
    tick_ms: u64,
    phase: Phase,
}

impl SnakeState {
    /// Start a fresh game: 3 segments in the centre of the grid, moving right.
    fn new() -> Self {
        let cx = GRID_SIZE / 2;
        let cy = GRID_SIZE / 2;
        let body = VecDeque::from([(cx + 1, cy), (cx, cy), (cx - 1, cy)]);
        let mut s = Self {
            body,
            dir: Dir::Right,
            next_dir: Dir::Right,
            food: (0, 0),
            score: 0,
            tick_ms: INIT_TICK_MS,
            phase: Phase::Playing,
        };
        s.spawn_food();
        s
    }

    /// Place food at a random empty cell (guaranteed to not overlap the snake).
    fn spawn_food(&mut self) {
        let body_set: HashSet<(i16, i16)> = self.body.iter().copied().collect();
        let mut rng = new_rng();
        loop {
            let x = rng.gen_range(0..GRID_SIZE);
            let y = rng.gen_range(0..GRID_SIZE);
            if !body_set.contains(&(x, y)) {
                self.food = (x, y);
                return;
            }
        }
    }

    /// Buffer a new direction; direct reversals (e.g. Left while moving Right) are ignored.
    fn queue_dir(&mut self, dir: Dir) {
        if !dir.is_opposite(self.dir) {
            self.next_dir = dir;
        }
    }

    /// Advance the snake by one tick: move, check collisions, handle food.
    fn advance(&mut self) {
        if self.phase != Phase::Playing {
            return;
        }

        self.dir = self.next_dir;
        let (dx, dy) = self.dir.delta();
        let (hx, hy) = *self.body.front().unwrap();
        let new_head = (hx + dx, hy + dy);

        // Wall collision.
        if new_head.0 < 0
            || new_head.0 >= GRID_SIZE
            || new_head.1 < 0
            || new_head.1 >= GRID_SIZE
        {
            self.phase = Phase::GameOver;
            return;
        }

        let eating = new_head == self.food;

        // When not eating the tail vacates its cell this tick, so skip it in the check.
        let check_len = if eating {
            self.body.len()
        } else {
            self.body.len().saturating_sub(1)
        };
        if self.body.iter().take(check_len).any(|&seg| seg == new_head) {
            self.phase = Phase::GameOver;
            return;
        }

        self.body.push_front(new_head);

        if eating {
            self.score += 10;
            // Speed up by 10 ms every 50 points, down to the minimum.
            self.tick_ms = INIT_TICK_MS
                .saturating_sub((self.score / 50) as u64 * 10)
                .max(MIN_TICK_MS);
            self.spawn_food();
        } else {
            self.body.pop_back();
        }
    }
}

/// Build one styled [`Line`] per grid row for rendering inside a [`Paragraph`].
fn build_grid_lines(state: &SnakeState) -> Vec<Line<'static>> {
    let head = *state.body.front().unwrap();
    let body_set: HashSet<(i16, i16)> = state.body.iter().copied().skip(1).collect();

    (0..GRID_SIZE as usize)
        .map(|y| {
            let spans: Vec<Span<'static>> = (0..GRID_SIZE as usize)
                .map(|x| {
                    let pos = (x as i16, y as i16);
                    if pos == head {
                        Span::styled(
                            "██",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else if body_set.contains(&pos) {
                        Span::styled("██", Style::default().fg(Color::Green))
                    } else if pos == state.food {
                        Span::styled(
                            "● ",
                            Style::default()
                                .fg(Color::LightRed)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::raw("  ")
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

/// Snake game entry point.
pub struct Snake;

impl Game for Snake {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = SnakeState::new();
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|frame| {
                let area = frame.area();

                // Vertical: padding | score | grid+wall | help | padding
                // Minimum height: 1 + 22 + 1 = 24 rows — fits a standard 24-line terminal.
                let v = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1),             // score
                    Constraint::Length(GRID_BOX_ROWS), // grid + wall border
                    Constraint::Length(1),             // help / status
                    Constraint::Fill(1),
                ])
                .split(area);

                let h = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Length(GRID_BOX_COLS),
                    Constraint::Fill(1),
                ])
                .split(v[2]);

                // Score
                frame.render_widget(
                    Paragraph::new(format!("Score: {}", state.score))
                        .alignment(Alignment::Center)
                        .style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    v[1],
                );

                // Grid inside a wall border; the block title carries the game name.
                let wall = Block::default()
                    .title(" Snake ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(Color::White));
                frame.render_widget(
                    Paragraph::new(build_grid_lines(&state)).block(wall),
                    h[1],
                );

                // Help / game-over line
                let (msg, style) = match state.phase {
                    Phase::Playing => (
                        "↑↓←→ steer  •  Esc — menu".to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Phase::GameOver => (
                        format!("Game Over!  Score: {}  •  Q — menu", state.score),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                };
                frame.render_widget(
                    Paragraph::new(msg).alignment(Alignment::Center).style(style),
                    v[3],
                );
            })?;

            match state.phase {
                Phase::GameOver => {
                    if event::poll(Duration::from_millis(100))?
                        && let Event::Key(key) = event::read()?
                        && key.kind == KeyEventKind::Press
                        && matches!(
                            key.code,
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
                        )
                    {
                        return Ok(GameResult::BackToMenu);
                    }
                }
                Phase::Playing => {
                    let tick_dur = Duration::from_millis(state.tick_ms);
                    let timeout = tick_dur.saturating_sub(last_tick.elapsed());

                    if event::poll(timeout)?
                        && let Event::Key(key) = event::read()?
                        && key.kind == KeyEventKind::Press
                    {
                        match key.code {
                            KeyCode::Up => state.queue_dir(Dir::Up),
                            KeyCode::Down => state.queue_dir(Dir::Down),
                            KeyCode::Left => state.queue_dir(Dir::Left),
                            KeyCode::Right => state.queue_dir(Dir::Right),
                            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                                return Ok(GameResult::BackToMenu);
                            }
                            _ => {}
                        }
                    }

                    if last_tick.elapsed() >= tick_dur {
                        last_tick = Instant::now();
                        state.advance();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Dir ─────────────────────────────────────────────────────────────────

    #[test]
    fn dir_opposites_detected() {
        assert!(Dir::Up.is_opposite(Dir::Down));
        assert!(Dir::Down.is_opposite(Dir::Up));
        assert!(Dir::Left.is_opposite(Dir::Right));
        assert!(Dir::Right.is_opposite(Dir::Left));
    }

    #[test]
    fn dir_non_opposites_not_detected() {
        assert!(!Dir::Up.is_opposite(Dir::Left));
        assert!(!Dir::Up.is_opposite(Dir::Right));
        assert!(!Dir::Up.is_opposite(Dir::Up));
        assert!(!Dir::Left.is_opposite(Dir::Down));
    }

    #[test]
    fn dir_delta_values() {
        assert_eq!(Dir::Up.delta(), (0, -1));
        assert_eq!(Dir::Down.delta(), (0, 1));
        assert_eq!(Dir::Left.delta(), (-1, 0));
        assert_eq!(Dir::Right.delta(), (1, 0));
    }

    // ── SnakeState::new ──────────────────────────────────────────────────────

    #[test]
    fn new_has_three_segments() {
        let s = SnakeState::new();
        assert_eq!(s.body.len(), 3);
    }

    #[test]
    fn new_starts_moving_right() {
        let s = SnakeState::new();
        assert_eq!(s.dir, Dir::Right);
    }

    #[test]
    fn new_food_not_on_body() {
        for _ in 0..20 {
            let s = SnakeState::new();
            assert!(!s.body.contains(&s.food));
        }
    }

    #[test]
    fn new_score_is_zero() {
        let s = SnakeState::new();
        assert_eq!(s.score, 0);
    }

    #[test]
    fn new_starts_in_playing_phase() {
        let s = SnakeState::new();
        assert_eq!(s.phase, Phase::Playing);
    }

    // ── queue_dir ────────────────────────────────────────────────────────────

    #[test]
    fn queue_dir_accepts_valid_turn() {
        let mut s = SnakeState::new(); // moving right
        s.queue_dir(Dir::Up);
        assert_eq!(s.next_dir, Dir::Up);
    }

    #[test]
    fn queue_dir_ignores_reverse() {
        let mut s = SnakeState::new(); // moving right
        s.queue_dir(Dir::Left);
        assert_eq!(s.next_dir, Dir::Right); // unchanged
    }

    // ── advance helpers ──────────────────────────────────────────────────────

    fn make_state(body: &[(i16, i16)], dir: Dir, food: (i16, i16)) -> SnakeState {
        SnakeState {
            body: VecDeque::from(body.to_vec()),
            dir,
            next_dir: dir,
            food,
            score: 0,
            tick_ms: INIT_TICK_MS,
            phase: Phase::Playing,
        }
    }

    // ── advance: basic movement ──────────────────────────────────────────────

    #[test]
    fn advance_moves_head_forward() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (0, 0));
        s.advance();
        assert_eq!(*s.body.front().unwrap(), (6, 5));
    }

    #[test]
    fn advance_length_unchanged_without_food() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (0, 0));
        s.advance();
        assert_eq!(s.body.len(), 3);
    }

    #[test]
    fn advance_tail_removed_when_not_eating() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (0, 0));
        s.advance();
        assert!(!s.body.contains(&(3, 5)));
    }

    // ── advance: eating ──────────────────────────────────────────────────────

    #[test]
    fn advance_grows_on_food() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (6, 5));
        s.advance();
        assert_eq!(s.body.len(), 4);
        assert_eq!(s.score, 10);
    }

    #[test]
    fn advance_spawns_new_food_after_eating() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (6, 5));
        s.advance();
        // The old food position is now the head; new food must differ.
        assert_ne!(s.food, (6, 5));
    }

    // ── advance: collision ───────────────────────────────────────────────────

    #[test]
    fn advance_right_wall_collision_ends_game() {
        let mut s = make_state(
            &[(GRID_SIZE - 1, 5), (GRID_SIZE - 2, 5), (GRID_SIZE - 3, 5)],
            Dir::Right,
            (0, 0),
        );
        s.advance();
        assert_eq!(s.phase, Phase::GameOver);
    }

    #[test]
    fn advance_left_wall_collision_ends_game() {
        let mut s = make_state(&[(0, 5), (1, 5), (2, 5)], Dir::Left, (19, 19));
        s.advance();
        assert_eq!(s.phase, Phase::GameOver);
    }

    #[test]
    fn advance_top_wall_collision_ends_game() {
        let mut s = make_state(&[(5, 0), (5, 1), (5, 2)], Dir::Up, (19, 19));
        s.advance();
        assert_eq!(s.phase, Phase::GameOver);
    }

    #[test]
    fn advance_bottom_wall_collision_ends_game() {
        let mut s = make_state(
            &[(5, GRID_SIZE - 1), (5, GRID_SIZE - 2), (5, GRID_SIZE - 3)],
            Dir::Down,
            (0, 0),
        );
        s.advance();
        assert_eq!(s.phase, Phase::GameOver);
    }

    #[test]
    fn advance_self_collision_ends_game() {
        // Snake curled so the next step hits a non-tail body segment.
        // body: head=(3,3) → (3,2) → (4,2) → (4,3) → tail=(4,4)
        // Moving right: new_head = (4,3), which is body[3] (not the tail).
        // check_len = 4 (body.len()-1), so body[0..4] is checked → collision.
        let mut s = make_state(
            &[(3, 3), (3, 2), (4, 2), (4, 3), (4, 4)],
            Dir::Right,
            (0, 0),
        );
        s.advance();
        assert_eq!(s.phase, Phase::GameOver);
    }

    #[test]
    fn advance_tail_vacates_before_check() {
        // Snake curled so the next step lands exactly on the current tail.
        // The tail vacates this tick → no collision.
        // body: head=(3,3) → (3,2) → (4,2) → tail=(4,3)
        // Moving right: new_head = (4,3) = tail position.
        // check_len = 3 (body.len()-1), body[0..3] = [(3,3),(3,2),(4,2)] → no collision.
        let mut s = make_state(&[(3, 3), (3, 2), (4, 2), (4, 3)], Dir::Right, (0, 0));
        s.advance();
        assert_eq!(s.phase, Phase::Playing);
        assert_eq!(*s.body.front().unwrap(), (4, 3));
    }

    #[test]
    fn advance_noop_after_game_over() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (0, 0));
        s.phase = Phase::GameOver;
        let head_before = *s.body.front().unwrap();
        s.advance();
        assert_eq!(*s.body.front().unwrap(), head_before);
    }

    // ── tick speed ───────────────────────────────────────────────────────────

    #[test]
    fn tick_ms_decreases_at_score_50() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (6, 5));
        s.score = 50;
        s.tick_ms = INIT_TICK_MS
            .saturating_sub((s.score / 50) as u64 * 10)
            .max(MIN_TICK_MS);
        assert_eq!(s.tick_ms, 240);
    }

    #[test]
    fn tick_ms_clamped_at_minimum() {
        let mut s = make_state(&[(5, 5), (4, 5), (3, 5)], Dir::Right, (6, 5));
        s.score = 1000;
        s.tick_ms = INIT_TICK_MS
            .saturating_sub((s.score / 50) as u64 * 10)
            .max(MIN_TICK_MS);
        assert_eq!(s.tick_ms, MIN_TICK_MS);
    }

    // ── rendering ────────────────────────────────────────────────────────────

    #[test]
    fn build_grid_lines_has_correct_row_count() {
        let s = SnakeState::new();
        assert_eq!(build_grid_lines(&s).len(), GRID_SIZE as usize);
    }

    #[test]
    fn build_grid_lines_each_row_has_grid_size_spans() {
        let s = SnakeState::new();
        for line in build_grid_lines(&s) {
            assert_eq!(line.spans.len(), GRID_SIZE as usize);
        }
    }
}
