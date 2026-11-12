use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use rusqlite::{Connection, OpenFlags, Row, types::ValueRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{MigrationError, MigrationResult, types::SourceCounts};

const STICKY_SENTINEL: i64 = -2_147_483_647;
const MAX_GROUP_DEPTH: usize = 64;
const MAX_DB_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFingerprint {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct LegacyGroup {
    pub legacy_id: i64,
    pub name: String,
    pub parent_legacy_id: Option<i64>,
    pub date_seconds: i64,
    pub clip_order: Option<f64>,
    pub clip_group_order: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct LegacyClipRow {
    pub legacy_id: i64,
    pub date_seconds: i64,
    pub text: String,
    pub shortcut: Option<i64>,
    pub dont_auto_delete: bool,
    pub crc: Option<i64>,
    pub parent_legacy_id: Option<i64>,
    pub quick_paste_text: String,
    pub clip_order: Option<f64>,
    pub clip_group_order: Option<f64>,
    pub global_shortcut: bool,
    pub last_paste_date_seconds: Option<i64>,
    pub sticky_order: Option<f64>,
    pub move_to_group_shortcut: Option<i64>,
    pub global_move_to_group_shortcut: bool,
}

#[derive(Debug, Clone)]
pub struct LegacyDataRow {
    pub legacy_data_id: i64,
    pub parent_legacy_id: i64,
    pub format_name: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LegacyCopyBuffer {
    pub legacy_id: i64,
    pub clip_legacy_id: Option<i64>,
    pub new_slot: u8,
}

#[derive(Debug, Clone)]
pub struct LegacyCatalog {
    pub fingerprint: SourceFingerprint,
    pub counts: SourceCounts,
    pub groups: Vec<LegacyGroup>,
    pub copy_buffers: Vec<LegacyCopyBuffer>,
    pub types: Vec<String>,
    pub format_distribution: BTreeMap<String, u64>,
    pub copy_buffer_slots: Vec<u8>,
    pub estimated_blob_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LegacySource {
    pub database: PathBuf,
    pub settings: Option<PathBuf>,
}

impl LegacySource {
    pub fn new(database: impl Into<PathBuf>, settings: Option<PathBuf>) -> Self {
        Self {
            database: database.into(),
            settings,
        }
    }

    pub fn catalog(&self) -> MigrationResult<LegacyCatalog> {
        let fingerprint = fingerprint(&self.database)?;
        if fingerprint.size_bytes > MAX_DB_BYTES {
            return Err(MigrationError::Invalid(format!(
                "legacy database is {} bytes; limit is {MAX_DB_BYTES}",
                fingerprint.size_bytes
            )));
        }
        let connection = open_read_only(&self.database)?;
        validate_schema(&connection)?;
        let quick_check: String =
            connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if !quick_check.eq_ignore_ascii_case("ok") {
            return Err(MigrationError::Invalid(format!(
                "legacy database quick_check failed: {quick_check}"
            )));
        }
        let mut warnings = Vec::new();
        let groups = read_groups(&connection, &mut warnings)?;
        let counts = read_counts(&connection)?;
        let copy_buffers = read_copy_buffers(&connection, &mut warnings)?;
        let types = read_types(&connection)?;
        let (format_distribution, estimated_blob_bytes) = read_format_estimate(&connection)?;
        let copy_buffer_slots = copy_buffers
            .iter()
            .filter(|buffer| buffer.clip_legacy_id.is_some())
            .map(|buffer| buffer.new_slot)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(LegacyCatalog {
            fingerprint,
            counts,
            groups,
            copy_buffers,
            types,
            format_distribution,
            copy_buffer_slots,
            estimated_blob_bytes,
            warnings,
        })
    }

    pub fn clip_batch(
        &self,
        after_legacy_id: i64,
        limit: u32,
    ) -> MigrationResult<Vec<LegacyClipRow>> {
        let connection = open_read_only(&self.database)?;
        let mut statement = connection.prepare(
            "SELECT * FROM Main
             WHERE coalesce(bIsGroup, 0) = 0 AND lID > ?1
             ORDER BY lID
             LIMIT ?2",
        )?;
        let column_indexes = column_indexes(&statement);
        let mut rows = statement.query(rusqlite::params![after_legacy_id, i64::from(limit)])?;
        let mut output = Vec::new();
        while let Some(row) = rows.next()? {
            output.push(clip_from_row(row, &column_indexes)?);
        }
        Ok(output)
    }

    pub fn formats_for_clips(
        &self,
        legacy_ids: &[i64],
    ) -> MigrationResult<BTreeMap<i64, Vec<LegacyDataRow>>> {
        if legacy_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let connection = open_read_only(&self.database)?;
        let placeholders = std::iter::repeat_n("?", legacy_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT lID, lParentID, strClipBoardFormat, ooData
             FROM Data WHERE lParentID IN ({placeholders}) ORDER BY lID"
        );
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(rusqlite::params_from_iter(legacy_ids))?;
        let mut output = BTreeMap::<i64, Vec<LegacyDataRow>>::new();
        while let Some(row) = rows.next()? {
            let parent_legacy_id: i64 = row.get(1)?;
            let data = match row.get_ref(3)? {
                ValueRef::Blob(bytes) => bytes.to_vec(),
                ValueRef::Text(bytes) => bytes.to_vec(),
                ValueRef::Null | ValueRef::Integer(_) | ValueRef::Real(_) => Vec::new(),
            };
            output
                .entry(parent_legacy_id)
                .or_default()
                .push(LegacyDataRow {
                    legacy_data_id: row.get(0)?,
                    parent_legacy_id,
                    format_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    data,
                });
        }
        Ok(output)
    }

    pub fn clip_by_legacy_id(&self, legacy_id: i64) -> MigrationResult<Option<LegacyClipRow>> {
        let connection = open_read_only(&self.database)?;
        let mut statement = connection
            .prepare("SELECT * FROM Main WHERE lID = ?1 AND coalesce(bIsGroup, 0) = 0")?;
        let indexes = column_indexes(&statement);
        let mut rows = statement.query([legacy_id])?;
        rows.next()?
            .map(|row| clip_from_row(row, &indexes))
            .transpose()
    }

    pub fn sample_clip_ids(&self, limit: usize) -> MigrationResult<Vec<i64>> {
        let connection = open_read_only(&self.database)?;
        let mut statement = connection.prepare(
            "SELECT lID FROM Main WHERE coalesce(bIsGroup, 0) = 0
             ORDER BY lID LIMIT ?1",
        )?;
        let mut rows = statement.query([limit as i64])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(row.get(0)?);
        }
        Ok(ids)
    }
}

pub fn fingerprint(path: &Path) -> MigrationResult<SourceFingerprint> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(MigrationError::Invalid(format!(
            "{} is not a file",
            path.display()
        )));
    }
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);
    Ok(SourceFingerprint {
        path: path.display().to_string(),
        sha256: hex::encode(digest.finalize()),
        size_bytes: metadata.len(),
        modified_ms,
    })
}

fn open_read_only(path: &Path) -> MigrationResult<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(connection)
}

