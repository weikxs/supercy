//! Detection of sensitive content: private keys, well-known token formats, card numbers and
//! password-like strings. Only the reason is returned; the content is never logged.

use cy_model::SensitiveReason;

/// Card numbers have 13 to 19 digits.
const CARD_DIGITS: std::ops::RangeInclusive<usize> = 13..=19;
/// Password-like strings: one word of this length.
const PASSWORD_LEN: std::ops::RangeInclusive<usize> = 12..=128;
/// Minimum Shannon entropy, in bits per character, for a password-like string.
const PASSWORD_MIN_ENTROPY: f64 = 3.5;
/// A password-like string mixes at least this many of: lowercase, uppercase, digits, symbols.
const PASSWORD_MIN_CLASSES: usize = 3;

/// Why `text` looks sensitive, if it does.
#[must_use]
pub fn detect_sensitive(text: &str) -> Option<SensitiveReason> {
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----") {
        return Some(SensitiveReason::PrivateKey);
    }
    if text.split_whitespace().any(is_access_token) {
        return Some(SensitiveReason::AccessToken);
    }
    let trimmed = text.trim();
    if is_card_number(trimmed) {
        return Some(SensitiveReason::CardNumber);
    }
    is_password_like(trimmed).then_some(SensitiveReason::HighEntropy)
}

/// Well-known token prefixes followed by a minimum number of token characters.
fn is_access_token(word: &str) -> bool {
    const PREFIXES: &[(&str, usize)] = &[
        ("AKIA", 16),
        ("ghp_", 36),
        ("gho_", 36),
        ("ghu_", 36),
        ("ghs_", 36),
        ("ghr_", 36),
        ("github_pat_", 22),
        ("xoxb-", 10),
        ("xoxp-", 10),
        ("xoxa-", 10),
        ("sk-", 32),
    ];
    let is_token_char = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    PREFIXES.iter().any(|(prefix, min)| {
        word.strip_prefix(prefix)
            .is_some_and(|rest| rest.len() >= *min && rest.chars().all(is_token_char))
    }) || is_jwt(word)
}

/// Three base64url parts separated by dots, the first one a JSON object (`eyJ`).
fn is_jwt(word: &str) -> bool {
    const MIN_PART: usize = 8;
    let parts: Vec<&str> = word.split('.').collect();
    let is_b64url = |part: &&str| {
        part.len() >= MIN_PART
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    parts.len() == 3 && word.starts_with("eyJ") && parts.iter().all(is_b64url)
}

/// Only digits, spaces and dashes, 13–19 digits, valid Luhn checksum.
fn is_card_number(text: &str) -> bool {
    if !text
        .chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || c == '-')
    {
        return false;
    }
    let digits: Vec<u32> = text.chars().filter_map(|c| c.to_digit(10)).collect();
    CARD_DIGITS.contains(&digits.len()) && luhn_valid(&digits)
}

fn luhn_valid(digits: &[u32]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| match (i % 2 == 1, d * 2) {
            (true, doubled) if doubled > 9 => doubled - 9,
            (true, doubled) => doubled,
            (false, _) => *d,
        })
        .sum();
    sum.is_multiple_of(10)
}

/// One word, not a link or a path, mixing character classes, with high entropy.
fn is_password_like(text: &str) -> bool {
    let len = text.chars().count();
    if !PASSWORD_LEN.contains(&len) || text.chars().any(char::is_whitespace) {
        return false;
    }
    if text.contains("://") || text.starts_with('/') || text.starts_with('~') {
        return false;
    }
    // Hashes and UUIDs are identifiers people copy all the time, not passwords.
    if text.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return false;
    }
    let classes = [
        text.chars().any(|c| c.is_ascii_lowercase()),
        text.chars().any(|c| c.is_ascii_uppercase()),
        text.chars().any(|c| c.is_ascii_digit()),
        text.chars().any(|c| !c.is_alphanumeric()),
    ];
    classes.iter().filter(|present| **present).count() >= PASSWORD_MIN_CLASSES
        && entropy_bits_per_char(text) >= PASSWORD_MIN_ENTROPY
}

