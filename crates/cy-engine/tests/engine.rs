//! The engine end to end: snapshots in, lists, search, payloads, pins, expiry, settings and
//! persistence out. Real SQLite in a temporary directory, in-memory keystore.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use cy_engine::{DropReason, Engine, EngineError, IngestOutcome, MatchMode, Settings};
use cy_model::{ClipEntry, ClipboardSnapshot, ItemId, RepKind, Representation, SourceApp};
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

    fn open(&self, now_ms: u64) -> Engine {
        Engine::open(self.dir.path(), &self.keys, now_ms).unwrap()
    }
}

fn snapshot_from(app: &str, at: u64, entries: Vec<ClipEntry>) -> ClipboardSnapshot {
    ClipboardSnapshot {
        entries,
        front_app: Some(SourceApp {
            bundle_id: app.into(),
            name: "Demo Editor".into(),
        }),
        captured_at_ms: at,
    }
}

fn copy(text: &str, at: u64) -> ClipboardSnapshot {
    snapshot_from(
        "com.example.editor",
        at,
        vec![ClipEntry::with_data(
            "public.utf8-plain-text",
            text.as_bytes().to_vec(),
        )],
    )
}

fn stored(engine: &mut Engine, text: &str, at: u64) -> ItemId {
    match engine.ingest(&copy(text, at)).unwrap().outcome {
        IngestOutcome::Stored { id, .. } => id,
        other => panic!("expected Stored, got {other:?}"),
    }
}

fn previews(engine: &Engine) -> Vec<String> {
    engine
        .list(None, 0, 100)
        .into_iter()
        .map(|s| s.preview)
        .collect()
}

#[test]
fn a_copy_is_stored_and_listed_with_its_source() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);

    let id = stored(&mut engine, "Hello from Supercy", T0);
    let listed = engine.list(None, 0, 10);

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].preview, "Hello from Supercy");
    assert_eq!(listed[0].kind, RepKind::Text);
    assert_eq!(
        listed[0].source.as_ref().unwrap().bundle_id,
        "com.example.editor"
    );
    assert_eq!(listed[0].first_copied_ms, T0);
}

#[test]
fn copying_the_same_content_again_moves_it_to_the_top_without_a_new_item() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let first = stored(&mut engine, "demo one", T0);
    stored(&mut engine, "demo two", T0 + 1);

    let again = engine.ingest(&copy("demo one\r\n", T0 + 2)).unwrap();

    assert_eq!(again.outcome, IngestOutcome::Updated { id: first });
    assert_eq!(previews(&engine), vec!["demo one", "demo two"]);
    assert_eq!(engine.len(), 2);
}

#[test]
fn concealed_content_and_ignored_apps_leave_no_trace() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let concealed = snapshot_from(
        "com.example.editor",
        T0,
        vec![
            ClipEntry::with_data("public.utf8-plain-text", b"demo-password-123".to_vec()),
            ClipEntry::marker("org.nspasteboard.ConcealedType"),
        ],
    );
    let from_password_manager = snapshot_from(
        "org.keepassxc.keepassxc",
        T0,
        vec![ClipEntry::with_data(
            "public.utf8-plain-text",
            b"demo-password-456".to_vec(),
        )],
    );

    let a = engine.ingest(&concealed).unwrap();
    let b = engine.ingest(&from_password_manager).unwrap();

    assert_eq!(
        a.outcome,
        IngestOutcome::Dropped {
            reason: DropReason::Concealed
        }
    );
    assert_eq!(
        b.outcome,
        IngestOutcome::Dropped {
            reason: DropReason::IgnoredApp
        }
    );
    assert!(engine.is_empty());
}

