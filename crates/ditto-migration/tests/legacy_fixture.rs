use std::{
    fs,
    path::{Path, PathBuf},
};

use ditto_clipboard::dib_to_png;
use ditto_migration::{
    ExportOptions, ImportOptions, MigrationOptions, MigrationSource, SourceFingerprint,
    backup as backup_package, compact, create_export, dry_run, fingerprint, import_package,
    migrate, restore, verify_package,
};
use ditto_storage::Storage;
use rusqlite::{Connection, params};
use serde_json::Value;
use tempfile::TempDir;

const ROOT_FIXTURE: &str = "tests/fixtures/legacy";

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(ROOT_FIXTURE)
}

fn utf16le(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn html_format(fragment: &str) -> Vec<u8> {
    let prefix = "<html><body><!--StartFragment-->";
    let suffix = "<!--EndFragment--></body></html>";
    let mut html = format!("{prefix}{fragment}{suffix}").into_bytes();
    let start = prefix.len();
    let end = start + fragment.len();
    let header = format!(
        "Version:0.9\r\nStartHTML:0000000105\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
        html.len() + 105,
        start + 105,
        end + 105
    );
    let mut output = header.into_bytes();
    output.append(&mut html);
    output
}

fn hdrop(paths: &[&str]) -> Vec<u8> {
    let mut output = vec![0_u8; 20];
    output[0..4].copy_from_slice(&20_u32.to_le_bytes());
    output[16..20].copy_from_slice(&1_u32.to_le_bytes());
    for path in paths {
        output.extend_from_slice(&utf16le(path));
        output.extend_from_slice(&0_u16.to_le_bytes());
    }
    output.extend_from_slice(&0_u16.to_le_bytes());
    output
}

fn dib_2x2() -> Vec<u8> {
    let mut dib = vec![0_u8; 40 + 16];
    dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
    dib[4..8].copy_from_slice(&2_i32.to_le_bytes());
    dib[8..12].copy_from_slice(&2_i32.to_le_bytes());
    dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32_u16.to_le_bytes());
    for offset in [40, 48] {
        dib[offset..offset + 4].copy_from_slice(&[10, 20, 30, 255]);
        dib[offset + 4..offset + 8].copy_from_slice(&[40, 50, 60, 255]);
    }
    dib
}

#[allow(clippy::too_many_arguments)]
fn insert_main(
    connection: &Connection,
    id: i64,
    date: i64,
    text: &str,
    shortcut: i64,
    dont_auto_delete: i64,
    crc: i64,
    is_group: i64,
    parent: i64,
    quick: &str,
    clip_order: f64,
    clip_group_order: f64,
    global_shortcut: i64,
    last_paste: i64,
    sticky_clip: f64,
    sticky_group: f64,
    move_shortcut: i64,
    global_move_shortcut: i64,
) {
    connection
        .execute(
            "INSERT INTO Main(
               lID, lDate, mText, lShortCut, lDontAutoDelete, CRC, bIsGroup,
               lParentID, QuickPasteText, clipOrder, clipGroupOrder,
               globalShortCut, lastPasteDate, stickyClipOrder,
               stickyClipGroupOrder, MoveToGroupShortCut,
               GlobalMoveToGroupShortCut
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
               ?14, ?15, ?16, ?17
             )",
            params![
                id,
                date,
                text,
                shortcut,
                dont_auto_delete,
                crc,
                is_group,
                parent,
                quick,
                clip_order,
                clip_group_order,
                global_shortcut,
                last_paste,
                sticky_clip,
                sticky_group,
                move_shortcut,
                global_move_shortcut
            ],
        )
        .expect("insert Main");
}

fn insert_data(connection: &Connection, id: i64, parent: i64, format: &str, data: &[u8]) {
    connection
        .execute(
            "INSERT INTO Data(lID, lParentID, strClipBoardFormat, ooData)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, parent, format, data],
        )
        .expect("insert Data");
}

