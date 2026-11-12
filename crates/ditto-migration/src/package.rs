use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use ditto_model::SCHEMA_VERSION;
use ditto_storage::{APP_VERSION, Storage, now_ms};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    MigrationError, MigrationResult,
    migrate::{absolute_path, replace_destination, unique_sibling, write_json},
    types::PackageCounts,
};

const PACKAGE_FORMAT_VERSION: u32 = 1;
const MAX_ZIP_ENTRIES: usize = 100_000;
const MAX_ZIP_UNCOMPRESSED_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    pub zip: bool,
    pub replace_existing: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub replace_existing: bool,
    pub backup_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageReport {
    pub status: String,
    pub package_path: String,
    pub format_version: u32,
    pub schema_version: u32,
    pub verified: bool,
    pub counts: PackageCounts,
    pub files_verified: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    kind: String,
    format_version: u32,
    schema_version: u32,
    app_version: String,
    created_at_ms: i64,
    platform: String,
    counts: PackageCounts,
    database: FileDigest,
    blobs: Vec<FileDigest>,
    thumbs: Vec<FileDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileDigest {
    path: String,
    size_bytes: u64,
    sha256: String,
}

pub fn create_export(
    source_data_root: &Path,
    destination: &Path,
    options: &ExportOptions,
) -> MigrationResult<PackageReport> {
    create_package(source_data_root, destination, "export", options)
}

pub fn backup(
    source_data_root: &Path,
    backup_dir: &Path,
    zip: bool,
) -> MigrationResult<PackageReport> {
    fs::create_dir_all(backup_dir)?;
    let extension = if zip { "ditto.zip" } else { "ditto" };
    let destination = backup_dir.join(format!("ditto-backup-{}.{extension}", now_ms()));
    let options = ExportOptions {
        zip,
        replace_existing: false,
    };
    let mut report = create_package(source_data_root, &destination, "backup", &options)?;
    if let Err(error) = rotate_package_backups(backup_dir, 3) {
        report
            .warnings
            .push(format!("backup rotation failed: {error}"));
    }
    Ok(report)
}

pub fn verify_package(package: &Path) -> MigrationResult<PackageReport> {
    let package = absolute_path(package)?;
    if package.is_file() {
        let temp = TempDir::new()?;
        extract_zip(&package, temp.path())?;
        verify_package_dir(temp.path(), &package)
    } else if package.is_dir() {
        if package.join("manifest.json").is_file() {
            verify_package_dir(&package, &package)
        } else {
            verify_data_root(&package)
        }
    } else {
        Err(MigrationError::Invalid(format!(
            "{} is not a package file or directory",
            package.display()
        )))
    }
}

fn verify_data_root(root: &Path) -> MigrationResult<PackageReport> {
    let connection = open_package_database(root)?;
    let schema_version: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if schema_version > SCHEMA_VERSION {
        return Err(MigrationError::Unsupported(format!(
            "data root schema version {schema_version} is newer than supported {SCHEMA_VERSION}"
        )));
    }
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if !quick_check.eq_ignore_ascii_case("ok") {
        return Err(MigrationError::Checksum(format!(
            "data root quick_check failed: {quick_check}"
        )));
    }
    verify_referenced_files(root, &connection)?;
    let counts = database_counts(&connection)?;
    Ok(PackageReport {
        status: "verified".into(),
        package_path: root.display().to_string(),
        format_version: PACKAGE_FORMAT_VERSION,
        schema_version,
        verified: true,
        counts,
        files_verified: referenced_file_count(root, &connection)?,
        warnings: Vec::new(),
    })
}

pub fn import_package(
    package: &Path,
    destination: &Path,
    options: &ImportOptions,
) -> MigrationResult<PackageReport> {
    let package = absolute_path(package)?;
    let destination = absolute_path(destination)?;
    if destination.exists()
        && fs::read_dir(&destination)?.next().transpose()?.is_some()
        && !options.replace_existing
    {
        return Err(MigrationError::Conflict(format!(
            "destination {} is not empty; use --replace-existing",
            destination.display()
        )));
    }
    let staging = unique_sibling(&destination, ".ditto-import")?;
    if let Err(error) = copy_or_extract_package(&package, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let report = match verify_package_dir(&staging, &package) {
        Ok(report) => report,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    if destination.exists() && fs::read_dir(&destination)?.next().transpose()?.is_some() {
        let backup_dir = options.backup_dir.clone().unwrap_or_else(|| {
            destination
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("ditto-backups")
        });
        backup(&destination, &backup_dir, true)?;
    }
    if let Err(error) = replace_destination(&staging, &destination, options.replace_existing) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(PackageReport {
        package_path: destination.display().to_string(),
        status: "imported".into(),
        ..report
    })
}

pub fn restore(
    backup_package: &Path,
    destination: &Path,
    options: &ImportOptions,
) -> MigrationResult<PackageReport> {
    import_package(backup_package, destination, options)
}

pub fn compact(data_root: &Path) -> MigrationResult<PackageReport> {
    let data_root = absolute_path(data_root)?;
    let storage = Storage::open_data_root(&data_root)?;
    storage.checkpoint()?;
    drop(storage);
    let database = data_root.join("ditto.db");
    let connection = Connection::open(&database)?;
    connection.execute_batch("PRAGMA quick_check; VACUUM; PRAGMA optimize;")?;
    let counts = database_counts(&connection)?;
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if !quick_check.eq_ignore_ascii_case("ok") {
        return Err(MigrationError::Checksum(format!(
            "compact quick_check failed: {quick_check}"
        )));
    }
    Ok(PackageReport {
        status: "compacted".into(),
        package_path: data_root.display().to_string(),
        format_version: PACKAGE_FORMAT_VERSION,
        schema_version: SCHEMA_VERSION,
        verified: true,
        counts,
        files_verified: 1,
        warnings: Vec::new(),
    })
}

fn create_package(
    source_data_root: &Path,
    destination: &Path,
    kind: &str,
    options: &ExportOptions,
) -> MigrationResult<PackageReport> {
    let source_data_root = absolute_path(source_data_root)?;
    let destination = absolute_path(destination)?;
    let database = source_data_root.join("ditto.db");
    if !database.is_file() {
        return Err(MigrationError::Invalid(format!(
            "{} does not contain ditto.db",
            source_data_root.display()
        )));
    }
    let staging = unique_sibling(&destination, ".ditto-export")?;
    fs::create_dir_all(&staging)?;
    let result = build_package(&source_data_root, &staging, kind);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if options.zip {
        let temp_zip = unique_sibling(&destination, ".ditto-zip")?;
        if let Err(error) = zip_directory(&staging, &temp_zip) {
            let _ = fs::remove_dir_all(&staging);
            let _ = fs::remove_file(&temp_zip);
            return Err(error);
        }
        let _ = fs::remove_dir_all(&staging);
        if destination.exists() && !options.replace_existing {
            let _ = fs::remove_file(&temp_zip);
            return Err(MigrationError::Conflict(format!(
                "{} already exists",
                destination.display()
            )));
        }
        if destination.exists() {
            fs::remove_file(&destination)?;
        }
        fs::rename(temp_zip, &destination)?;
        verify_package(&destination)
    } else {
        if destination.exists() {
            if !options.replace_existing {
                let _ = fs::remove_dir_all(&staging);
                return Err(MigrationError::Conflict(format!(
                    "{} already exists",
                    destination.display()
                )));
            }
            replace_destination(&staging, &destination, true)?;
        } else {
            fs::rename(&staging, &destination)?;
        }
        verify_package(&destination)
    }
}

fn build_package(source_data_root: &Path, staging: &Path, kind: &str) -> MigrationResult<()> {
    let storage = Storage::open_data_root(source_data_root)?;
    storage.checkpoint()?;
    drop(storage);
    let source_database = source_data_root.join("ditto.db");
    let snapshot = staging.join("ditto.db");
    let source = Connection::open(&source_database)?;
    source.backup(rusqlite::MAIN_DB, &snapshot, None)?;
    drop(source);

    for directory in ["blobs", "thumbs"] {
        let from = source_data_root.join(directory);
        if from.is_dir() {
            copy_tree(&from, &staging.join(directory))?;
        }
    }
    let connection = open_package_database(staging)?;
    verify_referenced_files(staging, &connection)?;
    let counts = database_counts(&connection)?;
    let schema_version: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    drop(connection);
    let manifest = Manifest {
        kind: kind.to_string(),
        format_version: PACKAGE_FORMAT_VERSION,
        schema_version,
        app_version: APP_VERSION.into(),
        created_at_ms: now_ms(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        counts,
        database: file_digest(&snapshot, "ditto.db")?,
        blobs: directory_digests(staging, "blobs")?,
        thumbs: directory_digests(staging, "thumbs")?,
    };
    write_json(&staging.join("manifest.json"), &manifest)
}

fn verify_package_dir(root: &Path, display_path: &Path) -> MigrationResult<PackageReport> {
    let manifest_path = root.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(MigrationError::Invalid(
            "package manifest.json is missing".into(),
        ));
    }
    let manifest: Manifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.format_version != PACKAGE_FORMAT_VERSION {
        return Err(MigrationError::Unsupported(format!(
            "package format version {}",
            manifest.format_version
        )));
    }
    if manifest.schema_version > SCHEMA_VERSION {
        return Err(MigrationError::Unsupported(format!(
            "package schema version {} is newer than supported {}",
            manifest.schema_version, SCHEMA_VERSION
        )));
    }
    if manifest.database.path != "ditto.db" {
        return Err(MigrationError::Invalid(
            "manifest database path must be ditto.db".into(),
        ));
    }
    let mut files_verified = 0_usize;
    verify_file_digest(root, &manifest.database)?;
    files_verified += 1;
    for digest in manifest.blobs.iter().chain(&manifest.thumbs) {
        verify_file_digest(root, digest)?;
        files_verified += 1;
    }
    let connection = open_package_database(root)?;
    let schema_version: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if schema_version != manifest.schema_version {
        return Err(MigrationError::Checksum(format!(
            "manifest schema {} differs from database schema {schema_version}",
            manifest.schema_version
        )));
    }
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if !quick_check.eq_ignore_ascii_case("ok") {
        return Err(MigrationError::Checksum(format!(
            "package quick_check failed: {quick_check}"
        )));
    }
    verify_referenced_files(root, &connection)?;
    let counts = database_counts(&connection)?;
    if counts.clips != manifest.counts.clips
        || counts.groups != manifest.counts.groups
        || counts.formats != manifest.counts.formats
        || counts.copy_buffers != manifest.counts.copy_buffers
        || counts.settings != manifest.counts.settings
    {
        return Err(MigrationError::Checksum(
            "manifest counts differ from package database".into(),
        ));
    }
    Ok(PackageReport {
        status: "verified".into(),
        package_path: display_path.display().to_string(),
        format_version: manifest.format_version,
        schema_version: manifest.schema_version,
        verified: true,
        counts,
        files_verified,
        warnings: Vec::new(),
    })
}

fn open_package_database(root: &Path) -> MigrationResult<Connection> {
    let database = root.join("ditto.db");
    if !database.is_file() {
        return Err(MigrationError::Invalid(
            "package ditto.db is missing".into(),
        ));
    }
    let connection = Connection::open_with_flags(
        &database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok(connection)
}

fn database_counts(connection: &Connection) -> MigrationResult<PackageCounts> {
    Ok(PackageCounts {
        clips: query_count(connection, "SELECT COUNT(*) FROM clips")?,
        groups: query_count(connection, "SELECT COUNT(*) FROM groups")?,
        formats: query_count(connection, "SELECT COUNT(*) FROM clip_formats")?,
        copy_buffers: query_count(
            connection,
            "SELECT COUNT(*) FROM copy_buffers WHERE clip_id IS NOT NULL",
        )?,
        settings: query_count(connection, "SELECT COUNT(*) FROM settings")?,
    })
}

fn query_count(connection: &Connection, sql: &str) -> MigrationResult<u64> {
    let value: i64 = connection.query_row(sql, [], |row| row.get(0))?;
    Ok(value.max(0) as u64)
}

fn verify_referenced_files(root: &Path, connection: &Connection) -> MigrationResult<()> {
    let mut statement = connection
        .prepare("SELECT data_path, sha256 FROM clip_formats WHERE data_path IS NOT NULL")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let relative = row.get::<_, String>(0)?;
        let expected_hash = row.get::<_, Option<Vec<u8>>>(1)?.unwrap_or_default();
        let path = safe_join(root, &relative)?;
        if !path.is_file() {
            return Err(MigrationError::Checksum(format!(
                "package is missing referenced blob {relative}"
            )));
        }
        if !expected_hash.is_empty() && file_sha256(&path)? != expected_hash {
            return Err(MigrationError::Checksum(format!(
                "blob hash mismatch for {relative}"
            )));
        }
    }
    let mut statement =
        connection.prepare("SELECT thumb_path FROM clips WHERE thumb_path IS NOT NULL")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let relative: String = row.get(0)?;
        if !safe_join(root, &relative)?.is_file() {
            return Err(MigrationError::Checksum(format!(
                "package is missing referenced thumbnail {relative}"
            )));
        }
    }
    Ok(())
}

fn referenced_file_count(root: &Path, connection: &Connection) -> MigrationResult<usize> {
    let mut paths = std::collections::BTreeSet::new();
    let mut statement = connection.prepare(
        "SELECT data_path FROM clip_formats WHERE data_path IS NOT NULL
         UNION ALL SELECT thumb_path FROM clips WHERE thumb_path IS NOT NULL",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let relative: String = row.get(0)?;
        if safe_join(root, &relative)?.is_file() {
            paths.insert(relative);
        }
    }
    Ok(paths.len() + 1)
}

fn verify_file_digest(root: &Path, digest: &FileDigest) -> MigrationResult<()> {
    let path = safe_join(root, &digest.path)?;
    if !path.is_file() {
        return Err(MigrationError::Checksum(format!(
            "package file {} is missing",
            digest.path
        )));
    }
    let metadata = fs::metadata(&path)?;
    if metadata.len() != digest.size_bytes {
        return Err(MigrationError::Checksum(format!(
            "package file {} size mismatch",
            digest.path
        )));
    }
    if hex::encode(file_sha256(&path)?) != digest.sha256 {
        return Err(MigrationError::Checksum(format!(
            "package file {} hash mismatch",
            digest.path
        )));
    }
    Ok(())
}

fn safe_join(root: &Path, relative: &str) -> MigrationResult<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(MigrationError::Invalid(format!(
            "package path {relative:?} escapes the package root"
        )));
    }
    Ok(root.join(relative_path))
}

