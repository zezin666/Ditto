//! Narrow Win32 helpers shared by the Ditto host and resident popup.

use std::{
    collections::HashMap,
    ffi::OsStr,
    io::{self, Read, Write},
    os::windows::ffi::OsStrExt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use ditto_ipc::{
    ClientKind, ErrorCode, Event, Frame, FrameType, IpcError, Request, Response, read_frame,
    write_frame,
};
use ditto_model::{PROTOCOL_VERSION, SCHEMA_VERSION};
use parking_lot::Mutex;
use windows::{
    Win32::{
        Foundation::{
            COLORREF, CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_ALREADY_EXISTS,
            ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE,
            GetLastError, HANDLE, HINSTANCE, LPARAM, LRESULT, LocalFree, RECT, WAIT_OBJECT_0,
            WAIT_TIMEOUT, WPARAM,
        },
        Graphics::Gdi::{
            BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, COLOR_WINDOWTEXT,
            CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_END_ELLIPSIS,
            DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint,
            FW_NORMAL, FillRect, GetSysColor, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT,
            SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE,
            FILE_FLAG_OVERLAPPED, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING,
            PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            LibraryLoader::GetModuleHandleW,
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_TYPE_BYTE, PIPE_WAIT, PeekNamedPipe, WaitNamedPipeW,
            },
            Registry::{
                HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
                RegGetValueW,
            },
            Threading::{
                CreateEventW, GetCurrentProcess, INFINITE, OpenProcessToken, SetEvent,
                WaitForSingleObject,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{
                HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
            },
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
                DispatchMessageW, FindWindowW, GWL_EXSTYLE, GetClientRect, GetMessageW,
                GetWindowLongPtrW, HWND_TOPMOST, KillTimer, LWA_ALPHA, MB_ICONWARNING, MB_OK, MSG,
                MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW, SPI_GETWORKAREA,
                SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                SWP_SHOWWINDOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetLayeredWindowAttributes,
                SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, SystemParametersInfoW,
                TranslateMessage, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WM_TIMER,
                WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, PWSTR, w},
};

const MAX_INSTANCES: u32 = 32;
const PIPE_BUFFER_SIZE: u32 = 64 * 1024;
/// A slow or hostile pipe client must never block the core's capture callback.
/// Overlapped writes are cancelled after this budget and the client is dropped.
const WRITE_TIMEOUT_MS: u32 = 250;
const WEBVIEW2_CLIENT_KEY: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// Returns the canonical SID string for the current process token.
pub fn current_user_sid_string() -> ditto_model::Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            windows::Win32::Security::TOKEN_QUERY,
            &mut token,
        )
        .map_err(|error| windows_error("OpenProcessToken", error))?;
        let _token = OwnedHandle(token);

        let mut size = 0_u32;
        let _ = windows::Win32::Security::GetTokenInformation(
            token,
            windows::Win32::Security::TokenUser,
            None,
            size,
            &mut size,
        );
        let mut buffer = vec![0_u8; size as usize];
        windows::Win32::Security::GetTokenInformation(
            token,
            windows::Win32::Security::TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        )
        .map_err(|error| windows_error("GetTokenInformation", error))?;

        let token_user = &*(buffer
            .as_ptr()
            .cast::<windows::Win32::Security::TOKEN_USER>());
        let mut sid = PWSTR::null();
        ConvertSidToStringSidW(token_user.User.Sid, &mut sid)
            .map_err(|error| windows_error("ConvertSidToStringSidW", error))?;
        let sid_string = sid.to_string().map_err(|error| {
            ditto_model::DittoError::Windows(format!("SID conversion failed: {error}"))
        })?;
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(sid.0.cast())));
        Ok(sid_string)
    }
}

/// Exit code used when an isolation override is present but malformed.
///
/// The process refuses to start instead of silently falling back to the
/// production pipe and single-instance names. Falling back could signal a
/// running user instance and violates the QA isolation guarantee.
pub const INVALID_SUFFIX_EXIT_CODE: i32 = 2;

/// Maximum accepted QA/development suffix length.
pub const MAX_INSTANCE_SUFFIX_LEN: usize = 48;

/// A malformed `DITTO_PIPE_SUFFIX`/`DITTO_INSTANCE_ID` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidInstanceSuffix {
    pub variable: &'static str,
    pub reason: String,
}

impl std::fmt::Display for InvalidInstanceSuffix {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} is invalid: {}; use 1-{} characters from [A-Za-z0-9_-]",
            self.variable, self.reason, MAX_INSTANCE_SUFFIX_LEN
        )
    }
}

impl std::error::Error for InvalidInstanceSuffix {}

/// Validates an isolation suffix read from the environment.
///
/// `None` means "not configured" and keeps the production names. A present
/// variable is never sanitized or truncated: whitespace-only, over-long, or
/// out-of-charset values are rejected so a malformed QA override cannot touch
/// a real user instance.
pub fn parse_instance_suffix(
    variable: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, InvalidInstanceSuffix> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(InvalidInstanceSuffix {
            variable,
            reason: "the value is empty".to_string(),
        });
    }
    if trimmed.len() > MAX_INSTANCE_SUFFIX_LEN {
        return Err(InvalidInstanceSuffix {
            variable,
            reason: format!("the value is {} bytes long", trimmed.len()),
        });
    }
    if let Some(character) = trimmed
        .chars()
        .find(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    {
        return Err(InvalidInstanceSuffix {
            variable,
            reason: format!("it contains {character:?}"),
        });
    }
    Ok(Some(trimmed.to_string()))
}

