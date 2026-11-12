use ditto_model::{ClipKind, NewClip, NewClipFormat};
use tempfile::TempDir;

use crate::{Storage, blob};

pub fn temp_storage() -> (TempDir, Storage) {
    let temp = tempfile::tempdir().expect("temp directory");
    let storage = Storage::open_data_root(temp.path()).expect("open storage");
    (temp, storage)
}

pub fn test_clip(text: &str, hash_seed: &str) -> NewClip {
    let text_bytes = text.as_bytes().to_vec();
    let rich = format!("{{\\rtf1 {text}}}").into_bytes();
    let content_hash = blob::sha256(format!("CF_UNICODETEXT\0{hash_seed}").as_bytes()).to_vec();
    let mut format_hashes = Vec::new();
    format_hashes.extend_from_slice(&blob::sha256(&text_bytes));
    format_hashes.extend_from_slice(&blob::sha256(&rich));
    let formats_hash = blob::sha256(&format_hashes).to_vec();
    NewClip {
        kind: ClipKind::Text,
        primary_format: Some("CF_UNICODETEXT".to_string()),
        text_content: Some(text.to_string()),
        text_truncated: false,
        preview_text: text.chars().take(512).collect(),
        quick_paste_text: String::new(),
        description: text.to_string(),
        source_app: Some("synthetic.exe".to_string()),
        source_window_title: Some("Synthetic fixture".to_string()),
        content_hash,
        formats_hash,
        hash_algo: "sha256".to_string(),
        size_bytes: (text_bytes.len() + rich.len()) as u64,
        is_pinned: false,
        pin_order: 0.0,
        group_id: None,
        sort_order: 1.0,
        group_sort_order: 1.0,
        created_at_ms: 1_700_000_000_000,
        dont_auto_delete: false,
        has_image: false,
        thumbnail_png: None,
        formats: vec![
            NewClipFormat {
                format_name: "CF_UNICODETEXT".to_string(),
                seq: 0,
                data: text_bytes,
                mime: Some("text/plain".to_string()),
                is_primary: true,
            },
            NewClipFormat {
                format_name: "Rich Text Format".to_string(),
                seq: 1,
                data: rich,
                mime: Some("application/rtf".to_string()),
                is_primary: false,
            },
        ],
    }
}
