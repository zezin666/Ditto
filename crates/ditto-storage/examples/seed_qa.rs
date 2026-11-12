//! Seeds a synthetic Ditto v2 data root for integration QA.
//!
//! Usage: cargo run -p ditto-storage --example seed_qa -- <data-root>
//!
//! The fixture never reads the clipboard, the user profile, or an existing
//! Ditto database. It only writes into the directory passed on the command line.

use std::path::Path;

use ditto_model::{ClipKind, NewClip, NewClipFormat};
use ditto_storage::Storage;

const TEXTS: [&str; 3] = [
    "DITTO-QA-SYNTHETIC seed clip",
    "Synthetic QA clip with a longer body for preview and search checks.",
    "合成 QA 测试剪贴条目：搜索、分组与复制缓冲区。",
];

fn main() {
    let root = std::env::args().nth(1).expect("usage: seed_qa <data-root>");
    let storage = Storage::open_data_root(Path::new(&root)).expect("open synthetic data root");
    let group_id = storage
        .create_group("QA synthetic group", None, 1.0)
        .expect("create synthetic group");
    for (index, text) in TEXTS.iter().enumerate() {
        let created_at_ms = 1_762_000_000_000_i64 + index as i64 * 1_000;
        let clip = NewClip {
            kind: ClipKind::Text,
            primary_format: Some("CF_UNICODETEXT".to_string()),
            text_content: Some((*text).to_string()),
            text_truncated: false,
            preview_text: (*text).to_string(),
            quick_paste_text: (*text).to_string(),
            description: (*text).to_string(),
            source_app: Some("Ditto QA".to_string()),
            source_window_title: Some("Synthetic fixture".to_string()),
            content_hash: vec![u8::try_from(index + 1).unwrap_or(1); 32],
            formats_hash: vec![u8::try_from(index + 11).unwrap_or(11); 32],
            hash_algo: "sha256".to_string(),
            size_bytes: text.len() as u64,
            is_pinned: index == 0,
            pin_order: f64::from(u32::try_from(index).unwrap_or(0)),
            group_id: (index == 0).then_some(group_id),
            sort_order: 1.0 + index as f64,
            group_sort_order: 1.0 + index as f64,
            created_at_ms,
            dont_auto_delete: false,
            has_image: false,
            thumbnail_png: None,
            formats: vec![NewClipFormat {
                format_name: "CF_UNICODETEXT".to_string(),
                seq: 0,
                data: text
                    .encode_utf16()
                    .chain(Some(0))
                    .flat_map(u16::to_le_bytes)
                    .collect(),
                mime: Some("text/plain".to_string()),
                is_primary: true,
            }],
        };
        storage
            .insert_clip(&clip, false)
            .expect("insert synthetic clip");
    }
    println!(
        "seeded {} clips and group {group_id} into {}",
        TEXTS.len(),
        Path::new(&root).display()
    );
}
