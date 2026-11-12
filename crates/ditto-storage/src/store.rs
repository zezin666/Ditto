use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use ditto_model::{
    ClipDetail, ClipFormatInfo, ClipKind, ClipPage, ClipQuery, ClipSort, ClipSummary,
    CopyBufferSlot, GroupNode, NewClip, SearchMode,
};
use parking_lot::{Condvar, Mutex};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, types::Value as SqlValue};
use serde::{Deserialize, Serialize};

use crate::{
    StorageError, StorageResult,
    blob::{self, collect_files_recursive},
    schema::{configure_connection, migrate, now_ms},
};

const FORMAT_SEPARATOR: char = '\u{1f}';

#[derive(Debug, Clone)]
pub struct InsertOutcome {
    pub clip: ClipSummary,
    pub inserted: bool,
}

#[derive(Debug)]
pub struct Storage {
    pub(crate) writer: Arc<Mutex<Connection>>,
    readers: Arc<ReaderPool>,
    data_root: PathBuf,
    database_path: PathBuf,
    schema_version: u32,
}

impl Clone for Storage {
    fn clone(&self) -> Self {
        Self {
            writer: Arc::clone(&self.writer),
            readers: Arc::clone(&self.readers),
            data_root: self.data_root.clone(),
            database_path: self.database_path.clone(),
            schema_version: self.schema_version,
        }
    }
}

impl Storage {
    /// Opens an explicit database path. The parent directory is the data root.
    pub fn open(path: &Path) -> StorageResult<Self> {
        let data_root = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self::open_paths(data_root, path.to_path_buf())
    }

    pub fn open_data_root(data_root: &Path) -> StorageResult<Self> {
        fs::create_dir_all(data_root)?;
        Self::open_paths(data_root.to_path_buf(), data_root.join("ditto.db"))
    }

