//! Engine latency and memory budgets (NFR-1, NFR-4, M2-04), measured in release mode with
//! `scripts/docker-test.sh bench`. Times are asserted in release builds only.

#![allow(
    clippy::unwrap_used,
    clippy::print_stderr,
    reason = "the benchmark reports its measurements"
)]

use std::time::{Duration, Instant};

use cy_engine::{Engine, MatchMode, Settings};
use cy_model::{ClipEntry, ClipboardSnapshot, SourceApp};
use cy_platform::MemoryKeyStore;

const T0: u64 = 1_700_000_000_000;
/// Average time to store one copy.
const INGEST_BUDGET: Duration = Duration::from_millis(5);
/// Slowest search keystroke with 10 000 items.
const SEARCH_BUDGET_10K: Duration = Duration::from_millis(50);
/// One tick with nothing due (median of five).
const TICK_BUDGET: Duration = Duration::from_millis(2);
/// In-memory state of the engine with the default history size.
const HEAP_BUDGET_500: usize = 10 * 1024 * 1024;
/// In-memory state with the largest history size.
const HEAP_BUDGET_10K: usize = 32 * 1024 * 1024;

const WORDS: &[&str] = &[
    "demo",
    "supercy",
    "clipboard",
    "会议",
    "纪要",
    "报销",
    "example",
    "link",
    "银行",
    "通知",
    "release",
    "lorem",
    "ipsum",
    "项目",
    "进度",
    "commit",
    "readme",
    "设计",
    "文档",
    "测试",
];

fn text(seed: u64) -> String {
    let mut state = seed.wrapping_add(1);
    let mut out = format!("{seed} ");
    while out.chars().count() < 200 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        out.push_str(WORDS[usize::try_from(state >> 33).unwrap() % WORDS.len()]);
        out.push(' ');
    }
    out
}

fn copy(n: u64) -> ClipboardSnapshot {
    ClipboardSnapshot {
        entries: vec![ClipEntry::with_data(
            "public.utf8-plain-text",
            text(n).into_bytes(),
        )],
        front_app: Some(SourceApp {
            bundle_id: "com.example.editor".into(),
            name: "Demo Editor".into(),
        }),
        captured_at_ms: T0 + n,
    }
}

/// Resident memory of this process in bytes (Linux), for a real-world cross-check.
fn resident_bytes() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kib: usize = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

fn measure(count: u64, heap_budget: usize) {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let mut engine = Engine::open(dir.path(), &keys, T0).unwrap();
    let max_items = u32::try_from(count).unwrap().max(50);
    engine
        .update_settings(Settings {
            max_items,
            ..Settings::default()
        })
        .unwrap();

    let start = Instant::now();
    for n in 0..count {
        engine.ingest(&copy(n)).unwrap();
    }
    let per_ingest = start.elapsed() / u32::try_from(count).unwrap();
    drop(engine);

    let before_open = resident_bytes();
    let start = Instant::now();
    let mut engine = Engine::open(dir.path(), &keys, T0 + count).unwrap();
    let opened = start.elapsed();
    let rss_delta = before_open
        .zip(resident_bytes())
        .map(|(a, b)| b.saturating_sub(a));

    let slowest_search = ["h", "hy", "hyj", "hyjy", "supercy", "release notes", "zzzz"]
        .iter()
        .map(|q| {
            let start = Instant::now();
            std::hint::black_box(engine.search(q, MatchMode::Fuzzy, None, 50));
            start.elapsed()
        })
        .max()
        .unwrap();
    let mut ticks: Vec<Duration> = (0..5)
        .map(|n| {
            let start = Instant::now();
            engine.tick(T0 + count + 1 + n).unwrap();
            start.elapsed()
        })
        .collect();
    ticks.sort();
    let tick = ticks[ticks.len() / 2];
    let heap = engine.heap_bytes();

    eprintln!(
        "perf: engine {count} items: ingest {per_ingest:?} each, open {opened:?}, slowest search {slowest_search:?}, \
         tick {tick:?}, engine heap {heap} bytes, resident growth on open {rss_delta:?} bytes"
    );
    assert!(heap <= heap_budget, "engine heap {heap} > {heap_budget}");
    if !cfg!(debug_assertions) {
        assert!(per_ingest <= INGEST_BUDGET, "ingest {per_ingest:?}");
        assert!(tick <= TICK_BUDGET, "tick {tick:?}");
        if count >= 10_000 {
            assert!(
                slowest_search <= SEARCH_BUDGET_10K,
                "search {slowest_search:?}"
            );
        }
    }
}

#[test]
#[ignore = "benchmark; run with scripts/docker-test.sh bench"]
fn perf_engine_500_items() {
    measure(500, HEAP_BUDGET_500);
}

#[test]
#[ignore = "benchmark; run with scripts/docker-test.sh bench"]
fn perf_engine_10k_items() {
    measure(10_000, HEAP_BUDGET_10K);
}
