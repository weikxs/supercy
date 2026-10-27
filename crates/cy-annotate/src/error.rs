//! Annotation errors. Messages describe what is wrong and never contain the user's text or pixels.

/// What went wrong while editing, rendering or exporting an annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AnnotateError {
    /// The base image could not be decoded, or it is empty.
    #[error("the image could not be read; only PNG images are supported")]
    InvalidImage,
    /// The base image is larger than the editor accepts.
    #[error("the image is too large to edit")]
    ImageTooLarge,
    /// A coordinate or size is not a finite number, or is out of range.
    #[error("a position or size is out of range")]
    InvalidGeometry,
    /// A style value (stroke width, opacity, font size, block size, radius) is out of range.
    #[error("a style value is out of range")]
    InvalidStyle,
    /// No shape has this id.
    #[error("the shape no longer exists")]
    UnknownShape,
    /// This shape cannot be resized with this handle.
    #[error("this shape cannot be resized that way")]
    InvalidHandle,
    /// Only text shapes have text.
    #[error("only text shapes have text")]
    NotText,
    /// Text is empty or longer than the limit.
    #[error("the text is empty or too long")]
    InvalidText,
    /// A limit (shapes, pen points, step numbers) would be exceeded.
    #[error("the annotation has too many shapes or points")]
    LimitExceeded,
    /// The crop box does not overlap the image.
    #[error("the crop box is outside the image")]
    InvalidCrop,
    /// An encoded document is damaged, too large or from a newer Supercy.
    #[error("the saved annotation is damaged or from a newer Supercy")]
    InvalidDocument,
    /// Font data could not be read.
    #[error("the font could not be read")]
    InvalidFont,
    /// A region to render is empty or outside the image.
    #[error("the region is empty or outside the image")]
    InvalidRegion,
    /// JPEG quality is 1 to 100.
    #[error("JPEG quality must be from 1 to 100")]
    InvalidQuality,
    /// Encoding the exported image failed.
    #[error("the image could not be encoded")]
    Encode,
    /// A file name template or target folder is not usable.
    #[error("the file name or folder is not valid")]
    InvalidFileName,
}
