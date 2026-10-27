//! Turns clipboard types into representations, applying the per-type switches and size limits.

use cy_model::{ClipEntry, ImageFormat, Representation, SecretBytes, SecretString};

use crate::{DropReason, FilterPolicy};

/// Most file references kept from one copy.
const MAX_FILES: usize = 1_000;

/// Representations built from one snapshot.
pub(crate) struct Built {
    pub(crate) reps: Vec<Representation>,
    pub(crate) truncated: bool,
}

/// Where a clipboard type goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Text,
    Html,
    Rtf,
    Url,
    FileUrl,
    UriList,
    Image(ImageFormat),
}

/// Platform type names, macOS first, then Windows, then Linux (MIME and X11 targets). Text types
/// arrive as UTF-8 (the shell converts).
const TYPES: &[(&str, Slot)] = &[
    ("public.utf8-plain-text", Slot::Text),
    ("NSStringPboardType", Slot::Text),
    ("CF_UNICODETEXT", Slot::Text),
    ("text/plain;charset=utf-8", Slot::Text),
    ("UTF8_STRING", Slot::Text),
    ("text/plain", Slot::Text),
    ("public.html", Slot::Html),
    ("HTML Format", Slot::Html),
    ("text/html", Slot::Html),
    ("public.rtf", Slot::Rtf),
    ("Rich Text Format", Slot::Rtf),
    ("text/rtf", Slot::Rtf),
    ("public.url", Slot::Url),
    ("UniformResourceLocatorW", Slot::Url),
    ("public.file-url", Slot::FileUrl),
    ("text/uri-list", Slot::UriList),
    ("public.png", Slot::Image(ImageFormat::Png)),
    ("PNG", Slot::Image(ImageFormat::Png)),
    ("image/png", Slot::Image(ImageFormat::Png)),
    ("public.tiff", Slot::Image(ImageFormat::Tiff)),
    ("image/tiff", Slot::Image(ImageFormat::Tiff)),
    ("public.jpeg", Slot::Image(ImageFormat::Jpeg)),
    ("JFIF", Slot::Image(ImageFormat::Jpeg)),
    ("image/jpeg", Slot::Image(ImageFormat::Jpeg)),
    ("com.compuserve.gif", Slot::Image(ImageFormat::Gif)),
    ("GIF", Slot::Image(ImageFormat::Gif)),
    ("image/gif", Slot::Image(ImageFormat::Gif)),
];

fn slot_of(type_id: &str) -> Option<Slot> {
    TYPES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(type_id))
        .map(|(_, slot)| *slot)
}

/// The first entry of each slot as found on the clipboard, and every file reference.
#[derive(Default)]
struct Slots<'a> {
    text: Option<&'a ClipEntry>,
    html: Option<&'a ClipEntry>,
    rtf: Option<&'a ClipEntry>,
    url: Option<&'a ClipEntry>,
    image: Option<(&'a ClipEntry, ImageFormat)>,
    files: Vec<String>,
}

fn sort_into_slots(entries: &[ClipEntry]) -> Slots<'_> {
    let mut slots = Slots::default();
    for entry in entries {
        match slot_of(&entry.type_id) {
            Some(Slot::Text) => slots.text = slots.text.or(Some(entry)),
            Some(Slot::Html) => slots.html = slots.html.or(Some(entry)),
            Some(Slot::Rtf) => slots.rtf = slots.rtf.or(Some(entry)),
            Some(Slot::Url) => slots.url = slots.url.or(Some(entry)),
            Some(Slot::FileUrl) => slots.files.extend(file_urls(entry, false)),
            Some(Slot::UriList) => slots.files.extend(file_urls(entry, true)),
            Some(Slot::Image(format)) => slots.image = slots.image.or(Some((entry, format))),
            None => {}
        }
    }
    slots.files.truncate(MAX_FILES);
    slots
}

/// `file://` URLs in an entry: one per entry on macOS, one per line in a URI list.
fn file_urls(entry: &ClipEntry, list: bool) -> Vec<String> {
    let Some(text) = entry
        .data
        .as_ref()
        .and_then(|d| std::str::from_utf8(d.expose()).ok())
    else {
        return Vec::new();
    };
    let lines: Vec<&str> = if list {
        text.lines().collect()
    } else {
        vec![text]
    };
    lines
        .into_iter()
        .map(|line| line.trim().trim_end_matches('\0'))
        .filter(|line| !line.starts_with('#') && line.starts_with("file://"))
        .map(str::to_owned)
        .collect()
}

