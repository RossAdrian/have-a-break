# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

`have-a-break` is a Rust terminal game collection designed as short, bounded study-break games. The design principle: every game has a natural stopping point (attempt-boxed, completion-boxed, or time-boxed) so the player can genuinely stop after one session.

Planned games: Snake, Graph Coloring, Tower of Hanoi, Pattern Guesser, Sliding Puzzle, 2048, Estimation Challenge, Typing Speed, Hangman with categories.

## Commands

```bash
cargo build          # debug build
cargo build --release
cargo run            # run the app
cargo test           # run all tests
cargo test <name>    # run a single test by name/module
cargo clippy         # lint
```

## Planned architecture

Each game is a module under `src/games/` and implements a common trait — roughly `fn run(terminal: &mut Terminal) -> GameResult` — so `main.rs` dispatches to whichever game the player selects and gets back a result.

```
src/
├── main.rs            ← menu loop, game dispatch
├── terminal.rs        ← ratatui/crossterm setup & teardown
├── games/
│   ├── mod.rs
│   ├── snake.rs
│   ├── graph_coloring.rs
│   ├── hanoi.rs
│   ├── pattern.rs
│   ├── sliding.rs
│   ├── game_2048.rs
│   ├── estimation.rs
│   ├── typing.rs
│   └── hangman.rs
└── common/
    ├── words.rs       ← word list loader
    ├── rng.rs         ← shared RNG setup
    └── score.rs       ← score tracking
assets/
└── words.txt          ← embedded at compile time via include_str!()
```

## Key crates (to be added)

| Crate | Purpose |
|---|---|
| `ratatui` | TUI layout, widgets, rendering |
| `crossterm` | Raw input events, cursor, color (ratatui backend) |
| `rand` | RNG, shuffling, distributions |
| `petgraph` | Graph structure for Graph Coloring (optional) |

Word lists are embedded directly into the binary using `include_str!()` — no external file dependency at runtime.

## Design constraints

- No async, no network, no database — intentionally simple
- Each game session should complete in under 3 minutes
- No "play again" prompt; the game ends and returns to the main menu
- Word lists come from `assets/words.txt` or `/usr/share/dict/words`, embedded at compile time

## Conventions

- Write unit tests for every change and make sure they pass
- Prioterize readability over cleverness
- Maintain all over the project clean and complete rustdoc docstrings
- Keep the overall architecture extensible to add at any time new games
