//! Vector shapes: rectangles, ellipses, lines, arrows, strokes, text and step markers.

use tiny_skia::{
    BlendMode, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke,
    Transform,
};

use super::glyphs::text_path;
use super::{to_offset, to_pixel};
use crate::geometry::{Point, Rect};
use crate::shape::{Metrics, Shape, ShapeKind};
use crate::style::{
    ARROW_HEAD_HALF_ANGLE_DEGREES, HIGHLIGHTER_ALPHA, HIGHLIGHTER_WIDTH_FACTOR,
    RECT_CORNER_RADIUS_PER_WIDTH, Rgba, STEP_FONT_SIZE_PER_RADIUS, Style, arrow_head_length,
    step_radius,
};

/// How one shape is drawn onto the canvas.
struct Pen<'a> {
    canvas: &'a mut Pixmap,
    transform: Transform,
}

impl Pen<'_> {
    fn fill(&mut self, path: &Path, paint: &Paint<'_>) {
        self.canvas
            .fill_path(path, paint, FillRule::Winding, self.transform, None);
    }

    fn stroke(&mut self, path: &Path, paint: &Paint<'_>, width: f32) {
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        self.canvas
            .stroke_path(path, paint, &stroke, self.transform, None);
    }
}

fn paint(color: Rgba, alpha: f32) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, to_u8(alpha));
    paint.anti_alias = true;
    paint
}

fn to_u8(alpha: f32) -> u8 {
    u8::try_from(to_pixel(alpha.clamp(0.0, 1.0) * 255.0)).unwrap_or(u8::MAX)
}

/// Draws a vector shape; `origin` is the canvas's top-left in image pixels.
pub(super) fn draw(canvas: &mut Pixmap, shape: &Shape, origin: (f32, f32), metrics: Metrics<'_>) {
    let transform = Transform::from_translate(-origin.0, -origin.1);
    let style = &shape.style;
    let width = style.stroke_width * metrics.scale;
    let mut pen = Pen { canvas, transform };
    match &shape.kind {
        ShapeKind::Rect { rect, filled } => {
            let radius = RECT_CORNER_RADIUS_PER_WIDTH * width;
            if let Some(path) = rounded_rect(rect, radius) {
                outline_or_fill(&mut pen, &path, style, *filled, width);
            }
        }
        ShapeKind::Ellipse { rect, filled } => {
            if let Some(path) = to_skia(rect).and_then(PathBuilder::from_oval) {
                outline_or_fill(&mut pen, &path, style, *filled, width);
            }
        }
        ShapeKind::Line { from, to } => {
            if let Some(path) = polyline(&[*from, *to]) {
                pen.stroke(&path, &paint(style.color, style.alpha()), width);
            }
        }
        ShapeKind::Arrow { from, to } => arrow(&mut pen, *from, *to, style, metrics.scale, origin),
        ShapeKind::Pen { points } => {
            stroke_points(&mut pen, points, &paint(style.color, style.alpha()), width);
        }
        ShapeKind::Highlighter { points } => {
            let mut marker = paint(style.color, style.alpha() * HIGHLIGHTER_ALPHA);
            marker.blend_mode = BlendMode::Multiply;
            stroke_points(&mut pen, points, &marker, width * HIGHLIGHTER_WIDTH_FACTOR);
        }
        ShapeKind::Text {
            origin: at,
            text,
            font_size,
        } => {
            let path = metrics
                .font
                .and_then(|font| text_path(font, text, *at, font_size * metrics.scale));
            if let Some(path) = path {
                pen.fill(&path, &paint(style.color, style.alpha()));
            }
        }
        ShapeKind::Step { center, number } => step(&mut pen, *center, *number, style, metrics),
        ShapeKind::Mosaic { .. } | ShapeKind::Blur { .. } => {}
    }
}

fn outline_or_fill(pen: &mut Pen<'_>, path: &Path, style: &Style, filled: bool, width: f32) {
    let paint = paint(style.color, style.alpha());
    if filled {
        pen.fill(path, &paint);
    } else {
        pen.stroke(path, &paint, width);
    }
}

fn to_skia(rect: &Rect) -> Option<tiny_skia::Rect> {
    tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

/// A rectangle whose corners are quadratic curves with the corner as control point.
fn rounded_rect(rect: &Rect, radius: f32) -> Option<Path> {
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0);
    if radius <= 0.0 {
        return to_skia(rect).map(PathBuilder::from_rect);
    }
    let (left, top, right, bottom) = (rect.x, rect.y, rect.right(), rect.bottom());
    let mut builder = PathBuilder::new();
    builder.move_to(left + radius, top);
    builder.line_to(right - radius, top);
    builder.quad_to(right, top, right, top + radius);
    builder.line_to(right, bottom - radius);
    builder.quad_to(right, bottom, right - radius, bottom);
    builder.line_to(left + radius, bottom);
    builder.quad_to(left, bottom, left, bottom - radius);
    builder.line_to(left, top + radius);
    builder.quad_to(left, top, left + radius, top);
    builder.close();
    builder.finish()
}

