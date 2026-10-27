//! Queries, scoring and highlighting.
//!
//! A query is split into terms at whitespace; in the exact and fuzzy modes an item matches when
//! every term matches its text (fuzzy also tries the pinyin forms). Fuzzy terms match as a
//! subsequence; a contiguous match scores highest, then matches at word or syllable starts, then
//! tight matches. The regular-expression mode matches the whole query as one pattern. The mixed
//! mode (as in Maccy) returns exact matches when there are any, otherwise regular-expression
//! matches, otherwise fuzzy matches.

use regex::{Regex, RegexBuilder};

use crate::fold::fold;

/// Longest query kept, in bytes.
const MAX_QUERY_BYTES: usize = 256;
/// Compiled size limit of a query pattern.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

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

/// How a query matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// Characters in order, gaps allowed; pinyin included.
    #[default]
    Fuzzy,
    /// Substring of the text.
    Exact,
    /// A regular expression (case-insensitive) over the text.
    Regex,
    /// Exact matches if any, else regular-expression matches, else fuzzy matches.
    Mixed,
}

/// A parsed search query.
#[derive(Clone, Debug)]
pub struct Query {
    terms: Vec<Vec<char>>,
    mode: MatchMode,
    regex: Option<Regex>,
    regex_error: Option<String>,
}

impl Query {
    /// Parses user input: folded like the index and split at whitespace; in the regex and mixed
    /// modes the input is also compiled as a pattern.
    #[must_use]
    pub fn parse(input: &str, mode: MatchMode) -> Self {
        let folded = fold(input, MAX_QUERY_BYTES);
        let terms = folded
            .split(' ')
            .filter(|t| !t.is_empty())
            .map(|t| t.chars().collect())
            .collect();
        let (regex, regex_error) = match mode {
            MatchMode::Regex | MatchMode::Mixed if !input.trim().is_empty() => {
                match compile(input.trim()) {
                    Ok(regex) => (Some(regex), None),
                    Err(error) => (None, Some(error)),
                }
            }
            _ => (None, None),
        };
        Self {
            terms,
            mode,
            regex,
            regex_error,
        }
    }

    /// Whether the query has no terms (lists everything).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// The mode.
    #[must_use]
    pub const fn mode(&self) -> MatchMode {
        self.mode
    }

    /// Why the input is not a valid regular expression (regex mode), if it is not.
    #[must_use]
    pub fn regex_error(&self) -> Option<&str> {
        self.regex_error.as_deref()
    }

    pub(crate) fn terms(&self) -> &[Vec<char>] {
        &self.terms
    }

    pub(crate) fn regex(&self) -> Option<&Regex> {
        self.regex.as_ref()
    }
}

fn compile(pattern: &str) -> Result<Regex, String> {
    if pattern.len() > MAX_QUERY_BYTES {
        return Err(format!("a pattern has at most {MAX_QUERY_BYTES} bytes"));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|e| e.to_string())
}

/// Bit set of the characters in a text, for cheap rejection: a term can only match a text whose
/// mask contains the term's mask.
pub(crate) fn char_mask(chars: impl Iterator<Item = char>) -> u64 {
    chars.fold(0, |mask, c| mask | bit(c))
}

fn bit(c: char) -> u64 {
    1 << (u32::from(c.to_ascii_lowercase()) % 64)
}

/// Score of `term` in `hay` with substring or subsequence matching, or `None`. `hay` is folded
/// text or a pinyin form whose uppercase letters mark syllable starts; `term` is folded.
pub(crate) fn score(hay: &str, term: &[char], subsequence: bool) -> Option<u32> {
    if let Some(start) = find_contiguous(hay, term) {
        let at_boundary = is_boundary(hay, start);
        let penalty = u32::try_from(start)
            .unwrap_or(u32::MAX)
            .min(MAX_START_PENALTY);
        return Some(CONTIGUOUS_BASE + if at_boundary { BOUNDARY_BONUS * 2 } else { 0 } - penalty);
    }
    if subsequence {
        subsequence_score(hay, term)
    } else {
        None
    }
}

