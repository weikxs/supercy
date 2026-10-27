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
    let due = report.next_tick_ms.unwrap();

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

#[test]
fn a_paused_engine_reads_nothing_until_resumed_or_the_pause_ends() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);

    engine.pause(None);
    let while_paused = engine.ingest(&copy("secret while paused", T0)).unwrap();
    engine.resume();
    engine.pause(Some(T0 + 1_000));
    let report = engine.ingest(&copy("still paused", T0 + 10)).unwrap();
    let after = engine.ingest(&copy("after the pause", T0 + 1_000)).unwrap();

    assert_eq!(while_paused.outcome, IngestOutcome::Paused);
    assert_eq!(report.next_tick_ms, Some(T0 + 1_000));
    assert!(matches!(after.outcome, IngestOutcome::Stored { .. }));
    assert_eq!(previews(&engine), vec!["after the pause"]);
    assert!(!engine.pause_state(T0 + 1_000).is_paused(T0 + 1_000));
}

#[test]
fn a_timed_pause_ends_on_tick() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    engine.pause(Some(T0 + 500));

    assert_eq!(engine.tick(T0).unwrap(), Some(T0 + 500));
    assert_eq!(engine.tick(T0 + 500).unwrap(), None);
    assert_eq!(engine.pause_state(T0 + 500).paused_until_ms, None);
}

#[test]
fn skip_next_ignores_exactly_the_next_copies() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);

    engine.skip_next();
    let skipped = engine.ingest(&copy("skipped", T0)).unwrap();
    let kept = engine.ingest(&copy("kept", T0 + 1)).unwrap();

    assert_eq!(skipped.outcome, IngestOutcome::Skipped);
    assert!(matches!(kept.outcome, IngestOutcome::Stored { .. }));
    assert_eq!(engine.pause_state(T0).skip_next, 0);
}

#[test]
fn pinning_assigns_letters_which_can_be_changed_and_are_freed_on_unpin() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let a = stored(&mut engine, "a", T0);
    let b = stored(&mut engine, "b", T0 + 1);

    engine.set_pinned(a, true, T0 + 2).unwrap();
    engine.set_pinned(b, true, T0 + 3).unwrap();
    let taken = engine.set_pin_key(b, Some('a'));
    engine.set_pin_key(b, Some('q')).unwrap();
    engine.set_pinned(a, false, T0 + 4).unwrap();
    let c = stored(&mut engine, "c", T0 + 5);
    engine.set_pinned(c, true, T0 + 6).unwrap();

    assert!(matches!(taken, Err(EngineError::PinKeyTaken)));
    assert_eq!(engine.item_for_pin_key('q'), Some(b));
    assert_eq!(engine.item_for_pin_key('a'), Some(c));
    assert!(matches!(
        engine.set_pin_key(a, Some('z')),
        Err(EngineError::NotPinned)
    ));
    assert!(matches!(
        engine.set_pin_key(b, Some('Z')),
        Err(EngineError::InvalidPinKey)
    ));
    let summary = engine
        .list(None, 0, 10)
        .into_iter()
        .find(|s| s.id == b)
        .unwrap();
    assert_eq!(summary.pin_key, Some('q'));
}

#[test]
fn copy_counts_drive_the_copy_count_sort_and_pins_can_go_to_the_bottom() {
    use cy_engine::{PinPositionSetting, SortSetting};
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let pinned = stored(&mut engine, "pinned", T0);
    stored(&mut engine, "often", T0 + 1);
    stored(&mut engine, "once", T0 + 2);
    engine.ingest(&copy("often", T0 + 3)).unwrap();
    engine.ingest(&copy("often", T0 + 4)).unwrap();
    engine.set_pinned(pinned, true, T0 + 5).unwrap();
    stored(&mut engine, "newest", T0 + 6);

    engine
        .update_settings(Settings {
            sort: SortSetting::CopyCount,
            pin_position: PinPositionSetting::Bottom,
            ..Settings::default()
        })
        .unwrap();

    assert_eq!(previews(&engine), vec!["often", "newest", "once", "pinned"]);
    assert_eq!(engine.list(None, 0, 1)[0].copy_count, 3);
}

#[test]
fn regex_and_mixed_search_with_highlights() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    stored(&mut engine, "order 123456 shipped", T0);
    stored(&mut engine, "Hello from Supercy", T0 + 1);

    let regex = engine.search(r"\d{6}", MatchMode::Regex, None, 10);
    let mixed = engine.search("from", MatchMode::Mixed, None, 10);
    let fuzzy = engine.search("hfs", MatchMode::Fuzzy, None, 10);

    assert_eq!(regex.len(), 1);
    assert_eq!(regex[0].matches, vec![(6, 12)]);
    assert_eq!(mixed[0].matches, vec![(6, 10)]);
    assert_eq!(fuzzy[0].matches, vec![(0, 1), (6, 7), (11, 12)]);
    assert!(Engine::query_error("(", MatchMode::Regex).is_some());
    assert!(Engine::query_error("(", MatchMode::Fuzzy).is_none());
}

