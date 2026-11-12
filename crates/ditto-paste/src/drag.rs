use std::path::PathBuf;

use ditto_clipboard::WriteSuppression;
use ditto_clipboard::hdrop_paths;
use windows::{
    Win32::{
        Foundation::{
            DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, HWND, S_OK,
        },
        System::{
            Com::IDataObject,
            Ole::{
                DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, DoDragDrop, IDropSource,
                IDropSource_Impl, OleGetClipboard, OleInitialize, OleUninitialize,
            },
            SystemServices::{MK_LBUTTON, MK_RBUTTON, MODIFIERKEYS_FLAGS},
        },
    },
    core::{BOOL, HRESULT, implement},
};

use crate::{
    ClipboardBackend, PasteError, PasteFormat, PasteResult, WindowsClipboardBackend,
    clipboard::suppression_hash, validate_file_paths,
};

pub fn drag_out(
    formats: &[PasteFormat],
    _owner_hwnd: isize,
    suppression: &WriteSuppression,
) -> PasteResult<bool> {
    if formats.is_empty() {
        return Err(PasteError::Unsupported(
            "cannot drag an empty payload".to_string(),
        ));
    }
    suppression.remember(suppression_hash(formats)?);
    WindowsClipboardBackend.write(formats)?;
    let ole = OleSession::start()?;
    let data_object = unsafe { OleGetClipboard() }
        .map_err(|error| PasteError::Clipboard(format!("OleGetClipboard failed: {error}")))?;
    let source: IDropSource = SimpleDropSource.into();
    let mut effect = DROPEFFECT_NONE;
    let result = unsafe {
        DoDragDrop(
            &data_object,
            &source,
            DROPEFFECT_COPY,
            &mut effect as *mut DROPEFFECT,
        )
    };
    drop(ole);
    if result.is_err() {
        return Err(PasteError::Clipboard(format!(
            "DoDragDrop failed: {result:?}"
        )));
    }
    Ok(effect.0 & DROPEFFECT_COPY.0 != 0)
}

pub fn parse_file_drop(bytes: &[u8]) -> PasteResult<Vec<PathBuf>> {
    let paths = hdrop_paths(bytes)
        .map_err(|error| PasteError::InvalidFiles(error.to_string()))?
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    validate_file_paths(&paths)?;
    Ok(paths)
}

#[implement(IDropSource)]
struct SimpleDropSource;

impl IDropSource_Impl for SimpleDropSource_Impl {
    fn QueryContinueDrag(&self, escape_pressed: BOOL, key_state: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape_pressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        if key_state.0 & (MK_LBUTTON.0 | MK_RBUTTON.0) == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

struct OleSession;

impl OleSession {
    fn start() -> PasteResult<Self> {
        unsafe { OleInitialize(None) }
            .map_err(|error| PasteError::Clipboard(format!("OleInitialize failed: {error}")))?;
        Ok(Self)
    }
}

impl Drop for OleSession {
    fn drop(&mut self) {
        unsafe { OleUninitialize() };
    }
}

#[allow(dead_code)]
fn _data_object_type(_value: &IDataObject, _hwnd: HWND) {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::build_file_drop;

    #[test]
    fn file_drop_receiver_only_returns_existing_paths() {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        path.push(format!("ditto-t4-drop-{stamp}.txt"));
        fs::write(&path, b"QA sample 002").expect("fixture");
        let data = build_file_drop(std::slice::from_ref(&path)).expect("drop");
        let parsed = parse_file_drop(&data).expect("receive");
        assert_eq!(parsed, vec![path.clone()]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn malformed_file_drop_is_rejected() {
        assert!(matches!(
            parse_file_drop(&[1, 2, 3]),
            Err(PasteError::InvalidFiles(_))
        ));
    }

    #[test]
    fn existing_path_round_trip_is_stable() {
        let path = std::env::temp_dir();
        let data = build_file_drop(std::slice::from_ref(&path)).expect("directory drop");
        let parsed = parse_file_drop(&data).expect("receive");
        assert_eq!(parsed, vec![path]);
    }
}
