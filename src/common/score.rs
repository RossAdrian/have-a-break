//! Score tracking for games.

#![allow(dead_code)]

/// Tracks accumulated points within a single game session.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    /// Total points earned this session.
    pub points: u32,
}

impl Score {
    /// Creates a zero-point score.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `n` points and returns the new total.
    pub fn add(&mut self, n: u32) -> u32 {
        self.points += n;
        self.points
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_score_is_zero() {
        assert_eq!(Score::new().points, 0);
    }

    #[test]
    fn add_accumulates() {
        let mut s = Score::new();
        s.add(10);
        s.add(5);
        assert_eq!(s.points, 15);
    }
}
