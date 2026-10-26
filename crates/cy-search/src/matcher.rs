//! Queries and scoring.
//!
//! A query is split into terms at whitespace; an item matches when every term matches its text
//! or one of its pinyin forms. Fuzzy terms match as a subsequence; a contiguous match scores
//! highest, then matches at word or syllable starts, then tight matches.

use crate::fold::fold;

/// Longest query kept, in bytes.
const MAX_QUERY_BYTES: usize = 256;

/// Score for a contiguous (substring) match, before position adjustments.
const CONTIGUOUS_BASE: u32 = 1_000;
/// Score per matched character in a subsequence match.
const PER_CHAR: u32 = 16;
/// Extra score when a matched character follows the previous match directly.
const CONSECUTIVE_BONUS: u32 = 12;
/// Extra score when a matched character starts a word or a pinyin syllable.
const BOUNDARY_BONUS: u32 = 10;
/// Highest penalty for where the match starts.
const MAX_START_PENALTY: u32 = 50;

/// How terms match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// Characters in order, gaps allowed; pinyin included.
    #[default]
    Fuzzy,
    /// Substring of the text only.
    Exact,
}

/// A parsed search query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    terms: Vec<Vec<char>>,
    mode: MatchMode,
}

impl Query {
    /// Parses user input: folded like the index, split at whitespace.
    #[must_use]
    pub fn parse(input: &str, mode: MatchMode) -> Self {
        let folded = fold(input, MAX_QUERY_BYTES);
        let terms = folded
            .split(' ')
            .filter(|t| !t.is_empty())
            .map(|t| t.chars().collect())
            .collect();
        Self { terms, mode }
    }

    /// Whether the query has no terms (lists everything).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub(crate) fn terms(&self) -> &[Vec<char>] {
        &self.terms
    }

    pub(crate) const fn mode(&self) -> MatchMode {
        self.mode
    }
}

/// Bit set of the characters in a text, for cheap rejection: a term can only match a text whose
/// mask contains the term's mask.
pub(crate) fn char_mask(chars: impl Iterator<Item = char>) -> u64 {
    chars.fold(0, |mask, c| mask | bit(c))
}

fn bit(c: char) -> u64 {
    1 << (u32::from(c.to_ascii_lowercase()) % 64)
}

/// Score of `term` in `hay`, or `None`. `hay` is folded text or a pinyin form whose uppercase
/// letters mark syllable starts; `term` is folded.
pub(crate) fn score(hay: &str, term: &[char], mode: MatchMode) -> Option<u32> {
    if let Some(start) = find_contiguous(hay, term) {
        let at_boundary = is_boundary(hay, start);
        let penalty = u32::try_from(start)
            .unwrap_or(u32::MAX)
            .min(MAX_START_PENALTY);
        return Some(CONTIGUOUS_BASE + if at_boundary { BOUNDARY_BONUS * 2 } else { 0 } - penalty);
    }
    match mode {
        MatchMode::Exact => None,
        MatchMode::Fuzzy => subsequence_score(hay, term),
    }
}

/// Character position of the first case-insensitive occurrence of `term` in `hay`.
fn find_contiguous(hay: &str, term: &[char]) -> Option<usize> {
    let first = *term.first()?;
    hay.char_indices()
        .enumerate()
        .find_map(|(position, (byte, c))| {
            (c.to_ascii_lowercase() == first && starts_with_term(&hay[byte..], term))
                .then_some(position)
        })
}

fn starts_with_term(hay: &str, term: &[char]) -> bool {
    let mut chars = hay.chars();
    term.iter()
        .all(|t| chars.next().is_some_and(|h| h.to_ascii_lowercase() == *t))
}

fn is_boundary(hay: &str, char_index: usize) -> bool {
    if char_index == 0 {
        return true;
    }
    let mut chars = hay.chars().skip(char_index - 1);
    let previous = chars.next();
    let current = chars.next();
    current.is_some_and(|c| c.is_ascii_uppercase())
        || previous.is_some_and(|p| !p.is_alphanumeric())
}

/// Leftmost greedy subsequence match with bonuses for consecutive and boundary characters.
fn subsequence_score(hay: &str, term: &[char]) -> Option<u32> {
    let mut wanted = term.iter().peekable();
    let mut total = 0u32;
    let mut previous_matched = false;
    let mut previous: Option<char> = None;
    for c in hay.chars() {
        let Some(&&want) = wanted.peek() else { break };
        let matched = c.to_ascii_lowercase() == want;
        if matched {
            let boundary = c.is_ascii_uppercase() || previous.is_none_or(|p| !p.is_alphanumeric());
            total += PER_CHAR
                + if previous_matched {
                    CONSECUTIVE_BONUS
                } else {
                    0
                }
                + if boundary { BOUNDARY_BONUS } else { 0 };
            wanted.next();
        }
        previous_matched = matched;
        previous = Some(c);
    }
    wanted.peek().is_none().then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn parse_folds_and_splits() {
        let query = Query::parse("  Demo   LINK ", MatchMode::Fuzzy);

        assert_eq!(query.terms(), &[term("demo"), term("link")]);
        assert!(Query::parse("   ", MatchMode::Exact).is_empty());
        assert_eq!(query.mode(), MatchMode::Fuzzy);
    }

    #[test]
    fn contiguous_beats_scattered_and_start_beats_middle() {
        let contiguous = score("hello from supercy", &term("from"), MatchMode::Fuzzy).unwrap();
        let scattered = score("f r o m", &term("from"), MatchMode::Fuzzy).unwrap();
        let at_start = score("from here", &term("from"), MatchMode::Fuzzy).unwrap();

        assert!(contiguous > scattered);
        assert!(at_start > contiguous);
    }

    #[test]
    fn exact_mode_needs_a_substring() {
        assert!(score("hello from supercy", &term("hfs"), MatchMode::Exact).is_none());
        assert!(score("hello from supercy", &term("hfs"), MatchMode::Fuzzy).is_some());
        assert!(score("hello", &term("hello world"), MatchMode::Fuzzy).is_none());
    }

    #[test]
    fn syllable_starts_score_as_boundaries() {
        let initials = score("HuiYiJiYao", &term("hyjy"), MatchMode::Fuzzy).unwrap();
        let inner = score("huiyijiyao", &term("uiia"), MatchMode::Fuzzy).unwrap();

        assert!(initials > inner);
    }

    #[test]
    fn mask_rejects_missing_characters() {
        let hay = char_mask("demo".chars());

        assert_eq!(hay & char_mask("med".chars()), char_mask("med".chars()));
        assert_ne!(hay & char_mask("x".chars()), char_mask("x".chars()));
    }
}
