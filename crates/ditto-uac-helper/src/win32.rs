use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_CANCELLED, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE,
            GetLastError, HANDLE, HLOCAL, HWND, INVALID_HANDLE_VALUE, LocalFree,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
            GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
            TOKEN_USER, TokenUser,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE,
            FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
            ReadFile, WriteFile,
        },
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
                GetNamedPipeServerProcessId, PIPE_NOWAIT, PIPE_READMODE_BYTE,
                PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PeekNamedPipe,
            },
            Threading::{
                AttachThreadInput, GetCurrentProcess, GetCurrentThreadId, GetProcessId,
                OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, SendInput,
                VK_CONTROL, VK_V,
            },
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::{
                BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
                IsWindow, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
            },
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::{
    HELPER_TIMEOUT, UacError, UacResult,
    protocol::{
        HelperArgs, HelperDone, HelperHello, HelperMessage, HelperMode, MAX_HELPER_MESSAGE,
        OneShotAuthorize, now_ms,
    },
    token::{TokenBinding, TokenStore},
};

const MAX_MESSAGE: usize = MAX_HELPER_MESSAGE;

pub fn launch_elevated_paste(
    helper_path: &Path,
    target_window: isize,
    target_pid: u32,
    clip_id: i64,
    mode: HelperMode,
    focus_delay_ms: u64,
) -> UacResult<()> {
    if !helper_path.is_file() {
        return Err(UacError::HelperNotFound);
    }
    if helper_path.file_name().and_then(OsStr::to_str) != Some("DittoUacHelper.exe") {
        return Err(UacError::Validation(
            "helper path does not name DittoUacHelper.exe".to_string(),
        ));
    }

    let sid = current_user_sid_string()?;
    let request_id = random_u64()?;
    let token = random_bytes::<32>()?;
    let args = HelperArgs {
        core_pid: std::process::id(),
        target_pid,
        target_window,
        clip_id,
        mode,
        request_id,
        focus_delay_ms: focus_delay_ms.min(300),
    };
    let binding = TokenBinding {
        core_pid: args.core_pid,
        target_pid,
        target_window,
        clip_id,
        mode: args.mode,
        user_sid: sid.clone(),
    };
    let store = TokenStore::default();
    let issued = store.issue(
        request_id,
        token,
        binding.clone(),
        now_ms(),
        HELPER_TIMEOUT.as_millis() as u64,
    );
    let pipe_name = pipe_name(&sid, args.core_pid, request_id);
    let mut server = ServerPipe::create(&pipe_name, &sid)?;
    let helper = launch_helper(helper_path, &args)?;
    let deadline = Instant::now() + HELPER_TIMEOUT;
    server.wait_for_client(deadline)?;
    let client_pid = server.client_pid()?;
    if client_pid != helper.pid {
        return Err(UacError::Authentication(
            "pipe client is not the helper process launched by the core".to_string(),
        ));
    }
    let client_sid = process_user_sid(client_pid)?;
    if client_sid != sid {
        return Err(UacError::Authentication(
            "helper is running under a different user".to_string(),
        ));
    }

    let hello = match read_message(&server.handle.0, deadline)? {
        HelperMessage::Hello {
            protocol,
            request_id,
            core_pid,
            target_pid,
            target_window,
            clip_id,
            mode,
            client_pid,
            user_sid,
        } => HelperHello {
            protocol,
            request_id,
            core_pid,
            target_pid,
            target_window,
            clip_id,
            mode,
            client_pid,
            user_sid,
        },
        _ => {
            return Err(UacError::Authentication(
                "expected helper hello".to_string(),
            ));
        }
    };
    hello.validate_against(&args, client_pid)?;
    if hello.user_sid != sid {
        return Err(UacError::Authentication(
            "helper reported a different user SID".to_string(),
        ));
    }

    store.authorize(request_id, issued.token, &binding, now_ms())?;
    write_message(
        &server.handle.0,
        &HelperMessage::Authorize {
            request_id,
            token: issued.token,
            expires_at_ms: issued.expires_at_ms,
            target_pid,
            target_window,
            focus_delay_ms: args.focus_delay_ms,
        },
    )?;

    match read_message(&server.handle.0, deadline)? {
        HelperMessage::Ready {
            request_id: ready_id,
        } if ready_id == request_id => {}
        _ => {
            return Err(UacError::Authentication(
                "helper did not acknowledge authorization".to_string(),
            ));
        }
    }
    let done = match read_message(&server.handle.0, deadline)? {
        HelperMessage::Done {
            request_id: done_id,
            ok,
            error,
        } if done_id == request_id => HelperDone {
            request_id: done_id,
            ok,
            error,
        },
        _ => {
            return Err(UacError::Authentication(
                "malformed helper result".to_string(),
            ));
        }
    };
    if !done.ok {
        return Err(UacError::Validation(
            done.error.unwrap_or_else(|| "helper failed".to_string()),
        ));
    }
    Ok(())
}

