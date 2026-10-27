//! Search behavior: English and Chinese cases, pinyin, ordering, filters.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail loudly, like the tests themselves"
)]

use std::borrow::Cow;

use cy_model::{DeviceTag, Hlc, ItemId, RepKind};
use cy_search::{IndexDoc, ListOrder, MatchMode, Query, SearchIndex};
use proptest::prelude::*;

fn id(n: u8) -> ItemId {
    ItemId::from_bytes([n; 16])
}

fn doc(n: u8, text: &str) -> IndexDoc<'_> {
    IndexDoc {
        id: id(n),
        kind: RepKind::Text,
        pinned: false,
        last_used: Hlc::new(u64::from(n), 0, DeviceTag::default()),
        first_copied_ms: u64::from(n),
        copy_count: 1,
        text: Cow::Borrowed(text),
    }
}

fn index(texts: &[&str]) -> SearchIndex {
    let mut index = SearchIndex::new();
    for (n, text) in texts.iter().enumerate() {
        index.upsert(&doc(u8::try_from(n).unwrap(), text));
    }
    index
}

fn top(index: &SearchIndex, query: &str) -> Option<ItemId> {
    index
        .search(
            &Query::parse(query, MatchMode::Fuzzy),
            None,
            ListOrder::default(),
            1,
        )
        .first()
        .map(|hit| hit.id)
}

const HISTORY: &[&str] = &[
    "Hello from Supercy",
    "会议纪要：下周一发布",
    "https://example.com/docs/getting-started",
    "报销单号 2026-001",
    "银行卡信息（示例）",
    "git commit -m \"docs: update readme\"",
    "重要通知",
    "lorem ipsum dolor sit amet",
];

#[test]
fn english_chinese_and_pinyin_queries_find_their_item() {
    let index = index(HISTORY);
    let cases = [
        ("supercy", 0),
        ("hfs", 0),
        ("会议", 1),
        ("huiyi", 1),
        ("hyjy", 1),
        ("huiyjy", 1),
        ("example docs", 2),
        ("getstart", 2),
        ("bxdh", 3),
        ("baoxiao", 3),
        ("yinhang", 4),
        ("commit readme", 5),
        ("zhongyao", 6),
        ("lorem", 7),
    ];

    for (query, expected) in cases {
        assert_eq!(top(&index, query), Some(id(expected)), "query {query:?}");
    }
}

#[test]
fn queries_that_match_nothing_return_nothing() {
    let index = index(HISTORY);

    assert_eq!(top(&index, "zzzz"), None);
    assert_eq!(top(&index, "supercy qqq"), None);
}

#[test]
fn exact_mode_matches_substrings_of_the_text_only() {
    let index = index(HISTORY);
    let exact = |q| {
        index.search(
            &Query::parse(q, MatchMode::Exact),
            None,
            ListOrder::default(),
            5,
        )
    };

    assert_eq!(exact("from super").first().map(|h| h.id), Some(id(0)));
    assert!(exact("hfs").is_empty());
    assert!(exact("huiyi").is_empty());
}

#[test]
fn better_matches_rank_first_and_ties_go_to_the_most_recent() {
    let index = index(&["a demo b", "demo", "d e m o", "demo"]);

    let hits = index.search(
        &Query::parse("demo", MatchMode::Fuzzy),
        None,
        ListOrder::default(),
        10,
    );
    let ids: Vec<ItemId> = hits.iter().map(|h| h.id).collect();

    assert_eq!(ids, vec![id(3), id(1), id(0), id(2)]);
}

#[test]
fn pinned_items_win_ties_and_lead_the_list() {
    let mut index = index(&["demo one", "demo two"]);
    index.upsert(&IndexDoc {
        pinned: true,
        ..doc(0, "demo one")
    });

    let listed = index.list(None, ListOrder::default(), 0, 10);
    let searched = index.search(
        &Query::parse("demo", MatchMode::Fuzzy),
        None,
        ListOrder::default(),
        10,
    );

    assert_eq!(listed, vec![id(0), id(1)]);
    assert_eq!(searched[0].id, id(0));
}

#[test]
fn empty_query_lists_by_recency_with_paging() {
    let index = index(&["a", "b", "c"]);

    let all = index.search(
        &Query::parse("", MatchMode::Fuzzy),
        None,
        ListOrder::default(),
        10,
    );

    assert_eq!(
        all.iter().map(|h| h.id).collect::<Vec<_>>(),
        vec![id(2), id(1), id(0)]
    );
    assert_eq!(index.list(None, ListOrder::default(), 1, 1), vec![id(1)]);
}

