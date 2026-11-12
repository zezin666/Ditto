use std::path::{Path, PathBuf};

use crate::{ElevationBackend, PasteError, PasteResult, TargetWindow};

#[derive(Debug, Clone)]
pub struct UacElevationBackend {
    helper_path: PathBuf,
}

impl UacElevationBackend {
    pub fn new(helper_path: PathBuf) -> Self {
        Self { helper_path }
    }

    pub fn discover() -> Self {
        let path = helper_path().unwrap_or_else(|| PathBuf::from("DittoUacHelper.exe"));
        Self::new(path)
    }
}

impl ElevationBackend for UacElevationBackend {
    fn paste_elevated(
        &self,
        target: &TargetWindow,
        clip_id: i64,
        mode: ditto_uac_helper::HelperMode,
        focus_delay_ms: u64,
    ) -> PasteResult<()> {
        ditto_uac_helper::launch_elevated_paste(
            &self.helper_path,
            target.hwnd,
            target.pid,
            clip_id,
            mode,
            focus_delay_ms,
        )
        .map_err(|error| PasteError::Elevation(error.to_string()))
    }
}

fn helper_path() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("DITTO_UAC_HELPER_PATH") {
        return Some(PathBuf::from(path));
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|directory| directory.join("DittoUacHelper.exe"))
}