#[test]
fn sensitive_items_expire_when_the_shell_ticks_at_the_reported_time() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let pem = format!("-----BEGIN {}PRIVATE KEY-----\ndemo", "EC ");

    let report = engine.ingest(&copy(&pem, T0)).unwrap();
    let due = report.next_expiry_ms.unwrap();

    assert_eq!(due, T0 + 120_000);
    assert_eq!(engine.list(None, 0, 10)[0].sensitive_until_ms, Some(due));
    assert_eq!(engine.tick(due - 1).unwrap(), Some(due));
    assert_eq!(engine.len(), 1);
    assert_eq!(engine.tick(due).unwrap(), None);
    assert!(engine.is_empty());
}

#[test]
fn pinned_sensitive_items_do_not_expire() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let id = stored(
        &mut engine,
        &format!("-----BEGIN {}PRIVATE KEY-----", "EC "),
        T0,
    );

    engine.set_pinned(id, true, T0 + 1).unwrap();

    assert_eq!(engine.tick(T0 + 10_000_000).unwrap(), None);
    assert!(engine.list(None, 0, 10)[0].pinned);
}

#[test]
fn expired_items_are_removed_when_the_history_is_opened() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    stored(
        &mut engine,
        &format!("-----BEGIN {}PRIVATE KEY-----", "EC "),
        T0,
    );
    stored(&mut engine, "ordinary", T0);
    drop(engine);

    let engine = fixture.open(T0 + 1_000_000);

    assert_eq!(previews(&engine), vec!["ordinary"]);
}

#[test]
fn search_finds_english_and_pinyin() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    stored(&mut engine, "会议纪要：下周一发布", T0);
    let link = stored(&mut engine, "https://example.com/docs", T0 + 1);
    stored(&mut engine, "lorem ipsum", T0 + 2);

    let pinyin = engine.search("hyjy", MatchMode::Fuzzy, None, 10);
    let links = engine.search("example", MatchMode::Fuzzy, Some(RepKind::Url), 10);

    assert_eq!(pinyin[0].preview, "会议纪要：下周一发布");
    assert_eq!(links.iter().map(|s| s.id).collect::<Vec<_>>(), vec![link]);
    assert_eq!(engine.search("", MatchMode::Fuzzy, None, 10).len(), 3);
}

#[test]
fn payload_returns_every_form_for_pasting() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let rich = snapshot_from(
        "com.example.editor",
        T0,
        vec![
            ClipEntry::with_data("public.utf8-plain-text", b"Demo".to_vec()),
            ClipEntry::with_data("public.html", b"<b>Demo</b>".to_vec()),
        ],
    );
    let IngestOutcome::Stored { id, .. } = engine.ingest(&rich).unwrap().outcome else {
        panic!("not stored")
    };

    let payload = engine.payload(id).unwrap();

    assert!(
        matches!(&payload[0], Representation::RichText { html: Some(html), .. } if html.expose() == "<b>Demo</b>")
    );
    assert!(matches!(
        engine.payload(ItemId::from_bytes([9; 16])),
        Err(EngineError::NotFound)
    ));
}

#[test]
fn marking_an_item_used_moves_it_to_the_top() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let old = stored(&mut engine, "old", T0);
    stored(&mut engine, "new", T0 + 1);

    engine.mark_used(old, T0 + 2).unwrap();

    assert_eq!(previews(&engine), vec!["old", "new"]);
    assert!(matches!(
        engine.mark_used(ItemId::from_bytes([9; 16]), T0),
        Err(EngineError::NotFound)
    ));
}

#[test]
fn order_follows_ingest_even_when_the_wall_clock_goes_back() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    stored(&mut engine, "first", T0 + 5_000);
    stored(&mut engine, "second", T0);

    assert_eq!(previews(&engine), vec!["second", "first"]);
}

