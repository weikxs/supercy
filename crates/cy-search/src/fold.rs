//! Folding text for search: lowercase, whitespace collapsed to single spaces, cut to a byte limit.

/// Folds `text` and stops before `limit` bytes.
pub(crate) fn fold(text: &str, limit: usize) -> String {
    let mut out = String::with_capacity(text.len().min(limit));
    let mut last_space = true;
    for c in text.chars() {
        let pieces: Vec<char> = if c.is_whitespace() {
            vec![' ']
        } else {
            c.to_lowercase().collect()
        };
        if pieces == [' '] && last_space {
            continue;
        }
        let needed: usize = pieces.iter().map(|p| p.len_utf8()).sum();
        if out.len() + needed > limit {
            break;
        }
        last_space = pieces == [' '];
        out.extend(pieces);
    }
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.shrink_to_fit();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_collapses_whitespace() {
        assert_eq!(
            fold("  Hello\n\tFrom   SUPERCY ", 100),
            "hello from supercy"
        );
    }

    #[test]
    fn stops_at_the_limit_on_a_character_boundary() {
        assert_eq!(fold("示例文本", 7), "示例");
        assert_eq!(fold("abcdef", 3), "abc");
    }
}
