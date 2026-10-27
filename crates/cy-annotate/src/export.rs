//! Export: composite, crop, encode (PNG or JPEG); file names from a template.

use crate::AnnotateError;
use crate::document::Document;
use crate::render::{Rendered, render};
use crate::text::Font;

/// Longest exported file name, in bytes (common file systems allow 255).
pub const MAX_FILE_NAME_BYTES: usize = 200;
/// The name used when a template expands to nothing usable.
pub const FALLBACK_FILE_NAME: &str = "Supercy";

/// The encoded format of an export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// PNG, lossless; keeps transparency.
    Png,
    /// JPEG with `quality` from 1 to 100; transparent areas become white.
    Jpeg {
        /// Quality, 1 (smallest) to 100 (best).
        quality: u8,
    },
}

impl ExportFormat {
    /// The file extension, without the dot.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg { .. } => "jpg",
        }
    }
}

/// Composites `document`, crops it to its crop box and encodes it.
///
/// # Errors
///
/// [`AnnotateError::InvalidQuality`] for a JPEG quality outside 1–100;
/// [`AnnotateError::Encode`] when encoding fails.
pub fn export(
    document: &Document,
    font: Option<&Font>,
    format: ExportFormat,
) -> Result<Vec<u8>, AnnotateError> {
    if let ExportFormat::Jpeg { quality } = format
        && !(1..=100).contains(&quality)
    {
        return Err(AnnotateError::InvalidQuality);
    }
    let rendered = render(document, font)?;
    match format {
        ExportFormat::Png => encode_png(&rendered),
        ExportFormat::Jpeg { quality } => encode_jpeg(&rendered, quality),
    }
}

/// Encodes as PNG: RGB when every pixel is opaque (smaller), RGBA otherwise. Uses the fast
/// compression level: screenshots compress well, and export must stay under a second.
///
/// # Errors
///
/// [`AnnotateError::Encode`] when the encoder fails.
pub fn encode_png(rendered: &Rendered) -> Result<Vec<u8>, AnnotateError> {
    let (color, data) = if rendered.is_opaque() {
        (png::ColorType::Rgb, rendered.to_rgb_on_white())
    } else {
        (png::ColorType::Rgba, rendered.to_rgba())
    };
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, rendered.width(), rendered.height());
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|_| AnnotateError::Encode)?;
    writer
        .write_image_data(&data)
        .map_err(|_| AnnotateError::Encode)?;
    writer.finish().map_err(|_| AnnotateError::Encode)?;
    Ok(out)
}

/// Encodes as JPEG over a white background.
///
/// # Errors
///
/// [`AnnotateError::InvalidQuality`] outside 1–100; [`AnnotateError::Encode`] for images wider or
/// taller than 65535 pixels.
pub fn encode_jpeg(rendered: &Rendered, quality: u8) -> Result<Vec<u8>, AnnotateError> {
    let width = u16::try_from(rendered.width()).map_err(|_| AnnotateError::Encode)?;
    let height = u16::try_from(rendered.height()).map_err(|_| AnnotateError::Encode)?;
    if !(1..=100).contains(&quality) {
        return Err(AnnotateError::InvalidQuality);
    }
    Ok(crate::jpeg::encode(
        &rendered.to_rgb_on_white(),
        width,
        height,
        quality,
    ))
}

/// The local date and time and a counter, supplied by the shell (the core has no clock or
/// time zone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileNameContext {
    /// Year, such as 2026.
    pub year: u16,
    /// Month, 1–12.
    pub month: u8,
    /// Day of the month, 1–31.
    pub day: u8,
    /// Hour, 0–23.
    pub hour: u8,
    /// Minute, 0–59.
    pub minute: u8,
    /// Second, 0–60 (leap seconds).
    pub second: u8,
    /// A running number, such as the count of exports today.
    pub n: u32,
}

impl FileNameContext {
    fn is_valid(&self) -> bool {
        (1..=12).contains(&self.month)
            && (1..=31).contains(&self.day)
            && self.hour <= 23
            && self.minute <= 59
            && self.second <= 60
    }
}

