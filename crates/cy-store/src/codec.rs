//! The sealed part of an item, encoded as CBOR.

use ciborium::{from_reader, into_writer};
use cy_model::{Representation, SensitiveReason, SourceApp};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::StoreError;

/// Largest decoded body accepted; larger input is treated as damaged.
pub(crate) const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// The sealed fields, borrowed for encoding.
#[derive(Serialize)]
pub(crate) struct BodyRef<'a> {
    pub(crate) reps: &'a [Representation],
    pub(crate) source: Option<&'a SourceApp>,
    pub(crate) truncated: bool,
    pub(crate) sensitive: Option<SensitiveReason>,
}

/// The sealed fields, decoded.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub(crate) struct Body {
    pub(crate) reps: Vec<Representation>,
    pub(crate) source: Option<SourceApp>,
    pub(crate) truncated: bool,
    pub(crate) sensitive: Option<SensitiveReason>,
}

pub(crate) fn encode(body: &BodyRef<'_>) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    let mut out = Zeroizing::new(Vec::new());
    into_writer(body, &mut *out).map_err(|_| StoreError::Corrupt)?;
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Body, StoreError> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(StoreError::Corrupt);
    }
    let body: Body = from_reader(bytes).map_err(|_| StoreError::Corrupt)?;
    if body.reps.is_empty() {
        return Err(StoreError::Corrupt);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cy_model::{ImageFormat, SecretBytes, SecretString};
    use proptest::prelude::*;

    fn rep_strategy() -> impl Strategy<Value = Representation> {
        prop_oneof![
            ".*".prop_map(|t| Representation::PlainText(SecretString::new(t))),
            (
                ".*",
                proptest::option::of(".*"),
                proptest::option::of(any::<Vec<u8>>())
            )
                .prop_map(|(plain, html, rtf)| {
                    Representation::RichText {
                        rtf: rtf.map(SecretBytes::new),
                        html: html.map(SecretString::new),
                        plain: SecretString::new(plain),
                    }
                }),
            (".*", proptest::option::of(".*")).prop_map(|(url, title)| Representation::Url {
                url: SecretString::new(url),
                title: title.map(SecretString::new),
            }),
            (any::<u32>(), any::<u32>(), any::<Vec<u8>>()).prop_map(|(width, height, bytes)| {
                Representation::Image {
                    format: ImageFormat::Png,
                    width,
                    height,
                    bytes: SecretBytes::new(bytes),
                }
            }),
            proptest::collection::vec(".*", 0..3)
                .prop_map(|names| Representation::FileRef { names }),
        ]
    }

    proptest! {
        #[test]
        fn bodies_round_trip(reps in proptest::collection::vec(rep_strategy(), 1..4), truncated in any::<bool>()) {
            let source = SourceApp { bundle_id: "com.example.editor".into(), name: "Demo Editor".into() };
            let encoded = encode(&BodyRef { reps: &reps, source: Some(&source), truncated, sensitive: None }).unwrap();

            let decoded = decode(&encoded).unwrap();

            prop_assert_eq!(decoded, Body { reps, source: Some(source), truncated, sensitive: None });
        }

        #[test]
        fn decoding_garbage_never_panics(bytes in any::<Vec<u8>>()) {
            let _ = decode(&bytes);
        }
    }

    #[test]
    fn empty_representation_lists_are_damaged() {
        let encoded = encode(&BodyRef {
            reps: &[],
            source: None,
            truncated: false,
            sensitive: None,
        })
        .unwrap();

        assert!(matches!(decode(&encoded), Err(StoreError::Corrupt)));
    }
}