fn create_fixture(temp: &TempDir) -> (PathBuf, PathBuf) {
    let fixture = fixture_dir();
    let database = temp.path().join("Ditto.db");
    let connection = Connection::open(&database).expect("open fixture db");
    connection
        .execute_batch(include_str!("../../../tests/fixtures/legacy/schema.sql"))
        .expect("legacy schema");
    insert_main(
        &connection,
        10,
        1_700_000_000,
        "Work",
        0,
        0,
        0,
        1,
        -1,
        "",
        100.0,
        100.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        11,
        1_700_000_001,
        "中文分组",
        0,
        0,
        0,
        1,
        10,
        "",
        101.0,
        101.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        20,
        1_700_000_100,
        "中文 Sample 001",
        17,
        1,
        0x1234_5678,
        0,
        -1,
        "qp 中文",
        300.5,
        301.5,
        1,
        1_700_000_200,
        42.0,
        -2_147_483_647.0,
        7,
        1,
    );
    insert_main(
        &connection,
        21,
        1_700_000_101,
        "Rich <b>sample</b>",
        18,
        0,
        0x2222,
        0,
        11,
        "quick rich",
        302.5,
        303.5,
        0,
        1_700_000_201,
        -2_147_483_647.0,
        50.0,
        8,
        0,
    );
    insert_main(
        &connection,
        22,
        1_700_000_102,
        "C:\\QA\\one.txt",
        0,
        0,
        0x3333,
        0,
        -1,
        "",
        304.0,
        304.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        23,
        1_700_000_103,
        "image clip",
        0,
        0,
        0x4444,
        0,
        -1,
        "",
        305.0,
        305.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        24,
        1_700_000_104,
        "custom clip",
        0,
        0,
        0x5555,
        0,
        -1,
        "",
        306.0,
        306.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        25,
        1_700_000_105,
        "中文 Sample 001",
        19,
        0,
        0x6666,
        0,
        -1,
        "duplicate",
        307.0,
        307.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        26,
        1_700_000_106,
        "empty data",
        0,
        0,
        0x7777,
        0,
        -1,
        "",
        308.0,
        308.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        27,
        1_700_000_107,
        "bad dib",
        0,
        0,
        0x8888,
        0,
        -1,
        "",
        309.0,
        309.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );

    insert_data(
        &connection,
        100,
        20,
        "CF_UNICODETEXT",
        &utf16le("中文 Sample 001"),
    );
    insert_data(
        &connection,
        101,
        21,
        "CF_UNICODETEXT",
        &utf16le("Rich sample"),
    );
    insert_data(
        &connection,
        102,
        21,
        "HTML Format",
        &html_format("<b>Hello &amp; world</b>"),
    );
    insert_data(
        &connection,
        103,
        21,
        "Rich Text Format",
        br"{\rtf1 Rich\par sample}",
    );
    insert_data(
        &connection,
        104,
        22,
        "CF_HDROP",
        &hdrop(&["C:\\QA\\one.txt", "C:\\QA\\two.txt"]),
    );
    let dib = dib_2x2();
    connectless_assert_png(&dib);
    insert_data(&connection, 105, 23, "CF_DIB", &dib);
    insert_data(&connection, 106, 24, "My Custom Format", b"custom\x00bytes");
    insert_data(
        &connection,
        107,
        25,
        "CF_UNICODETEXT",
        &utf16le("中文 Sample 001"),
    );
    insert_data(&connection, 108, 26, "CF_TEXT", &[]);
    insert_data(&connection, 109, 27, "CF_DIB", b"bad");
    connection
        .execute_batch(
            "INSERT INTO Types(lID, TypeText) VALUES
               (1, 'CF_TEXT'), (2, 'HTML Format'), (3, 'Rich Text Format'),
               (4, 'PNG'), (5, 'CF_DIB'), (6, 'CF_HDROP'),
               (7, 'My Custom Format');
             INSERT INTO CopyBuffers(lID, lClipID, lCopyBuffer) VALUES
               (1, 20, 0), (2, 21, 1), (3, 22, 2), (4, 23, 3), (5, 24, 4),
               (6, 25, 1);",
        )
        .expect("seed types and buffers");
    drop(connection);
    (database, fixture.join("Ditto.Settings.fixture"))
}

