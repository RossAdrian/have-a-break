//! Graph Coloring game.

use std::collections::HashSet;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    common::rng::new_rng,
    games::{Game, GameResult},
    terminal::Term,
};

const NODE_COUNT: usize = 6;
const NODE_LABELS: [char; NODE_COUNT] = ['A', 'B', 'C', 'D', 'E', 'F'];

/// Fixed width of the right-hand panel in terminal columns.
const PANEL_W: u16 = 32;

/// Terminal foreground colors for coloring slots 1–5.
const PALETTE: [Color; 5] = [
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Cyan,
    Color::Magenta,
];

const PALETTE_NAMES: [&str; 5] = ["Red", "Green", "Yellow", "Cyan", "Magenta"];

const MIN_EDGES: usize = 8;
const MAX_EDGES: usize = 10;

/// Number of render ticks for which the conflict warning remains visible.
const FLASH_TICKS: u8 = 20;

/// Using ≤ this many colors earns an "Optimal!" note.
const HINT_THRESHOLD: usize = 3;

// ── node layout ────────────────────────────────────────────────────────────

/// Compute `(col, row)` center positions for the six nodes, fitted to the
/// given canvas dimensions.
///
/// The hexagon is centred at `(w/2, h/2)`.  The row radius is `h/2 − 2` so
/// the top and bottom nodes always have a 2-row gap from the canvas edge.
/// The column radius is `min(2 × row_radius, w/2 − 4)` so every node label
/// `[X]` has at least 3 columns of clearance from the left and right edges.
fn node_positions(w: usize, h: usize) -> [(usize, usize); NODE_COUNT] {
    let cx = w / 2;
    let cy = h / 2;
    let r = cy.saturating_sub(2);
    let half_r = r.div_ceil(2);
    let c = (r.saturating_mul(2)).min(cx.saturating_sub(4));
    [
        (cx,                         cy.saturating_sub(r)),       // A – top
        (cx.saturating_add(c),       cy.saturating_sub(half_r)),  // B – upper right
        (cx.saturating_add(c),       cy + half_r),                // C – lower right
        (cx,                         cy + r),                     // D – bottom
        (cx.saturating_sub(c),       cy + half_r),                // E – lower left
        (cx.saturating_sub(c),       cy.saturating_sub(half_r)),  // F – upper left
    ]
}

// ── state ──────────────────────────────────────────────────────────────────

/// All mutable state for one Graph Coloring session.
struct GraphState {
    edges: Vec<(usize, usize)>,
    /// Assigned color slot (0–4) per node, or `None` if uncolored.
    colors: [Option<usize>; NODE_COUNT],
    /// Currently selected node index.
    selected: Option<usize>,
    /// Countdown ticks for the conflict warning flash.
    conflict_flash: u8,
    solved: bool,
}

impl GraphState {
    fn new() -> Self {
        let mut rng = new_rng();
        let target = rng.gen_range(MIN_EDGES..=MAX_EDGES);
        let edges = generate_graph(&mut rng, target);
        Self {
            edges,
            colors: [None; NODE_COUNT],
            selected: None,
            conflict_flash: 0,
            solved: false,
        }
    }

    /// Reset color assignments for a new attempt on the same graph.
    fn reset_colors(&mut self) {
        self.colors = [None; NODE_COUNT];
        self.conflict_flash = 0;
        self.solved = false;
        self.selected = None;
    }

    /// `true` if assigning `color` to `node` would clash with an already-colored
    /// adjacent node.
    fn would_conflict(&self, node: usize, color: usize) -> bool {
        self.edges.iter().any(|&(u, v)| {
            if u == node {
                self.colors[v] == Some(color)
            } else if v == node {
                self.colors[u] == Some(color)
            } else {
                false
            }
        })
    }

    fn has_any_conflict(&self) -> bool {
        self.edges.iter().any(|&(u, v)| {
            self.colors[u].is_some() && self.colors[u] == self.colors[v]
        })
    }

    /// Number of distinct color slots currently in use.
    fn colors_used(&self) -> usize {
        let mut seen: HashSet<usize> = HashSet::new();
        for c in self.colors.iter().flatten() {
            seen.insert(*c);
        }
        seen.len()
    }

