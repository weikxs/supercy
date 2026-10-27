//! The annotation document: a base image, shapes in drawing order, and a crop box.
//!
//! A document is a value. Commands return a new document that shares the base image and every
//! unchanged shape with the old one (`Arc`), so keeping old documents for undo costs only what
//! changed.

use std::io::Cursor;
use std::sync::Arc;

use tiny_skia::{IntSize, Pixmap};

use crate::AnnotateError;
use crate::geometry::Rect;
use crate::limits::{MAX_IMAGE_PIXELS, MAX_IMAGE_SIDE, MAX_SCALE, MIN_SCALE};
use crate::shape::{Shape, ShapeId};

/// Bytes per RGBA pixel.
const RGBA: usize = 4;

/// The screenshot being annotated, decoded once and shared by every version of the document.
#[derive(Clone)]
pub struct BaseImage {
    pixels: Arc<Pixmap>,
    scale: f32,
}

impl std::fmt::Debug for BaseImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaseImage")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

impl PartialEq for BaseImage {
    fn eq(&self, other: &Self) -> bool {
        self.scale.to_bits() == other.scale.to_bits()
            && (Arc::ptr_eq(&self.pixels, &other.pixels) || self.pixels == other.pixels)
    }
}

impl BaseImage {
    /// Decodes a PNG; `scale` is pixels per point (2 for a Retina screenshot).
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidImage`] for data that is not a PNG or a scale out of range;
    /// [`AnnotateError::ImageTooLarge`] above [`MAX_IMAGE_SIDE`] or [`MAX_IMAGE_PIXELS`].
    pub fn from_png(png: &[u8], scale: f32) -> Result<Self, AnnotateError> {
        let limits = png::Limits {
            bytes: usize::try_from(MAX_IMAGE_PIXELS).unwrap_or(usize::MAX) * RGBA,
        };
        let mut decoder = png::Decoder::new_with_limits(Cursor::new(png), limits);
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder
            .read_info()
            .map_err(|_| AnnotateError::InvalidImage)?;
        let (width, height) = (reader.info().width, reader.info().height);
        check_size(width, height)?;
        let mut buffer = vec![0; reader.output_buffer_size()];
        let frame = reader
            .next_frame(&mut buffer)
            .map_err(|_| AnnotateError::InvalidImage)?;
        let rgba = to_rgba(&buffer[..frame.buffer_size()], frame.color_type)?;
        Self::from_rgba(width, height, rgba, scale)
    }

    /// Wraps straight-alpha RGBA pixels, row by row.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidImage`] when the size does not match the data or the scale is
    /// out of range; [`AnnotateError::ImageTooLarge`] above the limits.
    pub fn from_rgba(
        width: u32,
        height: u32,
        mut rgba: Vec<u8>,
        scale: f32,
    ) -> Result<Self, AnnotateError> {
        check_size(width, height)?;
        if !(MIN_SCALE..=MAX_SCALE).contains(&scale) {
            return Err(AnnotateError::InvalidImage);
        }
        premultiply(&mut rgba);
        let size = IntSize::from_wh(width, height).ok_or(AnnotateError::InvalidImage)?;
        let pixels = Pixmap::from_vec(rgba, size).ok_or(AnnotateError::InvalidImage)?;
        Ok(Self {
            pixels: Arc::new(pixels),
            scale,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.pixels.width()
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.pixels.height()
    }

    /// Pixels per point.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The whole image as a rectangle.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        #[allow(clippy::cast_precision_loss, reason = "image sides are at most 16384")]
        Rect::new(0.0, 0.0, self.width() as f32, self.height() as f32)
    }

    /// The premultiplied pixels.
    pub(crate) fn pixmap(&self) -> &Pixmap {
        &self.pixels
    }
}

fn check_size(width: u32, height: u32) -> Result<(), AnnotateError> {
    if width == 0 || height == 0 {
        return Err(AnnotateError::InvalidImage);
    }
    let too_large = width > MAX_IMAGE_SIDE
        || height > MAX_IMAGE_SIDE
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS;
    if too_large {
        Err(AnnotateError::ImageTooLarge)
    } else {
        Ok(())
    }
}

/// Expands decoded 8-bit PNG rows to RGBA.
fn to_rgba(data: &[u8], color: png::ColorType) -> Result<Vec<u8>, AnnotateError> {
    let expand = |pixel: &[u8]| -> [u8; RGBA] {
        match *pixel {
            [g] => [g, g, g, 255],
            [g, a] => [g, g, g, a],
            [r, g, b] => [r, g, b, 255],
            [r, g, b, a] => [r, g, b, a],
            _ => [0; RGBA],
        }
    };
    let channels = match color {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(AnnotateError::InvalidImage),
    };
    Ok(data.chunks_exact(channels).flat_map(expand).collect())
}

/// Converts straight alpha to premultiplied alpha in place.
fn premultiply(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(RGBA) {
        let alpha = u16::from(pixel[3]);
        if alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = mul_div_255(u16::from(*channel), alpha);
            }
        }
    }
}

/// `a * b / 255`, rounded.
pub(crate) fn mul_div_255(a: u16, b: u16) -> u8 {
    let product = u32::from(a) * u32::from(b) + 128;
    u8::try_from((product + (product >> 8)) >> 8).unwrap_or(u8::MAX)
}

/// One version of an annotation.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub(crate) base: BaseImage,
    pub(crate) shapes: Vec<Arc<Shape>>,
    pub(crate) crop: Option<Rect>,
    pub(crate) next_step_number: u32,
    pub(crate) next_id: u64,
}

