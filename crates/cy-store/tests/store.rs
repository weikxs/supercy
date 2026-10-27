//! The history store against a real SQLite file in a temporary directory, with an in-memory
//! keystore.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use std::path::Path;

use cy_model::{
    DeviceTag, Fingerprint, Hlc, ImageFormat, Item, ItemId, Representation, SecretBytes,
    SecretString, SensitiveReason, Sensitivity, SourceApp,
};
use cy_platform::{KeyStore, MemoryKeyStore};
use cy_store::{DATA_KEY_ACCOUNT, DATABASE_FILE, Limits, SCHEMA_VERSION, Store, StoreError};
use tempfile::TempDir;

const DEVICE: DeviceTag = DeviceTag([3; 8]);
const MARKER: &str = "Hello from Supercy, plaintext marker";

fn temp_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn item(store: &Store, n: u64, reps: Vec<Representation>) -> Item {
    let mut id = [0u8; 16];
    id[..8].copy_from_slice(&n.to_be_bytes());
    Item {
        id: ItemId::from_bytes(id),
        fingerprint: Fingerprint::of(store.fingerprint_key(), &reps).unwrap(),
        reps,
        source: Some(SourceApp {
            bundle_id: "com.example.editor".into(),
            name: "Demo Editor".into(),
        }),
        origin: DEVICE,
        first_copied_ms: 1_000 + n,
        last_used: Hlc::new(1_000 + n, 0, DEVICE),
        pinned: false,
        pinned_at: None,
        sensitivity: Sensitivity::Normal,
        truncated: false,
        copy_count: 1,
        pin_key: None,
    }
}

fn text(value: &str) -> Vec<Representation> {
    vec![Representation::PlainText(SecretString::from(value))]
}

fn image(size: usize, seed: u8) -> Vec<Representation> {
    vec![Representation::Image {
        format: ImageFormat::Png,
        width: 1,
        height: 1,
        bytes: SecretBytes::new(vec![seed; size]),
    }]
}

fn files_contain(dir: &Path, needle: &[u8]) -> bool {
    std::fs::read_dir(dir).unwrap().any(|entry| {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        bytes.windows(needle.len()).any(|window| window == needle)
    })
}

fn ids(items: &[Item]) -> Vec<ItemId> {
    items.iter().map(|i| i.id).collect()
}

#[test]
fn open_creates_a_key_and_reopening_reads_everything_back() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let rich = vec![
        Representation::Url {
            url: SecretString::from("https://example.com/a"),
            title: Some(SecretString::from("Example")),
        },
        Representation::RichText {
            rtf: Some(SecretBytes::from(&b"{\\rtf1 demo}"[..])),
            html: Some(SecretString::from("<b>demo</b>")),
            plain: SecretString::from("demo"),
        },
    ];
    let saved = vec![
        item(&store, 1, text(MARKER)),
        item(&store, 2, rich),
        item(&store, 3, image(64, 9)),
    ];
    for it in &saved {
        store.save(it, &Limits::default()).unwrap();
    }
    drop(store);

    let reopened = Store::open(dir.path(), &keys).unwrap();
    let report = reopened.load().unwrap();

    assert_eq!(keys.read(DATA_KEY_ACCOUNT).unwrap().unwrap().len(), 32);
    assert!(report.unreadable.is_empty());
    assert_eq!(report.items, saved.into_iter().rev().collect::<Vec<_>>());
}

#[test]
fn content_and_source_app_never_reach_the_disk_in_plain_form() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    store
        .save(&item(&store, 1, text(MARKER)), &Limits::default())
        .unwrap();

    assert!(!files_contain(dir.path(), MARKER.as_bytes()));
    drop(store);

    assert!(!files_contain(dir.path(), MARKER.as_bytes()));
    assert!(!files_contain(dir.path(), b"com.example.editor"));
}

#[test]
fn sensitivity_pins_and_truncation_round_trip() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let sensitive = Item {
        sensitivity: Sensitivity::Sensitive {
            expires_at_ms: 9_000,
            reason: SensitiveReason::AccessToken,
        },
        truncated: true,
        ..item(&store, 1, text("demo-password-123"))
    }
    .with_pinned(true, Hlc::new(5_000, 1, DEVICE));

    store.save(&sensitive, &Limits::default()).unwrap();

    assert_eq!(store.get(sensitive.id).unwrap(), Some(sensitive));
    assert_eq!(store.get(ItemId::from_bytes([0xee; 16])).unwrap(), None);
}

