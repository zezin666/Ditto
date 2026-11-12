use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use ditto_storage::{
    Storage, configure_connection,
    legacy::{LegacyBatch, LegacyCopyBufferRecord, LegacyGroupRecord},
    now_ms,
};
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{
    MigrationError, MigrationResult, MigrationSource,
    format::map_clip,
    ini::map_settings,
    source::{LegacyCatalog, LegacySource},
    types::{BackupRecord, ImportedCounts, MigrationProgress, MigrationReport, MigrationStage},
};

#[derive(Debug, Clone)]
pub struct MigrationOptions {
    pub batch_size: u32,
    pub backup_dir: Option<PathBuf>,
    pub keep_backup: bool,
    pub replace_existing: bool,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            batch_size: 500,
            backup_dir: None,
            keep_backup: true,
            replace_existing: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MigrationOutcome {
    pub report: MigrationReport,
    pub destination: PathBuf,
}

pub fn dry_run(source: &MigrationSource) -> MigrationResult<MigrationReport> {
    let started_at = now_ms();
    let started = Instant::now();
    let reader = LegacySource::new(&source.database, source.settings.clone());
    let catalog = reader.catalog()?;
    let settings = map_settings(source.settings.as_deref(), &catalog)?;
    let mut report = MigrationReport::new(true);
    report.status = "dry_run".into();
    report.source_path = catalog.fingerprint.path.clone();
    report.source_sha256 = catalog.fingerprint.sha256.clone();
    report.source_size_bytes = catalog.fingerprint.size_bytes;
    report.source_modified_ms = catalog.fingerprint.modified_ms;
    report.destination = String::new();
    report.started_at_ms = started_at;
    report.duration_ms = started.elapsed().as_millis() as u64;
    report.source_counts = catalog.counts.clone();
    report.format_distribution = catalog.format_distribution.clone();
    report.copy_buffer_slots = catalog.copy_buffer_slots.clone();
    report.settings_mapped = settings.mapped_count;
    report.settings_unknown = settings.unknown.keys().cloned().collect();
    report.warnings.extend(catalog.warnings.clone());
    report.warnings.extend(settings.warnings);
    verify_source_unchanged(&reader, &catalog.fingerprint)?;
    Ok(report)
}

pub fn migrate(
    source: &MigrationSource,
    destination: &Path,
    options: &MigrationOptions,
) -> MigrationResult<MigrationOutcome> {
    migrate_with_progress(source, destination, options, &mut |_| {})
}

pub fn migrate_with_progress(
    source: &MigrationSource,
    destination: &Path,
    options: &MigrationOptions,
    progress: &mut dyn FnMut(MigrationProgress),
) -> MigrationResult<MigrationOutcome> {
    let started_at = now_ms();
    let started = Instant::now();
    let destination = absolute_path(destination)?;
    let source_database = absolute_path(&source.database)?;
    if destination.parent().is_none() {
        return Err(MigrationError::Invalid(
            "destination must not be a filesystem root".into(),
        ));
    }
    if source_database.starts_with(&destination) {
        return Err(MigrationError::Conflict(
            "the legacy source must not be inside the destination data root".into(),
        ));
    }
    if path_is_nonempty(&destination)? && !options.replace_existing {
        return Err(MigrationError::Conflict(format!(
            "destination {} is not empty; use --replace-existing",
            destination.display()
        )));
    }
    let mut preexisting_backup = None;
    if path_is_nonempty(&destination)? {
        let backup_dir = options.backup_dir.clone().unwrap_or_else(|| {
            destination
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("ditto-backups")
        });
        let package = crate::package::backup(&destination, &backup_dir, true)?;
        preexisting_backup = Some(package_backup_record(&package)?);
    }
    let reader = LegacySource::new(&source.database, source.settings.clone());
    let catalog = reader.catalog()?;
    progress(MigrationProgress {
        stage: MigrationStage::Preflight,
        processed: 0,
        total: catalog.counts.clips + catalog.counts.groups,
        message: "legacy database validated".into(),
    });
    let staging = unique_sibling(&destination, ".ditto-migrate")?;
    fs::create_dir_all(&staging)?;
    let result = build_staging(
        &reader, &catalog, source, &staging, options, started_at, started, progress,
    );
    let mut outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    if let Some(backup) = preexisting_backup {
        outcome.report.backups.push(backup);
    }
    verify_source_unchanged(&reader, &catalog.fingerprint)?;
    progress(MigrationProgress {
        stage: MigrationStage::Commit,
        processed: catalog.counts.clips,
        total: catalog.counts.clips,
        message: "switching to verified migration output".into(),
    });
    if let Err(error) = replace_destination(&staging, &destination, options.replace_existing) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    outcome.destination = destination.clone();
    outcome.report.destination = destination.display().to_string();
    for backup in &mut outcome.report.backups {
        let source_path = PathBuf::from(&backup.path);
        if let Ok(relative) = source_path.strip_prefix(&staging) {
            backup.path = destination.join(relative).display().to_string();
        }
    }
    outcome.report.status = "success".into();
    outcome.report.duration_ms = started.elapsed().as_millis() as u64;
    if let Err(error) = write_json(&destination.join("migration-report.json"), &outcome.report) {
        outcome
            .report
            .warnings
            .push(format!("final migration report rewrite failed: {error}"));
    }
    progress(MigrationProgress {
        stage: MigrationStage::Complete,
        processed: catalog.counts.clips,
        total: catalog.counts.clips,
        message: "migration complete".into(),
    });
    Ok(outcome)
}

#[allow(clippy::too_many_arguments)]
fn build_staging(
    reader: &LegacySource,
    catalog: &LegacyCatalog,
    source: &MigrationSource,
    staging: &Path,
    options: &MigrationOptions,
    started_at: i64,
    started: Instant,
    progress: &mut dyn FnMut(MigrationProgress),
) -> MigrationResult<MigrationOutcome> {
    let storage = Storage::open_data_root(staging)?;
    let state = Connection::open(storage.database_path())?;
    configure_connection(&state, false)?;
    let state_id = begin_migration_state(&state, source, &catalog.fingerprint.sha256, started_at)?;
    let mut report = MigrationReport::new(false);
    report.source_path = catalog.fingerprint.path.clone();
    report.source_sha256 = catalog.fingerprint.sha256.clone();
    report.source_size_bytes = catalog.fingerprint.size_bytes;
    report.source_modified_ms = catalog.fingerprint.modified_ms;
    report.destination = staging.display().to_string();
    report.started_at_ms = started_at;
    report.source_counts = catalog.counts.clone();
    report.format_distribution = catalog.format_distribution.clone();
    report.copy_buffer_slots = catalog.copy_buffer_slots.clone();
    report.warnings.extend(catalog.warnings.clone());

    let internal_backup = staging
        .join("backups")
        .join(format!("source-{}.db", &catalog.fingerprint.sha256[..16]));
    let source_backup = copy_file_with_hash(&source.database, &internal_backup)?;
    copy_source_sidecars(&source.database, &internal_backup)?;
    report.backups.push(source_backup);
    if let Some(settings_path) = &source.settings {
        if settings_path.is_file() {
            let target = staging.join("backups").join("source-Ditto.Settings");
            copy_file_with_hash(settings_path, &target)?;
        }
    }
    if let Some(backup_dir) = &options.backup_dir {
        fs::create_dir_all(backup_dir)?;
        let target = backup_dir.join(format!("ditto-legacy-source.{}.db", now_ms()));
        report
            .backups
            .push(copy_file_with_hash(&source.database, &target)?);
        copy_source_sidecars(&source.database, &target)?;
    }

    let group_records = catalog
        .groups
        .iter()
        .map(|group| {
            let created_at_ms = group.date_seconds.max(0).saturating_mul(1000);
            LegacyGroupRecord {
                legacy_id: group.legacy_id,
                name: if group.name.trim().is_empty() {
                    format!("Legacy group {}", group.legacy_id)
                } else {
                    group.name.clone()
                },
                parent_legacy_id: group.parent_legacy_id,
                sort_order: group
                    .clip_group_order
                    .or(group.clip_order)
                    .unwrap_or(created_at_ms as f64),
                created_at_ms,
                updated_at_ms: created_at_ms,
            }
        })
        .collect::<Vec<_>>();
    if !group_records.is_empty() {
        storage.insert_legacy_batch(&LegacyBatch {
            groups: group_records,
            ..LegacyBatch::default()
        })?;
    }
    update_migration_state(&state, state_id, "groups", None)?;
    progress(MigrationProgress {
        stage: MigrationStage::Groups,
        processed: catalog.counts.groups,
        total: catalog.counts.groups,
        message: "legacy groups inserted".into(),
    });

    let mut last_legacy_id = 0_i64;
    let mut processed = 0_u64;
    let mut expected_imported_formats = 0_u64;
    let mut migrated_clip_ids = BTreeMap::new();
    loop {
        let rows = reader.clip_batch(last_legacy_id, options.batch_size.clamp(1, 2_000))?;
        if rows.is_empty() {
            break;
        }
        last_legacy_id = rows.last().map_or(last_legacy_id, |row| row.legacy_id);
        let ids = rows.iter().map(|row| row.legacy_id).collect::<Vec<_>>();
        let formats = reader.formats_for_clips(&ids)?;
        let mut records = Vec::with_capacity(rows.len());
        for row in &rows {
            let mapped = map_clip(
                row,
                formats
                    .get(&row.legacy_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
            );
            append_warnings(&mut report.warnings, mapped.warnings);
            expected_imported_formats += mapped
                .record
                .formats
                .iter()
                .filter(|format| format.legacy_data_id.is_some())
                .count() as u64;
            records.push(mapped.record);
        }
        let result = storage.insert_legacy_batch(&LegacyBatch {
            clips: records,
            ..LegacyBatch::default()
        })?;
        migrated_clip_ids.extend(result.clip_ids);
        processed += rows.len() as u64;
        progress(MigrationProgress {
            stage: MigrationStage::Clips,
            processed,
            total: catalog.counts.clips,
            message: "legacy clips inserted".into(),
        });
    }
    update_migration_state(&state, state_id, "clips", None)?;

    let copy_buffers = catalog
        .copy_buffers
        .iter()
        .filter_map(|buffer| {
            let clip_legacy_id = buffer.clip_legacy_id?;
            if !migrated_clip_ids.contains_key(&clip_legacy_id) {
                append_warnings(
                    &mut report.warnings,
                    vec![format!(
                        "copy buffer {} references missing clip {}; ignored",
                        buffer.new_slot, clip_legacy_id
                    )],
                );
                return None;
            }
            Some(LegacyCopyBufferRecord {
                slot: buffer.new_slot,
                clip_legacy_id: Some(clip_legacy_id),
                name: String::new(),
                updated_at_ms: Some(now_ms()),
                legacy_id: Some(buffer.legacy_id),
            })
        })
        .collect::<Vec<_>>();
    if !copy_buffers.is_empty() {
        storage.insert_legacy_batch(&LegacyBatch {
            copy_buffers,
            ..LegacyBatch::default()
        })?;
    }
    update_migration_state(&state, state_id, "copy_buffers", None)?;
    progress(MigrationProgress {
        stage: MigrationStage::CopyBuffers,
        processed: catalog.counts.copy_buffers,
        total: catalog.counts.copy_buffers,
        message: "copy buffers inserted".into(),
    });

    let settings = map_settings(source.settings.as_deref(), catalog)?;
    report.settings_mapped = settings.mapped_count;
    report.settings_unknown = settings.unknown.keys().cloned().collect();
    append_warnings(&mut report.warnings, settings.warnings);
    if settings.mapped_count > 0 {
        storage.set_settings(&settings.values)?;
    }
    let completed_at = now_ms();
    let mut migration_settings = Map::new();
    migration_settings.insert("completed".into(), Value::Bool(true));
    migration_settings.insert(
        "source_path".into(),
        Value::String(catalog.fingerprint.path.clone()),
    );
    migration_settings.insert(
        "source_sha256".into(),
        Value::String(catalog.fingerprint.sha256.clone()),
    );
    migration_settings.insert("completed_at_ms".into(), json!(completed_at));
    migration_settings.insert("dry_run".into(), Value::Bool(false));
    storage.set_setting("migration", Value::Object(migration_settings))?;
    write_meta(
        &state,
        "legacy_settings",
        &serde_json::to_string(&settings.unknown)?,
    )?;
    update_migration_state(&state, state_id, "settings", None)?;
    progress(MigrationProgress {
        stage: MigrationStage::Settings,
        processed: report.settings_mapped,
        total: report.settings_mapped,
        message: "legacy settings mapped".into(),
    });

    storage.optimize()?;
    storage.checkpoint()?;
    let imported = read_imported_counts(&state)?;
    let expected_buffers = catalog
        .copy_buffers
        .iter()
        .filter(|buffer| {
            buffer
                .clip_legacy_id
                .is_some_and(|clip| migrated_clip_ids.contains_key(&clip))
        })
        .count() as u64;
    if imported.clips != catalog.counts.clips {
        return Err(MigrationError::Checksum(format!(
            "clip count mismatch: expected {}, imported {}",
            catalog.counts.clips, imported.clips
        )));
    }
    if imported.groups != catalog.counts.groups {
        return Err(MigrationError::Checksum(format!(
            "group count mismatch: expected {}, imported {}",
            catalog.counts.groups, imported.groups
        )));
    }
    if imported.formats != expected_imported_formats {
        return Err(MigrationError::Checksum(format!(
            "format count mismatch: expected {}, imported {}",
            expected_imported_formats, imported.formats
        )));
    }
    if imported.copy_buffers != expected_buffers {
        return Err(MigrationError::Checksum(format!(
            "copy buffer count mismatch: expected {}, imported {}",
            expected_buffers, imported.copy_buffers
        )));
    }
    verify_sample(reader, &state, catalog.counts.clips)?;
    let quick_check: String = state.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if !quick_check.eq_ignore_ascii_case("ok") {
        return Err(MigrationError::Checksum(format!(
            "destination quick_check failed: {quick_check}"
        )));
    }
    report.imported_counts = imported;
    update_migration_state_with_counts(
        &state,
        state_id,
        "verified",
        &serde_json::to_string(&report.imported_counts)?,
    )?;
    progress(MigrationProgress {
        stage: MigrationStage::Verify,
        processed: report.imported_counts.clips,
        total: report.imported_counts.clips,
        message: "counts and deterministic samples verified".into(),
    });

    drop(state);
    drop(storage);
    let new_backup = staging
        .join("backups")
        .join(format!("ditto.post-migration.{}.db", completed_at));
    report
        .backups
        .push(snapshot_database(&staging.join("ditto.db"), &new_backup)?);
    report.duration_ms = started.elapsed().as_millis() as u64;
    write_json(&staging.join("migration-report.json"), &report)?;
    if !options.keep_backup {
        rotate_backups(&staging.join("backups"), 3)?;
    }
    Ok(MigrationOutcome {
        report,
        destination: staging.to_path_buf(),
    })
}

fn begin_migration_state(
    connection: &Connection,
    source: &MigrationSource,
    source_sha256: &str,
    started_at_ms: i64,
) -> MigrationResult<i64> {
    connection.execute(
        "INSERT INTO migration_state(
           source_path, source_sha256, dry_run, phase, started_at_ms, updated_at_ms
         ) VALUES (?1, ?2, 0, 'preflight', ?3, ?3)",
        rusqlite::params![
            source.database.display().to_string(),
            hex::decode(source_sha256)
                .map_err(|error| MigrationError::Invalid(error.to_string()))?,
            started_at_ms
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

fn update_migration_state(
    connection: &Connection,
    id: i64,
    phase: &str,
    error: Option<&str>,
) -> MigrationResult<()> {
    connection.execute(
        "UPDATE migration_state
         SET phase = ?1, updated_at_ms = ?2, error = ?3
         WHERE id = ?4",
        rusqlite::params![phase, now_ms(), error, id],
    )?;
    Ok(())
}

fn update_migration_state_with_counts(
    connection: &Connection,
    id: i64,
    phase: &str,
    counts_json: &str,
) -> MigrationResult<()> {
    let now = now_ms();
    connection.execute(
        "UPDATE migration_state
         SET phase = ?1, updated_at_ms = ?2, completed_at_ms = ?2, counts_json = ?3
         WHERE id = ?4",
        rusqlite::params![phase, now, counts_json, id],
    )?;
    Ok(())
}

fn write_meta(connection: &Connection, key: &str, value: &str) -> MigrationResult<()> {
    connection.execute(
        "INSERT INTO meta(key, value, updated_at_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                       updated_at_ms = excluded.updated_at_ms",
        rusqlite::params![key, value, now_ms()],
    )?;
    Ok(())
}

fn read_imported_counts(connection: &Connection) -> MigrationResult<ImportedCounts> {
    Ok(ImportedCounts {
        clips: query_count(
            connection,
            "SELECT COUNT(*) FROM clips WHERE legacy_id IS NOT NULL",
        )?,
        groups: query_count(
            connection,
            "SELECT COUNT(*) FROM groups WHERE legacy_id IS NOT NULL",
        )?,
        formats: query_count(
            connection,
            "SELECT COUNT(*) FROM clip_formats WHERE legacy_data_id IS NOT NULL",
        )?,
        copy_buffers: query_count(
            connection,
            "SELECT COUNT(*) FROM copy_buffers WHERE legacy_id IS NOT NULL AND clip_id IS NOT NULL",
        )?,
        pinned: query_count(connection, "SELECT COUNT(*) FROM clips WHERE is_pinned = 1")?,
    })
}

fn query_count(connection: &Connection, sql: &str) -> MigrationResult<u64> {
    let count: i64 = connection.query_row(sql, [], |row| row.get(0))?;
    Ok(count.max(0) as u64)
}

fn verify_sample(
    reader: &LegacySource,
    connection: &Connection,
    clips: u64,
) -> MigrationResult<()> {
    let sample_size = if clips <= 100 {
        clips as usize
    } else {
        (clips as usize / 100).clamp(100, 500)
    };
    for legacy_id in reader.sample_clip_ids(sample_size)? {
        let Some(row) = reader.clip_by_legacy_id(legacy_id)? else {
            continue;
        };
        let formats = reader.formats_for_clips(&[legacy_id])?;
        let mapped = map_clip(
            &row,
            formats.get(&legacy_id).map(Vec::as_slice).unwrap_or(&[]),
        );
        let expected = mapped.record;
        let actual = connection
            .query_row(
                "SELECT content_hash, formats_hash, is_pinned, pin_order, shortcut,
                        global_shortcut, last_pasted_at_ms, dont_auto_delete, legacy_crc,
                        quick_paste_text
                 FROM clips WHERE legacy_id = ?1",
                [legacy_id],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)? != 0,
                        row.get::<_, f64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, i64>(5)? != 0,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, i64>(7)? != 0,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, String>(9)?,
                    ))
                },
            )
            .map_err(|error| {
                MigrationError::Checksum(format!(
                    "sample clip {legacy_id} is missing after migration: {error}"
                ))
            })?;
        if actual.0 != expected.content_hash
            || actual.1 != expected.formats_hash
            || actual.2 != expected.is_pinned
            || (actual.3 - expected.pin_order).abs() > f64::EPSILON
            || actual.4 != expected.shortcut
            || actual.5 != expected.global_shortcut
            || actual.6 != expected.last_pasted_at_ms
            || actual.7 != expected.dont_auto_delete
            || actual.8 != expected.legacy_crc
            || actual.9 != expected.quick_paste_text
        {
            return Err(MigrationError::Checksum(format!(
                "sample clip {legacy_id} metadata or hashes differ"
            )));
        }
        let mut statement = connection.prepare(
            "SELECT legacy_data_id, format_name, sha256, size_bytes
             FROM clip_formats WHERE clip_id = (SELECT id FROM clips WHERE legacy_id = ?1)
             ORDER BY legacy_data_id",
        )?;
        let actual_formats = statement
            .query_map([legacy_id], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)? as u64,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for expected_format in &expected.formats {
            let Some(legacy_data_id) = expected_format.legacy_data_id else {
                continue;
            };
            let found = actual_formats.iter().find(|format| {
                format.0 == Some(legacy_data_id)
                    && format.1 == expected_format.format_name
                    && format.2 == Sha256::digest(&expected_format.data).to_vec()
                    && format.3 == expected_format.data.len() as u64
            });
            if found.is_none() {
                return Err(MigrationError::Checksum(format!(
                    "sample clip {legacy_id} format {} differs",
                    expected_format.format_name
                )));
            }
        }
        let expected_group: Option<i64> = connection.query_row(
            "SELECT g.legacy_id FROM clips c
             LEFT JOIN groups g ON g.id = c.group_id
             WHERE c.legacy_id = ?1",
            [legacy_id],
            |row| row.get(0),
        )?;
        if expected_group != expected.group_legacy_id {
            return Err(MigrationError::Checksum(format!(
                "sample clip {legacy_id} group mapping differs"
            )));
        }
    }
    Ok(())
}

