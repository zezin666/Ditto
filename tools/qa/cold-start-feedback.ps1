param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$binaryDir = Split-Path -Parent $DittoExe
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-feedback-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DittoHintQa {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "FindWindowW")]
    public static extern IntPtr FindWindowByClass(string className, IntPtr windowName);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);
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

function Wait-HintWindow([int]$timeoutMs = 20000) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        $hwnd = [DittoHintQa]::FindHintWindow()
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Wait-HintClosed([IntPtr]$handle, [int]$timeoutMs = 12000) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        if ([DittoHintQa]::FindHintWindow() -eq [IntPtr]::Zero) { return $true }
        Start-Sleep -Milliseconds 150
    }
    return $false
}

function Save-HintScreenshot([IntPtr]$handle, [string]$path) {
    $rect = New-Object DittoHintQa+RECT
    if (-not [DittoHintQa]::GetWindowRect($handle, [ref]$rect)) { return $false }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { return $false }
    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    try {
        $printed = [DittoHintQa]::PrintWindow($handle, $hdc, 2)
    } finally {
        $graphics.ReleaseHdc($hdc)
        $graphics.Dispose()
    }
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    # A locked/headless desktop can return an all-black frame. Report rather
    # than fail, and never fabricate a visual result.
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

function Stop-RunProcesses([datetime]$startedAt) {
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "$binaryDir\*" -and $_.StartTime -ge $startedAt } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

function Seed([string]$dataRoot, [string]$settingsJson) {
    $cargoArgs = @(
        "run", "--quiet", "--manifest-path", (Join-Path $repoRoot "Cargo.toml"),
        "-p", "ditto-storage", "--example", "seed_qa", "--", $dataRoot, "1", $settingsJson
    )
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "failed to seed QA settings" }
}

$ok = $true

# --- Scenario 1: configurable startup message -----------------------------
$startupRoot = Join-Path $Root "startup-message"
$startupData = Join-Path $startupRoot "data"
New-Item -ItemType Directory -Force -Path $startupData | Out-Null
$startupSuffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
$env:DITTO_PIPE_SUFFIX = $startupSuffix
$env:DITTO_DATA_DIR = $startupData
$env:DITTO_HOST_TRACE = Join-Path $startupRoot "host-trace.log"
$env:DITTO_HINT_TRACE = Join-Path $startupRoot "hint-trace.log"
Remove-Item Env:\DITTO_POPUP_TRACE -ErrorAction SilentlyContinue
Seed $startupData '{"general":{"startup_message":"DITTO-QA-STARTUP-HINT"}}'
$startupAt = Get-Date
$startupCore = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
try {
    $hint = Wait-HintWindow
    $startupVisible = $hint -ne [IntPtr]::Zero
    Write-Output ("STARTUP_MESSAGE_HINT_VISIBLE={0}" -f $startupVisible)
    if ($startupVisible) {
        $screenshot = Join-Path $startupRoot "startup-hint.png"
        $captured = Save-HintScreenshot $hint $screenshot
        Write-Output ("STARTUP_MESSAGE_HINT_SCREENSHOT={0}" -f $captured)
        Write-Output ("STARTUP_MESSAGE_HINT_SCREENSHOT_PATH={0}" -f $screenshot)
        $closed = Wait-HintClosed $hint
        Write-Output ("STARTUP_MESSAGE_HINT_AUTO_CLOSED={0}" -f $closed)
        if (-not $closed) { $ok = $false }
    } else {
        $ok = $false
    }
} finally {
    if (-not $startupCore.HasExited) {
        Stop-Process -Id $startupCore.Id -Force -ErrorAction SilentlyContinue
        $startupCore.WaitForExit(5000) | Out-Null
    }
    Stop-RunProcesses $startupAt
}

# --- Scenario 2: first hotkey while the popup is not ready ----------------
# The tray is hidden and the popup executable is deliberately unavailable, so
# the toggle path must still show a native, non-blocking preparing hint.
$hotkeyRoot = Join-Path $Root "hotkey-hint"
$hotkeyData = Join-Path $hotkeyRoot "data"
New-Item -ItemType Directory -Force -Path $hotkeyData | Out-Null
$hotkeySuffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
$env:DITTO_PIPE_SUFFIX = $hotkeySuffix
$env:DITTO_DATA_DIR = $hotkeyData
$env:DITTO_HOST_TRACE = Join-Path $hotkeyRoot "host-trace.log"
$env:DITTO_HINT_TRACE = Join-Path $hotkeyRoot "hint-trace.log"
$env:DITTO_POPUP_PATH = Join-Path $hotkeyRoot "missing\DittoPopup.exe"
Seed $hotkeyData '{"general":{"show_tray_icon":false,"startup_message":""}}'
$hotkeyAt = Get-Date
$hotkeyCore = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
try {
    # Wait for the isolated pipe, then trigger the same toggle path a second
    # launch uses. ShowPopup over IPC bypasses toggle_popup, so use the signal.
    $hotkeyPipe = "ditto-$sid-$hotkeySuffix"
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
        $names = [System.IO.Directory]::GetFiles("//./pipe/") | ForEach-Object { [System.IO.Path]::GetFileName($_) }
        if ($names -contains $hotkeyPipe) { break }
        Start-Sleep -Milliseconds 100
    }
    $second = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    $second.WaitForExit(15000) | Out-Null
    $hint = Wait-HintWindow 15000
    $hotkeyVisible = $hint -ne [IntPtr]::Zero
    Write-Output ("HOTKEY_PREPARING_HINT_VISIBLE={0}" -f $hotkeyVisible)
    if ($hotkeyVisible) {
        $screenshot = Join-Path $hotkeyRoot "hotkey-hint.png"
        $captured = Save-HintScreenshot $hint $screenshot
        Write-Output ("HOTKEY_PREPARING_HINT_SCREENSHOT={0}" -f $captured)
        Write-Output ("HOTKEY_PREPARING_HINT_SCREENSHOT_PATH={0}" -f $screenshot)
        $closed = Wait-HintClosed $hint
        Write-Output ("HOTKEY_PREPARING_HINT_AUTO_CLOSED={0}" -f $closed)
        if (-not $closed) { $ok = $false }
    } else {
        $ok = $false
    }
} finally {
    if (-not $hotkeyCore.HasExited) {
        Stop-Process -Id $hotkeyCore.Id -Force -ErrorAction SilentlyContinue
        $hotkeyCore.WaitForExit(5000) | Out-Null
    }
    Stop-RunProcesses $hotkeyAt
    Remove-Item Env:\DITTO_POPUP_PATH -ErrorAction SilentlyContinue
}

Write-Output ("COLD_START_FEEDBACK_QA={0}" -f $(if ($ok) { "PASS" } else { "FAIL" }))
if (-not $ok) { throw "cold-start feedback QA assertions failed" }
Write-Output "QA_ROOT=$Root"
