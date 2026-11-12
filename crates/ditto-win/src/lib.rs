//! Narrow Win32 helpers shared by the Ditto host and resident popup.

use std::{
    collections::HashMap,
    ffi::OsStr,
    io::{self, Read, Write},
    os::windows::ffi::OsStrExt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
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
            CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_BROKEN_PIPE,
            ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, LocalFree,
        },
        Graphics::Gdi::InvalidateRect,
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE,
            FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
            ReadFile, WriteFile,
        },
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_TYPE_BYTE, PIPE_WAIT, PeekNamedPipe, WaitNamedPipeW,
        },
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
        UI::Input::KeyboardAndMouse::{
            HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
        },
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SW_HIDE, SW_SHOWNOACTIVATE,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    },
    core::{PWSTR, w},
};

const MAX_INSTANCES: u32 = 32;
const PIPE_BUFFER_SIZE: u32 = 64 * 1024;

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

pub fn pipe_name() -> ditto_model::Result<String> {
    Ok(format!(r"\\.\pipe\ditto-{}", current_user_sid_string()?))
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
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    Ok(())
}

pub fn set_popup_visible(hwnd: windows::Win32::Foundation::HWND, visible: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        if visible {
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
}

pub type RequestHandler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;

impl PipeServer {
    pub fn start(name: &str) -> ditto_model::Result<Self> {
        Self::start_internal(name, None)
    }

    pub fn start_with_handler<F>(name: &str, handler: F) -> ditto_model::Result<Self>
    where
        F: Fn(&Request) -> Response + Send + Sync + 'static,
    {
        Self::start_internal(name, Some(Arc::new(handler)))
    }

    fn start_internal(name: &str, handler: Option<RequestHandler>) -> ditto_model::Result<Self> {
        let name = name.to_string();
        let clients = Arc::new(Mutex::new(HashMap::<u64, Arc<Connection>>::new()));
        let first = Connection::create_server_instance(&name, true)?;
        let clients_for_thread = Arc::clone(&clients);
        let handler_for_thread = handler.clone();

        thread::Builder::new()
            .name("ditto-ipc-server".to_string())
            .spawn(move || accept_loop(name, first, clients_for_thread, handler_for_thread))
            .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;

        Ok(Self {
            clients,
            next_event: Arc::new(AtomicU64::new(1)),
        })
    }

    pub fn broadcast(&self, event: &Event) -> ditto_model::Result<()> {
        let sequence = self.next_event.fetch_add(1, Ordering::Relaxed);
        let frame = Frame::json(FrameType::Event, sequence, event)
            .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
        let clients: Vec<_> = self.clients.lock().values().cloned().collect();
        for client in clients {
            if let Err(error) = client.write_frame(&frame) {
                tracing::debug!(%error, "IPC event delivery failed");
            }
        }
        Ok(())
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
}

impl Connection {
    fn create_server_instance(name: &str, first_instance: bool) -> ditto_model::Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let sid = current_user_sid_string()?;
        let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})(A;;GR;;;BA)");
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let sddl_wide = wide(&sddl);
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
        let mut access = PIPE_ACCESS_DUPLEX;
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
        })
    }

    fn connect(&self) -> ditto_model::Result<()> {
        unsafe {
            if ConnectNamedPipe(self.read_handle.0, None).is_err() {
                let code = GetLastError();
                if code != ERROR_PIPE_CONNECTED {
                    return Err(ditto_model::DittoError::Windows(format!(
                        "ConnectNamedPipe failed: {code:?}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn read_frame(&self) -> Result<Frame, IpcError> {
        read_frame(&mut PipeReader(self.read_handle.0))
    }

    fn write_frame(&self, frame: &Frame) -> Result<(), IpcError> {
        let _guard = self.write_lock.lock();
        write_frame(&mut PipeWriter(self.write_handle.0), frame)
    }
}

fn accept_loop(
    name: String,
    mut current: Connection,
    clients: Arc<Mutex<HashMap<u64, Arc<Connection>>>>,
    handler: Option<RequestHandler>,
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
        thread::spawn(move || {
            handle_connection(connection, clients_for_handler, handler_for_connection)
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
) {
    loop {
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

        let response = dispatch_request(&request, &clients, handler.as_deref());
        if send_response(&connection, frame.request_id, &response).is_err() {
            break;
        }
        if matches!(request, Request::Shutdown) {
            break;
        }
    }
    clients.lock().remove(&connection.id);
}

fn dispatch_request(
    request: &Request,
    clients: &Arc<Mutex<HashMap<u64, Arc<Connection>>>>,
    handler: Option<&(dyn Fn(&Request) -> Response + Send + Sync)>,
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
                    FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0),
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
        let mut read = 0_u32;
        unsafe {
            ReadFile(self.0, Some(buffer), Some(&mut read), None)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        Ok(read as usize)
    }
}

struct PipeWriter(HANDLE);

impl Write for PipeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut written = 0_u32;
        unsafe {
            WriteFile(self.0, Some(buffer), Some(&mut written), None)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        Ok(written as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
}
