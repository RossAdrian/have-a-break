//! Sliding Puzzle game (3×3 / 8-puzzle).

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

/// Goal state: tiles 1–8 in row-major order, blank (0) at bottom-right.
const GOAL: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 0];

/// ASCII separator row that divides grid cells.
const SEP: &str = "+-----+-----+-----+";

/// Width of the rendered grid in terminal columns.
const GRID_WIDTH: u16 = 19; // "+-----+-----+-----+" = 19 chars

/// Height of the rendered grid in terminal rows (4 separators + 3 data rows).
const GRID_HEIGHT: u16 = 7;

/// All mutable state for one Sliding Puzzle session.
struct SlidingState {
    /// Board in row-major order. `0` represents the blank.
    tiles: [u8; 9],
    moves: u32,
}

impl SlidingState {
    /// Construct a random solvable starting configuration that differs from the goal.
    fn new() -> Self {
        let mut rng = new_rng();
        let mut tiles = GOAL;
        loop {
            tiles.shuffle(&mut rng);
            if is_solvable(&tiles) && tiles != GOAL {
                break;
            }
        }
        Self { tiles, moves: 0 }
    }

    /// Returns the index of the blank tile.
    fn blank_pos(&self) -> usize {
        self.tiles
            .iter()
            .position(|&t| t == 0)
            .expect("board must contain a blank")
    }

    /// Returns `true` when the board matches the goal state.
    fn is_solved(&self) -> bool {
        self.tiles == GOAL
    }

    /// Try to move the blank in the given direction.
    ///
    /// Returns `true` if the move was legal and applied; `false` when the blank
    /// is already at the edge in that direction.
    fn slide(&mut self, dir: Direction) -> bool {
        let blank = self.blank_pos();
        let row = blank / 3;
        let col = blank % 3;

        let new_blank = match dir {
            Direction::Up if row > 0 => blank - 3,
            Direction::Down if row < 2 => blank + 3,
            Direction::Left if col > 0 => blank - 1,
            Direction::Right if col < 2 => blank + 1,
            _ => return false,
        };

        self.tiles.swap(blank, new_blank);
        self.moves += 1;
        true
    }
}

/// Direction of blank movement (matches the pressed arrow key).
#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Returns `true` when the configuration is solvable.
///
/// For a 3×3 grid with the blank in the bottom-right corner at the goal,
/// a configuration is solvable iff the inversion count of its non-blank tiles
/// is even.
fn is_solvable(tiles: &[u8; 9]) -> bool {
    let flat: Vec<u8> = tiles.iter().copied().filter(|&t| t != 0).collect();
    let inversions: usize = flat
        .iter()
        .enumerate()
        .map(|(i, &a)| flat[i + 1..].iter().filter(|&&b| a > b).count())
        .sum();
    inversions.is_multiple_of(2)
}

/// Performance rating label based on move count.
fn rating(moves: u32) -> &'static str {
    if moves < 30 {
        "Impressive!"
    } else if moves <= 60 {
        "Solid!"
    } else {
        "Solved it!"
    }
}

/// Build the styled `Line`s that render the 3×3 grid.
///
/// Layout: alternating separator rows and data rows, 7 lines total.
fn build_grid_lines(tiles: &[u8; 9]) -> Vec<Line<'static>> {
    let sep_style = Style::default().fg(Color::DarkGray);
    let tile_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let mut lines = Vec::with_capacity(GRID_HEIGHT as usize);
    lines.push(Line::from(Span::styled(SEP, sep_style)));

    for row in 0..3 {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("|", sep_style));
        for col in 0..3 {
            let tile = tiles[row * 3 + col];
            if tile == 0 {
                spans.push(Span::raw("     "));
            } else {
                spans.push(Span::styled(format!("  {}  ", tile), tile_style));
            }
            spans.push(Span::styled("|", sep_style));
        }
        lines.push(Line::from(spans));
        lines.push(Line::from(Span::styled(SEP, sep_style)));
    }

    lines
}

/// Sliding Puzzle game entry point.
pub struct Sliding;

impl Game for Sliding {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = SlidingState::new();

