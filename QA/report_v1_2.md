# QA Report v1.2 — have-a-break

**Date:** 2026-07-07
**Reviewer:** Claude Sonnet 5
**Scope:** Full source review of all 9 games, common modules, assets, `main.rs`, and `terminal.rs`. Follow-up to `QA/report_v1_1.md`.
**Test suite:** 157 tests — all pass. `cargo build` and `cargo clippy --all-targets -- -D warnings` are both clean.

---

## 1. Status of report v1.1 issues #1–9

All nine High/Medium/Low issues from `report_v1_1.md` have been fixed and verified against the current source.

| # | Sev. | Issue | Status | Evidence |
|---|------|-------|--------|----------|
| 1 | High | Hangman won screen showed `Esc` instead of `Q` | **Fixed** | `hangman.rs:222` now reads `"You won!  Press Q to return to menu"` |
| 2 | Medium | Snake accepted `Q` during play but didn't show it | **Fixed** | `snake.rs:317` — Playing-phase handler now matches only `KeyCode::Esc`; the `Q`/`q` arm was removed |
| 3 | Medium | 2048 rejected `Q` during play (unlike Snake) | **Resolved as a side effect of #2** | Both games are now Esc-only during play (`game_2048.rs:415`), so the inconsistency is gone. Note this took the opposite direction from Suggestion #10 below — see that entry |
| 4 | Medium | Hanoi win condition/start state unexplained | **Fixed** | `hanoi.rs:163–169` renders `"Goal: move all disks onto a single peg"` during play |
| 5 | Low | Typing Speed help text formatting inconsistent | **Fixed** | `typing.rs:316–320` now uses `"Q — return to menu"` / `"Backspace — delete  •  Esc — menu"` |
| 6 | Low | Hangman won/lost messages had trailing periods, inconsistent spacing | **Fixed** | Both messages now end without a period and use single-space separation (`hangman.rs:222,231`) |
| 7 | Low | Hanoi won screen duplicated "press Q" in status + help lines | **Fixed** | `hanoi.rs:206–214` — help line is now suppressed (`if !won`) on the won screen |
| 8 | Low | `score.rs` was dead code | **Fixed** | `src/common/score.rs` deleted; `mod score;` removed from `src/common/mod.rs` |
| 9 | Low | Snake `spawn_food` theoretical infinite loop | **Fixed** | `snake.rs:114–119` returns early when the grid is full; covered by `spawn_food_returns_when_grid_is_full` |

No regressions found in the surrounding code for any of these fixes.

---

## 2. New Findings

### 2.1 Esc does nothing at the main menu (Medium)

**File:** `src/main.rs:106–124`

Every game now honors the convention "Esc exits at any time, from any phase" (as stated in report v1.1 §1.3). The main menu is the one place that convention doesn't apply: its key handler only recognizes `Up`, `Down`, `Enter`, and `Q`/`q`.

```rust
match key.code {
    KeyCode::Up => { ... }
    KeyCode::Down => { ... }
    KeyCode::Enter => { ... }
    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
    _ => {}
}
```

