//! Screenshots and edited exports end to end (M4-06): the 20 MB screenshot limit (D-10), the
//! shared image total, thumbnails, and an edited export saved as a new item.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use cy_annotate::{BaseImage, Command, Document, ExportFormat, Point, ShapeKind, Style, export};
use cy_engine::{
    ANNOTATION_SOURCE_ID, DropReason, Engine, ImageOrigin, IngestOutcome, MAX_SCREENSHOT_BYTES,
    OwnImage, SCREENSHOT_SOURCE_ID, Settings,
};
use cy_model::{ImageFormat, ItemId, RepKind, Representation};
use cy_platform::MemoryKeyStore;

const T0: u64 = 1_700_000_000_000;
const MIB: usize = 1024 * 1024;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// PNG-signed demo bytes of `size` (the engine checks the signature, not the whole image).
fn png_like(size: usize, fill: u8) -> Vec<u8> {
    let mut bytes = PNG_SIGNATURE.to_vec();
    bytes.resize(size, fill);
    bytes
}

fn screenshot(bytes: Vec<u8>) -> OwnImage {
    OwnImage {
        format: ImageFormat::Png,
        bytes,
        width: 64,
        height: 48,
        thumbnail: None,
        origin: ImageOrigin::Screenshot,
    }
}

fn stored(engine: &mut Engine, image: OwnImage, at: u64) -> ItemId {
    match engine.ingest_image(image, at).unwrap().outcome {
        IngestOutcome::Stored { id, .. } => id,
        other => panic!("expected Stored, got {other:?}"),
    }
}

fn open(dir: &tempfile::TempDir, keys: &MemoryKeyStore) -> Engine {
    Engine::open(dir.path(), keys, T0).unwrap()
}

#[test]
fn a_screenshot_above_the_image_limit_but_within_20_mb_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = open(&dir, &keys);
    let mut image = screenshot(png_like(12 * MIB, 7));
    image.thumbnail = Some(png_like(200, 1));
    let id = stored(&mut engine, image, T0);

    let summary = &engine.list(Some(RepKind::Image), 0, 10)[0];
    assert_eq!(summary.id, id);
    assert_eq!(summary.image_size, Some((64, 48)));
    assert_eq!(
        summary.source.as_ref().unwrap().bundle_id,
        SCREENSHOT_SOURCE_ID
    );
    assert_eq!(engine.thumbnail(id).unwrap().unwrap().len(), 200);
    match &engine.payload(id).unwrap()[0] {
        Representation::Image { format, bytes, .. } => {
            assert_eq!(*format, ImageFormat::Png);
            assert_eq!(bytes.len(), 12 * MIB);
        }
        other => panic!("expected an image, got {other:?}"),
    }
}

#[test]
fn screenshots_over_20_mb_or_not_png_or_jpeg_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = open(&dir, &keys);
    let cases = [
        (
            screenshot(png_like(MAX_SCREENSHOT_BYTES + 1, 0)),
            DropReason::TooLarge,
        ),
        (
            screenshot(b"not an image".to_vec()),
            DropReason::Unsupported,
        ),
        (
            OwnImage {
                format: ImageFormat::Tiff,
                ..screenshot(png_like(100, 0))
            },
            DropReason::Unsupported,
        ),
        (
            OwnImage {
                width: 0,
                ..screenshot(png_like(100, 0))
            },
            DropReason::Unsupported,
        ),
        (
            OwnImage {
                format: ImageFormat::Jpeg,
                ..screenshot(png_like(100, 0))
            },
            DropReason::Unsupported,
        ),
    ];
    for (image, reason) in cases {
        let outcome = engine.ingest_image(image, T0).unwrap().outcome;
        assert_eq!(outcome, IngestOutcome::Dropped { reason });
    }
    assert!(engine.is_empty());
}

#[test]
fn screenshots_are_kept_while_recording_is_paused() {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = open(&dir, &keys);
    engine.pause(Some(T0 + 60_000));
    stored(&mut engine, screenshot(png_like(1000, 3)), T0 + 1);
    assert_eq!(engine.len(), 1);
}

#[test]
fn screenshots_count_toward_the_image_total() {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = open(&dir, &keys);
    engine
        .update_settings(Settings {
            max_image_total_bytes: 2 * MIB as u64,
            ..Settings::default()
        })
        .unwrap();
    let first = stored(&mut engine, screenshot(png_like(800 * 1024, 1)), T0);
    let second = stored(&mut engine, screenshot(png_like(800 * 1024, 2)), T0 + 1);
    let third = match engine
        .ingest_image(screenshot(png_like(800 * 1024, 3)), T0 + 2)
        .unwrap()
        .outcome
    {
        IngestOutcome::Stored { id, evicted } => {
            assert_eq!(evicted, [first]);
            id
        }
        other => panic!("expected Stored, got {other:?}"),
    };
    let ids: Vec<ItemId> = engine.list(None, 0, 10).iter().map(|s| s.id).collect();
    assert_eq!(ids, [third, second]);
}

#[test]
fn the_same_screenshot_again_moves_to_the_top() {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = open(&dir, &keys);
    let id = stored(&mut engine, screenshot(png_like(500, 9)), T0);
    stored(&mut engine, screenshot(png_like(500, 8)), T0 + 1);
    let again = engine
        .ingest_image(screenshot(png_like(500, 9)), T0 + 2)
        .unwrap();
    assert_eq!(again.outcome, IngestOutcome::Updated { id });
    assert_eq!(engine.list(None, 0, 1)[0].id, id);
}

fn demo_png() -> Vec<u8> {
    let base = BaseImage::from_rgba(64, 48, vec![200; 64 * 48 * 4], 1.0).unwrap();
    export(&Document::new(base), None, ExportFormat::Png).unwrap()
}

#[test]
fn an_edited_export_is_a_new_item_and_the_original_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = open(&dir, &keys);
    let original_png = demo_png();
    let original = stored(&mut engine, screenshot(original_png.clone()), T0);

    let Representation::Image { bytes, .. } = &engine.payload(original).unwrap()[0] else {
        panic!("expected an image");
    };
    let base = BaseImage::from_png(bytes.expose(), 1.0).unwrap();
    let arrow = ShapeKind::Arrow {
        from: Point::new(5.0, 5.0),
        to: Point::new(50.0, 40.0),
    };
    let edited = Document::new(base)
        .apply(
            &Command::Add {
                kind: arrow,
                style: Style::default(),
            },
            None,
        )
        .unwrap()
        .document;
    let edited_png = export(&edited, None, ExportFormat::Png).unwrap();
    let copy = stored(
        &mut engine,
        OwnImage {
            origin: ImageOrigin::Edited,
            ..screenshot(edited_png.clone())
        },
        T0 + 1,
    );

    assert_ne!(copy, original);
    assert_eq!(engine.len(), 2);
    let payload_of = |id| match &engine.payload(id).unwrap()[0] {
        Representation::Image { bytes, .. } => bytes.expose().to_vec(),
        other => panic!("expected an image, got {other:?}"),
    };
    assert_eq!(payload_of(original), original_png);
    assert_eq!(payload_of(copy), edited_png);
    let summary = &engine.list(None, 0, 1)[0];
    assert_eq!(
        summary.source.as_ref().unwrap().bundle_id,
        ANNOTATION_SOURCE_ID
    );

    drop(engine);
    let reopened = open(&dir, &keys);
    assert_eq!(reopened.len(), 2);
}