/// Optional QA/development isolation suffix.
///
/// `DITTO_PIPE_SUFFIX` (or the alias `DITTO_INSTANCE_ID`) appends a validated
/// token to the pipe and single-instance names. It never weakens the pipe ACL:
/// the descriptor still grants access only to SYSTEM and the current SID.
/// A present-but-invalid variable is an error, not a silent fallback.
pub fn instance_suffix() -> Result<Option<String>, InvalidInstanceSuffix> {
    let variables: [&'static str; 2] = ["DITTO_PIPE_SUFFIX", "DITTO_INSTANCE_ID"];
    for variable in variables {
        if std::env::var_os(variable).is_none() {
            continue;
        }
        let value = std::env::var_os(variable)
            .expect("checked above")
            .into_string()
            .map_err(|_| InvalidInstanceSuffix {
                variable,
                reason: "the value is not valid Unicode".to_string(),
            })?;
        return parse_instance_suffix(variable, Some(&value));
    }
    Ok(None)
}

pub fn pipe_name() -> ditto_model::Result<String> {
    let sid = current_user_sid_string()?;
    let suffix = instance_suffix().map_err(|error| {
        ditto_model::DittoError::Configuration(format!("refusing to start: {error}"))
    })?;
    Ok(match suffix {
        Some(suffix) => format!(r"\\.\pipe\ditto-{sid}-{suffix}"),
        None => format!(r"\\.\pipe\ditto-{sid}"),
    })
}

/// Per-user Win32 auto-reset event used as a fast single-instance signal.
///
/// The first process creates the event; later processes detect the existing
/// object and signal it instead of initializing the whole Tauri application.
pub struct SingleInstanceSignal {
    raw: isize,
    already_running: bool,
}

// The handle is an owned Win32 event handle. `SetEvent`,
// `WaitForSingleObject`, and `CloseHandle` are safe to call from any thread.
unsafe impl Send for SingleInstanceSignal {}
unsafe impl Sync for SingleInstanceSignal {}

impl SingleInstanceSignal {
    pub fn create(name: &str) -> ditto_model::Result<Self> {
        let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
        let handle =
            unsafe { CreateEventW(None, false, false, windows::core::PCWSTR(wide.as_ptr())) }
                .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
        let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        Ok(Self {
            raw: handle.0 as isize,
            already_running,
        })
    }

    pub const fn already_running(&self) -> bool {
        self.already_running
    }

    pub fn signal(&self) {
        unsafe {
            let _ = SetEvent(self.handle());
        }
    }

    /// Waits up to `timeout_ms`; `INFINITE` blocks until the event is set.
    pub fn wait(&self, timeout_ms: u32) -> bool {
        unsafe { WaitForSingleObject(self.handle(), timeout_ms) == WAIT_OBJECT_0 }
    }

    pub fn wait_forever(&self) -> bool {
        self.wait(INFINITE)
    }

    const fn handle(&self) -> HANDLE {
        HANDLE(self.raw as *mut std::ffi::c_void)
    }
}

impl Drop for SingleInstanceSignal {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle());
        }
    }
}

pub fn system_theme() -> ditto_model::ThemeMode {
    let mut value = 1_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    if status.is_ok() && value == 0 {
        ditto_model::ThemeMode::Dark
    } else {
        ditto_model::ThemeMode::Light
    }
}

pub fn hotkey_available(modifiers: u32, virtual_key: u32) -> bool {
    const PROBE_ID: i32 = 0xD170;
    unsafe {
        let registered = RegisterHotKey(
            None,
            PROBE_ID,
            HOT_KEY_MODIFIERS(modifiers | MOD_NOREPEAT.0),
            virtual_key,
        )
        .is_ok();
        if registered {
            let _ = UnregisterHotKey(None, PROBE_ID);
        }
        registered
    }
}

/// Returns the installed WebView2 Runtime version, if any.
///
/// The Evergreen bootstrapper writes the client version under `pv` for both
/// machine-wide and per-user installs; the machine-wide key is redirected for
/// 32-bit processes, so both registry views are checked.
pub fn webview2_version() -> Option<String> {
    let machine_views = [
        format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_KEY}"),
        format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_KEY}"),
    ];
    let user_view = format!(r"Software\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_KEY}");
    for key in &machine_views {
        if let Some(version) = registry_string(HKEY_LOCAL_MACHINE, key, "pv") {
            return Some(version);
        }
    }
    registry_string(HKEY_CURRENT_USER, &user_view, "pv")
}

/// True when either the Evergreen Runtime is registered or an installed
/// `msedgewebview2.exe` exists. Registry data can be absent on stripped images,
/// so the install directory is also checked.
pub fn webview2_available() -> bool {
    if webview2_version().is_some() {
        return true;
    }
    for root in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        let Some(base) = std::env::var_os(root) else {
            continue;
        };
        let candidate = std::path::PathBuf::from(base)
            .join("Microsoft")
            .join("EdgeWebView")
            .join("Application");
        if candidate
            .read_dir()
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| entry.path().join("msedgewebview2.exe").is_file())
        {
            return true;
        }
    }
    false
}

fn registry_string(
    root: windows::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<String> {
    let key_wide = wide(key);
    let value_wide = wide(value);
    let mut buffer = [0_u16; 128];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(key_wide.as_ptr()),
            PCWSTR(value_wide.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() || size == 0 {
        return None;
    }
    let units = size as usize / 2;
    let end = buffer[..units.min(buffer.len())]
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.min(buffer.len()));
    Some(String::from_utf16_lossy(&buffer[..end]))
}

/// Shows a native blocking message box. Used when WebView2 is unavailable and
/// the Settings window cannot be created.
pub fn show_message_box(title: &str, text: &str, warning: bool) {
    let title_wide = wide(title);
    let text_wide = wide(text);
    let flags = if warning {
        MB_OK | MB_ICONWARNING
    } else {
        MB_OK
    };
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(text_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            flags,
        );
    }
}

static NATIVE_HINT_TEXT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static NATIVE_HINT_VISIBLE: AtomicBool = AtomicBool::new(false);

fn hint_trace(message: &str) {
    if let Ok(path) = std::env::var("DITTO_HINT_TRACE") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{} hint:{message}", unix_millis());
        }
    }
}