impl Document {
    /// An empty document over `base`.
    #[must_use]
    pub fn new(base: BaseImage) -> Self {
        Self {
            base,
            shapes: Vec::new(),
            crop: None,
            next_step_number: 1,
            next_id: 1,
        }
    }

    /// The base image.
    #[must_use]
    pub fn base(&self) -> &BaseImage {
        &self.base
    }

    /// The shapes, bottom first (later shapes are drawn on top).
    pub fn shapes(&self) -> impl DoubleEndedIterator<Item = &Shape> + ExactSizeIterator {
        self.shapes.iter().map(AsRef::as_ref)
    }

    /// The shape `id`.
    #[must_use]
    pub fn shape(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes().find(|shape| shape.id == id)
    }

    /// Position of `id` in drawing order.
    pub(crate) fn position(&self, id: ShapeId) -> Option<usize> {
        self.shapes.iter().position(|shape| shape.id == id)
    }

    /// The crop box applied on export, in pixels.
    #[must_use]
    pub fn crop(&self) -> Option<Rect> {
        self.crop
    }

    /// The area exported: the crop box, or the whole image.
    #[must_use]
    pub fn export_area(&self) -> Rect {
        self.crop.unwrap_or_else(|| self.base.bounds())
    }

    /// The number the next step marker gets.
    #[must_use]
    pub fn next_step_number(&self) -> u32 {
        self.next_step_number
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(
        color: png::ColorType,
        depth: png::BitDepth,
        data: &[u8],
        w: u32,
        h: u32,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(color);
        encoder.set_depth(depth);
        if color == png::ColorType::Indexed {
            encoder.set_palette(vec![10, 20, 30, 200, 100, 50]);
        }
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn decodes_every_png_colour_type_to_premultiplied_rgba() {
        let cases = [
            (png::ColorType::Grayscale, vec![100], [100, 100, 100, 255]),
            (png::ColorType::GrayscaleAlpha, vec![100, 0], [0, 0, 0, 0]),
            (png::ColorType::Rgb, vec![1, 2, 3], [1, 2, 3, 255]),
            (
                png::ColorType::Rgba,
                vec![200, 100, 50, 128],
                [100, 50, 25, 128],
            ),
            (png::ColorType::Indexed, vec![1], [200, 100, 50, 255]),
        ];
        for (color, data, expected) in cases {
            let png = png_bytes(color, png::BitDepth::Eight, &data, 1, 1);
            let base = BaseImage::from_png(&png, 2.0).unwrap();
            assert_eq!(base.pixmap().data(), &expected, "{color:?}");
            assert_eq!((base.width(), base.height(), base.scale()), (1, 1, 2.0));
        }
        let sixteen = png_bytes(
            png::ColorType::Rgb,
            png::BitDepth::Sixteen,
            &[255, 0, 0, 0, 0, 0],
            1,
            1,
        );
        assert_eq!(
            BaseImage::from_png(&sixteen, 1.0).unwrap().pixmap().data(),
            &[255, 0, 0, 255]
        );
    }

    #[test]
    fn rejects_bad_images_and_scales() {
        assert_eq!(
            BaseImage::from_png(b"not a png", 1.0).err(),
            Some(AnnotateError::InvalidImage)
        );
        let png = png_bytes(png::ColorType::Rgb, png::BitDepth::Eight, &[0, 0, 0], 1, 1);
        assert_eq!(
            BaseImage::from_png(&png[..png.len() - 20], 1.0).err(),
            Some(AnnotateError::InvalidImage)
        );
        assert_eq!(
            BaseImage::from_rgba(1, 1, vec![0; 4], 9.0).err(),
            Some(AnnotateError::InvalidImage)
        );
        assert_eq!(
            BaseImage::from_rgba(2, 1, vec![0; 4], 1.0).err(),
            Some(AnnotateError::InvalidImage)
        );
        assert_eq!(
            BaseImage::from_rgba(0, 1, vec![], 1.0).err(),
            Some(AnnotateError::InvalidImage)
        );
        assert_eq!(
            BaseImage::from_rgba(MAX_IMAGE_SIDE + 1, 1, vec![], 1.0).err(),
            Some(AnnotateError::ImageTooLarge)
        );
        assert_eq!(
            BaseImage::from_rgba(10_000, 10_000, vec![], 1.0).err(),
            Some(AnnotateError::ImageTooLarge)
        );
        assert_eq!(
            to_rgba(&[0], png::ColorType::Indexed),
            Err(AnnotateError::InvalidImage)
        );
    }

    #[test]
    fn equality_and_accessors() {
        let a = BaseImage::from_rgba(2, 1, vec![9; 8], 1.0).unwrap();
        let b = BaseImage::from_rgba(2, 1, vec![9; 8], 1.0).unwrap();
        let c = BaseImage::from_rgba(2, 1, vec![9; 8], 2.0).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(format!("{a:?}").contains("width: 2"));
        let document = Document::new(a);
        assert_eq!(document.export_area(), Rect::new(0.0, 0.0, 2.0, 1.0));
        assert_eq!(document.next_step_number(), 1);
        assert_eq!(document.shapes().len(), 0);
        assert!(document.shape(ShapeId(1)).is_none());
        assert_eq!(mul_div_255(255, 255), 255);
        assert_eq!(mul_div_255(0, 255), 0);
    }
}
