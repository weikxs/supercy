//! Pinyin forms of text with Chinese characters.
//!
//! Each Chinese character becomes its toneless pinyin syllable with the first letter in
//! uppercase, which marks syllable starts for the matcher ("会议" becomes `HuiYi`). Other
//! characters are kept as they are. The primary form uses each character's most common reading;
//! the alternate form, kept only when some character has more than one reading, uses the second
//! reading of every such character ("银行" is found as "yinhang" as well as "yinxing").

use pinyin::{ToPinyin, ToPinyinMulti};

/// Pinyin forms of one text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PinyinForms {
    pub(crate) primary: Box<str>,
    pub(crate) alternate: Option<Box<str>>,
}

impl PinyinForms {
    pub(crate) fn heap_bytes(&self) -> usize {
        self.primary.len() + self.alternate.as_ref().map_or(0, |a| a.len())
    }
}

/// The pinyin forms of `folded` text, or `None` when it has no Chinese characters.
pub(crate) fn pinyin_forms(folded: &str, limit: usize) -> Option<PinyinForms> {
    if !folded.chars().any(|c| c.to_pinyin().is_some()) {
        return None;
    }
    let mut primary = String::new();
    let mut alternate = String::new();
    let mut has_alternate = false;
    for c in folded.chars() {
        let (first, second) = readings(c);
        if primary.len() + first.len() > limit || alternate.len() + second.len() > limit {
            break;
        }
        has_alternate |= first != second;
        primary.push_str(&first);
        alternate.push_str(&second);
    }
    Some(PinyinForms {
        primary: primary.into_boxed_str(),
        alternate: has_alternate.then(|| alternate.into_boxed_str()),
    })
}

/// The first and second readings of `c` as capitalized syllables; the character itself when it
/// is not Chinese.
fn readings(c: char) -> (String, String) {
    let Some(multi) = c.to_pinyin_multi() else {
        return (c.to_string(), c.to_string());
    };
    let mut plain = multi.into_iter().map(|p| capitalize(p.plain()));
    let first = plain.next().unwrap_or_else(|| c.to_string());
    let second = plain.next().unwrap_or_else(|| first.clone());
    (first, second)
}

fn capitalize(syllable: &str) -> String {
    let mut chars = syllable.chars();
    chars
        .next()
        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_becomes_capitalized_syllables_and_other_characters_stay() {
        let forms = pinyin_forms("会议notes", 100).unwrap();

        assert_eq!(&*forms.primary, "HuiYinotes");
    }

    #[test]
    fn text_without_chinese_has_no_forms() {
        assert_eq!(pinyin_forms("hello from supercy", 100), None);
    }

    #[test]
    fn polyphonic_characters_get_an_alternate_form() {
        let forms = pinyin_forms("银行", 100).unwrap();
        let both = [Some(&*forms.primary), forms.alternate.as_deref()];

        assert!(both.contains(&Some("YinHang")), "{both:?}");
    }

    #[test]
    fn forms_stop_at_the_limit() {
        let forms = pinyin_forms(&"中".repeat(100), 20).unwrap();

        assert!(forms.primary.len() <= 20);
        assert!(forms.heap_bytes() <= 40);
    }
}
