//! Properties of editing (design 05, section 3): any command sequence followed by as many undos
//! gives back the original document, redo after undo gives back the result, the eraser never
//! touches the base image, crops stay inside the image, and every document survives encoding.

#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

mod common;

use cy_annotate::{
    Arrange, Command, Document, Editor, Handle, Point, Rect, Rgba, ShapeId, ShapeKind, Style,
};
use proptest::prelude::*;

fn coordinate() -> impl Strategy<Value = f32> {
    -40.0f32..160.0
}

fn point() -> impl Strategy<Value = Point> {
    (coordinate(), coordinate()).prop_map(|(x, y)| Point::new(x, y))
}

fn rect() -> impl Strategy<Value = Rect> {
    (coordinate(), coordinate(), 0.0f32..80.0, 0.0f32..80.0)
        .prop_map(|(x, y, w, h)| Rect::new(x, y, w, h))
}

fn style() -> impl Strategy<Value = Style> {
    (any::<[u8; 4]>(), 0.5f32..12.0, 0.0f32..=1.0).prop_map(
        |([r, g, b, a], stroke_width, opacity)| Style {
            color: Rgba { r, g, b, a },
            stroke_width,
            opacity,
        },
    )
}

fn kind() -> impl Strategy<Value = ShapeKind> {
    prop_oneof![
        (rect(), any::<bool>()).prop_map(|(rect, filled)| ShapeKind::Rect { rect, filled }),
        (rect(), any::<bool>()).prop_map(|(rect, filled)| ShapeKind::Ellipse { rect, filled }),
        (point(), point()).prop_map(|(from, to)| ShapeKind::Line { from, to }),
        (point(), point()).prop_map(|(from, to)| ShapeKind::Arrow { from, to }),
        prop::collection::vec(point(), 1..6).prop_map(|points| ShapeKind::Pen { points }),
        prop::collection::vec(point(), 1..6).prop_map(|points| ShapeKind::Highlighter { points }),
        (point(), "[a-z示例 ]{1,8}", 4.0f32..40.0).prop_map(|(origin, text, font_size)| {
            ShapeKind::Text {
                origin,
                text,
                font_size,
            }
        }),
        point().prop_map(|center| ShapeKind::Step { center, number: 1 }),
        (rect(), 1u32..20).prop_map(|(rect, block)| ShapeKind::Mosaic { rect, block }),
        (rect(), 0.5f32..20.0).prop_map(|(rect, radius)| ShapeKind::Blur { rect, radius }),
    ]
}

fn ids() -> impl Strategy<Value = Vec<ShapeId>> {
    prop::collection::vec((0u64..8).prop_map(ShapeId), 0..3)
}

fn command() -> impl Strategy<Value = Command> {
    prop_oneof![
        3 => (kind(), style()).prop_map(|(kind, style)| Command::Add { kind, style }),
        1 => (ids(), -30.0f32..30.0, -30.0f32..30.0).prop_map(|(ids, dx, dy)| Command::Move { ids, dx, dy }),
        1 => ((0u64..8).prop_map(ShapeId), prop::sample::select(Handle::ALL.to_vec()), point())
            .prop_map(|(id, handle, to)| Command::Resize { id, handle, to }),
        1 => (ids(), style()).prop_map(|(ids, style)| Command::Restyle { ids, style }),
        1 => ((0u64..8).prop_map(ShapeId), "[A-Za-z示例]{0,6}", 2.0f32..30.0)
            .prop_map(|(id, text, font_size)| Command::EditText { id, text, font_size }),
        1 => ids().prop_map(|ids| Command::Delete { ids }),
        1 => (prop::collection::vec(point(), 1..4), 0.5f32..20.0)
            .prop_map(|(path, radius)| Command::Erase { path, radius }),
        1 => proptest::option::of(rect()).prop_map(|rect| Command::Crop { rect }),
        1 => (ids(), any::<bool>()).prop_map(|(ids, front)| Command::Arrange {
            ids,
            to: if front { Arrange::Front } else { Arrange::Back },
        }),
    ]
}

fn editor() -> Editor {
    Editor::new(Document::new(common::base_image(120, 90, 1.0)))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn undo_returns_to_the_original_and_redo_to_the_result(commands in prop::collection::vec(command(), 0..25)) {
        let mut editor = editor();
        let original = editor.document().clone();
        for command in &commands {
            let before = editor.document().clone();
            if editor.apply(command).is_err() {
                prop_assert_eq!(editor.document(), &before, "a failed command changes nothing");
            }
        }
        let result = editor.document().clone();
        let mut undone = 0;
        while editor.undo() {
            undone += 1;
        }
        prop_assert_eq!(editor.document(), &original);
        for _ in 0..undone {
            prop_assert!(editor.redo());
        }
        prop_assert_eq!(editor.document(), &result);
        prop_assert!(!editor.redo());
    }

    #[test]
    fn edits_keep_the_base_image_and_crops_inside_it(commands in prop::collection::vec(command(), 0..25)) {
        let mut editor = editor();
        let base = editor.document().base().clone();
        for command in &commands {
            let _ = editor.apply(command);
            prop_assert_eq!(editor.document().base(), &base);
            if let Some(crop) = editor.document().crop() {
                prop_assert_eq!(crop.intersection(&base.bounds()), Some(crop));
                prop_assert!(crop.width >= 1.0 && crop.height >= 1.0);
            }
        }
    }

    #[test]
    fn every_edited_document_survives_encoding(commands in prop::collection::vec(command(), 0..25)) {
        let mut editor = editor();
        for command in &commands {
            let _ = editor.apply(command);
        }
        let document = editor.document();
        let decoded = Document::decode(&document.encode().unwrap(), document.base().clone()).unwrap();
        prop_assert_eq!(&decoded, document);
    }

    #[test]
    fn decoding_arbitrary_bytes_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = Document::decode(&bytes, common::base_image(4, 4, 1.0));
    }
}
