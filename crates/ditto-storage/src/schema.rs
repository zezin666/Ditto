use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

use regex::{Regex, RegexBuilder};
use rusqlite::{Connection, Transaction};

use crate::{StorageError, StorageResult};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SCHEMA_V1: &str = r#"
CREATE TABLE schema_version (
  version       INTEGER PRIMARY KEY,
  applied_at_ms INTEGER NOT NULL,
  app_version   TEXT    NOT NULL
) STRICT;

CREATE TABLE meta (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE groups (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT    NOT NULL,
  parent_id     INTEGER REFERENCES groups(id) ON DELETE CASCADE,
  sort_order    REAL    NOT NULL DEFAULT 0,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  legacy_id     INTEGER UNIQUE
) STRICT;

CREATE UNIQUE INDEX groups_unique_name
  ON groups (COALESCE(parent_id, -1), name COLLATE NOCASE);
CREATE INDEX groups_parent_sort ON groups (parent_id, sort_order, name);

CREATE TABLE clips (
  id                    INTEGER PRIMARY KEY AUTOINCREMENT,
  legacy_id             INTEGER UNIQUE,
  kind                  TEXT    NOT NULL
                          CHECK (kind IN ('text','html','rtf','image','files','custom')),
  primary_format        TEXT,
  text_content          TEXT,
  text_truncated        INTEGER NOT NULL DEFAULT 0 CHECK (text_truncated IN (0,1)),
  preview_text          TEXT    NOT NULL DEFAULT '',
  quick_paste_text      TEXT    NOT NULL DEFAULT '',
  description           TEXT    NOT NULL DEFAULT '',
  source_app            TEXT,
  source_window_title   TEXT,
  content_hash          BLOB    NOT NULL,
  formats_hash          BLOB    NOT NULL,
  hash_algo             TEXT    NOT NULL DEFAULT 'sha256',
  size_bytes            INTEGER NOT NULL DEFAULT 0,
  is_pinned             INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0,1)),
  pin_order             REAL    NOT NULL DEFAULT 0,
  group_id              INTEGER REFERENCES groups(id) ON DELETE SET NULL,
  sort_order            REAL    NOT NULL DEFAULT 0,
  group_sort_order      REAL    NOT NULL DEFAULT 0,
  is_deleted            INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0,1)),
  deleted_at_ms         INTEGER,
  created_at_ms         INTEGER NOT NULL,
  updated_at_ms         INTEGER NOT NULL,
  last_pasted_at_ms     INTEGER,
  paste_count           INTEGER NOT NULL DEFAULT 0,
  dont_auto_delete      INTEGER NOT NULL DEFAULT 0 CHECK (dont_auto_delete IN (0,1)),
  shortcut              INTEGER,
  global_shortcut       INTEGER NOT NULL DEFAULT 0 CHECK (global_shortcut IN (0,1)),
  move_to_group_shortcut INTEGER,
  global_move_to_group_shortcut INTEGER NOT NULL DEFAULT 0
                                  CHECK (global_move_to_group_shortcut IN (0,1)),
  has_image             INTEGER NOT NULL DEFAULT 0 CHECK (has_image IN (0,1)),
  thumb_path            TEXT,
  legacy_crc            INTEGER,
  legacy_meta           TEXT
) STRICT;

CREATE INDEX clips_recent
  ON clips (is_deleted, is_pinned DESC, pin_order DESC, sort_order DESC, id DESC);
CREATE INDEX clips_group
  ON clips (group_id, is_deleted, is_pinned DESC, pin_order DESC, group_sort_order DESC, id DESC);
CREATE INDEX clips_hash
  ON clips (content_hash, formats_hash, is_deleted);
CREATE INDEX clips_shortcut
  ON clips (shortcut) WHERE shortcut IS NOT NULL;
CREATE INDEX clips_global_shortcut
  ON clips (global_shortcut) WHERE global_shortcut = 1;
CREATE INDEX clips_deleted
  ON clips (is_deleted, deleted_at_ms) WHERE is_deleted = 1;

CREATE TABLE clip_formats (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  clip_id       INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
  format_name   TEXT    NOT NULL,
  seq           INTEGER NOT NULL DEFAULT 0,
  size_bytes    INTEGER NOT NULL DEFAULT 0,
  mime          TEXT,
  data          BLOB,
  data_path     TEXT,
  sha256        BLOB,
  is_primary    INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0,1)),
  legacy_data_id INTEGER UNIQUE,
  UNIQUE (clip_id, format_name, seq),
  CHECK (data IS NOT NULL OR data_path IS NOT NULL)
) STRICT;

CREATE INDEX clip_formats_clip ON clip_formats (clip_id, seq);
CREATE INDEX clip_formats_name ON clip_formats (format_name);
CREATE INDEX clip_formats_sha  ON clip_formats (sha256);
CREATE INDEX clip_formats_legacy ON clip_formats (legacy_data_id);

