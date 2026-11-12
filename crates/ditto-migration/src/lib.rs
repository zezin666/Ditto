//! Legacy data migration boundary. Fixture and mapping work belongs to T6.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MigrationSource {
    pub database: PathBuf,
    pub settings: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct MigrationPlan {
    pub source: Option<MigrationSource>,
    pub estimated_clips: u64,
}

pub fn dry_run(_source: &MigrationSource) -> ditto_model::Result<MigrationPlan> {
    Err(ditto_model::DittoError::NotImplemented(
        "migration preflight",
    ))
}
