//! Compositing: the base image with every shape drawn on top, for export and for the shell's
//! previews of pixel effects (mosaic, blur) and text.
//!
//! Only the requested region is drawn. Mosaic and blur read the pixels inside their own box and
//! nothing outside it, so a region is exact as long as every effect box that touches it is drawn
//! whole; [`render_region`] widens its work area to make sure of that.

mod effects;
mod glyphs;
mod shapes;

use tiny_skia::{BlendMode, IntRect, Pixmap, PixmapPaint, Transform};

use crate::AnnotateError;
use crate::document::{Document, mul_div_255};
use crate::geometry::Rect;
use crate::shape::{Metrics, Shape, ShapeKind};
use crate::text::Font;

/// A rendered part of the composite.
#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    /// Left edge in base-image pixels.
    pub x: u32,
    /// Top edge in base-image pixels.
    pub y: u32,
    pixmap: Pixmap,
}

impl Rendered {
    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// The pixels as straight-alpha RGBA, row by row.
    #[must_use]
    pub fn to_rgba(&self) -> Vec<u8> {
        self.pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect()
    }

    /// Whether every pixel is opaque.
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.pixmap
            .pixels()
            .iter()
            .all(|pixel| pixel.alpha() == u8::MAX)
    }

    /// The pixels as RGB over a white background (for formats without alpha).
    #[must_use]
    pub fn to_rgb_on_white(&self) -> Vec<u8> {
        self.pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let white = u16::from(u8::MAX - pixel.alpha());
                [pixel.red(), pixel.green(), pixel.blue()]
                    .map(|channel| channel.saturating_add(mul_div_255(255, white)))
            })
            .collect()
    }
}

/// Renders the composite inside `region` (snapped outwards to whole pixels, clipped to the
/// image).
///
/// # Errors
///
/// [`AnnotateError::InvalidRegion`] when `region` is invalid or outside the image.
pub fn render_region(
    document: &Document,
    font: Option<&Font>,
    region: Rect,
) -> Result<Rendered, AnnotateError> {
    let bounds = document.base().bounds();
    let wanted = Some(region)
        .filter(Rect::is_valid)
        .and_then(|r| r.round_out().intersection(&bounds))
        .ok_or(AnnotateError::InvalidRegion)?;
    let metrics = Metrics {
        scale: document.base().scale(),
        font,
    };
    let work = work_area(document, wanted, &bounds);
    let mut canvas = canvas_for(document, &work)?;
    let origin = (work.x, work.y);
    for shape in document.shapes() {
        if shape.bounds(metrics).intersection(&work).is_some() {
            draw(&mut canvas, shape, origin, metrics);
        }
    }
    let pixmap = canvas
        .clone_rect(int_rect(&wanted.offset(-work.x, -work.y))?)
        .ok_or(AnnotateError::InvalidRegion)?;
    Ok(Rendered {
        x: to_pixel(wanted.x),
        y: to_pixel(wanted.y),
        pixmap,
    })
}

/// Renders the export area (the crop box, or the whole image).
///
/// # Errors
///
/// Only on internal failures; the export area is always inside the image.
pub fn render(document: &Document, font: Option<&Font>) -> Result<Rendered, AnnotateError> {
    render_region(document, font, document.export_area())
}

/// `wanted` grown until it holds every effect box it touches, clipped to the image.
fn work_area(document: &Document, wanted: Rect, bounds: &Rect) -> Rect {
    let effects: Vec<Rect> = document
        .shapes()
        .filter(|shape| {
            matches!(
                shape.kind,
                ShapeKind::Mosaic { .. } | ShapeKind::Blur { .. }
            )
        })
        .filter_map(|shape| shape.kind.area()?.round_out().intersection(bounds))
        .collect();
    let mut work = wanted;
    loop {
        let grown = effects
            .iter()
            .filter(|area| area.intersection(&work).is_some())
            .fold(work, |acc, area| acc.union(area));
        if grown == work {
            return work;
        }
        work = grown;
    }
}

