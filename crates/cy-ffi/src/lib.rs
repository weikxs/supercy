//! UniFFI bindings of the Supercy engine.
//!
//! The interface is coarse on purpose (design 04): the shell hands over clipboard snapshots and
//! user actions and gets back summaries and paste payloads. Item ids cross the boundary as
//! 32-digit hexadecimal strings. The engine never calls back on its own; the only callback is
//! the [`Keystore`] the shell implements over the system keychain.
//!
//! The screenshot editor has its own object, [`AnnotationDocument`] (design 05).

uniffi::setup_scaffolding!();

mod annotation;
mod annotation_types;
mod engine;
mod error;
mod images;
mod keystore;
mod settings;
mod types;

pub use annotation::{
    AnnotationDocument, AnnotationError, AnnotationStyleConstants, annotation_file_name,
    annotation_style_constants, annotation_target_path,
};
pub use annotation_types::{
    AnnotationColor, AnnotationCommand, AnnotationPoint, AnnotationRect, AnnotationShape,
    AnnotationShapeKind, AnnotationStyle, ExportImageFormat, FileNameParts, RenderedRegion,
    ResizeHandle,
};
pub use engine::{
    PauseStatus, SupercyEngine, check_ignore_pattern, core_version, query_error, thumbnail_type,
};
pub use error::{KeystoreError, SupercyError};
pub use images::{
    CapturedImage, ImageOrigin, annotation_source_id, max_screenshot_bytes, screenshot_source_id,
};
pub use keystore::Keystore;
pub use settings::{Language, MenuIcon, PinPosition, PopupPosition, Settings, SortOrder};
pub use types::{
    ClipEntry, ClipboardSnapshot, DropReason, ImageFormat, IngestOutcome, IngestResult, ItemKind,
    ItemSummary, PasteRepresentation, PinsImport, SearchMode, SourceApp, TextRange,
};
