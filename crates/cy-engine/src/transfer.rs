//! Pinned items to and from a file, to keep them or move them to another Mac.
//!
//! The file is JSON (`supercy-pins`, version 1) and NOT encrypted: the shell says so before
//! exporting. Images and RTF are base64. Only pinned items are written; their order and letter
//! keys come back on import, except a letter another pinned item already uses.

use std::io::Write;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use cy_model::{
    Fingerprint, ImageFormat, Item, ItemId, Representation, SecretBytes, SecretString, Sensitivity,
};
use serde::{Deserialize, Serialize};

use crate::{Engine, EngineError};

/// The `format` field of a pins file.
pub const PINS_FORMAT: &str = "supercy-pins";
/// The newest pins file version this build reads and the one it writes.
pub const PINS_VERSION: u32 = 1;
/// Largest pins file read, in bytes.
pub const MAX_PINS_FILE_BYTES: u64 = 256 << 20;

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PinsImport {
    /// New pinned items.
    pub added: usize,
    /// Items already in the history, now pinned.
    pub pinned_existing: usize,
    /// Items that were pinned already.
    pub already_pinned: usize,
    /// Entries without content.
    pub skipped: usize,
}

#[derive(Serialize, Deserialize)]
struct PinsFile {
    format: String,
    version: u32,
    items: Vec<PinnedEntry>,
}

#[derive(Serialize, Deserialize)]
struct PinnedEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<char>,
    content: Vec<Content>,
}