    /// Attempt to assign `color` to the currently selected node.
    ///
    /// Starts the conflict flash and returns `false` if the assignment would
    /// create an adjacency conflict or if no node is selected.
    fn assign_color(&mut self, color: usize) -> bool {
        let Some(node) = self.selected else {
            return false;
        };
        if self.would_conflict(node, color) {
            self.conflict_flash = FLASH_TICKS;
            return false;
        }
        self.colors[node] = Some(color);
        if self.colors.iter().all(|c| c.is_some()) && !self.has_any_conflict() {
            self.solved = true;
        }
        true
    }
}

// ── graph generation ───────────────────────────────────────────────────────

/// Generate a random connected graph with `target` edges over [`NODE_COUNT`] nodes.
///
/// Connectivity is guaranteed by first building a random spanning tree via
/// incremental Prim-style construction, then padding with random extra edges
/// until `target` is reached.
fn generate_graph(rng: &mut impl Rng, target: usize) -> Vec<(usize, usize)> {
    let mut edges: HashSet<(usize, usize)> = HashSet::new();

    let mut visited = [false; NODE_COUNT];
    visited[rng.gen_range(0..NODE_COUNT)] = true;

    for _ in 1..NODE_COUNT {
        let vis: Vec<usize> = (0..NODE_COUNT).filter(|&i| visited[i]).collect();
        let unvis: Vec<usize> = (0..NODE_COUNT).filter(|&i| !visited[i]).collect();
        let from = vis[rng.gen_range(0..vis.len())];
        let to = unvis[rng.gen_range(0..unvis.len())];
        visited[to] = true;
        edges.insert((from.min(to), from.max(to)));
    }

    // Add extra edges up to target, discarding duplicates and self-loops.
    let mut attempts = 0;
    while edges.len() < target && attempts < 500 {
        let u = rng.gen_range(0..NODE_COUNT);
        let v = rng.gen_range(0..NODE_COUNT);
        if u != v {
            edges.insert((u.min(v), u.max(v)));
        }
        attempts += 1;
    }

    edges.into_iter().collect()
}

// ── ASCII canvas ───────────────────────────────────────────────────────────

/// A 2-D grid of characters with per-cell [`Style`] for graph rendering.
struct Canvas {
    width: usize,
    height: usize,
    chars: Vec<Vec<char>>,
    styles: Vec<Vec<Style>>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Canvas {
            width,
            height,
            chars: vec![vec![' '; width]; height],
            styles: vec![vec![Style::default(); width]; height],
        }
    }

    fn put(&mut self, col: usize, row: usize, ch: char, style: Style) {
        if row < self.height && col < self.width {
            self.chars[row][col] = ch;
            self.styles[row][col] = style;
        }
    }

    /// Draw a line between `(x0, y0)` and `(x1, y1)` using Bresenham's algorithm.
    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, style: Style) {
        let ch = edge_char(x1 - x0, y1 - y0);
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx: i32 = if x0 < x1 { 1 } else { -1 };
        let sy: i32 = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            if x >= 0 && y >= 0 {
                self.put(x as usize, y as usize, ch, style);
            }
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Consume the canvas and convert it to a `Vec<Line<'static>>` for ratatui.
    fn into_lines(self) -> Vec<Line<'static>> {
        self.chars
            .into_iter()
            .zip(self.styles)
            .map(|(chars, styles)| {
                let mut spans: Vec<Span<'static>> = Vec::new();
                if chars.is_empty() {
                    return Line::default();
                }
                let mut cur_style = styles[0];
                let mut buf = String::new();
                for (ch, st) in chars.into_iter().zip(styles) {
                    if st == cur_style {
                        buf.push(ch);
                    } else {
                        spans.push(Span::styled(buf.clone(), cur_style));
                        buf.clear();
                        cur_style = st;
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    spans.push(Span::styled(buf, cur_style));
                }
                Line::from(spans)
            })
            .collect()
    }
}

/// Choose the ASCII character that best represents a line segment's direction.
///
/// Applies a 2:1 aspect-ratio correction (terminal rows are roughly twice as
/// tall as columns are wide) before classifying the slope.
fn edge_char(dx: i32, dy: i32) -> char {
    if dy == 0 {
        return '─';
    }
    if dx == 0 {
        return '│';
    }
    let dx_abs = dx.abs() as f32;
    let dy_vis = dy.abs() as f32 * 0.5; // compress rows to visual units
    if dy_vis < dx_abs * 0.4 {
        '─'
    } else if dx_abs < dy_vis * 0.4 {
        '│'
    } else if (dx > 0) == (dy > 0) {
        '\\'
    } else {
        '/'
    }
}

