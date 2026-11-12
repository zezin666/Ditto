use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicIsize, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{
                AddClipboardFormatListener, CloseClipboard, EnumClipboardFormats, GetClipboardData,
                GetClipboardFormatNameW, OpenClipboard, RegisterClipboardFormatW,
                RemoveClipboardFormatListener,
            },
            LibraryLoader::GetModuleHandleW,
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
            Ole::{CF_DIB, CF_DIBV5, CF_HDROP, CF_OEMTEXT, CF_TEXT, CF_UNICODETEXT},
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
            GetForegroundWindow, GetMessageW, GetWindowTextW, GetWindowThreadProcessId,
            HWND_MESSAGE, KillTimer, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
            SetTimer, SetWindowLongPtrW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
            WM_DESTROY, WM_TIMER, WNDCLASSEXW,
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::{
    CaptureConfig, CaptureEngine, CapturedClip, ClipboardError, ClipboardResult, ClipboardSource,
    RawClipboardFormat, RawClipboardSnapshot, WriteSuppression,
};

const WM_CLIPBOARDUPDATE: u32 = 0x031D;
const TIMER_ID: usize = 0xD170;
const CF_BITMAP_FORMAT: u32 = 2;
const CF_METAFILEPICT_FORMAT: u32 = 3;
const CF_PALETTE_FORMAT: u32 = 9;
const CF_ENHMETAFILE_FORMAT: u32 = 14;
const CF_OWNERDISPLAY_FORMAT: u32 = 0x0080;

pub struct WindowsClipboardSource {
    config: CaptureConfig,
}

impl WindowsClipboardSource {
    pub fn new(config: CaptureConfig) -> Self {
        Self { config }
    }
}

impl ClipboardSource for WindowsClipboardSource {
    fn read(&self) -> ClipboardResult<Option<RawClipboardSnapshot>> {
        let foreground = foreground_source();
        open_clipboard_with_retry()?;
        let _guard = ClipboardGuard;
        if is_excluded_format_set()? {
            tracing::debug!("clipboard event excluded by Windows privacy signal");
            return Ok(None);
        }

        let mut format = 0_u32;
        let mut formats = Vec::new();
        let mut warnings = Vec::new();
        let mut count = 0_u32;
        loop {
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            count += 1;
            if count > 1_024 {
                warnings.push("clipboard contains more than 1024 formats".to_string());
                break;
            }
            let name = clipboard_format_name(format);
            if self
                .config
                .ignored_formats
                .iter()
                .any(|ignored| ignored.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            if is_non_global_format(format) {
                continue;
            }
            let limit = if is_image_format(&name) {
                self.config.max_image_bytes
            } else {
                self.config.max_clip_bytes
            };
            match read_global_format(format) {
                Ok(bytes) if bytes.is_empty() => {
                    warnings.push(format!("{name} was empty"));
                }
                Ok(bytes) if bytes.len() > limit => {
                    warnings.push(format!("{name} exceeded the {} byte limit", limit));
                }
                Ok(bytes) => formats.push(RawClipboardFormat {
                    name,
                    data: bytes,
                    is_primary: false,
                }),
                Err(error) => {
                    warnings.push(format!("{name}: {error}"));
                }
            }
        }
        if formats.is_empty() {
            return Ok(None);
        }
        Ok(Some(RawClipboardSnapshot {
            formats,
            source_app: foreground.app,
            source_window_title: foreground.title,
            warnings,
        }))
    }
}

pub struct ClipboardMonitor {
    hwnd: Arc<AtomicIsize>,
    thread: Option<thread::JoinHandle<()>>,
    suppression: Arc<WriteSuppression>,
}

impl ClipboardMonitor {
    pub fn start(callback: impl Fn(CapturedClip) + Send + Sync + 'static) -> ClipboardResult<Self> {
        Self::start_with_config(CaptureConfig::default(), callback)
    }

    pub fn start_with_config(
        config: CaptureConfig,
        callback: impl Fn(CapturedClip) + Send + Sync + 'static,
    ) -> ClipboardResult<Self> {
        let source = WindowsClipboardSource::new(config.clone());
        Self::start_with_source(source, config, callback)
    }

