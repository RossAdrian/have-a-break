# QA Report v1 — have-a-break

**Date:** 2026-07-02  
**Reviewer:** Claude Sonnet 4.6  
**Scope:** Full source review of all 9 games, common modules, assets, and main entry point.  
**Test suite:** 158 tests — all pass.

---

## Overall Health

The project is in solid shape. The architecture is clean, every game follows the `Game` trait contract, terminal teardown is handled correctly (including panic hook), and the test coverage is thorough. No crashes or data-loss bugs were found. The issues below are mostly polish and UX consistency problems.

---

## 1. Exit Key Inconsistency (Priority: High)

This is the main UX problem. The codebase has an implicit convention — **Esc during play, Q on end-screens** — but it is not applied uniformly.

### 1.1 Current state per game

| Game | Phase | Key shown in UI | Also silently works |
|------|-------|-----------------|---------------------|
| Main menu | Always | Q | — |
| Snake | Playing | `Esc` | `Q`/`q` (undiscoverable) |
| Snake | Game over | `Q` | `Esc` |
| 2048 | Playing | `Esc` | *(only Esc, Q not accepted)* |
| 2048 | Win overlay | `Q` | `Esc` |
| 2048 | Game over | `Q` | `Esc` |
| Graph Coloring | Playing/solving | `Esc` | — |
| Graph Coloring | Solved | `Q` | `Esc` |
| Sliding Puzzle | Playing | `Esc` | — |
| Sliding Puzzle | Solved | `Q` | `Esc` |
| Pattern Guesser | Answering / result | `Esc` | — |
| Pattern Guesser | Summary | `Q` | `Esc` |
| Tower of Hanoi | Playing | `Esc` | — |
| Tower of Hanoi | Won | `Q` | `Esc` |
| Estimation | Answering / result | `Esc` | — |
| Estimation | Summary | `Q` | `Esc` |
| Typing Speed | Typing | `Esc` | — |
| Typing Speed | Done | `Q` | `Esc` |
| Hangman | Playing | `Esc` | — (Q conflicts with letter guessing) |
| Hangman | **Won** | **`Esc`** ← **wrong** | `Q`/`q` |
| Hangman | Lost | `Q` | `Esc` |

### 1.2 Specific bugs

**Bug 1 — Hangman won screen uses the wrong key (`hangman.rs:222`)**  
All other end-game screens display `Q — return to menu`. Hangman's won screen uniquely shows `"Press Esc to return to menu."`. The code actually accepts both `Q`/`q` and `Esc` when `game_over` is true, so the handler is fine — only the message is wrong. The lost screen correctly shows Q.

```rust
// hangman.rs:222 — status message when won
Paragraph::new("You won!  Press Esc to return to menu.")
// Should be:
Paragraph::new("You won!  Press Q to return to menu.")
```

**Bug 2 — Snake: Q works during play but is not shown (`snake.rs:317`)**  
During `Phase::Playing`, the help text reads `"↑↓←→ steer  •  Esc — menu"`, but the key handler also accepts `Q`/`q`. Either remove `Q`/`q` from the Playing phase handler (to match the hint) or add it to the hint. Every other game with a playing phase only handles Esc to exit during play (except 2048 below which is consistent). Making Snake match the others would mean removing the Q/q arm from the Playing handler.

**Issue 3 — 2048: Q does not work during play (`game_2048.rs:414`)**  
In `Phase::Playing`, only `Esc` is handled. Snake accepts both during play. These should behave the same. Since Q is not a game key in 2048 (unlike Hangman where Q could be a letter guess), there is no reason to exclude it. The hint correctly shows only Esc, so either add silent Q support or leave it as-is and note the inconsistency with Snake.

### 1.3 Recommended convention

> **Esc** — exits at any time, from any phase, in every game. Always shown during play.  
> **Q** — shortcut on end-screens (win / loss / summary) only. Always shown on those screens.  
> During active gameplay where letters are typed (Hangman), only Esc is available.

Applying this consistently requires:
- Fix Hangman won message (Bug 1 above).
- Decide whether Snake's Playing phase should silently accept Q (it does now) and make 2048 match.

---

## 2. Per-Game Issues

### 2.1 Tower of Hanoi — Non-classic starting state

**File:** `hanoi.rs:45–75`

The game starts with disks randomly distributed across all three pegs, which is intentional to make each session feel different. However, the win condition is "all disks on *any* peg", not "all disks on peg C". This departs from the classic Tower of Hanoi formulation where you move everything from peg A to peg C. Players familiar with the puzzle may find the goal ambiguous.

The status message says `"Solved in {} moves!"` but never specifies the target peg. A short note like `"Goal: move all disks to one peg"` would help orientation.

**Suggestion:** Either keep random start and clarify the win condition in the UI, or add an option to start with all disks on peg A with target peg C.

### 2.2 Typing Speed — Help text formatting inconsistency

**File:** `typing.rs:316–319`

The help text at the bottom of the screen uses different formatting from every other game:

