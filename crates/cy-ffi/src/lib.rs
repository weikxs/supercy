//! UniFFI bindings of the Supercy engine.
//!
//! The interface is coarse on purpose (design 04): the shell hands over clipboard snapshots and
//! user actions and gets back summaries and paste payloads. Item ids cross the boundary as
//! 32-digit hexadecimal strings. The engine never calls back on its own; the only callback is
//! the [`Keystore`] the shell implements over the system keychain.

uniffi::setup_scaffolding!();

mod engine;
mod error;
mod keystore;
mod settings;
mod types;

pub use engine::{
    PauseStatus, SupercyEngine, check_ignore_pattern, core_version, query_error, thumbnail_type,
};
pub use error::{KeystoreError, SupercyError};
pub use keystore::Keystore;
pub use settings::{Language, MenuIcon, PinPosition, PopupPosition, Settings, SortOrder};
pub use types::{
    ClipEntry, ClipboardSnapshot, DropReason, ImageFormat, IngestOutcome, IngestResult, ItemKind,
    ItemSummary, PasteRepresentation, SearchMode, SourceApp, TextRange,
};