fn validate_schema(connection: &Connection) -> MigrationResult<()> {
    for table in ["Main", "Data"] {
        if !table_exists(connection, table)? {
            return Err(MigrationError::Invalid(format!(
                "legacy database is missing table {table}"
            )));
        }
    }
    let main_columns = table_columns(connection, "Main")?;
    for column in ["lID", "lDate", "bIsGroup", "lParentID"] {
        if !main_columns.contains(column) {
            return Err(MigrationError::Invalid(format!(
                "legacy Main table is missing required column {column}"
            )));
        }
    }
    Ok(())
}

fn table_columns(connection: &Connection, table: &str) -> MigrationResult<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    let mut columns = BTreeSet::new();
    while let Some(row) = rows.next()? {
        columns.insert(row.get::<_, String>(1)?);
    }
    Ok(columns)
}

fn table_exists(connection: &Connection, table: &str) -> MigrationResult<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn read_counts(connection: &Connection) -> MigrationResult<SourceCounts> {
    let clips = scalar_u64(
        connection,
        "SELECT COUNT(*) FROM Main WHERE coalesce(bIsGroup, 0) = 0",
    )?;
    let groups = scalar_u64(
        connection,
        "SELECT COUNT(*) FROM Main WHERE coalesce(bIsGroup, 0) = 1",
    )?;
    let formats = scalar_u64(connection, "SELECT COUNT(*) FROM Data")?;
    let empty_formats = scalar_u64(
        connection,
        "SELECT COUNT(*) FROM Data WHERE ooData IS NULL OR length(ooData) = 0",
    )?;
    let copy_buffers = if table_exists(connection, "CopyBuffers")? {
        scalar_u64(
            connection,
            "SELECT COUNT(DISTINCT lCopyBuffer) FROM CopyBuffers
             WHERE coalesce(lClipID, -1) >= 0 AND lCopyBuffer BETWEEN 0 AND 4",
        )?
    } else {
        0
    };
    let types = if table_exists(connection, "Types")? {
        scalar_u64(connection, "SELECT COUNT(*) FROM Types")?
    } else {
        0
    };
    Ok(SourceCounts {
        clips,
        groups,
        formats,
        copy_buffers,
        types,
        empty_formats,
    })
}

