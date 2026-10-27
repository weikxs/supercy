#![no_main]
//! Decoding a stored item body never panics, whatever the bytes.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = cy_store::fuzzing::decode_body(bytes);
});
