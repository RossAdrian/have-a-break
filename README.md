# have-a-break

A terminal game collection for study breaks — nine short, bounded games that end naturally so you can actually get back to work.

## Origin

This project was an experiment in agentic coding with [Claude Code](https://claude.ai/code). The entire codebase was written by Claude Code under human supervision, with minor edits and testing by the human author.

The concept was designed collaboratively in a conversation with Claude (see [`Concept.md`](Concept.md)), starting from the question: *what makes Wordle, Spelling Bee, and Bastet good study-break games?* The answer — **bounded engagement** — drove both the game selection and the design constraints. The conversation settled on nine games and a Rust + ratatui tech stack, and Claude Code then implemented the full collection from scratch.

## Games

| Game | Type | Constraint |
|---|---|---|
| Snake | Spatial | One life — die once, back to menu |
| Graph Coloring | Logic | Solve the puzzle, session ends |
| Tower of Hanoi | Strategy | Solve it in minimum moves |
| Pattern Guesser | Logic / Math | Fixed number of guesses |
| Sliding Puzzle | Spatial | Solve the 8-puzzle, session ends |
| 2048 | Spatial | One game — board fills or you win |
| Estimation Challenge | Math | Fixed question set, scored on closeness |
| Typing Speed | Reflex / Typing | One passage, one attempt, done |
| Hangman | Word / Language | One word, limited guesses |

Every game returns to the main menu when it ends. There is no "play again" prompt — that is a deliberate design choice, not an omission.

## Design principles

The games follow three rules drawn from the concept:

- **One session = one attempt.** No infinite loops, no replay prompts.
- **Clear done state.** You can tell yourself "I'll play one game" and the game cooperates with that.
- **Cognitive domain shift.** The mix of spatial, logic, word, and reflex games means you can pick something that rests the circuits you've been using.

## Tech stack

- **Rust** — memory safety, single binary output, `include_str!()` for embedded word lists
- **[ratatui](https://ratatui.rs)** — TUI layout and widgets
- **[crossterm](https://github.com/crossterm-rs/crossterm)** — raw terminal input, color, cursor
- **[rand](https://docs.rs/rand)** — shuffling, random generation
- **[anyhow](https://docs.rs/anyhow)** — error handling

No async, no network, no database.

## Build and run

```bash
cargo build --release
cargo run
```

Navigate the menu with `↑`/`↓`, press `Enter` to launch a game, `Q` to quit.

## Project structure

```
src/
├── main.rs              ← menu loop and game dispatch
├── terminal.rs          ← ratatui/crossterm setup and teardown
├── games/
│   ├── mod.rs           ← Game trait and GameResult
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
    ├── words.rs
    ├── rng.rs
    └── score.rs
assets/
└── words.txt            ← embedded at compile time via include_str!()
```

Each game implements the `Game` trait (`fn run(&mut self, terminal: &mut Term) -> Result<GameResult>`), so adding a new game requires only a new module and a single dispatch arm in `main.rs`.

## Architecture note

The `GameResult` enum has two variants: `BackToMenu` (game ended normally) and `Quit` (player pressed the global quit key). The main loop acts on this — `BackToMenu` returns to the menu, `Quit` exits the process. Games never call `process::exit` directly.
