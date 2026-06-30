//! 2048 game.

use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    common::rng::new_rng,
    games::{Game, GameResult},
    terminal::Term,
};

/// Highest score achieved since the process started; survives menu round-trips.
static SESSION_BEST: AtomicU32 = AtomicU32::new(0);

const SIZE: usize = 4;

/// Character width of the rendered board: `+------+------+------+------+` = 29.
const GRID_WIDTH: u16 = 29;

/// Row height of the rendered board: 5 separator rows + 4 × 3 data rows = 17.
const GRID_HEIGHT: u16 = 17;

/// Visual phase of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Playing,
    /// Reached 2048; overlay shown until the player chooses to continue or quit.
    Win,
    /// No legal moves remain.
    GameOver,
}

/// Direction of a 2048 slide move.
#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// All mutable state for one 2048 session.
struct State2048 {
    board: [[u32; SIZE]; SIZE],
    score: u32,
    phase: Phase,
    /// Set once the player first reaches 2048 so the win overlay is shown only once.
    has_won: bool,
}

impl State2048 {
    /// Construct a fresh board and spawn two starting tiles.
    fn new() -> Self {
        let mut s = Self {
            board: [[0; SIZE]; SIZE],
            score: 0,
            phase: Phase::Playing,
            has_won: false,
        };
        s.spawn_tile();
        s.spawn_tile();
        s
    }

    /// Number of empty (zero) cells on the board.
    fn empty_count(&self) -> usize {
        self.board.iter().flatten().filter(|&&v| v == 0).count()
    }

    /// Place a 2 (90 % chance) or 4 (10 % chance) in a random empty cell.
    /// Does nothing when the board is full.
    fn spawn_tile(&mut self) {
        let empty = self.empty_count();
        if empty == 0 {
            return;
        }
        let mut rng = new_rng();
        let target = rng.gen_range(0..empty);
        let value: u32 = if rng.gen_bool(0.9) { 2 } else { 4 };
        let mut count = 0;
        'outer: for r in 0..SIZE {
            for c in 0..SIZE {
                if self.board[r][c] == 0 {
                    if count == target {
                        self.board[r][c] = value;
                        break 'outer;
                    }
                    count += 1;
                }
            }
        }
    }

    /// Slide and merge a single row toward index 0 (leftward).
    ///
    /// Each pair of equal adjacent tiles merges once per call.
    /// Returns the resulting row and the merge score earned.
    fn slide_row_left(row: &[u32; SIZE]) -> ([u32; SIZE], u32) {
        let mut vals: Vec<u32> = row.iter().copied().filter(|&v| v != 0).collect();
        let mut points = 0u32;
        let mut i = 0;
        while i + 1 < vals.len() {
            if vals[i] == vals[i + 1] {
                vals[i] *= 2;
                points += vals[i];
                vals.remove(i + 1);
            }
            i += 1;
        }
        let mut result = [0u32; SIZE];
        for (j, &v) in vals.iter().enumerate() {
            result[j] = v;
        }
        (result, points)
    }

    /// Apply a slide move in `dir`.
    ///
    /// Spawns a new tile and updates [`Phase`] when the board changes.
    /// Returns `true` if at least one tile moved or merged.
    fn shift(&mut self, dir: Direction) -> bool {
        let old = self.board;
        let mut gained = 0u32;

        match dir {
            Direction::Left => {
                for r in 0..SIZE {
                    let (row, pts) = Self::slide_row_left(&self.board[r]);
                    self.board[r] = row;
                    gained += pts;
                }
            }
            Direction::Right => {
                for r in 0..SIZE {
                    let mut rev = self.board[r];
                    rev.reverse();
                    let (mut row, pts) = Self::slide_row_left(&rev);
                    row.reverse();
                    self.board[r] = row;
                    gained += pts;
                }
            }
            Direction::Up => {
                for c in 0..SIZE {
                    let col: [u32; SIZE] = std::array::from_fn(|r| self.board[r][c]);
                    let (new_col, pts) = Self::slide_row_left(&col);
                    for (r, &v) in new_col.iter().enumerate() {
                        self.board[r][c] = v;
                    }
                    gained += pts;
                }
            }
            Direction::Down => {
                for c in 0..SIZE {
                    // Reverse column so slide_row_left merges toward the bottom.
                    let col: [u32; SIZE] = std::array::from_fn(|r| self.board[SIZE - 1 - r][c]);
                    let (mut new_col, pts) = Self::slide_row_left(&col);
                    new_col.reverse();
                    for (r, &v) in new_col.iter().enumerate() {
                        self.board[r][c] = v;
                    }
                    gained += pts;
                }
            }
        }

        self.score += gained;
        SESSION_BEST.fetch_max(self.score, Ordering::Relaxed);

        if self.board == old {
            // Board unchanged — still update phase if stuck on a full board.
            if self.phase == Phase::Playing && !self.has_moves() {
                self.phase = Phase::GameOver;
            }
            return false;
        }

        self.spawn_tile();

        if self.phase == Phase::Playing {
            if !self.has_won && self.board.iter().flatten().any(|&v| v == 2048) {
                self.has_won = true;
                self.phase = Phase::Win;
            } else if !self.has_moves() {
                self.phase = Phase::GameOver;
            }
        }

        true
    }

    /// `true` when at least one legal move remains.
    fn has_moves(&self) -> bool {
        if self.empty_count() > 0 {
            return true;
        }
        for r in 0..SIZE {
            for c in 0..SIZE {
                let v = self.board[r][c];
                if c + 1 < SIZE && self.board[r][c + 1] == v {
                    return true;
                }
                if r + 1 < SIZE && self.board[r + 1][c] == v {
                    return true;
                }
            }
        }
        false
    }
}

