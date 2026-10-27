//! User-supplied regular expressions: content matching one of them is not kept.

use regex::{RegexSet, RegexSetBuilder};

/// Longest pattern accepted, in bytes.
pub const MAX_PATTERN_LEN: usize = 1_000;
/// Most patterns accepted.
pub const MAX_PATTERNS: usize = 100;
/// Compiled size limit, so a pattern cannot use much memory.
const SIZE_LIMIT: usize = 1 << 20;

/// Why a pattern was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("pattern {index} is not a valid regular expression: {message}")]
pub struct PatternError {
    /// Position of the pattern in the list (0-based).
    pub index: usize,
    /// What is wrong, from the regular-expression parser.
    pub message: String,
}

/// A compiled list of patterns. Equal when the sources are equal.
#[derive(Clone, Debug, Default)]
pub struct PatternSet {
    sources: Vec<String>,
    set: Option<RegexSet>,
}

impl PartialEq for PatternSet {
    fn eq(&self, other: &Self) -> bool {
        self.sources == other.sources
    }
}

impl Eq for PatternSet {}

impl PatternSet {
    /// Compiles `patterns`. Matching ignores case; `.` does not match line breaks.
    ///
    /// # Errors
    ///
    /// The first pattern that is too long, too large or not valid; or too many patterns.
    pub fn new(patterns: &[String]) -> Result<Self, PatternError> {
        if patterns.len() > MAX_PATTERNS {
            return Err(PatternError {
                index: MAX_PATTERNS,
                message: format!("at most {MAX_PATTERNS} patterns"),
            });
        }
        for (index, pattern) in patterns.iter().enumerate() {
            validate_pattern(pattern).map_err(|message| PatternError { index, message })?;
        }
        if patterns.is_empty() {
            return Ok(Self::default());
        }
        let set = builder(patterns).build().map_err(|e| PatternError {
            index: 0,
            message: e.to_string(),
        })?;
        Ok(Self {
            sources: patterns.to_vec(),
            set: Some(set),
        })
    }

    /// Whether any pattern matches somewhere in `text`.
    #[must_use]
    pub fn is_match(&self, text: &str) -> bool {
        self.set.as_ref().is_some_and(|set| set.is_match(text))
    }

    /// The patterns as given.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }
}

fn builder(patterns: &[String]) -> RegexSetBuilder {
    let mut builder = RegexSetBuilder::new(patterns);
    builder
        .case_insensitive(true)
        .size_limit(SIZE_LIMIT)
        .dfa_size_limit(SIZE_LIMIT);
    builder
}

/// Checks one pattern; the error text says what is wrong.
///
/// # Errors
///
/// The parser's message, or a length message.
pub fn validate_pattern(pattern: &str) -> Result<(), String> {
    if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
        return Err(format!("a pattern has 1 to {MAX_PATTERN_LEN} bytes"));
    }
    builder(&[pattern.to_owned()])
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(patterns: &[&str]) -> PatternSet {
        PatternSet::new(&patterns.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn matches_any_pattern_ignoring_case() {
        let patterns = set(&["^otp:", r"\bDEMO-\d+\b"]);

        assert!(patterns.is_match("OTP: 123456"));
        assert!(patterns.is_match("ticket demo-42 done"));
        assert!(!patterns.is_match("Hello from Supercy"));
        assert!(!PatternSet::default().is_match("anything"));
    }

    #[test]
    fn invalid_long_empty_and_too_many_patterns_are_refused_with_their_position() {
        let invalid = PatternSet::new(&["ok".into(), "(".into()]);
        let long = validate_pattern(&"a".repeat(MAX_PATTERN_LEN + 1));
        let many = PatternSet::new(&vec!["a".to_owned(); MAX_PATTERNS + 1]);

        assert_eq!(invalid.map(|_| ()).map_err(|e| e.index), Err(1));
        assert!(long.is_err());
        assert!(validate_pattern("").is_err());
        assert!(many.is_err());
    }

    #[test]
    fn sets_compare_by_their_sources() {
        assert_eq!(set(&["a"]), set(&["a"]));
        assert_ne!(set(&["a"]), set(&["b"]));
        assert_eq!(set(&["a"]).sources(), ["a".to_owned()]);
    }
}
