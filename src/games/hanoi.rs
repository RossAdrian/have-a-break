//! Tower of Hanoi game.

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

const NUM_DISKS: usize = 5;
const PEG_NAMES: [char; 3] = ['A', 'B', 'C'];
/// Display width of each peg column (must fit the widest disk: 2*NUM_DISKS-1 = 9).
const COL_WIDTH: usize = 13;

const DISK_COLORS: [Color; 5] = [
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Red,
];

/// All mutable state for one Tower of Hanoi session.
struct HanoiState {
    /// Three pegs ordered bottom-to-top (index 0 = bottommost / largest disk).
    pegs: [Vec<u8>; 3],
    /// Index of the peg selected as a move source, if any.
    selected: Option<usize>,
    moves: u32,
    /// Raised after an illegal move attempt; cleared after the next rendered frame.
    error: bool,
}

impl HanoiState {
    /// Construct a random valid starting configuration with at least one disk per peg.
    fn new() -> Self {
        let mut rng = new_rng();
        let mut pegs: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];

        // Guarantee one disk per peg by distributing the three largest across all three
        // pegs in shuffled order.
        let mut peg_order = [0usize, 1, 2];
        peg_order.shuffle(&mut rng);
        let large_disks = [
            NUM_DISKS as u8,
            NUM_DISKS as u8 - 1,
            NUM_DISKS as u8 - 2,
        ];
        for (&slot, &disk) in peg_order.iter().zip(large_disks.iter()) {
            pegs[slot].push(disk);
        }

        // Assign remaining smaller disks randomly; they are always valid atop the
        // already-placed larger disks.
        for disk in (1..=(NUM_DISKS as u8 - 3)).rev() {
            let peg_idx = rng.gen_range(0..3usize);
            pegs[peg_idx].push(disk);
        }

        Self {
            pegs,
            selected: None,
            moves: 0,
            error: false,
        }
    }

    /// Returns `true` when all disks are on a single peg.
    fn is_won(&self) -> bool {
        self.pegs.iter().any(|p| p.len() == NUM_DISKS)
    }

    /// Attempt to move the top disk of `src` onto `dst`.
    ///
    /// Returns `false` without mutating state if the move is illegal (placing a
    /// larger disk on a smaller one, moving from an empty peg, or src == dst).
    fn try_move(&mut self, src: usize, dst: usize) -> bool {
        if src == dst {
            return false;
        }
        let src_top = match self.pegs[src].last().copied() {
            Some(d) => d,
            None => return false,
        };
        if let Some(&dst_top) = self.pegs[dst].last()
            && src_top > dst_top
        {
            return false;
        }
        self.pegs[src].pop();
        self.pegs[dst].push(src_top);
        self.moves += 1;
        true
    }

    /// Map key character A/B/C (case-insensitive) to a peg index (0/1/2).
    fn peg_from_key(c: char) -> Option<usize> {
        match c.to_ascii_uppercase() {
            'A' => Some(0),
            'B' => Some(1),
            'C' => Some(2),
            _ => None,
        }
    }
}

/// Tower of Hanoi game entry point.
pub struct Hanoi;

impl Game for Hanoi {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = HanoiState::new();

        loop {
            let won = state.is_won();

            terminal.draw(|frame| {
                let area = frame.area();
                let block = Block::default().title(" Tower of Hanoi ").borders(Borders::ALL);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                let v = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1),                     // [1] move counter
                    Constraint::Length(1),                     // [2] gap
                    Constraint::Length(NUM_DISKS as u16 + 2), // [3] peg art (disks + ground + label)
                    Constraint::Length(1),                     // [4] gap
                    Constraint::Length(1),                     // [5] status
                    Constraint::Length(1),                     // [6] help
                    Constraint::Fill(1),
                ])
                .split(inner);

                let centred = |area| {
                    Layout::horizontal([
                        Constraint::Fill(1),
                        Constraint::Length((COL_WIDTH * 3) as u16),
                        Constraint::Fill(1),
                    ])
                    .split(area)[1]
                };