/// Shows a small top-most, non-activating native hint near the bottom-right
/// work-area corner. Used when the tray icon is hidden, so the user still gets
/// visible feedback while the resident popup is cold-starting.
///
/// Returns after the window has been created; the helper thread owns the
/// window and closes it after `duration_ms`. The call never blocks the caller
/// on a user action and never steals focus.
///
/// The boolean is `true` only when this call created the hint window. An empty
/// message or an already visible hint returns `Ok(false)`, so the caller can
/// tell whether it owns the window and may dismiss it later.
pub fn show_native_hint(text: &str, duration_ms: u32) -> ditto_model::Result<bool> {
    let text = text.trim().to_string();
    if text.is_empty() {
        hint_trace("skip-empty");
        return Ok(false);
    }
    if NATIVE_HINT_VISIBLE.swap(true, Ordering::SeqCst) {
        hint_trace("skip-active");
        return Ok(false);
    }
    hint_trace(&format!("show len={} duration={duration_ms}", text.len()));
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
    let spawn = thread::Builder::new()
        .name("ditto-native-hint".to_string())
        .spawn(move || {
            if let Ok(mut current) = NATIVE_HINT_TEXT.lock() {
                *current = text.clone();
            }
            let result = unsafe { run_native_hint(&text, duration_ms, &ready_tx) };
            NATIVE_HINT_VISIBLE.store(false, Ordering::SeqCst);
            if let Err(error) = result {
                hint_trace(&format!("thread-error {error}"));
                let _ = ready_tx.send(Err(error));
            } else {
                hint_trace("thread-closed");
            }
        });
    if let Err(error) = spawn {
        NATIVE_HINT_VISIBLE.store(false, Ordering::SeqCst);
        hint_trace(&format!("spawn-error {error}"));
        return Err(ditto_model::DittoError::Windows(format!(
            "failed to spawn native hint thread: {error}"
        )));
    }
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(true),
        Ok(Err(error)) => {
            hint_trace(&format!("ready-error {error}"));
            Err(ditto_model::DittoError::Windows(error))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(
            ditto_model::DittoError::Windows("native hint thread ended early".to_string()),
        ),
    }
}

/// Closes the native hint window immediately, if one is visible.
///
/// Used when the popup finishes initializing, so the "preparing" hint does not
/// linger on top of the popup it was covering. The owning thread receives
/// `WM_CLOSE` through its message queue; calling this from any other thread is
/// safe and never blocks.
pub fn dismiss_native_hint() {
    let Ok(hwnd) = (unsafe { FindWindowW(w!("DittoNativeHint"), PCWSTR::null()) }) else {
        return;
    };
    hint_trace("dismiss");
    let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
}

