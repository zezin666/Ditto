param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

# P-2 regression: with the tray hidden and no startup message, the first hotkey
# during the real cold-start gap must still produce visible, non-activating
# feedback. The old code treated the IPC prewarm Hello as "popup ready", so the
# ~1.7 s eframe window/GL initialization had no feedback at all.
#
# This script waits for the popup's `hello-ack-prewarm` trace, signals the same
# toggle event a second instance uses, then asserts:
#   * the native preparing hint appears before the popup window reports ready;
#   * the hint does not take focus;
#   * the hint is dismissed once the popup can render and the popup activates;
#   * signal->hint and signal->activation timings are recorded.
#
# It uses a synthetic data root, a unique valid DITTO_PIPE_SUFFIX, and never
# touches the real user data root, clipboard history, or Ditto processes.

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$binaryDir = Split-Path -Parent $DittoExe
$popupExe = Join-Path $binaryDir "DittoPopup.exe"
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-ready-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$startedAt = Get-Date

function Stop-RunPopups {
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $popupExe -and $_.StartTime -ge $startedAt } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DittoReadyQa {
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern IntPtr OpenEventW(uint access, bool inherit, string name);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetEvent(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr handle);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "FindWindowW")]
    public static extern IntPtr FindWindowByClass(string className, IntPtr windowName);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
    public static IntPtr FindHintWindow() {
        return FindWindowByClass("DittoNativeHint", IntPtr.Zero);
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

function Now-Ms {
    return [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
}

function Wait-TraceMatch([string]$path, [string]$pattern, [int]$timeoutMs) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $path) {
            $match = Select-String -LiteralPath $path -Pattern $pattern | Select-Object -First 1
            if ($null -ne $match) { return $match }
        }
        Start-Sleep -Milliseconds 10
    }
    return $null
}

function Wait-Hint([int]$timeoutMs) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        $hwnd = [DittoReadyQa]::FindHintWindow()
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        Start-Sleep -Milliseconds 10
    }
    return [IntPtr]::Zero
}

function Wait-HintClosed([int]$timeoutMs) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        if ([DittoReadyQa]::FindHintWindow() -eq [IntPtr]::Zero) { return $true }
        Start-Sleep -Milliseconds 20
    }
    return $false
}

function Save-HintScreenshot([IntPtr]$handle, [string]$path) {
    $rect = New-Object DittoReadyQa+RECT
    if (-not [DittoReadyQa]::GetWindowRect($handle, [ref]$rect)) { return $false }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { return $false }
    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    try {
        $printed = [DittoReadyQa]::PrintWindow($handle, $hdc, 2)
    } finally {
        $graphics.ReleaseHdc($hdc)
        $graphics.Dispose()
    }
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $nonBlack = 0
    for ($x = 0; $x -lt $width; $x += [Math]::Max(1, [int]($width / 16))) {
        for ($y = 0; $y -lt $height; $y += [Math]::Max(1, [int]($height / 8))) {
            $pixel = $bitmap.GetPixel($x, $y)
            if ($pixel.R -gt 8 -or $pixel.G -gt 8 -or $pixel.B -gt 8) { $nonBlack++ }
        }
    }
    $bitmap.Dispose()
    return ($printed -and $nonBlack -gt 0)
}

function Seed([string]$dataRoot, [string]$settingsJson) {
    $cargoArgs = @(
        "run", "--quiet", "--manifest-path", (Join-Path $repoRoot "Cargo.toml"),
        "-p", "ditto-storage", "--example", "seed_qa", "--", $dataRoot, "1", $settingsJson
    )
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "failed to seed QA settings" }
}

$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$suffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
$dataRoot = Join-Path $Root "data"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
$env:DITTO_PIPE_SUFFIX = $suffix
$env:DITTO_DATA_DIR = $dataRoot
$hostTrace = Join-Path $Root "host-trace.log"
$popupTrace = Join-Path $Root "popup-trace.log"
$env:DITTO_HOST_TRACE = $hostTrace
$env:DITTO_POPUP_TRACE = $popupTrace
$env:DITTO_HINT_TRACE = Join-Path $Root "hint-trace.log"
$env:DITTO_PERF_LOG = Join-Path $Root "popup-latency.csv"
Remove-Item Env:\DITTO_POPUP_PATH -ErrorAction SilentlyContinue

if (-not (Test-Path $popupExe)) {
    throw "DittoPopup.exe was not found beside $DittoExe; build the popup sidecar first"
}

