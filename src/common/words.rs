//! Embedded word list for games that need vocabulary.

#![allow(dead_code)]

/// Returns the word list embedded at compile time from `assets/words.txt`.
///
/// Each line is one word; callers should split on `'\n'` and filter empty lines.
pub fn word_list() -> &'static str {
    include_str!("../../assets/words.txt")
}

/// Returns the word list as an iterator of individual words.
pub fn words() -> impl Iterator<Item = &'static str> {
    word_list().lines().filter(|l| !l.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_list_is_not_empty() {
        assert!(words().count() > 0);
    }
}
