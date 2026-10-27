//! Fonts, text measurement, the editor's previews and export encoding.

#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

mod common;

use common::{FIXTURE_TEXT, add, base_image, decode, font};
use cy_annotate::{
    AnnotateError, BaseImage, Command, Document, Editor, ExportFormat, Font, Point, Rect,
    STYLE_CONSTANTS, ShapeKind, Style, encode_jpeg, export, measure_text, render,
};

#[test]
fn the_fixture_font_measures_text() {
    let font = font();
    assert!(format!("{font:?}").contains("index: 0"));
    let one = measure_text(Some(&font), "Hello", 20.0);
    let two = measure_text(Some(&font), "Hello\n示例文本", 20.0);
    assert!(one.width > 30.0 && one.width < 70.0, "{}", one.width);
    assert!(
        two.width > one.width,
        "four CJK characters are wider than Hello"
    );
    assert!((two.height - 2.0 * STYLE_CONSTANTS.text_line_height * 20.0).abs() < 1e-3);
    assert!(one.ascent > 15.0 && one.ascent < 26.0, "{}", one.ascent);
    assert!(
        font.line_width("Z", 20.0) > 0.0,
        "a missing glyph uses the fallback glyph"
    );
    assert!(
        FIXTURE_TEXT
            .chars()
            .all(|c| c == ' ' || font.line_width(&c.to_string(), 10.0) > 0.0)
    );
}

#[test]
fn bad_font_data_and_indexes_are_refused() {
    let bytes = include_bytes!("fixtures/DemoSans-subset.ttf").to_vec();
    assert_eq!(Font::new(bytes, 3).err(), Some(AnnotateError::InvalidFont));
    let mut truncated = include_bytes!("fixtures/DemoSans-subset.ttf").to_vec();
    truncated.truncate(64);
    assert_eq!(
        Font::new(truncated, 0).err(),
        Some(AnnotateError::InvalidFont)
    );
}

#[test]
fn the_editor_measures_text_with_its_font_and_renders_previews() {
    let mut editor = Editor::new(Document::new(base_image(120, 80, 1.0)));
    let kind = ShapeKind::Text {
        origin: Point::new(10.0, 10.0),
        text: "示例文本".into(),
        font_size: 20.0,
    };
    let id = editor
        .apply(&Command::Add {
            kind,
            style: Style::default(),
        })
        .unwrap()
        .unwrap();
    let estimated = editor.selection_bounds(&[id]).unwrap();
    editor.set_font(Some(font()));
    let measured = editor.selection_bounds(&[id]).unwrap();
    assert!((estimated.width - 80.0).abs() < 1e-3);
    assert!((measured.width - 80.0).abs() < 4.0, "{}", measured.width);
    assert_eq!(editor.hit_test(Point::new(30.0, 20.0)), Some(id));
    let preview = editor
        .render_region(Rect::new(5.0, 5.0, 30.0, 30.0))
        .unwrap();
    assert_eq!(
        (preview.x, preview.y, preview.width(), preview.height()),
        (5, 5, 30, 30)
    );
    let drawn = preview
        .to_rgba()
        .chunks_exact(4)
        .filter(|p| p[0] > 200 && p[1] < 120)
        .count();
    assert!(drawn > 20, "red text pixels in the preview: {drawn}");
    assert!(
        editor
            .export(ExportFormat::Png)
            .unwrap()
            .starts_with(b"\x89PNG")
    );
}

#[test]
fn png_export_is_the_cropped_composite() {
    let doc = add(
        &Document::new(base_image(60, 40, 1.0)),
        ShapeKind::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(60.0, 40.0),
        },
        Style::default(),
    );
    let cropped = doc
        .apply(
            &Command::Crop {
                rect: Some(Rect::new(10.0, 5.0, 30.0, 20.0)),
            },
            None,
        )
        .unwrap()
        .document;
    let png = export(&cropped, None, ExportFormat::Png).unwrap();
    let (width, height, pixels) = decode(&png);
    assert_eq!((width, height), (30, 20));
    assert_eq!(pixels, render(&cropped, None).unwrap().to_rgba());
    assert_eq!(
        decode(&export(&doc, None, ExportFormat::Png).unwrap()).0,
        60
    );
}

#[test]
fn png_export_keeps_transparency() {
    let base = BaseImage::from_rgba(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 100], 1.0).unwrap();
    let png = export(&Document::new(base), None, ExportFormat::Png).unwrap();
    let (_, _, pixels) = decode(&png);
    assert_eq!(pixels[..4], [255, 0, 0, 255]);
    assert_eq!(pixels[7], 100);
}

#[test]
fn jpeg_export_checks_quality_and_encodes() {
    let doc = Document::new(base_image(64, 48, 1.0));
    for quality in [0, 101] {
        assert_eq!(
            export(&doc, None, ExportFormat::Jpeg { quality }),
            Err(AnnotateError::InvalidQuality)
        );
    }
    let low = export(&doc, None, ExportFormat::Jpeg { quality: 10 }).unwrap();
    let high = export(&doc, None, ExportFormat::Jpeg { quality: 95 }).unwrap();
    assert!(low.starts_with(&[0xFF, 0xD8]) && low.ends_with(&[0xFF, 0xD9]));
    assert!(high.len() > low.len());
    assert_eq!(ExportFormat::Jpeg { quality: 50 }.extension(), "jpg");
    let rendered = render(&doc, None).unwrap();
    assert!(encode_jpeg(&rendered, 80).is_ok());
    assert_eq!(
        encode_jpeg(&rendered, 0),
        Err(AnnotateError::InvalidQuality)
    );
}

/// Mean absolute difference per channel between the decoded JPEG and the composite.
fn jpeg_error(document: &Document, quality: u8) -> f64 {
    let jpeg = export(document, None, ExportFormat::Jpeg { quality }).unwrap();
    let mut decoder =
        zune_jpeg::JpegDecoder::new(zune_jpeg::zune_core::bytestream::ZCursor::new(&jpeg));
    let pixels = decoder.decode().unwrap();
    let expected = render(document, None).unwrap().to_rgb_on_white();
    assert_eq!(pixels.len(), expected.len());
    let total: u64 = pixels
        .iter()
        .zip(&expected)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    #[allow(clippy::cast_precision_loss, reason = "test images are small")]
    let mean = total as f64 / expected.len() as f64;
    mean
}

#[test]
fn jpeg_output_decodes_close_to_the_composite() {
    // Odd sizes exercise the repeated edge pixels of partial blocks.
    let doc = add(
        &Document::new(base_image(83, 45, 1.0)),
        ShapeKind::Arrow {
            from: Point::new(5.0, 5.0),
            to: Point::new(70.0, 40.0),
        },
        Style::default(),
    );
    let fine = jpeg_error(&doc, 95);
    let coarse = jpeg_error(&doc, 20);
    assert!(fine < 3.0, "mean error at quality 95: {fine}");
    assert!(
        coarse < 12.0 && coarse > fine,
        "mean error at quality 20: {coarse}"
    );
}
