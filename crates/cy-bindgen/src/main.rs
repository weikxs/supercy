//! UniFFI's binding generator, pinned to the workspace's UniFFI version.
//!
//! `scripts/docker-test.sh bindings-write` runs it against the built `cy-ffi` library.

fn main() {
    uniffi::uniffi_bindgen_main();
}
