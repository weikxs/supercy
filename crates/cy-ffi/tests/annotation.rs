//! The annotation FFI the way the Swift editor uses it: open a screenshot, apply gestures as
//! commands, undo and redo, query geometry, preview a region, export, save and reopen, and
//! store screenshots and edited exports in the history.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cy_ffi::{
    AnnotationColor, AnnotationCommand, AnnotationDocument, AnnotationError, AnnotationPoint,
    AnnotationRect, AnnotationShapeKind, AnnotationStyle, CapturedImage, DropReason,
    ExportImageFormat, FileNameParts, ImageFormat, ImageOrigin, IngestOutcome, Keystore,
    KeystoreError, PasteRepresentation, ResizeHandle, SupercyEngine, annotation_file_name,
    annotation_source_id, annotation_style_constants, annotation_target_path, max_screenshot_bytes,
    screenshot_source_id,
};

const T0: u64 = 1_700_000_000_000;
const FONT: &[u8] = include_bytes!("../../cy-annotate/tests/fixtures/DemoSans-subset.ttf");

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|i| [u8::try_from(i % 200).unwrap(), 120, 200])
        .collect();
    writer.write_image_data(&pixels).unwrap();
    writer.finish().unwrap();
    out
}

fn style() -> AnnotationStyle {
    AnnotationStyle {
        color: AnnotationColor {
            r: 235,
            g: 64,
            b: 52,
            a: 255,
        },
        stroke_width: 4.0,
        opacity: 1.0,
    }
}