#[test]
fn kind_filter_treats_rich_text_as_text() {
    let mut index = index(&["plain demo"]);
    index.upsert(&IndexDoc {
        kind: RepKind::RichText,
        ..doc(1, "rich demo")
    });
    index.upsert(&IndexDoc {
        kind: RepKind::Url,
        ..doc(2, "https://example.com/demo")
    });

    let text_only = index.search(
        &Query::parse("demo", MatchMode::Fuzzy),
        Some(RepKind::Text),
        ListOrder::default(),
        10,
    );
    let links = index.list(Some(RepKind::Url), ListOrder::default(), 0, 10);

    assert_eq!(text_only.len(), 2);
    assert_eq!(links, vec![id(2)]);
}

#[test]
fn upsert_replaces_and_remove_deletes() {
    let mut index = index(&["old text"]);

    index.upsert(&doc(0, "new text"));

    assert_eq!(index.len(), 1);
    assert_eq!(top(&index, "old"), None);
    assert_eq!(top(&index, "new"), Some(id(0)));
    assert!(index.remove(id(0)));
    assert!(!index.remove(id(0)));
    assert!(index.is_empty());
}

#[test]
fn only_the_first_part_of_long_text_is_indexed() {
    let long = format!("{} needle", "x".repeat(10_000));
    let index = index(&[&long]);

    assert_eq!(top(&index, "needle"), None);
    assert!(index.heap_bytes() < 2_000);
}

proptest! {
    #[test]
    fn any_substring_of_indexed_ascii_text_is_found(text in "[a-z ]{1,80}", start in 0usize..80, len in 1usize..10) {
        let index = index(&[&text]);
        let start = start.min(text.len() - 1);
        let end = (start + len).min(text.len());
        let needle = text[start..end].trim();
        prop_assume!(!needle.is_empty() && !needle.contains("  "));

        prop_assert_eq!(top(&index, needle), Some(id(0)));
    }

    #[test]
    fn search_never_panics(texts in proptest::collection::vec(".{0,40}", 0..8), query in ".{0,20}") {
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let index = index(&refs);

        let _ = index.search(&Query::parse(&query, MatchMode::Fuzzy), None, ListOrder::default(), 10);
    }
}

fn ids_for(index: &SearchIndex, query: &str, mode: MatchMode) -> Vec<ItemId> {
    index
        .search(&Query::parse(query, mode), None, ListOrder::default(), 10)
        .into_iter()
        .map(|h| h.id)
        .collect()
}

#[test]
fn regex_mode_matches_patterns_and_invalid_patterns_match_nothing() {
    let index = index(&["order 123456 shipped", "no digits here", "Demo-42"]);

    assert_eq!(ids_for(&index, r"\d{6}", MatchMode::Regex), vec![id(0)]);
    assert_eq!(
        ids_for(&index, r"^demo-\d+$", MatchMode::Regex),
        vec![id(2)]
    );
    assert!(ids_for(&index, "(", MatchMode::Regex).is_empty());
}

#[test]
fn mixed_mode_prefers_exact_then_regex_then_fuzzy() {
    let index = index(&["hello from supercy", "h.f.s", "hxfxs"]);

    assert_eq!(ids_for(&index, "from", MatchMode::Mixed), vec![id(0)]);
    assert_eq!(ids_for(&index, r"h\.f", MatchMode::Mixed), vec![id(1)]);
    let fuzzy = ids_for(&index, "hfs", MatchMode::Mixed);
    assert_eq!(fuzzy.len(), 3);
}

fn sorted_index() -> SearchIndex {
    let mut index = SearchIndex::new();
    let docs = [
        (0u8, 30u64, 1u32, false),
        (1, 10, 5, false),
        (2, 20, 3, true),
    ];
    for (n, first, count, pinned) in docs {
        index.upsert(&IndexDoc {
            pinned,
            first_copied_ms: first,
            copy_count: count,
            ..doc(n, "demo")
        });
    }
    index
}

#[test]
fn list_orders_by_last_use_first_copy_or_copy_count() {
    use cy_search::{PinPosition, SortOrder};
    let index = sorted_index();
    let order = |sort, pins| ListOrder { sort, pins };

    assert_eq!(
        index.list(None, order(SortOrder::LastUsed, PinPosition::Top), 0, 9),
        vec![id(2), id(1), id(0)]
    );
    assert_eq!(
        index.list(None, order(SortOrder::FirstCopied, PinPosition::Top), 0, 9),
        vec![id(2), id(0), id(1)]
    );
    assert_eq!(
        index.list(None, order(SortOrder::CopyCount, PinPosition::Top), 0, 9),
        vec![id(2), id(1), id(0)]
    );
    assert_eq!(
        index.list(
            None,
            order(SortOrder::FirstCopied, PinPosition::Bottom),
            0,
            9
        ),
        vec![id(0), id(1), id(2)]
    );
    let searched = index.search(
        &Query::parse("demo", MatchMode::Fuzzy),
        None,
        order(SortOrder::CopyCount, PinPosition::Bottom),
        9,
    );
    assert_eq!(
        searched.iter().map(|h| h.id).collect::<Vec<_>>(),
        vec![id(1), id(0), id(2)]
    );
}
