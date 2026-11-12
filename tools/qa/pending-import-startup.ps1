param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

# P-1 regression: a staged import/restore/migration must replace the data root
# BEFORE the first startup settings snapshot is read. The imported open chord,
# startup message, language, tray policy, and clipboard settings must win, and
# the old snapshot must not be reconciled back over them.
#
# The script uses two synthetic data roots, a real export package created by
# ditto-migrate, a temporary DITTO_DATA_DIR, and a unique valid
# DITTO_PIPE_SUFFIX. It never reads the real user data root or clipboard.

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$binaryDir = Split-Path -Parent $DittoExe
$popupExe = Join-Path $binaryDir "DittoPopup.exe"
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-pending-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$startedAt = Get-Date

function Stop-RunPopups {
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $popupExe -and $_.StartTime -ge $startedAt } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DittoPendingQa {
    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool RegisterHotKey(IntPtr hwnd, int id, uint modifiers, uint vk);
    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool UnregisterHotKey(IntPtr hwnd, int id);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "FindWindowW")]
    public static extern IntPtr FindWindowByClass(string className, IntPtr windowName);
    public static bool CanRegister(uint modifiers, uint vk) {
        bool ok = RegisterHotKey(IntPtr.Zero, 0x7B11, modifiers, vk);
        if (ok) { UnregisterHotKey(IntPtr.Zero, 0x7B11); }
        return ok;
    }
    public static IntPtr FindHintWindow() {
        return FindWindowByClass("DittoNativeHint", IntPtr.Zero);
    }
}
"@

$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$suffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
$env:DITTO_PIPE_SUFFIX = $suffix
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
function Call($stream, [string]$type) {
    $script:requestId++
    Send-Frame $stream 1 $script:requestId ('{"type":"' + $type + '"}')
    return Receive-Response $stream $script:requestId
}
function Wait-Pipe([int]$timeoutMs = 30000) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        $names = [System.IO.Directory]::GetFiles("//./pipe/") | ForEach-Object { [System.IO.Path]::GetFileName($_) }
        if ($names -contains $pipeName) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}
function Seed([string]$dataRoot, [string]$settingsJson, [int]$count = 1) {
    $cargoArgs = @(
        "run", "--quiet", "--manifest-path", (Join-Path $repoRoot "Cargo.toml"),
        "-p", "ditto-storage", "--example", "seed_qa", "--", $dataRoot, "$count", $settingsJson
    )
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "failed to seed synthetic data root $dataRoot" }
}

$ok = $true
$core = $null
$pipe = $null
$targetRoot = Join-Path $Root "target"
$targetData = Join-Path $targetRoot "data"
$sourceRoot = Join-Path $Root "source"
$sourceData = Join-Path $sourceRoot "data"
$package = Join-Path $Root "t15-import.zip"
New-Item -ItemType Directory -Force -Path $targetData, $sourceData | Out-Null