CREATE TABLE copy_buffers (
  slot          INTEGER PRIMARY KEY CHECK (slot BETWEEN 1 AND 5),
  clip_id       INTEGER REFERENCES clips(id) ON DELETE SET NULL,
  name          TEXT    NOT NULL DEFAULT '',
  updated_at_ms INTEGER,
  legacy_id     INTEGER
) STRICT;

CREATE TABLE settings (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  revision      INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE migration_state (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  source_path     TEXT    NOT NULL,
  source_sha256   BLOB    NOT NULL,
  dry_run         INTEGER NOT NULL DEFAULT 0 CHECK (dry_run IN (0,1)),
  phase           TEXT    NOT NULL,
  started_at_ms   INTEGER NOT NULL,
  updated_at_ms   INTEGER NOT NULL,
  completed_at_ms INTEGER,
  counts_json     TEXT,
  error           TEXT
) STRICT;

CREATE VIRTUAL TABLE clips_fts USING fts5(
  preview_text,
  quick_paste_text,
  text_content,
  description,
  content = 'clips',
  content_rowid = 'id',
  tokenize = 'trigram'
);

CREATE TRIGGER clips_ai AFTER INSERT ON clips WHEN new.is_deleted = 0 BEGIN
  INSERT INTO clips_fts(rowid, preview_text, quick_paste_text, text_content, description)
  VALUES (
    new.id,
    new.preview_text,
    new.quick_paste_text,
    coalesce(new.text_content, ''),
    new.description
  );
END;

CREATE TRIGGER clips_ad AFTER DELETE ON clips WHEN old.is_deleted = 0 BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, preview_text, quick_paste_text, text_content, description)
  VALUES (
    'delete',
    old.id,
    old.preview_text,
    old.quick_paste_text,
    coalesce(old.text_content, ''),
    old.description
  );
END;

CREATE TRIGGER clips_au AFTER UPDATE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, preview_text, quick_paste_text, text_content, description)
  VALUES (
    'delete',
    old.id,
    old.preview_text,
    old.quick_paste_text,
    coalesce(old.text_content, ''),
    old.description
  );
  INSERT INTO clips_fts(rowid, preview_text, quick_paste_text, text_content, description)
  SELECT new.id, new.preview_text, new.quick_paste_text,
         coalesce(new.text_content, ''), new.description
  WHERE new.is_deleted = 0;
END;
"#;

pub fn configure_connection(connection: &Connection, read_only: bool) -> StorageResult<()> {
    connection.busy_timeout(std::time::Duration::from_millis(5_000))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    connection.pragma_update(None, "wal_autocheckpoint", 1_000)?;
    register_sql_functions(connection)?;
    if read_only {
        connection.pragma_update(None, "query_only", "ON")?;
    } else {
        connection.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
        let journal_mode: String =
            connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            connection.pragma_update(None, "journal_mode", "WAL")?;
        }
        connection.pragma_update(None, "synchronous", "NORMAL")?;
    }
    Ok(())
}

pub fn migrate_down_to_zero(connection: &mut Connection) -> StorageResult<u32> {
    let current: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))?;
    if current == 0 {
        return Ok(0);
    }
    if current != 1 {
        return Err(StorageError::Migration(format!(
            "no down migration for schema {current}"
        )));
    }
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "DROP TRIGGER IF EXISTS clips_ai;
         DROP TRIGGER IF EXISTS clips_ad;
         DROP TRIGGER IF EXISTS clips_au;
         DROP TABLE IF EXISTS clips_fts;
         DROP TABLE IF EXISTS migration_state;
         DROP TABLE IF EXISTS settings;
         DROP TABLE IF EXISTS copy_buffers;
         DROP TABLE IF EXISTS clip_formats;
         DROP TABLE IF EXISTS clips;
         DROP TABLE IF EXISTS groups;
         DROP TABLE IF EXISTS meta;
         DROP TABLE IF EXISTS schema_version;",
    )?;
    transaction.pragma_update(None, "user_version", 0_u32)?;
    transaction.commit()?;
    Ok(0)
}

pub fn verify_fts5(connection: &Connection) -> StorageResult<()> {
    let mut statement = connection.prepare("PRAGMA compile_options")?;
    let options = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !options.iter().any(|option| option == "ENABLE_FTS5") {
        return Err(StorageError::Migration(
            "bundled SQLite does not include ENABLE_FTS5".to_string(),
        ));
    }
    connection.execute_batch(
        "DROP TABLE IF EXISTS temp.fts5_probe;
         CREATE VIRTUAL TABLE temp.fts5_probe USING fts5(text, tokenize = 'trigram');
         DROP TABLE temp.fts5_probe;",
    )?;
    Ok(())
}

