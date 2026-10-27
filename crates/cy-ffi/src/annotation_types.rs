//! Annotation records and enums as Swift sees them, and their conversions to `cy-annotate`.
//!
//! Positions are base-image pixels; style sizes are points (multiplied by the image's scale).

use cy_annotate as core;

/// A point in base-image pixels.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct AnnotationPoint {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position (down).
    pub y: f32,
}

/// A rectangle in base-image pixels.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct AnnotationRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// A colour with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct AnnotationColor {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 is opaque).
    pub a: u8,
}

/// How a shape is drawn.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct AnnotationStyle {
    /// Colour.
    pub color: AnnotationColor,
    /// Line width in points (0.5–100).
    pub stroke_width: f32,
    /// Opacity, 0–1.
    pub opacity: f32,
}

/// What a shape is and where it sits.
#[derive(Clone, Debug, PartialEq, uniffi::Enum)]
pub enum AnnotationShapeKind {
    /// A rectangle.
    Rect {
        /// The box.
        rect: AnnotationRect,
        /// Filled instead of outlined.
        filled: bool,
    },
    /// An ellipse in a box.
    Ellipse {
        /// The bounding box.
        rect: AnnotationRect,
        /// Filled instead of outlined.
        filled: bool,
    },
    /// A straight line.
    Line {
        /// Start.
        from: AnnotationPoint,
        /// End.
        to: AnnotationPoint,
    },
    /// An arrow pointing at `to`.
    Arrow {
        /// Tail.
        from: AnnotationPoint,
        /// Head.
        to: AnnotationPoint,
    },
    /// A freehand stroke.
    Pen {
        /// Its points.
        points: Vec<AnnotationPoint>,
    },
    /// A highlighter stroke (multiply blend).
    Highlighter {
        /// Its points.
        points: Vec<AnnotationPoint>,
    },
    /// Text; `\n` starts a new line.
    Text {
        /// Top-left corner of the first line.
        origin: AnnotationPoint,
        /// The text.
        text: String,
        /// Font size in points.
        font_size: f32,
    },
    /// A numbered circle; the number is assigned when it is added.
    Step {
        /// Centre.
        center: AnnotationPoint,
        /// The number.
        number: u32,
    },
    /// Pixelation.
    Mosaic {
        /// The area.
        rect: AnnotationRect,
        /// Block size in points.
        block: u32,
    },
    /// Blur.
    Blur {
        /// The area.
        rect: AnnotationRect,
        /// Radius in points.
        radius: f32,
    },
}

/// One shape of the document.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct AnnotationShape {
    /// Its id.
    pub id: u64,
    /// What and where.
    pub kind: AnnotationShapeKind,
    /// How it is drawn.
    pub style: AnnotationStyle,
}

/// A grip used to resize a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ResizeHandle {
    /// Top-left corner.
    TopLeft,
    /// Top edge.
    Top,
    /// Top-right corner.
    TopRight,
    /// Right edge.
    Right,
    /// Bottom-right corner.
    BottomRight,
    /// Bottom edge.
    Bottom,
    /// Bottom-left corner.
    BottomLeft,
    /// Left edge.
    Left,
    /// Start of a line or arrow.
    Start,
    /// End of a line or arrow.
    End,
}

/// One edit.
#[derive(Clone, Debug, PartialEq, uniffi::Enum)]
pub enum AnnotationCommand {
    /// Adds a shape on top.
    Add {
        /// What.
        kind: AnnotationShapeKind,
        /// Style.
        style: AnnotationStyle,
    },
    /// Moves shapes by (`dx`, `dy`) pixels.
    Move {
        /// The shapes.
        ids: Vec<u64>,
        /// Horizontal offset.
        dx: f32,
        /// Vertical offset.
        dy: f32,
    },
    /// Drags a handle of a shape (not text or steps).
    Resize {
        /// The shape.
        id: u64,
        /// The handle.
        handle: ResizeHandle,
        /// Where it was dropped.
        to: AnnotationPoint,
    },
    /// Restyles shapes.
    Restyle {
        /// The shapes.
        ids: Vec<u64>,
        /// The new style.
        style: AnnotationStyle,
    },
    /// Replaces the text and size of a text shape.
    EditText {
        /// The text shape.
        id: u64,
        /// New text.
        text: String,
        /// New font size in points.
        font_size: f32,
    },
    /// Deletes shapes.
    Delete {
        /// The shapes.
        ids: Vec<u64>,
    },
    /// Deletes every shape the eraser path touches.
    Erase {
        /// The path.
        path: Vec<AnnotationPoint>,
        /// Eraser radius in points.
        radius: f32,
    },
    /// Sets or clears the crop box.
    Crop {
        /// The crop box, or none for the whole image.
        rect: Option<AnnotationRect>,
    },
    /// Moves shapes above all others.
    BringToFront {
        /// The shapes.
        ids: Vec<u64>,
    },
    /// Moves shapes below all others.
    SendToBack {
        /// The shapes.
        ids: Vec<u64>,
    },
}