fn scalar_u64(connection: &Connection, sql: &str) -> MigrationResult<u64> {
    let value: i64 = connection.query_row(sql, [], |row| row.get(0))?;
    Ok(value.max(0) as u64)
}

fn read_groups(
    connection: &Connection,
    warnings: &mut Vec<String>,
) -> MigrationResult<Vec<LegacyGroup>> {
    let mut statement =
        connection.prepare("SELECT * FROM Main WHERE coalesce(bIsGroup, 0) = 1 ORDER BY lID")?;
    let indexes = column_indexes(&statement);
    let mut rows = statement.query([])?;
    let mut groups = Vec::new();
    while let Some(row) = rows.next()? {
        groups.push(LegacyGroup {
            legacy_id: required_i64(row, &indexes, "lID")?,
            name: text_or_default(row, &indexes, "mText"),
            parent_legacy_id: parent_id(opt_i64(row, &indexes, "lParentID")),
            date_seconds: opt_i64(row, &indexes, "lDate").unwrap_or_default(),
            clip_order: opt_f64(row, &indexes, "clipOrder"),
            clip_group_order: opt_f64(row, &indexes, "clipGroupOrder"),
        });
    }
    sanitize_groups(&mut groups, warnings);
    Ok(groups)
}

fn sanitize_groups(groups: &mut [LegacyGroup], warnings: &mut Vec<String>) {
    let ids = groups
        .iter()
        .map(|group| group.legacy_id)
        .collect::<BTreeSet<_>>();
    let parent_by_id = groups
        .iter()
        .map(|group| (group.legacy_id, group.parent_legacy_id))
        .collect::<BTreeMap<_, _>>();
    for group in groups {
        let mut seen = BTreeSet::new();
        let mut current = group.legacy_id;
        let mut depth = 0_usize;
        let mut invalid = false;
        while let Some(parent) = parent_by_id.get(&current).copied().flatten() {
            if !ids.contains(&parent) {
                invalid = true;
                warnings.push(format!(
                    "group {} references missing parent {}; attached to root",
                    group.legacy_id, parent
                ));
                break;
            }
            if !seen.insert(parent) || parent == group.legacy_id || depth >= MAX_GROUP_DEPTH {
                invalid = true;
                warnings.push(format!(
                    "group {} has a parent cycle or exceeds depth {}; attached to root",
                    group.legacy_id, MAX_GROUP_DEPTH
                ));
                break;
            }
            current = parent;
            depth += 1;
        }
        if invalid {
            group.parent_legacy_id = None;
        }
    }
}