/// Score of a regular-expression match: earlier matches score higher.
pub(crate) fn regex_score(hay: &str, regex: &Regex) -> Option<u32> {
    let found = regex.find(hay)?;
    let start = u32::try_from(hay[..found.start()].chars().count()).unwrap_or(u32::MAX);
    Some(CONTIGUOUS_BASE - start.min(MAX_START_PENALTY))
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

/// Leftmost greedy subsequence match with bonuses for consecutive and boundary characters, in
/// one pass without allocating (this runs for every item on every keystroke).
fn subsequence_score(hay: &str, term: &[char]) -> Option<u32> {
    let mut wanted = term.iter().peekable();
    let mut total = 0u32;
    let mut previous_matched = false;
    let mut previous: Option<char> = None;
    for c in hay.chars() {
        let Some(&&want) = wanted.peek() else { break };
        let matched = lower(c) == want;
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

/// Character positions of a leftmost greedy subsequence match, comparing case-insensitively.
fn subsequence_positions(hay: &str, term: &[char]) -> Option<Vec<usize>> {
    let mut wanted = term.iter().peekable();
    let mut positions = Vec::with_capacity(term.len());
    for (at, c) in hay.chars().enumerate() {
        let Some(&&want) = wanted.peek() else { break };
        if lower(c) == want {
            positions.push(at);
            wanted.next();
        }
    }
    wanted.peek().is_none().then_some(positions)
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Character ranges `[start, end)` of `text` that match `query`, for highlighting. Pinyin-only
/// matches have no ranges in the original text.
#[must_use]
pub fn highlight(text: &str, query: &Query) -> Vec<(usize, usize)> {
    if let Some(regex) = query.regex().filter(|_| query.mode() == MatchMode::Regex) {
        return regex_ranges(text, regex);
    }
    let subsequence = matches!(query.mode(), MatchMode::Fuzzy | MatchMode::Mixed);
    let folded: Vec<char> = text.chars().map(lower).collect();
    let mut ranges: Vec<(usize, usize)> = query
        .terms()
        .iter()
        .flat_map(|term| term_ranges(&folded, term, subsequence))
        .collect();
    ranges.sort_unstable();
    merge(ranges)
}

fn term_ranges(text: &[char], term: &[char], subsequence: bool) -> Vec<(usize, usize)> {
    if term.is_empty() || term.len() > text.len() {
        return Vec::new();
    }
    if let Some(start) = text.windows(term.len()).position(|w| w == term) {
        return vec![(start, start + term.len())];
    }
    if !subsequence {
        return Vec::new();
    }
    let joined: String = text.iter().collect();
    subsequence_positions(&joined, term)
        .map(|positions| positions.into_iter().map(|p| (p, p + 1)).collect())
        .unwrap_or_default()
}

fn regex_ranges(text: &str, regex: &Regex) -> Vec<(usize, usize)> {
    regex
        .find_iter(text)
        .filter(|m| !m.as_str().is_empty())
        .map(|m| {
            (
                text[..m.start()].chars().count(),
                text[..m.end()].chars().count(),
            )
        })
        .collect()
}

/// Joins touching or overlapping ranges.
fn merge(ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ranges.into_iter().fold(
        Vec::new(),
        |mut merged: Vec<(usize, usize)>, (start, end)| {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
            merged
        },
    )
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
        assert!(query.regex().is_none());
    }

    #[test]
    fn regex_queries_compile_or_report_why_not() {
        assert!(Query::parse(r"\d{6}", MatchMode::Regex).regex().is_some());
        assert!(Query::parse("(", MatchMode::Regex).regex_error().is_some());
        assert!(Query::parse("(", MatchMode::Mixed).regex().is_none());
        assert!(
            Query::parse(&"a".repeat(300), MatchMode::Regex)
                .regex_error()
                .is_some()
        );
    }

    #[test]
    fn contiguous_beats_scattered_and_start_beats_middle() {
        let contiguous = score("hello from supercy", &term("from"), true).unwrap();
        let scattered = score("f r o m", &term("from"), true).unwrap();
        let at_start = score("from here", &term("from"), true).unwrap();

        assert!(contiguous > scattered);
        assert!(at_start > contiguous);
    }

    #[test]
    fn substring_only_scoring_needs_a_substring() {
        assert!(score("hello from supercy", &term("hfs"), false).is_none());
        assert!(score("hello from supercy", &term("hfs"), true).is_some());
        assert!(score("hello", &term("hello world"), true).is_none());
    }

    #[test]
    fn syllable_starts_score_as_boundaries() {
        let initials = score("HuiYiJiYao", &term("hyjy"), true).unwrap();
        let inner = score("huiyijiyao", &term("uiia"), true).unwrap();

        assert!(initials > inner);
    }

    #[test]
    fn regex_scores_earlier_matches_higher() {
        let regex = compile("demo").unwrap();

        assert!(regex_score("demo x", &regex) > regex_score("x demo", &regex));
        assert_eq!(regex_score("none", &regex), None);
    }

    #[test]
    fn mask_rejects_missing_characters() {
        let hay = char_mask("demo".chars());

        assert_eq!(hay & char_mask("med".chars()), char_mask("med".chars()));
        assert_ne!(hay & char_mask("x".chars()), char_mask("x".chars()));
    }

    #[test]
    fn highlight_marks_substrings_subsequences_and_regex_matches() {
        let fuzzy = highlight(
            "Hello from Supercy",
            &Query::parse("from hfs", MatchMode::Fuzzy),
        );
        let exact = highlight("Hello from Supercy", &Query::parse("hfs", MatchMode::Exact));
        let regex = highlight("会议 2026 纪要 42", &Query::parse(r"\d+", MatchMode::Regex));
        let chinese = highlight("会议纪要", &Query::parse("纪要", MatchMode::Fuzzy));

        assert_eq!(fuzzy, vec![(0, 1), (6, 10), (11, 12)]);
        assert!(exact.is_empty());
        assert_eq!(regex, vec![(3, 7), (11, 13)]);
        assert_eq!(chinese, vec![(2, 4)]);
    }
}
