//! Search performance and memory budgets (NFR-4, M1-03). Run in release mode:
//! `scripts/docker-test.sh bench`. Times are only asserted in release builds.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]
#![allow(
    clippy::print_stderr,
    reason = "the benchmark reports its measurements"
)]

use std::borrow::Cow;
use std::time::{Duration, Instant};

use cy_model::{DeviceTag, Hlc, ItemId, RepKind};
use cy_search::{IndexDoc, ListOrder, MatchMode, Query, SearchIndex};

/// Per-keystroke budget with the default history size.
const BUDGET_500: Duration = Duration::from_millis(10);
/// Per-keystroke budget with the largest history size.
const BUDGET_10K: Duration = Duration::from_millis(50);
/// Memory budget of the index with the default history size.
const HEAP_BUDGET_500: usize = 1024 * 1024;
/// Memory budget of the index with the largest history size.
const HEAP_BUDGET_10K: usize = 16 * 1024 * 1024;

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

/// Deterministic text of about 200 characters mixing English and Chinese words.
fn text(seed: u64) -> String {
    let mut state = seed;
    let mut out = String::new();
    while out.chars().count() < 200 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        out.push_str(WORDS[usize::try_from(state >> 33).unwrap() % WORDS.len()]);
        out.push(' ');
    }
    out
}

fn build(count: u64) -> SearchIndex {
    let mut index = SearchIndex::new();
    for n in 0..count {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&n.to_be_bytes());
        index.upsert(&IndexDoc {
            id: ItemId::from_bytes(id),
            kind: RepKind::Text,
            pinned: false,
            pinned_at: None,
            last_used: Hlc::new(n, 0, DeviceTag::default()),
            first_copied_ms: n,
            copy_count: 1,
            text: Cow::Owned(text(n)),
        });
    }
    index
}

/// Slowest keystroke while typing each query one character at a time.
fn slowest_keystroke(index: &SearchIndex) -> Duration {
    ["huiyi", "hyjy", "supercy", "release notes", "zzzz"]
        .iter()
        .flat_map(|query| (1..=query.len()).map(move |end| &query[..end]))
        .map(|typed| {
            let start = Instant::now();
            let hits = index.search(
                &Query::parse(typed, MatchMode::Fuzzy),
                None,
                ListOrder::default(),
                50,
            );
            std::hint::black_box(hits);
            start.elapsed()
        })
        .max()
        .unwrap_or_default()
}

fn measure(count: u64, budget: Duration, heap_budget: usize) {
    let start = Instant::now();
    let index = build(count);
    let built = start.elapsed();
    let slowest = slowest_keystroke(&index);
    let heap = index.heap_bytes();

    eprintln!(
        "perf: {count} items: build {built:?}, slowest keystroke {slowest:?}, index heap {heap} bytes"
    );
    assert!(heap <= heap_budget, "index heap {heap} > {heap_budget}");
    if !cfg!(debug_assertions) {
        assert!(
            slowest <= budget,
            "slowest keystroke {slowest:?} > {budget:?}"
        );
    }
}

#[test]
#[ignore = "benchmark; run with scripts/docker-test.sh bench"]
fn perf_search_500_items() {
    measure(500, BUDGET_500, HEAP_BUDGET_500);
}

#[test]
#[ignore = "benchmark; run with scripts/docker-test.sh bench"]
fn perf_search_10k_items() {
    measure(10_000, BUDGET_10K, HEAP_BUDGET_10K);
}

#[test]
fn index_memory_stays_within_budget_for_the_default_size() {
    assert!(build(500).heap_bytes() <= HEAP_BUDGET_500);
}