pub(crate) fn run_helper(args: &HelperArgs) -> UacResult<()> {
    let sid = current_user_sid_string()?;
    let pipe_name = pipe_name(&sid, args.core_pid, args.request_id);
    let pipe = ClientPipe::connect(&pipe_name)?;
    let server_pid = pipe_server_pid(&pipe.0)?;
    if server_pid != args.core_pid {
        return Err(UacError::Authentication(
            "pipe server is not the requesting core process".to_string(),
        ));
    }
    if process_user_sid(server_pid)? != sid {
        return Err(UacError::Authentication(
            "pipe server is running under a different user".to_string(),
        ));
    }

    let hello = HelperHello::from(args.clone());
    write_message(
        &pipe.0,
        &HelperMessage::Hello {
            protocol: hello.protocol,
            request_id: hello.request_id,
            core_pid: hello.core_pid,
            target_pid: hello.target_pid,
            target_window: hello.target_window,
            clip_id: hello.clip_id,
            mode: hello.mode,
            client_pid: std::process::id(),
            user_sid: sid.clone(),
        },
    )?;
    let deadline = Instant::now() + HELPER_TIMEOUT;
    let authorize = match read_message(&pipe.0, deadline)? {
        HelperMessage::Authorize {
            request_id,
            token,
            expires_at_ms,
            target_pid,
            target_window,
            focus_delay_ms,
        } => OneShotAuthorize {
            request_id,
            token,
            expires_at_ms,
            target_pid,
            target_window,
            focus_delay_ms,
        },
        _ => {
            return Err(UacError::Authentication(
                "expected authorization".to_string(),
            ));
        }
    };
    if authorize.request_id != args.request_id
        || authorize.target_pid != args.target_pid
        || authorize.target_window != args.target_window
        || authorize.expires_at_ms < now_ms()
    {
        return Err(UacError::Authentication(
            "authorization binding failed".to_string(),
        ));
    }

    let target = HWND(authorize.target_window as *mut _);
    if target.0.is_null()
        || unsafe { !IsWindow(Some(target)).as_bool() }
        || target_pid_for_window(target) != args.target_pid
    {
        send_done(
            &pipe.0,
            args.request_id,
            false,
            Some("target window changed"),
        );
        return Err(UacError::TargetUnavailable);
    }
    activate_target(target);
    if authorize.focus_delay_ms > 0 {
        thread::sleep(Duration::from_millis(authorize.focus_delay_ms.min(300)));
    }
    if unsafe { GetForegroundWindow() } != target {
        send_done(
            &pipe.0,
            args.request_id,
            false,
            Some("target did not regain focus"),
        );
        return Err(UacError::TargetUnavailable);
    }

    write_message(
        &pipe.0,
        &HelperMessage::Ready {
            request_id: args.request_id,
        },
    )?;
    if let Err(error) = send_ctrl_v() {
        send_done(&pipe.0, args.request_id, false, Some("SendInput failed"));
        return Err(error);
    }
    send_done(&pipe.0, args.request_id, true, None);
    Ok(())
}

pub fn send_ctrl_v() -> UacResult<()> {
    let inputs = [
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_V, false),
        keyboard_input(VK_V, true),
        keyboard_input(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(UacError::SendInput)
    }
}

fn send_done(handle: &HANDLE, request_id: u64, ok: bool, error: Option<&str>) {
    let _ = write_message(
        handle,
        &HelperMessage::Done {
            request_id,
            ok,
            error: error.map(ToString::to_string),
        },
    );
}

