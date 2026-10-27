//! Exporting pinned items to a file and importing them into another history. Real SQLite in
//! temporary directories, in-memory keystores.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use std::path::Path;

use cy_engine::{Engine, EngineError, IngestOutcome, PinsImport};
use cy_model::{ClipEntry, ClipboardSnapshot, ItemId, SourceApp};
use cy_platform::MemoryKeyStore;
use tempfile::TempDir;

const T0: u64 = 1_700_000_000_000;

struct Fixture {
    dir: TempDir,
    keys: MemoryKeyStore,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
            keys: MemoryKeyStore::new(),
        }
    }

    fn open(&self) -> Engine {
        Engine::open(self.dir.path(), &self.keys, T0).unwrap()
    }
}

fn copy(engine: &mut Engine, uti: &str, data: &str, at: u64) -> ItemId {
    let snapshot = ClipboardSnapshot {
        entries: vec![ClipEntry::with_data(uti, data.as_bytes().to_vec())],
        front_app: Some(SourceApp {
            bundle_id: "com.example.editor".into(),
            name: "Demo Editor".into(),
        }),
        captured_at_ms: at,
    };
    match engine.ingest(&snapshot).unwrap().outcome {
        IngestOutcome::Stored { id, .. } | IngestOutcome::Updated { id } => id,
        other => panic!("expected the copy to be kept, got {other:?}"),
    }
}

fn text(engine: &mut Engine, value: &str, at: u64) -> ItemId {
    copy(engine, "public.utf8-plain-text", value, at)
}

/// Previews and letter keys of the pinned items, in list order.
fn pins(engine: &Engine) -> Vec<(String, Option<char>)> {
    engine
        .list(None, 0, 100)
        .into_iter()
        .filter(|s| s.pinned)
        .map(|s| (s.preview, s.pin_key))
        .collect()
}

fn contains(path: &Path, needle: &str) -> bool {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).contains(needle)
}

#[test]
fn pinned_items_move_to_another_history_in_order_with_their_keys() {
    let source = Fixture::new();
    let mut engine = source.open();
    let reply = text(&mut engine, "Thanks, I will get back to you today.", T0);
    let link = copy(
        &mut engine,
        "public.url",
        "https://example.com/demo",
        T0 + 1,
    );
    let address = text(&mut engine, "示例地址 1 号", T0 + 2);
    text(&mut engine, "not pinned, never exported", T0 + 3);
    for (n, id) in [address, reply, link].into_iter().enumerate() {
        engine.set_pinned(id, true, T0 + 10 + n as u64).unwrap();
    }
    engine.set_pin_key(link, Some('q')).unwrap();
    let file = source.dir.path().join("pins.json");

    assert_eq!(engine.export_pins(&file).unwrap(), 3);
    assert!(!contains(&file, "never exported"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    let target = Fixture::new();
    let mut other = target.open();
    text(&mut other, "already here", T0);
    let report = other.import_pins(&file, T0 + 100).unwrap();

    assert_eq!(
        report,
        PinsImport {
            added: 3,
            pinned_existing: 0,
            already_pinned: 0,
            skipped: 0,
        }
    );
    assert_eq!(pins(&other), pins(&engine));
    assert_eq!(
        pins(&other),
        vec![
            ("示例地址 1 号".to_owned(), Some('a')),
            (
                "Thanks, I will get back to you today.".to_owned(),
                Some('b')
            ),
            ("https://example.com/demo".to_owned(), Some('q')),
        ]
    );
}

#[test]
fn importing_again_adds_nothing_and_pins_matching_items() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let pinned = text(&mut engine, "demo snippet", T0);
    engine.set_pinned(pinned, true, T0 + 1).unwrap();
    let file = fixture.dir.path().join("pins.json");
    engine.export_pins(&file).unwrap();

    let again = engine.import_pins(&file, T0 + 2).unwrap();
    assert_eq!((again.added, again.already_pinned), (0, 1));

    engine.set_pinned(pinned, false, T0 + 3).unwrap();
    let repinned = engine.import_pins(&file, T0 + 4).unwrap();
    assert_eq!((repinned.added, repinned.pinned_existing), (0, 1));
    assert_eq!(engine.len(), 1);
}

#[test]
fn a_taken_letter_key_is_replaced_by_a_free_one() {
    let source = Fixture::new();
    let mut engine = source.open();
    let id = text(&mut engine, "imported snippet", T0);
    engine.set_pinned(id, true, T0 + 1).unwrap(); // key 'a'
    let file = source.dir.path().join("pins.json");
    engine.export_pins(&file).unwrap();

    let target = Fixture::new();
    let mut other = target.open();
    let local = text(&mut other, "local snippet", T0);
    other.set_pinned(local, true, T0 + 1).unwrap(); // also key 'a'
    other.import_pins(&file, T0 + 2).unwrap();

    assert_eq!(
        pins(&other),
        vec![
            ("local snippet".to_owned(), Some('a')),
            ("imported snippet".to_owned(), Some('b')),
        ]
    );
}

#[test]
fn files_that_are_not_supercy_pins_change_nothing() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    text(&mut engine, "demo", T0);
    let write = |name: &str, body: &str| {
        let path = fixture.dir.path().join(name);
        std::fs::write(&path, body).unwrap();
        path
    };
    let cases = [
        write("garbage.json", "not json at all"),
        write(
            "other.json",
            r#"{"format":"something-else","version":1,"items":[]}"#,
        ),
        write(
            "newer.json",
            r#"{"format":"supercy-pins","version":99,"items":[]}"#,
        ),
        write(
            "bad-key.json",
            r#"{"format":"supercy-pins","version":1,"items":[{"key":"7","content":[{"type":"text","text":"x"}]}]}"#,
        ),
        write(
            "bad-image.json",
            r#"{"format":"supercy-pins","version":1,"items":[{"content":[{"type":"image","format":"png","width":1,"height":1,"data":"%%%"}]}]}"#,
        ),
    ];
    for path in &cases {
        assert!(
            matches!(
                engine.import_pins(path, T0 + 1),
                Err(EngineError::InvalidPinsFile)
            ),
            "{} was accepted",
            path.display()
        );
    }
    assert!(matches!(
        engine.import_pins(&fixture.dir.path().join("missing.json"), T0 + 1),
        Err(EngineError::File(_))
    ));
    assert_eq!(engine.len(), 1);
    assert!(pins(&engine).is_empty());
}

#[test]
fn items_without_content_are_skipped() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let path = fixture.dir.path().join("pins.json");
    std::fs::write(
        &path,
        r#"{"format":"supercy-pins","version":1,"items":[
            {"content":[]},
            {"content":[{"type":"text","text":"kept"}]}
        ]}"#,
    )
    .unwrap();

    let report = engine.import_pins(&path, T0).unwrap();

    assert_eq!((report.added, report.skipped), (1, 1));
    assert_eq!(pins(&engine), vec![("kept".to_owned(), Some('a'))]);
}
