//! The FFI surface from Rust, the way Swift will use it: a keystore implemented by the "shell",
//! records in, summaries and payloads out, errors mapped to their Swift cases.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cy_ffi::{
    ClipEntry, ClipboardSnapshot, DropReason, IngestOutcome, ItemKind, Keystore, KeystoreError,
    PasteRepresentation, SearchMode, Settings, SourceApp, SupercyEngine, SupercyError,
    core_version,
};

const T0: u64 = 1_700_000_000_000;

/// The shell's keychain, in memory.
#[derive(Default)]
struct DemoKeychain {
    entries: Mutex<HashMap<String, Vec<u8>>>,
    refuse: bool,
}

impl Keystore for DemoKeychain {
    fn read(&self, account: String) -> Result<Option<Vec<u8>>, KeystoreError> {
        if self.refuse {
            return Err(KeystoreError::Denied);
        }
        Ok(self.entries.lock().unwrap().get(&account).cloned())
    }

    fn write(&self, account: String, secret: Vec<u8>) -> Result<(), KeystoreError> {
        self.entries.lock().unwrap().insert(account, secret);
        Ok(())
    }

    fn delete(&self, account: String) -> Result<(), KeystoreError> {
        self.entries.lock().unwrap().remove(&account);
        Ok(())
    }
}

fn open(
    dir: &tempfile::TempDir,
    keychain: &Arc<DemoKeychain>,
) -> Result<Arc<SupercyEngine>, SupercyError> {
    SupercyEngine::open(
        dir.path().to_string_lossy().into_owned(),
        keychain.clone(),
        T0,
    )
}

fn copy(text: &str, at: u64) -> ClipboardSnapshot {
    ClipboardSnapshot {
        entries: vec![ClipEntry {
            type_id: "public.utf8-plain-text".into(),
            data: Some(text.as_bytes().to_vec()),
            size: 0,
        }],
        front_app: Some(SourceApp {
            bundle_id: "com.example.editor".into(),
            name: "Demo Editor".into(),
        }),
        captured_at_ms: at,
    }
}

fn stored_id(engine: &SupercyEngine, text: &str, at: u64) -> String {
    match engine.ingest(copy(text, at)).unwrap().outcome {
        IngestOutcome::Stored { id, .. } => id,
        other => panic!("expected Stored, got {other:?}"),
    }
}

#[test]
fn a_full_round_trip_through_the_interface() {
    let dir = tempfile::tempdir().unwrap();
    let keychain = Arc::new(DemoKeychain::default());
    let engine = open(&dir, &keychain).unwrap();

    let id = stored_id(&engine, "Hello from Supercy", T0);
    let listed = engine.list(None, 0, 10).unwrap();
    let found = engine
        .search("hfs".into(), SearchMode::Fuzzy, Some(ItemKind::Text), 10)
        .unwrap();
    let payload = engine.payload(id.clone()).unwrap();

    assert_eq!(id.len(), 32);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].source_name.as_deref(), Some("Demo Editor"));
    assert_eq!(listed[0].last_used_ms, T0);
    assert_eq!(found.len(), 1);
    assert_eq!(
        payload,
        vec![PasteRepresentation::PlainText {
            text: "Hello from Supercy".into()
        }]
    );
    assert_eq!(engine.item_count().unwrap(), 1);
    assert!(keychain.entries.lock().unwrap().contains_key("data-key"));
}

#[test]
fn pins_use_delete_clear_and_tick() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(&dir, &Arc::new(DemoKeychain::default())).unwrap();
    let a = stored_id(&engine, "first", T0);
    let b = stored_id(&engine, "second", T0 + 1);

    engine.set_pinned(a.clone(), true, T0 + 2).unwrap();
    engine.mark_used(b.clone(), T0 + 3).unwrap();
    let order: Vec<String> = engine
        .list(None, 0, 10)
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();

    assert_eq!(order, vec![a.clone(), b.clone()]);
    assert!(engine.delete(b.clone()).unwrap());
    assert_eq!(engine.clear(true).unwrap(), 0);
    assert_eq!(engine.tick(T0 + 4).unwrap(), None);
    assert_eq!(engine.clear(false).unwrap(), 1);
}

