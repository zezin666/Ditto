//! Narrow, additive helpers for full-fidelity legacy migration.
//!
//! These APIs deliberately bypass T2 capture dedupe because every legacy
//! `Main` row must become one clip. They still use the existing v1 schema,
//! FTS triggers, blob writer, thumbnail writer, and transaction machinery.

use std::collections::{BTreeMap, BTreeSet};

use ditto_model::{ClipKind, NewClipFormat};
use rusqlite::{OptionalExtension, params};

use crate::{
    Storage, StorageError, StorageResult, blob,
    store::{bump_state_by, store_format_with_legacy, truncate_utf8},
};

#[derive(Debug, Clone)]
pub struct LegacyFormatRecord {
    pub legacy_data_id: Option<i64>,
    pub format_name: String,
    pub seq: u32,
    pub data: Vec<u8>,
    pub mime: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct LegacyClipRecord {
    pub legacy_id: i64,
    pub kind: ClipKind,
    pub primary_format: Option<String>,
    pub text_content: Option<String>,
    pub text_truncated: bool,
    pub preview_text: String,
    pub quick_paste_text: String,
    pub description: String,
    pub source_app: Option<String>,
    pub source_window_title: Option<String>,
    pub content_hash: Vec<u8>,
    pub formats_hash: Vec<u8>,
    pub hash_algo: String,
    pub size_bytes: u64,
    pub is_pinned: bool,
    pub pin_order: f64,
    pub group_legacy_id: Option<i64>,
    pub sort_order: f64,
    pub group_sort_order: f64,
    pub created_at_ms: i64,
    pub last_pasted_at_ms: Option<i64>,
    pub paste_count: u32,
    pub dont_auto_delete: bool,
    pub shortcut: Option<i64>,
    pub global_shortcut: bool,
    pub move_to_group_shortcut: Option<i64>,
    pub global_move_to_group_shortcut: bool,
    pub has_image: bool,
    pub thumbnail_png: Option<Vec<u8>>,
    pub legacy_crc: Option<i64>,
    pub legacy_meta: Option<String>,
    pub formats: Vec<LegacyFormatRecord>,
}

#[derive(Debug, Clone)]
pub struct LegacyGroupRecord {
    pub legacy_id: i64,
    pub name: String,
    pub parent_legacy_id: Option<i64>,
    pub sort_order: f64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LegacyCopyBufferRecord {
    /// New one-based slot, 1..=5.
    pub slot: u8,
    pub clip_legacy_id: Option<i64>,
    pub name: String,
    pub updated_at_ms: Option<i64>,
    pub legacy_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct LegacyBatch {
    pub groups: Vec<LegacyGroupRecord>,
    pub clips: Vec<LegacyClipRecord>,
    pub copy_buffers: Vec<LegacyCopyBufferRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct LegacyInsertResult {
    pub group_ids: BTreeMap<i64, i64>,
    pub clip_ids: BTreeMap<i64, i64>,
    pub inserted_groups: u64,
    pub skipped_groups: u64,
    pub inserted_clips: u64,
    pub skipped_clips: u64,
    pub assigned_copy_buffers: u64,
}

impl Storage {
    /// Inserts legacy clips without capture dedupe.
    pub fn insert_legacy_many(
        &self,
        clips: &[LegacyClipRecord],
    ) -> StorageResult<LegacyInsertResult> {
        self.insert_legacy_batch_parts(&[], clips, &[])
    }

    /// Inserts one legacy batch atomically. Existing `legacy_id` values are
    /// treated as already migrated and returned in the ID maps.
    pub fn insert_legacy_batch(&self, batch: &LegacyBatch) -> StorageResult<LegacyInsertResult> {
        self.insert_legacy_batch_parts(&batch.groups, &batch.clips, &batch.copy_buffers)
    }

    fn insert_legacy_batch_parts(
        &self,
        groups: &[LegacyGroupRecord],
        clips: &[LegacyClipRecord],
        copy_buffers: &[LegacyCopyBufferRecord],
    ) -> StorageResult<LegacyInsertResult> {
        validate_unique_groups(groups)?;
        validate_unique_clips(clips)?;
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let mut result = LegacyInsertResult::default();

        {
            let mut statement = transaction
                .prepare("SELECT legacy_id, id FROM groups WHERE legacy_id IS NOT NULL")?;
            for row in
                statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
            {
                let (legacy_id, id) = row?;
                result.group_ids.insert(legacy_id, id);
            }
        }

        let mut pending = groups
            .iter()
            .filter(|group| !result.group_ids.contains_key(&group.legacy_id))
            .collect::<Vec<_>>();
        let mut progress = true;
        while !pending.is_empty() && progress {
            progress = false;
            let mut next = Vec::new();
            for group in pending {
                let parent_id = match group.parent_legacy_id {
                    None => None,
                    Some(parent) if parent == group.legacy_id => {
                        return Err(StorageError::InvalidData(format!(
                            "legacy group {} is its own parent",
                            group.legacy_id
                        )));
                    }
                    Some(parent) => match result.group_ids.get(&parent) {
                        Some(id) => Some(*id),
                        None => {
                            next.push(group);
                            continue;
                        }
                    },
                };
                let now = group.updated_at_ms.max(group.created_at_ms);
                transaction.execute(
                    "INSERT INTO groups(
                       name, parent_id, sort_order, created_at_ms, updated_at_ms, legacy_id
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        group.name.trim(),
                        parent_id,
                        group.sort_order,
                        group.created_at_ms,
                        now,
                        group.legacy_id
                    ],
                )?;
                result
                    .group_ids
                    .insert(group.legacy_id, transaction.last_insert_rowid());
                result.inserted_groups += 1;
                progress = true;
            }
            pending = next;
        }
        if !pending.is_empty() {
            let missing = pending
                .iter()
                .map(|group| group.legacy_id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(StorageError::InvalidData(format!(
                "legacy group parent cycle or missing parent for group(s): {missing}"
            )));
        }

        for clip in clips {
            if let Some(existing_id) = transaction
                .query_row(
                    "SELECT id FROM clips WHERE legacy_id = ?1",
                    [clip.legacy_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
            {
                result.clip_ids.insert(clip.legacy_id, existing_id);
                result.skipped_clips += 1;
                continue;
            }
            validate_legacy_clip(clip)?;
            let group_id = match clip.group_legacy_id {
                None => None,
                Some(legacy_group_id) => {
                    Some(*result.group_ids.get(&legacy_group_id).ok_or_else(|| {
                        StorageError::NotFound(format!(
                            "legacy group {legacy_group_id} for clip {}",
                            clip.legacy_id
                        ))
                    })?)
                }
            };
            let thumb_path = match &clip.thumbnail_png {
                Some(bytes) => Some(blob::write_thumbnail(
                    self.data_root(),
                    &clip.content_hash,
                    bytes,
                )?),
                None => None,
            };
            let text_was_truncated = clip
                .text_content
                .as_ref()
                .is_some_and(|text| text.len() > 1024 * 1024);
            transaction.execute(
                "INSERT INTO clips(
                   legacy_id, kind, primary_format, text_content, text_truncated,
                   preview_text, quick_paste_text, description, source_app,
                   source_window_title, content_hash, formats_hash, hash_algo,
                   size_bytes, is_pinned, pin_order, group_id, sort_order,
                   group_sort_order, created_at_ms, updated_at_ms, last_pasted_at_ms,
                   paste_count, dont_auto_delete, shortcut, global_shortcut,
                   move_to_group_shortcut, global_move_to_group_shortcut, has_image,
                   thumb_path, legacy_crc, legacy_meta
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?20, ?21, ?22, ?23, ?24,
                   ?25, ?26, ?27, ?28, ?29, ?30, ?31
                 )",
                params![
                    clip.legacy_id,
                    clip.kind.as_db_str(),
                    clip.primary_format,
                    truncate_utf8(
                        clip.text_content.as_deref().unwrap_or_default(),
                        1024 * 1024
                    ),
                    (clip.text_truncated || text_was_truncated) as i64,
                    truncate_utf8(&clip.preview_text, 8 * 1024),
                    truncate_utf8(&clip.quick_paste_text, 8 * 1024),
                    truncate_utf8(&clip.description, 8 * 1024),
                    clip.source_app,
                    clip.source_window_title,
                    clip.content_hash,
                    clip.formats_hash,
                    clip.hash_algo,
                    clip.size_bytes as i64,
                    clip.is_pinned as i64,
                    clip.pin_order,
                    group_id,
                    clip.sort_order,
                    clip.group_sort_order,
                    clip.created_at_ms,
                    clip.last_pasted_at_ms,
                    i64::from(clip.paste_count),
                    clip.dont_auto_delete as i64,
                    clip.shortcut,
                    clip.global_shortcut as i64,
                    clip.move_to_group_shortcut,
                    clip.global_move_to_group_shortcut as i64,
                    clip.has_image as i64,
                    thumb_path,
                    clip.legacy_crc,
                    clip.legacy_meta,
                ],
            )?;
            let clip_id = transaction.last_insert_rowid();
            for format in &clip.formats {
                store_format_with_legacy(
                    &transaction,
                    self.data_root(),
                    clip_id,
                    &NewClipFormat {
                        format_name: format.format_name.clone(),
                        seq: format.seq,
                        data: format.data.clone(),
                        mime: format.mime.clone(),
                        is_primary: format.is_primary,
                    },
                    clip.primary_format.as_deref(),
                    format.legacy_data_id,
                )?;
            }
            result.clip_ids.insert(clip.legacy_id, clip_id);
            result.inserted_clips += 1;
        }

        for buffer in copy_buffers {
            validate_slot(buffer.slot)?;
            let clip_id = match buffer.clip_legacy_id {
                None => None,
                Some(legacy_id) => {
                    let clip_id = if let Some(clip_id) = result.clip_ids.get(&legacy_id) {
                        *clip_id
                    } else {
                        transaction
                            .query_row(
                                "SELECT id FROM clips WHERE legacy_id = ?1",
                                [legacy_id],
                                |row| row.get::<_, i64>(0),
                            )
                            .optional()?
                            .ok_or_else(|| {
                                StorageError::NotFound(format!(
                                    "legacy clip {legacy_id} for copy buffer {}",
                                    buffer.slot
                                ))
                            })?
                    };
                    Some(clip_id)
                }
            };
            let changed = transaction.execute(
                "UPDATE copy_buffers
                 SET clip_id = ?1, name = ?2, updated_at_ms = ?3, legacy_id = ?4
                 WHERE slot = ?5",
                params![
                    clip_id,
                    truncate_utf8(&buffer.name, 256),
                    buffer.updated_at_ms,
                    buffer.legacy_id,
                    buffer.slot
                ],
            )?;
            if changed == 0 {
                return Err(StorageError::InvalidData(format!(
                    "copy buffer slot {} is unavailable",
                    buffer.slot
                )));
            }
            result.assigned_copy_buffers += 1;
        }

        bump_state_by(
            &transaction,
            result.inserted_groups + result.inserted_clips + result.assigned_copy_buffers,
        )?;
        transaction.commit()?;
        Ok(result)
    }
}

fn validate_unique_groups(groups: &[LegacyGroupRecord]) -> StorageResult<()> {
    let mut ids = BTreeSet::new();
    for group in groups {
        if !ids.insert(group.legacy_id) {
            return Err(StorageError::Conflict(format!(
                "duplicate legacy group {} in batch",
                group.legacy_id
            )));
        }
        if group.name.trim().is_empty() {
            return Err(StorageError::InvalidData(format!(
                "legacy group {} has an empty name",
                group.legacy_id
            )));
        }
    }
    Ok(())
}

fn validate_unique_clips(clips: &[LegacyClipRecord]) -> StorageResult<()> {
    let mut ids = BTreeSet::new();
    for clip in clips {
        if !ids.insert(clip.legacy_id) {
            return Err(StorageError::Conflict(format!(
                "duplicate legacy clip {} in batch",
                clip.legacy_id
            )));
        }
    }
    Ok(())
}

fn validate_legacy_clip(clip: &LegacyClipRecord) -> StorageResult<()> {
    if clip.content_hash.len() != 32 || clip.formats_hash.len() != 32 {
        return Err(StorageError::InvalidData(format!(
            "legacy clip {} must carry 32-byte SHA-256 hashes",
            clip.legacy_id
        )));
    }
    if clip.hash_algo != "sha256" {
        return Err(StorageError::InvalidData(format!(
            "legacy clip {} uses unsupported hash algorithm {}",
            clip.legacy_id, clip.hash_algo
        )));
    }
    let mut format_ids = BTreeSet::new();
    for format in &clip.formats {
        if let Some(legacy_data_id) = format.legacy_data_id {
            if !format_ids.insert(legacy_data_id) {
                return Err(StorageError::Conflict(format!(
                    "duplicate legacy data id {legacy_data_id} in clip {}",
                    clip.legacy_id
                )));
            }
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Storage;
    use tempfile::TempDir;

    fn sample_clip(legacy_id: i64, group: Option<i64>, hash_seed: &str) -> LegacyClipRecord {
        LegacyClipRecord {
            legacy_id,
            kind: ClipKind::Text,
            primary_format: Some("CF_UNICODETEXT".into()),
            text_content: Some(format!("legacy text {hash_seed}")),
            text_truncated: false,
            preview_text: format!("legacy text {hash_seed}"),
            quick_paste_text: "qp".into(),
            description: "legacy".into(),
            source_app: None,
            source_window_title: None,
            content_hash: blob::sha256(hash_seed.as_bytes()).to_vec(),
            formats_hash: blob::sha256(format!("formats-{hash_seed}").as_bytes()).to_vec(),
            hash_algo: "sha256".into(),
            size_bytes: 10,
            is_pinned: legacy_id == 2,
            pin_order: if legacy_id == 2 { 7.0 } else { 0.0 },
            group_legacy_id: group,
            sort_order: legacy_id as f64,
            group_sort_order: legacy_id as f64,
            created_at_ms: 1_700_000_000_000 + legacy_id,
            last_pasted_at_ms: Some(1_700_000_100_000 + legacy_id),
            paste_count: 3,
            dont_auto_delete: legacy_id == 2,
            shortcut: Some(20 + legacy_id),
            global_shortcut: legacy_id == 2,
            move_to_group_shortcut: Some(30 + legacy_id),
            global_move_to_group_shortcut: legacy_id == 2,
            has_image: false,
            thumbnail_png: None,
            legacy_crc: Some(1234),
            legacy_meta: Some(r#"{"source":"test"}"#.into()),
            formats: vec![LegacyFormatRecord {
                legacy_data_id: Some(100 + legacy_id),
                format_name: "CF_UNICODETEXT".into(),
                seq: 0,
                data: format!("legacy text {hash_seed}")
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
                mime: Some("text/plain".into()),
                is_primary: true,
            }],
        }
    }

    #[test]
    fn legacy_batch_preserves_duplicate_hashes_and_maps_ids() {
        let temp = TempDir::new().expect("temp");
        let storage = Storage::open_data_root(temp.path()).expect("storage");
        let batch = LegacyBatch {
            groups: vec![
                LegacyGroupRecord {
                    legacy_id: 10,
                    name: "Root".into(),
                    parent_legacy_id: None,
                    sort_order: 1.0,
                    created_at_ms: 100,
                    updated_at_ms: 100,
                },
                LegacyGroupRecord {
                    legacy_id: 11,
                    name: "Child".into(),
                    parent_legacy_id: Some(10),
                    sort_order: 2.0,
                    created_at_ms: 101,
                    updated_at_ms: 101,
                },
            ],
            clips: vec![
                sample_clip(1, Some(11), "same"),
                sample_clip(2, Some(11), "same"),
            ],
            copy_buffers: vec![LegacyCopyBufferRecord {
                slot: 1,
                clip_legacy_id: Some(2),
                name: "one".into(),
                updated_at_ms: Some(200),
                legacy_id: Some(50),
            }],
        };
        let result = storage.insert_legacy_batch(&batch).expect("insert");
        assert_eq!(result.inserted_clips, 2);
        assert_eq!(result.inserted_groups, 2);
        assert_eq!(result.assigned_copy_buffers, 1);
        assert_eq!(
            result.group_ids.get(&11),
            result
                .clip_ids
                .get(&1)
                .map(|_| result.group_ids.get(&11).expect("child"))
        );

        let connection = rusqlite::Connection::open(storage.database_path()).expect("open");
        let clip_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))
            .expect("clip count");
        assert_eq!(clip_count, 2);
        let mapped_group: i64 = connection
            .query_row(
                "SELECT group_id FROM clips WHERE legacy_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("group id");
        assert_eq!(mapped_group, result.group_ids[&11]);
        let slot_clip: i64 = connection
            .query_row(
                "SELECT clip_id FROM copy_buffers WHERE slot = 1",
                [],
                |row| row.get(0),
            )
            .expect("copy buffer");
        assert_eq!(slot_clip, result.clip_ids[&2]);
        let duplicate_hashes: i64 = connection
            .query_row(
                "SELECT COUNT(DISTINCT content_hash) FROM clips",
                [],
                |row| row.get(0),
            )
            .expect("hash count");
        assert_eq!(duplicate_hashes, 1);
        let legacy_data: i64 = connection
            .query_row(
                "SELECT legacy_data_id FROM clip_formats WHERE clip_id = ?1",
                [result.clip_ids[&1]],
                |row| row.get(0),
            )
            .expect("legacy data");
        assert_eq!(legacy_data, 101);

        let again = storage.insert_legacy_batch(&batch).expect("idempotent");
        assert_eq!(again.inserted_clips, 0);
        assert_eq!(again.skipped_clips, 2);
        let after: i64 = connection
            .query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))
            .expect("clip count");
        assert_eq!(after, 2);
    }

    #[test]
    fn legacy_batch_rolls_back_on_group_conflict() {
        let temp = TempDir::new().expect("temp");
        let storage = Storage::open_data_root(temp.path()).expect("storage");
        let batch = LegacyBatch {
            groups: vec![
                LegacyGroupRecord {
                    legacy_id: 1,
                    name: "Duplicate".into(),
                    parent_legacy_id: None,
                    sort_order: 1.0,
                    created_at_ms: 1,
                    updated_at_ms: 1,
                },
                LegacyGroupRecord {
                    legacy_id: 2,
                    name: "duplicate".into(),
                    parent_legacy_id: None,
                    sort_order: 2.0,
                    created_at_ms: 2,
                    updated_at_ms: 2,
                },
            ],
            ..LegacyBatch::default()
        };
        assert!(storage.insert_legacy_batch(&batch).is_err());
        let connection = rusqlite::Connection::open(storage.database_path()).expect("open");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM groups", [], |row| row.get(0))
            .expect("group count");
        assert_eq!(count, 0);
    }
}
