//! The annotation core of Supercy's screenshot editor (design 05).
//!
//! The core owns the model and the final pixels; the shell owns the window and the live
//! preview (ADR-10):
//!
//! - [`Document`]: the base image, shapes in drawing order and a crop box. A document is a value;
//!   [`Command`]s return a new one and share everything unchanged.
//! - [`Editor`]: the current document with undo and redo, the font, hit testing and selection
//!   bounds.
//! - [`render_region`] and [`export`]: compositing with tiny-skia, pixel effects (mosaic, blur),
//!   cropping and PNG or JPEG encoding.
//! - [`STYLE_CONSTANTS`]: the drawing parameters the shell's preview uses to match the export.
//!
//! Coordinates are base-image pixels; style sizes are points, multiplied by the image's scale.
//! Every input is checked against the [`limits`], so damaged or hostile input fails with an
//! [`AnnotateError`] instead of exhausting memory or panicking.

mod codec;
mod command;
mod document;
mod editor;
mod error;
mod export;
mod geometry;
mod jpeg;
mod jpeg_tables;
pub mod limits;
mod render;
mod shape;
mod style;
mod text;

pub use codec::DOCUMENT_FORMAT_VERSION;
pub use command::{Applied, Arrange, Command};
pub use document::{BaseImage, Document};
pub use editor::Editor;
pub use error::AnnotateError;
pub use export::{
    ExportFormat, FALLBACK_FILE_NAME, FileNameContext, MAX_FILE_NAME_BYTES, encode_jpeg,
    encode_png, expand_file_name, export, target_path,
};
pub use geometry::{Handle, Point, Rect};
pub use render::{Rendered, render, render_region};
pub use shape::{Metrics, Shape, ShapeId, ShapeKind};
pub use style::{
    ARROW_HEAD_HALF_ANGLE_DEGREES, ARROW_HEAD_LENGTH_PER_WIDTH, ARROW_HEAD_MIN_LENGTH,
    HIGHLIGHTER_ALPHA, HIGHLIGHTER_WIDTH_FACTOR, HIT_TOLERANCE, RECT_CORNER_RADIUS_PER_WIDTH, Rgba,
    STEP_BASE_RADIUS, STEP_FONT_SIZE_PER_RADIUS, STEP_RADIUS_PER_WIDTH, STYLE_CONSTANTS, Style,
    StyleConstants, TEXT_LINE_HEIGHT, arrow_head_length, step_radius,
};
pub use text::{Font, TextSize, measure_text};