                // Move counter
                frame.render_widget(
                    Paragraph::new(format!("Moves: {}", state.moves))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::Cyan)),
                    v[1],
                );

                // Goal reminder — disks start scattered across all three pegs, so the
                // win condition (stack all of them on any single peg) is not obvious.
                if !won {
                    frame.render_widget(
                        Paragraph::new("Goal: move all disks onto a single peg")
                            .alignment(Alignment::Center)
                            .style(Style::default().fg(Color::DarkGray)),
                        v[2],
                    );
                }

                // Peg ASCII art
                frame.render_widget(Paragraph::new(build_peg_lines(&state)), centred(v[3]));

                // Status line
                let status: Paragraph = if won {
                    Paragraph::new(format!(
                        "Solved in {} move{}!  Press Q to return.",
                        state.moves,
                        if state.moves == 1 { "" } else { "s" }
                    ))
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if state.error {
                    Paragraph::new("Invalid move!")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                } else if let Some(sel) = state.selected {
                    Paragraph::new(format!(
                        "Peg {} selected — press A, B, or C to place",
                        PEG_NAMES[sel]
                    ))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow))
                } else {
                    Paragraph::new("Press A, B, or C to pick a source peg")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray))
                };
                frame.render_widget(status, v[5]);

                // Help line — omitted on the won screen since the status line above
                // already tells the player to press Q.
                if !won {
                    frame.render_widget(
                        Paragraph::new("A/B/C — pick/place  •  Esc — menu")
                            .alignment(Alignment::Center)
                            .style(Style::default().fg(Color::DarkGray)),
                        v[6],
                    );
                }
            })?;

            // Clear the one-shot error flag now that it has been rendered.
            state.error = false;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Esc => return Ok(GameResult::BackToMenu),
                    KeyCode::Char('q') | KeyCode::Char('Q') if won => {
                        return Ok(GameResult::BackToMenu)
                    }
                    KeyCode::Char(c) if !won => {
                        if let Some(peg_idx) = HanoiState::peg_from_key(c) {
                            match state.selected {
                                // Pressing the already-selected peg deselects it.
                                Some(src) if src == peg_idx => {
                                    state.selected = None;
                                }
                                None => {
                                    if !state.pegs[peg_idx].is_empty() {
                                        state.selected = Some(peg_idx);
                                    }
                                }
                                Some(src) => {
                                    state.selected = None;
                                    if !state.try_move(src, peg_idx) {
                                        state.error = true;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Build the ASCII-art `Line`s for all three pegs.
///
/// Layout per column (width = `COL_WIDTH`):
/// - Rows 0..NUM_DISKS: disk slot or bare peg bar
/// - Row NUM_DISKS: ground dashes
/// - Row NUM_DISKS+1: peg label (A / B / C)
fn build_peg_lines(state: &HanoiState) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(NUM_DISKS + 2);

    // Disk rows (row 0 = topmost slot).
    for row in 0..NUM_DISKS {
        let spans: Vec<Span<'static>> = (0..3)
            .map(|peg_idx| {
                let peg = &state.pegs[peg_idx];
                let k = peg.len();
                if row >= NUM_DISKS - k {
                    // Disk present at this row.
                    // peg[0] = bottom (largest), peg[k-1] = top (smallest).
                    // Display row `r` maps to peg index `NUM_DISKS - 1 - r`.
                    let disk = peg[NUM_DISKS - 1 - row] as usize;
                    let w = 2 * disk - 1;
                    let pad = COL_WIDTH / 2 - (disk - 1); // = (COL_WIDTH - w) / 2
                    let content = format!(
                        "{}{}{}",
                        " ".repeat(pad),
                        "=".repeat(w),
                        " ".repeat(pad)
                    );
                    // Highlight the top disk of the selected source peg.
                    let is_top = row == NUM_DISKS - k;
                    let style = if state.selected == Some(peg_idx) && is_top {
                        Style::default()
                            .fg(DISK_COLORS[disk - 1])
                            .add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default().fg(DISK_COLORS[disk - 1])
                    };
                    Span::styled(content, style)
                } else {
                    // Empty slot: bare peg bar centred in the column.
                    let half = COL_WIDTH / 2;
                    Span::raw(format!(
                        "{}|{}",
                        " ".repeat(half),
                        " ".repeat(COL_WIDTH - half - 1)
                    ))
                }
            })
            .collect();
        lines.push(Line::from(spans));
    }

    // Ground row.
    lines.push(Line::from(
        (0..3)
            .map(|_| Span::styled("-".repeat(COL_WIDTH), Style::default().fg(Color::Gray)))
            .collect::<Vec<_>>(),
    ));

    // Label row.
    lines.push(Line::from(
        (0..3)
            .map(|i| {
                let label = format!("{:^width$}", PEG_NAMES[i], width = COL_WIDTH);
                let style = if state.selected == Some(i) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Span::styled(label, style)
            })
            .collect::<Vec<_>>(),
    ));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_valid_distribution() {
        let state = HanoiState::new();
        // All NUM_DISKS disks are present exactly once.
        let mut all: Vec<u8> = state.pegs.iter().flatten().copied().collect();
        all.sort_unstable();
        let expected: Vec<u8> = (1..=NUM_DISKS as u8).collect();
        assert_eq!(all, expected);
    }

    #[test]
    fn new_game_every_peg_non_empty() {
        // Run several times since the distribution is random.
        for _ in 0..20 {
            let state = HanoiState::new();
            for (i, peg) in state.pegs.iter().enumerate() {
                assert!(!peg.is_empty(), "peg {i} must start with at least one disk");
            }
        }
    }

    #[test]
    fn new_game_stacking_invariant() {
        for _ in 0..20 {
            let state = HanoiState::new();
            for peg in &state.pegs {
                for w in peg.windows(2) {
                    assert!(w[0] > w[1], "lower disk must be larger: {} > {}", w[0], w[1]);
                }
            }
        }
    }

    #[test]
    fn not_won_at_start() {
        // With every peg having at least one disk, no peg can hold all NUM_DISKS.
        let state = HanoiState::new();
        assert!(!state.is_won());
    }

    #[test]
    fn is_won_all_on_one_peg() {
        let state = HanoiState {
            pegs: [vec![5, 4, 3, 2, 1], vec![], vec![]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(state.is_won());
    }

    #[test]
    fn is_won_on_any_peg() {
        let state = HanoiState {
            pegs: [vec![], vec![], vec![5, 4, 3, 2, 1]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(state.is_won());
    }

    #[test]
    fn try_move_valid_move() {
        let mut state = HanoiState {
            pegs: [vec![3, 1], vec![2], vec![]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(state.try_move(0, 2));
        assert_eq!(state.pegs[0], vec![3]);
        assert_eq!(state.pegs[2], vec![1]);
        assert_eq!(state.moves, 1);
    }

    #[test]
    fn try_move_larger_onto_smaller_rejected() {
        let mut state = HanoiState {
            pegs: [vec![1], vec![2], vec![]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(!state.try_move(1, 0));
        assert_eq!(state.pegs[0], vec![1]);
        assert_eq!(state.pegs[1], vec![2]);
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn try_move_from_empty_peg_rejected() {
        let mut state = HanoiState {
            pegs: [vec![], vec![3], vec![]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(!state.try_move(0, 1));
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn try_move_same_peg_rejected() {
        let mut state = HanoiState {
            pegs: [vec![3, 1], vec![], vec![]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(!state.try_move(0, 0));
        assert_eq!(state.moves, 0);
    }

    #[test]
    fn try_move_to_empty_peg_accepted() {
        let mut state = HanoiState {
            pegs: [vec![5], vec![], vec![]],
            selected: None,
            moves: 0,
            error: false,
        };
        assert!(state.try_move(0, 2));
        assert!(state.pegs[0].is_empty());
        assert_eq!(state.pegs[2], vec![5]);
    }

    #[test]
    fn peg_from_key_covers_all_cases() {
        assert_eq!(HanoiState::peg_from_key('a'), Some(0));
        assert_eq!(HanoiState::peg_from_key('A'), Some(0));
        assert_eq!(HanoiState::peg_from_key('b'), Some(1));
        assert_eq!(HanoiState::peg_from_key('B'), Some(1));
        assert_eq!(HanoiState::peg_from_key('c'), Some(2));
        assert_eq!(HanoiState::peg_from_key('C'), Some(2));
        assert_eq!(HanoiState::peg_from_key('d'), None);
        assert_eq!(HanoiState::peg_from_key('1'), None);
    }
}
