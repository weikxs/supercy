//! Points, rectangles and resize handles, in base-image pixel coordinates.

use serde::{Deserialize, Serialize};

use crate::AnnotateError;
use crate::limits::MAX_COORDINATE;

/// A point in base-image pixels.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal position, growing to the right.
    pub x: f32,
    /// Vertical position, growing downwards.
    pub y: f32,
}

impl Point {
    /// The point (`x`, `y`).
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// This point moved by (`dx`, `dy`).
    #[must_use]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// Distance to `other`.
    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// Whether both coordinates are finite and within [`MAX_COORDINATE`].
    #[must_use]
    pub fn is_valid(self) -> bool {
        valid_coordinate(self.x) && valid_coordinate(self.y)
    }
}

/// An axis-aligned rectangle in base-image pixels; `width` and `height` are never negative.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// The rectangle at (`x`, `y`) of the given size; a negative size is flipped.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::from_corners(Point::new(x, y), Point::new(x + width, y + height))
    }

    /// The rectangle spanned by two opposite corners, in any order.
    #[must_use]
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            width: (a.x - b.x).abs(),
            height: (a.y - b.y).abs(),
        }
    }

    /// The smallest rectangle holding every point, or `None` for no points.
    #[must_use]
    pub fn around(points: &[Point]) -> Option<Self> {
        let first = points.first()?;
        let (min, max) = points.iter().fold((*first, *first), |(min, max), p| {
            (
                Point::new(min.x.min(p.x), min.y.min(p.y)),
                Point::new(max.x.max(p.x), max.y.max(p.y)),
            )
        });
        Some(Self::from_corners(min, max))
    }

    /// Right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// The centre.
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Whether the edges are valid coordinates and the size is not negative.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        Point::new(self.x, self.y).is_valid()
            && Point::new(self.right(), self.bottom()).is_valid()
            && self.width >= 0.0
            && self.height >= 0.0
    }

    /// This rectangle moved by (`dx`, `dy`).
    #[must_use]
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }

    /// This rectangle grown by `by` on every side (shrunk for a negative `by`, never below 0).
    #[must_use]
    pub fn inflate(&self, by: f32) -> Self {
        Self {
            x: self.x - by,
            y: self.y - by,
            width: (self.width + 2.0 * by).max(0.0),
            height: (self.height + 2.0 * by).max(0.0),
        }
    }

    /// The smallest rectangle holding both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self::from_corners(
            Point::new(self.x.min(other.x), self.y.min(other.y)),
            Point::new(
                self.right().max(other.right()),
                self.bottom().max(other.bottom()),
            ),
        )
    }

    /// The overlap of both, or `None` when they do not overlap with a positive area.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > left && bottom > top)
            .then(|| Self::from_corners(Point::new(left, top), Point::new(right, bottom)))
    }

    /// Whether `point` is inside or on the edge.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    /// Distance from `point` to the nearest point of the filled rectangle (0 inside).
    #[must_use]
    pub fn distance_to(&self, point: Point) -> f32 {
        let dx = (self.x - point.x).max(point.x - self.right()).max(0.0);
        let dy = (self.y - point.y).max(point.y - self.bottom()).max(0.0);
        dx.hypot(dy)
    }

    /// The rectangle snapped outwards to whole pixels.
    #[must_use]
    pub fn round_out(&self) -> Self {
        let left = self.x.floor();
        let top = self.y.floor();
        Self {
            x: left,
            y: top,
            width: self.right().ceil() - left,
            height: self.bottom().ceil() - top,
        }
    }

    /// The position of `handle` on this rectangle, or `None` for line handles.
    #[must_use]
    pub fn handle_point(&self, handle: Handle) -> Option<Point> {
        let (h, v) = handle.box_edges()?;
        Some(Point::new(
            h.pick(self.x, self.right()),
            v.pick(self.y, self.bottom()),
        ))
    }

    /// This rectangle with `handle` dragged to `to`; the opposite edges stay put.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidHandle`] for the line handles.
    pub fn resized(&self, handle: Handle, to: Point) -> Result<Self, AnnotateError> {
        let (h, v) = handle.box_edges().ok_or(AnnotateError::InvalidHandle)?;
        let (left, right) = h.drag(self.x, self.right(), to.x);
        let (top, bottom) = v.drag(self.y, self.bottom(), to.y);
        Ok(Self::from_corners(
            Point::new(left, top),
            Point::new(right, bottom),
        ))
    }
}

