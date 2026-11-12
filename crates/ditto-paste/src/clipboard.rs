use std::{ffi::OsStr, mem::size_of, os::windows::ffi::OsStrExt, thread, time::Duration};

use ditto_clipboard::{
    CaptureConfig, ClipboardSource, RawClipboardFormat, RawClipboardSnapshot,
    WindowsClipboardSource, build_captured_clip,
};
use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
                SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
    },
    core::PCWSTR,
};

use crate::{ClipboardBackend, PasteError, PasteFormat, PasteResult};

const CF_TEXT: u32 = 1;
const CF_OEMTEXT: u32 = 7;
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsClipboardBackend;

impl ClipboardBackend for WindowsClipboardBackend {
    fn snapshot(&self) -> PasteResult<Vec<PasteFormat>> {
        read_clipboard()
    }

    fn write(&self, formats: &[PasteFormat]) -> PasteResult<()> {
        write_clipboard(formats)
    }
}

pub fn read_clipboard() -> PasteResult<Vec<PasteFormat>> {
    let source = WindowsClipboardSource::new(CaptureConfig::default());
    let snapshot = source
        .read()
        .map_err(|error| PasteError::Clipboard(error.to_string()))?;
    Ok(snapshot
        .into_iter()
        .flat_map(|snapshot| snapshot.formats)
        .enumerate()
        .map(|(seq, format)| PasteFormat {
            name: format.name,
            data: format.data,
            seq: seq as u32,
            is_primary: format.is_primary,
        })
        .collect())
}

pub fn write_clipboard(formats: &[PasteFormat]) -> PasteResult<()> {
    if formats.is_empty() {
        return Err(PasteError::Clipboard(
            "refusing to clear the clipboard without a payload".to_string(),
        ));
    }
    open_clipboard_with_retry()?;
    let _guard = ClipboardGuard;
    unsafe { EmptyClipboard() }
        .map_err(|error| PasteError::Clipboard(format!("EmptyClipboard failed: {error}")))?;

    for format in formats {
        let format_id = clipboard_format_id(&format.name)?;
        let global = global_from_bytes(&format.data)?;
        let set = unsafe { SetClipboardData(format_id, Some(HANDLE(global.0))) };
        if let Err(error) = set {
            let _ = unsafe { GlobalFree(Some(global)) };
            return Err(PasteError::Clipboard(format!(
                "SetClipboardData {} failed: {error}",
                format.name
            )));
        }
    }
    Ok(())
}

pub(crate) fn suppression_hash(formats: &[PasteFormat]) -> PasteResult<[u8; 32]> {
    let snapshot = RawClipboardSnapshot {
        formats: formats
            .iter()
            .map(|format| RawClipboardFormat {
                name: format.name.clone(),
                data: format.data.clone(),
                is_primary: format.is_primary,
            })
            .collect(),
        source_app: None,
        source_window_title: None,
        warnings: Vec::new(),
    };
    build_captured_clip(snapshot, &CaptureConfig::default())
        .map(|clip| clip.content_hash)
        .map_err(|error| PasteError::Clipboard(error.to_string()))
}

fn clipboard_format_id(name: &str) -> PasteResult<u32> {
    let id = match name {
        "CF_TEXT" => CF_TEXT,
        "CF_OEMTEXT" => CF_OEMTEXT,
        "CF_UNICODETEXT" => CF_UNICODETEXT,
        "CF_DIB" => CF_DIB,
        "CF_DIBV5" => CF_DIBV5,
        "CF_HDROP" => CF_HDROP,
        "PNG" | "HTML Format" | "Rich Text Format" => register_format(name),
        custom if custom.starts_with("CF_") => {
            return Err(PasteError::Unsupported(format!(
                "unknown standard clipboard format {custom}"
            )));
        }
        custom => register_format(custom),
    };
    if id == 0 {
        Err(PasteError::Clipboard(format!(
            "RegisterClipboardFormatW failed for {name}"
        )))
    } else {
        Ok(id)
    }
}

fn register_format(name: &str) -> u32 {
    let wide = wide(name);
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

fn global_from_bytes(bytes: &[u8]) -> PasteResult<HGLOBAL> {
    let length = bytes.len().max(1);
    let global = unsafe { GlobalAlloc(GMEM_MOVEABLE, length) }
        .map_err(|error| PasteError::Clipboard(format!("GlobalAlloc failed: {error}")))?;
    let pointer = unsafe { GlobalLock(global) };
    if pointer.is_null() {
        let _ = unsafe { GlobalFree(Some(global)) };
        return Err(PasteError::Clipboard(
            "GlobalLock returned null".to_string(),
        ));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
    }
    unsafe { GlobalUnlock(global) }
        .map_err(|error| PasteError::Clipboard(format!("GlobalUnlock failed: {error}")))?;
    Ok(global)
}

fn open_clipboard_with_retry() -> PasteResult<()> {
    let mut last_error = None;
    for _ in 0..5 {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(10));
    }
    tracing::debug!(?last_error, "OpenClipboard remained unavailable");
    Err(PasteError::Busy)
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

const _: usize = size_of::<HANDLE>();