fn directory_digests(root: &Path, directory: &str) -> MigrationResult<Vec<FileDigest>> {
    let directory_root = root.join(directory);
    if !directory_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_files(&directory_root, &mut files)?;
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| MigrationError::Invalid("package path escaped root".into()))?;
            file_digest(&path, &relative.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

fn file_digest(path: &Path, relative: &str) -> MigrationResult<FileDigest> {
    Ok(FileDigest {
        path: relative.into(),
        size_bytes: fs::metadata(path)?.len(),
        sha256: hex::encode(file_sha256(path)?),
    })
}

fn file_sha256(path: &Path) -> MigrationResult<Vec<u8>> {
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

fn copy_tree(source: &Path, target: &Path) -> MigrationResult<()> {
    let mut files = Vec::new();
    collect_files(source, &mut files)?;
    for file in files {
        let relative = file
            .strip_prefix(source)
            .map_err(|_| MigrationError::Invalid("copy path escaped source".into()))?;
        let destination = target.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(file, destination)?;
    }
    Ok(())
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> MigrationResult<()> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                output.push(entry.path());
            }
        }
    }
    Ok(())
}

fn rotate_package_backups(directory: &Path, keep: usize) -> MigrationResult<()> {
    let mut entries = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("ditto-backup-")
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(modified, _)| *modified);
    let remove_count = entries.len().saturating_sub(keep);
    for (_, path) in entries.into_iter().take(remove_count) {
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn copy_or_extract_package(package: &Path, staging: &Path) -> MigrationResult<()> {
    if package.is_file() {
        extract_zip(package, staging)
    } else if package.is_dir() {
        fs::create_dir_all(staging)?;
        copy_tree(package, staging)
    } else {
        Err(MigrationError::Invalid(format!(
            "{} is not a package",
            package.display()
        )))
    }
}

fn zip_directory(source: &Path, destination: &Path) -> MigrationResult<()> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(destination)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut files = Vec::new();
    collect_files(source, &mut files)?;
    files.sort();
    for path in files {
        let relative = path
            .strip_prefix(source)
            .map_err(|_| MigrationError::Invalid("zip path escaped source".into()))?
            .to_string_lossy()
            .replace('\\', "/");
        zip.start_file(relative, options)
            .map_err(|error| MigrationError::Invalid(format!("zip write failed: {error}")))?;
        let mut input = File::open(path)?;
        std::io::copy(&mut input, &mut zip)?;
    }
    zip.finish()
        .map_err(|error| MigrationError::Invalid(format!("zip finish failed: {error}")))?;
    Ok(())
}