#[test]
fn pins_lead_the_list_and_survive_clearing() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let pinned = stored(&mut engine, "pinned", T0);
    stored(&mut engine, "other", T0 + 1);
    engine.set_pinned(pinned, true, T0 + 2).unwrap();

    assert_eq!(previews(&engine), vec!["pinned", "other"]);
    assert_eq!(engine.clear(true).unwrap(), 1);
    assert_eq!(previews(&engine), vec!["pinned"]);
    assert_eq!(engine.clear(false).unwrap(), 1);
    assert!(engine.is_empty());
    assert_eq!(engine.search("pinned", MatchMode::Fuzzy, None, 10), vec![]);
}

#[test]
fn delete_removes_from_list_and_search() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let id = stored(&mut engine, "demo", T0);

    assert!(engine.delete(id).unwrap());
    assert!(!engine.delete(id).unwrap());
    assert!(engine.search("demo", MatchMode::Fuzzy, None, 10).is_empty());
}

#[test]
fn limits_evict_the_oldest_and_report_it() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    engine
        .update_settings(Settings {
            max_items: 50,
            ..Settings::default()
        })
        .unwrap();
    let first = stored(&mut engine, "demo 0", T0);
    for n in 1..50 {
        stored(&mut engine, &format!("demo {n}"), T0 + n);
    }

    let report = engine.ingest(&copy("demo 50", T0 + 50)).unwrap();

    assert!(
        matches!(report.outcome, IngestOutcome::Stored { evicted, .. } if evicted == vec![first])
    );
    assert_eq!(engine.len(), 50);
    assert!(
        engine
            .search("demo 0", MatchMode::Exact, None, 100)
            .iter()
            .all(|s| s.id != first)
    );
}

#[test]
fn lowering_the_item_limit_applies_at_once() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    for n in 0..60 {
        stored(&mut engine, &format!("demo {n}"), T0 + n);
    }

    engine
        .update_settings(Settings {
            max_items: 50,
            ..Settings::default()
        })
        .unwrap();

    assert_eq!(engine.len(), 50);
    assert_eq!(engine.list(None, 0, 1)[0].preview, "demo 59");
}

#[test]
fn invalid_settings_are_refused_and_nothing_changes() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);

    let result = engine.update_settings(Settings {
        max_items: 1,
        ..Settings::default()
    });

    assert!(matches!(result, Err(EngineError::Setting(e)) if e.field == "max_items"));
    assert_eq!(engine.settings(), &Settings::default());
}

#[test]
fn history_settings_and_device_survive_reopening() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let custom = Settings {
        max_items: 200,
        ignored_apps: vec!["com.example.passwords".into()],
        ..Settings::default()
    };
    engine.update_settings(custom.clone()).unwrap();
    stored(&mut engine, "one", T0);
    stored(&mut engine, "two", T0 + 1);
    let device = engine.device();
    drop(engine);

    let mut engine = fixture.open(T0 + 2);

    assert_eq!(engine.settings(), &custom);
    assert_eq!(engine.device(), device);
    assert_eq!(previews(&engine), vec!["two", "one"]);
    assert_eq!(engine.unreadable(), 0);
    stored(&mut engine, "three", T0);
    assert_eq!(previews(&engine)[0], "three");
}

#[test]
fn images_are_summarized_without_their_bytes() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&[0, 0, 0, 40, 0, 0, 0, 30]);
    let snapshot = snapshot_from(
        "com.example.editor",
        T0,
        vec![ClipEntry::with_data("public.png", png)],
    );

    engine.ingest(&snapshot).unwrap();
    let summary = &engine.list(Some(RepKind::Image), 0, 10)[0];

    assert_eq!(summary.image_size, Some((40, 30)));
    assert_eq!(summary.preview, "");
    assert!(engine.heap_bytes() < 64 * 1024);
}

#[test]
fn a_history_without_its_key_does_not_open() {
    let fixture = Fixture::new();
    drop(fixture.open(T0));

    let result = Engine::open(fixture.dir.path(), &MemoryKeyStore::new(), T0);

    assert!(matches!(
        result,
        Err(EngineError::Store(cy_store::StoreError::KeyMissing))
    ));
}
