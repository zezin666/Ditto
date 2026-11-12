use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceCounts {
    pub clips: u64,
    pub groups: u64,
    pub formats: u64,
    pub copy_buffers: u64,
    pub types: u64,
    pub empty_formats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportedCounts {
    pub clips: u64,
    pub groups: u64,
    pub formats: u64,
    pub copy_buffers: u64,
    pub pinned: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStage {
    Preflight,
    Groups,
    Clips,
    CopyBuffers,
    Settings,
    Verify,
    Commit,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub stage: MigrationStage,
    pub processed: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub status: String,
    pub dry_run: bool,
    pub source_path: String,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub source_modified_ms: Option<i64>,
    pub destination: String,
    pub started_at_ms: i64,
    pub duration_ms: u64,
    pub source_counts: SourceCounts,
    pub imported_counts: ImportedCounts,
    pub format_distribution: BTreeMap<String, u64>,
    pub copy_buffer_slots: Vec<u8>,
    pub settings_mapped: u64,
    pub settings_unknown: Vec<String>,
    pub warnings: Vec<String>,
    pub backups: Vec<BackupRecord>,
    pub error: Option<String>,
}

impl MigrationReport {
    pub fn new(dry_run: bool) -> Self {
        Self {
            status: "running".into(),
            dry_run,
            source_path: String::new(),
            source_sha256: String::new(),
            source_size_bytes: 0,
            source_modified_ms: None,
            destination: String::new(),
            started_at_ms: 0,
            duration_ms: 0,
            source_counts: SourceCounts {
                clips: 0,
                groups: 0,
                formats: 0,
                copy_buffers: 0,
                types: 0,
                empty_formats: 0,
            },
            imported_counts: ImportedCounts {
                clips: 0,
                groups: 0,
                formats: 0,
                copy_buffers: 0,
                pinned: 0,
            },
            format_distribution: BTreeMap::new(),
            copy_buffer_slots: Vec::new(),
            settings_mapped: 0,
            settings_unknown: Vec::new(),
            warnings: Vec::new(),
            backups: Vec::new(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageCounts {
    pub clips: u64,
    pub groups: u64,
    pub formats: u64,
    pub copy_buffers: u64,
    pub settings: u64,
}
