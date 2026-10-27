//! Limits that keep a damaged or hostile document, or a runaway gesture, from exhausting memory.

/// Most shapes in one document.
pub const MAX_SHAPES: usize = 1_000;
/// Most points in one pen or highlighter stroke (the shell simplifies strokes before adding).
pub const MAX_PEN_POINTS: usize = 10_000;
/// Most characters in one text shape.
pub const MAX_TEXT_CHARS: usize = 2_000;
/// Largest encoded document accepted by the decoder.
pub const MAX_ENCODED_BYTES: usize = 4 * 1024 * 1024;
/// Largest absolute coordinate, in base-image pixels.
pub const MAX_COORDINATE: f32 = 100_000.0;
/// Longest side of a base image, in pixels.
pub const MAX_IMAGE_SIDE: u32 = 16_384;
/// Most pixels in a base image (an 8K by 5K screenshot fits).
pub const MAX_IMAGE_PIXELS: u64 = 48_000_000;
/// Largest font file the shell may hand over (system fonts and collections are large).
pub const MAX_FONT_BYTES: usize = 128 * 1024 * 1024;
/// Highest step number.
pub const MAX_STEP_NUMBER: u32 = 999;
/// Undo steps kept; the oldest are dropped beyond this.
pub const MAX_UNDO_STEPS: usize = 200;
/// Most points in an eraser path.
pub const MAX_ERASE_POINTS: usize = 10_000;

/// Thinnest stroke, in points.
pub const MIN_STROKE_WIDTH: f32 = 0.5;
/// Thickest stroke, in points.
pub const MAX_STROKE_WIDTH: f32 = 100.0;
/// Smallest font size, in points.
pub const MIN_FONT_SIZE: f32 = 4.0;
/// Largest font size, in points.
pub const MAX_FONT_SIZE: f32 = 512.0;
/// Smallest mosaic block, in points.
pub const MIN_MOSAIC_BLOCK: u32 = 2;
/// Largest mosaic block, in points.
pub const MAX_MOSAIC_BLOCK: u32 = 256;
/// Smallest blur radius, in points.
pub const MIN_BLUR_RADIUS: f32 = 1.0;
/// Largest blur radius, in points.
pub const MAX_BLUR_RADIUS: f32 = 100.0;
/// Smallest base-image scale (points to pixels).
pub const MIN_SCALE: f32 = 0.5;
/// Largest base-image scale.
pub const MAX_SCALE: f32 = 4.0;
/// Largest eraser radius, in points.
pub const MAX_ERASE_RADIUS: f32 = 200.0;
