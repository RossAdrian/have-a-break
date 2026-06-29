//! Shared RNG setup for games.

#![allow(dead_code)]

use rand::{rngs::ThreadRng, thread_rng};

/// Returns a thread-local RNG seeded from system entropy.
pub fn new_rng() -> ThreadRng {
    thread_rng()
}