#[test]
fn a_tampered_body_is_reported_unreadable_and_kept() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let good = item(&store, 1, text("good"));
    let bad = item(&store, 2, text("bad"));
    store.save(&good, &Limits::default()).unwrap();
    store.save(&bad, &Limits::default()).unwrap();
    drop(store);

    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let mut body: Vec<u8> = raw
        .query_row(
            "SELECT body FROM items WHERE id = ?1",
            [bad.id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    body[30] ^= 1;
    raw.execute(
        "UPDATE items SET body = ?1 WHERE id = ?2",
        rusqlite::params![body, bad.id.as_bytes().as_slice()],
    )
    .unwrap();
    drop(raw);
    let store = Store::open(dir.path(), &keys).unwrap();
    let report = store.load().unwrap();

    assert_eq!(ids(&report.items), vec![good.id]);
    assert_eq!(report.unreadable, vec![bad.id]);
    assert!(matches!(store.get(bad.id), Err(StoreError::Unseal)));
}

#[test]
fn bodies_swapped_between_rows_are_unreadable() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let a = item(&store, 1, text("first"));
    let b = item(&store, 2, text("second"));
    store.save(&a, &Limits::default()).unwrap();
    store.save(&b, &Limits::default()).unwrap();
    drop(store);

    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    raw.execute_batch(
        "CREATE TEMP TABLE b AS SELECT id, body FROM items;
         UPDATE items SET body = (SELECT body FROM b WHERE b.id <> items.id);",
    )
    .unwrap();
    drop(raw);
    let report = Store::open(dir.path(), &keys).unwrap().load().unwrap();

    assert!(report.items.is_empty());
    assert_eq!(report.unreadable.len(), 2);
}

#[test]
fn a_missing_key_refuses_to_open_and_leaves_the_file_alone() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    store
        .save(&item(&store, 1, text("demo")), &Limits::default())
        .unwrap();
    drop(store);
    let before = std::fs::read(dir.path().join(DATABASE_FILE)).unwrap();

    let result = Store::open(dir.path(), &MemoryKeyStore::new());

    assert!(matches!(result, Err(StoreError::KeyMissing)));
    assert_eq!(
        std::fs::read(dir.path().join(DATABASE_FILE)).unwrap(),
        before
    );
}

#[test]
fn another_key_reads_nothing() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    store
        .save(&item(&store, 1, text("demo")), &Limits::default())
        .unwrap();
    drop(store);
    let other = MemoryKeyStore::new();
    other
        .write(DATA_KEY_ACCOUNT, &SecretBytes::new(vec![1; 32]))
        .unwrap();

    let report = Store::open(dir.path(), &other).unwrap().load().unwrap();

    assert!(report.items.is_empty());
    assert_eq!(report.unreadable.len(), 1);
}

#[test]
fn a_damaged_key_is_reported() {
    let keys = MemoryKeyStore::new();
    keys.write(DATA_KEY_ACCOUNT, &SecretBytes::new(vec![1; 5]))
        .unwrap();

    assert!(matches!(
        Store::open(temp_dir().path(), &keys),
        Err(StoreError::KeyInvalid)
    ));
}

#[test]
fn only_one_store_can_hold_the_history() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let _first = Store::open(dir.path(), &keys).unwrap();

    assert!(matches!(
        Store::open(dir.path(), &keys),
        Err(StoreError::Locked)
    ));
}

#[test]
fn a_newer_format_is_refused_without_writing() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    drop(Store::open(dir.path(), &keys).unwrap());
    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    raw.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(raw);
    let before = std::fs::read(dir.path().join(DATABASE_FILE)).unwrap();

    let result = Store::open(dir.path(), &keys);

    assert!(
        matches!(result, Err(StoreError::TooNew { found, supported }) if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION)
    );
    assert_eq!(
        std::fs::read(dir.path().join(DATABASE_FILE)).unwrap(),
        before
    );
}

#[test]
fn an_unfinished_transaction_is_gone_after_a_crash() {
    let dir = temp_dir();
    let crashed = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let committed = item(&store, 1, text("committed"));
    let pending = item(&store, 2, text("never committed"));
    store.save(&committed, &Limits::default()).unwrap();
    drop(store);

    // A second writer starts a transaction and the machine "loses power" before it commits:
    // the files are copied while the transaction is open.
    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    raw.execute_batch("PRAGMA cache_size = 1; BEGIN;").unwrap();
    for n in 0..200u64 {
        raw.execute(
            "INSERT INTO items (id, fingerprint, kind, first_copied, last_used, pinned, origin, size, body)
             VALUES (?1, ?1, 1, 0, ?2, 0, ?3, 0, zeroblob(4000))",
            rusqlite::params![
                [&n.to_be_bytes()[..], &[0xffu8; 8]].concat(),
                pending.last_used.to_bytes().as_slice(),
                DEVICE.0.as_slice()
            ],
        )
        .unwrap();
    }
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, crashed.path().join(path.file_name().unwrap())).unwrap();
    }
    drop(raw);

    let report = Store::open(crashed.path(), &keys).unwrap().load().unwrap();

    assert_eq!(ids(&report.items), vec![committed.id]);
}

