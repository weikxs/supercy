//! Edit commands. Each one is validated and returns a new document; on error nothing changes.

use std::sync::Arc;

use crate::AnnotateError;
use crate::document::Document;
use crate::geometry::{Handle, Point, Rect, valid_coordinate};
use crate::limits::{MAX_ERASE_POINTS, MAX_ERASE_RADIUS, MAX_SHAPES, MAX_STEP_NUMBER};
use crate::shape::{Metrics, Shape, ShapeId, ShapeKind, check_text};
use crate::style::Style;
use crate::text::Font;

/// Most points sampled along an eraser path; longer paths are sampled more coarsely.
const MAX_ERASE_SAMPLES: f32 = 100_000.0;

/// Where [`Command::Arrange`] moves shapes in the drawing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrange {
    /// On top of every other shape.
    Front,
    /// Below every other shape.
    Back,
}

/// One edit.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Adds a shape on top. A step's number is assigned by the document.
    Add {
        /// What to add.
        kind: ShapeKind,
        /// How it is drawn.
        style: Style,
    },
    /// Moves shapes by (`dx`, `dy`) pixels.
    Move {
        /// The shapes.
        ids: Vec<ShapeId>,
        /// Horizontal offset.
        dx: f32,
        /// Vertical offset.
        dy: f32,
    },
    /// Drags one handle of a shape.
    Resize {
        /// The shape.
        id: ShapeId,
        /// The handle dragged.
        handle: Handle,
        /// Where it was dropped.
        to: Point,
    },
    /// Gives shapes a new style.
    Restyle {
        /// The shapes.
        ids: Vec<ShapeId>,
        /// The new style.
        style: Style,
    },
    /// Replaces the text and font size of a text shape.
    EditText {
        /// The text shape.
        id: ShapeId,
        /// The new text.
        text: String,
        /// The new font size in points.
        font_size: f32,
    },
    /// Deletes shapes.
    Delete {
        /// The shapes.
        ids: Vec<ShapeId>,
    },
    /// Deletes every shape the eraser path touches; the base image is never changed.
    Erase {
        /// The eraser's path.
        path: Vec<Point>,
        /// The eraser's radius in points.
        radius: f32,
    },
    /// Sets or clears the crop box (snapped to whole pixels inside the image).
    Crop {
        /// The new crop box, or `None` for the whole image.
        rect: Option<Rect>,
    },
    /// Moves shapes to the front or the back, keeping their order among themselves.
    Arrange {
        /// The shapes.
        ids: Vec<ShapeId>,
        /// Where to.
        to: Arrange,
    },
}

/// The result of applying a command.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    /// The new document.
    pub document: Document,
    /// The id of the added shape, for [`Command::Add`].
    pub added: Option<ShapeId>,
}

impl Document {
    /// Applies `command`; `font` measures text for the eraser.
    ///
    /// # Errors
    ///
    /// Validation errors; this document is unchanged either way.
    pub fn apply(&self, command: &Command, font: Option<&Font>) -> Result<Applied, AnnotateError> {
        let document = match command {
            Command::Add { kind, style } => return self.add(kind, *style),
            Command::Move { ids, dx, dy } => {
                if !(valid_coordinate(*dx) && valid_coordinate(*dy)) {
                    return Err(AnnotateError::InvalidGeometry);
                }
                self.replace(ids, |shape| with_kind(shape, shape.kind.moved(*dx, *dy)))?
            }
            Command::Resize { id, handle, to } => {
                if !to.is_valid() {
                    return Err(AnnotateError::InvalidGeometry);
                }
                self.replace(&[*id], |shape| {
                    with_kind(shape, shape.kind.resized(*handle, *to)?)
                })?
            }
            Command::Restyle { ids, style } => {
                style.validate()?;
                self.replace(ids, |shape| {
                    Ok(Shape {
                        style: *style,
                        ..shape.clone()
                    })
                })?
            }
            Command::EditText {
                id,
                text,
                font_size,
            } => self.edit_text(*id, text, *font_size)?,
            Command::Delete { ids } => self.delete(ids)?,
            Command::Erase { path, radius } => self.erase(path, *radius, font)?,
            Command::Crop { rect } => self.cropped(*rect)?,
            Command::Arrange { ids, to } => self.arranged(ids, *to)?,
        };
        Ok(Applied {
            document,
            added: None,
        })
    }