    fn open_paths(data_root: PathBuf, database_path: PathBuf) -> StorageResult<Self> {
        fs::create_dir_all(&data_root)?;
        fs::create_dir_all(data_root.join("blobs"))?;
        fs::create_dir_all(data_root.join("thumbs"))?;
        fs::create_dir_all(data_root.join("backups"))?;
        let mut connection = Connection::open(&database_path)?;
        configure_connection(&connection, false)?;
        let backup_path = data_root
            .join("backups")
            .join(format!("ditto.{}.db", now_ms()));
        let schema_version = migrate(&mut connection, Some(&backup_path))?;
        let mut readers = Vec::with_capacity(4);
        for _ in 0..4 {
            let reader = Connection::open_with_flags(
                &database_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            )?;
            configure_connection(&reader, true)?;
            readers.push(reader);
        }
        Ok(Self {
            writer: Arc::new(Mutex::new(connection)),
            readers: Arc::new(ReaderPool {
                connections: Mutex::new(readers),
                available: Condvar::new(),
            }),
            data_root,
            database_path,
            schema_version,
        })
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn user_version(&self) -> StorageResult<u32> {
        let connection = self.writer.lock();
        Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn state_rev(&self) -> StorageResult<u64> {
        self.meta_u64("state_rev")
    }

    pub fn settings_rev(&self) -> StorageResult<u64> {
        self.meta_u64("settings_rev")
    }

    pub(crate) fn meta_u64(&self, key: &str) -> StorageResult<u64> {
        let connection = self.reader()?;
        let value: Option<String> = connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(value
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default())
    }

    pub(crate) fn reader(&self) -> StorageResult<ReaderGuard> {
        Ok(self.readers.acquire())
    }

    pub fn insert_clip(
        &self,
        new: &NewClip,
        allow_duplicates: bool,
    ) -> StorageResult<InsertOutcome> {
        validate_new_clip(new)?;
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let existing_id = if allow_duplicates {
            None
        } else {
            transaction
                .query_row(
                    "SELECT id FROM clips
                     WHERE content_hash = ?1 AND formats_hash = ?2 AND is_deleted = 0
                     ORDER BY id DESC LIMIT 1",
                    rusqlite::params![new.content_hash, new.formats_hash],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
        };
        if let Some(existing_id) = existing_id {
            transaction.execute(
                "UPDATE clips
                 SET updated_at_ms = ?1, sort_order = ?2, group_sort_order = ?3,
                     group_id = coalesce(?4, group_id)
                 WHERE id = ?5",
                rusqlite::params![
                    new.created_at_ms,
                    new.sort_order,
                    new.group_sort_order,
                    new.group_id,
                    existing_id
                ],
            )?;
            bump_state(&transaction)?;
            transaction.commit()?;
            return Ok(InsertOutcome {
                clip: self.get_summary(existing_id)?,
                inserted: false,
            });
        }

        let thumb_path = match &new.thumbnail_png {
            Some(bytes) => Some(blob::write_thumbnail(
                &self.data_root,
                &new.content_hash,
                bytes,
            )?),
            None => None,
        };
        transaction.execute(
            "INSERT INTO clips(
               kind, primary_format, text_content, text_truncated, preview_text,
               quick_paste_text, description, source_app, source_window_title,
               content_hash, formats_hash, hash_algo, size_bytes, is_pinned,
               pin_order, group_id, sort_order, group_sort_order, created_at_ms,
               updated_at_ms, dont_auto_delete, has_image, thumb_path
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
               ?14, ?15, ?16, ?17, ?18, ?19, ?19, ?20, ?21, ?22
             )",
            rusqlite::params![
                new.kind.as_db_str(),
                new.primary_format,
                truncate_utf8(new.text_content.as_deref().unwrap_or_default(), 1024 * 1024),
                new.text_truncated as i64,
                truncate_utf8(&new.preview_text, 8 * 1024),
                truncate_utf8(&new.quick_paste_text, 8 * 1024),
                truncate_utf8(&new.description, 8 * 1024),
                new.source_app,
                new.source_window_title,
                new.content_hash,
                new.formats_hash,
                new.hash_algo,
                new.size_bytes as i64,
                new.is_pinned as i64,
                new.pin_order,
                new.group_id,
                new.sort_order,
                new.group_sort_order,
                new.created_at_ms,
                new.dont_auto_delete as i64,
                new.has_image as i64,
                thumb_path,
            ],
        )?;
        let clip_id = transaction.last_insert_rowid();
        for format in &new.formats {
            store_format(
                &transaction,
                &self.data_root,
                clip_id,
                format,
                new.primary_format.as_deref(),
            )?;
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(InsertOutcome {
            clip: self.get_summary(clip_id)?,
            inserted: true,
        })
    }

    pub fn insert_many(&self, clips: &[NewClip]) -> StorageResult<u64> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let mut inserted = 0_u64;
        for new in clips {
            validate_new_clip(new)?;
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM clips
                   WHERE content_hash = ?1 AND formats_hash = ?2 AND is_deleted = 0
                 )",
                rusqlite::params![new.content_hash, new.formats_hash],
                |row| row.get(0),
            )?;
            if exists {
                continue;
            }
            let thumb_path = match &new.thumbnail_png {
                Some(bytes) => Some(blob::write_thumbnail(
                    &self.data_root,
                    &new.content_hash,
                    bytes,
                )?),
                None => None,
            };
            transaction.execute(
                "INSERT INTO clips(
                   kind, primary_format, text_content, text_truncated, preview_text,
                   quick_paste_text, description, source_app, source_window_title,
                   content_hash, formats_hash, hash_algo, size_bytes, is_pinned,
                   pin_order, group_id, sort_order, group_sort_order, created_at_ms,
                   updated_at_ms, dont_auto_delete, has_image, thumb_path
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18, ?19, ?19, ?20, ?21, ?22
                 )",
                rusqlite::params![
                    new.kind.as_db_str(),
                    new.primary_format,
                    truncate_utf8(new.text_content.as_deref().unwrap_or_default(), 1024 * 1024,),
                    new.text_truncated as i64,
                    truncate_utf8(&new.preview_text, 8 * 1024),
                    truncate_utf8(&new.quick_paste_text, 8 * 1024),
                    truncate_utf8(&new.description, 8 * 1024),
                    new.source_app,
                    new.source_window_title,
                    new.content_hash,
                    new.formats_hash,
                    new.hash_algo,
                    new.size_bytes as i64,
                    new.is_pinned as i64,
                    new.pin_order,
                    new.group_id,
                    new.sort_order,
                    new.group_sort_order,
                    new.created_at_ms,
                    new.dont_auto_delete as i64,
                    new.has_image as i64,
                    thumb_path,
                ],
            )?;
            let clip_id = transaction.last_insert_rowid();
            for format in &new.formats {
                store_format(
                    &transaction,
                    &self.data_root,
                    clip_id,
                    format,
                    new.primary_format.as_deref(),
                )?;
            }
            inserted += 1;
        }
        bump_state_by(&transaction, inserted)?;
        transaction.commit()?;
        Ok(inserted)
    }

    pub fn get_summary(&self, clip_id: i64) -> StorageResult<ClipSummary> {
        let state_rev = self.state_rev()?;
        let connection = self.reader()?;
        let sql = format!(
            "SELECT {} FROM clips c WHERE c.id = ?1 AND c.is_deleted = 0",
            summary_columns()
        );
        let mut statement = connection.prepare(&sql)?;
        let result = statement
            .query_row([clip_id], |row| summary_from_row(row, state_rev))
            .optional()?;
        result.ok_or_else(|| StorageError::NotFound(format!("clip {clip_id}")))
    }

    pub fn get_clip(&self, clip_id: i64) -> StorageResult<ClipDetail> {
        let summary = self.get_summary(clip_id)?;
        let connection = self.reader()?;
        let (text_content, quick_paste_text, description, source_window_title, updated_at_ms) =
            connection
                .query_row(
                    "SELECT text_content, quick_paste_text, description,
                            source_window_title, updated_at_ms
                     FROM clips WHERE id = ?1 AND is_deleted = 0",
                    [clip_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| StorageError::NotFound(format!("clip {clip_id}")))?;
        let mut statement = connection.prepare(
            "SELECT format_name, seq, size_bytes, mime, sha256, data, data_path, is_primary
             FROM clip_formats WHERE clip_id = ?1 ORDER BY seq, id",
        )?;
        let formats = statement
            .query_map([clip_id], |row| {
                Ok(ClipFormatInfo {
                    format_name: row.get(0)?,
                    seq: row.get::<_, i64>(1)? as u32,
                    size_bytes: row.get::<_, i64>(2)? as u64,
                    mime: row.get(3)?,
                    sha256: row.get::<_, Option<Vec<u8>>>(4)?.unwrap_or_default(),
                    data: row.get(5)?,
                    data_path: row.get(6)?,
                    is_primary: row.get::<_, i64>(7)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ClipDetail {
            dont_auto_delete: self.clip_flag(clip_id, "dont_auto_delete")?,
            summary,
            text_content,
            quick_paste_text,
            description,
            source_window_title,
            formats,
            updated_at_ms,
        })
    }

    pub fn search_clips(&self, query: &ClipQuery) -> StorageResult<ClipPage> {
        let started = Instant::now();
        if query.mode == SearchMode::Regex {
            return Err(StorageError::InvalidData(
                "regex search is deferred beyond T2".to_string(),
            ));
        }
        validate_search_mode(query.mode)?;
        let state_rev = self.state_rev()?;
        let connection = self.reader()?;
        let mut clauses = vec!["c.is_deleted = 0".to_string()];
        let mut params = Vec::<SqlValue>::new();
        append_group_filter(&mut clauses, &mut params, query.group_id);
        append_kind_pin_filters(&mut clauses, &mut params, query);
        let use_fts = append_text_filter(&mut clauses, &mut params, query)?;
        let fts_rowid_order = use_fts && matches!(query.sort, ClipSort::Recent | ClipSort::Oldest);

        if let Some(cursor) = query.cursor.as_deref() {
            append_cursor_filter(
                &mut clauses,
                &mut params,
                query.sort,
                fts_rowid_order,
                &decode_cursor(cursor)?,
            );
        }
        let where_sql = clauses.join(" AND ");
        let order_sql = order_sql(query.sort, fts_rowid_order);
        let limit = query.limit.clamp(1, 1_000);
        let join_sql = if use_fts {
            "JOIN clips_fts ON clips_fts.rowid = c.id"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {} FROM clips c {join_sql}
             WHERE {where_sql}
             ORDER BY {order_sql}
             LIMIT ?",
            summary_columns()
        );
        params.push(SqlValue::Integer(i64::from(limit) + 1));
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement
            .query_map(rusqlite::params_from_iter(params), |row| {
                summary_from_row(row, state_rev)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = rows.len() > limit as usize;
        if has_more {
            rows.pop();
        }
        let next_cursor = if has_more {
            rows.last()
                .map(|summary| encode_cursor(&connection, summary.id, query.sort, fts_rowid_order))
                .transpose()?
        } else {
            None
        };
        if !query.include_preview {
            for item in &mut rows {
                item.preview_text.clear();
            }
        }
        let total_is_estimate = has_more || query.cursor.is_some();
        let total = rows.len() as u64;
        Ok(ClipPage {
            items: rows,
            total,
            total_is_estimate,
            took_ms: started.elapsed().as_millis() as u64,
            next_cursor,
        })
    }

    /// Total number of clips matching a query, independent of paging.
    ///
    /// The paged search returns only the rows it fetched, so the settings
    /// history needs a real count to render page controls.
    pub fn count_clips(&self, query: &ClipQuery) -> StorageResult<u64> {
        validate_search_mode(query.mode)?;
        let connection = self.reader()?;
        let mut clauses = vec!["c.is_deleted = 0".to_string()];
        let mut params = Vec::<SqlValue>::new();
        append_group_filter(&mut clauses, &mut params, query.group_id);
        append_kind_pin_filters(&mut clauses, &mut params, query);
        let use_fts = append_text_filter(&mut clauses, &mut params, query)?;
        let join_sql = if use_fts {
            "JOIN clips_fts ON clips_fts.rowid = c.id"
        } else {
            ""
        };
        let sql = format!(
            "SELECT COUNT(*) FROM clips c {join_sql} WHERE {}",
            clauses.join(" AND ")
        );
        let count: i64 =
            connection.query_row(&sql, rusqlite::params_from_iter(params), |row| row.get(0))?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    pub fn soft_delete_clips(&self, clip_ids: &[i64]) -> StorageResult<u64> {
        if clip_ids.is_empty() {
            return Ok(0);
        }
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        let mut changed = 0_u64;
        for clip_id in clip_ids {
            changed += transaction.execute(
                "UPDATE clips
                 SET is_deleted = 1, deleted_at_ms = ?1, updated_at_ms = ?1
                 WHERE id = ?2 AND is_deleted = 0",
                rusqlite::params![now, clip_id],
            )? as u64;
        }
        if changed > 0 {
            bump_state(&transaction)?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub fn restore_clips(&self, clip_ids: &[i64]) -> StorageResult<u64> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        let mut changed = 0_u64;
        for clip_id in clip_ids {
            changed += transaction.execute(
                "UPDATE clips
                 SET is_deleted = 0, deleted_at_ms = NULL, updated_at_ms = ?1
                 WHERE id = ?2 AND is_deleted = 1",
                rusqlite::params![now, clip_id],
            )? as u64;
        }
        if changed > 0 {
            bump_state(&transaction)?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub fn clear_history(&self) -> StorageResult<u64> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        let changed = transaction.execute(
            "UPDATE clips SET is_deleted = 1, deleted_at_ms = ?1, updated_at_ms = ?1
             WHERE is_deleted = 0",
            [now],
        )? as u64;
        if changed > 0 {
            bump_state(&transaction)?;
        }
        transaction.commit()?;
        tracing::info!(changed, "soft-deleted all clips");
        Ok(changed)
    }

    pub fn purge_deleted_before(&self, cutoff_ms: i64) -> StorageResult<u64> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "DELETE FROM clips WHERE is_deleted = 1 AND deleted_at_ms < ?1",
            [cutoff_ms],
        )? as u64;
        if changed > 0 {
            bump_state(&transaction)?;
        }
        transaction.commit()?;
        drop(connection);
        self.gc_orphan_blobs()?;
        Ok(changed)
    }

    pub fn cleanup_retention(&self, max_entries: u32, expire_days: u32) -> StorageResult<u64> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        let expire_before = now.saturating_sub(i64::from(expire_days) * 86_400_000);
        let mut ids = Vec::new();
        {
            let mut statement = transaction.prepare(
                "SELECT c.id FROM clips c
                 WHERE c.is_deleted = 0
                   AND c.is_pinned = 0
                   AND c.dont_auto_delete = 0
                   AND c.global_shortcut = 0
                   AND NOT EXISTS (SELECT 1 FROM copy_buffers b WHERE b.clip_id = c.id)
                   AND c.created_at_ms < ?1",
            )?;
            ids.extend(
                statement
                    .query_map([expire_before], |row| row.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?,
            );
        }
        if max_entries > 0 {
            let mut statement = transaction.prepare(
                "SELECT c.id FROM clips c
                 WHERE c.is_deleted = 0
                   AND c.is_pinned = 0
                   AND c.dont_auto_delete = 0
                   AND c.global_shortcut = 0
                   AND NOT EXISTS (SELECT 1 FROM copy_buffers b WHERE b.clip_id = c.id)
                 ORDER BY c.sort_order DESC, c.id DESC
                 LIMIT -1 OFFSET ?1",
            )?;
            ids.extend(
                statement
                    .query_map([i64::from(max_entries)], |row| row.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?,
            );
        }
        ids.sort_unstable();
        ids.dedup();
        let mut changed = 0_u64;
        for id in ids {
            changed += transaction.execute(
                "UPDATE clips
                 SET is_deleted = 1, deleted_at_ms = ?1, updated_at_ms = ?1
                 WHERE id = ?2 AND is_deleted = 0",
                rusqlite::params![now, id],
            )? as u64;
        }
        if changed > 0 {
            bump_state(&transaction)?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub fn mark_pasted(&self, clip_id: i64) -> StorageResult<()> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        let changed = transaction.execute(
            "UPDATE clips
             SET last_pasted_at_ms = ?1, paste_count = paste_count + 1, updated_at_ms = ?1
             WHERE id = ?2 AND is_deleted = 0",
            rusqlite::params![now, clip_id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!("clip {clip_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    /// Replaces the plain-text view of a clip while leaving the captured raw
    /// formats intact. Preview, quick-paste text, and description follow the
    /// edited text so search and the popup stay consistent.
    pub fn update_clip_text(&self, clip_id: i64, text: &str) -> StorageResult<()> {
        let text = truncate_utf8(text, 1024 * 1024);
        let preview = truncate_utf8(&text, 8 * 1024);
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE clips
             SET text_content = ?1, preview_text = ?2, quick_paste_text = ?2,
                 description = ?2, size_bytes = ?3, updated_at_ms = ?4
             WHERE id = ?5 AND is_deleted = 0",
            rusqlite::params![text, preview, text.len() as i64, now_ms(), clip_id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!("clip {clip_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn set_pinned(&self, clip_id: i64, pinned: bool) -> StorageResult<f64> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let pin_order = if pinned {
            transaction.query_row(
                "SELECT coalesce(max(pin_order), 0) + 1 FROM clips WHERE is_pinned = 1",
                [],
                |row| row.get::<_, f64>(0),
            )?
        } else {
            0.0
        };
        let changed = transaction.execute(
            "UPDATE clips SET is_pinned = ?1, pin_order = ?2, updated_at_ms = ?3
             WHERE id = ?4 AND is_deleted = 0",
            rusqlite::params![pinned as i64, pin_order, now_ms(), clip_id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!("clip {clip_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(pin_order)
    }

    pub fn set_clip_sort_order(&self, clip_id: i64, order: f64, group: bool) -> StorageResult<()> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let sql = if group {
            "UPDATE clips SET group_sort_order = ?1, updated_at_ms = ?2
             WHERE id = ?3 AND is_deleted = 0"
        } else {
            "UPDATE clips SET sort_order = ?1, updated_at_ms = ?2
             WHERE id = ?3 AND is_deleted = 0"
        };
        if transaction.execute(sql, rusqlite::params![order, now_ms(), clip_id])? == 0 {
            return Err(StorageError::NotFound(format!("clip {clip_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn move_clips_to_group(
        &self,
        clip_ids: &[i64],
        group_id: Option<i64>,
    ) -> StorageResult<u64> {
        if let Some(group_id) = group_id {
            let connection = self.reader()?;
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM groups WHERE id = ?1)",
                [group_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(StorageError::NotFound(format!("group {group_id}")));
            }
        }
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        let mut changed = 0_u64;
        for clip_id in clip_ids {
            changed += transaction.execute(
                "UPDATE clips SET group_id = ?1, updated_at_ms = ?2
                 WHERE id = ?3 AND is_deleted = 0",
                rusqlite::params![group_id, now, clip_id],
            )? as u64;
        }
        if changed > 0 {
            bump_state(&transaction)?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub fn list_groups(&self) -> StorageResult<Vec<GroupNode>> {
        let connection = self.reader()?;
        let mut statement = connection.prepare(
            "SELECT id, parent_id, name, sort_order FROM groups
             ORDER BY parent_id, sort_order, name COLLATE NOCASE, id",
        )?;
        Ok(statement
            .query_map([], |row| {
                Ok(GroupNode {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_group(
        &self,
        name: &str,
        parent_id: Option<i64>,
        sort_order: f64,
    ) -> StorageResult<i64> {
        if name.trim().is_empty() {
            return Err(StorageError::InvalidData("group name is empty".to_string()));
        }
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let now = now_ms();
        transaction.execute(
            "INSERT INTO groups(name, parent_id, sort_order, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![name.trim(), parent_id, sort_order, now],
        )?;
        let id = transaction.last_insert_rowid();
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn rename_group(&self, group_id: i64, name: &str) -> StorageResult<()> {
        if name.trim().is_empty() {
            return Err(StorageError::InvalidData("group name is empty".to_string()));
        }
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        if transaction.execute(
            "UPDATE groups SET name = ?1, updated_at_ms = ?2 WHERE id = ?3",
            rusqlite::params![name.trim(), now_ms(), group_id],
        )? == 0
        {
            return Err(StorageError::NotFound(format!("group {group_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn move_group(
        &self,
        group_id: i64,
        parent_id: Option<i64>,
        sort_order: f64,
    ) -> StorageResult<()> {
        if parent_id == Some(group_id) {
            return Err(StorageError::Conflict(
                "a group cannot be its own parent".to_string(),
            ));
        }
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        if let Some(parent_id) = parent_id {
            let creates_cycle: bool = transaction.query_row(
                "WITH RECURSIVE descendants(id) AS (
                   SELECT id FROM groups WHERE parent_id = ?1
                   UNION ALL
                   SELECT g.id FROM groups g JOIN descendants d ON g.parent_id = d.id
                 )
                 SELECT EXISTS(SELECT 1 FROM descendants WHERE id = ?2)",
                rusqlite::params![group_id, parent_id],
                |row| row.get(0),
            )?;
            if creates_cycle {
                return Err(StorageError::Conflict(
                    "moving the group would create a cycle".to_string(),
                ));
            }
        }
        if transaction.execute(
            "UPDATE groups SET parent_id = ?1, sort_order = ?2, updated_at_ms = ?3
             WHERE id = ?4",
            rusqlite::params![parent_id, sort_order, now_ms(), group_id],
        )? == 0
        {
            return Err(StorageError::NotFound(format!("group {group_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_group(&self, group_id: i64) -> StorageResult<()> {
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        if transaction.execute("DELETE FROM groups WHERE id = ?1", [group_id])? == 0 {
            return Err(StorageError::NotFound(format!("group {group_id}")));
        }
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn copy_buffers(&self) -> StorageResult<Vec<CopyBufferSlot>> {
        let connection = self.reader()?;
        let mut statement =
            connection.prepare("SELECT slot, name, clip_id FROM copy_buffers ORDER BY slot")?;
        Ok(statement
            .query_map([], |row| {
                Ok(CopyBufferSlot {
                    slot: row.get::<_, i64>(0)? as u8,
                    name: row.get(1)?,
                    clip_id: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn assign_copy_buffer(&self, slot: u8, clip_id: Option<i64>) -> StorageResult<()> {
        validate_slot(slot)?;
        if let Some(clip_id) = clip_id {
            let connection = self.reader()?;
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM clips WHERE id = ?1 AND is_deleted = 0)",
                [clip_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(StorageError::NotFound(format!("clip {clip_id}")));
            }
        }
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE copy_buffers SET clip_id = ?1, updated_at_ms = ?2 WHERE slot = ?3",
            rusqlite::params![clip_id, now_ms(), slot],
        )?;
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn set_copy_buffer_name(&self, slot: u8, name: &str) -> StorageResult<()> {
        validate_slot(slot)?;
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE copy_buffers SET name = ?1, updated_at_ms = ?2 WHERE slot = ?3",
            rusqlite::params![truncate_utf8(name, 256), now_ms(), slot],
        )?;
        bump_state(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn clear_copy_buffer(&self, slot: u8) -> StorageResult<()> {
        self.assign_copy_buffer(slot, None)
    }

    pub fn optimize(&self) -> StorageResult<()> {
        let connection = self.writer.lock();
        connection.execute_batch("PRAGMA optimize; PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(())
    }

    pub fn checkpoint(&self) -> StorageResult<()> {
        let connection = self.writer.lock();
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub fn database_size_bytes(&self) -> StorageResult<u64> {
        let mut total = 0_u64;
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.database_path.as_os_str().to_os_string();
            path.push(suffix);
            let path = PathBuf::from(path);
            if let Ok(metadata) = fs::metadata(path) {
                total = total.saturating_add(metadata.len());
            }
        }
        Ok(total)
    }

    pub fn gc_orphan_blobs(&self) -> StorageResult<u64> {
        let referenced = {
            let connection = self.reader()?;
            let mut paths = HashSet::new();
            let mut statement = connection
                .prepare("SELECT data_path FROM clip_formats WHERE data_path IS NOT NULL")?;
            for path in statement.query_map([], |row| row.get::<_, Option<String>>(0))? {
                if let Some(path) = path? {
                    paths.insert(path);
                }
            }
            let mut statement = connection.prepare("SELECT thumb_path FROM clips")?;
            for path in statement.query_map([], |row| row.get::<_, Option<String>>(0))? {
                if let Some(path) = path? {
                    paths.insert(path);
                }
            }
            paths
        };

        let mut removed = 0_u64;
        for directory in ["blobs", "thumbs"] {
            let root = self.data_root.join(directory);
            let canonical_root = root.canonicalize().unwrap_or(root.clone());
            for file in collect_files_recursive(&root)? {
                let relative = file
                    .strip_prefix(&self.data_root)
                    .map_err(|_| StorageError::InvalidData("blob escaped data root".into()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                if !referenced.contains(&relative) {
                    let canonical_file = file.canonicalize().unwrap_or(file.clone());
                    if canonical_file.starts_with(&canonical_root) {
                        fs::remove_file(file)?;
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }

    fn clip_flag(&self, clip_id: i64, column: &str) -> StorageResult<bool> {
        let connection = self.reader()?;
        let sql = format!("SELECT {column} FROM clips WHERE id = ?1 AND is_deleted = 0");
        connection
            .query_row(&sql, [clip_id], |row| row.get::<_, i64>(0))
            .optional()?
            .map(|value| value != 0)
            .ok_or_else(|| StorageError::NotFound(format!("clip {clip_id}")))
    }
}

#[derive(Debug)]
struct ReaderPool {
    connections: Mutex<Vec<Connection>>,
    available: Condvar,
}

impl ReaderPool {
    fn acquire(self: &Arc<Self>) -> ReaderGuard {
        let mut connections = self.connections.lock();
        loop {
            if let Some(connection) = connections.pop() {
                return ReaderGuard {
                    pool: Arc::clone(self),
                    connection: Some(connection),
                };
            }
            self.available.wait(&mut connections);
        }
    }
}

pub(crate) struct ReaderGuard {
    pool: Arc<ReaderPool>,
    connection: Option<Connection>,
}

impl std::ops::Deref for ReaderGuard {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.connection.as_ref().expect("reader present")
    }
}

impl Drop for ReaderGuard {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            self.pool.connections.lock().push(connection);
            self.pool.available.notify_one();
        }
    }
}

fn validate_new_clip(new: &NewClip) -> StorageResult<()> {
    if new.formats.is_empty() {
        return Err(StorageError::InvalidData(
            "clip must contain at least one format".to_string(),
        ));
    }
    if new.content_hash.len() != 32 || new.formats_hash.len() != 32 {
        return Err(StorageError::InvalidData(
            "SHA-256 hashes must be 32 bytes".to_string(),
        ));
    }
    if new.hash_algo != "sha256" {
        return Err(StorageError::InvalidData(format!(
            "unsupported hash algorithm {}",
            new.hash_algo
        )));
    }
    Ok(())
}

fn validate_search_mode(mode: SearchMode) -> StorageResult<()> {
    match mode {
        SearchMode::Contains
        | SearchMode::Fulltext
        | SearchMode::QuickPaste
        | SearchMode::Wildcard => Ok(()),
        SearchMode::Regex => Err(StorageError::InvalidData(
            "regex search is not supported".to_string(),
        )),
    }
}

fn validate_slot(slot: u8) -> StorageResult<()> {
    if (1..=5).contains(&slot) {
        Ok(())
    } else {
        Err(StorageError::InvalidData(format!(
            "copy buffer slot {slot} must be 1..=5"
        )))
    }
}

pub(crate) fn store_format(
    transaction: &rusqlite::Transaction<'_>,
    data_root: &Path,
    clip_id: i64,
    format: &ditto_model::NewClipFormat,
    primary_format: Option<&str>,
) -> StorageResult<()> {
    store_format_with_legacy(
        transaction,
        data_root,
        clip_id,
        format,
        primary_format,
        None,
    )
}

pub(crate) fn store_format_with_legacy(
    transaction: &rusqlite::Transaction<'_>,
    data_root: &Path,
    clip_id: i64,
    format: &ditto_model::NewClipFormat,
    primary_format: Option<&str>,
    legacy_data_id: Option<i64>,
) -> StorageResult<()> {
    let hash = blob::sha256(&format.data);
    let mime = format.mime.as_deref();
    let (data, data_path) = if blob::should_store_blob(&format.format_name, mime, format.data.len())
    {
        (
            None,
            Some(blob::write_blob(
                data_root,
                &format.data,
                &format.format_name,
                mime,
            )?),
        )
    } else {
        (Some(format.data.clone()), None)
    };
    transaction.execute(
        "INSERT INTO clip_formats(
           clip_id, format_name, seq, size_bytes, mime, data, data_path, sha256,
           is_primary, legacy_data_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            clip_id,
            format.format_name,
            i64::from(format.seq),
            format.data.len() as i64,
            format.mime,
            data,
            data_path,
            hash.to_vec(),
            (format.is_primary || primary_format == Some(format.format_name.as_str())) as i64,
            legacy_data_id,
        ],
    )?;
    Ok(())
}

fn bump_state(transaction: &rusqlite::Transaction<'_>) -> StorageResult<()> {
    bump_state_by(transaction, 1)
}

pub(crate) fn bump_state_by(
    transaction: &rusqlite::Transaction<'_>,
    amount: u64,
) -> StorageResult<()> {
    if amount == 0 {
        return Ok(());
    }
    transaction.execute(
        "UPDATE meta
         SET value = CAST(CAST(value AS INTEGER) + ?1 AS TEXT), updated_at_ms = ?2
         WHERE key = 'state_rev'",
        rusqlite::params![amount as i64, now_ms()],
    )?;
    Ok(())
}

fn summary_columns() -> &'static str {
    "c.id, c.kind, c.preview_text, c.source_app, c.created_at_ms,
     c.last_pasted_at_ms, c.is_pinned, c.pin_order, c.group_id,
     (SELECT b.slot FROM copy_buffers b WHERE b.clip_id = c.id ORDER BY b.slot LIMIT 1),
     (SELECT group_concat(f.format_name, char(31)) FROM clip_formats f WHERE f.clip_id = c.id),
     c.has_image, c.thumb_path, c.paste_count, c.sort_order, c.group_sort_order,
     c.size_bytes"
}

fn summary_from_row(row: &Row<'_>, state_rev: u64) -> rusqlite::Result<ClipSummary> {
    let kind_name: String = row.get(1)?;
    let kind = ClipKind::from_db_str(&kind_name).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(StorageError::InvalidData(format!(
                "unknown clip kind {kind_name}"
            ))),
        )
    })?;
    let pinned: i64 = row.get(6)?;
    let pin_order: f64 = row.get(7)?;
    let format_list: Option<String> = row.get(10)?;
    Ok(ClipSummary {
        id: row.get(0)?,
        kind,
        preview_text: row.get(2)?,
        source_app: row.get(3)?,
        created_at_ms: row.get(4)?,
        last_pasted_at_ms: row.get(5)?,
        pinned: pinned != 0,
        pin_order: (pinned != 0).then_some(pin_order),
        group_id: row.get(8)?,
        copy_buffer: row.get::<_, Option<i64>>(9)?.map(|slot| slot as u8),
        formats: format_list
            .unwrap_or_default()
            .split(FORMAT_SEPARATOR)
            .filter(|name| !name.is_empty())
            .map(ToString::to_string)
            .collect(),
        has_image: row.get::<_, i64>(11)? != 0,
        image_thumb_ref: row.get(12)?,
        pasted_count: row.get::<_, i64>(13)? as u32,
        size_bytes: row.get::<_, i64>(16)? as u64,
        state_rev,
    })
}

fn append_group_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    group_id: Option<i64>,
) {
    match group_id {
        Some(-1) => clauses.push("c.group_id IS NULL".to_string()),
        Some(group_id) => {
            clauses.push("c.group_id = ?".to_string());
            params.push(SqlValue::Integer(group_id));
        }
        None => {}
    }
}

fn append_kind_pin_filters(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    query: &ClipQuery,
) {
    if let Some(kind) = query.kind {
        clauses.push("c.kind = ?".to_string());
        params.push(SqlValue::Text(kind.as_db_str().to_string()));
    }
    if query.pinned_only {
        clauses.push("c.is_pinned = 1".to_string());
    }
}

fn append_text_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    query: &ClipQuery,
) -> StorageResult<bool> {
    let text = query.query.trim();
    if text.is_empty() {
        return Ok(false);
    }

    if query.mode == SearchMode::Wildcard {
        let pattern = SqlValue::Text(format!("%{}%", escape_wildcard(text)));
        clauses.push(
            "(c.preview_text LIKE ? ESCAPE '\\' OR c.quick_paste_text LIKE ? ESCAPE '\\'
              OR coalesce(c.text_content, '') LIKE ? ESCAPE '\\'
              OR c.description LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.extend([pattern.clone(), pattern.clone(), pattern.clone(), pattern]);
        return Ok(false);
    }

    if text.chars().count() < 3 {
        let pattern = SqlValue::Text(format!("%{}%", escape_like(text)));
        let sql = match query.mode {
            SearchMode::QuickPaste => "c.quick_paste_text LIKE ? ESCAPE '\\'".to_string(),
            SearchMode::Fulltext => "(coalesce(c.text_content, '') LIKE ? ESCAPE '\\'
                  OR c.preview_text LIKE ? ESCAPE '\\'
                  OR c.description LIKE ? ESCAPE '\\')"
                .to_string(),
            _ => "(c.preview_text LIKE ? ESCAPE '\\' OR c.quick_paste_text LIKE ? ESCAPE '\\'
                  OR coalesce(c.text_content, '') LIKE ? ESCAPE '\\'
                  OR c.description LIKE ? ESCAPE '\\')"
                .to_string(),
        };
        clauses.push(sql);
        let count = params_count_in_fragment(clauses.last().expect("just pushed"));
        for _ in 0..count {
            params.push(pattern.clone());
        }
        return Ok(false);
    }

    let phrase = format!("\"{}\"", text.replace('"', "\"\""));
    let match_expression = match query.mode {
        SearchMode::Fulltext => format!("text_content : {phrase}"),
        SearchMode::QuickPaste => format!("quick_paste_text : {phrase}"),
        _ => phrase,
    };
    clauses.push("clips_fts MATCH ?".to_string());
    params.push(SqlValue::Text(match_expression));
    Ok(true)
}

fn params_count_in_fragment(fragment: &str) -> usize {
    fragment.matches('?').count()
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn escape_wildcard(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '*' => output.push('%'),
            '?' => output.push('_'),
            '%' => output.push_str("\\%"),
            '_' => output.push_str("\\_"),
            '\\' => output.push_str("\\\\"),
            other => output.push(other),
        }
    }
    output
}

fn order_sql(sort: ClipSort, fts_rowid_order: bool) -> &'static str {
    if fts_rowid_order {
        return match sort {
            ClipSort::Oldest => "clips_fts.rowid ASC",
            _ => "clips_fts.rowid DESC",
        };
    }
    match sort {
        ClipSort::Oldest => "c.is_pinned DESC, c.pin_order DESC, c.sort_order ASC, c.id ASC",
        ClipSort::GroupOrder => {
            "c.is_pinned DESC, c.pin_order DESC, c.group_sort_order DESC, c.id DESC"
        }
        ClipSort::Recent | ClipSort::PinnedFirst => {
            "c.is_pinned DESC, c.pin_order DESC, c.sort_order DESC, c.id DESC"
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    pinned: bool,
    pin_order: f64,
    sort_order: f64,
    id: i64,
    #[serde(default)]
    fts_rowid: Option<i64>,
}

fn encode_cursor(
    connection: &Connection,
    clip_id: i64,
    sort: ClipSort,
    fts_rowid_order: bool,
) -> StorageResult<String> {
    if fts_rowid_order {
        return Ok(serde_json::to_string(&Cursor {
            pinned: false,
            pin_order: 0.0,
            sort_order: 0.0,
            id: clip_id,
            fts_rowid: Some(clip_id),
        })?);
    }
    let sort_column = if sort == ClipSort::GroupOrder {
        "group_sort_order"
    } else {
        "sort_order"
    };
    let sql = format!("SELECT is_pinned, pin_order, {sort_column}, id FROM clips WHERE id = ?1");
    let (pinned, pin_order, sort_order, id) = connection.query_row(&sql, [clip_id], |row| {
        Ok((
            row.get::<_, i64>(0)? != 0,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    let cursor = Cursor {
        pinned,
        pin_order,
        sort_order,
        id,
        fts_rowid: None,
    };
    Ok(serde_json::to_string(&cursor)?)
}

fn decode_cursor(cursor: &str) -> StorageResult<Cursor> {
    Ok(serde_json::from_str(cursor)?)
}

fn append_cursor_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    sort: ClipSort,
    fts_rowid_order: bool,
    cursor: &Cursor,
) {
    if fts_rowid_order {
        let comparison = if sort == ClipSort::Oldest { ">" } else { "<" };
        clauses.push(format!("clips_fts.rowid {comparison} ?"));
        params.push(SqlValue::Integer(cursor.fts_rowid.unwrap_or(cursor.id)));
        return;
    }
    let sort_column = if sort == ClipSort::GroupOrder {
        "c.group_sort_order"
    } else {
        "c.sort_order"
    };
    let comparison = if sort == ClipSort::Oldest { ">" } else { "<" };
    clauses.push(format!(
        "(c.is_pinned < ?
          OR (c.is_pinned = ? AND c.pin_order < ?)
          OR (c.is_pinned = ? AND c.pin_order = ? AND {sort_column} {comparison} ?)
          OR (c.is_pinned = ? AND c.pin_order = ? AND {sort_column} = ? AND c.id {comparison} ?))"
    ));
    let pinned = i64::from(cursor.pinned);
    params.extend([
        SqlValue::Integer(pinned),
        SqlValue::Integer(pinned),
        SqlValue::Real(cursor.pin_order),
        SqlValue::Integer(pinned),
        SqlValue::Real(cursor.pin_order),
        SqlValue::Real(cursor.sort_order),
        SqlValue::Integer(pinned),
        SqlValue::Real(cursor.pin_order),
        SqlValue::Real(cursor.sort_order),
        SqlValue::Integer(cursor.id),
    ]);
}

pub(crate) fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use ditto_model::{NewClip, NewClipFormat};

    use super::Storage;
    use crate::test_support::{temp_storage, test_clip};

    #[test]
    fn dedupe_updates_existing_and_multi_format_stays_one_clip() {
        let (_temp, storage) = temp_storage();
        let clip = test_clip("QA sample 001", "hash-001");
        let first = storage.insert_clip(&clip, false).expect("insert");
        assert!(first.inserted);
        let second = storage.insert_clip(&clip, false).expect("dedupe");
        assert!(!second.inserted);
        assert_eq!(first.clip.id, second.clip.id);
        assert_eq!(
            storage
                .get_clip(first.clip.id)
                .expect("detail")
                .formats
                .len(),
            2
        );
    }

    #[test]
    fn fts_search_and_two_character_fallback_find_english_and_chinese() {
        let (_temp, storage) = temp_storage();
        storage
            .insert_clip(&test_clip("alpha sample content", "hash-alpha"), false)
            .expect("insert alpha");
        storage
            .insert_clip(&test_clip("中文搜索内容", "hash-zh"), false)
            .expect("insert zh");
        let english = storage
            .search_clips(&ditto_model::ClipQuery {
                query: "sample".to_string(),
                ..Default::default()
            })
            .expect("search English");
        assert_eq!(english.total, 1);
        let chinese = storage
            .search_clips(&ditto_model::ClipQuery {
                query: "搜索内".to_string(),
                ..Default::default()
            })
            .expect("search Chinese");
        assert_eq!(chinese.total, 1);
        let short = storage
            .search_clips(&ditto_model::ClipQuery {
                query: "搜".to_string(),
                ..Default::default()
            })
            .expect("short fallback");
        assert_eq!(short.total, 1);
    }

    #[test]
    fn pin_group_and_copy_buffer_mutations_are_reflected_in_summary() {
        let (_temp, storage) = temp_storage();
        let outcome = storage
            .insert_clip(&test_clip("metadata sample", "hash-meta"), false)
            .expect("insert");
        let group = storage.create_group("Work", None, 1.0).expect("group");
        storage
            .move_clips_to_group(&[outcome.clip.id], Some(group))
            .expect("move");
        storage.set_pinned(outcome.clip.id, true).expect("pin");
        storage
            .assign_copy_buffer(2, Some(outcome.clip.id))
            .expect("buffer");
        let summary = storage.get_summary(outcome.clip.id).expect("summary");
        assert!(summary.pinned);
        assert_eq!(summary.group_id, Some(group));
        assert_eq!(summary.copy_buffer, Some(2));
    }

    #[test]
    fn hard_delete_and_gc_remove_blob_files() {
        let (_temp, storage) = temp_storage();
        let new = NewClip {
            formats: vec![NewClipFormat {
                format_name: "PNG".to_string(),
                seq: 0,
                data: tiny_png(),
                mime: Some("image/png".to_string()),
                is_primary: true,
            }],
            ..test_clip("", "hash-blob")
        };
        let outcome = storage.insert_clip(&new, false).expect("insert image");
        let before = storage.database_size_bytes().expect("database size before");
        assert!(before > 0);
        storage
            .soft_delete_clips(&[outcome.clip.id])
            .expect("soft delete");
        storage
            .purge_deleted_before(i64::MAX)
            .expect("purge deleted");
        assert!(storage.get_clip(outcome.clip.id).is_err());
        assert_eq!(storage.gc_orphan_blobs().expect("gc"), 0);
    }

    #[test]
    fn settings_revision_advances_only_for_changed_values() {
        let (_temp, storage) = temp_storage();
        let first = storage
            .set_setting("clipboard.max_entries", serde_json::json!(42))
            .expect("set setting");
        let second = storage
            .set_setting("clipboard.max_entries", serde_json::json!(42))
            .expect("set same setting");
        let third = storage
            .set_setting("clipboard.max_entries", serde_json::json!(43))
            .expect("change setting");
        assert_eq!(first.values["clipboard"]["max_entries"], 42);
        assert_eq!(first.revision, second.revision);
        assert!(third.revision > second.revision);
    }

    #[test]
    fn retention_protects_pinned_dont_delete_and_copy_buffer_clips() {
        let (_temp, storage) = temp_storage();
        let mut protected_pin = test_clip("pinned", "hash-pinned");
        protected_pin.is_pinned = true;
        protected_pin.pin_order = 10.0;
        protected_pin.sort_order = 10.0;
        let pinned = storage
            .insert_clip(&protected_pin, false)
            .expect("pin")
            .clip;
        let mut protected_flag = test_clip("protected", "hash-protected");
        protected_flag.dont_auto_delete = true;
        protected_flag.sort_order = 20.0;
        let flagged = storage
            .insert_clip(&protected_flag, false)
            .expect("flag")
            .clip;
        let mut protected_buffer = test_clip("buffer", "hash-buffer");
        protected_buffer.sort_order = 30.0;
        let buffered = storage
            .insert_clip(&protected_buffer, false)
            .expect("buffer")
            .clip;
        storage
            .assign_copy_buffer(1, Some(buffered.id))
            .expect("assign buffer");
        let mut removable = test_clip("removable", "hash-removable");
        removable.sort_order = 40.0;
        let removed = storage
            .insert_clip(&removable, false)
            .expect("removable")
            .clip;
        let mut newer = test_clip("newer removable", "hash-newer");
        newer.sort_order = 50.0;
        storage.insert_clip(&newer, false).expect("newer");
        assert_eq!(storage.cleanup_retention(1, 100_000).expect("retention"), 1);
        assert_eq!(
            storage.get_summary(pinned.id).expect("pin kept").id,
            pinned.id
        );
        assert_eq!(
            storage.get_summary(flagged.id).expect("flag kept").id,
            flagged.id
        );
        assert_eq!(
            storage.get_summary(buffered.id).expect("buffer kept").id,
            buffered.id
        );
        assert!(storage.get_summary(removed.id).is_err());
    }

    #[test]
    fn copy_buffer_assignment_survives_reopen() {
        let temp = tempfile::tempdir().expect("temp root");
        let clip = test_clip("persistent buffer", "hash-persistent");
        let clip_id = {
            let storage = Storage::open_data_root(temp.path()).expect("open");
            let clip_id = storage.insert_clip(&clip, false).expect("insert").clip.id;
            storage
                .assign_copy_buffer(3, Some(clip_id))
                .expect("assign");
            clip_id
        };
        let reopened = Storage::open_data_root(temp.path()).expect("reopen");
        assert_eq!(
            reopened
                .copy_buffers()
                .expect("buffers")
                .into_iter()
                .find(|slot| slot.slot == 3)
                .and_then(|slot| slot.clip_id),
            Some(clip_id)
        );
    }

    fn tiny_png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 40, 60, 255]));
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .expect("png");
        output.into_inner()
    }
}