/// Build the canvas lines for the current graph state fitted to `(w, h)` cells.
fn build_graph_lines(state: &GraphState, w: usize, h: usize) -> Vec<Line<'static>> {
    let positions = node_positions(w, h);
    let mut canvas = Canvas::new(w, h);
    let edge_style = Style::default().fg(Color::DarkGray);

    // Draw edges first; node labels will overwrite the occupied cells.
    for &(u, v) in &state.edges {
        let (cx0, cy0) = positions[u];
        let (cx1, cy1) = positions[v];
        canvas.draw_line(cx0 as i32, cy0 as i32, cx1 as i32, cy1 as i32, edge_style);
    }

    // Draw `[X]` labels on top of edges.
    for (i, &(cx, cy)) in positions.iter().enumerate() {
        let fg = match state.colors[i] {
            Some(c) => PALETTE[c],
            None => Color::White,
        };
        let mut style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        if state.selected == Some(i) {
            style = style.add_modifier(Modifier::REVERSED);
        }
        if cx > 0 {
            canvas.put(cx - 1, cy, '[', style);
        }
        canvas.put(cx, cy, NODE_LABELS[i], style);
        canvas.put(cx + 1, cy, ']', style);
    }

    canvas.into_lines()
}

// ── game entry point ───────────────────────────────────────────────────────

/// Graph Coloring game entry point.
pub struct GraphColoring;