#[test]
fn drops_and_sensitive_items_report_reasons_and_tick_times() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(&dir, &Arc::new(DemoKeychain::default())).unwrap();
    let mut concealed = copy("demo-password-123", T0);
    concealed.entries.push(ClipEntry {
        type_id: "org.nspasteboard.ConcealedType".into(),
        data: None,
        size: 0,
    });

    let dropped = engine.ingest(concealed).unwrap();
    let sensitive = engine
        .ingest(copy(&format!("-----BEGIN {}PRIVATE KEY-----", "EC "), T0))
        .unwrap();

    assert_eq!(
        dropped.outcome,
        IngestOutcome::Dropped {
            reason: DropReason::Concealed
        }
    );
    assert_eq!(sensitive.next_tick_ms, Some(T0 + 120_000));
    assert_eq!(
        engine.list(None, 0, 1).unwrap()[0].sensitive_until_ms,
        Some(T0 + 120_000)
    );
}

#[test]
fn settings_round_trip_and_invalid_values_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(&dir, &Arc::new(DemoKeychain::default())).unwrap();
    let mut settings = engine.settings().unwrap();
    settings.max_items = 200;

    let updated = engine.update_settings(settings.clone()).unwrap();
    let invalid = engine.update_settings(Settings {
        max_items: 1,
        ..settings.clone()
    });

    assert_eq!(updated, settings);
    assert!(
        matches!(invalid, Err(SupercyError::InvalidSetting { message }) if message.contains("max_items"))
    );
    assert_eq!(engine.settings().unwrap().max_items, 200);
}

#[test]
fn errors_map_to_their_swift_cases() {
    let dir = tempfile::tempdir().unwrap();
    let keychain = Arc::new(DemoKeychain::default());
    let engine = open(&dir, &keychain).unwrap();

    assert!(matches!(
        engine.payload("not-an-id".into()),
        Err(SupercyError::InvalidId { .. })
    ));
    assert!(matches!(
        engine.payload("00".repeat(16)),
        Err(SupercyError::NotFound { .. })
    ));
    assert!(matches!(
        open(&dir, &keychain),
        Err(SupercyError::Locked { .. })
    ));
    drop(engine);
    assert!(matches!(
        open(&dir, &Arc::new(DemoKeychain::default())),
        Err(SupercyError::KeyMissing { .. })
    ));
    let refusing = Arc::new(DemoKeychain {
        refuse: true,
        ..DemoKeychain::default()
    });
    assert!(matches!(
        open(&dir, &refusing),
        Err(SupercyError::Keystore { .. })
    ));
}

#[test]
fn calls_from_several_threads_are_serialized() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(&dir, &Arc::new(DemoKeychain::default())).unwrap();

    let threads: Vec<_> = (0..4u64)
        .map(|t| {
            let engine = engine.clone();
            std::thread::spawn(move || {
                for n in 0..25 {
                    engine
                        .ingest(copy(&format!("thread {t} item {n}"), T0 + t * 100 + n))
                        .unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }

    assert_eq!(engine.item_count().unwrap(), 100);
}

#[test]
fn rich_text_images_and_links_cross_the_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(&dir, &Arc::new(DemoKeychain::default())).unwrap();
    let entry = |type_id: &str, data: &[u8]| ClipEntry {
        type_id: type_id.into(),
        data: Some(data.to_vec()),
        size: 0,
    };
    let snapshot = ClipboardSnapshot {
        entries: vec![
            entry("public.utf8-plain-text", b"https://example.com/a"),
            entry("public.html", b"<a href='https://example.com/a'>a</a>"),
            entry(
                "public.png",
                b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x03",
            ),
        ],
        front_app: None,
        captured_at_ms: T0,
    };

    let IngestOutcome::Stored { id, .. } = engine.ingest(snapshot).unwrap().outcome else {
        panic!("not stored")
    };
    let summary = &engine.list(None, 0, 1).unwrap()[0];
    let payload = engine.payload(id).unwrap();

    assert_eq!(summary.kind, ItemKind::Url);
    assert_eq!(
        (summary.image_width, summary.image_height),
        (Some(2), Some(3))
    );
    assert!(
        matches!(&payload[0], PasteRepresentation::Url { url, .. } if url == "https://example.com/a")
    );
    assert!(matches!(
        &payload[1],
        PasteRepresentation::RichText { html: Some(_), .. }
    ));
    assert!(matches!(
        &payload[2],
        PasteRepresentation::Image {
            width: 2,
            height: 3,
            ..
        }
    ));
    assert!(!core_version().is_empty());
}
