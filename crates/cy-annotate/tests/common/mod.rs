//! Shared test helpers: generated base images, the font fixture and golden-image comparison.
//!
//! The font fixture `fixtures/DemoSans-subset.ttf` is a subset of Noto Sans SC (SIL Open Font
//! License 1.1, `fixtures/DemoSans-subset.OFL.txt`), instanced at weight 400 and cut down with
//! `fontTools` to the glyphs these tests draw. It is test data only (decision D-7): it is excluded
//! from the package and never linked into the library.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    reason = "helpers shared by several test files; tests fail loudly"
)]

use std::path::PathBuf;

use cy_annotate::{BaseImage, Command, Document, Font, Rendered, ShapeKind, Style};

/// Every character the golden images draw; the font fixture holds exactly these glyphs.
pub const FIXTURE_TEXT: &str = "Hello, Supercy! Demo 示例文本标注 0123456789";

/// Set to rewrite the golden images (`scripts/docker-test.sh golden-write`).
const UPDATE_ENV: &str = "SUPERCY_UPDATE_GOLDEN";
/// A channel may differ by this much (anti-aliasing and float rounding).
const CHANNEL_TOLERANCE: u8 = 8;
/// Share of pixels allowed beyond the channel tolerance.
const MAX_DIFFERENT_SHARE: f64 = 0.005;

/// The font fixture.
pub fn font() -> Font {
    Font::new(
        include_bytes!("../fixtures/DemoSans-subset.ttf").to_vec(),
        0,
    )
    .unwrap()
}

/// A deterministic test picture: a colour gradient with a grid of dark "text" bars and an
/// 8-pixel checkerboard band, so effects and blending are visible.
pub fn base_image(width: u32, height: u32, scale: f32) -> BaseImage {
    let mut rgba = Vec::with_capacity(usize::try_from(width * height * 4).unwrap());
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&pixel(x, y, width, height));
        }
    }
    BaseImage::from_rgba(width, height, rgba, scale).unwrap()
}

fn pixel(x: u32, y: u32, width: u32, height: u32) -> [u8; 4] {
    let checker_band = y >= height / 2 && y < height / 2 + height / 4;
    if checker_band {
        let dark = (x / 8 + y / 8).is_multiple_of(2);
        return if dark {
            [30, 30, 40, 255]
        } else {
            [240, 240, 230, 255]
        };
    }
    let bar = y % 20 >= 6 && y % 20 < 10 && x % 40 < 30;
    if bar {
        return [50, 50, 60, 255];
    }
    let r = u8::try_from(120 + x * 120 / width).unwrap();
    let g = u8::try_from(140 + y * 100 / height).unwrap();
    [r, g, 220, 255]
}

/// `document` with `kind` added in `style`.
pub fn add(document: &Document, kind: ShapeKind, style: Style) -> Document {
    document
        .apply(&Command::Add { kind, style }, None)
        .unwrap()
        .document
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

/// Compares `rendered` with the committed golden image `name`, or rewrites it when
/// `SUPERCY_UPDATE_GOLDEN` is set.
pub fn check_golden(name: &str, rendered: &Rendered) {
    let path = golden_path(name);
    let actual = rendered.to_rgba();
    if std::env::var_os(UPDATE_ENV).is_some() {
        std::fs::write(&path, encode(rendered.width(), rendered.height(), &actual)).unwrap();
        return;
    }
    let Ok(bytes) = std::fs::read(&path) else {
        panic!("missing golden image {name}.png; run scripts/docker-test.sh golden-write");
    };
    let (width, height, expected) = decode(&bytes);
    assert_eq!(
        (width, height),
        (rendered.width(), rendered.height()),
        "{name}: size"
    );
    let different = actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .filter(|(a, e)| {
            a.iter()
                .zip(*e)
                .any(|(a, e)| a.abs_diff(*e) > CHANNEL_TOLERANCE)
        })
        .count();
    let allowed = f64::from(width * height) * MAX_DIFFERENT_SHARE;
    assert!(
        f64::from(u32::try_from(different).unwrap()) <= allowed,
        "{name}: {different} pixels differ from the golden image"
    );
}

/// RGBA PNG bytes.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
    writer.finish().unwrap();
    out
}

/// Width, height and RGBA (or RGB) pixels of PNG bytes, expanded to RGBA.
pub fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).unwrap();
    let data = &buffer[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        other => panic!("unexpected PNG colour type {other:?}"),
    };
    (frame.width, frame.height, rgba)
}
