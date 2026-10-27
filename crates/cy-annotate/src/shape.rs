//! Shapes: what each annotation is, where it is, and how it is styled.

use serde::{Deserialize, Serialize};

use crate::AnnotateError;
use crate::geometry::{Handle, Point, Rect, distance_to_polyline, distance_to_segment};
use crate::limits::{
    MAX_BLUR_RADIUS, MAX_FONT_SIZE, MAX_MOSAIC_BLOCK, MAX_PEN_POINTS, MAX_STEP_NUMBER,
    MAX_TEXT_CHARS, MIN_BLUR_RADIUS, MIN_FONT_SIZE, MIN_MOSAIC_BLOCK,
};
use crate::style::{HIGHLIGHTER_WIDTH_FACTOR, Style, step_radius};
use crate::text::{Font, measure_text};

/// Identifies a shape within its document; never reused within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ShapeId(pub u64);

/// What a shape is and where it sits, in base-image pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShapeKind {
    /// A rectangle, outlined or filled.
    Rect {
        /// The box.
        rect: Rect,
        /// Filled instead of outlined.
        filled: bool,
    },
    /// An ellipse inside a box, outlined or filled.
    Ellipse {
        /// The bounding box.
        rect: Rect,
        /// Filled instead of outlined.
        filled: bool,
    },
    /// A straight line.
    Line {
        /// Start.
        from: Point,
        /// End.
        to: Point,
    },
    /// A line with an arrow head at `to`.
    Arrow {
        /// Tail.
        from: Point,
        /// Head.
        to: Point,
    },
    /// A freehand stroke (simplified by the shell; smoothed when drawn).
    Pen {
        /// The stroke's points.
        points: Vec<Point>,
    },
    /// A wide translucent stroke blended with multiply, so text below stays readable.
    Highlighter {
        /// The stroke's points.
        points: Vec<Point>,
    },
    /// One paragraph of text; `\n` starts a new line.
    Text {
        /// Top-left corner of the first line.
        origin: Point,
        /// The text.
        text: String,
        /// Font size in points.
        font_size: f32,
    },
    /// A numbered circle. The number is assigned by the document when the step is added.
    Step {
        /// Centre of the circle.
        center: Point,
        /// The number shown.
        number: u32,
    },
    /// Pixelation of the area below.
    Mosaic {
        /// The area.
        rect: Rect,
        /// Block size in points.
        block: u32,
    },
    /// Blur of the area below.
    Blur {
        /// The area.
        rect: Rect,
        /// Blur radius in points.
        radius: f32,
    },
}

/// What measuring a shape needs: the base image's scale and the font for text.
#[derive(Clone, Copy, Debug)]
pub struct Metrics<'a> {
    /// Pixels per point.
    pub scale: f32,
    /// The font for text and step numbers; without one, text sizes are estimated.
    pub font: Option<&'a Font>,
}

/// One annotation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    /// Its id.
    pub id: ShapeId,
    /// What and where.
    pub kind: ShapeKind,
    /// How it is drawn.
    pub style: Style,
}

