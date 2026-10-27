//! Golden images: each shape kind, effects, text (English and Chinese), a 2x (Retina) image and a combined
//! scene, rendered and compared with the PNGs committed under `tests/golden` (small tolerance
//! for anti-aliasing). Regenerate with `scripts/docker-test.sh golden-write` and review the
//! images before committing them.

#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

mod common;

use common::{add, base_image, check_golden, font};
use cy_annotate::{Command, Document, Point, Rect, Rgba, ShapeKind, Style, render, render_region};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 100;

fn blank() -> Document {
    Document::new(base_image(WIDTH, HEIGHT, 1.0))
}

fn style(color: Rgba, stroke_width: f32, opacity: f32) -> Style {
    Style {
        color,
        stroke_width,
        opacity,
    }
}

const BLUE: Rgba = Rgba::rgb(30, 90, 230);
const GREEN: Rgba = Rgba::rgb(20, 160, 80);

fn zigzag(x: f32, y: f32) -> Vec<Point> {
    (0..8u8)
        .map(|i| {
            let step = f32::from(i);
            Point::new(
                x + step * 14.0,
                y + if i.is_multiple_of(2) { 0.0 } else { 18.0 },
            )
        })
        .collect()
}

#[test]
fn golden_rectangles() {
    let doc = add(
        &blank(),
        ShapeKind::Rect {
            rect: Rect::new(10.0, 10.0, 60.0, 40.0),
            filled: false,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Rect {
            rect: Rect::new(80.0, 30.0, 60.0, 50.0),
            filled: true,
        },
        style(BLUE, 2.0, 0.5),
    );
    let doc = add(
        &doc,
        ShapeKind::Rect {
            rect: Rect::new(20.0, 60.0, 30.0, 25.0),
            filled: false,
        },
        style(GREEN, 1.0, 1.0),
    );
    check_golden("rectangles", &render(&doc, None).unwrap());
}

#[test]
fn golden_ellipses() {
    let doc = add(
        &blank(),
        ShapeKind::Ellipse {
            rect: Rect::new(10.0, 10.0, 70.0, 45.0),
            filled: false,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Ellipse {
            rect: Rect::new(90.0, 40.0, 50.0, 50.0),
            filled: true,
        },
        style(GREEN, 3.0, 0.6),
    );
    check_golden("ellipses", &render(&doc, None).unwrap());
}

#[test]
fn golden_lines_and_arrows() {
    let doc = add(
        &blank(),
        ShapeKind::Line {
            from: Point::new(10.0, 90.0),
            to: Point::new(70.0, 10.0),
        },
        style(BLUE, 3.0, 1.0),
    );
    let doc = add(
        &doc,
        ShapeKind::Arrow {
            from: Point::new(20.0, 20.0),
            to: Point::new(140.0, 50.0),
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Arrow {
            from: Point::new(150.0, 90.0),
            to: Point::new(90.0, 70.0),
        },
        style(GREEN, 6.0, 0.5),
    );
    let doc = add(
        &doc,
        ShapeKind::Arrow {
            from: Point::new(100.0, 10.0),
            to: Point::new(104.0, 12.0),
        },
        style(BLUE, 2.0, 1.0),
    );
    check_golden("lines_arrows", &render(&doc, None).unwrap());
}

#[test]
fn golden_pen_and_highlighter() {
    let doc = add(
        &blank(),
        ShapeKind::Pen {
            points: zigzag(10.0, 10.0),
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Pen {
            points: vec![Point::new(140.0, 15.0)],
        },
        style(BLUE, 8.0, 1.0),
    );
    let highlighter = ShapeKind::Highlighter {
        points: vec![Point::new(5.0, 47.0), Point::new(150.0, 47.0)],
    };
    let doc = add(&doc, highlighter, style(Rgba::YELLOW, 4.0, 1.0));
    let doc = add(
        &doc,
        ShapeKind::Highlighter {
            points: zigzag(20.0, 60.0),
        },
        style(Rgba::YELLOW, 3.0, 1.0),
    );
    check_golden("pen_highlighter", &render(&doc, None).unwrap());
}

#[test]
fn golden_mosaic_and_blur() {
    let doc = add(
        &blank(),
        ShapeKind::Mosaic {
            rect: Rect::new(5.0, 40.0, 70.0, 40.0),
            block: 6,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Blur {
            rect: Rect::new(85.0, 30.0, 70.0, 50.0),
            radius: 4.0,
        },
        Style::default(),
    );
    check_golden("mosaic_blur", &render(&doc, None).unwrap());
}

#[test]
fn golden_steps() {
    let font = font();
    let doc = [(25.0, 25.0), (75.0, 50.0), (125.0, 75.0)]
        .into_iter()
        .fold(blank(), |doc, (x, y)| {
            add(
                &doc,
                ShapeKind::Step {
                    center: Point::new(x, y),
                    number: 0,
                },
                Style::default(),
            )
        });
    let doc = (0..7).fold(doc, |doc, _| {
        add(
            &doc,
            ShapeKind::Step {
                center: Point::new(130.0, 20.0),
                number: 0,
            },
            style(BLUE, 1.0, 1.0),
        )
    });
    check_golden("steps", &render(&doc, Some(&font)).unwrap());
}

#[test]
fn golden_text_english() {
    let font = font();
    let text = ShapeKind::Text {
        origin: Point::new(6.0, 8.0),
        text: "Hello, Supercy!".into(),
        font_size: 18.0,
    };
    let doc = add(&blank(), text, Style::default());
    let second = ShapeKind::Text {
        origin: Point::new(6.0, 50.0),
        text: "Demo\n0123456789".into(),
        font_size: 14.0,
    };
    let doc = add(&doc, second, style(BLUE, 1.0, 0.8));
    check_golden("text_english", &render(&doc, Some(&font)).unwrap());
}

#[test]
fn golden_text_chinese() {
    let font = font();
    let text = ShapeKind::Text {
        origin: Point::new(6.0, 8.0),
        text: "示例文本".into(),
        font_size: 24.0,
    };
    let doc = add(&blank(), text, Style::default());
    let mixed = ShapeKind::Text {
        origin: Point::new(6.0, 55.0),
        text: "标注 Demo 示例".into(),
        font_size: 16.0,
    };
    let doc = add(&doc, mixed, style(GREEN, 1.0, 1.0));
    check_golden("text_chinese", &render(&doc, Some(&font)).unwrap());
}

/// The same scene on a 2x (Retina) image: strokes, heads, steps and text scale with it.
#[test]
fn golden_hidpi() {
    let font = font();
    let doc = Document::new(base_image(WIDTH * 2, HEIGHT * 2, 2.0));
    let doc = add(
        &doc,
        ShapeKind::Rect {
            rect: Rect::new(20.0, 20.0, 120.0, 80.0),
            filled: false,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Arrow {
            from: Point::new(40.0, 180.0),
            to: Point::new(280.0, 120.0),
        },
        style(BLUE, 3.0, 1.0),
    );
    let doc = add(
        &doc,
        ShapeKind::Step {
            center: Point::new(250.0, 50.0),
            number: 0,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Text {
            origin: Point::new(30.0, 110.0),
            text: "示例 Demo".into(),
            font_size: 14.0,
        },
        style(GREEN, 1.0, 1.0),
    );
    check_golden("hidpi", &render(&doc, Some(&font)).unwrap());
}

/// Every kind together, cropped: the export path (composite, then crop).
#[test]
fn golden_scene_cropped() {
    let font = font();
    let doc = add(
        &blank(),
        ShapeKind::Mosaic {
            rect: Rect::new(0.0, 45.0, 60.0, 30.0),
            block: 5,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Highlighter {
            points: vec![Point::new(70.0, 47.0), Point::new(150.0, 47.0)],
        },
        style(Rgba::YELLOW, 4.0, 1.0),
    );
    let doc = add(
        &doc,
        ShapeKind::Rect {
            rect: Rect::new(65.0, 35.0, 90.0, 30.0),
            filled: false,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Arrow {
            from: Point::new(20.0, 15.0),
            to: Point::new(64.0, 36.0),
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Step {
            center: Point::new(14.0, 14.0),
            number: 0,
        },
        Style::default(),
    );
    let doc = add(
        &doc,
        ShapeKind::Text {
            origin: Point::new(70.0, 70.0),
            text: "示例 Demo".into(),
            font_size: 14.0,
        },
        style(BLUE, 1.0, 1.0),
    );
    let doc = add(
        &doc,
        ShapeKind::Blur {
            rect: Rect::new(120.0, 5.0, 35.0, 25.0),
            radius: 3.0,
        },
        Style::default(),
    );
    let cropped = doc
        .apply(
            &Command::Crop {
                rect: Some(Rect::new(4.0, 2.0, 150.0, 92.0)),
            },
            None,
        )
        .unwrap()
        .document;
    let rendered = render(&cropped, Some(&font)).unwrap();
    assert_eq!(
        (rendered.x, rendered.y, rendered.width(), rendered.height()),
        (4, 2, 150, 92)
    );
    check_golden("scene_cropped", &rendered);
    let region = render_region(&cropped, Some(&font), Rect::new(110.0, 0.0, 50.0, 40.0)).unwrap();
    check_golden("scene_region", &region);
}