fn point(x: f32, y: f32) -> AnnotationPoint {
    AnnotationPoint { x, y }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> AnnotationRect {
    AnnotationRect {
        x,
        y,
        width,
        height,
    }
}

fn add(document: &AnnotationDocument, kind: AnnotationShapeKind) -> u64 {
    document
        .apply(AnnotationCommand::Add {
            kind,
            style: style(),
        })
        .unwrap()
        .unwrap()
}

#[test]
#[allow(clippy::too_many_lines, reason = "one editing session, step by step")]
fn editing_round_trip() {
    let document = AnnotationDocument::from_png(png(80, 60), 2.0).unwrap();
    assert_eq!(
        (
            document.image_width(),
            document.image_height(),
            document.image_scale()
        ),
        (80, 60, 2.0)
    );
    document.set_font(Some(FONT.to_vec()), 0).unwrap();
    let arrow = add(
        &document,
        AnnotationShapeKind::Arrow {
            from: point(5.0, 5.0),
            to: point(60.0, 40.0),
        },
    );
    let text = add(
        &document,
        AnnotationShapeKind::Text {
            origin: point(10.0, 30.0),
            text: "示例".into(),
            font_size: 8.0,
        },
    );
    let step = add(
        &document,
        AnnotationShapeKind::Step {
            center: point(70.0, 10.0),
            number: 0,
        },
    );
    assert_eq!(document.next_step_number(), 2);
    assert!(matches!(
        document.shape(step).unwrap().kind,
        AnnotationShapeKind::Step { number: 1, .. }
    ));
    assert_eq!(document.shapes().len(), 3);

    document
        .apply(AnnotationCommand::Move {
            ids: vec![arrow],
            dx: 2.0,
            dy: 0.0,
        })
        .unwrap();
    document
        .apply(AnnotationCommand::Resize {
            id: arrow,
            handle: ResizeHandle::End,
            to: point(70.0, 50.0),
        })
        .unwrap();
    document
        .apply(AnnotationCommand::Restyle {
            ids: vec![text],
            style: AnnotationStyle {
                opacity: 0.5,
                ..style()
            },
        })
        .unwrap();
    document
        .apply(AnnotationCommand::EditText {
            id: text,
            text: "Demo".into(),
            font_size: 10.0,
        })
        .unwrap();
    document
        .apply(AnnotationCommand::SendToBack { ids: vec![step] })
        .unwrap();
    document
        .apply(AnnotationCommand::BringToFront { ids: vec![step] })
        .unwrap();
    document
        .apply(AnnotationCommand::Crop {
            rect: Some(rect(2.0, 2.0, 70.0, 50.0)),
        })
        .unwrap();
    assert_eq!(document.crop(), Some(rect(2.0, 2.0, 70.0, 50.0)));

    assert_eq!(document.hit_test(point(70.0, 10.0)), Some(step));
    assert!(document.selection_bounds(vec![arrow, step]).is_some());
    assert_eq!(
        document.shapes_in(rect(60.0, 0.0, 20.0, 20.0)),
        vec![arrow, step]
    );
    let preview = document.render_region(rect(0.0, 0.0, 20.0, 10.0)).unwrap();
    assert_eq!(
        (preview.width, preview.height, preview.rgba.len()),
        (20, 10, 800)
    );

    let png_out = document.export(ExportImageFormat::Png).unwrap();
    assert!(png_out.starts_with(b"\x89PNG"));
    let jpeg = document
        .export(ExportImageFormat::Jpeg { quality: 80 })
        .unwrap();
    assert!(jpeg.starts_with(&[0xFF, 0xD8]));

    let saved = document.save().unwrap();
    let reopened = AnnotationDocument::from_saved(png(80, 60), 2.0, saved).unwrap();
    assert_eq!(reopened.shapes(), document.shapes());

    while document.undo() {}
    assert!(document.shapes().is_empty() && !document.can_undo() && document.can_redo());
    assert!(document.redo());
    assert_eq!(document.shapes().len(), 1);
    document
        .apply(AnnotationCommand::Erase {
            path: vec![point(5.0, 5.0), point(30.0, 20.0)],
            radius: 4.0,
        })
        .unwrap();
    document
        .apply(AnnotationCommand::Crop { rect: None })
        .unwrap();
    assert!(document.shapes().is_empty());
    document.set_font(None, 0).unwrap();
}

#[test]
fn errors_map_to_their_swift_cases() {
    assert!(matches!(
        AnnotationDocument::from_png(b"no".to_vec(), 1.0),
        Err(AnnotationError::InvalidImage { .. })
    ));
    assert!(matches!(
        AnnotationDocument::from_saved(png(4, 4), 1.0, b"x".to_vec()),
        Err(AnnotationError::InvalidDocument { .. })
    ));
    let document = AnnotationDocument::from_png(png(10, 10), 1.0).unwrap();
    assert!(matches!(
        document.set_font(Some(b"x".to_vec()), 0),
        Err(AnnotationError::InvalidFont { .. })
    ));
    assert!(matches!(
        document.apply(AnnotationCommand::Delete { ids: vec![7] }),
        Err(AnnotationError::UnknownShape { .. })
    ));
    let nan = AnnotationCommand::Move {
        ids: vec![],
        dx: f32::NAN,
        dy: 0.0,
    };
    assert!(matches!(
        document.apply(nan),
        Err(AnnotationError::InvalidInput { .. })
    ));
    let too_many = AnnotationShapeKind::Pen {
        points: vec![point(1.0, 1.0); 20_000],
    };
    assert!(matches!(
        document.apply(AnnotationCommand::Add {
            kind: too_many,
            style: style()
        }),
        Err(AnnotationError::LimitExceeded { .. })
    ));
    assert!(matches!(
        document.export(ExportImageFormat::Jpeg { quality: 0 }),
        Err(AnnotationError::Export { .. })
    ));
    assert!(matches!(
        document.render_region(rect(50.0, 50.0, 5.0, 5.0)),
        Err(AnnotationError::InvalidInput { .. })
    ));
    assert!(format!("{document:?}").contains("AnnotationDocument"));
}

#[test]
fn every_shape_kind_crosses_the_boundary() {
    let document = AnnotationDocument::from_png(png(40, 40), 1.0).unwrap();
    let kinds = vec![
        AnnotationShapeKind::Rect {
            rect: rect(1.0, 1.0, 5.0, 5.0),
            filled: true,
        },
        AnnotationShapeKind::Ellipse {
            rect: rect(1.0, 1.0, 5.0, 5.0),
            filled: false,
        },
        AnnotationShapeKind::Line {
            from: point(0.0, 0.0),
            to: point(5.0, 5.0),
        },
        AnnotationShapeKind::Pen {
            points: vec![point(1.0, 1.0), point(2.0, 3.0)],
        },
        AnnotationShapeKind::Highlighter {
            points: vec![point(1.0, 1.0)],
        },
        AnnotationShapeKind::Mosaic {
            rect: rect(0.0, 0.0, 10.0, 10.0),
            block: 4,
        },
        AnnotationShapeKind::Blur {
            rect: rect(0.0, 0.0, 10.0, 10.0),
            radius: 2.0,
        },
    ];
    for kind in &kinds {
        add(&document, kind.clone());
    }
    let back: Vec<AnnotationShapeKind> = document.shapes().into_iter().map(|s| s.kind).collect();
    assert_eq!(back, kinds);
    for handle in [
        ResizeHandle::TopLeft,
        ResizeHandle::Top,
        ResizeHandle::TopRight,
        ResizeHandle::Right,
        ResizeHandle::BottomRight,
        ResizeHandle::Bottom,
        ResizeHandle::BottomLeft,
        ResizeHandle::Left,
    ] {
        document
            .apply(AnnotationCommand::Resize {
                id: 1,
                handle,
                to: point(8.0, 8.0),
            })
            .unwrap();
    }
    document
        .apply(AnnotationCommand::Resize {
            id: 3,
            handle: ResizeHandle::Start,
            to: point(2.0, 2.0),
        })
        .unwrap();
}

#[test]
fn constants_and_file_names() {
    let constants = annotation_style_constants();
    assert!(
        constants.round_caps && constants.highlighter_alpha > 0.0 && constants.hit_tolerance > 0.0
    );
    let parts = FileNameParts {
        year: 2026,
        month: 1,
        day: 2,
        hour: 3,
        minute: 4,
        second: 5,
        n: 6,
    };
    assert_eq!(
        annotation_file_name(
            "Shot {date} {time} {n}".into(),
            parts,
            ExportImageFormat::Png
        )
        .unwrap(),
        "Shot 2026-01-02 03-04-05 6.png"
    );
    let bad = FileNameParts { month: 0, ..parts };
    assert!(matches!(
        annotation_file_name("x".into(), bad, ExportImageFormat::Png),
        Err(AnnotationError::Export { .. })
    ));
    assert_eq!(
        annotation_target_path("/tmp/supercy-test".into(), "a.png".into()).unwrap(),
        "/tmp/supercy-test/a.png"
    );
    assert!(annotation_target_path("tmp".into(), "a.png".into()).is_err());
}

#[derive(Default)]
struct DemoKeychain(Mutex<HashMap<String, Vec<u8>>>);

impl Keystore for DemoKeychain {
    fn read(&self, account: String) -> Result<Option<Vec<u8>>, KeystoreError> {
        Ok(self.0.lock().unwrap().get(&account).cloned())
    }

    fn write(&self, account: String, secret: Vec<u8>) -> Result<(), KeystoreError> {
        self.0.lock().unwrap().insert(account, secret);
        Ok(())
    }

    fn delete(&self, account: String) -> Result<(), KeystoreError> {
        self.0.lock().unwrap().remove(&account);
        Ok(())
    }
}

#[test]
fn screenshots_and_edited_exports_enter_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let engine = SupercyEngine::open(
        dir.path().to_string_lossy().into_owned(),
        Arc::new(DemoKeychain::default()),
        T0,
    )
    .unwrap();
    let shot = png(30, 20);
    let image = |bytes: Vec<u8>, origin| CapturedImage {
        format: ImageFormat::Png,
        bytes,
        width: 30,
        height: 20,
        thumbnail: Some(png(3, 2)),
        origin,
    };
    let original = match engine
        .ingest_image(image(shot.clone(), ImageOrigin::Screenshot), T0)
        .unwrap()
        .outcome
    {
        IngestOutcome::Stored { id, .. } => id,
        other => panic!("expected Stored, got {other:?}"),
    };
    let document = AnnotationDocument::from_png(shot.clone(), 1.0).unwrap();
    add(
        &document,
        AnnotationShapeKind::Line {
            from: point(0.0, 0.0),
            to: point(30.0, 20.0),
        },
    );
    let edited = document.export(ExportImageFormat::Png).unwrap();
    let outcome = engine
        .ingest_image(image(edited, ImageOrigin::Edited), T0 + 1)
        .unwrap()
        .outcome;
    assert!(matches!(outcome, IngestOutcome::Stored { .. }));
    assert_eq!(engine.item_count().unwrap(), 2);
    match &engine.payload(original.clone()).unwrap()[0] {
        PasteRepresentation::Image { bytes, .. } => assert_eq!(bytes, &shot),
        other => panic!("expected an image, got {other:?}"),
    }
    assert!(engine.thumbnail(original).unwrap().is_some());
    let gif = CapturedImage {
        format: ImageFormat::Gif,
        ..image(vec![1, 2, 3], ImageOrigin::Screenshot)
    };
    let dropped = engine.ingest_image(gif, T0 + 2).unwrap().outcome;
    assert_eq!(
        dropped,
        IngestOutcome::Dropped {
            reason: DropReason::Unsupported
        }
    );
    assert_eq!(max_screenshot_bytes(), 20 * 1024 * 1024);
    assert_ne!(screenshot_source_id(), annotation_source_id());
}