unsafe fn run_native_hint(
    text: &str,
    duration_ms: u32,
    ready: &std::sync::mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let module = unsafe { GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
    let instance = HINSTANCE::from(module);
    let class_name = w!("DittoNativeHint");
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(native_hint_proc),
        hInstance: instance,
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe {
        // A previous hint may still be tearing down; an existing class is fine.
        let _ = RegisterClassW(&window_class);
    }

    let mut work = RECT {
        left: 0,
        top: 0,
        right: 1280,
        bottom: 720,
    };
    let _ = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut work as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    let width = 400
        .min((work.right - work.left).saturating_sub(32))
        .max(160);
    let height = 64;
    let x = work.right - width - 16;
    let y = work.bottom - height - 16;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            class_name,
            w!("Ditto"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    hint_trace(&format!("window-created hwnd={:?}", hwnd.0));
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 235, LWA_ALPHA);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(Some(hwnd), 1, duration_ms.max(500), None);
    }
    if let Ok(mut current) = NATIVE_HINT_TEXT.lock() {
        *current = text.to_string();
    }
    let _ = ready.send(Ok(()));

    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn native_hint_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            unsafe { paint_native_hint(hwnd) };
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_TIMER => {
            unsafe {
                let _ = KillTimer(Some(hwnd), wparam.0);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn paint_native_hint(hwnd: windows::Win32::Foundation::HWND) {
    let mut paint = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    if hdc.0.is_null() {
        return;
    }
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let background = unsafe { CreateSolidBrush(COLORREF(GetSysColor(COLOR_WINDOW))) };
    unsafe {
        FillRect(hdc, &client, background);
        let _ = DeleteObject(background.into());
    }
    let font = unsafe {
        CreateFontW(
            -17,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    };
    let previous = unsafe { SelectObject(hdc, font.into()) };
    unsafe {
        SetTextColor(hdc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
        SetBkMode(hdc, TRANSPARENT);
    }
    let mut text = NATIVE_HINT_TEXT
        .lock()
        .map(|text| OsStr::new(text.as_str()).encode_wide().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut bounds = RECT {
        left: 16,
        top: 10,
        right: client.right - 16,
        bottom: client.bottom - 10,
    };
    unsafe {
        DrawTextW(
            hdc,
            &mut text,
            &mut bounds,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(hdc, previous);
        let _ = DeleteObject(font.into());
        let _ = EndPaint(hwnd, &paint);
    }
}

/// Counts direct `msedgewebview2.exe` child processes for diagnostics.
pub fn webview_process_count() -> usize {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    let Ok(snapshot) = snapshot else {
        return 0;
    };
    let _snapshot = OwnedHandle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut count = 0_usize;
    let current = std::process::id();
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
        return 0;
    }
    loop {
        if entry.th32ParentProcessID == current {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry
                    .szExeFile
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(entry.szExeFile.len())],
            );
            if name.eq_ignore_ascii_case("msedgewebview2.exe") {
                count += 1;
            }
        }
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    count
}

/// Hide a pre-warmed top-level window from the taskbar without activation.
pub fn make_popup_window(hwnd: windows::Win32::Foundation::HWND) -> ditto_model::Result<()> {
    if hwnd.0.is_null() {
        return Err(ditto_model::DittoError::Windows(
            "popup HWND is not available".to_string(),
        ));
    }
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
        );
        // Keep the window alive and paintable while invisible: hide it with a
        // layered alpha of zero instead of SW_HIDE. winit stops delivering
        // redraws to natively hidden windows, which would swallow IPC Shows.
        set_layered_visibility(hwnd, false);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
    Ok(())
}

fn set_layered_visibility(hwnd: windows::Win32::Foundation::HWND, visible: bool) {
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let base = ex_style | WS_EX_NOACTIVATE.0 as isize | WS_EX_LAYERED.0 as isize;
        let next = if visible {
            base & !(WS_EX_TRANSPARENT.0 as isize)
        } else {
            base | WS_EX_TRANSPARENT.0 as isize
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        let _ = SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            if visible { 255 } else { 0 },
            LWA_ALPHA,
        );
    }
}

pub fn set_popup_visible(hwnd: windows::Win32::Foundation::HWND, visible: bool) {
    unsafe {
        set_layered_visibility(hwnd, visible);
        if visible {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

pub fn resize_popup(
    hwnd: windows::Win32::Foundation::HWND,
    width: i32,
    height: i32,
) -> ditto_model::Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .map_err(|error| windows_error("SetWindowPos", error))
    }
}

/// Named-pipe endpoint owned by the Tauri host.
#[derive(Clone)]
pub struct PipeServer {
    clients: Arc<Mutex<HashMap<u64, Arc<Connection>>>>,
    next_event: Arc<AtomicU64>,
    presence: PopupPresence,
    visibility_hook: Option<VisibilityHook>,
}

/// Shared popup lifecycle state. Kept in one struct so the pipe server and its
/// per-connection threads can be passed as one value.
#[derive(Clone)]
struct PopupPresence {
    visible: Arc<AtomicBool>,
    clients: Arc<AtomicU64>,
    /// True only after a popup client reported that its window exists.
    /// `clients` covers IPC prewarm, which happens well before the eframe
    /// window/GL context is usable.
    window_ready: Arc<AtomicBool>,
}

impl PopupPresence {
    fn new() -> Self {
        Self {
            visible: Arc::new(AtomicBool::new(false)),
            clients: Arc::new(AtomicU64::new(0)),
            window_ready: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub type RequestHandler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;
/// Called synchronously before a show/hide event is broadcast. The core uses
/// it to snapshot (or clear) the user's foreground window before the popup can
/// take keyboard focus.
pub type VisibilityHook = Arc<dyn Fn(bool) + Send + Sync>;

impl PipeServer {
    pub fn start(name: &str) -> ditto_model::Result<Self> {
        Self::start_internal(name, None, None)
    }

    pub fn start_with_handler<F>(name: &str, handler: F) -> ditto_model::Result<Self>
    where
        F: Fn(&Request) -> Response + Send + Sync + 'static,
    {
        Self::start_internal(name, Some(Arc::new(handler)), None)
    }

    /// Starts the server with both a request handler and a show/hide hook.
    pub fn start_with_handler_and_visibility<F, V>(
        name: &str,
        handler: F,
        visibility: V,
    ) -> ditto_model::Result<Self>
    where
        F: Fn(&Request) -> Response + Send + Sync + 'static,
        V: Fn(bool) + Send + Sync + 'static,
    {
        Self::start_internal(name, Some(Arc::new(handler)), Some(Arc::new(visibility)))
    }

    fn start_internal(
        name: &str,
        handler: Option<RequestHandler>,
        visibility_hook: Option<VisibilityHook>,
    ) -> ditto_model::Result<Self> {
        let name = name.to_string();
        let clients = Arc::new(Mutex::new(HashMap::<u64, Arc<Connection>>::new()));
        let presence = PopupPresence::new();
        let first = Connection::create_server_instance(&name, true)?;
        let clients_for_thread = Arc::clone(&clients);
        let handler_for_thread = handler.clone();
        let presence_for_thread = presence.clone();
        let hook_for_thread = visibility_hook.clone();

        thread::Builder::new()
            .name("ditto-ipc-server".to_string())
            .spawn(move || {
                accept_loop(
                    name,
                    first,
                    clients_for_thread,
                    handler_for_thread,
                    presence_for_thread,
                    hook_for_thread,
                )
            })
            .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;

        Ok(Self {
            clients,
            next_event: Arc::new(AtomicU64::new(1)),
            presence,
            visibility_hook,
        })
    }

    pub fn broadcast(&self, event: &Event) -> ditto_model::Result<()> {
        let sequence = self.next_event.fetch_add(1, Ordering::Relaxed);
        let frame = Frame::json(FrameType::Event, sequence, event)
            .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
        let clients: Vec<(u64, Arc<Connection>)> = self
            .clients
            .lock()
            .iter()
            .map(|(id, connection)| (*id, Arc::clone(connection)))
            .collect();
        for (id, client) in clients {
            if let Err(error) = client.write_frame(&frame) {
                // A client that stopped reading must be disconnected; otherwise
                // its stuck writer would keep the queue full forever.
                tracing::debug!(%error, client_id = id, "IPC event delivery failed; dropping client");
                self.clients.lock().remove(&id);
                client.mark_closed();
            }
        }
        Ok(())
    }

    pub fn popup_visible(&self) -> bool {
        self.presence.visible.load(Ordering::SeqCst)
    }

    pub fn popup_client_count(&self) -> u64 {
        self.presence.clients.load(Ordering::SeqCst)
    }

    /// True when a popup process reported that its window exists and can
    /// render. IPC prewarm (`popup_client_count`) is not enough: the eframe
    /// window/GL context is created after the handshake.
    pub fn popup_window_ready(&self) -> bool {
        self.presence.window_ready.load(Ordering::SeqCst)
    }

    /// Updates the shared popup state and broadcasts the matching control
    /// event. Clients that connect later query the state so a slow popup start
    /// cannot lose the first hotkey press.
    pub fn set_popup_visible(&self, visible: bool) -> ditto_model::Result<()> {
        if let Some(hook) = &self.visibility_hook {
            hook(visible);
        }
        self.presence.visible.store(visible, Ordering::SeqCst);
        let event = if visible {
            Event::ShowPopup {
                at_ms: unix_millis(),
                monitor: None,
            }
        } else {
            Event::HidePopup
        };
        self.broadcast(&event)
    }

    pub fn client_count(&self) -> usize {
        self.clients.lock().len()
    }
}

struct Connection {
    read_handle: SharedHandle,
    write_handle: SharedHandle,
    write_lock: Mutex<()>,
    id: u64,
    client_kind: Mutex<Option<ClientKind>>,
    closed: AtomicBool,
}

impl Connection {
    fn create_server_instance(name: &str, first_instance: bool) -> ditto_model::Result<Self> {
        let sid = current_user_sid_string()?;
        let sddl = pipe_security_descriptor(&sid);
        Self::create_server_instance_with_sddl(name, first_instance, &sddl)
    }

    fn create_server_instance_with_sddl(
        name: &str,
        first_instance: bool,
        sddl: &str,
    ) -> ditto_model::Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let sddl_wide = wide(sddl);
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                windows::core::PCWSTR(sddl_wide.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .map_err(|error| {
                windows_error(
                    "ConvertStringSecurityDescriptorToSecurityDescriptorW",
                    error,
                )
            })?;
        }

        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let mut access = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
        if first_instance {
            access |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }

        let name_wide = wide(name);
        let handle = unsafe {
            CreateNamedPipeW(
                windows::core::PCWSTR(name_wide.as_ptr()),
                access,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                MAX_INSTANCES,
                PIPE_BUFFER_SIZE,
                PIPE_BUFFER_SIZE,
                0,
                Some(&attributes),
            )
        };
        unsafe {
            let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(descriptor.0)));
        }
        if handle == INVALID_HANDLE_VALUE {
            return Err(windows_last_error("CreateNamedPipeW"));
        }

        let read_handle = SharedHandle(handle);
        let write_handle = duplicate_handle(read_handle.0)?;
        Ok(Self {
            read_handle,
            write_handle: SharedHandle(write_handle),
            write_lock: Mutex::new(()),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            client_kind: Mutex::new(None),
            closed: AtomicBool::new(false),
        })
    }

    fn mark_closed(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn connect(&self) -> ditto_model::Result<()> {
        // The instance was created with FILE_FLAG_OVERLAPPED, so
        // ConnectNamedPipe must receive an OVERLAPPED structure.
        let event = unsafe { CreateEventW(None, true, false, None) }
            .map_err(|error| ditto_model::DittoError::Windows(error.to_string()))?;
        let _event = OwnedHandle(event);
        let mut overlapped = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        let result = unsafe { ConnectNamedPipe(self.read_handle.0, Some(&mut overlapped)) };
        if result.is_ok() {
            return Ok(());
        }
        let code = unsafe { GetLastError() };
        if code == ERROR_PIPE_CONNECTED {
            // A client connected between CreateNamedPipeW and ConnectNamedPipe.
            return Ok(());
        }
        if code == ERROR_IO_PENDING {
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                unsafe {
                    let _ = CancelIoEx(self.read_handle.0, Some(&overlapped));
                }
                return Err(ditto_model::DittoError::Windows(
                    "ConnectNamedPipe wait failed".to_string(),
                ));
            }
            let mut transferred = 0_u32;
            unsafe {
                GetOverlappedResult(self.read_handle.0, &overlapped, &mut transferred, false)
            }
            .map_err(|error| {
                ditto_model::DittoError::Windows(format!("ConnectNamedPipe: {error}"))
            })?;
            return Ok(());
        }
        Err(ditto_model::DittoError::Windows(format!(
            "ConnectNamedPipe failed: {code:?}"
        )))
    }

    fn read_frame(&self) -> Result<Frame, IpcError> {
        read_frame(&mut PipeReader(self.read_handle.0))
    }

    fn write_frame(&self, frame: &Frame) -> Result<(), IpcError> {
        let _guard = self.write_lock.lock();
        if self.closed.load(Ordering::SeqCst) {
            return Err(IpcError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "IPC connection is closed",
            )));
        }
        write_frame(&mut PipeWriter(self.write_handle.0), frame)
    }
}

/// The main IPC pipe is readable only by SYSTEM and the current user.
/// Administrators are deliberately not granted access: the pipe carries clip
/// summaries and events for this user's session only.
fn pipe_security_descriptor(current_user_sid: &str) -> String {
    format!("D:P(A;;GA;;;SY)(A;;GA;;;{current_user_sid})")
}

fn accept_loop(
    name: String,
    mut current: Connection,
    clients: Arc<Mutex<HashMap<u64, Arc<Connection>>>>,
    handler: Option<RequestHandler>,
    presence: PopupPresence,
    visibility_hook: Option<VisibilityHook>,
) {
    loop {
        if let Err(error) = current.connect() {
            tracing::error!(%error, "named pipe accept failed");
            thread::sleep(Duration::from_millis(250));
            match Connection::create_server_instance(&name, false) {
                Ok(next) => current = next,
                Err(error) => {
                    tracing::error!(%error, "failed to recreate named pipe instance");
                    return;
                }
            }
            continue;
        }

        let connection = Arc::new(current);
        clients
            .lock()
            .insert(connection.id, Arc::clone(&connection));
        let clients_for_handler = Arc::clone(&clients);
        let handler_for_connection = handler.clone();
        let presence_for_connection = presence.clone();
        let hook_for_connection = visibility_hook.clone();
        thread::spawn(move || {
            handle_connection(
                connection,
                clients_for_handler,
                handler_for_connection,
                presence_for_connection,
                hook_for_connection,
            )
        });

        match Connection::create_server_instance(&name, false) {
            Ok(next) => current = next,
            Err(error) => {
                tracing::error!(%error, "failed to create next named pipe instance");
                return;
            }
        }
    }
}

fn handle_connection(
    connection: Arc<Connection>,
    clients: Arc<Mutex<HashMap<u64, Arc<Connection>>>>,
    handler: Option<RequestHandler>,
    presence: PopupPresence,
    visibility_hook: Option<VisibilityHook>,
) {
    loop {
        if connection.closed.load(Ordering::SeqCst) {
            break;
        }
        match pipe_available(connection.read_handle.0) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Ok(_) => {}
            Err(_) => break,
        }
        let frame = match connection.read_frame() {
            Ok(frame) => frame,
            Err(IpcError::Io(error))
                if error.raw_os_error() == Some(ERROR_BROKEN_PIPE.0 as i32) =>
            {
                break;
            }
            Err(error) => {
                tracing::debug!(%error, "IPC connection ended");
                break;
            }
        };
        if frame.frame_type != FrameType::Request {
            break;
        }

        let request = match frame.decode_request() {
            Ok(request) => request,
            Err(error) => {
                let response = Response::Error {
                    code: ErrorCode::BadFrame,
                    message: error.to_string(),
                    retryable: false,
                };
                let _ = send_response(&connection, frame.request_id, &response);
                break;
            }
        };

        let response = dispatch_request(
            &request,
            &clients,
            handler.as_deref(),
            &presence,
            visibility_hook.as_deref(),
        );
        if let Request::Hello { client, .. } = &request {
            if *client == ClientKind::Popup {
                presence.clients.fetch_add(1, Ordering::SeqCst);
            }
            *connection.client_kind.lock() = Some(*client);
        }
        if send_response(&connection, frame.request_id, &response).is_err() {
            break;
        }
        if matches!(request, Request::Shutdown) {
            break;
        }
    }
    clients.lock().remove(&connection.id);
    connection.mark_closed();
    if connection.client_kind.lock().take() == Some(ClientKind::Popup)
        && presence.clients.fetch_sub(1, Ordering::SeqCst) == 1
    {
        // The resident popup process is gone (crash or exit). Do not leave the
        // core believing it is still visible, or the next hotkey would toggle
        // it hidden instead of showing it. Window readiness must reset too:
        // the next popup process starts with IPC prewarm, not a usable window.
        presence.visible.store(false, Ordering::SeqCst);
        presence.window_ready.store(false, Ordering::SeqCst);
    }
}

fn dispatch_request(
    request: &Request,
    clients: &Arc<Mutex<HashMap<u64, Arc<Connection>>>>,
    handler: Option<&(dyn Fn(&Request) -> Response + Send + Sync)>,
    presence: &PopupPresence,
    visibility_hook: Option<&(dyn Fn(bool) + Send + Sync)>,
) -> Response {
    match request {
        Request::Hello {
            proto_min,
            proto_max,
            client,
            ..
        } => {
            if *proto_max < PROTOCOL_VERSION || *proto_min > PROTOCOL_VERSION {
                return Response::Error {
                    code: ErrorCode::Protocol,
                    message: format!(
                        "client supports {proto_min}..{proto_max}, core supports {PROTOCOL_VERSION}"
                    ),
                    retryable: false,
                };
            }
            tracing::debug!(?client, "IPC client connected");
            Response::HelloAck {
                proto: PROTOCOL_VERSION,
                core_pid: std::process::id(),
                session: std::env::var("SESSIONNAME").unwrap_or_default(),
                schema_version: SCHEMA_VERSION,
                capabilities: vec![
                    "clipboard".to_string(),
                    "groups".to_string(),
                    "copy_buffers".to_string(),
                    "preview".to_string(),
                    "uac_paste".to_string(),
                ],
                state_rev: 0,
                theme_rev: 0,
            }
        }
        Request::ShowPopup => {
            if let Some(hook) = visibility_hook {
                hook(true);
            }
            presence.visible.store(true, Ordering::SeqCst);
            let event = Event::ShowPopup {
                at_ms: unix_millis(),
                monitor: None,
            };
            let frame = match Frame::json(FrameType::Event, unix_millis(), &event) {
                Ok(frame) => frame,
                Err(error) => {
                    return Response::Error {
                        code: ErrorCode::Internal,
                        message: error.to_string(),
                        retryable: true,
                    };
                }
            };
            for client in clients.lock().values() {
                let _ = client.write_frame(&frame);
            }
            Response::PopupVisibility { visible: true }
        }
        Request::HidePopup => {
            if let Some(hook) = visibility_hook {
                hook(false);
            }
            presence.visible.store(false, Ordering::SeqCst);
            let event = Event::HidePopup;
            let frame = match Frame::json(FrameType::Event, unix_millis(), &event) {
                Ok(frame) => frame,
                Err(error) => {
                    return Response::Error {
                        code: ErrorCode::Internal,
                        message: error.to_string(),
                        retryable: true,
                    };
                }
            };
            for client in clients.lock().values() {
                let _ = client.write_frame(&frame);
            }
            Response::PopupVisibility { visible: false }
        }
        Request::GetPopupVisibility => Response::PopupVisibility {
            visible: presence.visible.load(Ordering::SeqCst),
        },
        Request::PopupWindowReady => {
            presence.window_ready.store(true, Ordering::SeqCst);
            Response::Ok
        }
        _ => handler.map_or_else(
            || Response::Error {
                code: ErrorCode::NotImplemented,
                message: "message handler is not configured".to_string(),
                retryable: false,
            },
            |handler| handler(request),
        ),
    }
}

fn send_response(
    connection: &Connection,
    request_id: u64,
    response: &Response,
) -> Result<(), IpcError> {
    let frame = Frame::json(FrameType::Response, request_id, response)?;
    connection.write_frame(&frame)
}

/// Blocking client connection used by the resident popup.
pub struct PipeClient {
    connection: Connection,
}

impl PipeClient {
    pub fn connect(name: &str) -> ditto_model::Result<Self> {
        let name_wide = wide(name);
        let mut last_error = None;
        for _ in 0..20 {
            let handle = unsafe {
                CreateFileW(
                    windows::core::PCWSTR(name_wide.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0 | FILE_FLAG_OVERLAPPED.0),
                    None,
                )
            };
            match handle {
                Ok(handle) => {
                    let read_handle = SharedHandle(handle);
                    let write_handle = duplicate_handle(read_handle.0)?;
                    return Ok(Self {
                        connection: Connection {
                            read_handle,
                            write_handle: SharedHandle(write_handle),
                            write_lock: Mutex::new(()),
                            id: 0,
                            client_kind: Mutex::new(None),
                            closed: AtomicBool::new(false),
                        },
                    });
                }
                Err(error) => last_error = Some(error.to_string()),
            }
            unsafe {
                let _ = WaitNamedPipeW(windows::core::PCWSTR(name_wide.as_ptr()), 500);
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(ditto_model::DittoError::Ipc(format!(
            "unable to connect to {name}: {last_error:?}"
        )))
    }

    pub fn send(&self, request_id: u64, request: &Request) -> ditto_model::Result<()> {
        let frame = Frame::json(FrameType::Request, request_id, request)
            .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
        self.connection
            .write_frame(&frame)
            .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))
    }

    pub fn read_response(&self) -> ditto_model::Result<Response> {
        loop {
            let frame = self
                .connection
                .read_frame()
                .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
            if frame.frame_type == FrameType::Response {
                return frame
                    .decode_response()
                    .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()));
            }
        }
    }

    pub fn read_event(&self) -> ditto_model::Result<Event> {
        loop {
            let frame = self
                .connection
                .read_frame()
                .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
            if frame.frame_type == FrameType::Event {
                return frame
                    .decode_event()
                    .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()));
            }
        }
    }
}

