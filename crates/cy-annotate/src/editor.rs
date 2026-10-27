//! The editing session: the current document, undo and redo, the font, and geometry queries.

use crate::AnnotateError;
use crate::command::Command;
use crate::document::Document;
use crate::export::{ExportFormat, export};
use crate::geometry::{Point, Rect};
use crate::limits::MAX_UNDO_STEPS;
use crate::render::{Rendered, render_region};
use crate::shape::{Metrics, ShapeId};
use crate::style::HIT_TOLERANCE;
use crate::text::Font;

/// One annotation being edited. Undo and redo switch between kept documents, so they are O(1)
/// and keep only what each edit changed.
#[derive(Clone, Debug)]
pub struct Editor {
    current: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
    font: Option<Font>,
}

impl Editor {
    /// A session over `document`, with nothing to undo.
    #[must_use]
    pub fn new(document: Document) -> Self {
        Self {
            current: document,
            undo: Vec::new(),
            redo: Vec::new(),
            font: None,
        }
    }

    /// The current document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.current
    }

    /// The font for text and step numbers, if the shell gave one.
    #[must_use]
    pub fn font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    /// Sets the font used to measure and draw text.
    pub fn set_font(&mut self, font: Option<Font>) {
        self.font = font;
    }

    /// Applies `command`; returns the id of an added shape. A command that changes nothing (an
    /// eraser that touched nothing) is not recorded for undo.
    ///
    /// # Errors
    ///
    /// Validation errors; the document is unchanged then.
    pub fn apply(&mut self, command: &Command) -> Result<Option<ShapeId>, AnnotateError> {
        let applied = self.current.apply(command, self.font.as_ref())?;
        if applied.document != self.current {
            let previous = std::mem::replace(&mut self.current, applied.document);
            self.undo.push(previous);
            if self.undo.len() > MAX_UNDO_STEPS {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(applied.added)
    }

    /// Goes back one edit; returns whether there was one.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                self.redo
                    .push(std::mem::replace(&mut self.current, previous));
                true
            }
            None => false,
        }
    }

    /// Repeats the last undone edit; returns whether there was one.
    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.current, next));
                true
            }
            None => false,
        }
    }

    /// Whether [`Editor::undo`] would do something.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether [`Editor::redo`] would do something.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// How sizes are measured in this session.
    #[must_use]
    pub fn metrics(&self) -> Metrics<'_> {
        Metrics {
            scale: self.current.base().scale(),
            font: self.font.as_ref(),
        }
    }

    /// The topmost shape at `point`, within [`HIT_TOLERANCE`] points.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<ShapeId> {
        let metrics = self.metrics();
        let tolerance = HIT_TOLERANCE * metrics.scale;
        self.current
            .shapes()
            .rev()
            .find(|shape| shape.distance(point, metrics) <= tolerance)
            .map(|shape| shape.id)
    }

    /// The box around the drawn shapes `ids` (unknown ids are skipped), for the selection frame.
    #[must_use]
    pub fn selection_bounds(&self, ids: &[ShapeId]) -> Option<Rect> {
        let metrics = self.metrics();
        ids.iter()
            .filter_map(|id| self.current.shape(*id))
            .map(|shape| shape.bounds(metrics))
            .reduce(|a, b| a.union(&b))
    }

    /// Renders the composite inside `region` (for previews of effects and text).
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidRegion`] for a region outside the image.
    pub fn render_region(&self, region: Rect) -> Result<Rendered, AnnotateError> {
        render_region(&self.current, self.font.as_ref(), region)
    }

    /// Exports the current document (composited, cropped, encoded).
    ///
    /// # Errors
    ///
    /// See [`export`].
    pub fn export(&self, format: ExportFormat) -> Result<Vec<u8>, AnnotateError> {
        export(&self.current, self.font.as_ref(), format)
    }

    /// The shapes whose drawn box overlaps `area` (rubber-band selection), bottom first.
    #[must_use]
    pub fn shapes_in(&self, area: Rect) -> Vec<ShapeId> {
        let metrics = self.metrics();
        self.current
            .shapes()
            .filter(|shape| shape.bounds(metrics).intersection(&area).is_some())
            .map(|shape| shape.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::BaseImage;
    use crate::shape::ShapeKind;
    use crate::style::Style;

    fn editor() -> Editor {
        Editor::new(Document::new(
            BaseImage::from_rgba(100, 100, vec![255; 100 * 100 * 4], 1.0).unwrap(),
        ))
    }

    fn rect(x: f32, filled: bool) -> Command {
        Command::Add {
            kind: ShapeKind::Rect {
                rect: Rect::new(x, 10.0, 20.0, 20.0),
                filled,
            },
            style: Style::default(),
        }
    }

    #[test]
    fn undo_and_redo_switch_documents() {
        let mut editor = editor();
        let original = editor.document().clone();
        assert!(!editor.can_undo() && !editor.undo() && !editor.redo());
        let id = editor.apply(&rect(10.0, false)).unwrap();
        assert_eq!(id, Some(ShapeId(1)));
        let after = editor.document().clone();
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.document(), &after);
        assert!(editor.undo());
        editor.apply(&rect(50.0, false)).unwrap();
        assert!(!editor.can_redo(), "a new edit clears redo");
    }

    #[test]
    fn no_op_commands_are_not_recorded_and_errors_change_nothing() {
        let mut editor = editor();
        editor.apply(&rect(10.0, false)).unwrap();
        let erase = Command::Erase {
            path: vec![Point::new(90.0, 90.0)],
            radius: 1.0,
        };
        assert_eq!(editor.apply(&erase), Ok(None));
        assert!(editor.undo());
        assert!(!editor.can_undo());
        let bad = Command::Delete {
            ids: vec![ShapeId(99)],
        };
        assert_eq!(editor.apply(&bad), Err(AnnotateError::UnknownShape));
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut editor = editor();
        for n in 0..=MAX_UNDO_STEPS {
            #[allow(clippy::cast_precision_loss, reason = "test values are small")]
            editor.apply(&rect(n as f32 % 50.0, false)).unwrap();
        }
        let mut undone = 0;
        while editor.undo() {
            undone += 1;
        }
        assert_eq!(undone, MAX_UNDO_STEPS);
        assert_eq!(editor.document().shapes().len(), 1);
    }

    #[test]
    fn hit_testing_prefers_the_top_shape_and_respects_fill() {
        let mut editor = editor();
        let outline = editor.apply(&rect(10.0, false)).unwrap().unwrap();
        assert_eq!(editor.hit_test(Point::new(10.0, 20.0)), Some(outline));
        assert_eq!(
            editor.hit_test(Point::new(20.0, 20.0)),
            None,
            "an outline is hit on its edge"
        );
        let filled = editor.apply(&rect(15.0, true)).unwrap().unwrap();
        assert_eq!(editor.hit_test(Point::new(25.0, 20.0)), Some(filled));
        assert_eq!(editor.hit_test(Point::new(90.0, 90.0)), None);
    }

    #[test]
    fn selection_bounds_and_rubber_band() {
        let mut editor = editor();
        let a = editor.apply(&rect(10.0, false)).unwrap().unwrap();
        let b = editor.apply(&rect(60.0, false)).unwrap().unwrap();
        assert_eq!(
            editor.selection_bounds(&[a, b, ShapeId(9)]),
            Some(Rect::new(8.0, 8.0, 74.0, 24.0))
        );
        assert_eq!(editor.selection_bounds(&[]), None);
        assert_eq!(editor.shapes_in(Rect::new(0.0, 0.0, 40.0, 40.0)), [a]);
        assert!(editor.font().is_none());
        editor.set_font(None);
        assert!((editor.metrics().scale - 1.0).abs() < f32::EPSILON);
    }
}