#[test]
fn garbage_instead_of_a_database_is_an_error_not_a_crash() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    keys.write(DATA_KEY_ACCOUNT, &SecretBytes::new(vec![2; 32]))
        .unwrap();
    std::fs::write(dir.path().join(DATABASE_FILE), vec![0x5a; 8192]).unwrap();

    assert!(Store::open(dir.path(), &keys).is_err());
}

#[test]
fn limits_remove_the_oldest_unpinned_items_and_never_pinned_ones() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let limits = Limits {
        max_items: 50,
        ..Limits::default()
    };
    let pinned = item(&store, 0, text("pinned")).with_pinned(true, Hlc::new(1, 0, DEVICE));
    store.save(&pinned, &limits).unwrap();

    let mut removed = Vec::new();
    for n in 1..=55 {
        removed.extend(
            store
                .save(&item(&store, n, text(&format!("demo {n}"))), &limits)
                .unwrap(),
        );
    }
    let report = store.load().unwrap();

    assert_eq!(report.items.len(), 51);
    assert_eq!(report.items[0].id, pinned.id);
    assert_eq!(removed.len(), 5);
    assert!(removed.iter().all(|id| !ids(&report.items).contains(id)));
    assert_eq!(report.items[1].first_copied_ms, 1_055);
}

#[test]
fn image_total_limit_removes_the_oldest_images() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let limits = Limits {
        max_image_total_bytes: 250,
        ..Limits::default()
    };

    let mut removed = Vec::new();
    for n in 1..=3 {
        removed.extend(
            store
                .save(
                    &item(&store, n, image(100, u8::try_from(n).unwrap())),
                    &limits,
                )
                .unwrap(),
        );
    }
    store
        .save(&item(&store, 4, text("text is not an image")), &limits)
        .unwrap();

    assert_eq!(removed, vec![item(&store, 1, image(100, 1)).id]);
    assert_eq!(store.load().unwrap().items.len(), 3);
}

#[test]
fn saving_the_same_id_updates_in_place_and_fingerprints_are_unique() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let first = item(&store, 1, text("demo"));
    store.save(&first, &Limits::default()).unwrap();

    let touched = first.touched(Hlc::new(9_999, 0, DEVICE));
    store.save(&touched, &Limits::default()).unwrap();
    let duplicate = item(&store, 2, text("demo"));

    assert_eq!(store.load().unwrap().items, vec![touched]);
    assert_eq!(
        store.find_by_fingerprint(&first.fingerprint).unwrap(),
        Some(first.id)
    );
    assert!(store.save(&duplicate, &Limits::default()).is_err());
    assert_eq!(
        store.find_by_fingerprint(&Fingerprint([0; 32])).unwrap(),
        None
    );
}

#[test]
fn clear_removes_items_and_their_ciphertext_from_the_file() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let pinned = item(&store, 1, text("keep me")).with_pinned(true, Hlc::new(1, 0, DEVICE));
    let plain = item(&store, 2, image(4096, 0xa7));
    store.save(&pinned, &Limits::default()).unwrap();
    store.save(&plain, &Limits::default()).unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let sealed: Vec<u8> = raw
        .query_row(
            "SELECT body FROM items WHERE id = ?1",
            [plain.id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    drop(raw);
    let mut store = Store::open(dir.path(), &keys).unwrap();

    assert_eq!(store.clear(true).unwrap(), 1);
    assert_eq!(ids(&store.load().unwrap().items), vec![pinned.id]);
    drop(store);
    assert!(!files_contain(dir.path(), &sealed[24..88]));

    let mut store = Store::open(dir.path(), &keys).unwrap();
    assert_eq!(store.clear(false).unwrap(), 1);
    assert!(store.load().unwrap().items.is_empty());
}

#[test]
fn delete_removes_one_item() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let it = item(&store, 1, text("demo"));
    store.save(&it, &Limits::default()).unwrap();

    assert!(store.delete(it.id).unwrap());
    assert!(!store.delete(it.id).unwrap());
    assert!(store.load().unwrap().items.is_empty());
}

