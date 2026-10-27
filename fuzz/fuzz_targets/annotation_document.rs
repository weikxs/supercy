#![no_main]
//! Decoding a saved annotation never panics, whatever the bytes.
use std::sync::OnceLock;

use cy_annotate::{BaseImage, Document};
use libfuzzer_sys::fuzz_target;

/// The image the saved bytes are decoded over (decoding checks that its size matches).
fn base() -> Option<&'static BaseImage> {
    static BASE: OnceLock<Option<BaseImage>> = OnceLock::new();
    BASE.get_or_init(|| BaseImage::from_rgba(64, 48, vec![200; 64 * 48 * 4], 2.0).ok())
        .as_ref()
}

fuzz_target!(|bytes: &[u8]| {
    if let Some(base) = base() {
        let _ = Document::decode(bytes, base.clone());
    }
});