pub fn hello_request(client: ClientKind) -> Request {
    Request::Hello {
        proto_min: PROTOCOL_VERSION,
        proto_max: PROTOCOL_VERSION,
        client,
        pid: std::process::id(),
        nonce: format!("{}-{}", std::process::id(), unix_millis()),
        subscribe: true,
    }
}

pub fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

struct PipeReader(HANDLE);

impl Read for PipeReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        overlapped_read(self.0, buffer)
    }
}

struct PipeWriter(HANDLE);

impl Write for PipeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        overlapped_write(self.0, buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn overlapped_read(handle: HANDLE, buffer: &mut [u8]) -> io::Result<usize> {
    let event = unsafe { CreateEventW(None, true, false, None) }
        .map_err(|error| io::Error::other(error.to_string()))?;
    let _event = OwnedHandle(event);
    let mut overlapped = OVERLAPPED {
        hEvent: event,
        ..Default::default()
    };
    let mut direct_read = 0_u32;
    let result = unsafe {
        ReadFile(
            handle,
            Some(buffer),
            Some(&mut direct_read),
            Some(&mut overlapped),
        )
    };
    match result {
        Ok(()) => Ok(direct_read as usize),
        Err(error) if error.code() == ERROR_IO_PENDING.into() => {
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                unsafe {
                    let _ = CancelIoEx(handle, Some(&overlapped));
                }
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "IPC read did not complete",
                ));
            }
            let mut transferred = 0_u32;
            unsafe { GetOverlappedResult(handle, &overlapped, &mut transferred, false) }
                .map_err(|error| io::Error::other(error.to_string()))?;
            Ok(transferred as usize)
        }
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}

