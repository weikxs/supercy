//! Images made by Supercy itself: screenshots and edited (annotated) exports.
//!
//! The user asked for these explicitly, so they skip the clipboard rules (pause, ignored apps,
//! type switches) but not the size limits: a screenshot may be up to 20 MB (decision D-10)
//! instead of the usual image limit, and it counts toward the total for images like any other.
//! An edited export always becomes a new item; the item it was made from is left as it was.

use cy_filter::{Capture, DropReason};
use cy_model::{ImageFormat, Representation, SecretBytes, Sensitivity, SourceApp};

use crate::{Engine, EngineError, IngestOutcome, IngestReport};

/// Largest screenshot or edited export kept, in bytes (D-10).
pub const MAX_SCREENSHOT_BYTES: usize = 20 * 1024 * 1024;
/// The source recorded on screenshot items; the shell shows a localized name for it.
pub const SCREENSHOT_SOURCE_ID: &str = "io.github.weikxs.supercy.screenshot";
/// The source recorded on edited exports.
pub const ANNOTATION_SOURCE_ID: &str = "io.github.weikxs.supercy.annotation";

/// The PNG file signature.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
/// The start of every JPEG file.
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];

/// Where an image made by Supercy comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOrigin {
    /// A screen capture.
    Screenshot,
    /// An annotated image exported from the editor.
    Edited,
}

/// An encoded image made by Supercy, with the thumbnail the shell made for the list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OwnImage {
    /// PNG or JPEG.
    pub format: ImageFormat,
    /// The encoded image.
    pub bytes: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// A small PNG for the history list, if the shell made one.
    pub thumbnail: Option<Vec<u8>>,
    /// Screenshot or edited export.
    pub origin: ImageOrigin,
}

impl OwnImage {
    /// Why it cannot be kept, if it cannot.
    fn refusal(&self) -> Option<DropReason> {
        let signature = match self.format {
            ImageFormat::Png => PNG_SIGNATURE,
            ImageFormat::Jpeg => JPEG_SIGNATURE,
            ImageFormat::Tiff | ImageFormat::Gif => return Some(DropReason::Unsupported),
        };
        if !self.bytes.starts_with(signature) || self.width == 0 || self.height == 0 {
            return Some(DropReason::Unsupported);
        }
        (self.bytes.len() > MAX_SCREENSHOT_BYTES).then_some(DropReason::TooLarge)
    }

    fn source(&self) -> SourceApp {
        let (bundle_id, name) = match self.origin {
            ImageOrigin::Screenshot => (SCREENSHOT_SOURCE_ID, "Supercy Screenshot"),
            ImageOrigin::Edited => (ANNOTATION_SOURCE_ID, "Supercy Annotation"),
        };
        SourceApp {
            bundle_id: bundle_id.into(),
            name: name.into(),
        }
    }
}

impl Engine {
    /// Stores a screenshot or an edited export as an image item (or moves the identical image
    /// already in the history to the top).
    ///
    /// # Errors
    ///
    /// Storage failures; the history is unchanged then.
    pub fn ingest_image(
        &mut self,
        image: OwnImage,
        now_ms: u64,
    ) -> Result<IngestReport, EngineError> {
        let outcome = if let Some(reason) = image.refusal() {
            IngestOutcome::Dropped { reason }
        } else {
            let source = image.source();
            let capture = Capture {
                reps: vec![Representation::Image {
                    format: image.format,
                    width: image.width,
                    height: image.height,
                    bytes: SecretBytes::new(image.bytes),
                }],
                truncated: false,
                source: Some(source),
                sensitivity: Sensitivity::Normal,
            };
            self.keep(capture, now_ms)?
        };
        if let (IngestOutcome::Stored { id, .. } | IngestOutcome::Updated { id }, Some(thumbnail)) =
            (&outcome, &image.thumbnail)
        {
            self.save_thumbnail(*id, thumbnail)?;
        }
        Ok(IngestReport {
            outcome,
            next_tick_ms: self.next_tick(),
        })
    }
}
