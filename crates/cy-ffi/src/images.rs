//! Screenshots and edited exports entering the history (M4-06).

use cy_model as model;

use crate::{ImageFormat, IngestResult, SupercyEngine, SupercyError};

/// Where an image made by Supercy comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ImageOrigin {
    /// A screen capture.
    Screenshot,
    /// An annotated image exported from the editor (always a new item).
    Edited,
}

/// An encoded image made by Supercy.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CapturedImage {
    /// PNG or JPEG (others are refused as unsupported).
    pub format: ImageFormat,
    /// The encoded image, at most `max_screenshot_bytes()`.
    pub bytes: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// A small PNG for the history list.
    pub thumbnail: Option<Vec<u8>>,
    /// Screenshot or edited export.
    pub origin: ImageOrigin,
}

impl From<ImageFormat> for model::ImageFormat {
    fn from(format: ImageFormat) -> Self {
        match format {
            ImageFormat::Png => Self::Png,
            ImageFormat::Tiff => Self::Tiff,
            ImageFormat::Jpeg => Self::Jpeg,
            ImageFormat::Gif => Self::Gif,
        }
    }
}

impl From<CapturedImage> for cy_engine::OwnImage {
    fn from(image: CapturedImage) -> Self {
        Self {
            format: image.format.into(),
            bytes: image.bytes,
            width: image.width,
            height: image.height,
            thumbnail: image.thumbnail,
            origin: match image.origin {
                ImageOrigin::Screenshot => cy_engine::ImageOrigin::Screenshot,
                ImageOrigin::Edited => cy_engine::ImageOrigin::Edited,
            },
        }
    }
}

#[uniffi::export]
impl SupercyEngine {
    /// Stores a screenshot or an edited export as an image item. Kept while recording is paused
    /// and regardless of the clipboard rules (the user asked for it), up to 20 MB (D-10).
    ///
    /// # Errors
    ///
    /// `Internal` when storage fails.
    pub fn ingest_image(
        &self,
        image: CapturedImage,
        now_ms: u64,
    ) -> Result<IngestResult, SupercyError> {
        Ok(self.engine()?.ingest_image(image.into(), now_ms)?.into())
    }
}

/// Largest screenshot or edited export the history keeps, in bytes.
#[uniffi::export]
#[must_use]
pub fn max_screenshot_bytes() -> u64 {
    cy_engine::MAX_SCREENSHOT_BYTES as u64
}

/// The source bundle id recorded on screenshot items (show a localized name for it).
#[uniffi::export]
#[must_use]
pub fn screenshot_source_id() -> String {
    cy_engine::SCREENSHOT_SOURCE_ID.to_owned()
}

/// The source bundle id recorded on edited exports.
#[uniffi::export]
#[must_use]
pub fn annotation_source_id() -> String {
    cy_engine::ANNOTATION_SOURCE_ID.to_owned()
}