fn keyboard_input(
    key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    up: bool,
) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if up {
                    KEYBD_EVENT_FLAGS(2)
                } else {
                    KEYBD_EVENT_FLAGS::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn launch_helper(path: &Path, args: &HelperArgs) -> UacResult<HelperProcess> {
    let verb = wide("runas");
    let file = wide(&path.to_string_lossy());
    let parameters = wide(&format!(
        "--core-pid {} --target-pid {} --target-window {} --clip-id {} --mode {} --request-id {} --focus-delay-ms {}",
        args.core_pid,
        args.target_pid,
        args.target_window,
        args.clip_id,
        args.mode.as_str(),
        args.request_id,
        args.focus_delay_ms
    ));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: HWND::default(),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: PCWSTR::null(),
        nShow: SW_SHOWNORMAL.0,
        ..SHELLEXECUTEINFOW::default()
    };
    if let Err(error) = unsafe { ShellExecuteExW(&mut info) } {
        if error.code().0 as u32 == ERROR_CANCELLED.0 || error.code().0 == 0x800704C7_u32 as i32 {
            return Err(UacError::Cancelled);
        }
        return Err(UacError::Windows(format!(
            "ShellExecuteExW failed: {error}"
        )));
    }
    if info.hProcess.0.is_null() {
        return Err(UacError::Windows(
            "ShellExecuteExW returned no helper process".to_string(),
        ));
    }
    let pid = unsafe { GetProcessId(info.hProcess) };
    Ok(HelperProcess {
        _handle: OwnedHandle(info.hProcess),
        pid,
    })
}

struct HelperProcess {
    _handle: OwnedHandle,
    pid: u32,
}

struct ServerPipe {
    handle: OwnedHandle,
    connected: bool,
}

impl ServerPipe {
    fn create(name: &str, sid: &str) -> UacResult<Self> {
        let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})");
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let sddl_wide = wide(&sddl);
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl_wide.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let name_wide = wide(name);
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(name_wide.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                4096,
                0,
                Some(&attributes),
            )
        };
        let _ = unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        if handle == INVALID_HANDLE_VALUE {
            return Err(windows_error("CreateNamedPipeW"));
        }
        Ok(Self {
            handle: OwnedHandle(handle),
            connected: false,
        })
    }

    fn wait_for_client(&mut self, deadline: Instant) -> UacResult<()> {
        while Instant::now() < deadline {
            let connected = match unsafe { ConnectNamedPipe(self.handle.0, None) } {
                Ok(()) => true,
                Err(_) => {
                    let code = unsafe { GetLastError() };
                    code == ERROR_PIPE_CONNECTED
                }
            };
            if connected {
                self.connected = true;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(20));
        }
        Err(UacError::Timeout(
            "waiting for helper to connect".to_string(),
        ))
    }

    fn client_pid(&self) -> UacResult<u32> {
        let mut pid = 0_u32;
        unsafe { GetNamedPipeClientProcessId(self.handle.0, &mut pid)? };
        Ok(pid)
    }
}

struct ClientPipe(HANDLE);

impl ClientPipe {
    fn connect(name: &str) -> UacResult<Self> {
        let name_wide = wide(name);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(name_wide.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0),
                    None,
                )
            };
            match handle {
                Ok(handle) => return Ok(Self(handle)),
                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => {
                    return Err(UacError::Ipc(format!(
                        "CreateFileW helper pipe failed: {error}"
                    )));
                }
            }
        }
    }
}