/// Where a box handle sits along one axis.
#[derive(Clone, Copy)]
enum Edge {
    /// The left or top edge.
    Min,
    /// The middle.
    Mid,
    /// The right or bottom edge.
    Max,
}

impl Edge {
    fn pick(self, min: f32, max: f32) -> f32 {
        match self {
            Self::Min => min,
            Self::Mid => f32::midpoint(min, max),
            Self::Max => max,
        }
    }

    /// The edges after dragging this handle to `to`.
    fn drag(self, min: f32, max: f32, to: f32) -> (f32, f32) {
        match self {
            Self::Min => (to, max),
            Self::Mid => (min, max),
            Self::Max => (min, to),
        }
    }
}

/// A grip used to resize a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Handle {
    /// Top-left corner of the bounding box.
    TopLeft,
    /// Middle of the top edge.
    Top,
    /// Top-right corner.
    TopRight,
    /// Middle of the right edge.
    Right,
    /// Bottom-right corner.
    BottomRight,
    /// Middle of the bottom edge.
    Bottom,
    /// Bottom-left corner.
    BottomLeft,
    /// Middle of the left edge.
    Left,
    /// The start of a line or arrow.
    Start,
    /// The end of a line or arrow (the arrow head).
    End,
}

impl Handle {
    /// Every handle.
    pub const ALL: [Self; 10] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
        Self::Start,
        Self::End,
    ];

    /// Where the handle sits on a box, horizontally and vertically.
    fn box_edges(self) -> Option<(Edge, Edge)> {
        match self {
            Self::TopLeft => Some((Edge::Min, Edge::Min)),
            Self::Top => Some((Edge::Mid, Edge::Min)),
            Self::TopRight => Some((Edge::Max, Edge::Min)),
            Self::Right => Some((Edge::Max, Edge::Mid)),
            Self::BottomRight => Some((Edge::Max, Edge::Max)),
            Self::Bottom => Some((Edge::Mid, Edge::Max)),
            Self::BottomLeft => Some((Edge::Min, Edge::Max)),
            Self::Left => Some((Edge::Min, Edge::Mid)),
            Self::Start | Self::End => None,
        }
    }
}

/// Whether `value` is finite and within [`MAX_COORDINATE`].
#[must_use]
pub fn valid_coordinate(value: f32) -> bool {
    value.is_finite() && value.abs() <= MAX_COORDINATE
}

/// Distance from `point` to the segment `a`–`b`.
#[must_use]
pub fn distance_to_segment(point: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return point.distance(a);
    }
    let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared).clamp(0.0, 1.0);
    point.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Distance from `point` to the polyline through `points` (a single point counts as a dot).
#[must_use]
pub fn distance_to_polyline(point: Point, points: &[Point]) -> f32 {
    match points {
        [] => f32::INFINITY,
        [only] => point.distance(*only),
        _ => points
            .windows(2)
            .map(|pair| distance_to_segment(point, pair[0], pair[1]))
            .fold(f32::INFINITY, f32::min),
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "the expected values are exact by construction"
)]
mod tests {
    use super::*;

    #[test]
    fn rect_from_corners_is_normalized() {
        let rect = Rect::from_corners(Point::new(30.0, 40.0), Point::new(10.0, 5.0));
        assert_eq!(rect, Rect::new(10.0, 5.0, 20.0, 35.0));
        assert_eq!(
            Rect::new(10.0, 10.0, -4.0, -6.0),
            Rect::new(6.0, 4.0, 4.0, 6.0)
        );
    }