    fn add(&self, kind: &ShapeKind, style: Style) -> Result<Applied, AnnotateError> {
        if self.shapes.len() >= MAX_SHAPES {
            return Err(AnnotateError::LimitExceeded);
        }
        style.validate()?;
        let (kind, next_step_number) = match kind {
            ShapeKind::Step { center, .. } if self.next_step_number <= MAX_STEP_NUMBER => (
                ShapeKind::Step {
                    center: *center,
                    number: self.next_step_number,
                },
                self.next_step_number + 1,
            ),
            ShapeKind::Step { .. } => return Err(AnnotateError::LimitExceeded),
            other => (other.clone(), self.next_step_number),
        };
        kind.validate()?;
        let id = ShapeId(self.next_id);
        let mut shapes = self.shapes.clone();
        shapes.push(Arc::new(Shape { id, kind, style }));
        let document = Self {
            shapes,
            next_step_number,
            next_id: self.next_id + 1,
            ..self.clone()
        };
        Ok(Applied {
            document,
            added: Some(id),
        })
    }

    /// A copy with every shape in `ids` replaced by `change(shape)`.
    fn replace(
        &self,
        ids: &[ShapeId],
        change: impl Fn(&Shape) -> Result<Shape, AnnotateError>,
    ) -> Result<Self, AnnotateError> {
        self.check_ids(ids)?;
        let shapes = self
            .shapes
            .iter()
            .map(|shape| {
                if ids.contains(&shape.id) {
                    change(shape).map(Arc::new)
                } else {
                    Ok(Arc::clone(shape))
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            shapes,
            ..self.clone()
        })
    }

    fn check_ids(&self, ids: &[ShapeId]) -> Result<(), AnnotateError> {
        if ids.iter().all(|id| self.position(*id).is_some()) {
            Ok(())
        } else {
            Err(AnnotateError::UnknownShape)
        }
    }

    fn edit_text(&self, id: ShapeId, text: &str, font_size: f32) -> Result<Self, AnnotateError> {
        check_text(text)?;
        self.replace(&[id], |shape| match &shape.kind {
            ShapeKind::Text { origin, .. } => with_kind(
                shape,
                ShapeKind::Text {
                    origin: *origin,
                    text: text.to_owned(),
                    font_size,
                },
            ),
            _ => Err(AnnotateError::NotText),
        })
    }

    fn delete(&self, ids: &[ShapeId]) -> Result<Self, AnnotateError> {
        self.check_ids(ids)?;
        Ok(self.without(|shape| ids.contains(&shape.id)))
    }

    fn without(&self, remove: impl Fn(&Shape) -> bool) -> Self {
        let shapes = self
            .shapes
            .iter()
            .filter(|shape| !remove(shape))
            .cloned()
            .collect();
        Self {
            shapes,
            ..self.clone()
        }
    }

    fn erase(
        &self,
        path: &[Point],
        radius: f32,
        font: Option<&Font>,
    ) -> Result<Self, AnnotateError> {
        let path_ok = !path.is_empty()
            && path.len() <= MAX_ERASE_POINTS
            && path.iter().all(|p| p.is_valid())
            && radius > 0.0
            && radius <= MAX_ERASE_RADIUS;
        if !path_ok {
            return Err(AnnotateError::InvalidGeometry);
        }
        let metrics = Metrics {
            scale: self.base.scale(),
            font,
        };
        let reach = radius * metrics.scale;
        let samples = sample_path(path, reach);
        let area = Rect::around(path).unwrap_or_default().inflate(reach);
        Ok(self.without(|shape| {
            shape
                .bounds(metrics)
                .inflate(1.0)
                .intersection(&area)
                .is_some()
                && samples.iter().any(|p| shape.distance(*p, metrics) <= reach)
        }))
    }

    fn cropped(&self, rect: Option<Rect>) -> Result<Self, AnnotateError> {
        let crop = match rect {
            None => None,
            Some(rect) if rect.is_valid() => {
                let bounds = self.base.bounds();
                let snapped = rect
                    .intersection(&bounds)
                    .and_then(|inside| inside.round_out().intersection(&bounds))
                    .filter(|r| r.width >= 1.0 && r.height >= 1.0)
                    .ok_or(AnnotateError::InvalidCrop)?;
                Some(snapped)
            }
            Some(_) => return Err(AnnotateError::InvalidGeometry),
        };
        Ok(Self {
            crop,
            ..self.clone()
        })
    }

    fn arranged(&self, ids: &[ShapeId], to: Arrange) -> Result<Self, AnnotateError> {
        self.check_ids(ids)?;
        let (picked, rest): (Vec<_>, Vec<_>) = self
            .shapes
            .iter()
            .cloned()
            .partition(|shape| ids.contains(&shape.id));
        let shapes = match to {
            Arrange::Front => rest.into_iter().chain(picked).collect(),
            Arrange::Back => picked.into_iter().chain(rest).collect(),
        };
        Ok(Self {
            shapes,
            ..self.clone()
        })
    }
}

/// `shape` with `kind`, after validating it.
fn with_kind(shape: &Shape, kind: ShapeKind) -> Result<Shape, AnnotateError> {
    kind.validate()?;
    Ok(Shape {
        kind,
        ..shape.clone()
    })
}

/// Points along `path`, at most `reach` apart (coarser for very long paths).
fn sample_path(path: &[Point], reach: f32) -> Vec<Point> {
    let length: f32 = path.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    let spacing = reach.max(1.0).max(length / MAX_ERASE_SAMPLES);
    let mut samples = vec![path[0]];
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = (a.distance(b) / spacing).ceil().max(1.0);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "steps is a small positive whole number"
        )]
        let count = steps as u32;
        #[allow(clippy::cast_precision_loss, reason = "count is small")]
        samples.extend((1..=count).map(|i| {
            let t = i as f32 / steps;
            Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
        }));
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::BaseImage;

    fn document() -> Document {
        Document::new(BaseImage::from_rgba(100, 80, vec![255; 100 * 80 * 4], 1.0).unwrap())
    }

    fn add(document: &Document, kind: ShapeKind) -> (Document, ShapeId) {
        let applied = document
            .apply(
                &Command::Add {
                    kind,
                    style: Style::default(),
                },
                None,
            )
            .unwrap();
        (applied.document, applied.added.unwrap())
    }

    fn line(x: f32) -> ShapeKind {
        ShapeKind::Line {
            from: Point::new(x, 10.0),
            to: Point::new(x, 70.0),
        }
    }

    #[test]
    fn add_assigns_ids_and_step_numbers() {
        let start = document();
        let (one, first) = add(
            &start,
            ShapeKind::Step {
                center: Point::new(5.0, 5.0),
                number: 77,
            },
        );
        let (two, second) = add(
            &one,
            ShapeKind::Step {
                center: Point::new(9.0, 5.0),
                number: 77,
            },
        );
        assert_eq!((first, second), (ShapeId(1), ShapeId(2)));
        let numbers: Vec<_> = two
            .shapes()
            .map(|s| match s.kind {
                ShapeKind::Step { number, .. } => number,
                _ => 0,
            })
            .collect();
        assert_eq!(numbers, [1, 2]);
        assert_eq!(two.next_step_number(), 3);
        assert_eq!(start.shapes().len(), 0, "the old document is unchanged");
    }

    #[test]
    #[allow(clippy::too_many_lines, reason = "one table of cases")]
    fn invalid_commands_fail_and_change_nothing() {
        let (doc, id) = add(&document(), line(10.0));
        let bad = [
            (
                Command::Move {
                    ids: vec![id],
                    dx: f32::NAN,
                    dy: 0.0,
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Move {
                    ids: vec![ShapeId(9)],
                    dx: 1.0,
                    dy: 0.0,
                },
                AnnotateError::UnknownShape,
            ),
            (
                Command::Move {
                    ids: vec![id],
                    dx: 99_999.0,
                    dy: 0.0,
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Resize {
                    id,
                    handle: Handle::End,
                    to: Point::new(f32::INFINITY, 0.0),
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Resize {
                    id,
                    handle: Handle::Top,
                    to: Point::default(),
                },
                AnnotateError::InvalidHandle,
            ),
            (
                Command::Restyle {
                    ids: vec![id],
                    style: Style {
                        opacity: 2.0,
                        ..Style::default()
                    },
                },
                AnnotateError::InvalidStyle,
            ),
            (
                Command::EditText {
                    id,
                    text: "demo".into(),
                    font_size: 12.0,
                },
                AnnotateError::NotText,
            ),
            (
                Command::EditText {
                    id,
                    text: String::new(),
                    font_size: 12.0,
                },
                AnnotateError::InvalidText,
            ),
            (
                Command::Delete {
                    ids: vec![ShapeId(42)],
                },
                AnnotateError::UnknownShape,
            ),
            (
                Command::Erase {
                    path: vec![],
                    radius: 4.0,
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Erase {
                    path: vec![Point::default()],
                    radius: 0.0,
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Crop {
                    rect: Some(Rect::new(200.0, 200.0, 10.0, 10.0)),
                },
                AnnotateError::InvalidCrop,
            ),
            (
                Command::Crop {
                    rect: Some(Rect {
                        x: f32::NAN,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    }),
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Arrange {
                    ids: vec![ShapeId(5)],
                    to: Arrange::Front,
                },
                AnnotateError::UnknownShape,
            ),
            (
                Command::Add {
                    kind: line(f32::NAN),
                    style: Style::default(),
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                Command::Add {
                    kind: line(1.0),
                    style: Style {
                        stroke_width: 0.0,
                        ..Style::default()
                    },
                },
                AnnotateError::InvalidStyle,
            ),
        ];
        for (command, error) in bad {
            assert_eq!(doc.apply(&command, None).err(), Some(error), "{command:?}");
        }
    }

    #[test]
    fn move_resize_restyle_edit_and_delete() {
        let (doc, line_id) = add(&document(), line(10.0));
        let text = ShapeKind::Text {
            origin: Point::new(1.0, 1.0),
            text: "demo".into(),
            font_size: 12.0,
        };
        let (doc, text_id) = add(&doc, text);
        let moved = doc
            .apply(
                &Command::Move {
                    ids: vec![line_id],
                    dx: 5.0,
                    dy: 0.0,
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(moved.shape(line_id).unwrap().kind, line(15.0));
        assert!(
            Arc::ptr_eq(&moved.shapes[1], &doc.shapes[1]),
            "unchanged shapes are shared"
        );
        let resized = moved
            .apply(
                &Command::Resize {
                    id: line_id,
                    handle: Handle::End,
                    to: Point::new(50.0, 50.0),
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(
            resized.shape(line_id).unwrap().kind,
            ShapeKind::Line {
                from: Point::new(15.0, 10.0),
                to: Point::new(50.0, 50.0)
            }
        );
        let blue = Style {
            color: crate::Rgba::rgb(0, 0, 255),
            ..Style::default()
        };
        let restyled = resized
            .apply(
                &Command::Restyle {
                    ids: vec![line_id, text_id],
                    style: blue,
                },
                None,
            )
            .unwrap()
            .document;
        assert!(restyled.shapes().all(|s| s.style == blue));
        let edited = restyled
            .apply(
                &Command::EditText {
                    id: text_id,
                    text: "示例".into(),
                    font_size: 20.0,
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(
            edited.shape(text_id).unwrap().kind,
            ShapeKind::Text {
                origin: Point::new(1.0, 1.0),
                text: "示例".into(),
                font_size: 20.0
            }
        );
        let deleted = edited
            .apply(&Command::Delete { ids: vec![line_id] }, None)
            .unwrap()
            .document;
        assert_eq!(
            deleted.shapes().map(|s| s.id).collect::<Vec<_>>(),
            [text_id]
        );
    }

    #[test]
    fn erase_removes_only_touched_shapes() {
        let (doc, left) = add(&document(), line(10.0));
        let (doc, right) = add(&doc, line(60.0));
        let erased = doc
            .apply(
                &Command::Erase {
                    path: vec![Point::new(0.0, 40.0), Point::new(30.0, 40.0)],
                    radius: 2.0,
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(erased.shapes().map(|s| s.id).collect::<Vec<_>>(), [right]);
        assert!(erased.shape(left).is_none());
        assert_eq!(erased.base(), doc.base());
        let missed = doc
            .apply(
                &Command::Erase {
                    path: vec![Point::new(35.0, 40.0)],
                    radius: 2.0,
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(missed, doc);
    }

    #[test]
    fn crop_snaps_inside_the_image_and_clears() {
        let doc = document();
        let cropped = doc
            .apply(
                &Command::Crop {
                    rect: Some(Rect::new(-10.0, 10.4, 50.0, 200.0)),
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(cropped.crop(), Some(Rect::new(0.0, 10.0, 40.0, 70.0)));
        assert_eq!(cropped.export_area(), Rect::new(0.0, 10.0, 40.0, 70.0));
        let cleared = cropped
            .apply(&Command::Crop { rect: None }, None)
            .unwrap()
            .document;
        assert_eq!(cleared.crop(), None);
    }

    #[test]
    fn arrange_moves_to_front_and_back() {
        let (doc, a) = add(&document(), line(1.0));
        let (doc, b) = add(&doc, line(2.0));
        let (doc, c) = add(&doc, line(3.0));
        let order = |d: &Document| d.shapes().map(|s| s.id).collect::<Vec<_>>();
        let front = doc
            .apply(
                &Command::Arrange {
                    ids: vec![a],
                    to: Arrange::Front,
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(order(&front), [b, c, a]);
        let back = doc
            .apply(
                &Command::Arrange {
                    ids: vec![c, b],
                    to: Arrange::Back,
                },
                None,
            )
            .unwrap()
            .document;
        assert_eq!(order(&back), [b, c, a]);
    }

    #[test]
    fn limits_on_shapes_and_steps() {
        let mut doc = document();
        doc.shapes = (0..MAX_SHAPES)
            .map(|_| {
                Arc::new(Shape {
                    id: ShapeId(1),
                    kind: line(1.0),
                    style: Style::default(),
                })
            })
            .collect();
        let full = doc.apply(
            &Command::Add {
                kind: line(2.0),
                style: Style::default(),
            },
            None,
        );
        assert_eq!(full.err(), Some(AnnotateError::LimitExceeded));
        let mut steps = document();
        steps.next_step_number = MAX_STEP_NUMBER + 1;
        let step = ShapeKind::Step {
            center: Point::default(),
            number: 1,
        };
        let over = steps.apply(
            &Command::Add {
                kind: step,
                style: Style::default(),
            },
            None,
        );
        assert_eq!(over.err(), Some(AnnotateError::LimitExceeded));
    }

    #[test]
    fn long_eraser_paths_are_sampled_coarsely() {
        let samples = sample_path(&[Point::new(0.0, 0.0), Point::new(90_000.0, 0.0)], 0.5);
        assert!(samples.len() <= 100_002);
        assert_eq!(sample_path(&[Point::new(1.0, 1.0)], 3.0).len(), 1);
    }
}