impl ShapeKind {
    /// Checks coordinates, sizes and limits.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidGeometry`], [`AnnotateError::InvalidStyle`],
    /// [`AnnotateError::InvalidText`] or [`AnnotateError::LimitExceeded`].
    pub fn validate(&self) -> Result<(), AnnotateError> {
        let geometry_ok = self.points().iter().all(|p| p.is_valid())
            && self.area().is_none_or(|rect| rect.is_valid());
        if !geometry_ok {
            return Err(AnnotateError::InvalidGeometry);
        }
        match self {
            Self::Pen { points } | Self::Highlighter { points } => check_points(points),
            Self::Text {
                text, font_size, ..
            } => {
                check_text(text)?;
                in_range((MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(font_size))
            }
            Self::Step { number, .. } => (1..=MAX_STEP_NUMBER)
                .contains(number)
                .then_some(())
                .ok_or(AnnotateError::LimitExceeded),
            Self::Mosaic { block, .. } => {
                in_range((MIN_MOSAIC_BLOCK..=MAX_MOSAIC_BLOCK).contains(block))
            }
            Self::Blur { radius, .. } => {
                in_range((MIN_BLUR_RADIUS..=MAX_BLUR_RADIUS).contains(radius))
            }
            _ => Ok(()),
        }
    }

    /// The box of the shapes defined by one (rectangle, ellipse, mosaic, blur).
    #[must_use]
    pub fn area(&self) -> Option<Rect> {
        match self {
            Self::Rect { rect, .. }
            | Self::Ellipse { rect, .. }
            | Self::Mosaic { rect, .. }
            | Self::Blur { rect, .. } => Some(*rect),
            _ => None,
        }
    }

    /// The points of the shapes defined by points.
    #[must_use]
    pub fn points(&self) -> Vec<Point> {
        match self {
            Self::Line { from, to } | Self::Arrow { from, to } => vec![*from, *to],
            Self::Pen { points } | Self::Highlighter { points } => points.clone(),
            Self::Text { origin, .. } => vec![*origin],
            Self::Step { center, .. } => vec![*center],
            _ => Vec::new(),
        }
    }

    /// This shape moved by (`dx`, `dy`).
    #[must_use]
    pub fn moved(&self, dx: f32, dy: f32) -> Self {
        self.map_points(|p| p.offset(dx, dy), |r| r.offset(dx, dy))
    }

    /// This shape with `handle` dragged to `to`. Boxes move the dragged edges; lines and arrows
    /// move one end (`Start`, `End`); strokes scale with their bounding box.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidHandle`] for text and steps (change text size with
    /// `EditText`), for box handles on lines, and for line handles on everything else.
    pub fn resized(&self, handle: Handle, to: Point) -> Result<Self, AnnotateError> {
        match (self, handle) {
            (Self::Line { from, .. }, Handle::End) => Ok(Self::Line { from: *from, to }),
            (Self::Line { to: end, .. }, Handle::Start) => Ok(Self::Line { from: to, to: *end }),
            (Self::Arrow { from, .. }, Handle::End) => Ok(Self::Arrow { from: *from, to }),
            (Self::Arrow { to: end, .. }, Handle::Start) => Ok(Self::Arrow { from: to, to: *end }),
            (Self::Pen { points } | Self::Highlighter { points }, _) => {
                let old = Rect::around(points).ok_or(AnnotateError::InvalidGeometry)?;
                let new = old.resized(handle, to)?;
                Ok(self.map_points(|p| rescale(p, &old, &new), |r| r))
            }
            (Self::Text { .. } | Self::Step { .. } | Self::Line { .. } | Self::Arrow { .. }, _) => {
                Err(AnnotateError::InvalidHandle)
            }
            _ => {
                let new = self
                    .area()
                    .ok_or(AnnotateError::InvalidHandle)?
                    .resized(handle, to)?;
                Ok(self.map_points(|p| p, |_| new))
            }
        }
    }

    fn map_points(&self, point: impl Fn(Point) -> Point, rect: impl Fn(Rect) -> Rect) -> Self {
        match self {
            Self::Rect { rect: r, filled } => Self::Rect {
                rect: rect(*r),
                filled: *filled,
            },
            Self::Ellipse { rect: r, filled } => Self::Ellipse {
                rect: rect(*r),
                filled: *filled,
            },
            Self::Mosaic { rect: r, block } => Self::Mosaic {
                rect: rect(*r),
                block: *block,
            },
            Self::Blur { rect: r, radius } => Self::Blur {
                rect: rect(*r),
                radius: *radius,
            },
            Self::Line { from, to } => Self::Line {
                from: point(*from),
                to: point(*to),
            },
            Self::Arrow { from, to } => Self::Arrow {
                from: point(*from),
                to: point(*to),
            },
            Self::Pen { points } => Self::Pen {
                points: points.iter().map(|p| point(*p)).collect(),
            },
            Self::Highlighter { points } => Self::Highlighter {
                points: points.iter().map(|p| point(*p)).collect(),
            },
            Self::Text {
                origin,
                text,
                font_size,
            } => Self::Text {
                origin: point(*origin),
                text: text.clone(),
                font_size: *font_size,
            },
            Self::Step { center, number } => Self::Step {
                center: point(*center),
                number: *number,
            },
        }
    }
}

impl Shape {
    /// The box the drawn shape covers, including its stroke, in pixels.
    #[must_use]
    pub fn bounds(&self, metrics: Metrics<'_>) -> Rect {
        let half = self.half_width(metrics.scale);
        match &self.kind {
            ShapeKind::Rect { rect, .. } | ShapeKind::Ellipse { rect, .. } => rect.inflate(half),
            ShapeKind::Mosaic { rect, .. } | ShapeKind::Blur { rect, .. } => *rect,
            ShapeKind::Line { from, to } | ShapeKind::Arrow { from, to } => {
                let reach = match self.kind {
                    ShapeKind::Arrow { .. } => half.max(
                        crate::style::arrow_head_length(self.style.stroke_width) * metrics.scale,
                    ),
                    _ => half,
                };
                Rect::from_corners(*from, *to).inflate(reach)
            }
            ShapeKind::Pen { points } | ShapeKind::Highlighter { points } => {
                Rect::around(points).unwrap_or_default().inflate(half)
            }
            ShapeKind::Text {
                origin,
                text,
                font_size,
            } => {
                let size = measure_text(metrics.font, text, font_size * metrics.scale);
                Rect::new(origin.x, origin.y, size.width, size.height)
            }
            ShapeKind::Step { center, .. } => {
                let radius = step_radius(self.style.stroke_width) * metrics.scale;
                Rect::new(center.x, center.y, 0.0, 0.0).inflate(radius)
            }
        }
    }