fn read_copy_buffers(
    connection: &Connection,
    warnings: &mut Vec<String>,
) -> MigrationResult<Vec<LegacyCopyBuffer>> {
    if !table_exists(connection, "CopyBuffers")? {
        return Ok(Vec::new());
    }
    let columns = table_columns(connection, "CopyBuffers")?;
    if !columns.contains("lCopyBuffer") || !columns.contains("lClipID") {
        return Ok(Vec::new());
    }
    let mut statement =
        connection.prepare("SELECT * FROM CopyBuffers ORDER BY coalesce(lID, 0), lCopyBuffer")?;
    let indexes = column_indexes(&statement);
    let mut rows = statement.query([])?;
    let mut latest = BTreeMap::<u8, LegacyCopyBuffer>::new();
    while let Some(row) = rows.next()? {
        let Some(slot) = opt_i64(row, &indexes, "lCopyBuffer")
            .and_then(|slot| u8::try_from(slot).ok())
            .filter(|slot| *slot <= 4)
        else {
            continue;
        };
        let clip_id = opt_i64(row, &indexes, "lClipID").unwrap_or(-1);
        let replaced = latest.insert(
            slot,
            LegacyCopyBuffer {
                legacy_id: opt_i64(row, &indexes, "lID").unwrap_or_default(),
                clip_legacy_id: (clip_id >= 0).then_some(clip_id),
                new_slot: slot + 1,
            },
        );
        if replaced.is_some() {
            warnings.push(format!(
                "copy buffer slot {} had multiple legacy rows; kept the latest lID",
                slot + 1
            ));
        }
    }
    Ok(latest.into_values().collect())
}

