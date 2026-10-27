//! Fonts and text measurement.
//!
//! The core ships no font (decision D-7): the shell hands over the bytes of a system font at run
//! time. Without a font, text sizes are estimated so selection and hit testing still work, and
//! text is not drawn.

use std::sync::Arc;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::AnnotateError;
use crate::limits::MAX_FONT_BYTES;
use crate::style::TEXT_LINE_HEIGHT;

/// Estimated advance of a narrow (Latin) character without a font, per pixel of font size.
const ESTIMATED_NARROW_ADVANCE: f32 = 0.6;
/// Estimated advance of a wide (CJK) character without a font.
const ESTIMATED_WIDE_ADVANCE: f32 = 1.0;
/// Estimated ascent without a font.
const ESTIMATED_ASCENT: f32 = 0.88;
/// Characters from this code point up are estimated as wide.
const FIRST_WIDE_CHAR: char = '\u{1100}';

/// A parsed font face, cheap to clone.
#[derive(Clone)]
pub struct Font {
    data: Arc<[u8]>,
    index: u32,
    units_per_em: f32,
    ascender: f32,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("bytes", &self.data.len())
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl Font {
    /// Parses face `index` of `data` (a TrueType or OpenType font, or a collection).
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidFont`] when the data is too large or not a usable font.
    pub fn new(data: Vec<u8>, index: u32) -> Result<Self, AnnotateError> {
        if data.len() > MAX_FONT_BYTES {
            return Err(AnnotateError::InvalidFont);
        }
        let face = FontRef::from_index(&data, index).map_err(|_| AnnotateError::InvalidFont)?;
        let metrics = face.metrics(Size::unscaled(), LocationRef::default());
        let units_per_em = f32::from(metrics.units_per_em);
        if units_per_em <= 0.0 || !metrics.ascent.is_finite() {
            return Err(AnnotateError::InvalidFont);
        }
        let ascender = metrics.ascent;
        Ok(Self {
            data: Arc::from(data),
            index,
            units_per_em,
            ascender,
        })
    }

    /// Runs `visit` with the parsed face.
    pub(crate) fn with_face<T>(&self, visit: impl FnOnce(&FontRef<'_>) -> T) -> Option<T> {
        FontRef::from_index(&self.data, self.index)
            .ok()
            .map(|face| visit(&face))
    }

    /// Pixels per font unit at `size_px`.
    pub(crate) fn unit_scale(&self, size_px: f32) -> f32 {
        size_px / self.units_per_em
    }

    /// Distance from the top of a line to its baseline at `size_px`.
    #[must_use]
    pub fn ascent(&self, size_px: f32) -> f32 {
        self.ascender * self.unit_scale(size_px)
    }

    /// Horizontal advance of `text` (one line) at `size_px`.
    #[must_use]
    pub fn line_width(&self, text: &str, size_px: f32) -> f32 {
        let scale = self.unit_scale(size_px);
        self.with_face(|face| {
            let advances = Advances::new(face);
            text.chars().map(|c| advances.of(c).1 * scale).sum()
        })
        .unwrap_or(0.0)
    }
}

/// Glyph lookup and advances (in font units) for one parsed face.
pub(crate) struct Advances<'a> {
    charmap: skrifa::charmap::Charmap<'a>,
    metrics: skrifa::metrics::GlyphMetrics<'a>,
}

impl<'a> Advances<'a> {
    pub(crate) fn new(face: &FontRef<'a>) -> Self {
        Self {
            charmap: face.charmap(),
            metrics: face.glyph_metrics(Size::unscaled(), LocationRef::default()),
        }
    }

    /// The glyph for `c` (the fallback glyph 0 when the font lacks it) and its advance.
    pub(crate) fn of(&self, c: char) -> (GlyphId, f32) {
        let glyph = self.charmap.map(c).unwrap_or(GlyphId::new(0));
        (glyph, self.metrics.advance_width(glyph).unwrap_or(0.0))
    }
}

/// The size of laid-out text, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSize {
    /// Width of the longest line.
    pub width: f32,
    /// Line count times the line height.
    pub height: f32,
    /// Distance from the top to the first baseline.
    pub ascent: f32,
}

/// Lays out `text` at `size_px`: one line per `\n`, lines [`TEXT_LINE_HEIGHT`] apart.
#[must_use]
pub fn measure_text(font: Option<&Font>, text: &str, size_px: f32) -> TextSize {
    let lines = text.split('\n');
    let line_count = text.split('\n').count();
    let width = lines
        .map(|line| match font {
            Some(font) => font.line_width(line, size_px),
            None => estimated_width(line, size_px),
        })
        .fold(0.0, f32::max);
    #[allow(clippy::cast_precision_loss, reason = "line counts are small")]
    let height = line_count as f32 * TEXT_LINE_HEIGHT * size_px;
    let ascent = font.map_or(ESTIMATED_ASCENT * size_px, |font| font.ascent(size_px));
    TextSize {
        width,
        height,
        ascent,
    }
}

fn estimated_width(line: &str, size_px: f32) -> f32 {
    line.chars()
        .map(|c| {
            if c >= FIRST_WIDE_CHAR {
                ESTIMATED_WIDE_ADVANCE
            } else {
                ESTIMATED_NARROW_ADVANCE
            }
        })
        .sum::<f32>()
        * size_px
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_without_a_font() {
        let size = measure_text(None, "ab\n示例文本", 10.0);
        assert!((size.width - 40.0).abs() < 1e-4);
        assert!((size.height - 25.0).abs() < 1e-4);
        assert!((size.ascent - 8.8).abs() < 1e-4);
    }

    #[test]
    fn rejects_data_that_is_not_a_font() {
        assert_eq!(
            Font::new(b"not a font".to_vec(), 0).err(),
            Some(AnnotateError::InvalidFont)
        );
        assert_eq!(
            Font::new(vec![0; MAX_FONT_BYTES + 1], 0).err(),
            Some(AnnotateError::InvalidFont)
        );
    }
}
