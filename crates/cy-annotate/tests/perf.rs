//! Export and preview budgets (NFR-13), measured in release mode with
//! `scripts/docker-test.sh bench`. Times are asserted in release builds only.

#![allow(
    clippy::unwrap_used,
    clippy::print_stderr,
    reason = "the benchmark reports its measurements"
)]

mod common;

use std::time::{Duration, Instant};

use cy_annotate::{
    Document, ExportFormat, Point, Rect, Rgba, ShapeKind, Style, export, render_region,
};

/// A 5K screenshot.
const WIDTH: u32 = 5120;
const HEIGHT: u32 = 2880;
/// Export of a 5K image with 50 shapes.
const EXPORT_BUDGET: Duration = Duration::from_secs(1);
/// Preview of one mosaic or blur area.
const PREVIEW_BUDGET: Duration = Duration::from_millis(16);

fn kind(n: u16) -> ShapeKind {
    let x = f32::from(n % 10) * 500.0 + 40.0;
    let y = f32::from(n / 10) * 540.0 + 40.0;
    match n % 8 {
        0 => ShapeKind::Rect {
            rect: Rect::new(x, y, 400.0, 300.0),
            filled: false,
        },
        1 => ShapeKind::Ellipse {
            rect: Rect::new(x, y, 400.0, 300.0),
            filled: n.is_multiple_of(3),
        },
        2 => ShapeKind::Arrow {
            from: Point::new(x, y),
            to: Point::new(x + 420.0, y + 380.0),
        },
        3 => ShapeKind::Pen {
            points: (0..200u16)
                .map(|i| Point::new(x + f32::from(i) * 2.0, y + f32::from(i % 20) * 10.0))
                .collect(),
        },
        4 => ShapeKind::Highlighter {
            points: vec![Point::new(x, y + 100.0), Point::new(x + 450.0, y + 100.0)],
        },
        5 => ShapeKind::Step {
            center: Point::new(x + 50.0, y + 50.0),
            number: 1,
        },
        6 => ShapeKind::Mosaic {
            rect: Rect::new(x, y, 400.0, 300.0),
            block: 8,
        },
        _ => ShapeKind::Blur {
            rect: Rect::new(x, y, 400.0, 300.0),
            radius: 8.0,
        },
    }
}

fn document() -> Document {
    let base = common::base_image(WIDTH, HEIGHT, 2.0);
    (0..50).fold(Document::new(base), |doc, n| {
        let style = Style {
            color: Rgba::RED,
            stroke_width: 4.0,
            opacity: 1.0,
        };
        common::add(&doc, kind(n), style)
    })
}

#[test]
#[ignore = "benchmark; run with scripts/docker-test.sh bench"]
fn perf_export_5k_png_with_50_shapes() {
    let doc = document();
    let font = common::font();
    let mut times: Vec<Duration> = (0..3)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(export(&doc, Some(&font), ExportFormat::Png).unwrap());
            start.elapsed()
        })
        .collect();
    times.sort();
    let median = times[1];
    let jpeg_start = Instant::now();
    let jpeg = export(&doc, Some(&font), ExportFormat::Jpeg { quality: 85 }).unwrap();
    let jpeg_time = jpeg_start.elapsed();
    let blur_start = Instant::now();
    std::hint::black_box(render_region(&doc, None, Rect::new(3540.0, 40.0, 400.0, 300.0)).unwrap());
    let blur_preview = blur_start.elapsed();
    eprintln!(
        "perf: 5K export with 50 shapes: PNG median {median:?} (runs {times:?}); JPEG q85 {jpeg_time:?} \
         ({} bytes); blur area preview {blur_preview:?}",
        jpeg.len()
    );
    if !cfg!(debug_assertions) {
        assert!(median <= EXPORT_BUDGET, "PNG export {median:?}");
        assert!(
            blur_preview <= PREVIEW_BUDGET,
            "blur preview {blur_preview:?}"
        );
    }
}