impl Game for GraphColoring {
    fn run(&mut self, terminal: &mut Term) -> Result<GameResult> {
        let mut state = GraphState::new();

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let block = Block::default()
                    .title(" Graph Coloring ")
                    .borders(Borders::ALL);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                // Left: canvas fills remaining space  |  Right: fixed-width panel.
                let h = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Length(PANEL_W),
                ])
                .split(inner);

                // Graph canvas: always the full height and width of h[0].
                let canvas_area = h[0];
                let graph_lines = build_graph_lines(
                    &state,
                    canvas_area.width as usize,
                    canvas_area.height as usize,
                );
                frame.render_widget(Paragraph::new(graph_lines), canvas_area);

                // Right panel: legend, node summary, status, help.
                let mut panel: Vec<Line<'static>> = Vec::new();

                panel.push(Line::from(Span::styled(
                    "Color palette:",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                for (i, (&col, &name)) in PALETTE.iter().zip(PALETTE_NAMES.iter()).enumerate() {
                    panel.push(Line::from(vec![
                        Span::raw(format!("  {} ", i + 1)),
                        Span::styled(format!("■ {name}"), Style::default().fg(col)),
                    ]));
                }

                panel.push(Line::raw(""));

                // Node summary row.
                let node_spans: Vec<Span<'static>> = (0..NODE_COUNT)
                    .flat_map(|i| {
                        let fg = match state.colors[i] {
                            Some(c) => PALETTE[c],
                            None => Color::DarkGray,
                        };
                        let mut st =
                            Style::default().fg(fg).add_modifier(Modifier::BOLD);
                        if state.selected == Some(i) {
                            st = st.add_modifier(Modifier::REVERSED);
                        }
                        [
                            Span::styled(format!("[{}]", NODE_LABELS[i]), st),
                            Span::raw(" "),
                        ]
                    })
                    .collect();
                panel.push(Line::from(node_spans));
                panel.push(Line::raw(""));

                let colors_used = state.colors_used();
                if colors_used > 0 {
                    panel.push(Line::from(Span::styled(
                        format!("Colors used: {colors_used}"),
                        Style::default().fg(Color::Cyan),
                    )));
                    panel.push(Line::raw(""));
                }

                if state.conflict_flash > 0 {
                    panel.push(Line::from(Span::styled(
                        "! Conflict — try another color",
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )));
                    panel.push(Line::raw(""));
                }

                if state.solved {
                    let note = if colors_used <= HINT_THRESHOLD {
                        " Optimal!"
                    } else {
                        " Try fewer!"
                    };
                    let plural = if colors_used == 1 { "" } else { "s" };
                    panel.push(Line::from(Span::styled(
                        format!("Solved: {colors_used} color{plural}!{note}"),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )));
                    panel.push(Line::raw(""));
                    panel.push(Line::from(Span::styled(
                        "R — try with fewer colors",
                        Style::default().fg(Color::DarkGray),
                    )));
                    panel.push(Line::from(Span::styled(
                        "Q — back to menu",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    let hint = match state.selected {
                        Some(i) => format!("Selected [{}] — press 1–5", NODE_LABELS[i]),
                        None => "A–F select node  •  1–5 color".to_string(),
                    };
                    panel.push(Line::from(Span::styled(
                        hint,
                        Style::default().fg(Color::DarkGray),
                    )));
                    panel.push(Line::from(Span::styled(
                        "Esc — back to menu",
                        Style::default().fg(Color::DarkGray),
                    )));
                }

                frame.render_widget(Paragraph::new(panel), h[1]);
            })?;

            if state.conflict_flash > 0 {
                state.conflict_flash -= 1;
            }

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Esc => return Ok(GameResult::BackToMenu),
                    KeyCode::Char('q') | KeyCode::Char('Q') if state.solved => {
                        return Ok(GameResult::BackToMenu);
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') if state.solved => {
                        state.reset_colors();
                    }
                    KeyCode::Char(c) if !state.solved => match c {
                        'a'..='f' => {
                            state.selected = Some(c as usize - b'a' as usize);
                        }
                        'A'..='F' => {
                            state.selected = Some(c as usize - b'A' as usize);
                        }
                        '1'..='5' => {
                            let color_idx = c as usize - b'1' as usize;
                            state.assign_color(color_idx);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    fn seeded_graph(seed: u64, target: usize) -> Vec<(usize, usize)> {
        let mut rng = StdRng::seed_from_u64(seed);
        generate_graph(&mut rng, target)
    }

    fn is_connected(edges: &[(usize, usize)]) -> bool {
        let mut visited = [false; NODE_COUNT];
        visited[0] = true;
        let mut changed = true;
        while changed {
            changed = false;
            for &(u, v) in edges {
                if visited[u] && !visited[v] {
                    visited[v] = true;
                    changed = true;
                }
                if visited[v] && !visited[u] {
                    visited[u] = true;
                    changed = true;
                }
            }
        }
        visited.iter().all(|&v| v)
    }

    #[test]
    fn graph_is_always_connected() {
        for seed in 0..50u64 {
            let edges = seeded_graph(seed, 9);
            assert!(is_connected(&edges), "graph with seed {seed} not connected");
        }
    }

    #[test]
    fn graph_edge_count_in_range() {
        for seed in 0..50u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let target = rng.gen_range(MIN_EDGES..=MAX_EDGES);
            let edges = generate_graph(&mut rng, target);
            assert!(
                edges.len() >= NODE_COUNT - 1,
                "seed {seed}: fewer edges than spanning tree"
            );
            assert!(
                edges.len() <= MAX_EDGES,
                "seed {seed}: exceeds max edges ({} > {})",
                edges.len(),
                MAX_EDGES
            );
        }
    }

    #[test]
    fn no_self_loops() {
        for seed in 0..50u64 {
            for (u, v) in seeded_graph(seed, 9) {
                assert_ne!(u, v, "self-loop at {u} in seed {seed}");
            }
        }
    }

    #[test]
    fn no_duplicate_edges() {
        for seed in 0..50u64 {
            let edges = seeded_graph(seed, 10);
            let set: HashSet<_> = edges.iter().copied().collect();
            assert_eq!(
                set.len(),
                edges.len(),
                "duplicate edge detected at seed {seed}"
            );
        }
    }

    #[test]
    fn conflict_detection_adjacent_same_color() {
        let mut state = GraphState {
            edges: vec![(0, 1)],
            colors: [None; NODE_COUNT],
            selected: Some(0),
            conflict_flash: 0,
            solved: false,
        };
        state.colors[1] = Some(0);
        assert!(state.would_conflict(0, 0));
        assert!(!state.would_conflict(0, 1));
    }

    #[test]
    fn assign_color_rejects_conflict() {
        let mut state = GraphState {
            edges: vec![(0, 1)],
            colors: [None; NODE_COUNT],
            selected: Some(0),
            conflict_flash: 0,
            solved: false,
        };
        state.colors[1] = Some(2);
        assert!(!state.assign_color(2));
        assert_eq!(state.conflict_flash, FLASH_TICKS);
        assert_eq!(state.colors[0], None);
    }

    #[test]
    fn assign_color_accepts_valid() {
        let mut state = GraphState {
            edges: vec![(0, 1)],
            colors: [None; NODE_COUNT],
            selected: Some(0),
            conflict_flash: 0,
            solved: false,
        };
        state.colors[1] = Some(1);
        assert!(state.assign_color(0));
        assert_eq!(state.colors[0], Some(0));
        assert_eq!(state.conflict_flash, 0);
    }

    #[test]
    fn solved_when_all_colored_without_conflict() {
        // Graph: 0-1-2 path.  Color assignment: 0→red, 1→green, rest→red.
        // Edge (0,1): 0≠1 ✓   Edge (1,2): 1≠0 ✓
        let mut state = GraphState {
            edges: vec![(0, 1), (1, 2)],
            colors: [Some(0), Some(1), Some(0), Some(0), Some(0), Some(0)],
            selected: Some(0),
            conflict_flash: 0,
            solved: false,
        };
        // Re-assign node 0 to trigger the solved check.
        state.colors[0] = None;
        assert!(state.assign_color(0));
        assert!(state.solved);
    }

    #[test]
    fn not_solved_while_uncolored_node_remains() {
        let mut state = GraphState {
            edges: vec![(0, 1)],
            colors: [None; NODE_COUNT],
            selected: Some(0),
            conflict_flash: 0,
            solved: false,
        };
        state.assign_color(0);
        assert!(!state.solved);
    }

    #[test]
    fn colors_used_counts_distinct_slots() {
        let mut state = GraphState {
            edges: vec![],
            colors: [None; NODE_COUNT],
            selected: None,
            conflict_flash: 0,
            solved: false,
        };
        state.colors = [Some(0), Some(2), Some(0), None, None, None];
        assert_eq!(state.colors_used(), 2);
    }

    #[test]
    fn edge_char_horizontal() {
        assert_eq!(edge_char(20, 0), '─');
    }

    #[test]
    fn edge_char_vertical() {
        assert_eq!(edge_char(0, 20), '│');
    }

    #[test]
    fn edge_char_diagonal_backslash() {
        assert_eq!(edge_char(8, 8), '\\');
    }

    #[test]
    fn edge_char_diagonal_slash() {
        assert_eq!(edge_char(8, -8), '/');
    }

    #[test]
    fn edge_char_nearly_horizontal_uses_dash() {
        assert_eq!(edge_char(30, 2), '─');
    }

    #[test]
    fn canvas_put_clamps_out_of_bounds() {
        let mut canvas = Canvas::new(10, 10);
        canvas.put(10, 10, 'X', Style::default()); // must not panic
    }

    #[test]
    fn canvas_into_lines_row_count() {
        let canvas = Canvas::new(20, 15);
        assert_eq!(canvas.into_lines().len(), 15);
    }

    /// Every node position must lie strictly inside the canvas with enough room
    /// for its `[X]` label (col − 1 through col + 1, same row).
    #[test]
    fn node_positions_fit_inside_canvas() {
        for (w, h) in [(80usize, 24usize), (60, 20), (40, 18), (120, 40)] {
            let positions = node_positions(w, h);
            for (i, (col, row)) in positions.iter().enumerate() {
                assert!(
                    *row < h,
                    "node {i} row {row} out of canvas height {h} for ({w}×{h})"
                );
                assert!(
                    col.saturating_sub(1) < w && col + 1 < w,
                    "node {i} col {col} label clips canvas width {w} for ({w}×{h})"
                );
            }
        }
    }

    /// All six nodes must be at distinct positions.
    #[test]
    fn node_positions_are_distinct() {
        let positions = node_positions(80, 24);
        let unique: HashSet<_> = positions.iter().copied().collect();
        assert_eq!(unique.len(), NODE_COUNT, "duplicate node positions: {positions:?}");
    }
}
