use std::{path::Path, time::Instant};

use ditto_migration::{MigrationOptions, MigrationSource, migrate};
use rusqlite::{Connection, params};
use tempfile::TempDir;
use windows::Win32::System::{
    ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
    Threading::GetCurrentProcess,
};

fn utf16le(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
#[ignore = "10k performance benchmark; run explicitly in release"]
fn migration_10k_reports_throughput_and_peak_memory() {
    let temp = TempDir::new().expect("temp");
    let database = temp.path().join("Ditto-10k.db");
    create_10k_fixture(&database);
    let destination = temp.path().join("data-v2");
    let started = Instant::now();
    let outcome = migrate(
        &MigrationSource {
            database: database.clone(),
            settings: None,
        },
        &destination,
        &MigrationOptions {
            batch_size: 500,
            ..MigrationOptions::default()
        },
    )
    .expect("10k migration");
    let elapsed = started.elapsed();
    let db_bytes = std::fs::metadata(destination.join("ditto.db"))
        .expect("db metadata")
        .len();
    let peak = peak_working_set_bytes();
    println!(
        "T6_10K_MIGRATE elapsed_ms={:.3} clips_per_sec={:.1} db_bytes={} peak_working_set_mib={:.1} clips={}",
        elapsed.as_secs_f64() * 1000.0,
        10_000.0 / elapsed.as_secs_f64(),
        db_bytes,
        peak as f64 / (1024.0 * 1024.0),
        outcome.report.imported_counts.clips
    );
    assert_eq!(outcome.report.imported_counts.clips, 10_000);
}

fn create_10k_fixture(path: &Path) {
    let mut connection = Connection::open(path).expect("open fixture");
    connection
        .execute_batch(include_str!("../../../tests/fixtures/legacy/schema.sql"))
        .expect("schema");
    let transaction = connection.transaction().expect("transaction");
    for group in 1..=10 {
        transaction
            .execute(
                "INSERT INTO Main(
                   lID, lDate, mText, lShortCut, lDontAutoDelete, CRC, bIsGroup,
                   lParentID, QuickPasteText, clipOrder, clipGroupOrder,
                   globalShortCut, lastPasteDate, stickyClipOrder,
                   stickyClipGroupOrder, MoveToGroupShortCut,
                   GlobalMoveToGroupShortCut
                 ) VALUES (?1, ?2, ?3, 0, 0, 0, 1, -1, '', ?4, ?4, 0, 0,
                           -2147483647, -2147483647, 0, 0)",
                params![
                    group as i64,
                    1_700_000_000_i64,
                    format!("G{group}"),
                    group as f64
                ],
            )
            .expect("group");
    }
    {
        let mut main = transaction
            .prepare(
                "INSERT INTO Main(
                   lID, lDate, mText, lShortCut, lDontAutoDelete, CRC, bIsGroup,
                   lParentID, QuickPasteText, clipOrder, clipGroupOrder,
                   globalShortCut, lastPasteDate, stickyClipOrder,
                   stickyClipGroupOrder, MoveToGroupShortCut,
                   GlobalMoveToGroupShortCut
                 ) VALUES (?1, ?2, ?3, 0, 0, ?4, 0, ?5, ?6, ?7, ?7, 0, ?2,
                           -2147483647, -2147483647, 0, 0)",
            )
            .expect("main statement");
        let mut data = transaction
            .prepare("INSERT INTO Data(lID, lParentID, strClipBoardFormat, ooData) VALUES (?1, ?2, 'CF_UNICODETEXT', ?3)")
            .expect("data statement");
        for index in 0..10_000_i64 {
            let id = 100 + index;
            let text = format!("perf sample {index:05} 中文内容");
            main.execute(params![
                id,
                1_700_000_000_i64 + index,
                text,
                index,
                (index % 10) + 1,
                format!("q{index}"),
                index as f64,
            ])
            .expect("clip");
            data.execute(params![200_000 + index, id, utf16le(&text)])
                .expect("data");
        }
    }
    transaction.commit().expect("commit fixture");
}

fn peak_working_set_bytes() -> u64 {
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let ok = GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
        if ok.is_ok() {
            counters.PeakWorkingSetSize as u64
        } else {
            0
        }
    }
}
