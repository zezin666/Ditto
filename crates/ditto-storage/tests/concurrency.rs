use std::thread;

use ditto_model::{ClipKind, ClipQuery, NewClip, NewClipFormat};
use ditto_storage::{Storage, sha256};

#[test]
fn concurrent_readers_and_writers_share_wal_storage() {
    let temp = tempfile::tempdir().expect("temporary data root");
    let storage = Storage::open_data_root(temp.path()).expect("open storage");
    let mut writers = Vec::new();
    for writer in 0..4 {
        let storage = storage.clone();
        writers.push(thread::spawn(move || {
            for index in 0..50 {
                let clip = clip(writer * 50 + index);
                storage
                    .insert_clip(&clip, false)
                    .expect("concurrent insert");
            }
        }));
    }
    let mut readers = Vec::new();
    for _ in 0..4 {
        let storage = storage.clone();
        readers.push(thread::spawn(move || {
            for _ in 0..50 {
                let page = storage
                    .search_clips(&ClipQuery {
                        query: "concurrent".to_string(),
                        limit: 50,
                        ..Default::default()
                    })
                    .expect("concurrent search");
                assert!(page.items.len() <= 50);
            }
        }));
    }
    for writer in writers {
        writer.join().expect("writer thread");
    }
    for reader in readers {
        reader.join().expect("reader thread");
    }
    let page = storage
        .search_clips(&ClipQuery {
            query: String::new(),
            limit: 1_000,
            ..Default::default()
        })
        .expect("final search");
    assert_eq!(page.total, 200);
    storage.checkpoint().expect("checkpoint");
}

fn clip(index: usize) -> NewClip {
    let text = format!("concurrent synthetic clip {index:03}");
    let data = text.as_bytes().to_vec();
    let content_hash = sha256(format!("CF_UNICODETEXT\0{text}").as_bytes()).to_vec();
    let formats_hash = sha256(&[sha256(b"CF_UNICODETEXT"), sha256(&data)].concat()).to_vec();
    NewClip {
        kind: ClipKind::Text,
        primary_format: Some("CF_UNICODETEXT".to_string()),
        text_content: Some(text.clone()),
        text_truncated: false,
        preview_text: text.clone(),
        quick_paste_text: String::new(),
        description: text,
        source_app: Some("concurrency-test.exe".to_string()),
        source_window_title: Some("Synthetic concurrency fixture".to_string()),
        content_hash,
        formats_hash,
        hash_algo: "sha256".to_string(),
        size_bytes: data.len() as u64,
        is_pinned: false,
        pin_order: 0.0,
        group_id: None,
        sort_order: index as f64,
        group_sort_order: index as f64,
        created_at_ms: 1_700_000_000_000 + index as i64,
        dont_auto_delete: false,
        has_image: false,
        thumbnail_png: None,
        formats: vec![NewClipFormat {
            format_name: "CF_UNICODETEXT".to_string(),
            seq: 0,
            data,
            mime: Some("text/plain".to_string()),
            is_primary: true,
        }],
    }
}