/// The encoded format of an export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ExportImageFormat {
    /// PNG.
    Png,
    /// JPEG; transparent areas become white.
    Jpeg {
        /// 1–100.
        quality: u8,
    },
}

/// Pixels of a rendered region, straight-alpha RGBA row by row.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RenderedRegion {
    /// Left edge in base-image pixels.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Local date, time and counter for file name templates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FileNameParts {
    /// Year.
    pub year: u16,
    /// Month, 1–12.
    pub month: u8,
    /// Day, 1–31.
    pub day: u8,
    /// Hour, 0–23.
    pub hour: u8,
    /// Minute, 0–59.
    pub minute: u8,
    /// Second, 0–60.
    pub second: u8,
    /// Running number for `{n}`.
    pub n: u32,
}

impl From<AnnotationPoint> for core::Point {
    fn from(p: AnnotationPoint) -> Self {
        Self::new(p.x, p.y)
    }
}

impl From<core::Point> for AnnotationPoint {
    fn from(p: core::Point) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<AnnotationRect> for core::Rect {
    fn from(r: AnnotationRect) -> Self {
        Self::new(r.x, r.y, r.width, r.height)
    }
}

impl From<core::Rect> for AnnotationRect {
    fn from(r: core::Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<AnnotationStyle> for core::Style {
    fn from(s: AnnotationStyle) -> Self {
        let c = s.color;
        Self {
            color: core::Rgba {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            },
            stroke_width: s.stroke_width,
            opacity: s.opacity,
        }
    }
}

impl From<core::Style> for AnnotationStyle {
    fn from(s: core::Style) -> Self {
        let c = s.color;
        Self {
            color: AnnotationColor {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            },
            stroke_width: s.stroke_width,
            opacity: s.opacity,
        }
    }
}

fn points_in(points: Vec<AnnotationPoint>) -> Vec<core::Point> {
    points.into_iter().map(Into::into).collect()
}

fn points_out(points: &[core::Point]) -> Vec<AnnotationPoint> {
    points.iter().copied().map(Into::into).collect()
}

impl From<AnnotationShapeKind> for core::ShapeKind {
    fn from(kind: AnnotationShapeKind) -> Self {
        use AnnotationShapeKind as K;
        match kind {
            K::Rect { rect, filled } => Self::Rect {
                rect: rect.into(),
                filled,
            },
            K::Ellipse { rect, filled } => Self::Ellipse {
                rect: rect.into(),
                filled,
            },
            K::Line { from, to } => Self::Line {
                from: from.into(),
                to: to.into(),
            },
            K::Arrow { from, to } => Self::Arrow {
                from: from.into(),
                to: to.into(),
            },
            K::Pen { points } => Self::Pen {
                points: points_in(points),
            },
            K::Highlighter { points } => Self::Highlighter {
                points: points_in(points),
            },
            K::Text {
                origin,
                text,
                font_size,
            } => Self::Text {
                origin: origin.into(),
                text,
                font_size,
            },
            K::Step { center, number } => Self::Step {
                center: center.into(),
                number,
            },
            K::Mosaic { rect, block } => Self::Mosaic {
                rect: rect.into(),
                block,
            },
            K::Blur { rect, radius } => Self::Blur {
                rect: rect.into(),
                radius,
            },
        }
    }
}

impl From<&core::ShapeKind> for AnnotationShapeKind {
    fn from(kind: &core::ShapeKind) -> Self {
        use core::ShapeKind as K;
        match kind {
            K::Rect { rect, filled } => Self::Rect {
                rect: (*rect).into(),
                filled: *filled,
            },
            K::Ellipse { rect, filled } => Self::Ellipse {
                rect: (*rect).into(),
                filled: *filled,
            },
            K::Line { from, to } => Self::Line {
                from: (*from).into(),
                to: (*to).into(),
            },
            K::Arrow { from, to } => Self::Arrow {
                from: (*from).into(),
                to: (*to).into(),
            },
            K::Pen { points } => Self::Pen {
                points: points_out(points),
            },
            K::Highlighter { points } => Self::Highlighter {
                points: points_out(points),
            },
            K::Text {
                origin,
                text,
                font_size,
            } => Self::Text {
                origin: (*origin).into(),
                text: text.clone(),
                font_size: *font_size,
            },
            K::Step { center, number } => Self::Step {
                center: (*center).into(),
                number: *number,
            },
            K::Mosaic { rect, block } => Self::Mosaic {
                rect: (*rect).into(),
                block: *block,
            },
            K::Blur { rect, radius } => Self::Blur {
                rect: (*rect).into(),
                radius: *radius,
            },
        }
    }
}

impl From<&core::Shape> for AnnotationShape {
    fn from(shape: &core::Shape) -> Self {
        Self {
            id: shape.id.0,
            kind: (&shape.kind).into(),
            style: shape.style.into(),
        }
    }
}

impl From<ResizeHandle> for core::Handle {
    fn from(handle: ResizeHandle) -> Self {
        match handle {
            ResizeHandle::TopLeft => Self::TopLeft,
            ResizeHandle::Top => Self::Top,
            ResizeHandle::TopRight => Self::TopRight,
            ResizeHandle::Right => Self::Right,
            ResizeHandle::BottomRight => Self::BottomRight,
            ResizeHandle::Bottom => Self::Bottom,
            ResizeHandle::BottomLeft => Self::BottomLeft,
            ResizeHandle::Left => Self::Left,
            ResizeHandle::Start => Self::Start,
            ResizeHandle::End => Self::End,
        }
    }
}

pub(crate) fn ids(ids: Vec<u64>) -> Vec<core::ShapeId> {
    ids.into_iter().map(core::ShapeId).collect()
}

impl From<AnnotationCommand> for core::Command {
    fn from(command: AnnotationCommand) -> Self {
        use AnnotationCommand as C;
        match command {
            C::Add { kind, style } => Self::Add {
                kind: kind.into(),
                style: style.into(),
            },
            C::Move { ids: list, dx, dy } => Self::Move {
                ids: ids(list),
                dx,
                dy,
            },
            C::Resize { id, handle, to } => Self::Resize {
                id: core::ShapeId(id),
                handle: handle.into(),
                to: to.into(),
            },
            C::Restyle { ids: list, style } => Self::Restyle {
                ids: ids(list),
                style: style.into(),
            },
            C::EditText {
                id,
                text,
                font_size,
            } => Self::EditText {
                id: core::ShapeId(id),
                text,
                font_size,
            },
            C::Delete { ids: list } => Self::Delete { ids: ids(list) },
            C::Erase { path, radius } => Self::Erase {
                path: points_in(path),
                radius,
            },
            C::Crop { rect } => Self::Crop {
                rect: rect.map(Into::into),
            },
            C::BringToFront { ids: list } => Self::Arrange {
                ids: ids(list),
                to: core::Arrange::Front,
            },
            C::SendToBack { ids: list } => Self::Arrange {
                ids: ids(list),
                to: core::Arrange::Back,
            },
        }
    }
}

impl From<ExportImageFormat> for core::ExportFormat {
    fn from(format: ExportImageFormat) -> Self {
        match format {
            ExportImageFormat::Png => Self::Png,
            ExportImageFormat::Jpeg { quality } => Self::Jpeg { quality },
        }
    }
}

impl From<FileNameParts> for core::FileNameContext {
    fn from(p: FileNameParts) -> Self {
        Self {
            year: p.year,
            month: p.month,
            day: p.day,
            hour: p.hour,
            minute: p.minute,
            second: p.second,
            n: p.n,
        }
    }
}