fn overlapped_write(handle: HANDLE, buffer: &[u8]) -> io::Result<usize> {
    let event = unsafe { CreateEventW(None, true, false, None) }
        .map_err(|error| io::Error::other(error.to_string()))?;
    let _event = OwnedHandle(event);
    let mut overlapped = OVERLAPPED {
        hEvent: event,
        ..Default::default()
    };
    let mut direct_written = 0_u32;
    let result = unsafe {
        WriteFile(
            handle,
            Some(buffer),
            Some(&mut direct_written),
            Some(&mut overlapped),
        )
    };
    match result {
        Ok(()) => Ok(direct_written as usize),
        Err(error) if error.code() == ERROR_IO_PENDING.into() => {
            let wait = unsafe { WaitForSingleObject(event, WRITE_TIMEOUT_MS) };
            if wait == WAIT_TIMEOUT {
                // Cancel the pending write and let the caller drop this
                // connection. A client that stops reading must not block the
                // core's capture callback indefinitely.
                unsafe {
                    let _ = CancelIoEx(handle, Some(&overlapped));
                }
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "IPC write timed out; slow client disconnected",
                ));
            }
            if wait != WAIT_OBJECT_0 {
                unsafe {
                    let _ = CancelIoEx(handle, Some(&overlapped));
                }
                return Err(io::Error::other("IPC write wait failed"));
            }
            let mut transferred = 0_u32;
            unsafe { GetOverlappedResult(handle, &overlapped, &mut transferred, false) }
                .map_err(|error| io::Error::other(error.to_string()))?;
            Ok(transferred as usize)
        }
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}

