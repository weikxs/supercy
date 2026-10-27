#![no_main]
//! Indexing and searching never panic, whatever the texts and the query (pinyin included).
use std::borrow::Cow;

use cy_model::{DeviceTag, Hlc, ItemId, RepKind};
use cy_search::{IndexDoc, ListOrder, MatchMode, Query, SearchIndex, highlight};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (Vec<String>, String, u8)| {
    let (texts, query, mode) = input;
    let mut index = SearchIndex::new();
    for (n, text) in texts.iter().enumerate().take(64) {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(n as u64).to_be_bytes());
        index.upsert(&IndexDoc {
            id: ItemId::from_bytes(id),
            kind: RepKind::Text,
            pinned: n % 3 == 0,
            pinned_at: (n % 3 == 0).then(|| Hlc::new(n as u64, 1, DeviceTag::default())),
            last_used: Hlc::new(n as u64, 0, DeviceTag::default()),
            first_copied_ms: n as u64,
            copy_count: 1,
            text: Cow::Borrowed(text),
        });
    }
    let modes = [
        MatchMode::Fuzzy,
        MatchMode::Exact,
        MatchMode::Regex,
        MatchMode::Mixed,
    ];
    let mode = modes[usize::from(mode % 4)];

    let hits = index.search(&Query::parse(&query, mode), None, ListOrder::default(), 20);

    assert!(hits.len() <= 20);
    for text in texts.iter().take(8) {
        let chars = text.chars().count();
        assert!(
            highlight(text, &Query::parse(&query, mode))
                .iter()
                .all(|(a, b)| a < b && *b <= chars)
        );
    }
});