pub fn migrate(connection: &mut Connection, backup_path: Option<&Path>) -> StorageResult<u32> {
    verify_fts5(connection)?;
    let current: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))?;
    let target = ditto_model::SCHEMA_VERSION;
    if current > target {
        return Err(StorageError::Migration(format!(
            "database schema {current} is newer than supported schema {target}"
        )));
    }
    if current == target {
        return Ok(current);
    }
    if current > 0 {
        let Some(path) = backup_path else {
            return Err(StorageError::Migration(
                "pre-migration backup path is required".to_string(),
            ));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        connection.backup(rusqlite::MAIN_DB, path, None)?;
        tracing::info!(path = %path.display(), from = current, to = target, "created pre-migration backup");
    }

    let transaction = connection.transaction()?;
    if current == 0 {
        apply_v1(&transaction)?;
    }
    transaction.pragma_update(None, "user_version", target)?;
    transaction.commit()?;
    Ok(target)
}

fn apply_v1(transaction: &Transaction<'_>) -> StorageResult<()> {
    transaction.execute_batch(SCHEMA_V1)?;
    let now = now_ms();
    transaction.execute(
        "INSERT INTO schema_version(version, applied_at_ms, app_version) VALUES (?1, ?2, ?3)",
        rusqlite::params![ditto_model::SCHEMA_VERSION, now, APP_VERSION],
    )?;
    for (key, value) in [
        ("state_rev", "0"),
        ("theme_rev", "0"),
        ("settings_rev", "0"),
        ("install_id", ""),
        ("last_vacuum_ms", "0"),
    ] {
        transaction.execute(
            "INSERT INTO meta(key, value, updated_at_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, value, now],
        )?;
    }
    for slot in 1..=5 {
        transaction.execute(
            "INSERT INTO copy_buffers(slot, clip_id, name, updated_at_ms) VALUES (?1, NULL, '', ?2)",
            rusqlite::params![slot, now],
        )?;
    }
    Ok(())
}

const MAX_REGEX_PATTERN_BYTES: usize = 1_024;
const REGEX_SIZE_LIMIT: usize = 1 << 20;
const REGEX_CACHE_ENTRIES: usize = 32;

static REGEX_CACHE: OnceLock<Mutex<HashMap<String, Arc<Regex>>>> = OnceLock::new();

fn register_sql_functions(connection: &Connection) -> StorageResult<()> {
    connection.create_scalar_function(
        "regexp",
        2,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            let pattern: String = context.get(0)?;
            let value: Option<String> = context.get(1)?;
            let Some(value) = value else {
                return Ok(false);
            };
            let regex = cached_regex(&pattern)?;
            Ok(regex.is_match(&value))
        },
    )?;
    Ok(())
}

fn cached_regex(pattern: &str) -> rusqlite::Result<Arc<Regex>> {
    let cache = REGEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(regex) = cache
        .lock()
        .expect("regex cache poisoned")
        .get(pattern)
        .cloned()
    {
        return Ok(regex);
    }
    let regex = Arc::new(
        build_regex(pattern)
            .map_err(|error| rusqlite::Error::UserFunctionError(Box::new(error)))?,
    );
    let mut cache = cache.lock().expect("regex cache poisoned");
    if cache.len() >= REGEX_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(pattern.to_string(), Arc::clone(&regex));
    Ok(regex)
}

fn build_regex(pattern: &str) -> Result<Regex, std::io::Error> {
    if pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "regex pattern is longer than 1024 bytes",
        ));
    }
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string()))
}

/// Validates a user regex before it reaches SQL. The regex engine is linear
/// time; the size and pattern limits bound compile-time memory.
pub(crate) fn validate_regex(pattern: &str) -> StorageResult<()> {
    build_regex(pattern)
        .map(|_| ())
        .map_err(|error| StorageError::InvalidData(format!("invalid regex: {error}")))
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_schema_is_wal_fts5_and_idempotent() {
        let mut connection = Connection::open_in_memory().expect("open database");
        configure_connection(&connection, false).expect("configure");
        assert_eq!(migrate(&mut connection, None).expect("migrate"), 1);
        assert_eq!(migrate(&mut connection, None).expect("migrate again"), 1);
        let fts_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'clips_fts'",
                [],
                |row| row.get(0),
            )
            .expect("read FTS schema");
        assert!(fts_sql.contains("trigram"));
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .expect("user version"),
            1
        );
    }

    #[test]
    fn v1_down_migration_returns_to_an_empty_database() {
        let mut connection = Connection::open_in_memory().expect("open database");
        configure_connection(&connection, false).expect("configure");
        migrate(&mut connection, None).expect("migrate");
        assert_eq!(
            migrate_down_to_zero(&mut connection).expect("down migrate"),
            0
        );
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .expect("user version"),
            0
        );
        assert_eq!(migrate(&mut connection, None).expect("remigrate"), 1);
    }
}
