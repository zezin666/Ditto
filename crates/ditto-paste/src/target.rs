use std::{mem::size_of, path::Path, thread, time::Duration};

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND},
        Security::{
            GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_ELEVATION,
            TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenElevation, TokenIntegrityLevel,
        },
        System::{
            StationsAndDesktops::{
                CloseDesktop, GetThreadDesktop, GetUserObjectInformationW, UOI_NAME,
            },
            Threading::{
                AttachThreadInput, GetCurrentThreadId, OpenProcess, OpenProcessToken,
                PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
        UI::{
            Input::KeyboardAndMouse::SetFocus,
            WindowsAndMessaging::{
                BringWindowToTop, GUITHREADINFO, GetClassNameW, GetForegroundWindow,
                GetGUIThreadInfo, GetWindowThreadProcessId, IsIconic, IsWindow, SW_RESTORE,
                SetForegroundWindow, ShowWindow,
            },
        },
    },
    core::PWSTR,
};

use crate::{IntegrityLevel, PasteError, PasteResult, TargetBackend, TargetWindow};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsTargetBackend;

impl TargetBackend for WindowsTargetBackend {
    fn capture(&self) -> PasteResult<TargetWindow> {
        capture_foreground_window()
    }

    fn activate(&self, target: &TargetWindow) -> PasteResult<()> {
        activate_target(target)
    }

    fn is_current_process_elevated(&self) -> bool {
        process_integrity(std::process::id()).is_elevated_target()
    }
}

pub fn capture_foreground_window() -> PasteResult<TargetWindow> {
    if !desktop_is_default() {
        return Err(PasteError::SecureDesktop);
    }

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() || unsafe { !IsWindow(Some(hwnd)).as_bool() } {
        return Err(PasteError::NoTarget);
    }

    let mut pid = 0_u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 || thread_id == 0 {
        return Err(PasteError::NoTarget);
    }
    if pid == std::process::id() || is_ditto_process(pid) {
        return Err(PasteError::NoTarget);
    }

    let class_name = window_class_name(hwnd);
    if is_shell_surface(&class_name) {
        return Err(PasteError::NoTarget);
    }

    let mut info = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..GUITHREADINFO::default()
    };
    let focus_hwnd = if unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_ok()
        && !info.hwndFocus.0.is_null()
        && unsafe { IsWindow(Some(info.hwndFocus)).as_bool() }
    {
        info.hwndFocus
    } else {
        hwnd
    };

    Ok(TargetWindow {
        hwnd: hwnd.0 as isize,
        focus_hwnd: focus_hwnd.0 as isize,
        pid,
        thread_id,
        class_name,
        integrity: process_integrity(pid),
    })
}

pub fn activate_target(target: &TargetWindow) -> PasteResult<()> {
    let hwnd = HWND(target.hwnd as *mut _);
    if hwnd.0.is_null() || unsafe { !IsWindow(Some(hwnd)).as_bool() } {
        return Err(PasteError::TargetChanged);
    }
    if pid_for_window(hwnd) != target.pid {
        return Err(PasteError::TargetChanged);
    }

    if unsafe { IsIconic(hwnd).as_bool() } {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }

    let current_thread = unsafe { GetCurrentThreadId() };
    let foreground = unsafe { GetForegroundWindow() };
    let foreground_thread = if foreground.0.is_null() {
        0
    } else {
        unsafe { GetWindowThreadProcessId(foreground, None) }
    };
    let attach_foreground = foreground_thread != 0
        && foreground_thread != current_thread
        && unsafe { AttachThreadInput(current_thread, foreground_thread, true).as_bool() };
    let attach_target = target.thread_id != current_thread
        && unsafe { AttachThreadInput(current_thread, target.thread_id, true).as_bool() };

    let _ = unsafe { BringWindowToTop(hwnd) };
    let foreground_set = unsafe { SetForegroundWindow(hwnd).as_bool() };
    let focus = HWND(target.focus_hwnd as *mut _);
    if !focus.0.is_null() && unsafe { IsWindow(Some(focus)).as_bool() } {
        let _ = unsafe { SetFocus(Some(focus)) };
    }

    if attach_target {
        let _ = unsafe { AttachThreadInput(current_thread, target.thread_id, false) };
    }
    if attach_foreground {
        let _ = unsafe { AttachThreadInput(current_thread, foreground_thread, false) };
    }

    for _ in 0..25 {
        if unsafe { GetForegroundWindow() } == hwnd {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    if foreground_set {
        Err(PasteError::TargetChanged)
    } else {
        Err(PasteError::NoTarget)
    }
}

pub fn process_integrity(pid: u32) -> IntegrityLevel {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let Ok(process) = process else {
        return IntegrityLevel::Unknown;
    };
    let _process = OwnedHandle(process);

    let mut token = HANDLE::default();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.is_err() {
        return IntegrityLevel::Unknown;
    }
    let _token = OwnedHandle(token);

    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0_u32;
    if unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    }
    .is_ok()
        && elevation.TokenIsElevated != 0
    {
        return IntegrityLevel::High;
    }

    integrity_from_token(token)
}

fn integrity_from_token(token: HANDLE) -> IntegrityLevel {
    let mut needed = 0_u32;
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut needed) };
    if needed == 0 {
        return IntegrityLevel::Unknown;
    }
    let mut buffer = vec![0_u8; needed as usize];
    if unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
    }
    .is_err()
    {
        return IntegrityLevel::Unknown;
    }
    let label = unsafe { &*(buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()) };
    let count = unsafe { *GetSidSubAuthorityCount(label.Label.Sid) } as u32;
    if count == 0 {
        return IntegrityLevel::Unknown;
    }
    let rid = unsafe { *GetSidSubAuthority(label.Label.Sid, count - 1) };
    match rid {
        0..=0x0FFF => IntegrityLevel::Untrusted,
        0x1000..=0x1FFF => IntegrityLevel::Low,
        0x2000..=0x2FFF => IntegrityLevel::Medium,
        0x3000..=0x3FFF => IntegrityLevel::High,
        _ => IntegrityLevel::System,
    }
}