struct SharedHandle(HANDLE);

unsafe impl Send for SharedHandle {}
unsafe impl Sync for SharedHandle {}

impl Drop for SharedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

const INVALID_HANDLE_VALUE: HANDLE = HANDLE(-1_isize as *mut std::ffi::c_void);

fn duplicate_handle(handle: HANDLE) -> ditto_model::Result<HANDLE> {
    let process = unsafe { GetCurrentProcess() };
    let mut duplicate = HANDLE::default();
    unsafe {
        DuplicateHandle(
            process,
            handle,
            process,
            &mut duplicate,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
        .map_err(|error| windows_error("DuplicateHandle", error))?;
    }
    Ok(duplicate)
}

fn pipe_available(handle: HANDLE) -> ditto_model::Result<u32> {
    let mut available = 0_u32;
    unsafe {
        PeekNamedPipe(handle, None, 0, None, Some(&mut available), None)
            .map_err(|error| windows_error("PeekNamedPipe", error))?;
    }
    Ok(available)
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn windows_error(context: &str, error: windows::core::Error) -> ditto_model::DittoError {
    ditto_model::DittoError::Windows(format!("{context}: {error}"))
}

fn windows_last_error(context: &str) -> ditto_model::DittoError {
    ditto_model::DittoError::Windows(format!("{context}: {:?}", unsafe { GetLastError() }))
}

#[allow(dead_code)]
fn _keep_w() {
    let _ = w!("Ditto");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_suffix_validation_is_fail_closed() {
        assert_eq!(parse_instance_suffix("TEST", None).expect("unset"), None);
        assert_eq!(
            parse_instance_suffix("TEST", Some("qa-01_x")).expect("valid"),
            Some("qa-01_x".to_string())
        );
        assert_eq!(
            parse_instance_suffix("TEST", Some("  qa  ")).expect("trimmed"),
            Some("qa".to_string())
        );
        for invalid in ["", "   ", "!!!", "qa.1", "qa 1", "中文"] {
            assert!(
                parse_instance_suffix("TEST", Some(invalid)).is_err(),
                "{invalid:?} must be rejected instead of sanitized"
            );
        }
        let longest = "a".repeat(MAX_INSTANCE_SUFFIX_LEN);
        assert_eq!(
            parse_instance_suffix("TEST", Some(&longest))
                .expect("48 characters are allowed")
                .as_deref(),
            Some(longest.as_str())
        );
        assert!(
            parse_instance_suffix("TEST", Some(&"a".repeat(MAX_INSTANCE_SUFFIX_LEN + 1))).is_err(),
            "over-long suffixes must fail instead of being truncated"
        );
        let error = parse_instance_suffix("TEST", Some("!!!")).expect_err("invalid");
        assert!(error.to_string().contains("TEST"));
        assert!(error.to_string().contains("[A-Za-z0-9_-]"));
    }

    #[test]
    fn native_hint_window_is_created_and_closes() {
        show_native_hint("DITTO-NATIVE-HINT-TEST", 800).expect("show hint");
        assert!(
            unsafe { FindWindowW(w!("DittoNativeHint"), PCWSTR::null()) }.is_ok(),
            "hint window exists"
        );
        thread::sleep(Duration::from_millis(2200));
        assert!(
            unsafe { FindWindowW(w!("DittoNativeHint"), PCWSTR::null()) }.is_err(),
            "hint window closes after its timer"
        );

        // The popup-ready path closes the hint before its timer: keep the
        // native window alive for a long time, then dismiss it explicitly.
        assert!(
            show_native_hint("DITTO-NATIVE-HINT-DISMISS-TEST", 30_000).expect("show hint"),
            "a fresh hint is owned by this caller"
        );
        assert!(
            unsafe { FindWindowW(w!("DittoNativeHint"), PCWSTR::null()) }.is_ok(),
            "long-lived hint window exists"
        );
        dismiss_native_hint();
        let mut closed = false;
        for _ in 0..100 {
            if unsafe { FindWindowW(w!("DittoNativeHint"), PCWSTR::null()) }.is_err() {
                closed = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(closed, "dismiss_native_hint closes the hint window");
    }

    #[test]
    fn popup_window_readiness_follows_the_ready_message_not_the_handshake() {
        let name = format!(
            r"\\.\pipe\ditto-test-ready-{}-{}",
            std::process::id(),
            unix_millis()
        );
        let server = PipeServer::start(&name).expect("start server");
        assert!(!server.popup_window_ready());
        let client = PipeClient::connect(&name).expect("connect client");
        client
            .send(1, &hello_request(ClientKind::Popup))
            .expect("send hello");
        assert!(matches!(
            client.read_response().expect("hello response"),
            Response::HelloAck { .. }
        ));
        // IPC prewarm alone must not look like a usable popup window.
        assert!(!server.popup_window_ready());

        client
            .send(2, &Request::PopupWindowReady)
            .expect("send window ready");
        assert!(matches!(
            client.read_response().expect("ready response"),
            Response::Ok
        ));
        assert!(server.popup_window_ready());

        drop(client);
        for _ in 0..200 {
            if server.popup_client_count() == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(server.popup_client_count(), 0);
        assert!(
            !server.popup_window_ready(),
            "window readiness resets when the popup process disconnects"
        );
    }

    #[test]
    fn hello_and_popup_events_traverse_named_pipe() {
        let name = format!(
            r"\\.\pipe\ditto-test-{}-{}",
            std::process::id(),
            unix_millis()
        );
        let server = PipeServer::start(&name).expect("start server");
        let client = PipeClient::connect(&name).expect("connect client");
        client
            .send(1, &hello_request(ClientKind::Popup))
            .expect("send hello");
        assert!(matches!(
            client.read_response().expect("hello response"),
            Response::HelloAck {
                proto: PROTOCOL_VERSION,
                ..
            }
        ));

        server
            .broadcast(&Event::ShowPopup {
                at_ms: unix_millis(),
                monitor: None,
            })
            .expect("broadcast show");
        assert!(matches!(
            client.read_event().expect("show event"),
            Event::ShowPopup { .. }
        ));

        server.broadcast(&Event::HidePopup).expect("broadcast hide");
        assert!(matches!(
            client.read_event().expect("hide event"),
            Event::HidePopup
        ));
    }

    #[test]
    fn custom_request_handler_is_invoked_after_hello() {
        let name = format!(
            r"\\.\pipe\ditto-test-handler-{}-{}",
            std::process::id(),
            unix_millis()
        );
        let server = PipeServer::start_with_handler(&name, |request| match request {
            Request::GetGroups => Response::Groups { groups: Vec::new() },
            _ => Response::Error {
                code: ErrorCode::Internal,
                message: "unexpected test request".to_string(),
                retryable: false,
            },
        })
        .expect("start server");
        let client = PipeClient::connect(&name).expect("connect client");
        client
            .send(1, &hello_request(ClientKind::Popup))
            .expect("send hello");
        assert!(matches!(
            client.read_response().expect("hello response"),
            Response::HelloAck { .. }
        ));
        client.send(2, &Request::GetGroups).expect("send groups");
        assert!(matches!(
            client.read_response().expect("groups response"),
            Response::Groups { groups } if groups.is_empty()
        ));
        drop(server);
    }

    #[test]
    fn pipe_acl_contains_only_system_and_current_user() {
        let sddl = pipe_security_descriptor("S-1-5-21-1-2-3-1001");
        assert_eq!(sddl, "D:P(A;;GA;;;SY)(A;;GA;;;S-1-5-21-1-2-3-1001)");
        assert!(
            !sddl.contains(";;;BA"),
            "administrators must not get an ACE"
        );
    }

    #[test]
    fn opening_a_pipe_owned_by_another_sid_is_denied() {
        let name = format!(
            r"\\.\pipe\ditto-test-acl-{}-{}",
            std::process::id(),
            unix_millis()
        );
        let foreign = "S-1-5-21-1111111111-2222222222-3333333333-1001";
        let server = Connection::create_server_instance_with_sddl(
            &name,
            true,
            &pipe_security_descriptor(foreign),
        )
        .expect("create foreign-SID pipe");
        let name_wide = wide(&name);
        let opened = unsafe {
            CreateFileW(
                windows::core::PCWSTR(name_wide.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0 | FILE_FLAG_OVERLAPPED.0),
                None,
            )
        };
        assert!(
            opened.is_err(),
            "current user must not open a foreign SID pipe"
        );
        drop(server);
    }

    #[test]
    fn broadcast_disconnects_a_client_that_stops_reading() {
        let name = format!(
            r"\\.\pipe\ditto-test-slow-{}-{}",
            std::process::id(),
            unix_millis()
        );
        let server = PipeServer::start_with_handler(&name, |_| Response::Ok).expect("server");
        let _client = PipeClient::connect(&name).expect("connect");
        for _ in 0..200 {
            if server.client_count() > 0 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(server.client_count(), 1);
        let clip = ditto_model::ClipSummary {
            id: 1,
            kind: ditto_model::ClipKind::Text,
            preview_text: "x".repeat(60 * 1024),
            source_app: None,
            created_at_ms: 0,
            last_pasted_at_ms: None,
            pinned: false,
            pin_order: None,
            group_id: None,
            copy_buffer: None,
            formats: vec!["CF_UNICODETEXT".to_string()],
            size_bytes: 1,
            has_image: false,
            image_thumb_ref: None,
            pasted_count: 0,
            state_rev: 1,
        };
        let event = Event::ClipAdded { clip };
        let started = std::time::Instant::now();
        for _ in 0..4 {
            server.broadcast(&event).expect("broadcast");
        }
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "broadcast blocked for {:?}",
            started.elapsed()
        );
        assert_eq!(server.client_count(), 0, "slow client is dropped");
    }
}
