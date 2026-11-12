//! Legacy Ditto data migration and new-format import/export.

mod error;
mod format;
mod ini;
mod migrate;
mod package;
mod source;
mod types;

pub use error::{MigrationError, MigrationResult};
pub use migrate::{MigrationOptions, MigrationOutcome, dry_run, migrate, migrate_with_progress};
pub use package::{
    ExportOptions, ImportOptions, PackageReport, backup, compact, create_export, import_package,
    restore, verify_package,
};
pub use source::{LegacyCatalog, LegacySource, SourceFingerprint, fingerprint};
pub use types::{
    ImportedCounts, MigrationProgress, MigrationReport, MigrationStage, PackageCounts, SourceCounts,
};

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MigrationSource {
    pub database: PathBuf,
    pub settings: Option<PathBuf>,
}
