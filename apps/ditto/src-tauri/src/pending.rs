//! Deferred data operations that must run before the database is opened.
//!
//! Restoring a backup or importing a package replaces the whole data root.
//! That cannot happen while the core holds SQLite handles, so the settings
//! commands stage the request next to the data root and the next core start
//! applies it before `Storage::open_data_root`.

use std::path::{Path, PathBuf};

use ditto_migration::{ImportOptions, MigrationOptions, MigrationSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingImport {
    pub package: String,
    pub requested_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMigration {
    pub source: String,
    pub settings: Option<String>,
    pub requested_at_ms: i64,
}

pub fn import_marker(data_root: &Path) -> PathBuf {
    sibling(data_root, ".pending-import.json")
}

pub fn import_package_path(data_root: &Path) -> PathBuf {
    sibling(data_root, ".pending-import.zip")
}

pub fn migration_marker(data_root: &Path) -> PathBuf {
    sibling(data_root, ".pending-migration.json")
}

fn sibling(data_root: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", data_root.display()))
}

pub fn backup_dir(data_root: &Path) -> PathBuf {
    data_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("ditto-backups")
}

/// Applies all staged operations in order. Returns short descriptions of the
/// operations that ran so the caller can log them.
pub fn apply_staged(data_root: &Path) -> Result<Vec<String>, String> {
    let mut applied = Vec::new();
    if migration_marker(data_root).is_file() {
        apply_migration(data_root)?;
        applied.push("legacy migration".to_string());
    }
    if import_marker(data_root).is_file() {
        apply_import(data_root)?;
        applied.push("data import/restore".to_string());
    }
    Ok(applied)
}

fn apply_migration(data_root: &Path) -> Result<(), String> {
    let marker_path = migration_marker(data_root);
    let text = std::fs::read_to_string(&marker_path)
        .map_err(|error| format!("read {}: {error}", marker_path.display()))?;
    let pending: PendingMigration = serde_json::from_str(&text)
        .map_err(|error| format!("parse {}: {error}", marker_path.display()))?;
    let database = PathBuf::from(&pending.source);
    if !database.is_file() {
        let _ = std::fs::remove_file(&marker_path);
        return Err(format!(
            "pending legacy database {} no longer exists",
            database.display()
        ));
    }
    let settings = pending
        .settings
        .map(PathBuf::from)
        .filter(|path| path.is_file());
    let options = MigrationOptions {
        backup_dir: Some(backup_dir(data_root)),
        keep_backup: true,
        replace_existing: true,
        ..MigrationOptions::default()
    };
    ditto_migration::migrate(&MigrationSource { database, settings }, data_root, &options)
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&marker_path);
    Ok(())
}

fn apply_import(data_root: &Path) -> Result<(), String> {
    let marker_path = import_marker(data_root);
    let text = std::fs::read_to_string(&marker_path)
        .map_err(|error| format!("read {}: {error}", marker_path.display()))?;
    let pending: PendingImport = serde_json::from_str(&text)
        .map_err(|error| format!("parse {}: {error}", marker_path.display()))?;
    let package = PathBuf::from(&pending.package);
    if !package.is_file() {
        let _ = std::fs::remove_file(&marker_path);
        return Err(format!(
            "pending package {} no longer exists",
            package.display()
        ));
    }
    let options = ImportOptions {
        replace_existing: true,
        backup_dir: Some(backup_dir(data_root)),
    };
    ditto_migration::import_package(&package, data_root, &options)
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&marker_path);
    let staged = import_package_path(data_root);
    if staged == package {
        let _ = std::fs::remove_file(&staged);
    }
    Ok(())
}