fn read_types(connection: &Connection) -> MigrationResult<Vec<String>> {
    if !table_exists(connection, "Types")? {
        return Ok(Vec::new());
    }
    let columns = table_columns(connection, "Types")?;
    if !columns.contains("TypeText") {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare("SELECT TypeText FROM Types ORDER BY lID")?;
    let mut rows = statement.query([])?;
    let mut types = Vec::new();
    while let Some(row) = rows.next()? {
        let value = row.get::<_, Option<String>>(0)?.unwrap_or_default();
        if !value.trim().is_empty() {
            types.push(value);
        }
    }
    types.sort();
    types.dedup();
    Ok(types)
}

fn read_format_estimate(connection: &Connection) -> MigrationResult<(BTreeMap<String, u64>, u64)> {
    let mut statement = connection.prepare(
        "SELECT strClipBoardFormat, COUNT(*), coalesce(SUM(length(ooData)), 0)
         FROM Data GROUP BY strClipBoardFormat ORDER BY strClipBoardFormat",
    )?;
    let mut rows = statement.query([])?;
    let mut distribution = BTreeMap::new();
    let mut bytes = 0_u64;
    while let Some(row) = rows.next()? {
        let name = row
            .get::<_, Option<String>>(0)?
            .unwrap_or_else(|| "custom".into());
        let count: i64 = row.get(1)?;
        let size: i64 = row.get(2)?;
        distribution.insert(name, count.max(0) as u64);
        bytes = bytes.saturating_add(size.max(0) as u64);
    }
    Ok((distribution, bytes))
}

fn clip_from_row(
    row: &Row<'_>,
    indexes: &HashMap<String, usize>,
) -> MigrationResult<LegacyClipRow> {
    let parent_legacy_id = parent_id(opt_i64(row, indexes, "lParentID"));
    let sticky = if parent_legacy_id.is_none() {
        opt_f64(row, indexes, "stickyClipOrder")
    } else {
        opt_f64(row, indexes, "stickyClipGroupOrder")
    };
    let sticky_other = if parent_legacy_id.is_none() {
        opt_f64(row, indexes, "stickyClipGroupOrder")
    } else {
        opt_f64(row, indexes, "stickyClipOrder")
    };
    let sticky = sticky
        .filter(|value| *value != STICKY_SENTINEL as f64 && *value != 0.0)
        .or_else(|| sticky_other.filter(|value| *value != STICKY_SENTINEL as f64 && *value != 0.0));
    Ok(LegacyClipRow {
        legacy_id: required_i64(row, indexes, "lID")?,
        date_seconds: opt_i64(row, indexes, "lDate").unwrap_or_default(),
        text: text_or_default(row, indexes, "mText"),
        shortcut: opt_i64(row, indexes, "lShortCut").filter(|value| *value > 0),
        dont_auto_delete: bool_i64(row, indexes, "lDontAutoDelete"),
        crc: opt_i64(row, indexes, "CRC"),
        parent_legacy_id,
        quick_paste_text: text_or_default(row, indexes, "QuickPasteText"),
        clip_order: opt_f64(row, indexes, "clipOrder"),
        clip_group_order: opt_f64(row, indexes, "clipGroupOrder"),
        global_shortcut: bool_i64(row, indexes, "globalShortCut"),
        last_paste_date_seconds: opt_i64(row, indexes, "lastPasteDate").filter(|value| *value > 0),
        sticky_order: sticky,
        move_to_group_shortcut: opt_i64(row, indexes, "MoveToGroupShortCut")
            .filter(|value| *value > 0),
        global_move_to_group_shortcut: bool_i64(row, indexes, "GlobalMoveToGroupShortCut"),
    })
}

fn column_indexes(statement: &rusqlite::Statement<'_>) -> HashMap<String, usize> {
    statement
        .column_names()
        .iter()
        .enumerate()
        .map(|(index, name)| ((*name).to_string(), index))
        .collect()
}

fn required_i64(
    row: &Row<'_>,
    indexes: &HashMap<String, usize>,
    name: &str,
) -> MigrationResult<i64> {
    opt_i64(row, indexes, name).ok_or_else(|| {
        MigrationError::Invalid(format!("legacy row is missing required integer {name}"))
    })
}

fn opt_i64(row: &Row<'_>, indexes: &HashMap<String, usize>, name: &str) -> Option<i64> {
    let index = *indexes.get(name)?;
    match row.get_ref(index).ok()? {
        ValueRef::Integer(value) => Some(value),
        ValueRef::Real(value) => Some(value as i64),
        ValueRef::Text(value) => std::str::from_utf8(value).ok()?.trim().parse().ok(),
        ValueRef::Null | ValueRef::Blob(_) => None,
    }
}

fn opt_f64(row: &Row<'_>, indexes: &HashMap<String, usize>, name: &str) -> Option<f64> {
    let index = *indexes.get(name)?;
    match row.get_ref(index).ok()? {
        ValueRef::Integer(value) => Some(value as f64),
        ValueRef::Real(value) => Some(value),
        ValueRef::Text(value) => std::str::from_utf8(value).ok()?.trim().parse().ok(),
        ValueRef::Null | ValueRef::Blob(_) => None,
    }
}

fn text_or_default(row: &Row<'_>, indexes: &HashMap<String, usize>, name: &str) -> String {
    let Some(index) = indexes.get(name) else {
        return String::new();
    };
    row.get::<_, Option<String>>(*index)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn bool_i64(row: &Row<'_>, indexes: &HashMap<String, usize>, name: &str) -> bool {
    opt_i64(row, indexes, name).is_some_and(|value| value != 0)
}

fn parent_id(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_parent_is_attached_to_root_without_losing_group() {
        let mut groups = vec![LegacyGroup {
            legacy_id: 2,
            name: "orphan".into(),
            parent_legacy_id: Some(99),
            date_seconds: 1,
            clip_order: None,
            clip_group_order: None,
        }];
        let mut warnings = Vec::new();
        sanitize_groups(&mut groups, &mut warnings);
        assert_eq!(groups[0].parent_legacy_id, None);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("fixture.db");
        fs::write(&path, b"one").expect("write");
        let first = fingerprint(&path).expect("first");
        fs::write(&path, b"two").expect("write");
        let second = fingerprint(&path).expect("second");
        assert_ne!(first.sha256, second.sha256);
    }

    #[test]
    fn readonly_open_rejects_missing_tables() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("empty.db");
        Connection::open(&path).expect("create");
        let error = LegacySource::new(&path, None)
            .catalog()
            .expect_err("missing schema");
        assert!(error.to_string().contains("missing table"));
    }
}