    #[test]
    fn union_intersection_and_contains() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(a.intersection(&b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.intersection(&Rect::new(10.0, 0.0, 5.0, 5.0)), None);
        assert!(a.contains(Point::new(10.0, 0.0)));
        assert!(!a.contains(Point::new(10.1, 0.0)));
        assert_eq!(a.center(), Point::new(5.0, 5.0));
    }

    #[test]
    fn distances() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(rect.distance_to(Point::new(5.0, 5.0)), 0.0);
        assert_eq!(rect.distance_to(Point::new(13.0, 14.0)), 5.0);
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 0.0);
        assert_eq!(distance_to_segment(Point::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(Point::new(-3.0, 4.0), a, b), 5.0);
        assert_eq!(distance_to_segment(Point::new(3.0, 4.0), a, a), 5.0);
        assert_eq!(
            distance_to_polyline(Point::new(1.0, 1.0), &[]),
            f32::INFINITY
        );
        assert_eq!(distance_to_polyline(Point::new(3.0, 4.0), &[a]), 5.0);
        assert_eq!(
            distance_to_polyline(Point::new(10.0, 5.0), &[a, b, Point::new(10.0, 10.0)]),
            0.0
        );
    }

    #[test]
    fn resizing_moves_only_the_dragged_edges() {
        let rect = Rect::new(10.0, 10.0, 20.0, 20.0);
        let to = Point::new(50.0, 0.0);
        assert_eq!(
            rect.resized(Handle::TopRight, to),
            Ok(Rect::new(10.0, 0.0, 40.0, 30.0))
        );
        assert_eq!(
            rect.resized(Handle::Right, to),
            Ok(Rect::new(10.0, 10.0, 40.0, 20.0))
        );
        assert_eq!(
            rect.resized(Handle::Top, to),
            Ok(Rect::new(10.0, 0.0, 20.0, 30.0))
        );
        assert_eq!(
            rect.resized(Handle::BottomLeft, Point::new(40.0, 5.0)),
            Ok(Rect::new(30.0, 5.0, 10.0, 5.0))
        );
        assert_eq!(
            rect.resized(Handle::Start, to),
            Err(AnnotateError::InvalidHandle)
        );
        assert_eq!(
            rect.handle_point(Handle::Bottom),
            Some(Point::new(20.0, 30.0))
        );
        assert_eq!(rect.handle_point(Handle::End), None);
    }

    #[test]
    fn validity_and_rounding() {
        assert!(Point::new(1.0, 2.0).is_valid());
        assert!(!Point::new(f32::NAN, 2.0).is_valid());
        assert!(!Point::new(1.0, f32::INFINITY).is_valid());
        assert!(!Point::new(MAX_COORDINATE * 2.0, 0.0).is_valid());
        assert!(
            !Rect {
                x: 0.0,
                y: 0.0,
                width: -1.0,
                height: 1.0
            }
            .is_valid()
        );
        assert_eq!(
            Rect::new(0.5, 0.2, 2.0, 2.0).round_out(),
            Rect::new(0.0, 0.0, 3.0, 3.0)
        );
        assert_eq!(Rect::new(0.0, 0.0, 2.0, 2.0).inflate(-5.0).width, 0.0);
        assert_eq!(Rect::around(&[]), None);
        assert_eq!(
            Rect::around(&[Point::new(3.0, 1.0), Point::new(-1.0, 4.0)]),
            Some(Rect::new(-1.0, 1.0, 4.0, 3.0))
        );
        assert_eq!(Point::new(1.0, 1.0).offset(2.0, -1.0), Point::new(3.0, 0.0));
        assert_eq!(
            Rect::new(0.0, 0.0, 1.0, 1.0).offset(2.0, 3.0),
            Rect::new(2.0, 3.0, 1.0, 1.0)
        );
        assert_eq!(Handle::ALL.len(), 10);
    }
}