    /// Distance from `point` to the drawn shape, in pixels (0 on or inside it).
    #[must_use]
    pub fn distance(&self, point: Point, metrics: Metrics<'_>) -> f32 {
        let half = self.half_width(metrics.scale);
        let raw = match &self.kind {
            ShapeKind::Rect {
                rect,
                filled: false,
            } => outline_distance(rect, point) - half,
            ShapeKind::Ellipse { rect, filled } => ellipse_distance(rect, *filled, point) - half,
            ShapeKind::Line { from, to } | ShapeKind::Arrow { from, to } => {
                distance_to_segment(point, *from, *to) - half
            }
            ShapeKind::Pen { points } | ShapeKind::Highlighter { points } => {
                distance_to_polyline(point, points) - half
            }
            ShapeKind::Step { center, .. } => {
                point.distance(*center) - step_radius(self.style.stroke_width) * metrics.scale
            }
            _ => self.bounds(metrics).distance_to(point),
        };
        raw.max(0.0)
    }

    /// Half the drawn stroke width in pixels.
    fn half_width(&self, scale: f32) -> f32 {
        let factor = match self.kind {
            ShapeKind::Highlighter { .. } => HIGHLIGHTER_WIDTH_FACTOR,
            _ => 1.0,
        };
        self.style.stroke_width * factor * scale / 2.0
    }
}

fn in_range(ok: bool) -> Result<(), AnnotateError> {
    if ok {
        Ok(())
    } else {
        Err(AnnotateError::InvalidStyle)
    }
}

fn check_points(points: &[Point]) -> Result<(), AnnotateError> {
    match points.len() {
        0 => Err(AnnotateError::InvalidGeometry),
        n if n > MAX_PEN_POINTS => Err(AnnotateError::LimitExceeded),
        _ => Ok(()),
    }
}

/// Text must be non-empty, within the limit and free of control characters other than `\n`.
pub(crate) fn check_text(text: &str) -> Result<(), AnnotateError> {
    let ok = !text.trim().is_empty()
        && text.chars().count() <= MAX_TEXT_CHARS
        && text.chars().all(|c| c == '\n' || !c.is_control());
    if ok {
        Ok(())
    } else {
        Err(AnnotateError::InvalidText)
    }
}

/// `point` moved from `old`'s frame into `new`'s; a flat axis keeps its offset.
fn rescale(point: Point, old: &Rect, new: &Rect) -> Point {
    let axis = |value: f32, old_start: f32, old_len: f32, new_start: f32, new_len: f32| {
        if old_len > 0.0 {
            new_start + (value - old_start) / old_len * new_len
        } else {
            new_start + (value - old_start)
        }
    };
    Point::new(
        axis(point.x, old.x, old.width, new.x, new.width),
        axis(point.y, old.y, old.height, new.y, new.height),
    )
}

/// Distance to the outline of `rect`.
fn outline_distance(rect: &Rect, point: Point) -> f32 {
    if rect.contains(point) {
        let to_edges = [
            point.x - rect.x,
            rect.right() - point.x,
            point.y - rect.y,
            rect.bottom() - point.y,
        ];
        to_edges.into_iter().fold(f32::INFINITY, f32::min)
    } else {
        rect.distance_to(point)
    }
}

/// Approximate distance to an ellipse's outline (or its area when filled).
fn ellipse_distance(rect: &Rect, filled: bool, point: Point) -> f32 {
    let center = rect.center();
    let (rx, ry) = (rect.width / 2.0, rect.height / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return distance_to_segment(
            point,
            Point::new(rect.x, rect.y),
            Point::new(rect.right(), rect.bottom()),
        );
    }
    let normalized = ((point.x - center.x) / rx).hypot((point.y - center.y) / ry);
    let along = (normalized - 1.0) * rx.min(ry);
    if filled { along.max(0.0) } else { along.abs() }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "the expected values are exact by construction"
)]
mod tests {
    use super::*;