$ok = $true
$core = $null
$eventHandle = [IntPtr]::Zero
try {
    Seed $dataRoot '{"general":{"show_tray_icon":false,"startup_message":""}}'

    $core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru

    $prewarm = Wait-TraceMatch $popupTrace "hello-ack-prewarm" 30000
    Write-Output ("PREWARM_SEEN={0}" -f ($null -ne $prewarm))
    if ($null -eq $prewarm) { throw "popup never reported IPC prewarm" }

    # Signal the same named toggle event the fast single-instance path uses.
    # Opening the event directly avoids adding a second Ditto.exe startup to
    # the measurement.
    $eventName = "Local\Ditto.v2.$sid.toggle.$suffix"
    $eventHandle = [DittoReadyQa]::OpenEventW(0x0002, $false, $eventName)
    if ($eventHandle -eq [IntPtr]::Zero) { throw "could not open toggle event $eventName" }
    $signalAt = Now-Ms
    if (-not [DittoReadyQa]::SetEvent($eventHandle)) { throw "could not signal the toggle event" }

    $hint = Wait-Hint 5000
    $hintAt = Now-Ms
    $hintVisible = $hint -ne [IntPtr]::Zero
    Write-Output ("HINT_VISIBLE_AFTER_SIGNAL={0}" -f $hintVisible)

    $hintStoleFocus = $false
    if ($hintVisible) {
        $foreground = [DittoReadyQa]::GetForegroundWindow()
        $hintStoleFocus = ($foreground -eq $hint)
        Write-Output ("FOREGROUND_IS_HINT={0}" -f $hintStoleFocus)
        $screenshot = Join-Path $Root "preparing-hint.png"
        $captured = Save-HintScreenshot $hint $screenshot
        Write-Output ("PREPARING_HINT_SCREENSHOT={0}" -f $captured)
        Write-Output ("PREPARING_HINT_SCREENSHOT_PATH={0}" -f $screenshot)
    }
    if (-not $hintVisible -or $hintStoleFocus) { $ok = $false }

    $ready = Wait-TraceMatch $popupTrace "window-ready-ack" 20000
    $readySeen = $null -ne $ready
    $readyAt = $null
    if ($readySeen) {
        $readyAt = [int64](($ready.Line -split "\s+")[0])
    }
    Write-Output ("POPUP_WINDOW_READY_SEEN={0}" -f $readySeen)
    if (-not $readySeen) { $ok = $false }

    $hintBeforeReady = ($hintVisible -and $readySeen -and ($hintAt -lt $readyAt))
    Write-Output ("HINT_VISIBLE_BEFORE_WINDOW_READY={0}" -f $hintBeforeReady)
    if (-not $hintBeforeReady) { $ok = $false }

    $hintClosed = $false
    if ($hintVisible) {
        $hintClosed = Wait-HintClosed 5000
    }
    Write-Output ("HINT_DISMISSED_AFTER_WINDOW_READY={0}" -f $hintClosed)
    if ($hintVisible -and -not $hintClosed) { $ok = $false }

    $activation = Wait-TraceMatch $popupTrace "activate-foreground=" 20000
    $activationSeen = $null -ne $activation
    $activationAt = $null
    if ($activationSeen) {
        $activationAt = [int64](($activation.Line -split "\s+")[0])
    }
    Write-Output ("POPUP_ACTIVATION_SEEN={0}" -f $activationSeen)
    if (-not $activationSeen) { $ok = $false }

    if ($hintVisible) {
        Write-Output ("SIGNAL_TO_HINT_MS={0}" -f ($hintAt - $signalAt))
    }
    if ($readySeen) {
        Write-Output ("SIGNAL_TO_WINDOW_READY_MS={0}" -f ($readyAt - $signalAt))
    }
    if ($activationSeen) {
        Write-Output ("SIGNAL_TO_ACTIVATION_MS={0}" -f ($activationAt - $signalAt))
    }

    Write-Output ("POPUP_READY_FEEDBACK_QA={0}" -f $(if ($ok) { "PASS" } else { "FAIL" }))
    if (-not $ok) { throw "popup-ready feedback assertions failed" }
} finally {
    if ($eventHandle -ne [IntPtr]::Zero) { $null = [DittoReadyQa]::CloseHandle($eventHandle) }
    if ($null -ne $core -and -not $core.HasExited) {
        Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
        $core.WaitForExit(5000) | Out-Null
    }
    Stop-RunPopups
    Write-Output "QA_ROOT=$Root"
}
