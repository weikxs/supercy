//! Thumbnails of image items, made once by the shell when the image is copied and stored with
//! the item, so showing the history never decodes full images.

use cy_model::{ClipboardSnapshot, ItemId, RepKind};
use cy_store::MAX_THUMBNAIL_BYTES;

use crate::{Engine, EngineError, IngestOutcome};

/// The clipboard type under which the shell passes an image's thumbnail (PNG) with a snapshot.
/// It is never a pasteboard type; the capture filter ignores it.
pub const THUMBNAIL_TYPE: &str = "io.github.weikxs.supercy.thumbnail";

impl Engine {
    /// Keeps the thumbnail that came with `snapshot`, if the outcome is an image item.
    pub(crate) fn keep_thumbnail(
        &mut self,
        snapshot: &ClipboardSnapshot,
        outcome: &IngestOutcome,
    ) -> Result<(), EngineError> {
        let id = match outcome {
            IngestOutcome::Stored { id, .. } | IngestOutcome::Updated { id } => *id,
            _ => return Ok(()),
        };
        if self.summaries.get(&id).map(|s| s.kind) != Some(RepKind::Image) {
            return Ok(());
        }
        let thumbnail = snapshot
            .entries
            .iter()
            .find(|entry| entry.type_id == THUMBNAIL_TYPE)
            .and_then(|entry| entry.data.as_ref())
            .filter(|data| !data.is_empty() && data.len() <= MAX_THUMBNAIL_BYTES);
        if let Some(data) = thumbnail {
            self.store.set_thumbnail(id, data.expose())?;
        }
        Ok(())
    }

    /// The stored thumbnail of item `id` (PNG), if it has one.
    ///
    /// # Errors
    ///
    /// Storage failures.
    pub fn thumbnail(&self, id: ItemId) -> Result<Option<Vec<u8>>, EngineError> {
        Ok(self
            .store
            .thumbnail(id)?
            .map(|bytes| bytes.expose().to_vec()))
    }
}