/// Builds the representations to keep, or the reason to keep nothing.
pub(crate) fn build(entries: &[ClipEntry], policy: &FilterPolicy) -> Result<Built, DropReason> {
    let slots = sort_into_slots(entries);
    if !slots.files.is_empty() {
        // A file copy also carries the file's name and icon; only the references matter.
        if !policy.record_files {
            return Err(DropReason::TypeDisabled);
        }
        let reps = vec![Representation::FileRef { names: slots.files }];
        return Ok(Built {
            reps,
            truncated: false,
        });
    }
    let mut oversized = false;
    let mut disabled = false;
    let mut truncated = false;
    let mut reps = Vec::new();
    if policy.record_text {
        truncated = text_reps(&slots, policy, &mut oversized, &mut reps);
    } else {
        disabled |= slots.text.is_some() || slots.url.is_some();
    }
    match slots.image {
        Some((entry, format)) if policy.record_images => {
            reps.extend(read_image(entry, format, policy, &mut oversized));
        }
        Some(_) => disabled = true,
        None => {}
    }
    match (reps.is_empty(), oversized, disabled) {
        (false, _, _) => Ok(Built { reps, truncated }),
        (true, true, _) => Err(DropReason::TooLarge),
        (true, false, true) => Err(DropReason::TypeDisabled),
        (true, false, false) => Err(DropReason::Unsupported),
    }
}

/// Adds the link and the plain or rich text; returns whether text was cut.
fn text_reps(
    slots: &Slots<'_>,
    policy: &FilterPolicy,
    oversized: &mut bool,
    reps: &mut Vec<Representation>,
) -> bool {
    let limit = policy.max_text_bytes;
    let text = slots
        .text
        .and_then(|entry| read_text(entry, limit, oversized));
    let url = slots
        .url
        .and_then(|entry| read_whole_text(entry, limit, oversized))
        .or_else(|| {
            text.as_ref()
                .filter(|(t, _)| looks_like_url(t))
                .map(|(t, _)| t.trim().to_owned())
        });
    if let Some(url) = url {
        reps.push(Representation::Url {
            url: SecretString::new(url),
            title: None,
        });
    }
    let Some((text, cut)) = text else {
        return false;
    };
    reps.push(text_rep(slots, text, limit, oversized));
    cut
}

/// Plain or rich text; rich parts above the limit are left out.
fn text_rep(
    slots: &Slots<'_>,
    plain: String,
    limit: usize,
    oversized: &mut bool,
) -> Representation {
    let html = slots
        .html
        .and_then(|entry| read_whole_text(entry, limit, oversized));
    let rtf = slots
        .rtf
        .and_then(|entry| read_bytes(entry, limit, oversized));
    if html.is_none() && rtf.is_none() {
        return Representation::PlainText(SecretString::new(plain));
    }
    Representation::RichText {
        rtf: rtf.map(SecretBytes::new),
        html: html.map(SecretString::new),
        plain: SecretString::new(plain),
    }
}

/// Text cut to `limit` bytes at a character boundary; `None` when empty or not UTF-8.
fn read_text(entry: &ClipEntry, limit: usize, oversized: &mut bool) -> Option<(String, bool)> {
    let Some(data) = entry.data.as_ref() else {
        *oversized |= entry.size > 0;
        return None;
    };
    let text = std::str::from_utf8(data.expose())
        .ok()?
        .trim_end_matches('\0');
    if text.trim().is_empty() {
        return None;
    }
    if text.len() <= limit {
        return Some((text.to_owned(), false));
    }
    let cut = (0..=limit)
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    Some((text[..cut].to_owned(), true))
}

/// Text that must be kept whole (links, HTML): above the limit it is left out.
fn read_whole_text(entry: &ClipEntry, limit: usize, oversized: &mut bool) -> Option<String> {
    let bytes = read_bytes(entry, limit, oversized)?;
    let text = String::from_utf8(bytes).ok()?;
    let text = text.trim_end_matches('\0');
    (!text.trim().is_empty()).then(|| text.to_owned())
}

fn read_bytes(entry: &ClipEntry, limit: usize, oversized: &mut bool) -> Option<Vec<u8>> {
    match entry.data.as_ref() {
        Some(data) if data.len() <= limit => Some(data.expose().to_vec()),
        Some(_) => {
            *oversized = true;
            None
        }
        None => {
            *oversized |= entry.size > 0;
            None
        }
    }
}

fn read_image(
    entry: &ClipEntry,
    format: ImageFormat,
    policy: &FilterPolicy,
    oversized: &mut bool,
) -> Option<Representation> {
    let bytes = read_bytes(entry, policy.max_image_bytes, oversized)?;
    if bytes.is_empty() {
        return None;
    }
    let (width, height) = image_size(format, &bytes);
    Some(Representation::Image {
        format,
        width,
        height,
        bytes: SecretBytes::new(bytes),
    })
}

/// Width and height from a PNG header; (0, 0) for other formats or a short header.
fn image_size(format: ImageFormat, bytes: &[u8]) -> (u32, u32) {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let read_u32 = |at: usize| {
        bytes
            .get(at..at + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes)
    };
    if format != ImageFormat::Png || !bytes.starts_with(PNG_SIGNATURE) {
        return (0, 0);
    }
    read_u32(16).zip(read_u32(20)).unwrap_or((0, 0))
}