fn copy_file_with_hash(source: &Path, target: &Path) -> MigrationResult<BackupRecord> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(target)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
        size += read as u64;
    }
    output.sync_all()?;
    Ok(BackupRecord {
        path: target.display().to_string(),
        sha256: hex::encode(digest.finalize()),
        size_bytes: size,
    })
}

fn copy_source_sidecars(source: &Path, target: &Path) -> MigrationResult<()> {
    for suffix in ["-wal", "-shm"] {
        let mut source_sidecar = source.as_os_str().to_os_string();
        source_sidecar.push(suffix);
        let source_sidecar = PathBuf::from(source_sidecar);
        if source_sidecar.is_file() {
            let mut target_sidecar = target.as_os_str().to_os_string();
            target_sidecar.push(suffix);
            fs::copy(source_sidecar, PathBuf::from(target_sidecar))?;
        }
    }
    Ok(())
}

fn package_backup_record(report: &crate::PackageReport) -> MigrationResult<BackupRecord> {
    let package = PathBuf::from(&report.package_path);
    let digest_path = if package.is_dir() {
        package.join("manifest.json")
    } else {
        package.clone()
    };
    let size_bytes = fs::metadata(&digest_path)?.len();
    Ok(BackupRecord {
        path: report.package_path.clone(),
        sha256: hex::encode(hash_file(&digest_path)?),
        size_bytes,
    })
}