```
// typing.rs — non-standard:
"Q  return to menu"
"Backspace  delete  •  Esc  quit to menu"

// All other games use:
"Q — return to menu"
"↑↓←→ steer  •  Esc — menu"
```

Gaps are used instead of ` — ` dashes to separate key names from their actions. Minor but noticeable.

### 2.3 Hangman — Won/lost messages are inconsistent with each other

**File:** `hangman.rs:222–239`

Won: `"You won!  Press Esc to return to menu."`  (trailing period, uses Esc)  
Lost: `"Game over!  The word was: {word}   Press Q to return."` (trailing period, three spaces before "Press", uses Q)

Both messages have trailing periods which no other game uses. The three-space gap before "Press Q" in the lost message looks unintentional. Both should use Q for consistency (see Section 1.2 Bug 1).

### 2.4 Sliding Puzzle — Arrow key direction semantics

**File:** `sliding.rs:69–86`

Pressing ↑ moves the blank tile upward (the tile above slides down into the blank). This is the "move the blank" convention. The alternative — pressing ↑ moves a tile upward — is equally common. The help text `"↑↓←→ — slide"` doesn't clarify which thing is sliding. Given that most mobile/web sliding puzzle games use "move the blank" convention, this is likely correct but worth noting in the help as `"↑↓←→ — move blank"` or similar.

---

## 3. Code Quality Issues

### 3.1 `score.rs` is unused scaffolding

**File:** `src/common/score.rs`

The `Score` struct is defined and tested but is not imported or used by any game. All games implement their own scoring inline. The file has `#![allow(dead_code)]` suppressing the compiler warning.

This is harmless but represents dead code in the shipped binary. Either wire it up or remove it and its module declaration in `common/mod.rs`.

### 3.2 `spawn_food` in Snake: theoretical infinite loop

**File:** `snake.rs:113–125`

```rust
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
```

If the snake ever fills all 400 cells, this loops forever. In practice the player would need a score of ~3,970 to fill the grid, which is virtually impossible in a 3-minute session. Still, a guard `if body_set.len() >= (GRID_SIZE * GRID_SIZE) as usize { return; }` would make it provably safe.

### 3.3 Redundant help rendering in Tower of Hanoi won state

**File:** `hanoi.rs:166–206`

When won, the status line (`v[5]`) shows `"Solved in N moves!  Press Q to return."` *and* the help line (`v[6]`) shows `"Q — return to menu"`. Both lines say essentially the same thing. The help line is superfluous on the won screen.

---

## 4. Design / UX Suggestions

### 4.1 2048 — Q during active play

As noted in Section 1.2, 2048 is the only game that doesn't accept Q/q to quit during the active play phase (arrow keys only). Adding it would make the behavior consistent with Snake without any functional downside.

### 4.2 Pattern Guesser — No feedback on which pattern type was used

After each round's result is shown, the player sees whether they were right or wrong and what the correct answer was, but not the name of the pattern (e.g. "Geometric progression"). Showing the pattern type in the `ShowingResult` phase would make the game more educational.

### 4.3 Hangman — Single hardcoded category

**File:** `hangman.rs:149`

```rust
Paragraph::new("Category: Common English Word")
```

The category is always the same. Multiple categories (animals, countries, technology terms) with different word lists would add variety and make the category hint genuinely informative.

### 4.4 Graph Coloring — Optimal threshold might not be accurate

**File:** `graph_coloring.rs:45`

```rust
const HINT_THRESHOLD: usize = 3;
```

The game congratulates the player with "Optimal!" if they use ≤3 colors. However, the actual chromatic number of the generated graph varies — some graphs may have a chromatic number of 2 (bipartite). "Optimal" should ideally mean "you used the minimum possible colors." This would require computing the chromatic number during generation, which is NP-hard in general but trivially feasible for 6-node graphs.

---

## 5. Summary Table

| Severity | # | Description |
|----------|---|-------------|
| High | 1 | Hangman won screen shows `Esc` instead of `Q` |
| Medium | 2 | Snake accepts Q during play but doesn't show it |
| Medium | 3 | 2048 rejects Q during play (unlike Snake) |
| Medium | 4 | Hanoi win condition and start state not explained |
| Low | 5 | Typing Speed help text formatting inconsistent |
| Low | 6 | Hangman won/lost messages have trailing periods and inconsistent spacing |
| Low | 7 | Hanoi won: duplicate "press Q" in status + help lines |
| Low | 8 | `score.rs` is dead code (unused by any game) |
| Low | 9 | Snake `spawn_food` theoretical infinite loop |
| Suggestion | 10 | 2048: add Q as synonym for Esc during play |
| Suggestion | 11 | Pattern Guesser: show pattern type name in result phase |
| Suggestion | 12 | Hangman: multiple word categories |
| Suggestion | 13 | Graph Coloring: compute true chromatic number for "Optimal!" hint |
| Suggestion | 14 | Sliding Puzzle: clarify arrow key semantics in help text |

**Blocking before release:** Issue #1 (wrong key shown in Hangman won screen) is a clear bug and easy to fix. Issues #2 and #3 should be resolved to establish a coherent exit-key convention.
