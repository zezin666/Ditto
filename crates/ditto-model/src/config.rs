use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::DittoError;

/// Resolved writable data location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataRoot {
    pub path: PathBuf,
    pub portable: bool,
}

impl DataRoot {
    /// Resolve the data root using the approved precedence:
    /// CLI, `DITTO_DATA_DIR`, portable marker, then LocalAppData.
    pub fn resolve(
        cli: Option<PathBuf>,
        portable_requested: bool,
        exe_dir: &Path,
    ) -> Result<Self, DittoError> {
        if let Some(path) = cli {
            return Ok(Self {
                path,
                portable: portable_requested,
            });
        }

        if let Some(path) = std::env::var_os("DITTO_DATA_DIR") {
            return Ok(Self {
                path: PathBuf::from(path),
                portable: portable_requested,
            });
        }

        let portable_marker = exe_dir.join("portable");
        if portable_requested || portable_marker.is_file() {
            return Ok(Self {
                path: exe_dir.join("Data"),
                portable: true,
            });
        }

        let local_app_data = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            DittoError::Configuration("LOCALAPPDATA is not available".to_string())
        })?;
        Ok(Self {
            path: PathBuf::from(local_app_data).join("Ditto").join("v2"),
            portable: false,
        })
    }

    pub fn run_dir(&self) -> PathBuf {
        self.path.join("run")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.path.join("logs")
    }
}

/// System/light/dark/high-contrast theme selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
    HighContrast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub data_root: DataRoot,
    pub theme: ThemeMode,
    pub hotkey: String,
    pub language: String,
}

impl AppConfig {
    pub fn development(data_root: DataRoot) -> Self {
        Self {
            data_root,
            theme: ThemeMode::System,
            hotkey: "Ctrl+Alt+D".to_string(),
            language: "en".to_string(),
        }
    }
}
