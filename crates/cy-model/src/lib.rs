//! The Supercy history model: items, their representations, identifiers, clocks and fingerprints.
//!
//! Everything here is a plain value. There is no I/O and no clock; the only randomness is
//! [`ItemId::random`].

mod fingerprint;
mod hlc;
mod id;
mod item;
mod rep;
mod secret;
mod snapshot;

pub use fingerprint::{FINGERPRINT_LEN, Fingerprint, FingerprintKey, normalize_text, primary_rep};
pub use hlc::{DeviceTag, HLC_ENCODED_LEN, Hlc, HlcClock};
pub use id::{ITEM_ID_LEN, IdError, ItemId};
pub use item::{Item, SensitiveReason, Sensitivity, SourceApp};
pub use rep::{ImageFormat, RepKind, Representation};
pub use secret::{SecretBytes, SecretString};
pub use snapshot::{ClipEntry, ClipboardSnapshot};