#[test]
fn expiry_removes_unpinned_sensitive_items_and_reports_the_next_deadline() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let sensitive = |n, at| Item {
        sensitivity: Sensitivity::Sensitive {
            expires_at_ms: at,
            reason: SensitiveReason::HighEntropy,
        },
        ..item(&store, n, text(&format!("demo secret {n}")))
    };
    let soon = sensitive(1, 5_000);
    let later = sensitive(2, 8_000);
    let pinned = sensitive(3, 1_000).with_pinned(true, Hlc::new(1, 0, DEVICE));
    for it in [&soon, &later, &pinned] {
        store.save(it, &Limits::default()).unwrap();
    }

    assert_eq!(store.next_expiry().unwrap(), Some(5_000));
    assert!(store.expire(4_999).unwrap().is_empty());
    assert_eq!(store.expire(5_000).unwrap(), vec![soon.id]);
    assert_eq!(store.next_expiry().unwrap(), Some(8_000));
    assert_eq!(store.expire(u64::from(u32::MAX)).unwrap(), vec![later.id]);
    assert_eq!(store.next_expiry().unwrap(), None);
    assert_eq!(ids(&store.load().unwrap().items), vec![pinned.id]);
}

#[cfg(unix)]
#[test]
fn the_data_directory_and_database_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir();
    let data = dir.path().join("Supercy");

    drop(Store::open(&data, &MemoryKeyStore::new()).unwrap());

    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&data), 0o700);
    assert_eq!(mode(&data.join(DATABASE_FILE)), 0o600);
}

#[test]
fn meta_records_are_sealed_and_survive_reopening() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();

    store.write_meta("settings", MARKER.as_bytes()).unwrap();
    store.write_meta("settings", b"second value").unwrap();
    drop(store);
    let store = Store::open(dir.path(), &keys).unwrap();

    assert_eq!(
        store.read_meta("settings").unwrap().unwrap().expose(),
        b"second value"
    );
    assert_eq!(store.read_meta("missing").unwrap(), None);
    drop(store);
    assert!(!files_contain(dir.path(), b"second value"));
}

#[test]
fn a_meta_record_moved_to_another_key_cannot_be_read() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    store.write_meta("a", b"demo").unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    raw.execute("UPDATE meta SET key = 'b'", []).unwrap();
    drop(raw);

    let store = Store::open(dir.path(), &keys).unwrap();

    assert!(matches!(store.read_meta("b"), Err(StoreError::Unseal)));
}

#[test]
fn a_version_1_history_is_migrated_and_keeps_its_items() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let kept = item(&store, 1, text("from version 1"));
    store.save(&kept, &Limits::default()).unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    raw.execute_batch(
        "DROP INDEX items_pin_key; ALTER TABLE items DROP COLUMN pin_key; ALTER TABLE items DROP COLUMN copy_count;
         DROP TABLE meta; DROP INDEX items_expiry;
         CREATE INDEX items_expiry ON items(expires_at) WHERE expires_at IS NOT NULL;
         PRAGMA user_version = 1;",
    )
    .unwrap();
    drop(raw);

    let mut store = Store::open(dir.path(), &keys).unwrap();
    store.write_meta("settings", b"demo").unwrap();

    assert_eq!(store.load().unwrap().items, vec![kept]);
    let raw_version = || {
        rusqlite::Connection::open(dir.path().join(DATABASE_FILE))
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap()
    };
    drop(store);
    assert_eq!(raw_version(), SCHEMA_VERSION);
}

#[test]
fn lowering_the_limits_applies_them_at_once() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    for n in 1..=60 {
        store
            .save(
                &item(&store, n, text(&format!("demo {n}"))),
                &Limits {
                    max_items: 100,
                    ..Limits::default()
                },
            )
            .unwrap();
    }

    let removed = store
        .apply_limits(&Limits {
            max_items: 50,
            ..Limits::default()
        })
        .unwrap();

    assert_eq!(removed.len(), 10);
    assert_eq!(store.load().unwrap().items.len(), 50);
}

#[test]
fn copy_counts_and_pin_keys_round_trip_and_keys_are_unique() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let at = Hlc::new(5, 0, DEVICE);
    let a = Item {
        copy_count: 7,
        ..item(&store, 1, text("a"))
    }
    .with_pinned(true, at)
    .with_pin_key(Some('a'));
    let b = item(&store, 2, text("b"))
        .with_pinned(true, at)
        .with_pin_key(Some('a'));

    store.save(&a, &Limits::default()).unwrap();

    assert_eq!(store.get(a.id).unwrap(), Some(a));
    assert!(store.save(&b, &Limits::default()).is_err());
}

#[test]
fn a_damaged_pin_key_makes_the_row_unreadable() {
    let dir = temp_dir();
    let keys = MemoryKeyStore::new();
    let mut store = Store::open(dir.path(), &keys).unwrap();
    let it = item(&store, 1, text("demo"));
    store.save(&it, &Limits::default()).unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    raw.execute("UPDATE items SET pin_key = 'AB'", []).unwrap();
    drop(raw);

    let report = Store::open(dir.path(), &keys).unwrap().load().unwrap();

    assert_eq!(report.unreadable, vec![it.id]);
}