fn window_class_name(hwnd: HWND) -> String {
    let mut buffer = [0_u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

fn pid_for_window(hwnd: HWND) -> u32 {
    let mut pid = 0_u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    pid
}

fn is_ditto_process(pid: u32) -> bool {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let Ok(process) = process else {
        return false;
    };
    let _process = OwnedHandle(process);
    let mut buffer = vec![0_u16; 32 * 1024];
    let mut size = buffer.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    }
    .is_err()
    {
        return false;
    }
    let image = String::from_utf16_lossy(&buffer[..size as usize]);
    is_ditto_process_image(&image)
}

fn is_ditto_process_image(image: &str) -> bool {
    let Some(name) = Path::new(image).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "ditto.exe" | "dittopopup.exe" | "dittosettings.exe" | "dittouachelper.exe"
    )
}

fn is_shell_surface(class_name: &str) -> bool {
    matches!(
        class_name,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "TrayNotifyWnd"
            | "NotifyIconOverflowWindow"
            | "Winlogon"
    )
}

fn desktop_is_default() -> bool {
    let Ok(desktop) = (unsafe { GetThreadDesktop(GetCurrentThreadId()) }) else {
        return false;
    };
    let mut needed = 0_u32;
    let _ = unsafe {
        GetUserObjectInformationW(HANDLE(desktop.0), UOI_NAME, None, 0, Some(&mut needed))
    };
    if needed < 2 {
        let _ = unsafe { CloseDesktop(desktop) };
        return false;
    }
    let mut buffer = vec![0_u16; (needed as usize).div_ceil(2)];
    let read = unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            Some(&mut needed),
        )
    };
    let _ = unsafe { CloseDesktop(desktop) };
    if read.is_err() {
        return false;
    }
    let null = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..null]) == "Default"
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[allow(dead_code)]
fn _pwstr_is_imported(_value: PWSTR) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_surfaces_are_not_paste_targets() {
        assert!(is_shell_surface("Progman"));
        assert!(is_shell_surface("Shell_TrayWnd"));
        assert!(is_shell_surface("Winlogon"));
        assert!(!is_shell_surface("Notepad"));
    }

    #[test]
    fn integrity_thresholds_match_windows_rids() {
        let levels = [
            (0x0000, IntegrityLevel::Untrusted),
            (0x1000, IntegrityLevel::Low),
            (0x2000, IntegrityLevel::Medium),
            (0x3000, IntegrityLevel::High),
            (0x4000, IntegrityLevel::System),
        ];
        for (rid, expected) in levels {
            let actual = match rid {
                0..=0x0FFF => IntegrityLevel::Untrusted,
                0x1000..=0x1FFF => IntegrityLevel::Low,
                0x2000..=0x2FFF => IntegrityLevel::Medium,
                0x3000..=0x3FFF => IntegrityLevel::High,
                _ => IntegrityLevel::System,
            };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn own_process_images_are_not_valid_paste_targets() {
        assert!(is_ditto_process_image(r"C:\Program Files\Ditto\Ditto.exe"));
        assert!(is_ditto_process_image(r"C:\Ditto\DittoPopup.exe"));
        assert!(!is_ditto_process_image(r"C:\Windows\notepad.exe"));
    }
}