/// A single `http`/`https` link with nothing else around it.
fn looks_like_url(text: &str) -> bool {
    let text = text.trim();
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty() && !text.chars().any(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> FilterPolicy {
        FilterPolicy::default()
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn png_header_gives_the_size_and_other_formats_do_not() {
        assert_eq!(image_size(ImageFormat::Png, &png(640, 480)), (640, 480));
        assert_eq!(image_size(ImageFormat::Png, b"\x89PNG\r\n\x1a\n"), (0, 0));
        assert_eq!(image_size(ImageFormat::Tiff, &png(1, 1)), (0, 0));
    }

    #[test]
    fn link_detection() {
        assert!(looks_like_url("https://example.com/path?q=1"));
        assert!(looks_like_url("  http://demo.example  "));
        assert!(!looks_like_url("see https://example.com"));
        assert!(!looks_like_url("https://"));
        assert!(!looks_like_url("ftp://example.com"));
    }

    #[test]
    fn text_is_cut_at_a_character_boundary() {
        let entry = ClipEntry::with_data("public.utf8-plain-text", "示例文本".as_bytes().to_vec());
        let mut oversized = false;

        let (text, cut) = read_text(&entry, 7, &mut oversized).unwrap();

        assert_eq!(text, "示例");
        assert!(cut);
        assert!(!oversized);
    }

    #[test]
    fn image_above_the_limit_is_left_out_but_text_is_kept() {
        let small_limit = FilterPolicy {
            max_image_bytes: 10,
            ..policy()
        };
        let entries = [
            ClipEntry::with_data("public.utf8-plain-text", b"Demo chart".to_vec()),
            ClipEntry::with_data("public.png", png(2, 2)),
        ];

        let built = build(&entries, &small_limit).ok().unwrap();

        assert_eq!(built.reps.len(), 1);
        assert_eq!(built.reps[0].search_text(), "Demo chart");
    }

    #[test]
    fn unreadable_entries_are_unsupported() {
        let invalid_utf8 = [ClipEntry::with_data(
            "public.utf8-plain-text",
            vec![0xff, 0xfe],
        )];
        let unknown = [ClipEntry::with_data("com.example.private", b"x".to_vec())];
        let blank = [ClipEntry::with_data(
            "public.utf8-plain-text",
            b"  \n ".to_vec(),
        )];

        for entries in [&invalid_utf8[..], &unknown[..], &blank[..]] {
            assert!(matches!(
                build(entries, &policy()),
                Err(DropReason::Unsupported)
            ));
        }
    }

    #[test]
    fn file_copies_keep_only_their_references() {
        let entries = [
            ClipEntry::with_data(
                "public.file-url",
                b"file:///tmp/supercy-test/a.txt".to_vec(),
            ),
            ClipEntry::with_data(
                "public.file-url",
                b"file:///tmp/supercy-test/b.txt".to_vec(),
            ),
            ClipEntry::with_data("public.utf8-plain-text", b"a.txt".to_vec()),
            ClipEntry::with_data("public.tiff", vec![1, 2, 3]),
        ];
        let uri_list = [ClipEntry::with_data(
            "text/uri-list",
            b"# comment\r\nfile:///tmp/supercy-test/c.txt\r\nhttps://example.com\r\n".to_vec(),
        )];

        let built = build(&entries, &policy()).ok().unwrap();
        let listed = build(&uri_list, &policy()).ok().unwrap();

        assert_eq!(
            built.reps,
            vec![Representation::FileRef {
                names: vec![
                    "file:///tmp/supercy-test/a.txt".into(),
                    "file:///tmp/supercy-test/b.txt".into()
                ]
            }]
        );
        assert_eq!(
            listed.reps,
            vec![Representation::FileRef {
                names: vec!["file:///tmp/supercy-test/c.txt".into()]
            }]
        );
    }

    #[test]
    fn switched_off_kinds_are_left_out_or_disable_the_copy() {
        let no_files = FilterPolicy {
            record_files: false,
            ..policy()
        };
        let no_images = FilterPolicy {
            record_images: false,
            ..policy()
        };
        let no_text = FilterPolicy {
            record_text: false,
            ..policy()
        };
        let file = [ClipEntry::with_data(
            "public.file-url",
            b"file:///tmp/supercy-test/a.txt".to_vec(),
        )];
        let text_and_image = [
            ClipEntry::with_data("public.utf8-plain-text", b"Demo".to_vec()),
            ClipEntry::with_data("public.png", png(1, 1)),
        ];
        let image = [ClipEntry::with_data("public.png", png(1, 1))];

        assert!(matches!(
            build(&file, &no_files),
            Err(DropReason::TypeDisabled)
        ));
        assert_eq!(
            build(&text_and_image, &no_images).ok().unwrap().reps.len(),
            1
        );
        assert!(matches!(
            build(&image, &no_images),
            Err(DropReason::TypeDisabled)
        ));
        assert!(matches!(
            &build(&text_and_image, &no_text).ok().unwrap().reps[0],
            Representation::Image { .. }
        ));
    }
}