/// Background colour for a tile.
fn tile_bg(value: u32) -> Color {
    match value {
        0 => Color::Rgb(204, 192, 179),
        2 => Color::Rgb(238, 228, 218),
        4 => Color::Rgb(237, 224, 200),
        8 => Color::Rgb(242, 177, 121),
        16 => Color::Rgb(245, 149, 99),
        32 => Color::Rgb(246, 124, 95),
        64 => Color::Rgb(246, 94, 59),
        128 => Color::Rgb(237, 207, 114),
        256 => Color::Rgb(237, 204, 97),
        512 => Color::Rgb(237, 200, 80),
        1024 => Color::Rgb(237, 197, 63),
        2048 => Color::Rgb(237, 194, 46),
        _ => Color::Rgb(60, 58, 50),
    }
}

/// Foreground colour for a tile's number.
fn tile_fg(value: u32) -> Color {
    match value {
        0 | 2 | 4 => Color::Rgb(119, 110, 101),
        _ => Color::Rgb(249, 246, 242),
    }
}

/// Build the styled [`Line`]s that render the 4×4 board.
fn build_grid_lines(board: &[[u32; SIZE]; SIZE]) -> Vec<Line<'static>> {
    let sep_style = Style::default().fg(Color::Rgb(187, 173, 160));
    let border = "+------+------+------+------+";
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(GRID_HEIGHT as usize);

    for row_tiles in board.iter() {
        lines.push(Line::from(Span::styled(border, sep_style)));

        let mut top: Vec<Span<'static>> = vec![Span::styled("|", sep_style)];
        let mut mid: Vec<Span<'static>> = vec![Span::styled("|", sep_style)];
        let mut bot: Vec<Span<'static>> = vec![Span::styled("|", sep_style)];

        for &v in row_tiles.iter() {
            let bg = tile_bg(v);
            let fg = tile_fg(v);
            let label = if v == 0 {
                "      ".to_string()
            } else {
                format!("{:^6}", v)
            };
            let cell_bg = Style::default().bg(bg);
            let cell_val = Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD);

            top.push(Span::styled("      ", cell_bg));
            top.push(Span::styled("|", sep_style));
            mid.push(Span::styled(label, cell_val));
            mid.push(Span::styled("|", sep_style));
            bot.push(Span::styled("      ", cell_bg));
            bot.push(Span::styled("|", sep_style));
        }

        lines.push(Line::from(top));
        lines.push(Line::from(mid));
        lines.push(Line::from(bot));
    }

    lines.push(Line::from(Span::styled(border, sep_style)));
    lines
}