fn connectless_assert_png(dib: &[u8]) {
    assert!(dib_to_png(dib).expect("valid DIB").starts_with(b"\x89PNG"));
}

fn source(database: &Path, settings: &Path) -> MigrationSource {
    MigrationSource {
        database: database.to_path_buf(),
        settings: Some(settings.to_path_buf()),
    }
}

#[test]
fn synthetic_legacy_migration_matches_counts_content_and_is_idempotent() {
    let temp = TempDir::new().expect("temp");
    let (database, settings) = create_fixture(&temp);
    let source = source(&database, &settings);
    let before = fingerprint(&database).expect("fingerprint");
    let destination = temp.path().join("data-v2");

    let preview = dry_run(&source).expect("dry run");
    assert_eq!(preview.source_counts.clips, 8);
    assert_eq!(preview.source_counts.groups, 2);
    assert_eq!(preview.source_counts.empty_formats, 1);
    assert!(preview.copy_buffer_slots.contains(&1));
    assert!(
        !destination.exists(),
        "dry-run must not create the destination"
    );
    assert_eq!(
        before.sha256,
        fingerprint(&database)
            .expect("fingerprint after dry run")
            .sha256
    );

    let outcome = migrate(
        &source,
        &destination,
        &MigrationOptions {
            batch_size: 3,
            ..MigrationOptions::default()
        },
    )
    .expect("migration");
    assert_eq!(outcome.report.imported_counts.clips, 8);
    assert_eq!(outcome.report.imported_counts.groups, 2);
    assert!(outcome.report.imported_counts.pinned >= 2);
    assert!(
        outcome
            .report
            .warnings
            .iter()
            .any(|warning| warning.contains("empty"))
    );
    assert_source_unchanged(&database, &before);

    let connection = Connection::open(destination.join("ditto.db")).expect("new db");
    let clip_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))
        .expect("clip count");
    let group_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM groups", [], |row| row.get(0))
        .expect("group count");
    assert_eq!(clip_count, 8);
    assert_eq!(group_count, 2);

    let (
        preview_text,
        text_content,
        quick_paste,
        pinned,
        pin_order,
        dont_delete,
        shortcut,
        global_shortcut,
        move_shortcut,
        global_move_shortcut,
        last_pasted,
        legacy_crc,
    ): (
        String,
        String,
        String,
        i64,
        f64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = connection
        .query_row(
            "SELECT preview_text, text_content, quick_paste_text, is_pinned,
                    pin_order, dont_auto_delete, shortcut, global_shortcut,
                    move_to_group_shortcut, global_move_to_group_shortcut,
                    last_pasted_at_ms, legacy_crc
             FROM clips WHERE legacy_id = 20",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )
        .expect("clip 20");
    assert_eq!(preview_text, "中文 Sample 001");
    assert!(text_content.contains("中文 Sample 001"));
    assert_eq!(quick_paste, "qp 中文");
    assert_eq!(pinned, 1);
    assert_eq!(pin_order, 42.0);
    assert_eq!(dont_delete, 1);
    assert_eq!(shortcut, 17);
    assert_eq!(global_shortcut, 1);
    assert_eq!(move_shortcut, 7);
    assert_eq!(global_move_shortcut, 1);
    assert_eq!(last_pasted, 1_700_000_200_000);
    assert_eq!(legacy_crc, 0x1234_5678);

    let group_parent: Option<i64> = connection
        .query_row(
            "SELECT parent_id FROM groups WHERE legacy_id = 11",
            [],
            |row| row.get(0),
        )
        .expect("group parent");
    let work_id: i64 = connection
        .query_row("SELECT id FROM groups WHERE legacy_id = 10", [], |row| {
            row.get(0)
        })
        .expect("work id");
    assert_eq!(group_parent, Some(work_id));

    let rich_formats: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM clip_formats f
             JOIN clips c ON c.id = f.clip_id
             WHERE c.legacy_id = 21
               AND f.format_name IN ('CF_UNICODETEXT', 'HTML Format', 'Rich Text Format')",
            [],
            |row| row.get(0),
        )
        .expect("rich formats");
    assert_eq!(rich_formats, 3);

    let image_formats: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM clip_formats f
             JOIN clips c ON c.id = f.clip_id
             WHERE c.legacy_id = 23 AND f.format_name IN ('CF_DIB', 'PNG')",
            [],
            |row| row.get(0),
        )
        .expect("image formats");
    assert_eq!(image_formats, 2);
    let thumb_path: Option<String> = connection
        .query_row(
            "SELECT thumb_path FROM clips WHERE legacy_id = 23",
            [],
            |row| row.get(0),
        )
        .expect("thumb");
    assert!(
        destination
            .join(thumb_path.expect("thumbnail path"))
            .is_file()
    );

    let duplicate_count: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT content_hash) FROM clips WHERE legacy_id IN (20, 25)",
            [],
            |row| row.get(0),
        )
        .expect("duplicate hash");
    assert_eq!(duplicate_count, 1);
    let duplicate_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM clips WHERE legacy_id IN (20, 25)",
            [],
            |row| row.get(0),
        )
        .expect("duplicate rows");
    assert_eq!(duplicate_rows, 2);

    let empty_formats: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM clip_formats f
             JOIN clips c ON c.id = f.clip_id WHERE c.legacy_id = 26",
            [],
            |row| row.get(0),
        )
        .expect("empty formats");
    assert_eq!(empty_formats, 0);

    let buffer_slots: Vec<(i64, Option<i64>)> = {
        let mut statement = connection
            .prepare(
                "SELECT b.slot, c.legacy_id FROM copy_buffers b
                 LEFT JOIN clips c ON c.id = b.clip_id ORDER BY b.slot",
            )
            .expect("buffer query");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("buffers")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("buffer rows")
    };
    assert_eq!(
        buffer_slots,
        vec![
            (1, Some(20)),
            (2, Some(25)),
            (3, Some(22)),
            (4, Some(23)),
            (5, Some(24))
        ]
    );
    drop(connection);

    let storage = Storage::open_data_root(&destination).expect("reopen");
    assert_eq!(
        storage
            .get_setting("clipboard.max_entries")
            .expect("setting"),
        Some(Value::from(123))
    );
    assert_eq!(
        storage
            .get_setting("appearance.popup_width")
            .expect("width"),
        Some(Value::from(500))
    );
    let unknown: String = Connection::open(destination.join("ditto.db"))
        .expect("meta db")
        .query_row(
            "SELECT value FROM meta WHERE key = 'legacy_settings'",
            [],
            |row| row.get(0),
        )
        .expect("unknown settings");
    assert!(unknown.contains("UnknownMigrationKey"));
    assert!(!unknown.contains("never-migrate-this"));
    drop(storage);

    let second = migrate(
        &source,
        &destination,
        &MigrationOptions {
            batch_size: 4,
            replace_existing: true,
            ..MigrationOptions::default()
        },
    )
    .expect("idempotent rerun");
    assert_eq!(second.report.imported_counts.clips, 8);
    let connection = Connection::open(destination.join("ditto.db")).expect("reopen db");
    let count_after: i64 = connection
        .query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))
        .expect("count after");
    assert_eq!(count_after, 8);
    let (phase, completed_at, counts_json): (String, Option<i64>, Option<String>) = connection
        .query_row(
            "SELECT phase, completed_at_ms, counts_json
             FROM migration_state ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("migration state");
    assert_eq!(phase, "verified");
    assert!(completed_at.is_some());
    assert!(counts_json.expect("counts json").contains("\"clips\":8"));
    drop(connection);
    assert_source_unchanged(&database, &before);
}