impl Drop for ClientPipe {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn pipe_server_pid(handle: &HANDLE) -> UacResult<u32> {
    let mut pid = 0_u32;
    unsafe { GetNamedPipeServerProcessId(*handle, &mut pid)? };
    Ok(pid)
}

fn target_pid_for_window(hwnd: HWND) -> u32 {
    let mut pid = 0_u32;
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    pid
}

fn activate_target(target: HWND) {
    if unsafe { IsIconic(target).as_bool() } {
        let _ = unsafe { ShowWindow(target, SW_RESTORE) };
    }
    if unsafe { GetForegroundWindow() } == target {
        return;
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
    let target_thread = unsafe { GetWindowThreadProcessId(target, None) };
    let attach_target = target_thread != 0
        && target_thread != current_thread
        && unsafe { AttachThreadInput(current_thread, target_thread, true).as_bool() };
    let _ = unsafe { BringWindowToTop(target) };
    let _ = unsafe { SetForegroundWindow(target) };
    if attach_target {
        let _ = unsafe { AttachThreadInput(current_thread, target_thread, false) };
    }
    if attach_foreground {
        let _ = unsafe { AttachThreadInput(current_thread, foreground_thread, false) };
    }
    for _ in 0..25 {
        if unsafe { GetForegroundWindow() } == target {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn current_user_sid_string() -> UacResult<String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)? };
    let _token = OwnedHandle(token);
    sid_from_token(token)
}

fn process_user_sid(pid: u32) -> UacResult<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?;
    let _process = OwnedHandle(process);
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token)? };
    let _token = OwnedHandle(token);
    sid_from_token(token)
}

fn sid_from_token(token: HANDLE) -> UacResult<String> {
    let mut size = 0_u32;
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut size) };
    if size == 0 {
        return Err(windows_error("GetTokenInformation(TokenUser)"));
    }
    let mut buffer = vec![0_u8; size as usize];
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        )?
    };
    let token_user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
    let mut sid = PWSTR::null();
    unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid)? };
    let value = unsafe { sid.to_string() }
        .map_err(|error| UacError::Windows(format!("SID conversion failed: {error}")))?;
    let _ = unsafe { LocalFree(Some(windows::Win32::Foundation::HLOCAL(sid.0.cast()))) };
    Ok(value)
}

fn write_message(handle: &HANDLE, message: &HelperMessage) -> UacResult<()> {
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_MESSAGE {
        return Err(UacError::Ipc("helper message is too large".to_string()));
    }
    let length = (payload.len() as u32).to_le_bytes();
    write_all(handle, &length)?;
    write_all(handle, &payload)
}

fn read_message(handle: &HANDLE, deadline: Instant) -> UacResult<HelperMessage> {
    let mut length = [0_u8; 4];
    read_exact(handle, &mut length, deadline)?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_MESSAGE {
        return Err(UacError::Ipc("invalid helper message length".to_string()));
    }
    let mut payload = vec![0_u8; length];
    read_exact(handle, &mut payload, deadline)?;
    Ok(serde_json::from_slice(&payload)?)
}

fn write_all(handle: &HANDLE, bytes: &[u8]) -> UacResult<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let mut written = 0_u32;
        unsafe { WriteFile(*handle, Some(&bytes[offset..]), Some(&mut written), None)? };
        if written == 0 {
            return Err(windows_error("WriteFile"));
        }
        offset += written as usize;
    }
    Ok(())
}

fn read_exact(handle: &HANDLE, bytes: &mut [u8], deadline: Instant) -> UacResult<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let mut available = 0_u32;
        unsafe { PeekNamedPipe(*handle, None, 0, None, Some(&mut available), None)? };
        if available > 0 {
            let mut read = 0_u32;
            unsafe { ReadFile(*handle, Some(&mut bytes[offset..]), Some(&mut read), None)? };
            if read == 0 {
                return Err(windows_error("ReadFile"));
            }
            offset += read as usize;
            continue;
        }
        if Instant::now() >= deadline {
            return Err(UacError::Timeout("reading helper message".to_string()));
        }
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn random_bytes<const N: usize>() -> UacResult<[u8; N]> {
    let mut bytes = [0_u8; N];
    let status = unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status.is_err() {
        return Err(UacError::Windows(format!(
            "BCryptGenRandom failed: {status:?}"
        )));
    }
    Ok(bytes)
}

fn random_u64() -> UacResult<u64> {
    Ok(u64::from_le_bytes(random_bytes::<8>()?))
}

fn pipe_name(sid: &str, core_pid: u32, request_id: u64) -> String {
    format!(r"\\.\pipe\ditto-uac-{sid}-{core_pid}-{request_id}")
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn windows_error(context: &str) -> UacError {
    let code = unsafe { GetLastError() };
    UacError::Windows(format!("{context} failed: {code:?}"))
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}