fn append_warnings(target: &mut Vec<String>, warnings: Vec<String>) {
    const LIMIT: usize = 1_000;
    if target.len() >= LIMIT {
        return;
    }
    let remaining = LIMIT - target.len();
    target.extend(warnings.into_iter().take(remaining));
    if target.len() == LIMIT {
        target.push("additional migration warnings omitted".into());
    }
}

fn hash_file(path: &Path) -> MigrationResult<Vec<u8>> {
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
    Ok(digest.finalize().to_vec())
}

fn snapshot_database(source: &Path, target: &Path) -> MigrationResult<BackupRecord> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if target.exists() {
        fs::remove_file(target)?;
    }
    let connection = Connection::open(source)?;
    connection.backup(rusqlite::MAIN_DB, target, None)?;
    drop(connection);
    let fingerprint = crate::source::fingerprint(target)?;
    Ok(BackupRecord {
        path: target.display().to_string(),
        sha256: fingerprint.sha256,
        size_bytes: fingerprint.size_bytes,
    })
}

fn rotate_backups(directory: &Path, keep: usize) -> MigrationResult<()> {
    let mut files = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| *modified);
    if files.len() > keep {
        let remove_count = files.len().saturating_sub(keep);
        for (_, path) in files.into_iter().take(remove_count) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn verify_source_unchanged(
    reader: &LegacySource,
    expected: &crate::SourceFingerprint,
) -> MigrationResult<()> {
    let after = crate::source::fingerprint(&reader.database)?;
    if after.sha256 != expected.sha256 || after.size_bytes != expected.size_bytes {
        return Err(MigrationError::Checksum(format!(
            "legacy source changed during migration: before={} after={}",
            expected.sha256, after.sha256
        )));
    }
    Ok(())
}

fn path_is_nonempty(path: &Path) -> MigrationResult<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if !path.is_dir() {
        return Err(MigrationError::Conflict(format!(
            "{} exists and is not a directory",
            path.display()
        )));
    }
    Ok(fs::read_dir(path)?.next().transpose()?.is_some())
}