        loop {
            let solved = state.is_solved();

            terminal.draw(|frame| {
                let area = frame.area();
                let block = Block::default()
                    .title(" Sliding Puzzle ")
                    .borders(Borders::ALL);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                let v = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1),           // move counter
                    Constraint::Length(1),           // gap
                    Constraint::Length(GRID_HEIGHT), // grid
                    Constraint::Length(1),           // gap
                    Constraint::Length(1),           // status / congrats
                    Constraint::Length(1),           // help line
                    Constraint::Fill(1),
                ])
                .split(inner);

                let centred_grid = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Length(GRID_WIDTH),
                    Constraint::Fill(1),
                ])
                .split(v[3])[1];

                // Move counter
                frame.render_widget(
                    Paragraph::new(format!("Moves: {}", state.moves))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::Cyan)),
                    v[1],
                );

                // Grid
                frame.render_widget(
                    Paragraph::new(build_grid_lines(&state.tiles)),
                    centred_grid,
                );

                // Status / congratulations
                let status = if solved {
                    Paragraph::new(format!(
                        "Solved in {} move{}!  {}",
                        state.moves,
                        if state.moves == 1 { "" } else { "s" },
                        rating(state.moves),
                    ))
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Paragraph::new(String::new()).alignment(Alignment::Center)
                };
                frame.render_widget(status, v[5]);

                // Help line
                let help = if solved {
                    "Q — return to menu"
                } else {
                    "↑↓←→ — slide  •  Esc — menu"
                };
                frame.render_widget(
                    Paragraph::new(help)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray)),
                    v[6],
                );
            })?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Esc => return Ok(GameResult::BackToMenu),
                    KeyCode::Char('q') | KeyCode::Char('Q') if solved => {
                        return Ok(GameResult::BackToMenu)
                    }
                    KeyCode::Up if !solved => {
                        state.slide(Direction::Up);
                    }
                    KeyCode::Down if !solved => {
                        state.slide(Direction::Down);
                    }
                    KeyCode::Left if !solved => {
                        state.slide(Direction::Left);
                    }
                    KeyCode::Right if !solved => {
                        state.slide(Direction::Right);
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

    #[test]
    fn goal_is_solvable() {
        assert!(is_solvable(&GOAL));
    }

    #[test]
    fn single_swap_makes_unsolvable() {
        // Swapping exactly two non-blank tiles flips parity from even to odd.
        let mut bad = GOAL;
        bad.swap(0, 1); // [2, 1, 3, 4, 5, 6, 7, 8, 0] → 1 inversion (odd)
        assert!(!is_solvable(&bad));
    }

    #[test]
    fn double_swap_stays_solvable() {
        // Two independent swaps restore even parity.
        let mut tiles = GOAL;
        tiles.swap(0, 1);
        tiles.swap(2, 3); // [2, 1, 4, 3, 5, 6, 7, 8, 0] → 2 inversions (even)
        assert!(is_solvable(&tiles));
    }

    #[test]
    fn new_game_not_solved() {
        for _ in 0..20 {
            let state = SlidingState::new();
            assert!(!state.is_solved(), "new game must not start in the solved state");
        }
    }

    #[test]
    fn new_game_contains_all_tiles() {
        let state = SlidingState::new();
        let mut sorted = state.tiles;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn new_game_is_always_solvable() {
        for _ in 0..50 {
            let state = SlidingState::new();
            assert!(
                is_solvable(&state.tiles),
                "generated board must be solvable: {:?}",
                state.tiles
            );
        }
    }

    #[test]
    fn blank_pos_correct() {
        let state = SlidingState {
            tiles: [1, 2, 3, 4, 5, 0, 6, 7, 8],
            moves: 0,
        };
        assert_eq!(state.blank_pos(), 5);
    }

    #[test]
    fn slide_up_moves_blank() {
        // Blank at index 8 (row=2, col=2); sliding up moves it to index 5.
        let mut state = SlidingState {
            tiles: [1, 2, 3, 4, 5, 6, 7, 8, 0],
            moves: 0,
        };
        assert!(state.slide(Direction::Up));
        assert_eq!(state.tiles[5], 0, "blank should now be at index 5");
        assert_eq!(state.tiles[8], 6, "tile 6 should have slid down to index 8");
        assert_eq!(state.moves, 1);
    }

    #[test]
    fn slide_down_moves_blank() {
        // Blank at index 2 (row=0, col=2); sliding down moves it to index 5.
        let mut state = SlidingState {
            tiles: [1, 2, 0, 3, 4, 5, 6, 7, 8],
            moves: 0,
        };
        assert!(state.slide(Direction::Down));
        assert_eq!(state.tiles[5], 0);
        assert_eq!(state.tiles[2], 5);
        assert_eq!(state.moves, 1);
    }

    #[test]
    fn slide_left_moves_blank() {
        // Blank at index 5 (row=1, col=2); sliding left moves it to index 4.
        let mut state = SlidingState {
            tiles: [1, 2, 3, 4, 6, 0, 7, 8, 5],
            moves: 0,
        };
        assert!(state.slide(Direction::Left));
        assert_eq!(state.tiles[4], 0);
        assert_eq!(state.tiles[5], 6);
        assert_eq!(state.moves, 1);
    }

    #[test]
    fn slide_right_moves_blank() {
        // Blank at index 3 (row=1, col=0); sliding right moves it to index 4.
        let mut state = SlidingState {
            tiles: [1, 2, 3, 0, 5, 6, 7, 8, 4],
            moves: 0,
        };
        assert!(state.slide(Direction::Right));
        assert_eq!(state.tiles[4], 0);
        assert_eq!(state.tiles[3], 5);
        assert_eq!(state.moves, 1);
    }

    #[test]
    fn slide_blocked_at_top_edge() {
        let mut state = SlidingState {
            tiles: [0, 1, 2, 3, 4, 5, 6, 7, 8], // blank at row=0
            moves: 0,
        };
        assert!(!state.slide(Direction::Up));
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn slide_blocked_at_bottom_edge() {
        let mut state = SlidingState {
            tiles: [1, 2, 3, 4, 5, 6, 0, 7, 8], // blank at row=2, col=0
            moves: 0,
        };
        assert!(!state.slide(Direction::Down));
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn slide_blocked_at_left_edge() {
        let mut state = SlidingState {
            tiles: [0, 1, 2, 3, 4, 5, 6, 7, 8], // blank at col=0
            moves: 0,
        };
        assert!(!state.slide(Direction::Left));
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn slide_blocked_at_right_edge() {
        let mut state = SlidingState {
            tiles: [1, 2, 3, 4, 5, 6, 7, 8, 0], // blank at col=2
            moves: 0,
        };
        assert!(!state.slide(Direction::Right));
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn is_solved_goal_state() {
        let state = SlidingState {
            tiles: GOAL,
            moves: 0,
        };
        assert!(state.is_solved());
    }

    #[test]
    fn is_solved_non_goal() {
        let state = SlidingState {
            tiles: [1, 2, 3, 4, 5, 6, 7, 0, 8],
            moves: 0,
        };
        assert!(!state.is_solved());
    }

    #[test]
    fn rating_thresholds() {
        assert_eq!(rating(0), "Impressive!");
        assert_eq!(rating(29), "Impressive!");
        assert_eq!(rating(30), "Solid!");
        assert_eq!(rating(60), "Solid!");
        assert_eq!(rating(61), "Solved it!");
        assert_eq!(rating(200), "Solved it!");
    }

    #[test]
    fn build_grid_lines_has_correct_count() {
        // 4 separator rows + 3 data rows = 7 lines.
        let lines = build_grid_lines(&GOAL);
        assert_eq!(lines.len(), 7);
    }

    #[test]
    fn solve_by_reversing_one_move() {
        // Start one move away from solved: blank at index 7 instead of 8.
        let mut state = SlidingState {
            tiles: [1, 2, 3, 4, 5, 6, 7, 0, 8], // blank at (2,1)
            moves: 0,
        };
        assert!(!state.is_solved());
        assert!(state.slide(Direction::Right)); // blank moves right → solved
        assert!(state.is_solved());
        assert_eq!(state.moves, 1);
    }
}