/// Expands `template` into a file name with the format's extension.
///
/// Placeholders: `{date}` (`2026-10-27`), `{time}` (`16-05-09`; no colons, which macOS shows as
/// slashes), `{n}`. Unknown placeholders are kept as written. Path separators, colons and
/// control characters are removed, leading dots and spaces are dropped (no hidden files, no
/// `..`), and the name is cut to [`MAX_FILE_NAME_BYTES`].
///
/// # Errors
///
/// [`AnnotateError::InvalidFileName`] for an impossible date or time.
pub fn expand_file_name(
    template: &str,
    context: &FileNameContext,
    format: ExportFormat,
) -> Result<String, AnnotateError> {
    if !context.is_valid() {
        return Err(AnnotateError::InvalidFileName);
    }
    let date = format!(
        "{:04}-{:02}-{:02}",
        context.year, context.month, context.day
    );
    let time = format!(
        "{:02}-{:02}-{:02}",
        context.hour, context.minute, context.second
    );
    let expanded = template
        .replace("{date}", &date)
        .replace("{time}", &time)
        .replace("{n}", &context.n.to_string());
    let cleaned: String = expanded
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':') && !c.is_control())
        .collect();
    let trimmed = cleaned.trim_start_matches(['.', ' ']).trim_end();
    let stem = if trimmed.is_empty() {
        FALLBACK_FILE_NAME
    } else {
        cut(trimmed, MAX_FILE_NAME_BYTES)
    };
    Ok(format!("{stem}.{}", format.extension()))
}

/// `text` cut to at most `limit` bytes at a character boundary.
fn cut(text: &str, limit: usize) -> &str {
    let end = (0..=limit.min(text.len()))
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    &text[..end]
}

/// Joins the shell's chosen folder and a file name, after checking both: the folder must be an
/// absolute path without NUL bytes, the name a single plain component.
///
/// # Errors
///
/// [`AnnotateError::InvalidFileName`] otherwise.
pub fn target_path(folder: &str, file_name: &str) -> Result<String, AnnotateError> {
    let folder_ok = folder.starts_with('/') && !folder.contains('\0');
    let name_ok = !file_name.is_empty()
        && file_name.len() <= MAX_FILE_NAME_BYTES + 8
        && file_name != "."
        && file_name != ".."
        && !file_name.starts_with('.')
        && !file_name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':') || c.is_control());
    if !(folder_ok && name_ok) {
        return Err(AnnotateError::InvalidFileName);
    }
    Ok(format!("{}/{file_name}", folder.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: FileNameContext = FileNameContext {
        year: 2026,
        month: 10,
        day: 7,
        hour: 9,
        minute: 5,
        second: 3,
        n: 12,
    };

    #[test]
    fn expands_placeholders() {
        let name = expand_file_name("Supercy {date} {time} #{n}", &CONTEXT, ExportFormat::Png);
        assert_eq!(name.unwrap(), "Supercy 2026-10-07 09-05-03 #12.png");
        let jpeg = expand_file_name(
            "截图 {unknown}",
            &CONTEXT,
            ExportFormat::Jpeg { quality: 80 },
        );
        assert_eq!(jpeg.unwrap(), "截图 {unknown}.jpg");
    }

    #[test]
    fn sanitizes_separators_controls_and_hidden_names() {
        let cases = [
            ("../../etc/passwd", "etcpasswd.png"),
            ("a/b\\c:d\u{0}e\u{7}f\n", "abcdef.png"),
            ("  ..hidden ", "hidden.png"),
            ("..", "Supercy.png"),
            ("", "Supercy.png"),
            ("/", "Supercy.png"),
        ];
        for (template, expected) in cases {
            assert_eq!(
                expand_file_name(template, &CONTEXT, ExportFormat::Png).unwrap(),
                expected,
                "{template:?}"
            );
        }
    }

    #[test]
    fn long_names_are_cut_at_a_character_boundary() {
        let name = expand_file_name(&"示".repeat(100), &CONTEXT, ExportFormat::Png).unwrap();
        assert!(name.len() <= MAX_FILE_NAME_BYTES + 4);
        assert!(name.ends_with("示.png"));
        assert_eq!(cut("abc", 10), "abc");
    }

    #[test]
    fn impossible_dates_are_refused() {
        let bad = FileNameContext {
            month: 13,
            ..CONTEXT
        };
        assert_eq!(
            expand_file_name("x", &bad, ExportFormat::Png),
            Err(AnnotateError::InvalidFileName)
        );
        let bad_time = FileNameContext {
            minute: 60,
            ..CONTEXT
        };
        assert_eq!(
            expand_file_name("x", &bad_time, ExportFormat::Png),
            Err(AnnotateError::InvalidFileName)
        );
    }

    #[test]
    fn target_paths_are_checked() {
        assert_eq!(
            target_path("/tmp/supercy-test/", "demo.png").unwrap(),
            "/tmp/supercy-test/demo.png"
        );
        assert_eq!(
            target_path("/tmp/supercy-test", "demo.png").unwrap(),
            "/tmp/supercy-test/demo.png"
        );
        for (folder, name) in [
            ("relative", "demo.png"),
            ("/tmp/a\0b", "demo.png"),
            ("/tmp", "../demo.png"),
            ("/tmp", ".."),
            ("/tmp", ".hidden"),
            ("/tmp", ""),
            ("/tmp", "a:b.png"),
        ] {
            assert_eq!(
                target_path(folder, name),
                Err(AnnotateError::InvalidFileName),
                "{folder:?} {name:?}"
            );
        }
    }
}