A player who has just backed out of a game with Esc and reflexively presses it again at the menu will find it does nothing. Either add an `Esc` arm that quits (matching every game's exit key) or leave it and document the exception — but as it stands the convention silently stops at the menu boundary.

### 2.2 "Return to menu" phrasing is inconsistent across every game (Medium)

Report v1.1 fixed *which key* does what; the *wording* around those keys was never unified and differs game to game:

| Phrasing (Esc) | Used in |
|---|---|
| `"Esc — menu"` | `game_2048.rs:353`, `snake.rs:281`, `hanoi.rs:210`, `typing.rs:319`, `sliding.rs:228` |
| `"Esc to return to menu"` | `hangman.rs:241`, `estimation.rs:254,294` |
| `"Esc — back to menu"` | `graph_coloring.rs:458` |

| Phrasing (Q) | Used in |
|---|---|
| `"Q — menu"` | `game_2048.rs:358`, `snake.rs:285` |
| `"Q — return to menu"` | `typing.rs:317`, `sliding.rs:226` |
| `"Q — back to menu"` | `graph_coloring.rs:445` |
| `"Press Q to return to menu"` | `hangman.rs:222,231` |
| `"Press Q to return to the menu"` | `estimation.rs:383`, `pattern.rs:434` |

Five distinct phrasings for the same action ("go back to the menu"). Standardizing on one form per key (e.g. `"Esc — menu"` / `"Q — menu"` for the terse in-HUD hints, or `"Press Q to return to menu"` for full-sentence end screens) would finish the consistency work report v1.1 started.

### 2.3 `GameResult::Quit` is never constructed (Low)

**File:** `src/games/mod.rs:19–24`, `src/main.rs:116–121`

```rust
pub enum GameResult {
    Quit,
    BackToMenu,
}
...
let result = run_game(selected, &mut handle.terminal)?;
if result == GameResult::Quit {
    return Ok(());
}
```

Every one of the 9 games returns only `GameResult::BackToMenu` (verified — no `GameResult::Quit` construction exists anywhere under `src/games/`). The app's only quit path is the main menu's own `Char('q')` handler, which returns directly and bypasses `GameResult` entirely. `GameResult::Quit` is unreachable dead code baked into the trait contract described in `CLAUDE.md`. Either wire up a real use for it (e.g. let a game itself quit the app on some key) or remove the variant and simplify `main.rs`'s dispatch to always loop back to the menu.

### 2.4 Stale `#![allow(dead_code)]` on `rng.rs` and `words.rs` (Low)

**Files:** `src/common/rng.rs:3`, `src/common/words.rs:3`

Both modules carry a module-level `#![allow(dead_code)]`, presumably left over from when they were scaffolded ahead of any game using them (same vintage as the `score.rs` issue fixed in v1.1, #8). Every function in both files is now called from game code. Removing both attributes and rebuilding (`cargo build`) produces zero warnings — confirming the suppressions no longer guard anything.

### 2.5 Estimation Challenge: malformed numeric input silently scores 0 (Low)

**File:** `src/games/estimation.rs:433–436, 128–130`

```rust
KeyCode::Char(c) if is_answering && (c.is_ascii_digit() || c == '.') => {
    state.input.push(c);
}
```

The input filter allows digits and `.` but doesn't prevent multiple decimal points, so a player can type e.g. `"1.2.3"`. `confirm()` then does `submitted.parse::<f64>().map_or(0, ...)`, which silently yields 0 points — visually indistinguishable on the result screen from "a validly-parsed but very wrong guess." A player who fat-fingers an extra `.` gets no signal that their input was rejected outright rather than just scored harshly.

---

## 3. Suggestions carried over from report v1.1

Per request, all five Suggestion-severity items from `report_v1_1.md` §5 are carried forward unchanged. None have been acted on.

| # | Suggestion | Still applicable? |
|---|---|---|
| 10 | 2048: add Q as synonym for Esc during play | **Superseded.** Issues #2/#3 were resolved in the opposite direction — Snake was made Esc-only to match 2048, not the other way around (see §1 above). Acting on this suggestion now would reopen the inconsistency it was meant to close. Recommend dropping it in favor of the resolved convention (Esc-only during play, Q on end-screens). |
| 11 | Pattern Guesser: show pattern type name in result phase (`pattern.rs`) | Yes — unchanged, still a reasonable educational improvement |
| 12 | Hangman: multiple word categories (`hangman.rs:149`) | Yes — unchanged, still hardcoded to `"Category: Common English Word"` |
| 13 | Graph Coloring: compute true chromatic number for "Optimal!" hint (`graph_coloring.rs:45`) | Yes — unchanged, `HINT_THRESHOLD` is still a fixed constant of 3 |
| 14 | Sliding Puzzle: clarify arrow key semantics in help text (`sliding.rs:228`) | Yes — unchanged, help text still just says `"↑↓←→ — slide"` |

New suggestion from this pass:

| # | Suggestion |
|---|---|
| 15 | Main menu selection doesn't wrap (`main.rs:107–114`): pressing Up at the top item or Down at the bottom item does nothing, unlike many terminal menus that wrap around. Minor navigation polish, not a bug. |

---

## 4. Summary Table

| Severity | # | Description |
|----------|---|-------------|
| Medium | 1 | Esc does nothing at the main menu, breaking the exit-key convention every game now follows |
| Medium | 2 | "Return to menu" wording differs across every game (5 phrasings for the same action) |
| Low | 3 | `GameResult::Quit` is never constructed — dead code in the core `Game` trait contract |
| Low | 4 | `#![allow(dead_code)]` on `common/rng.rs` and `common/words.rs` is stale |
| Low | 5 | Estimation Challenge: malformed numeric input (e.g. `"1.2.3"`) silently scores 0 with no distinct feedback |
| Suggestion | 6 | Main menu selection doesn't wrap top/bottom |
| Suggestion (carried, superseded) | 7 | 2048: add Q during play — contradicted by the chosen fix for v1.1 #2/#3; recommend dropping |
| Suggestion (carried) | 8 | Pattern Guesser: show pattern type name in result phase |
| Suggestion (carried) | 9 | Hangman: multiple word categories |
| Suggestion (carried) | 10 | Graph Coloring: compute true chromatic number for "Optimal!" hint |
| Suggestion (carried) | 11 | Sliding Puzzle: clarify arrow key semantics in help text |

**Recommended next pass:** resolve #1 and #2 together — they're both about finishing the exit-key consistency work from v1.1 (behavior is now uniform; wording and menu-level coverage are the remaining gaps).
