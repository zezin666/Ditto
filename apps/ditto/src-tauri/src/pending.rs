//! Deferred data operations that must run before the database is opened.
//!
//! Restoring a backup or importing a package replaces the whole data root.
//! That cannot happen while the core holds SQLite handles, so the settings
//! commands stage the request next to the data root and the next core start
//! applies it before `Storage::open_data_root`.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

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
    /// Source counts from the staging dry run, used to show a real total while
    /// the one-shot migration waits for restart.
    #[serde(default)]
    pub clips: u64,
    #[serde(default)]
    pub groups: u64,
    #[serde(default)]
    pub formats: u64,
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

pub fn failed_marker(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.failed", path.display()))
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

/// Writes a staging marker atomically (temp file + rename), so a crash or
/// power loss leaves either the old state or the complete marker.
pub fn write_marker(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pending");
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

/// Applies all staged operations in order. Returns short descriptions of the
/// operations that ran or failed. A bad marker or package is moved to
/// `<marker>.failed` and never prevents the application from starting.
pub fn apply_staged(data_root: &Path) -> Vec<String> {
    let mut applied = Vec::new();
    for (marker, label, apply) in [
        (
            migration_marker(data_root),
            "legacy migration",
            apply_migration as fn(&Path) -> Result<(), String>,
        ),
        (
            import_marker(data_root),
            "data import/restore",
            apply_import as fn(&Path) -> Result<(), String>,
        ),
    ] {
        if !marker.is_file() {
            continue;
        }
        match apply(data_root) {
            Ok(()) => applied.push(format!("{label} applied")),
            Err(error) => {
                let failed = quarantine_marker(&marker);
                applied.push(format!(
                    "{label} failed ({error}); marker moved to {}",
                    failed.display()
                ));
            }
        }
    }
    applied
}

fn quarantine_marker(marker: &Path) -> PathBuf {
    let failed = failed_marker(marker);
    let _ = std::fs::remove_file(&failed);
    if std::fs::rename(marker, &failed).is_err() {
        let _ = std::fs::remove_file(marker);
    }
    failed
}

/// Deletes quarantined staging markers so the user can retry cleanly.
pub fn clear_failed(data_root: &Path) -> Vec<String> {
    let mut cleared = Vec::new();
    for marker in [migration_marker(data_root), import_marker(data_root)] {
        let failed = failed_marker(&marker);
        if failed.is_file() && std::fs::remove_file(&failed).is_ok() {
            cleared.push(failed.display().to_string());
        }
    }
    cleared
}

pub fn failed_marker_description(data_root: &Path) -> Option<String> {
    for marker in [migration_marker(data_root), import_marker(data_root)] {
        let failed = failed_marker(&marker);
        if failed.is_file() {
            return Some(failed.display().to_string());
        }
    }
    None
}

fn apply_migration(data_root: &Path) -> Result<(), String> {
    let marker_path = migration_marker(data_root);
    let text = std::fs::read_to_string(&marker_path)
        .map_err(|error| format!("read {}: {error}", marker_path.display()))?;
    let pending: PendingMigration = serde_json::from_str(&text)
        .map_err(|error| format!("parse {}: {error}", marker_path.display()))?;
    let database = PathBuf::from(&pending.source);
    if !database.is_file() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_writes_are_atomic_and_readable() {
        let temp = tempfile::tempdir().expect("temp");
        let marker = migration_marker(temp.path());
        write_marker(&marker, b"{\"source\":\"x\"}").expect("write");
        assert_eq!(std::fs::read(&marker).expect("read"), b"{\"source\":\"x\"}");
        write_marker(&marker, b"replacement").expect("replace");
        assert_eq!(std::fs::read(&marker).expect("read"), b"replacement");
        let leftovers = std::fs::read_dir(temp.path())
            .expect("dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn corrupt_marker_is_quarantined_and_startup_continues() {
        let temp = tempfile::tempdir().expect("temp");
        let marker = migration_marker(temp.path());
        std::fs::write(&marker, b"not json").expect("corrupt marker");
        let results = apply_staged(temp.path());
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("failed"));
        assert!(!marker.exists());
        assert!(failed_marker(&marker).is_file());
        assert!(failed_marker_description(temp.path()).is_some());
        let cleared = clear_failed(temp.path());
        assert_eq!(cleared.len(), 1);
        assert!(!failed_marker(&marker).exists());
    }

    #[test]
    fn missing_import_package_is_quarantined_instead_of_blocking_startup() {
        let temp = tempfile::tempdir().expect("temp");
        let marker = import_marker(temp.path());
        let pending = PendingImport {
            package: temp.path().join("missing.zip").display().to_string(),
            requested_at_ms: 1,
        };
        write_marker(&marker, &serde_json::to_vec(&pending).expect("json")).expect("marker");
        let results = apply_staged(temp.path());
        assert!(results[0].contains("failed"));
        assert!(!marker.exists());
        assert!(failed_marker(&marker).is_file());
    }
}