    pub fn start_with_source<S>(
        source: S,
        config: CaptureConfig,
        callback: impl Fn(CapturedClip) + Send + Sync + 'static,
    ) -> ClipboardResult<Self>
    where
        S: ClipboardSource + 'static,
    {
        let suppression = Arc::new(WriteSuppression::default());
        let source: Box<dyn ClipboardSource> = Box::new(source);
        let engine = CaptureEngine::new(source, config.clone(), Arc::clone(&suppression));
        let callback = Arc::new(callback);
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();
        let hwnd = Arc::new(AtomicIsize::new(0));
        let hwnd_for_thread = Arc::clone(&hwnd);
        let thread = thread::Builder::new()
            .name("ditto-clipboard-monitor".to_string())
            .spawn(move || {
                run_monitor_thread(
                    engine,
                    callback,
                    config.debounce_ms,
                    hwnd_for_thread,
                    ready_tx,
                )
            })
            .map_err(|error| ClipboardError::Windows(error.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(_)) => Ok(Self {
                hwnd,
                thread: Some(thread),
                suppression,
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(ClipboardError::Windows(error))
            }
            Err(error) => {
                let _ = thread.join();
                Err(ClipboardError::Windows(format!(
                    "clipboard listener did not become ready: {error}"
                )))
            }
        }
    }

    pub fn suppression(&self) -> Arc<WriteSuppression> {
        Arc::clone(&self.suppression)
    }

    pub fn suppress_next(&self, clip: &CapturedClip) {
        self.suppression.remember(clip.content_hash);
    }

    pub fn stop(&mut self) {
        let hwnd = self.hwnd.swap(0, Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut std::ffi::c_void)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ClipboardMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_monitor_thread(
    engine: CaptureEngine<Box<dyn ClipboardSource>>,
    callback: Arc<dyn Fn(CapturedClip) + Send + Sync>,
    debounce_ms: u64,
    hwnd_slot: Arc<AtomicIsize>,
    ready: mpsc::Sender<Result<isize, String>>,
) {
    static CLASS_COUNTER: AtomicU64 = AtomicU64::new(1);
    let class_name = format!(
        "DittoClipboardMonitor_{}_{}",
        std::process::id(),
        CLASS_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let class_wide = wide(&class_name);
    let window_name = wide("Ditto Clipboard Monitor");
    let state = Box::new(MonitorState {
        engine,
        callback,
        debounce_ms,
    });
    let state_ptr = Box::into_raw(state);
    let result = (|| -> Result<(), String> {
        let module = unsafe { GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(monitor_wndproc),
            hInstance: windows::Win32::Foundation::HINSTANCE(module.0),
            lpszClassName: PCWSTR(class_wide.as_ptr()),
            ..Default::default()
        };
        if unsafe { RegisterClassExW(&window_class) } == 0 {
            return Err("RegisterClassExW failed".to_string());
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_wide.as_ptr()),
                PCWSTR(window_name.as_ptr()),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(windows::Win32::Foundation::HINSTANCE(module.0)),
                Some(state_ptr.cast()),
            )
        }
        .map_err(|error| format!("CreateWindowExW failed: {error}"))?;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        }
        hwnd_slot.store(hwnd.0 as isize, Ordering::SeqCst);
        if let Err(error) = unsafe { AddClipboardFormatListener(hwnd) } {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(format!("AddClipboardFormatListener failed: {error}"));
        }
        ready
            .send(Ok(hwnd.0 as isize))
            .map_err(|error| format!("clipboard listener readiness failed: {error}"))?;

        let mut message = MSG::default();
        loop {
            let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if !status.as_bool() {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        hwnd_slot.store(0, Ordering::SeqCst);
        Ok(())
    })();
    if let Err(error) = &result {
        let _ = ready.send(Err(error.clone()));
        tracing::error!(%error, "clipboard monitor stopped");
    }
    unsafe {
        drop(Box::from_raw(state_ptr));
    }
}

struct MonitorState {
    engine: CaptureEngine<Box<dyn ClipboardSource>>,
    callback: Arc<dyn Fn(CapturedClip) + Send + Sync>,
    debounce_ms: u64,
}

impl ClipboardSource for Box<dyn ClipboardSource> {
    fn read(&self) -> ClipboardResult<Option<RawClipboardSnapshot>> {
        self.as_ref().read()
    }
}

unsafe extern "system" fn monitor_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLIPBOARDUPDATE => {
            let state_ptr = unsafe { monitor_state(hwnd) };
            let debounce_ms = if state_ptr.is_null() {
                DEBOUNCE_FALLBACK
            } else {
                unsafe { (*state_ptr).debounce_ms }
            };
            unsafe {
                let _ = SetTimer(
                    Some(hwnd),
                    TIMER_ID,
                    u32::try_from(debounce_ms).unwrap_or(150),
                    None,
                );
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == TIMER_ID => {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
            }
            let state_ptr = unsafe { monitor_state(hwnd) };
            if !state_ptr.is_null() {
                let state = unsafe { &*state_ptr };
                match state.engine.capture_once() {
                    Ok(crate::CaptureDisposition::Captured(clip)) => (state.callback)(*clip),
                    Ok(crate::CaptureDisposition::Empty) => {}
                    Ok(crate::CaptureDisposition::Suppressed) => {
                        tracing::debug!("ignored own clipboard write");
                    }
                    Err(error) => tracing::warn!(%error, "clipboard capture failed"),
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                let _ = RemoveClipboardFormatListener(hwnd);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn monitor_state(hwnd: HWND) -> *mut MonitorState {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
        ) as *mut MonitorState
    }
}

const DEBOUNCE_FALLBACK: u64 = 150;

struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn open_clipboard_with_retry() -> ClipboardResult<()> {
    let mut last_error = None;
    for _ in 0..5 {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error.to_string());
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    tracing::debug!(?last_error, "OpenClipboard remained unavailable");
    Err(ClipboardError::Occupied)
}

fn read_global_format(format: u32) -> ClipboardResult<Vec<u8>> {
    let handle = unsafe { GetClipboardData(format) }
        .map_err(|error| ClipboardError::ReadFailed(error.to_string()))?;
    if handle == HANDLE::default() {
        return Err(ClipboardError::ReadFailed(
            "GetClipboardData returned null".to_string(),
        ));
    }
    let memory = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(memory) };
    if size == 0 {
        return Ok(Vec::new());
    }
    let pointer = unsafe { GlobalLock(memory) };
    if pointer.is_null() {
        return Err(ClipboardError::ReadFailed(
            "clipboard handle is not HGLOBAL-compatible".to_string(),
        ));
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size) }.to_vec();
    let _ = unsafe { GlobalUnlock(memory) };
    Ok(bytes)
}

fn is_excluded_format_set() -> ClipboardResult<bool> {
    let include = register_format("CanIncludeInClipboardHistory");
    let exclude = register_format("ExcludeClipboardContentFromMonitorProcessing");
    Ok((include != 0 && global_dword(include) == Some(0))
        || (exclude != 0 && global_dword(exclude).is_some_and(|value| value != 0)))
}

fn global_dword(format: u32) -> Option<u32> {
    let bytes = read_global_format(format).ok()?;
    Some(u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?))
}

fn register_format(name: &str) -> u32 {
    let wide = wide(name);
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

fn clipboard_format_name(format: u32) -> String {
    match format {
        value if value == CF_TEXT.0 as u32 => "CF_TEXT".to_string(),
        value if value == CF_OEMTEXT.0 as u32 => "CF_OEMTEXT".to_string(),
        value if value == CF_UNICODETEXT.0 as u32 => "CF_UNICODETEXT".to_string(),
        value if value == CF_DIB.0 as u32 => "CF_DIB".to_string(),
        value if value == CF_DIBV5.0 as u32 => "CF_DIBV5".to_string(),
        value if value == CF_HDROP.0 as u32 => "CF_HDROP".to_string(),
        _ => {
            let mut buffer = vec![0_u16; 256];
            let copied = unsafe { GetClipboardFormatNameW(format, &mut buffer) };
            if copied > 0 {
                String::from_utf16_lossy(&buffer[..copied as usize])
            } else {
                format!("CF_{format}")
            }
        }
    }
}

fn is_non_global_format(format: u32) -> bool {
    matches!(
        format,
        CF_BITMAP_FORMAT
            | CF_METAFILEPICT_FORMAT
            | CF_PALETTE_FORMAT
            | CF_ENHMETAFILE_FORMAT
            | CF_OWNERDISPLAY_FORMAT
    )
}

fn is_image_format(name: &str) -> bool {
    matches!(name, "PNG" | "CF_DIB" | "CF_DIBV5")
}

struct ForegroundSource {
    app: Option<String>,
    title: Option<String>,
}

fn foreground_source() -> ForegroundSource {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd == HWND::default() {
        return ForegroundSource {
            app: None,
            title: None,
        };
    }
    let mut pid = 0_u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    ForegroundSource {
        app: process_name(pid),
        title: window_title(hwnd),
    }
}

fn window_title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextW(hwnd, &mut []) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    (copied > 0).then(|| String::from_utf16_lossy(&buffer[..copied as usize]))
}

fn process_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let _guard = OwnedHandle(handle);
    let mut buffer = vec![0_u16; 32_768];
    let mut size = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            Default::default(),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .ok()?;
    }
    let path = String::from_utf16_lossy(&buffer[..size as usize]);
    Path::new(&path)
        .file_name()
        .and_then(OsStr::to_str)
        .map(ToString::to_string)
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}