/// A transparent canvas for `work` with the base image copied in.
fn canvas_for(document: &Document, work: &Rect) -> Result<Pixmap, AnnotateError> {
    let mut canvas = Pixmap::new(to_pixel(work.width), to_pixel(work.height))
        .ok_or(AnnotateError::InvalidRegion)?;
    let paint = PixmapPaint {
        blend_mode: BlendMode::Source,
        ..PixmapPaint::default()
    };
    canvas.draw_pixmap(
        -to_offset(work.x),
        -to_offset(work.y),
        document.base().pixmap().as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
    Ok(canvas)
}

fn draw(canvas: &mut Pixmap, shape: &Shape, origin: (f32, f32), metrics: Metrics<'_>) {
    match &shape.kind {
        ShapeKind::Mosaic { rect, block } => {
            effects::mosaic(canvas, origin, rect, block_pixels(*block, metrics.scale));
        }
        ShapeKind::Blur { rect, radius } => {
            effects::blur(
                canvas,
                origin,
                rect,
                to_pixel(radius * metrics.scale).max(1),
            );
        }
        _ => shapes::draw(canvas, shape, origin, metrics),
    }
}

fn block_pixels(block: u32, scale: f32) -> u32 {
    #[allow(clippy::cast_precision_loss, reason = "blocks are at most 256")]
    to_pixel(block as f32 * scale).max(1)
}

/// A non-negative pixel count or position, rounded.
pub(crate) fn to_pixel(value: f32) -> u32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "clamped to the u32 range first"
    )]
    let pixel = value.round().clamp(0.0, u32::MAX as f32) as u32;
    pixel
}

/// A pixel offset, rounded.
pub(crate) fn to_offset(value: f32) -> i32 {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "image coordinates are far inside the i32 range"
    )]
    let offset = value.round() as i32;
    offset
}

fn int_rect(rect: &Rect) -> Result<IntRect, AnnotateError> {
    IntRect::from_xywh(
        to_offset(rect.x),
        to_offset(rect.y),
        to_pixel(rect.width),
        to_pixel(rect.height),
    )
    .ok_or(AnnotateError::InvalidRegion)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::BaseImage;
    use crate::geometry::Point;
    use crate::{Command, Style};

    fn document() -> Document {
        let rgba: Vec<u8> = (0..40 * 30)
            .flat_map(|i: u32| [u8::try_from(i % 251).unwrap(), 80, 160, 255])
            .collect();
        Document::new(BaseImage::from_rgba(40, 30, rgba, 1.0).unwrap())
    }

    fn with(document: &Document, kind: ShapeKind) -> Document {
        document
            .apply(
                &Command::Add {
                    kind,
                    style: Style::default(),
                },
                None,
            )
            .unwrap()
            .document
    }

    #[test]
    fn regions_match_the_full_render() {
        let doc = with(
            &document(),
            ShapeKind::Blur {
                rect: Rect::new(5.0, 5.0, 20.0, 15.0),
                radius: 3.0,
            },
        );
        let doc = with(
            &doc,
            ShapeKind::Mosaic {
                rect: Rect::new(18.0, 2.0, 15.0, 20.0),
                block: 4,
            },
        );
        let doc = with(
            &doc,
            ShapeKind::Line {
                from: Point::new(0.0, 0.0),
                to: Point::new(40.0, 30.0),
            },
        );
        let full = render(&doc, None).unwrap();
        let part = render_region(&doc, None, Rect::new(10.0, 8.0, 12.0, 6.0)).unwrap();
        assert_eq!(
            (part.x, part.y, part.width(), part.height()),
            (10, 8, 12, 6)
        );
        let full_rgba = full.to_rgba();
        let part_rgba = part.to_rgba();
        for row in 0..6 {
            let start = ((8 + row) * 40 + 10) * 4;
            assert_eq!(
                &full_rgba[start..start + 48],
                &part_rgba[row * 48..row * 48 + 48],
                "row {row}"
            );
        }
    }

    #[test]
    fn invalid_regions_are_refused() {
        let doc = document();
        for region in [
            Rect::new(50.0, 50.0, 5.0, 5.0),
            Rect {
                x: f32::NAN,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        ] {
            assert_eq!(
                render_region(&doc, None, region).err(),
                Some(AnnotateError::InvalidRegion)
            );
        }
    }

    #[test]
    fn pixel_conversions() {
        let doc = Document::new(BaseImage::from_rgba(1, 1, vec![255, 0, 0, 128], 1.0).unwrap());
        let rendered = render(&doc, None).unwrap();
        assert!(!rendered.is_opaque());
        assert_eq!(rendered.to_rgba(), [255, 0, 0, 128]);
        assert_eq!(rendered.to_rgb_on_white(), [255, 127, 127]);
        assert_eq!(to_pixel(-3.0), 0);
        assert_eq!(to_offset(-2.6), -3);
        assert_eq!(block_pixels(4, 2.0), 8);
    }
}