#[test]
fn export_import_and_verify_round_trip_preserve_counts() {
    let temp = TempDir::new().expect("temp");
    let (database, settings) = create_fixture(&temp);
    let source = source(&database, &settings);
    let original = temp.path().join("original");
    let outcome = migrate(
        &source,
        &original,
        &MigrationOptions {
            batch_size: 2,
            ..MigrationOptions::default()
        },
    )
    .expect("migrate");
    assert_eq!(outcome.report.imported_counts.clips, 8);

    let package = temp.path().join("export.zip");
    let export = create_export(
        &original,
        &package,
        &ExportOptions {
            zip: true,
            replace_existing: false,
        },
    )
    .expect("export");
    assert!(export.verified);
    let verified = verify_package(&package).expect("verify");
    assert_eq!(verified.counts.clips, 8);
    assert!(verified.files_verified > 1);

    let imported = temp.path().join("imported");
    let report = import_package(
        &package,
        &imported,
        &ImportOptions {
            replace_existing: false,
            backup_dir: None,
        },
    )
    .expect("import");
    assert_eq!(report.counts.clips, 8);
    let connection = Connection::open(imported.join("ditto.db")).expect("imported db");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 8);

    let backups = temp.path().join("backups");
    let backup_report = backup_package(&imported, &backups, true).expect("backup");
    let restored = temp.path().join("restored");
    let restore_report = restore(
        Path::new(&backup_report.package_path),
        &restored,
        &ImportOptions::default(),
    )
    .expect("restore");
    assert_eq!(restore_report.counts.clips, 8);
    let compact_report = compact(&restored).expect("compact");
    assert!(compact_report.verified);
}

