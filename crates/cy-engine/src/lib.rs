//! The Supercy core service.
//!
//! [`Engine`] composes the model, the capture filter, the search index and the encrypted store:
//! a platform shell hands it clipboard snapshots and user actions, and asks it for lists,
//! search results and paste payloads. The engine never starts threads or timers: time-based work
//! (expiring sensitive items) runs when the shell calls [`Engine::tick`], at the moment the
//! engine said it would be due.

mod engine;
mod error;
mod settings;
mod summary;

pub use cy_filter::DropReason;
pub use cy_search::MatchMode;
pub use engine::{Engine, IngestOutcome, IngestReport};
pub use error::EngineError;
pub use settings::{DEFAULT_IGNORED_APPS, Settings, SettingsError};
pub use summary::{ItemSummary, PREVIEW_CHARS};
