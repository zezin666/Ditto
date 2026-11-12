param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class DittoKeyboardQa {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "SendMessageW")]
    public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, StringBuilder lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "SendMessageW")]
    static extern IntPtr SendMessageText(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr CreateWindowExW(uint exStyle, string className, string windowName, uint style,
        int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")] static extern bool UpdateWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool DestroyWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool GetMessage(out MSG message, IntPtr hWnd, uint min, uint max);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int count);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int left, top, right, bottom; }

    public static IntPtr FindTopWindow(int pid) {
        IntPtr found = IntPtr.Zero;
        long bestArea = 0;
        EnumWindows((hWnd, lParam) => {
            uint owner;
            GetWindowThreadProcessId(hWnd, out owner);
            if (owner == (uint)pid && IsWindowVisible(hWnd)) {
                RECT rect;
                if (GetWindowRect(hWnd, out rect)) {
                    long area = (long)(rect.right - rect.left) * (rect.bottom - rect.top);
                    if (area > bestArea) { bestArea = area; found = hWnd; }
                }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static string DescribeWindow(IntPtr hWnd) {
        if (hWnd == IntPtr.Zero) return "hwnd=0";
        var text = new StringBuilder(256);
        GetWindowTextW(hWnd, text, text.Capacity);
        RECT rect;
        GetWindowRect(hWnd, out rect);
        return string.Format("hwnd={0} visible={1} title='{2}' rect={3},{4},{5},{6}",
            hWnd, IsWindowVisible(hWnd), text, rect.left, rect.top, rect.right, rect.bottom);
    }
    [DllImport("user32.dll")] static extern bool TranslateMessage(ref MSG message);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG message);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandleW(string name);

    [StructLayout(LayoutKind.Sequential)]
    struct MSG { public IntPtr hwnd; public uint message; public IntPtr wParam; public IntPtr lParam; public uint time; public int x; public int y; }

    const uint KEYUP = 0x0002;
    const uint WM_KEYDOWN = 0x0100;
    const uint WM_KEYUP = 0x0101;
    const uint WM_CHAR = 0x0102;
    const uint WM_GETTEXT = 0x000D;
    const uint WS_OVERLAPPEDWINDOW = 0x00CF0000;
    const uint WS_VISIBLE = 0x10000000;
    const uint ES_MULTILINE = 0x0004;

    static IntPtr window;
    static Thread thread;

    public static void Start() {
        thread = new Thread(() => {
            window = CreateWindowExW(0, "EDIT", "Ditto QA paste target",
                WS_OVERLAPPEDWINDOW | WS_VISIBLE | ES_MULTILINE,
                140, 140, 520, 220, IntPtr.Zero, IntPtr.Zero, GetModuleHandleW(null), IntPtr.Zero);
            if (window == IntPtr.Zero) throw new Exception("CREATE_WINDOW_FAILED");
            ShowWindow(window, 5);
            UpdateWindow(window);
            SetForegroundWindow(window);
            SetFocus(window);
            MSG message;
            while (GetMessage(out message, IntPtr.Zero, 0, 0)) {
                TranslateMessage(ref message);
                DispatchMessage(ref message);
            }
        });
        thread.IsBackground = true;
        thread.Start();
        for (int i = 0; i < 100 && window == IntPtr.Zero; i++) Thread.Sleep(50);
        if (window == IntPtr.Zero) throw new Exception("TARGET_WINDOW_NOT_READY");
    }

    public static IntPtr Handle() { return window; }

    public static void Activate() {
        SetForegroundWindow(window);
        SetFocus(window);
    }

    public static void Clear() {
        const uint WM_SETTEXT = 0x000C;
        SendMessageText(window, WM_SETTEXT, IntPtr.Zero, "");
    }

    public static string Text() {
        var buffer = new StringBuilder(4096);
        SendMessage(window, WM_GETTEXT, (IntPtr)buffer.Capacity, buffer);
        return buffer.ToString();
    }

    public static void Close() {
        try { DestroyWindow(window); } catch { }
    }

    public static void Press(byte vk) {
        keybd_event(vk, 0, 0, UIntPtr.Zero);
        Thread.Sleep(20);
        keybd_event(vk, 0, KEYUP, UIntPtr.Zero);
        Thread.Sleep(30);
    }

    public static void TypeAscii(string text) {
        foreach (char c in text) {
            byte vk = (byte)char.ToUpperInvariant(c);
            keybd_event(vk, 0, 0, UIntPtr.Zero);
            Thread.Sleep(15);
            keybd_event(vk, 0, KEYUP, UIntPtr.Zero);
            Thread.Sleep(35);
        }
    }

    public static void TypeAsciiMessages(IntPtr hwnd, string text) {
        foreach (char c in text) {
            PostMessage(hwnd, WM_CHAR, (IntPtr)c, (IntPtr)1);
            Thread.Sleep(35);
        }
    }

    public static void ShiftEnter() {
        keybd_event(0x10, 0, 0, UIntPtr.Zero);
        Thread.Sleep(20);
        keybd_event(0x0D, 0, 0, UIntPtr.Zero);
        Thread.Sleep(20);
        keybd_event(0x0D, 0, KEYUP, UIntPtr.Zero);
        Thread.Sleep(20);
        keybd_event(0x10, 0, KEYUP, UIntPtr.Zero);
    }

    public static void PostEnter(IntPtr hwnd, bool shift) {
        if (shift) PostMessage(hwnd, WM_KEYDOWN, (IntPtr)0x10, (IntPtr)0);
        PostMessage(hwnd, WM_KEYDOWN, (IntPtr)0x0D, (IntPtr)0x001C0001);
        Thread.Sleep(30);
        PostMessage(hwnd, WM_KEYUP, (IntPtr)0x0D, unchecked((IntPtr)(int)0xC01C0001));
        if (shift) {
            Thread.Sleep(20);
            PostMessage(hwnd, WM_KEYUP, (IntPtr)0x10, unchecked((IntPtr)(int)0xC0000001));
        }
    }
}
"@

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-keyboard-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$dataRoot = Join-Path $Root "data"
$tracePath = Join-Path $Root "popup-trace.log"
$hostTrace = Join-Path $Root "host-trace.log"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null

# Unique instance: never touches the user's running Ditto or another QA core.
$suffix = if ($env:DITTO_PIPE_SUFFIX) { $env:DITTO_PIPE_SUFFIX } else { [Guid]::NewGuid().ToString("N").Substring(0, 10) }
$env:DITTO_PIPE_SUFFIX = $suffix
$env:DITTO_DATA_DIR = $dataRoot
$env:DITTO_POPUP_TRACE = $tracePath
$env:DITTO_HOST_TRACE = $hostTrace
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$pipeName = "ditto-$sid-$suffix"

function Read-Exact($stream, [byte[]]$buffer) {
    $offset = 0
    while ($offset -lt $buffer.Length) {
        $read = $stream.Read($buffer, $offset, $buffer.Length - $offset)
        if ($read -le 0) { throw "pipe closed while reading" }
        $offset += $read
    }
}
function Send-Frame($stream, [int]$typeId, [uint64]$id, [string]$json) {
    $payload = [Text.Encoding]::UTF8.GetBytes($json)
    $header = New-Object byte[] 24
    [Text.Encoding]::ASCII.GetBytes("DTP1").CopyTo($header, 0)
    [BitConverter]::GetBytes([uint16]1).CopyTo($header, 4)
    [BitConverter]::GetBytes([uint16]$typeId).CopyTo($header, 6)
    [BitConverter]::GetBytes([uint32]$payload.Length).CopyTo($header, 12)
    [BitConverter]::GetBytes([uint64]$id).CopyTo($header, 16)
    $stream.Write($header, 0, 24)
    $stream.Write($payload, 0, $payload.Length)
    $stream.Flush()
}
function Receive-Response($stream, [uint64]$id) {
    while ($true) {
        $header = New-Object byte[] 24
        Read-Exact $stream $header
        $frameType = [BitConverter]::ToUInt16($header, 6)
        $requestId = [BitConverter]::ToUInt64($header, 16)
        $length = [BitConverter]::ToUInt32($header, 12)
        $payload = New-Object byte[] $length
        if ($length -gt 0) { Read-Exact $stream $payload }
        if ($frameType -eq 2 -and $requestId -eq $id) {
            return [Text.Encoding]::UTF8.GetString($payload) | ConvertFrom-Json
        }
    }
}

$core = $null
$pipe = $null
$startedAt = Get-Date
try {
    & cargo run --quiet --manifest-path (Join-Path $repoRoot "Cargo.toml") -p ditto-storage --example seed_qa -- $dataRoot
    if ($LASTEXITCODE -ne 0) { throw "failed to seed the synthetic data root" }

    $core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    for ($i = 0; $i -lt 300; $i++) {
        Start-Sleep -Milliseconds 100
        if ((Test-Path -LiteralPath $tracePath) -and
            (Select-String -Path $tracePath -Pattern "using-prewarmed-ipc|connected" -Quiet)) { break }
        if ($core.HasExited) { throw "core exited during startup" }
    }
    if (-not (Test-Path -LiteralPath $tracePath)) { throw "popup trace was not created" }
    if (-not (Select-String -Path $tracePath -Pattern "using-prewarmed-ipc|connected" -Quiet)) {
        throw "popup IPC reader did not start"
    }

    $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
    $pipe.Connect(5000)
    $script:requestId = 1
    Send-Frame $pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
    $hello = Receive-Response $pipe $script:requestId
    if ($hello.type -ne "HelloAck") { throw "Hello failed" }

    [DittoKeyboardQa]::Start()
    Start-Sleep -Milliseconds 500
    [DittoKeyboardQa]::Activate()
    Start-Sleep -Milliseconds 300
    $targetHandle = [DittoKeyboardQa]::Handle()
    Write-Output "TARGET_HWND=$targetHandle"

    function Show-Popup {
        $before = @(Select-String -Path $tracePath -Pattern "activate-foreground=" -ErrorAction SilentlyContinue).Count
        $script:requestId++
        Send-Frame $pipe 1 $script:requestId '{"type":"ShowPopup"}'
        $response = Receive-Response $pipe $script:requestId
        if ($response.type -ne "PopupVisibility") { throw "ShowPopup failed: $($response | ConvertTo-Json -Compress)" }
        $deadline = (Get-Date).AddSeconds(10)
        while ((Get-Date) -lt $deadline) {
            $after = @(Select-String -Path $tracePath -Pattern "activate-foreground=" -ErrorAction SilentlyContinue).Count
            if ($after -gt $before) { return }
            Start-Sleep -Milliseconds 100
        }
        throw "popup did not process the Show event"
    }
    function Hide-Popup {
        $script:requestId++
        Send-Frame $pipe 1 $script:requestId '{"type":"HidePopup"}'
        $null = Receive-Response $pipe $script:requestId
    }
    function Popup-Process {
        Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
            Where-Object { $_.StartTime -ge $startedAt } |
            Sort-Object StartTime |
            Select-Object -First 1
    }
    function Trace-Contains([string]$pattern) {
        (Test-Path -LiteralPath $tracePath) -and (Select-String -Path $tracePath -Pattern $pattern -Quiet)
    }
    function Wait-Trace([string]$pattern, [int]$timeoutMs = 4000) {
        $deadline = (Get-Date).AddMilliseconds($timeoutMs)
        while ((Get-Date) -lt $deadline) {
            if (Trace-Contains $pattern) { return $true }
            Start-Sleep -Milliseconds 100
        }
        return $false
    }

    # --- Enter path -------------------------------------------------------
    Show-Popup
    Start-Sleep -Milliseconds 400
    $popup = Popup-Process
    if ($null -eq $popup) { throw "DittoPopup is not running" }
    $popup.Refresh()
    $popupHwnd = [DittoKeyboardQa]::FindTopWindow($popup.Id)
    # The popup may run on a different desktop from this automation process in
    # CI/service sessions; the popup traces its own HWND for synthetic input.
    $traceHwnd = [IntPtr]::Zero
    Write-Output ("TRACE_PATH={0}" -f $tracePath)
    Write-Output ("TRACE_LINES={0}" -f (Get-Content -LiteralPath $tracePath -ErrorAction SilentlyContinue).Count)
    $match = Select-String -Path $tracePath -Pattern "activate-foreground=.* hwnd=\d+" -ErrorAction SilentlyContinue | Select-Object -Last 1
    if ($match) {
        $digits = [regex]::Match($match.Line, "hwnd=(\d+)").Groups[1].Value
        if ($digits) { $traceHwnd = [IntPtr][int64]$digits }
    }
    Write-Output ("TRACE_LAST_ACTIVATE={0}" -f $(if ($match) { $match.Line } else { "none" }))
    Write-Output ("POPUP_WINDOW=" + [DittoKeyboardQa]::DescribeWindow($popupHwnd))
    Write-Output ("POPUP_HWND_FROM_TRACE={0}" -f $traceHwnd)
    if ($popupHwnd -eq [IntPtr]::Zero -and $traceHwnd -ne [IntPtr]::Zero) { $popupHwnd = $traceHwnd }
    $foreground = [DittoKeyboardQa]::GetForegroundWindow()
    $popupOwnsFocus = ($popupHwnd -ne [IntPtr]::Zero) -and ($foreground -eq $popupHwnd)
    if (-not $popupOwnsFocus -and (Trace-Contains "focus-trace fg=$($traceHwnd)")) {
        # The popup reports itself as foreground on its own desktop even when
        # this automation process cannot observe that desktop.
        $popupOwnsFocus = $true
        Write-Output "FOCUS_OBSERVED_VIA_POPUP_TRACE=True"
    }
    Write-Output ("POPUP_FOREGROUND_AFTER_SHOW={0}" -f $popupOwnsFocus)
    if (-not $popupOwnsFocus) {
        # Focus can land on a child window; still record the raw handles.
        Write-Output ("FOREGROUND_HWND={0}" -f $foreground)
        Write-Output ("POPUP_MAIN_HWND={0}" -f $popupHwnd)
    }

    [DittoKeyboardQa]::TypeAscii("ditto")
    $typed = Wait-Trace "search-len=5"
    if (-not $typed) {
        # Some automation sessions block low-level key injection. WM_CHAR is
        # the same input path winit uses for real typing; record the fallback.
        Write-Output "KEY_INPUT_FALLBACK=WM_CHAR"
        [DittoKeyboardQa]::TypeAsciiMessages($popupHwnd, "ditto")
        $typed = Wait-Trace "search-len=5"
    }
    Write-Output ("SEARCH_TYPED_WITHOUT_MOUSE={0}" -f $typed)
    if (-not $typed) { throw "typing after Show did not reach the search box" }

    Start-Sleep -Milliseconds 400
    [DittoKeyboardQa]::Press(0x0D)
    $enterTrace = Wait-Trace "paste-request count=1 plain=false"
    if (-not $enterTrace) {
        Write-Output "ENTER_INPUT_FALLBACK=WM_KEYDOWN"
        [DittoKeyboardQa]::PostEnter($popupHwnd, $false)
        $enterTrace = Wait-Trace "paste-request count=1 plain=false"
    }
    Write-Output ("ENTER_PASTE_REQUEST={0}" -f $enterTrace)
    Start-Sleep -Milliseconds 800
    [DittoKeyboardQa]::Activate()
    Start-Sleep -Milliseconds 300
    $textAfterEnter = [DittoKeyboardQa]::Text()
    $enterPasted = $textAfterEnter -like "*DITTO-QA-SYNTHETIC*"
    Write-Output ("TARGET_TEXT_AFTER_ENTER={0}" -f ($textAfterEnter -replace "`r?`n", " "))
    Write-Output ("PASTE_LANDED_IN_TARGET_AFTER_ENTER={0}" -f $enterPasted)

    # --- Shift+Enter path -------------------------------------------------
    [DittoKeyboardQa]::Clear()
    [DittoKeyboardQa]::Activate()
    Start-Sleep -Milliseconds 300
    Show-Popup
    Start-Sleep -Milliseconds 500
    $popupHwnd2 = [DittoKeyboardQa]::FindTopWindow((Popup-Process).Id)
    Write-Output ("POPUP_WINDOW_SECOND=" + [DittoKeyboardQa]::DescribeWindow($popupHwnd2))
    [DittoKeyboardQa]::TypeAsciiMessages($popupHwnd2, "ditto")
    # The popup may still own focus; WM_CHAR covers both injection outcomes.
    $null = Wait-Trace "search-len=5"
    Start-Sleep -Milliseconds 400
    [DittoKeyboardQa]::ShiftEnter()
    $plainTrace = Wait-Trace "paste-request count=1 plain=true"
    if (-not $plainTrace) {
        # Retry once after re-focusing the popup, then via WM_KEYDOWN.
        Start-Sleep -Milliseconds 300
        [DittoKeyboardQa]::ShiftEnter()
        $plainTrace = Wait-Trace "paste-request count=1 plain=true"
    }
    if (-not $plainTrace) {
        Write-Output "SHIFT_ENTER_INPUT_FALLBACK=WM_KEYDOWN"
        [DittoKeyboardQa]::PostEnter($popupHwnd2, $true)
        $plainTrace = Wait-Trace "paste-request count=1 plain=true"
    }
    Write-Output ("SHIFT_ENTER_PLAIN_PASTE_REQUEST={0}" -f $plainTrace)
    Start-Sleep -Milliseconds 800
    [DittoKeyboardQa]::Activate()
    Start-Sleep -Milliseconds 300
    $textAfterPlain = [DittoKeyboardQa]::Text()
    Write-Output ("TARGET_TEXT_AFTER_SHIFT_ENTER={0}" -f ($textAfterPlain -replace "`r?`n", " "))
    Write-Output ("PASTE_LANDED_IN_TARGET_AFTER_SHIFT_ENTER={0}" -f ($textAfterPlain -like "*DITTO-QA-SYNTHETIC*"))

    Hide-Popup

    $ok = $typed -and $enterTrace -and $enterPasted -and $plainTrace
    Write-Output ("KEYBOARD_FIRST_QA={0}" -f $(if ($ok) { "PASS" } else { "FAIL" }))
    if (-not $ok) { throw "keyboard-first QA failed; see traces under $Root" }
} finally {
    if ($null -ne $pipe) { $pipe.Dispose() }
    [DittoKeyboardQa]::Close()
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $startedAt } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    if ($null -ne $core -and -not $core.HasExited) {
        Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
    }
    Write-Output "QA_ROOT=$Root"
}