#[test]
fn failed_migration_leaves_existing_destination_untouched() {
    let temp = TempDir::new().expect("temp");
    let database = temp.path().join("broken.Ditto.db");
    let connection = Connection::open(&database).expect("fixture");
    connection
        .execute_batch(include_str!("../../../tests/fixtures/legacy/schema.sql"))
        .expect("schema");
    insert_main(
        &connection,
        1,
        1,
        "A",
        0,
        0,
        0,
        1,
        -1,
        "",
        1.0,
        1.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    insert_main(
        &connection,
        2,
        2,
        "a",
        0,
        0,
        0,
        1,
        -1,
        "",
        2.0,
        2.0,
        0,
        0,
        -2_147_483_647.0,
        -2_147_483_647.0,
        0,
        0,
    );
    drop(connection);
    let destination = temp.path().join("existing");
    drop(Storage::open_data_root(&destination).expect("existing v2 root"));
    fs::write(destination.join("sentinel.txt"), b"keep").expect("sentinel");
    let error = migrate(
        &MigrationSource {
            database,
            settings: None,
        },
        &destination,
        &MigrationOptions {
            replace_existing: true,
            ..MigrationOptions::default()
        },
    )
    .expect_err("duplicate group names should fail");
    assert!(error.to_string().contains("UNIQUE") || error.to_string().contains("constraint"));
    assert_eq!(
        fs::read(destination.join("sentinel.txt")).expect("sentinel"),
        b"keep"
    );
    assert!(fs::read_dir(temp.path()).expect("temp").all(|entry| {
        !entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .contains(".ditto-migrate")
    }));
}

fn assert_source_unchanged(database: &Path, before: &SourceFingerprint) {
    let after = fingerprint(database).expect("fingerprint after");
    assert_eq!(before.sha256, after.sha256);
    assert_eq!(before.size_bytes, after.size_bytes);
}
