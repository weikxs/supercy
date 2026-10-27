//! The saved form of a document: CBOR with a format version, without the base image's pixels
//! (the image is stored with the history item; the document records its size to match it).

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::AnnotateError;
use crate::document::{BaseImage, Document};
use crate::geometry::Rect;
use crate::limits::{MAX_ENCODED_BYTES, MAX_SHAPES, MAX_STEP_NUMBER};
use crate::shape::Shape;

/// The format version written by this Supercy.
pub const DOCUMENT_FORMAT_VERSION: u16 = 1;

#[derive(Serialize)]
struct EncodedRef<'a> {
    version: u16,
    width: u32,
    height: u32,
    scale: f32,
    shapes: Vec<&'a Shape>,
    crop: Option<Rect>,
    next_step_number: u32,
    next_id: u64,
}

#[derive(Deserialize)]
struct Encoded {
    version: u16,
    width: u32,
    height: u32,
    scale: f32,
    shapes: Vec<Shape>,
    crop: Option<Rect>,
    next_step_number: u32,
    next_id: u64,
}

impl Document {
    /// Encodes the shapes, crop box and counters.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidDocument`] if the encoding exceeds [`MAX_ENCODED_BYTES`].
    pub fn encode(&self) -> Result<Vec<u8>, AnnotateError> {
        let encoded = EncodedRef {
            version: DOCUMENT_FORMAT_VERSION,
            width: self.base.width(),
            height: self.base.height(),
            scale: self.base.scale(),
            shapes: self.shapes().collect(),
            crop: self.crop,
            next_step_number: self.next_step_number,
            next_id: self.next_id,
        };
        let mut out = Vec::new();
        ciborium::into_writer(&encoded, &mut out).map_err(|_| AnnotateError::InvalidDocument)?;
        if out.len() > MAX_ENCODED_BYTES {
            return Err(AnnotateError::InvalidDocument);
        }
        Ok(out)
    }

    /// Decodes a saved document over `base`, the image it was made on.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidDocument`] for damaged, oversized or newer data, a document made
    /// for an image of another size, or shapes that fail validation.
    pub fn decode(bytes: &[u8], base: BaseImage) -> Result<Self, AnnotateError> {
        if bytes.len() > MAX_ENCODED_BYTES {
            return Err(AnnotateError::InvalidDocument);
        }
        let encoded: Encoded =
            ciborium::from_reader(bytes).map_err(|_| AnnotateError::InvalidDocument)?;
        let matches_base = encoded.version == DOCUMENT_FORMAT_VERSION
            && encoded.width == base.width()
            && encoded.height == base.height()
            && encoded.scale.to_bits() == base.scale().to_bits();
        if !matches_base || !counters_valid(&encoded) {
            return Err(AnnotateError::InvalidDocument);
        }
        for shape in &encoded.shapes {
            shape
                .kind
                .validate()
                .map_err(|_| AnnotateError::InvalidDocument)?;
            shape
                .style
                .validate()
                .map_err(|_| AnnotateError::InvalidDocument)?;
        }
        let document = Self {
            shapes: encoded.shapes.into_iter().map(Arc::new).collect(),
            ..Self::new(base)
        };
        let crop_ok = encoded.crop.is_none_or(|crop| {
            crop.is_valid() && crop.intersection(&document.base.bounds()) == Some(crop)
        });
        if !crop_ok {
            return Err(AnnotateError::InvalidDocument);
        }
        Ok(Self {
            crop: encoded.crop,
            next_step_number: encoded.next_step_number,
            next_id: encoded.next_id,
            ..document
        })
    }
}