pub(crate) fn absolute_path(path: &Path) -> MigrationResult<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub(crate) fn unique_sibling(destination: &Path, prefix: &str) -> MigrationResult<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| MigrationError::Invalid("destination has no parent".into()))?;
    fs::create_dir_all(parent)?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| MigrationError::Invalid("destination has no file name".into()))?;
    for attempt in 0..1_000_u32 {
        let candidate = parent.join(format!(
            "{prefix}-{name}-{}-{}-{attempt}",
            std::process::id(),
            now_ms()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(MigrationError::Conflict(
        "could not allocate a unique staging directory".into(),
    ))
}

pub(crate) fn replace_destination(
    staging: &Path,
    destination: &Path,
    replace_existing: bool,
) -> MigrationResult<()> {
    if !destination.exists() {
        fs::rename(staging, destination)?;
        return Ok(());
    }
    if !destination.is_dir() {
        return Err(MigrationError::Conflict(format!(
            "{} is not a directory",
            destination.display()
        )));
    }
    if fs::read_dir(destination)?.next().is_none() {
        fs::remove_dir(destination)?;
        fs::rename(staging, destination)?;
        return Ok(());
    }
    if !replace_existing {
        return Err(MigrationError::Conflict(format!(
            "{} is not empty",
            destination.display()
        )));
    }
    let old = unique_sibling(destination, ".ditto-migrate-old")?;
    fs::rename(destination, &old)?;
    if let Err(error) = fs::rename(staging, destination) {
        fs::rename(&old, destination)?;
        return Err(error.into());
    }
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

pub(crate) fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> MigrationResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}
