//! Text as vector paths: glyph outlines from the font, laid out one line per `\n`.
//!
//! Glyphs are filled like any other shape, so text is anti-aliased the same way and needs no
//! separate rasterizer. Layout is deliberately simple (no kerning, no shaping), which suits
//! Chinese and plain Latin labels.

use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use tiny_skia::{Path, PathBuilder};

use crate::geometry::Point;
use crate::style::TEXT_LINE_HEIGHT;
use crate::text::{Advances, Font};

/// Writes glyph outlines (font units, y up) into a path (pixels, y down).
struct Outline<'a> {
    path: &'a mut PathBuilder,
    x: f32,
    baseline: f32,
    scale: f32,
}

impl Outline<'_> {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.baseline - y * self.scale)
    }
}

impl OutlinePen for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.map(x1, y1);
        let (x, y) = self.map(x, y);
        self.path.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.map(x1, y1);
        let (x2, y2) = self.map(x2, y2);
        let (x, y) = self.map(x, y);
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

/// The outlines of `text` at `size_px`, with the top-left of the first line at `origin`.
/// `None` when nothing would be drawn (only spaces, or glyphs without outlines).
pub(crate) fn text_path(font: &Font, text: &str, origin: Point, size_px: f32) -> Option<Path> {
    let scale = font.unit_scale(size_px);
    let ascent = font.ascent(size_px);
    let mut builder = PathBuilder::new();
    font.with_face(|face| {
        let advances = Advances::new(face);
        let outlines = face.outline_glyphs();
        for (index, line) in text.split('\n').enumerate() {
            #[allow(clippy::cast_precision_loss, reason = "line counts are small")]
            let baseline = origin.y + ascent + index as f32 * TEXT_LINE_HEIGHT * size_px;
            let mut x = origin.x;
            for c in line.chars() {
                let (glyph, advance) = advances.of(c);
                let mut outline = Outline {
                    path: &mut builder,
                    x,
                    baseline,
                    scale,
                };
                if let Some(shape) = outlines.get(glyph) {
                    let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
                    // A glyph that fails to draw is skipped; the rest of the text still shows.
                    let _ = shape.draw(settings, &mut outline);
                }
                x += advance * scale;
            }
        }
    })?;
    builder.finish()
}