/// A path through `points`, smoothed with quadratic curves through the midpoints.
fn polyline(points: &[Point]) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    let mut builder = PathBuilder::new();
    builder.move_to(first.x, first.y);
    if let [middle @ .., last] = rest {
        for (index, point) in middle.iter().enumerate() {
            let next = rest[index + 1];
            builder.quad_to(
                point.x,
                point.y,
                f32::midpoint(point.x, next.x),
                f32::midpoint(point.y, next.y),
            );
        }
        builder.line_to(last.x, last.y);
    }
    builder.finish()
}

fn stroke_points(pen: &mut Pen<'_>, points: &[Point], paint: &Paint<'_>, width: f32) {
    match points {
        [dot] => {
            if let Some(path) = PathBuilder::from_circle(dot.x, dot.y, width / 2.0) {
                pen.fill(&path, paint);
            }
        }
        _ => {
            if let Some(path) = polyline(points) {
                pen.stroke(&path, paint, width);
            }
        }
    }
}

/// Shaft and head drawn opaque into a layer, then blended once, so a translucent arrow has no
/// darker overlap where the shaft meets the head.
fn arrow(pen: &mut Pen<'_>, from: Point, to: Point, style: &Style, scale: f32, origin: (f32, f32)) {
    let length = from.distance(to);
    if length <= 0.0 {
        return;
    }
    let width = style.stroke_width * scale;
    let head = (arrow_head_length(style.stroke_width) * scale).min(length);
    let (ux, uy) = ((to.x - from.x) / length, (to.y - from.y) / length);
    let half = head * ARROW_HEAD_HALF_ANGLE_DEGREES.to_radians().tan();
    let base = Point::new(to.x - ux * head, to.y - uy * head);
    let bounds = Rect::from_corners(from, to).inflate(head.max(width));
    let Some((mut layer, left, top)) = layer_for(pen.canvas, &bounds, origin) else {
        return;
    };
    let solid = paint(style.color, 1.0);
    #[allow(
        clippy::cast_precision_loss,
        reason = "canvas offsets are at most 16384"
    )]
    let transform = pen.transform.post_translate(-left as f32, -top as f32);
    let mut inner = Pen {
        canvas: &mut layer,
        transform,
    };
    if let Some(shaft) = polyline(&[from, base]) {
        inner.stroke(&shaft, &solid, width);
    }
    let mut builder = PathBuilder::new();
    builder.move_to(to.x, to.y);
    builder.line_to(base.x - uy * half, base.y + ux * half);
    builder.line_to(base.x + uy * half, base.y - ux * half);
    builder.close();
    if let Some(triangle) = builder.finish() {
        inner.fill(&triangle, &solid);
    }
    let blend = PixmapPaint {
        opacity: style.alpha(),
        ..PixmapPaint::default()
    };
    pen.canvas.draw_pixmap(
        left,
        top,
        layer.as_ref(),
        &blend,
        Transform::identity(),
        None,
    );
}

/// A transparent scratch pixmap over the part of `canvas` that `bounds` (image pixels) covers,
/// with its top-left corner in canvas pixels.
fn layer_for(canvas: &Pixmap, bounds: &Rect, origin: (f32, f32)) -> Option<(Pixmap, i32, i32)> {
    let local = bounds.offset(-origin.0, -origin.1).round_out();
    #[allow(clippy::cast_precision_loss, reason = "canvas sides are at most 16384")]
    let whole = Rect::new(0.0, 0.0, canvas.width() as f32, canvas.height() as f32);
    let area = local.intersection(&whole)?;
    let pixmap = Pixmap::new(to_pixel(area.width), to_pixel(area.height))?;
    Some((pixmap, to_offset(area.x), to_offset(area.y)))
}

fn step(pen: &mut Pen<'_>, center: Point, number: u32, style: &Style, metrics: Metrics<'_>) {
    let radius = step_radius(style.stroke_width) * metrics.scale;
    if let Some(circle) = PathBuilder::from_circle(center.x, center.y, radius) {
        pen.fill(&circle, &paint(style.color, style.alpha()));
    }
    let Some(font) = metrics.font else {
        return;
    };
    let size = radius * STEP_FONT_SIZE_PER_RADIUS;
    let Some(digits) = text_path(font, &number.to_string(), Point::default(), size) else {
        return;
    };
    let bounds = digits.bounds();
    let dx = center.x - f32::midpoint(bounds.left(), bounds.right());
    let dy = center.y - f32::midpoint(bounds.top(), bounds.bottom());
    if let Some(centered) = digits.transform(Transform::from_translate(dx, dy)) {
        pen.fill(&centered, &paint(Rgba::WHITE, style.alpha()));
    }
}