fn extract_zip(source: &Path, destination: &Path) -> MigrationResult<()> {
    let file = File::open(source)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| MigrationError::Invalid(format!("invalid zip: {error}")))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(MigrationError::Invalid(format!(
            "zip has {} entries; limit is {MAX_ZIP_ENTRIES}",
            archive.len()
        )));
    }
    fs::create_dir_all(destination)?;
    let mut extracted = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| MigrationError::Invalid(format!("zip read failed: {error}")))?;
        extracted = extracted.saturating_add(entry.size());
        if extracted > MAX_ZIP_UNCOMPRESSED_BYTES {
            return Err(MigrationError::Invalid(
                "zip uncompressed size exceeds limit".into(),
            ));
        }
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| MigrationError::Invalid("zip entry escapes package root".into()))?
            .to_path_buf();
        let target = destination.join(enclosed);
        if entry.is_dir() {
            fs::create_dir_all(target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(target)?;
        std::io::copy(&mut entry, &mut output)?;
        output.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ditto_model::{ClipKind, NewClipFormat};
    use serde_json::Value;

    fn synthetic_clip(text: &str) -> ditto_model::NewClip {
        let bytes = text.as_bytes().to_vec();
        let content_hash =
            Sha256::digest([b"CF_TEXT".as_slice(), &[0], bytes.as_slice()].concat()).to_vec();
        let formats_hash = Sha256::digest(&bytes).to_vec();
        ditto_model::NewClip {
            kind: ClipKind::Text,
            primary_format: Some("CF_TEXT".into()),
            text_content: Some(text.into()),
            text_truncated: false,
            preview_text: text.into(),
            quick_paste_text: String::new(),
            description: text.into(),
            source_app: None,
            source_window_title: None,
            content_hash,
            formats_hash,
            hash_algo: "sha256".into(),
            size_bytes: bytes.len() as u64,
            is_pinned: false,
            pin_order: 0.0,
            group_id: None,
            sort_order: 1.0,
            group_sort_order: 1.0,
            created_at_ms: 1,
            dont_auto_delete: false,
            has_image: false,
            thumbnail_png: None,
            formats: vec![NewClipFormat {
                format_name: "CF_TEXT".into(),
                seq: 0,
                data: bytes,
                mime: Some("text/plain".into()),
                is_primary: true,
            }],
        }
    }

    #[test]
    fn export_directory_round_trip_verifies() {
        let temp = TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let storage = Storage::open_data_root(&data).expect("storage");
        storage
            .insert_clip(&synthetic_clip("round trip"), false)
            .expect("insert");
        drop(storage);
        let package = temp.path().join("export");
        let report = create_export(
            &data,
            &package,
            &ExportOptions {
                zip: false,
                replace_existing: false,
            },
        )
        .expect("export");
        assert!(report.verified);
        assert_eq!(report.counts.clips, 1);
        let verified = verify_package(&package).expect("verify");
        assert_eq!(verified.counts.clips, 1);
    }

    #[test]
    fn zip_export_round_trip_verifies() {
        let temp = TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let storage = Storage::open_data_root(&data).expect("storage");
        storage
            .insert_clip(&synthetic_clip("zip round trip"), false)
            .expect("insert");
        drop(storage);
        let package = temp.path().join("export.zip");
        create_export(
            &data,
            &package,
            &ExportOptions {
                zip: true,
                replace_existing: false,
            },
        )
        .expect("export");
        let verified = verify_package(&package).expect("verify");
        assert_eq!(verified.counts.clips, 1);
    }

    #[test]
    fn manifest_hash_mismatch_is_rejected() {
        let temp = TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let storage = Storage::open_data_root(&data).expect("storage");
        storage
            .insert_clip(&synthetic_clip("manifest validation"), false)
            .expect("insert");
        drop(storage);
        let package = temp.path().join("export");
        create_export(&data, &package, &ExportOptions::default()).expect("export");
        let manifest_path = package.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
                .expect("manifest json");
        manifest["database"]["sha256"] = Value::String("00".repeat(32));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");
        let error = verify_package(&package).expect_err("hash mismatch");
        assert!(error.to_string().contains("hash mismatch"));
    }
}