#[allow(clippy::cast_precision_loss, reason = "counts are far below 2^52")]
fn entropy_bits_per_char(text: &str) -> f64 {
    let chars: Vec<char> = text.chars().collect();
    let mut counts = std::collections::BTreeMap::new();
    for c in &chars {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let total = chars.len() as f64;
    counts
        .values()
        .map(|count| {
            let p = *count as f64 / total;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random characters from `alphabet`, so no real token is ever written
    /// in the source.
    fn generated(alphabet: &str, len: usize, seed: u64) -> String {
        let chars: Vec<char> = alphabet.chars().collect();
        let mut state = seed;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                chars[usize::try_from(state >> 33).unwrap() % chars.len()]
            })
            .collect()
    }

    const ALNUM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    /// A random card-like number with a valid Luhn check digit.
    fn card_number(seed: u64) -> String {
        let body: Vec<u32> = generated("0123456789", 15, seed)
            .chars()
            .filter_map(|c| c.to_digit(10))
            .collect();
        let check = (0..10)
            .find(|d| luhn_valid(&[body.clone(), vec![*d]].concat()))
            .unwrap();
        body.iter()
            .chain([check].iter())
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn private_keys_are_sensitive() {
        let pem = format!("-----BEGIN {}PRIVATE KEY-----\nMIIE...\n", "RSA ");

        assert_eq!(detect_sensitive(&pem), Some(SensitiveReason::PrivateKey));
    }

    #[test]
    fn generated_tokens_of_known_forms_are_sensitive() {
        let tokens = [
            ["gh", "p_", &generated(ALNUM, 36, 1)].concat(),
            [
                "AK",
                "IA",
                &generated("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567", 16, 2),
            ]
            .concat(),
            ["s", "k-", &generated(ALNUM, 40, 3)].concat(),
            format!(
                "eyJ{}.eyJ{}.{}",
                generated(ALNUM, 12, 4),
                generated(ALNUM, 20, 5),
                generated(ALNUM, 30, 6)
            ),
        ];

        for token in tokens {
            assert_eq!(
                detect_sensitive(&format!("token: {token}")),
                Some(SensitiveReason::AccessToken)
            );
        }
    }

    #[test]
    fn card_numbers_with_valid_checksums_are_sensitive() {
        let number = card_number(7);
        let spaced: String = number
            .chars()
            .enumerate()
            .flat_map(|(i, c)| {
                if i > 0 && i % 4 == 0 {
                    vec![' ', c]
                } else {
                    vec![c]
                }
            })
            .collect();

        assert_eq!(detect_sensitive(&number), Some(SensitiveReason::CardNumber));
        assert_eq!(detect_sensitive(&spaced), Some(SensitiveReason::CardNumber));
    }

    #[test]
    fn numbers_failing_the_checksum_or_length_are_not_cards() {
        let number = card_number(8);
        let last = number.chars().last().and_then(|c| c.to_digit(10)).unwrap();
        let broken = format!("{}{}", &number[..number.len() - 1], (last + 1) % 10);

        assert_eq!(detect_sensitive(&broken), None);
        assert_eq!(detect_sensitive("123456789012"), None);
    }

    #[test]
    fn password_like_words_are_sensitive() {
        let password = format!("{}#{}", generated(ALNUM, 10, 9), generated(ALNUM, 8, 10));

        assert_eq!(
            detect_sensitive(&password),
            Some(SensitiveReason::HighEntropy)
        );
    }

    #[test]
    fn ordinary_text_links_and_paths_are_not_sensitive() {
        for text in [
            "Hello from Supercy",
            "示例文本，会议纪要",
            "https://example.com/Path/To/Page?id=42",
            "/tmp/supercy-test/Demo-File_01.txt",
            "aaaaaaaaaaaaaaaaaaaa",
            "CamelCaseIdentifier",
            "short1A!",
            "3f2a9c4e-0b7d-4e1a-9f3c-5d6e7a8b9c0d",
        ] {
            assert_eq!(detect_sensitive(text), None, "{text}");
        }
    }
}