    const PLAIN: Metrics<'static> = Metrics {
        scale: 1.0,
        font: None,
    };

    fn shape(kind: ShapeKind) -> Shape {
        Shape {
            id: ShapeId(1),
            kind,
            style: Style {
                stroke_width: 2.0,
                ..Style::default()
            },
        }
    }

    fn rect_kind(filled: bool) -> ShapeKind {
        ShapeKind::Rect {
            rect: Rect::new(10.0, 10.0, 20.0, 10.0),
            filled,
        }
    }

    #[test]
    fn validation_rejects_bad_values() {
        let bad = [
            (
                ShapeKind::Line {
                    from: Point::new(f32::NAN, 0.0),
                    to: Point::default(),
                },
                AnnotateError::InvalidGeometry,
            ),
            (
                ShapeKind::Pen { points: vec![] },
                AnnotateError::InvalidGeometry,
            ),
            (
                ShapeKind::Pen {
                    points: vec![Point::default(); MAX_PEN_POINTS + 1],
                },
                AnnotateError::LimitExceeded,
            ),
            (
                ShapeKind::Text {
                    origin: Point::default(),
                    text: " ".into(),
                    font_size: 12.0,
                },
                AnnotateError::InvalidText,
            ),
            (
                ShapeKind::Text {
                    origin: Point::default(),
                    text: "a\u{7}".into(),
                    font_size: 12.0,
                },
                AnnotateError::InvalidText,
            ),
            (
                ShapeKind::Text {
                    origin: Point::default(),
                    text: "x".repeat(MAX_TEXT_CHARS + 1),
                    font_size: 12.0,
                },
                AnnotateError::InvalidText,
            ),
            (
                ShapeKind::Text {
                    origin: Point::default(),
                    text: "demo".into(),
                    font_size: 1.0,
                },
                AnnotateError::InvalidStyle,
            ),
            (
                ShapeKind::Step {
                    center: Point::default(),
                    number: 0,
                },
                AnnotateError::LimitExceeded,
            ),
            (
                ShapeKind::Mosaic {
                    rect: Rect::default(),
                    block: 1,
                },
                AnnotateError::InvalidStyle,
            ),
            (
                ShapeKind::Blur {
                    rect: Rect::default(),
                    radius: 500.0,
                },
                AnnotateError::InvalidStyle,
            ),
            (
                ShapeKind::Rect {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: f32::INFINITY,
                        height: 1.0,
                    },
                    filled: false,
                },
                AnnotateError::InvalidGeometry,
            ),
        ];
        for (kind, error) in bad {
            assert_eq!(kind.validate(), Err(error), "{kind:?}");
        }
        let text = ShapeKind::Text {
            origin: Point::default(),
            text: "示例\nHello".into(),
            font_size: 12.0,
        };
        assert_eq!(text.validate(), Ok(()));
        assert_eq!(rect_kind(false).validate(), Ok(()));
    }

    #[test]
    fn moving_offsets_every_point() {
        let pen = ShapeKind::Pen {
            points: vec![Point::new(1.0, 1.0), Point::new(2.0, 3.0)],
        };
        assert_eq!(
            pen.moved(1.0, -1.0),
            ShapeKind::Pen {
                points: vec![Point::new(2.0, 0.0), Point::new(3.0, 2.0)]
            }
        );
        let step = ShapeKind::Step {
            center: Point::new(5.0, 5.0),
            number: 3,
        };
        assert_eq!(
            step.moved(1.0, 1.0),
            ShapeKind::Step {
                center: Point::new(6.0, 6.0),
                number: 3
            }
        );
        let blur = ShapeKind::Blur {
            rect: Rect::new(0.0, 0.0, 4.0, 4.0),
            radius: 3.0,
        };
        assert_eq!(
            blur.moved(2.0, 0.0).area(),
            Some(Rect::new(2.0, 0.0, 4.0, 4.0))
        );
    }

    #[test]
    fn resizing_lines_boxes_and_strokes() {
        let arrow = ShapeKind::Arrow {
            from: Point::new(0.0, 0.0),
            to: Point::new(10.0, 0.0),
        };
        assert_eq!(
            arrow.resized(Handle::End, Point::new(20.0, 5.0)),
            Ok(ShapeKind::Arrow {
                from: Point::new(0.0, 0.0),
                to: Point::new(20.0, 5.0)
            })
        );
        assert_eq!(
            arrow.resized(Handle::Start, Point::new(1.0, 1.0)),
            Ok(ShapeKind::Arrow {
                from: Point::new(1.0, 1.0),
                to: Point::new(10.0, 0.0)
            })
        );
        assert_eq!(
            arrow.resized(Handle::TopLeft, Point::default()),
            Err(AnnotateError::InvalidHandle)
        );
        assert_eq!(
            rect_kind(true).resized(Handle::End, Point::default()),
            Err(AnnotateError::InvalidHandle)
        );
        assert_eq!(
            rect_kind(true).resized(Handle::BottomRight, Point::new(50.0, 40.0)),
            Ok(ShapeKind::Rect {
                rect: Rect::new(10.0, 10.0, 40.0, 30.0),
                filled: true
            })
        );
        let pen = ShapeKind::Pen {
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
        };
        assert_eq!(
            pen.resized(Handle::Right, Point::new(20.0, 0.0)),
            Ok(ShapeKind::Pen {
                points: vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0)]
            })
        );
        let text = ShapeKind::Text {
            origin: Point::default(),
            text: "demo".into(),
            font_size: 12.0,
        };
        assert_eq!(
            text.resized(Handle::Right, Point::default()),
            Err(AnnotateError::InvalidHandle)
        );
        let line = ShapeKind::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(1.0, 1.0),
        };
        assert_eq!(
            line.resized(Handle::Start, Point::new(2.0, 2.0)),
            Ok(ShapeKind::Line {
                from: Point::new(2.0, 2.0),
                to: Point::new(1.0, 1.0)
            })
        );
        assert!(line.resized(Handle::End, Point::new(3.0, 3.0)).is_ok());
    }

    #[test]
    fn distance_and_bounds_include_the_stroke() {
        let outline = shape(rect_kind(false));
        assert_eq!(outline.distance(Point::new(20.0, 15.0), PLAIN), 4.0);
        assert_eq!(outline.distance(Point::new(10.5, 15.0), PLAIN), 0.0);
        assert_eq!(
            shape(rect_kind(true)).distance(Point::new(20.0, 15.0), PLAIN),
            0.0
        );
        assert_eq!(outline.bounds(PLAIN), Rect::new(9.0, 9.0, 22.0, 12.0));
        let ellipse = shape(ShapeKind::Ellipse {
            rect: Rect::new(0.0, 0.0, 20.0, 20.0),
            filled: false,
        });
        assert_eq!(ellipse.distance(Point::new(10.0, 10.0), PLAIN), 9.0);
        assert_eq!(ellipse.distance(Point::new(20.0, 10.0), PLAIN), 0.0);
        let flat = shape(ShapeKind::Ellipse {
            rect: Rect::new(0.0, 0.0, 20.0, 0.0),
            filled: true,
        });
        assert_eq!(flat.distance(Point::new(10.0, 5.0), PLAIN), 4.0);
        let step = shape(ShapeKind::Step {
            center: Point::new(50.0, 50.0),
            number: 1,
        });
        assert_eq!(step.distance(Point::new(50.0, 50.0), PLAIN), 0.0);
        assert_eq!(step.bounds(PLAIN), Rect::new(37.0, 37.0, 26.0, 26.0));
        let highlighter = shape(ShapeKind::Highlighter {
            points: vec![Point::new(0.0, 0.0)],
        });
        assert_eq!(highlighter.bounds(PLAIN), Rect::new(-4.0, -4.0, 8.0, 8.0));
        let arrow = shape(ShapeKind::Arrow {
            from: Point::new(0.0, 0.0),
            to: Point::new(40.0, 0.0),
        });
        assert_eq!(arrow.bounds(PLAIN), Rect::new(-10.0, -10.0, 60.0, 20.0));
        let text = shape(ShapeKind::Text {
            origin: Point::new(5.0, 5.0),
            text: "ab".into(),
            font_size: 10.0,
        });
        assert!(text.bounds(PLAIN).width > 0.0);
        assert_eq!(text.distance(Point::new(6.0, 6.0), PLAIN), 0.0);
        let mosaic = shape(ShapeKind::Mosaic {
            rect: Rect::new(0.0, 0.0, 4.0, 4.0),
            block: 4,
        });
        assert_eq!(mosaic.bounds(PLAIN), Rect::new(0.0, 0.0, 4.0, 4.0));
        let doubled = Metrics {
            scale: 2.0,
            font: None,
        };
        assert_eq!(outline.bounds(doubled), Rect::new(8.0, 8.0, 24.0, 14.0));
    }
}