try {
    # Target contains the pre-import settings. The startup message is empty so
    # any visible startup hint proves the imported non-empty message was used.
    Seed $targetData '{"hotkeys":{"open_chord":"Ctrl+Alt+K","plain_text_paste_chord":"","copy_buffers":[]},"general":{"language":"en","startup_message":"","show_tray_icon":true},"clipboard":{"max_entries":5}}' 1
    # The import package carries different values for every startup decision.
    Seed $sourceData '{"hotkeys":{"open_chord":"Ctrl+Alt+Q","plain_text_paste_chord":"","copy_buffers":[]},"general":{"language":"zh-CN","startup_message":"DITTO-T15-NEW-MESSAGE","show_tray_icon":false},"clipboard":{"max_entries":17}}' 1

    $exportArgs = @(
        "run", "--quiet", "--manifest-path", (Join-Path $repoRoot "Cargo.toml"),
        "-p", "ditto-migration", "--bin", "ditto-migrate", "--",
        "export", "--source", $sourceData, "--dest", $package, "--zip"
    )
    & cargo @exportArgs
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $package)) {
        throw "failed to build the synthetic import package"
    }

    # Stage exactly what import_data/restore_db stage: copy next to the data
    # root and write the marker without a BOM.
    $staged = "$targetData.pending-import.zip"
    $marker = "$targetData.pending-import.json"
    Copy-Item -LiteralPath $package -Destination $staged -Force
    $pendingJson = (@{ package = $staged; requested_at_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() } | ConvertTo-Json -Compress)
    [System.IO.File]::WriteAllText($marker, $pendingJson, (New-Object System.Text.UTF8Encoding($false)))

    $env:DITTO_DATA_DIR = $targetData
    $env:DITTO_HOST_TRACE = Join-Path $Root "host-trace.log"
    $env:DITTO_POPUP_TRACE = Join-Path $Root "popup-trace.log"
    $env:DITTO_HINT_TRACE = Join-Path $Root "hint-trace.log"
    $env:DITTO_PERF_LOG = Join-Path $Root "popup-latency.csv"
    Remove-Item Env:\DITTO_POPUP_PATH -ErrorAction SilentlyContinue

    $core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    if (-not (Wait-Pipe)) { throw "core did not create the isolated pipe $pipeName" }
    $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
    $pipe.Connect(5000)
    $script:requestId = 1
    Send-Frame $pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
    $null = Receive-Response $pipe $script:requestId

    # Watch for the startup hint while waiting for setup to finish. The hint is
    # only shown when the runtime startup message is non-empty, which is true
    # only for the imported value.
    $hintSeen = $false
    $hostTrace = $env:DITTO_HOST_TRACE
    $deadline = (Get-Date).AddSeconds(45)
    $startupComplete = $false
    while ((Get-Date) -lt $deadline) {
        if (-not $hintSeen -and [DittoPendingQa]::FindHintWindow() -ne [IntPtr]::Zero) {
            $hintSeen = $true
        }
        if ((Test-Path $hostTrace) -and (Select-String -Path $hostTrace -Pattern "startup-complete" -Quiet)) {
            $startupComplete = $true
            break
        }
        if ($core.HasExited) { throw "core exited during startup (code $($core.ExitCode))" }
        Start-Sleep -Milliseconds 50
    }
    Write-Output ("STARTUP_COMPLETE={0}" -f $startupComplete)
    if (-not $startupComplete) { throw "startup did not complete" }
    if (-not $hintSeen -and [DittoPendingQa]::FindHintWindow() -ne [IntPtr]::Zero) {
        $hintSeen = $true
    }

    $settings = Call $pipe "GetSettings"
    $values = $settings.values
    $openChord = $values.hotkeys.open_chord
    $startupMessage = $values.general.startup_message
    $language = $values.general.language
    $showTray = $values.general.show_tray_icon
    $maxEntries = $values.clipboard.max_entries

    Write-Output ("OPEN_CHORD_AFTER_IMPORT={0}" -f $openChord)
    Write-Output ("STARTUP_MESSAGE_AFTER_IMPORT={0}" -f $startupMessage)
    Write-Output ("LANGUAGE_AFTER_IMPORT={0}" -f $language)
    Write-Output ("SHOW_TRAY_AFTER_IMPORT={0}" -f $showTray)
    Write-Output ("MAX_ENTRIES_AFTER_IMPORT={0}" -f $maxEntries)

    $openImported = $openChord -eq "Ctrl+Alt+Q"
    $messageImported = $startupMessage -eq "DITTO-T15-NEW-MESSAGE"
    $languageImported = $language -eq "zh-CN"
    $trayImported = ($showTray -eq $false)
    $clipboardImported = ($maxEntries -eq 17)
    $noStaleWriteBack = ($openChord -ne "Ctrl+Alt+K")
    if (-not ($openImported -and $messageImported -and $languageImported -and $trayImported -and $clipboardImported -and $noStaleWriteBack)) {
        $ok = $false
    }

    # The imported open chord must actually be the registered one, and the old
    # chord must be free for other applications.
    $qRegistered = -not [DittoPendingQa]::CanRegister(0x0003, 0x51)
    $kFree = [DittoPendingQa]::CanRegister(0x0003, 0x4B)
    Write-Output ("IMPORTED_HOTKEY_Q_REGISTERED={0}" -f $qRegistered)
    Write-Output ("OLD_HOTKEY_K_FREE={0}" -f $kFree)
    if (-not ($qRegistered -and $kFree)) { $ok = $false }

    $traceText = if (Test-Path $hostTrace) { Get-Content -LiteralPath $hostTrace -Raw } else { "" }
    $activeQ = $traceText -match "hotkey-active=Ctrl\+Alt\+Q"
    $activeK = $traceText -match "hotkey-active=Ctrl\+Alt\+K"
    $trayHidden = $traceText -match "tray-hidden"
    $trayCreated = $traceText -match "tray-created"
    Write-Output ("HOST_TRACE_ACTIVE_OPEN_Q={0}" -f $activeQ)
    Write-Output ("HOST_TRACE_ACTIVE_OLD_OPEN_K={0}" -f $activeK)
    Write-Output ("HOST_TRACE_TRAY_HIDDEN={0}" -f $trayHidden)
    Write-Output ("HOST_TRACE_TRAY_CREATED={0}" -f $trayCreated)
    if (-not ($activeQ -and -not $activeK -and $trayHidden -and -not $trayCreated)) { $ok = $false }

    # The early popup process is launched with the imported language.
    $popupLanguageZh = $false
    for ($i = 0; $i -lt 100; $i++) {
        $process = Get-CimInstance Win32_Process -Filter "Name='DittoPopup.exe'" -ErrorAction SilentlyContinue |
            Where-Object { $_.ExecutablePath -eq $popupExe } |
            Select-Object -First 1
        if ($null -ne $process) {
            $popupLanguageZh = $process.CommandLine -match "--lang=zh-CN"
            break
        }
        Start-Sleep -Milliseconds 100
    }
    Write-Output ("POPUP_LANGUAGE_ZH={0}" -f $popupLanguageZh)
    if (-not $popupLanguageZh) { $ok = $false }

    Write-Output ("STARTUP_MESSAGE_HINT_VISIBLE={0}" -f $hintSeen)
    if (-not $hintSeen) { $ok = $false }

    Write-Output ("PENDING_IMPORT_STARTUP_QA={0}" -f $(if ($ok) { "PASS" } else { "FAIL" }))
    if (-not $ok) { throw "pending import startup assertions failed" }
} finally {
    if ($null -ne $pipe) { $pipe.Dispose() }
    if ($null -ne $core -and -not $core.HasExited) {
        Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
        $core.WaitForExit(5000) | Out-Null
    }
    Stop-RunPopups
    Write-Output "QA_ROOT=$Root"
}