#[test]
fn ignore_rules_from_the_settings_apply_to_new_copies() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    engine
        .update_settings(Settings {
            ignore_patterns: vec!["^otp".into()],
            ignored_types: vec!["com.example.private-type".into()],
            ..Settings::default()
        })
        .unwrap();
    let typed = snapshot_from(
        "com.example.editor",
        T0,
        vec![
            ClipEntry::with_data("public.utf8-plain-text", b"demo".to_vec()),
            ClipEntry::marker("com.example.private-type"),
        ],
    );

    let pattern = engine.ingest(&copy("OTP 123456", T0)).unwrap();
    let ignored_type = engine.ingest(&typed).unwrap();

    assert_eq!(
        pattern.outcome,
        IngestOutcome::Dropped {
            reason: DropReason::IgnoredPattern
        }
    );
    assert_eq!(
        ignored_type.outcome,
        IngestOutcome::Dropped {
            reason: DropReason::IgnoredType
        }
    );
}

#[test]
fn allow_list_mode_keeps_only_listed_apps() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    engine
        .update_settings(Settings {
            allow_list_mode: true,
            allowed_apps: vec!["com.example.notes".into()],
            ..Settings::default()
        })
        .unwrap();

    let other = engine.ingest(&copy("from the editor", T0)).unwrap();
    let listed = engine
        .ingest(&snapshot_from(
            "com.example.notes",
            T0 + 1,
            vec![ClipEntry::with_data(
                "public.utf8-plain-text",
                b"from notes".to_vec(),
            )],
        ))
        .unwrap();

    assert_eq!(
        other.outcome,
        IngestOutcome::Dropped {
            reason: DropReason::NotAllowedApp
        }
    );
    assert!(matches!(listed.outcome, IngestOutcome::Stored { .. }));
}

#[test]
fn copied_files_are_listed_by_name_and_pasted_by_reference() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let snapshot = snapshot_from(
        "com.example.files",
        T0,
        vec![ClipEntry::with_data(
            "public.file-url",
            b"file:///tmp/supercy-test/Demo%20Report.pdf".to_vec(),
        )],
    );

    let IngestOutcome::Stored { id, .. } = engine.ingest(&snapshot).unwrap().outcome else {
        panic!("not stored")
    };
    let summary = &engine.list(None, 0, 1)[0];

    assert_eq!(summary.kind, RepKind::FileRef);
    assert_eq!(summary.preview, "Demo Report.pdf");
    assert!(
        matches!(&engine.payload(id).unwrap()[0], Representation::FileRef { names } if names[0].ends_with("Report.pdf"))
    );
}

#[test]
fn image_thumbnails_from_the_shell_are_kept_and_text_ones_ignored() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&[0, 0, 0, 40, 0, 0, 0, 30]);
    let image = snapshot_from(
        "com.example.editor",
        T0,
        vec![
            ClipEntry::with_data("public.png", png),
            ClipEntry::with_data(cy_engine::THUMBNAIL_TYPE, b"small demo thumbnail".to_vec()),
        ],
    );
    let text = snapshot_from(
        "com.example.editor",
        T0 + 1,
        vec![
            ClipEntry::with_data("public.utf8-plain-text", b"demo".to_vec()),
            ClipEntry::with_data(cy_engine::THUMBNAIL_TYPE, b"ignored".to_vec()),
        ],
    );

    let IngestOutcome::Stored { id: image_id, .. } = engine.ingest(&image).unwrap().outcome else {
        panic!("not stored")
    };
    let IngestOutcome::Stored { id: text_id, .. } = engine.ingest(&text).unwrap().outcome else {
        panic!("not stored")
    };

    assert_eq!(
        engine.thumbnail(image_id).unwrap().as_deref(),
        Some(&b"small demo thumbnail"[..])
    );
    assert_eq!(engine.thumbnail(text_id).unwrap(), None);
}

#[test]
fn pinned_items_keep_their_place_and_can_be_moved() {
    let fixture = Fixture::new();
    let mut engine = fixture.open(T0);
    let a = stored(&mut engine, "pin a", T0);
    let b = stored(&mut engine, "pin b", T0 + 1);
    let c = stored(&mut engine, "pin c", T0 + 2);
    let plain = stored(&mut engine, "not pinned", T0 + 3);
    for (n, id) in [a, b, c].into_iter().enumerate() {
        engine.set_pinned(id, true, T0 + 10 + n as u64).unwrap();
    }
    let ids = |engine: &Engine| -> Vec<ItemId> {
        engine.list(None, 0, 10).into_iter().map(|s| s.id).collect()
    };
    assert_eq!(ids(&engine), vec![a, b, c, plain]);

    // Using a pinned item does not move it.
    engine.mark_used(c, T0 + 20).unwrap();
    assert_eq!(ids(&engine), vec![a, b, c, plain]);

    engine.move_pin(c, 0, T0 + 21).unwrap();
    assert_eq!(ids(&engine), vec![c, a, b, plain]);
    engine.move_pin(c, 99, T0 + 22).unwrap();
    assert_eq!(ids(&engine), vec![a, b, c, plain]);
    engine.move_pin(a, 1, T0 + 23).unwrap();
    assert_eq!(ids(&engine), vec![b, a, c, plain]);
    assert!(matches!(
        engine.move_pin(plain, 0, T0 + 24),
        Err(EngineError::NotPinned)
    ));
    assert!(matches!(
        engine.move_pin(ItemId::from_bytes([0xee; 16]), 0, T0 + 24),
        Err(EngineError::NotFound)
    ));

    // A newly pinned item goes last; the order survives reopening.
    engine.set_pinned(plain, true, T0 + 25).unwrap();
    drop(engine);
    let engine = fixture.open(T0 + 30);
    assert_eq!(ids(&engine), vec![b, a, c, plain]);
}
