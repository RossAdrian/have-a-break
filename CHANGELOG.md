# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] — 2026-07-07

Initial release: a complete collection of nine bounded terminal study-break games.

### Added

- Main menu loop with `ratatui`/`crossterm` terminal setup and teardown, and dispatch to the nine games via a shared `Game` trait.
- **Snake** — tick-based game loop with wall bounding box; one life ends the run.
- **2048** — colour-coded tiles with win and game-over phases.
- **Graph Coloring** — dynamic terminal-adaptive layout; ends when the puzzle is solved.
- **Sliding Puzzle** — solvability check on generation and a move-count rating on completion.
- **Pattern Guesser** — six sequence types with a fixed number of guesses.
- **Typing Speed** — external sentence bank, one passage, one attempt.
- **Tower of Hanoi** — randomised start state with ASCII-art pegs, solved in minimum moves.
- **Estimation Challenge** — external question bank, scored on closeness to the correct answer.
- **Hangman** — one word, limited guesses, categorised word list.
- Shared `common` utilities: word list loading (`words.rs`, embedded via `include_str!()`), RNG setup (`rng.rs`).
- `README.md` documenting the project's origin, game list, design principles, and architecture.
- `Concept.md` capturing the original design conversation.
- QA passes (`QA/report_v1_1.md`, `QA/report_v1_2.md`) covering all nine games, with all High/Medium/Low findings from the first pass resolved before release.

### Fixed

- Hangman: `Q` is now treated as a letter guess during active play instead of being swallowed as a menu-exit key.
- Unified the exit-key convention across all games: `Esc` quits at any time during play, `Q` quits from end screens.
- Hangman's won-screen key hint and message formatting.
- Tower of Hanoi's win condition is now stated in the UI, and the redundant help line on its won screen was removed.
- Typing Speed's help text formatting normalized to match the rest of the collection.
- Snake's `spawn_food` no longer risks an infinite loop when the grid is full.

### Removed

- Unused `common/score.rs` scaffolding.