/// Centre a rectangle of `width × height` inside `area`, clamped to fit.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    }
}

/// 2048 game entry point.
pub struct Game2048;

impl Game for Game2048 {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = State2048::new();

        loop {
            let best = SESSION_BEST.load(Ordering::Relaxed);

            terminal.draw(|frame| {
                let area = frame.area();
                let outer = Block::default().title(" 2048 ").borders(Borders::ALL);
                let inner = outer.inner(area);
                frame.render_widget(outer, area);

                let v = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1),           // score row
                    Constraint::Length(1),           // gap
                    Constraint::Length(GRID_HEIGHT), // board
                    Constraint::Length(1),           // gap
                    Constraint::Length(1),           // help / status
                    Constraint::Fill(1),
                ])
                .split(inner);

                let h = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Length(GRID_WIDTH),
                    Constraint::Fill(1),
                ])
                .split(v[3]);
                let grid_rect = h[1];

                // Score row
                frame.render_widget(
                    Paragraph::new(format!("Score: {:>6}   Best: {:>6}", state.score, best))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    v[1],
                );

                // Board
                frame.render_widget(
                    Paragraph::new(build_grid_lines(&state.board)),
                    grid_rect,
                );

                // Help / status line
                let (help, help_style) = match state.phase {
                    Phase::Playing => (
                        String::from("↑↓←→ slide  •  Esc — menu"),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Phase::Win => (String::new(), Style::default()),
                    Phase::GameOver => (
                        format!("Game Over!  Score: {}  •  Q — menu", state.score),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                };
                frame.render_widget(
                    Paragraph::new(help).alignment(Alignment::Center).style(help_style),
                    v[5],
                );

                // Win overlay
                if state.phase == Phase::Win {
                    let overlay = centered_rect(32, 9, inner);
                    frame.render_widget(Clear, overlay);
                    let win_lines: Vec<Line<'static>> = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  You Win!  ",
                            Style::default()
                                .fg(Color::Rgb(237, 194, 46))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("  Score: {}  ", state.score),
                            Style::default().fg(Color::White),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  C — continue   Q — menu  ",
                            Style::default().fg(Color::Cyan),
                        )),
                        Line::from(""),
                    ];
                    frame.render_widget(
                        Paragraph::new(win_lines)
                            .alignment(Alignment::Center)
                            .block(
                                Block::default()
                                    .title(" You Win! ")
                                    .borders(Borders::ALL)
                                    .style(
                                        Style::default()
                                            .fg(Color::Rgb(237, 194, 46))
                                            .add_modifier(Modifier::BOLD),
                                    ),
                            ),
                        overlay,
                    );
                }
            })?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match state.phase {
                    Phase::Playing => match key.code {
                        KeyCode::Esc => return Ok(GameResult::BackToMenu),
                        KeyCode::Up => {
                            state.shift(Direction::Up);
                        }
                        KeyCode::Down => {
                            state.shift(Direction::Down);
                        }
                        KeyCode::Left => {
                            state.shift(Direction::Left);
                        }
                        KeyCode::Right => {
                            state.shift(Direction::Right);
                        }
                        _ => {}
                    },
                    Phase::Win => match key.code {
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            state.phase = Phase::Playing;
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(GameResult::BackToMenu);
                        }
                        _ => {}
                    },
                    Phase::GameOver => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(GameResult::BackToMenu);
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── slide_row_left ──────────────────────────────────────────────────────

    #[test]
    fn slide_empty_row() {
        let (row, pts) = State2048::slide_row_left(&[0, 0, 0, 0]);
        assert_eq!(row, [0, 0, 0, 0]);
        assert_eq!(pts, 0);
    }

    #[test]
    fn slide_single_value_compacts() {
        let (row, pts) = State2048::slide_row_left(&[0, 0, 4, 0]);
        assert_eq!(row, [4, 0, 0, 0]);
        assert_eq!(pts, 0);
    }

    #[test]
    fn slide_merges_equal_pair() {
        let (row, pts) = State2048::slide_row_left(&[2, 2, 0, 0]);
        assert_eq!(row, [4, 0, 0, 0]);
        assert_eq!(pts, 4);
    }

    #[test]
    fn slide_four_equal_gives_two_merges() {
        // [2, 2, 2, 2] → two independent merges → [4, 4, 0, 0], not [8, 0, 0, 0].
        let (row, pts) = State2048::slide_row_left(&[2, 2, 2, 2]);
        assert_eq!(row, [4, 4, 0, 0]);
        assert_eq!(pts, 8);
    }

    #[test]
    fn slide_merged_tile_does_not_chain() {
        // [2, 2, 4, 0] → 2+2=4, then that 4 must NOT merge with the existing 4.
        let (row, pts) = State2048::slide_row_left(&[2, 2, 4, 0]);
        assert_eq!(row, [4, 4, 0, 0]);
        assert_eq!(pts, 4);
    }

    #[test]
    fn slide_full_row_no_equals() {
        let (row, pts) = State2048::slide_row_left(&[2, 4, 8, 16]);
        assert_eq!(row, [2, 4, 8, 16]);
        assert_eq!(pts, 0);
    }

    #[test]
    fn slide_compacts_gaps() {
        let (row, pts) = State2048::slide_row_left(&[0, 4, 0, 8]);
        assert_eq!(row, [4, 8, 0, 0]);
        assert_eq!(pts, 0);
    }

    // ── shift directions ────────────────────────────────────────────────────

    fn make_state(rows: [[u32; 4]; 4]) -> State2048 {
        State2048 {
            board: rows,
            score: 0,
            phase: Phase::Playing,
            has_won: false,
        }
    }

    #[test]
    fn shift_left_merges_to_left() {
        let mut s = make_state([[0, 2, 0, 2], [0; 4], [0; 4], [0; 4]]);
        s.shift(Direction::Left);
        assert_eq!(s.board[0][0], 4);
        assert_eq!(s.board[0][1], 0);
        assert!(s.score >= 4);
    }

    #[test]
    fn shift_right_merges_to_right() {
        let mut s = make_state([[2, 0, 2, 0], [0; 4], [0; 4], [0; 4]]);
        s.shift(Direction::Right);
        assert_eq!(s.board[0][3], 4);
        assert_eq!(s.board[0][2], 0);
        assert!(s.score >= 4);
    }

    #[test]
    fn shift_up_merges_to_top() {
        let mut s = make_state([[2, 0, 0, 0], [2, 0, 0, 0], [0; 4], [0; 4]]);
        s.shift(Direction::Up);
        assert_eq!(s.board[0][0], 4);
        assert_eq!(s.board[1][0], 0);
        assert!(s.score >= 4);
    }

    #[test]
    fn shift_down_merges_to_bottom() {
        let mut s = make_state([[2, 0, 0, 0], [2, 0, 0, 0], [0; 4], [0; 4]]);
        s.shift(Direction::Down);
        assert_eq!(s.board[3][0], 4);
        assert_eq!(s.board[2][0], 0);
        assert!(s.score >= 4);
    }

    #[test]
    fn shift_unchanged_returns_false() {
        // All tiles already at the left edge, nothing can move left.
        let mut s = make_state([[2, 4, 8, 16], [0; 4], [0; 4], [0; 4]]);
        assert!(!s.shift(Direction::Left));
    }

    #[test]
    fn shift_spawns_new_tile_on_change() {
        let mut s = make_state([[0, 0, 0, 2], [0; 4], [0; 4], [0; 4]]);
        let before = s.board.iter().flatten().filter(|&&v| v != 0).count();
        s.shift(Direction::Left);
        let after = s.board.iter().flatten().filter(|&&v| v != 0).count();
        assert_eq!(after, before + 1);
    }

    // ── has_moves ───────────────────────────────────────────────────────────

    #[test]
    fn has_moves_with_empty_cell() {
        let s = make_state([
            [2, 4, 8, 16],
            [32, 64, 128, 256],
            [512, 1024, 2048, 0],
            [4, 8, 16, 32],
        ]);
        assert!(s.has_moves());
    }

    #[test]
    fn has_moves_full_checkerboard_no_equals() {
        let s = make_state([
            [2, 4, 2, 4],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
            [4, 2, 4, 2],
        ]);
        assert!(!s.has_moves());
    }

    #[test]
    fn has_moves_full_with_adjacent_row_equals() {
        let s = make_state([
            [2, 2, 4, 8],
            [4, 8, 2, 4],
            [2, 4, 8, 2],
            [4, 8, 4, 8],
        ]);
        assert!(s.has_moves());
    }

    #[test]
    fn has_moves_full_with_adjacent_col_equals() {
        let s = make_state([
            [2, 4, 8, 16],
            [2, 8, 4, 2],
            [4, 2, 8, 4],
            [8, 4, 2, 8],
        ]);
        assert!(s.has_moves());
    }

    // ── phase transitions ───────────────────────────────────────────────────

    #[test]
    fn phase_win_triggered_on_2048() {
        let mut s = make_state([[1024, 1024, 0, 0], [0; 4], [0; 4], [0; 4]]);
        s.shift(Direction::Left);
        assert_eq!(s.phase, Phase::Win);
        assert!(s.has_won);
    }

    #[test]
    fn phase_win_not_retriggered_after_continue() {
        let mut s = make_state([[1024, 1024, 0, 0], [0; 4], [0; 4], [0; 4]]);
        s.shift(Direction::Left);
        assert_eq!(s.phase, Phase::Win);

        // Player presses C to continue.
        s.phase = Phase::Playing;

        // Set up another 1024+1024 merge.
        s.board = [[1024, 1024, 0, 0], [0; 4], [0; 4], [0; 4]];
        s.shift(Direction::Left);

        // has_won is still true, so Win must not re-trigger.
        assert_ne!(s.phase, Phase::Win);
    }

    #[test]
    fn phase_game_over_when_stuck() {
        let mut s = make_state([
            [2, 4, 2, 4],
            [4, 2, 4, 2],
            [2, 4, 2, 4],
            [4, 2, 4, 2],
        ]);
        s.shift(Direction::Left);
        assert_eq!(s.phase, Phase::GameOver);
    }

    // ── new game ─────────────────────────────────────────────────────────────

    #[test]
    fn new_game_has_exactly_two_tiles() {
        let s = State2048::new();
        let count = s.board.iter().flatten().filter(|&&v| v != 0).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn new_game_tiles_are_2_or_4() {
        for _ in 0..50 {
            let s = State2048::new();
            for &v in s.board.iter().flatten().filter(|&&v| v != 0) {
                assert!(v == 2 || v == 4, "unexpected tile: {v}");
            }
        }
    }

    #[test]
    fn new_game_starts_in_playing_phase() {
        let s = State2048::new();
        assert_eq!(s.phase, Phase::Playing);
    }

    // ── rendering helpers ────────────────────────────────────────────────────

    #[test]
    fn build_grid_lines_has_correct_row_count() {
        let board = [[0u32; SIZE]; SIZE];
        assert_eq!(build_grid_lines(&board).len(), GRID_HEIGHT as usize);
    }

    #[test]
    fn centered_rect_is_inside_area() {
        let area = Rect { x: 0, y: 0, width: 80, height: 24 };
        let r = centered_rect(32, 9, area);
        assert!(r.x + r.width <= area.x + area.width);
        assert!(r.y + r.height <= area.y + area.height);
    }

    #[test]
    fn centered_rect_clamps_to_area() {
        let tiny = Rect { x: 0, y: 0, width: 10, height: 5 };
        let r = centered_rect(32, 9, tiny);
        assert_eq!(r.width, 10);
        assert_eq!(r.height, 5);
    }
}