/// Shape count within the limit, ids unique and below the next id, step counter in range.
fn counters_valid(encoded: &Encoded) -> bool {
    let mut seen = HashSet::new();
    encoded.shapes.len() <= MAX_SHAPES
        && (1..=MAX_STEP_NUMBER + 1).contains(&encoded.next_step_number)
        && encoded
            .shapes
            .iter()
            .all(|shape| shape.id.0 < encoded.next_id && seen.insert(shape.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::shape::{ShapeId, ShapeKind};
    use crate::style::Style;

    fn base() -> BaseImage {
        BaseImage::from_rgba(20, 10, vec![200; 20 * 10 * 4], 2.0).unwrap()
    }

    fn sample() -> Document {
        let shapes = [
            ShapeKind::Arrow {
                from: Point::new(1.0, 1.0),
                to: Point::new(9.0, 5.0),
            },
            ShapeKind::Text {
                origin: Point::new(2.0, 2.0),
                text: "示例 Hello".into(),
                font_size: 14.0,
            },
            ShapeKind::Step {
                center: Point::new(5.0, 5.0),
                number: 1,
            },
        ];
        let doc = shapes.into_iter().fold(Document::new(base()), |doc, kind| {
            doc.apply(
                &crate::Command::Add {
                    kind,
                    style: Style::default(),
                },
                None,
            )
            .unwrap()
            .document
        });
        doc.apply(
            &crate::Command::Crop {
                rect: Some(Rect::new(1.0, 1.0, 10.0, 5.0)),
            },
            None,
        )
        .unwrap()
        .document
    }

    fn reencoded(mutate: impl FnOnce(&mut ciborium::Value)) -> Vec<u8> {
        let mut value: ciborium::Value =
            ciborium::from_reader(&sample().encode().unwrap()[..]).unwrap();
        mutate(&mut value);
        let mut out = Vec::new();
        ciborium::into_writer(&value, &mut out).unwrap();
        out
    }

    fn set(field: &'static str, to: ciborium::Value) -> impl FnOnce(&mut ciborium::Value) {
        move |value| {
            for (key, entry) in value.as_map_mut().unwrap() {
                if key.as_text() == Some(field) {
                    *entry = to;
                    return;
                }
            }
        }
    }

    #[test]
    fn round_trips() {
        let doc = sample();
        let decoded = Document::decode(&doc.encode().unwrap(), base()).unwrap();
        assert_eq!(decoded, doc);
        assert_eq!(decoded.next_step_number(), 2);
    }

    #[test]
    fn rejects_damaged_foreign_or_newer_documents() {
        let other = BaseImage::from_rgba(21, 10, vec![0; 21 * 10 * 4], 2.0).unwrap();
        assert_eq!(
            Document::decode(&sample().encode().unwrap(), other),
            Err(AnnotateError::InvalidDocument)
        );
        let bad = [
            b"garbage".to_vec(),
            vec![0; MAX_ENCODED_BYTES + 1],
            reencoded(set("version", 2.into())),
            reencoded(set("next_id", 1.into())),
            reencoded(set("next_step_number", 0.into())),
            reencoded(set("crop", ciborium::Value::Array(vec![]))),
            reencoded(set("shapes", ciborium::Value::Array(vec![]))),
        ];
        for bytes in &bad[..6] {
            assert_eq!(
                Document::decode(bytes, base()),
                Err(AnnotateError::InvalidDocument)
            );
        }
        assert!(
            Document::decode(&bad[6], base()).is_ok(),
            "no shapes is a valid document"
        );
    }

    #[test]
    fn rejects_invalid_shapes_and_crops() {
        let mut doc = sample();
        let mut duplicate = (*doc.shapes[0]).clone();
        duplicate.id = doc.shapes[1].id;
        doc.shapes.push(Arc::new(duplicate));
        assert_eq!(
            Document::decode(&doc.encode().unwrap(), base()),
            Err(AnnotateError::InvalidDocument)
        );

        let mut doc = sample();
        doc.shapes = vec![Arc::new(Shape {
            id: ShapeId(1),
            kind: ShapeKind::Step {
                center: Point::default(),
                number: 0,
            },
            style: Style::default(),
        })];
        assert_eq!(
            Document::decode(&doc.encode().unwrap(), base()),
            Err(AnnotateError::InvalidDocument)
        );

        let mut doc = sample();
        doc.shapes[0] = Arc::new(Shape {
            style: Style {
                opacity: 3.0,
                ..Style::default()
            },
            ..(*doc.shapes[0]).clone()
        });
        assert_eq!(
            Document::decode(&doc.encode().unwrap(), base()),
            Err(AnnotateError::InvalidDocument)
        );

        let mut doc = sample();
        doc.crop = Some(Rect::new(15.0, 0.0, 10.0, 5.0));
        assert_eq!(
            Document::decode(&doc.encode().unwrap(), base()),
            Err(AnnotateError::InvalidDocument)
        );

        let mut doc = sample();
        let template = (*doc.shapes[0]).clone();
        doc.shapes = (0..=MAX_SHAPES as u64)
            .map(|n| {
                Arc::new(Shape {
                    id: ShapeId(n),
                    ..template.clone()
                })
            })
            .collect();
        doc.next_id = 5_000;
        assert_eq!(
            Document::decode(&doc.encode().unwrap(), base()),
            Err(AnnotateError::InvalidDocument)
        );
    }

    #[test]
    fn oversized_documents_are_not_encoded() {
        let mut doc = sample();
        let long = ShapeKind::Pen {
            points: vec![Point::new(1.5, 2.5); 10_000],
        };
        doc.shapes = (0..60)
            .map(|n| {
                Arc::new(Shape {
                    id: ShapeId(n),
                    kind: long.clone(),
                    style: Style::default(),
                })
            })
            .collect();
        assert_eq!(doc.encode(), Err(AnnotateError::InvalidDocument));
    }
}
