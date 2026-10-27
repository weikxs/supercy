#![no_main]
//! Indexing and searching never panic, whatever the texts and the query (pinyin included).
use std::borrow::Cow;

use cy_model::{DeviceTag, Hlc, ItemId, RepKind};
use cy_search::{IndexDoc, MatchMode, Query, SearchIndex};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (Vec<String>, String, bool)| {
    let (texts, query, exact) = input;
    let mut index = SearchIndex::new();
    for (n, text) in texts.iter().enumerate().take(64) {
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&(n as u64).to_be_bytes());
        index.upsert(&IndexDoc {
            id: ItemId::from_bytes(id),
            kind: RepKind::Text,
            pinned: n % 3 == 0,
            last_used: Hlc::new(n as u64, 0, DeviceTag::default()),
            text: Cow::Borrowed(text),
        });
    }
    let mode = if exact {
        MatchMode::Exact
    } else {
        MatchMode::Fuzzy
    };

    let hits = index.search(&Query::parse(&query, mode), None, 20);

    assert!(hits.len() <= 20);
});