/// One form of an item, as written to the file.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Content {
    Text {
        text: String,
    },
    RichText {
        plain: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        html: Option<String>,
        /// Base64.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rtf: Option<String>,
    },
    Url {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Image {
        format: ImageName,
        width: u32,
        height: u32,
        /// Base64.
        data: String,
    },
    Files {
        names: Vec<String>,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ImageName {
    Png,
    Tiff,
    Jpeg,
    Gif,
}

impl From<ImageFormat> for ImageName {
    fn from(format: ImageFormat) -> Self {
        match format {
            ImageFormat::Png => Self::Png,
            ImageFormat::Tiff => Self::Tiff,
            ImageFormat::Jpeg => Self::Jpeg,
            ImageFormat::Gif => Self::Gif,
        }
    }
}

impl From<ImageName> for ImageFormat {
    fn from(name: ImageName) -> Self {
        match name {
            ImageName::Png => Self::Png,
            ImageName::Tiff => Self::Tiff,
            ImageName::Jpeg => Self::Jpeg,
            ImageName::Gif => Self::Gif,
        }
    }
}

impl Content {
    fn of(rep: &Representation) -> Self {
        match rep {
            Representation::PlainText(text) => Self::Text {
                text: text.expose().to_owned(),
            },
            Representation::RichText { rtf, html, plain } => Self::RichText {
                plain: plain.expose().to_owned(),
                html: html.as_ref().map(|h| h.expose().to_owned()),
                rtf: rtf.as_ref().map(|r| BASE64.encode(r.expose())),
            },
            Representation::Url { url, title } => Self::Url {
                url: url.expose().to_owned(),
                title: title.as_ref().map(|t| t.expose().to_owned()),
            },
            Representation::Image {
                format,
                width,
                height,
                bytes,
            } => Self::Image {
                format: (*format).into(),
                width: *width,
                height: *height,
                data: BASE64.encode(bytes.expose()),
            },
            Representation::FileRef { names } => Self::Files {
                names: names.clone(),
            },
        }
    }

    fn into_rep(self) -> Result<Representation, EngineError> {
        let decode = |data: &str| {
            BASE64
                .decode(data)
                .map(SecretBytes::new)
                .map_err(|_| EngineError::InvalidPinsFile)
        };
        Ok(match self {
            Self::Text { text } => Representation::PlainText(SecretString::new(text)),
            Self::RichText { plain, html, rtf } => Representation::RichText {
                rtf: rtf.as_deref().map(decode).transpose()?,
                html: html.map(SecretString::new),
                plain: SecretString::new(plain),
            },
            Self::Url { url, title } => Representation::Url {
                url: SecretString::new(url),
                title: title.map(SecretString::new),
            },
            Self::Image {
                format,
                width,
                height,
                data,
            } => Representation::Image {
                format: format.into(),
                width,
                height,
                bytes: decode(&data)?,
            },
            Self::Files { names } => Representation::FileRef { names },
        })
    }
}

impl Engine {
    /// Writes the pinned items, in list order, to `path` (readable by this user only); returns
    /// how many were written. Unreadable items are left out.
    ///
    /// # Errors
    ///
    /// [`EngineError::File`] when the file cannot be written; storage failures.
    pub fn export_pins(&self, path: &Path) -> Result<usize, EngineError> {
        let mut items = Vec::new();
        for id in self.pinned_order() {
            if let Ok(Some(item)) = self.store.get(id) {
                items.push(PinnedEntry {
                    key: item.pin_key,
                    content: item.reps.iter().map(Content::of).collect(),
                });
            }
        }
        let count = items.len();
        let file = PinsFile {
            format: PINS_FORMAT.to_owned(),
            version: PINS_VERSION,
            items,
        };
        let json = serde_json::to_vec_pretty(&file).map_err(|_| EngineError::Internal)?;
        write_private(path, &json)?;
        Ok(count)
    }

    /// Reads pinned items from `path` and pins them here, after the pinned items already here
    /// and in the file's order. Content already in the history is pinned rather than added
    /// again. The whole file is checked before anything changes.
    ///
    /// # Errors
    ///
    /// [`EngineError::File`] when it cannot be read, [`EngineError::InvalidPinsFile`] when it
    /// is not a pins file this build reads (or is too large); storage failures.
    pub fn import_pins(&mut self, path: &Path, now_ms: u64) -> Result<PinsImport, EngineError> {
        let entries = read_pins_file(path)?;
        let mut report = PinsImport::default();
        for (key, reps) in entries {
            if reps.is_empty() {
                report.skipped += 1;
                continue;
            }
            self.import_one(key, reps, now_ms, &mut report)?;
        }
        Ok(report)
    }

    fn import_one(
        &mut self,
        key: Option<char>,
        reps: Vec<Representation>,
        now_ms: u64,
        report: &mut PinsImport,
    ) -> Result<(), EngineError> {
        let fingerprint =
            Fingerprint::of(self.store.fingerprint_key(), &reps).ok_or(EngineError::Internal)?;
        if let Some(id) = self.store.find_by_fingerprint(&fingerprint)? {
            if self.summaries.get(&id).is_some_and(|s| s.pinned) {
                report.already_pinned += 1;
            } else {
                self.set_pinned(id, true, now_ms)?;
                report.pinned_existing += 1;
            }
            return Ok(());
        }
        let key = key
            .filter(|k| self.item_for_pin_key(*k).is_none())
            .or_else(|| self.free_pin_key());
        let at = self.stamp(now_ms);
        let item = Item {
            id: ItemId::random().map_err(|_| EngineError::Random)?,
            fingerprint,
            reps,
            source: None,
            origin: self.device(),
            first_copied_ms: now_ms,
            last_used: at,
            pinned: true,
            pinned_at: Some(at),
            sensitivity: Sensitivity::Normal,
            truncated: false,
            copy_count: 1,
            pin_key: key,
        };
        let evicted = self.store.save(&item, &self.settings().limits())?;
        self.remember(&item);
        self.forget(&evicted);
        report.added += 1;
        Ok(())
    }
}

/// One checked entry of a pins file: its letter key and its content.
type ImportEntry = (Option<char>, Vec<Representation>);

/// The entries of the pins file at `path`, checked: each with a valid key (`a`–`z`) and its
/// content decoded.
fn read_pins_file(path: &Path) -> Result<Vec<ImportEntry>, EngineError> {
    let size = std::fs::metadata(path)
        .map_err(|e| EngineError::File(e.kind()))?
        .len();
    if size > MAX_PINS_FILE_BYTES {
        return Err(EngineError::InvalidPinsFile);
    }
    let bytes = std::fs::read(path).map_err(|e| EngineError::File(e.kind()))?;
    let file: PinsFile =
        serde_json::from_slice(&bytes).map_err(|_| EngineError::InvalidPinsFile)?;
    if file.format != PINS_FORMAT || file.version > PINS_VERSION {
        return Err(EngineError::InvalidPinsFile);
    }
    file.items
        .into_iter()
        .map(|entry| {
            if entry.key.is_some_and(|k| !k.is_ascii_lowercase()) {
                return Err(EngineError::InvalidPinsFile);
            }
            let reps = entry
                .content
                .into_iter()
                .map(Content::into_rep)
                .collect::<Result<Vec<_>, _>>()?;
            Ok((entry.key, reps))
        })
        .collect()
}

/// Writes `bytes` to `path` through a temporary file in the same folder, readable by this user
/// only, so a failed write never leaves half a file.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), EngineError> {
    let file_error = |e: std::io::Error| EngineError::File(e.kind());
    let name = path
        .file_name()
        .ok_or(EngineError::File(std::io::ErrorKind::InvalidInput))?;
    let temporary = path.with_file_name(format!(".{}.partial", name.to_string_lossy()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options
        .open(&temporary)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(file_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_content_survives_the_file_form() {
        let reps = vec![
            Representation::PlainText(SecretString::from("demo")),
            Representation::RichText {
                rtf: Some(SecretBytes::from(&b"{\\rtf1 demo}"[..])),
                html: Some(SecretString::from("<b>demo</b>")),
                plain: SecretString::from("demo"),
            },
            Representation::Url {
                url: SecretString::from("https://example.com/"),
                title: None,
            },
            Representation::Image {
                format: ImageFormat::Gif,
                width: 2,
                height: 3,
                bytes: SecretBytes::from(&[0u8, 1, 2, 255][..]),
            },
            Representation::FileRef {
                names: vec!["file:///tmp/demo.txt".into()],
            },
        ];
        let json = serde_json::to_string(&reps.iter().map(Content::of).collect::<Vec<_>>())
            .unwrap_or_default();
        let back: Vec<Content> = serde_json::from_str(&json).unwrap_or_default();
        let back: Vec<Representation> =
            back.into_iter().filter_map(|c| c.into_rep().ok()).collect();
        assert_eq!(back, reps);
    }
}
