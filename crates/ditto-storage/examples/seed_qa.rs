//! Seeds a synthetic Ditto v2 data root for integration QA.
//!
//! Usage: cargo run -p ditto-storage --example seed_qa -- <data-root> [count] [settings-json]
//!
//! The optional third argument is a JSON object merged over the synthetic
//! defaults (for example to pre-configure QA hotkeys before the core starts).
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
    let mut arguments = std::env::args().skip(1);
    let root = arguments
        .next()
        .expect("usage: seed_qa <data-root> [count] [settings-json]");
    let count = arguments
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(TEXTS.len())
        .max(1);
    let extra_settings = arguments
        .next()
        .map(|value| {
            serde_json::from_str::<serde_json::Value>(&value)
                .expect("settings argument must be a JSON object")
        })
        .unwrap_or_else(|| serde_json::json!({}));
    let storage = Storage::open_data_root(Path::new(&root)).expect("open synthetic data root");
    let group_id = storage
        .list_groups()
        .expect("list synthetic groups")
        .into_iter()
        .find(|group| group.name == "QA synthetic group")
        .map_or_else(
            || {
                storage
                    .create_group("QA synthetic group", None, 1.0)
                    .expect("create synthetic group")
            },
            |group| group.id,
        );
    // Keep the synthetic corpus inside the configured retention window so
    // cursor-paging and scroll benchmarks see every row.
    let mut settings = serde_json::json!({
        "clipboard": {
            "max_entries": u32::try_from(count + 100).unwrap_or(u32::MAX),
            "expire_days": 3650
        }
    });
    merge_json(&mut settings, extra_settings);
    storage
        .set_settings(&settings)
        .expect("raise synthetic retention limits");
    for index in 0..count {
        let template = TEXTS[index % TEXTS.len()];
        let text = if index < TEXTS.len() {
            template.to_string()
        } else {
            format!("{template} synthetic row {index:05}")
        };
        let text = &text;
        // Keep the fixture inside the default 5-day retention window; the QA
        // harness starts the real core, which now runs retention at startup.
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(1_762_000_000_000)
            + index as i64 * 1_000;
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
            content_hash: {
                let mut hash = [0_u8; 32];
                hash[..8].copy_from_slice(&(index as u64).to_le_bytes());
                hash.to_vec()
            },
            formats_hash: {
                let mut hash = [0_u8; 32];
                hash[..8].copy_from_slice(&(index as u64 + 1_000_000).to_le_bytes());
                hash.to_vec()
            },
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
        "seeded {count} clips and group {group_id} into {}",
        Path::new(&root).display()
    );
}

fn merge_json(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base), serde_json::Value::Object(overlay)) => {
            for (key, value) in overlay {
                merge_json(base.entry(key).or_insert(serde_json::Value::Null), value);
            }
        }
        (base, overlay) => *base = overlay,
    }
}
