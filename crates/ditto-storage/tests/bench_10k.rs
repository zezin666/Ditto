use std::time::Instant;

use ditto_model::{ClipKind, ClipQuery, NewClip, NewClipFormat, SearchMode};
use ditto_storage::{Storage, sha256};

#[test]
fn synthetic_10k_insert_search_and_keyset_budget() {
    let temp = tempfile::tempdir().expect("temporary data root");
    let storage = Storage::open_data_root(temp.path()).expect("open storage");
    let mut groups = Vec::new();
    for index in 0..40 {
        groups.push(
            storage
                .create_group(&format!("Group {index:02}"), None, index as f64)
                .expect("root group"),
        );
    }
    for index in 0..10 {
        groups.push(
            storage
                .create_group(
                    &format!("Child {index:02}"),
                    Some(groups[index]),
                    1_000.0 + index as f64,
                )
                .expect("child group"),
        );
    }

    let clips = (0..10_000)
        .map(|index| synthetic_clip(index, &groups))
        .collect::<Vec<_>>();
    let insert_started = Instant::now();
    let inserted = storage.insert_many(&clips).expect("bulk insert");
    let insert_elapsed = insert_started.elapsed();
    assert_eq!(inserted, 10_000);
    storage.optimize().expect("optimize");

    let queries = [
        ("browse", String::new(), SearchMode::Contains),
        ("english", "needleword".to_string(), SearchMode::Contains),
        ("chinese", "中文搜索".to_string(), SearchMode::Contains),
        ("two_char_en", "ne".to_string(), SearchMode::Contains),
        ("two_char_zh", "搜索".to_string(), SearchMode::Contains),
        ("quick_paste", "qe".to_string(), SearchMode::QuickPaste),
    ];
    let mut report = Vec::new();
    for (name, query, mode) in queries {
        let mut timings = Vec::new();
        for _ in 0..5 {
            let _ = storage
                .search_clips(&ClipQuery {
                    query: query.clone(),
                    mode,
                    limit: 200,
                    ..Default::default()
                })
                .expect("warmup");
        }
        for _ in 0..30 {
            let started = Instant::now();
            let page = storage
                .search_clips(&ClipQuery {
                    query: query.clone(),
                    mode,
                    limit: 200,
                    ..Default::default()
                })
                .expect("search");
            assert!(page.items.len() <= 200);
            timings.push(started.elapsed().as_secs_f64() * 1_000.0);
        }
        timings.sort_by(f64::total_cmp);
        report.push((
            name,
            percentile(&timings, 50.0),
            percentile(&timings, 95.0),
            percentile(&timings, 99.0),
        ));
    }

    let mut cursor = None;
    for _ in 0..10 {
        let page = storage
            .search_clips(&ClipQuery {
                query: String::new(),
                limit: 200,
                cursor,
                ..Default::default()
            })
            .expect("keyset page");
        assert!(page.items.len() <= 200);
        cursor = page.next_cursor;
    }

    let db_size = storage.database_size_bytes().expect("database size");
    let throughput = 10_000.0 / insert_elapsed.as_secs_f64();
    println!(
        "T2_10K_BENCH insert_ms={:.3} insert_per_sec={throughput:.1} db_bytes={db_size}",
        insert_elapsed.as_secs_f64() * 1_000.0
    );
    for (name, p50, p95, p99) in &report {
        println!("T2_10K_SEARCH query={name} p50_ms={p50:.3} p95_ms={p95:.3} p99_ms={p99:.3}");
    }
    let required_p95 = report
        .iter()
        .map(|(_, _, p95, _)| *p95)
        .fold(0.0_f64, f64::max);
    assert!(
        required_p95 < 50.0,
        "required search p95 exceeded 50 ms: {required_p95:.3}"
    );
}

fn synthetic_clip(index: usize, groups: &[i64]) -> NewClip {
    let text = match index % 10 {
        0..=6 => format!(
            "needleword EN sample {index:05} {}",
            "alpha beta gamma delta ".repeat(8 + index % 20)
        ),
        7 | 8 => format!(
            "中文搜索 合成样本 {index:05} {}",
            "中文剪贴板内容测试 ".repeat(8 + index % 20)
        ),
        _ => format!(
            "mixed needleword 中文搜索 {index:05} {}",
            "content 内容 ".repeat(10 + index % 20)
        ),
    };
    let text_bytes = text.as_bytes().to_vec();
    let mut formats = vec![NewClipFormat {
        format_name: "CF_UNICODETEXT".to_string(),
        seq: 0,
        data: text_bytes.clone(),
        mime: Some("text/plain".to_string()),
        is_primary: true,
    }];
    if index % 5 == 0 {
        formats.push(NewClipFormat {
            format_name: "HTML Format".to_string(),
            seq: 1,
            data: format!("<p>{text}</p>").into_bytes(),
            mime: Some("text/html".to_string()),
            is_primary: false,
        });
    }
    if index % 1_000 == 0 {
        formats.push(NewClipFormat {
            format_name: "PNG".to_string(),
            seq: formats.len() as u32,
            data: one_pixel_png(),
            mime: Some("image/png".to_string()),
            is_primary: false,
        });
    }
    let content_hash = sha256(format!("CF_UNICODETEXT\0{text}").as_bytes()).to_vec();
    let format_hashes = formats
        .iter()
        .flat_map(|format| sha256(&format.data))
        .collect::<Vec<_>>();
    let formats_hash = sha256(&format_hashes).to_vec();
    let created_at_ms = 1_600_000_000_000_i64 + (index as i64 * 60_000);
    NewClip {
        kind: ClipKind::Text,
        primary_format: Some("CF_UNICODETEXT".to_string()),
        text_content: Some(text.clone()),
        text_truncated: false,
        preview_text: text.chars().take(512).collect(),
        quick_paste_text: if index < 100 {
            format!("qe quick paste {index:03}")
        } else {
            String::new()
        },
        description: text,
        source_app: Some(format!("synthetic-{}.exe", index % 17)),
        source_window_title: Some("T2 synthetic benchmark".to_string()),
        content_hash,
        formats_hash,
        hash_algo: "sha256".to_string(),
        size_bytes: formats.iter().map(|format| format.data.len() as u64).sum(),
        is_pinned: index >= 9_800,
        pin_order: if index >= 9_800 {
            (index - 9_800) as f64
        } else {
            0.0
        },
        group_id: Some(groups[index % groups.len()]),
        sort_order: created_at_ms as f64,
        group_sort_order: created_at_ms as f64,
        created_at_ms,
        dont_auto_delete: index % 997 == 0,
        has_image: index % 1_000 == 0,
        thumbnail_png: None,
        formats,
    }
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    let rank = ((percentile / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn one_pixel_png() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31,
        0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}
