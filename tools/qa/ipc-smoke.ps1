param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = "",
    [int]$ShowCycles = 12
)

$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class QaWin32 {
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
}
"@

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$dataRoot = Join-Path $Root "data"
$perfLog = Join-Path $Root "popup-latency.csv"
$resultDir = Join-Path $Root "results"
New-Item -ItemType Directory -Force -Path $resultDir | Out-Null

$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$pipePath = "\\.\pipe\ditto-$sid"

trap {
    if ($null -ne $core -and -not $core.HasExited) {
        Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $startedAt) {
        Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
            Where-Object { $_.StartTime -ge $startedAt } |
            Stop-Process -Force -ErrorAction SilentlyContinue
    }
    [Console]::Error.WriteLine("QA_TRAP: $_")
    exit 1
}

function Assert($condition, $message) {
    if (-not $condition) { throw "ASSERT: $message" }
    Write-Output "PASS $message"
}

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
    [BitConverter]::GetBytes([uint16]0).CopyTo($header, 8)
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
        if ([Text.Encoding]::ASCII.GetString($header, 0, 4) -ne "DTP1") { throw "bad frame magic" }
        $frameType = [BitConverter]::ToUInt16($header, 6)
        $requestId = [BitConverter]::ToUInt64($header, 16)
        $length = [BitConverter]::ToUInt32($header, 12)
        $payload = New-Object byte[] $length
        if ($length -gt 0) { Read-Exact $stream $payload }
        if ($frameType -eq 2 -and $requestId -eq $id) {
            return [Text.Encoding]::UTF8.GetString($payload)
        }
        # Event frames are broadcast to every client; ignore them here.
    }
}

$script:requestId = 1
function Call($stream, [string]$type, [string]$fields = "") {
    $script:requestId++
    $json = if ($fields) {
        "{`"type`":`"$type`",$fields}"
    } else {
        "{`"type`":`"$type`"}"
    }
    Send-Frame $stream 1 $script:requestId $json
    $response = Receive-Response $stream $script:requestId
    return $response | ConvertFrom-Json
}

# --- start core -----------------------------------------------------------
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
& cargo run --quiet --manifest-path (Join-Path $repoRoot "Cargo.toml") -p ditto-storage --example seed_qa -- $dataRoot
if ($LASTEXITCODE -ne 0) { throw "failed to seed the synthetic data root" }
$env:DITTO_DATA_DIR = $dataRoot
$env:DITTO_PERF_LOG = $perfLog
$env:DITTO_POPUP_TRACE = (Join-Path $Root "popup-trace.log")
$core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
Write-Output "CORE_PID=$($core.Id)"
$startedAt = Get-Date
$pipeReady = $false
for ($i = 0; $i -lt 200; $i++) {
    Start-Sleep -Milliseconds 50
    $pipeNames = [System.IO.Directory]::GetFiles("//./pipe/") |
        ForEach-Object { [System.IO.Path]::GetFileName($_) }
    if ($pipeNames -contains "ditto-$sid") { $pipeReady = $true; break }
    if ($core.HasExited) { throw "Ditto exited during startup with code $($core.ExitCode)" }
}
Assert $pipeReady "core created the per-user named pipe"
Assert (Test-Path -LiteralPath (Join-Path $dataRoot "ditto.db")) "core created ditto.db under the synthetic data root"
$mainHandle = $core.MainWindowHandle
$visibleMain = ($mainHandle -ne 0) -and [QaWin32]::IsWindowVisible($mainHandle)
$mainTitle = $core.MainWindowTitle
# Tauri's single-instance plugin owns a hidden helper window; only user-facing
# titles count for the "no window at startup" requirement.
Assert ($mainTitle -ne "Ditto" -and $mainTitle -ne "Ditto Settings") "core shows no user-facing window at startup"
Write-Output "CORE_MAIN_WINDOW_TITLE=$mainTitle"
Write-Output "CORE_MAIN_WINDOW_VISIBLE=$visibleMain"

$clipText = "DITTO-QA-SYNTHETIC"
$tracePath = Join-Path $Root "popup-trace.log"
$popupReady = $false
for ($i = 0; $i -lt 300; $i++) {
    Start-Sleep -Milliseconds 100
    if ((Test-Path -LiteralPath $tracePath) -and
        (Select-String -Path $tracePath -Pattern "hello-ack" -Quiet)) {
        $popupReady = $true
        break
    }
}
$popupReadyMs = ((Get-Date) - $startedAt).TotalMilliseconds
Write-Output ("POPUP_READY={0} POPUP_READY_MS={1:N1}" -f $popupReady, $popupReadyMs)
Start-Sleep -Milliseconds 800
$earlyPopup = Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
    Where-Object { $_.StartTime -ge $startedAt } | Select-Object -First 1
$earlyPerf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $_.IDProcess -eq $core.Id -or ($earlyPopup -and $_.IDProcess -eq $earlyPopup.Id) }
$earlyCore = $earlyPerf | Where-Object { $_.IDProcess -eq $core.Id } | Select-Object -First 1
$earlyPopupPerf = if ($earlyPopup) { $earlyPerf | Where-Object { $_.IDProcess -eq $earlyPopup.Id } | Select-Object -First 1 } else { $null }
$earlyTotal = [int64]$earlyCore.WorkingSetPrivate + [int64]$(if ($earlyPopupPerf) { $earlyPopupPerf.WorkingSetPrivate } else { 0 })
Write-Output ("IDLE_PRE_SHOW_PRIVATE_BYTES={0}" -f $earlyTotal)

# --- IPC session ----------------------------------------------------------
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", "ditto-$sid", [System.IO.Pipes.PipeDirection]::InOut)
$pipe.Connect(5000)
$stream = $pipe
$hello = Call $stream "Hello" "`"proto_min`":1,`"proto_max`":1,`"client`":`"cli`",`"pid`":$PID,`"nonce`":`"qa`",`"subscribe`":false"
Assert ($hello.type -eq "HelloAck") "Hello returns HelloAck (protocol $($hello.proto))"
Write-Output ("CAPABILITIES=" + ($hello.capabilities -join ","))

$settings = Call $stream "GetSettings"
Assert ($settings.type -eq "Settings") "GetSettings returns a real settings snapshot"
Assert ($null -ne $settings.values.general.language) "settings include general.language"

$groups = Call $stream "GetGroups"
Assert ($groups.type -eq "Groups") "GetGroups returns a group list"

$created = Call $stream "CreateGroup" "`"name`":`"QA IPC group`",`"parent_id`":null,`"sort_order`":1.0"
Assert ($created.type -eq "Ok") "CreateGroup succeeds"
$groups = Call $stream "GetGroups"
$qaGroup = $groups.groups | Where-Object { $_.name -eq "QA IPC group" } | Select-Object -First 1
Assert ($null -ne $qaGroup) "created group is listed"

$clips = Call $stream "GetClips" "`"query`":`"`",`"limit`":50"
Assert ($clips.type -eq "Clips") "GetClips returns clips"
Assert ($clips.items.Count -ge 3) "seeded synthetic clips are visible ($($clips.items.Count) clip(s))"
$clip = $clips.items | Where-Object { $_.preview_text -eq "DITTO-QA-SYNTHETIC seed clip" } | Select-Object -First 1
Assert ($null -ne $clip) "seeded clip is the known synthetic text"

$search = Call $stream "Search" "`"query`":`"$clipText`",`"limit`":50"
Assert ($search.type -eq "Clips" -and $search.items.Count -ge 1) "Search finds the synthetic clip"

$pin = Call $stream "SetPinned" "`"clip_id`":$($clip.id),`"pinned`":true"
Assert ($pin.type -eq "Ok") "SetPinned succeeds"
$move = Call $stream "MoveToGroup" "`"clip_id`":$($clip.id),`"group_id`":$($qaGroup.id)"
Assert ($move.type -eq "Ok") "MoveToGroup succeeds"
$buffer = Call $stream "CopyBufferAssign" "`"slot`":1,`"clip_id`":$($clip.id)"
Assert ($buffer.type -eq "Ok") "CopyBufferAssign succeeds"
$buffers = Call $stream "GetCopyBuffers"
Assert ($buffers.type -eq "CopyBuffers" -and $buffers.slots[0].clip_id -eq $clip.id) "copy buffer 1 references the synthetic clip"
$rename = Call $stream "RenameGroup" "`"group_id`":$($qaGroup.id),`"name`":`"QA IPC group renamed`""
Assert ($rename.type -eq "Ok") "RenameGroup succeeds"

# --- popup show/hide latency ---------------------------------------------
for ($i = 0; $i -lt $ShowCycles; $i++) {
    $show = Call $stream "ShowPopup"
    Start-Sleep -Milliseconds 350
    $hide = Call $stream "HidePopup"
    Start-Sleep -Milliseconds 150
}
Start-Sleep -Milliseconds 300

$delete = Call $stream "Delete" "`"clip_ids`":[$($clip.id)]"
Assert ($delete.type -eq "Ok") "Delete removes the synthetic clip"
$clipsAfter = Call $stream "GetClips" "`"query`":`"`",`"limit`":50"
Assert (-not ($clipsAfter.items | Where-Object { $_.id -eq $clip.id })) "deleted clip is gone"
$deleteGroup = Call $stream "DeleteGroup" "`"group_id`":$($qaGroup.id)"
Assert ($deleteGroup.type -eq "Ok") "DeleteGroup succeeds"

$pipe.Dispose()

# --- settings instance latency -------------------------------------------
$settingsWindow = $null
$openMs = $null
$exitMs = $null
$second = $null
try {
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $second = Start-Process -FilePath $DittoExe -ArgumentList "--settings" -PassThru
    for ($i = 0; $i -lt 600; $i++) {
        Start-Sleep -Milliseconds 10
        $current = Get-Process -Id $core.Id -ErrorAction Stop
        if ($current.MainWindowTitle -eq "Ditto Settings") {
            $openMs = $stopwatch.Elapsed.TotalMilliseconds
            $settingsWindow = $current.MainWindowHandle
            break
        }
    }
    $exitStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $second.WaitForExit(5000) | Out-Null
    $exitMs = $exitStopwatch.Elapsed.TotalMilliseconds
} catch {
    Write-Output "SETTINGS_LATENCY_UNVERIFIED=$($_.Exception.Message)"
}

if ($null -ne $openMs) {
    Write-Output ("SETTINGS_OPEN_MS={0:N1}" -f $openMs)
    Write-Output ("SECOND_INSTANCE_EXIT_MS={0:N1}" -f $exitMs)
    if ($settingsWindow -and $settingsWindow -ne 0) {
        $null = [QaWin32]::PostMessage($settingsWindow, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 500
    }
} else {
    Write-Output "SETTINGS_LATENCY_UNVERIFIED=settings window not observed"
}

# --- memory samples -------------------------------------------------------
$samples = @()
for ($i = 0; $i -lt 3; $i++) {
    Start-Sleep -Milliseconds 500
    $coreNow = Get-Process -Id $core.Id -ErrorAction Stop
    $popup = Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $startedAt } |
        Select-Object -First 1
    $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
        Where-Object { $_.IDProcess -eq $core.Id -or ($popup -and $_.IDProcess -eq $popup.Id) }
    $corePerf = $perf | Where-Object { $_.IDProcess -eq $core.Id } | Select-Object -First 1
    $popupPerf = if ($popup) { $perf | Where-Object { $_.IDProcess -eq $popup.Id } | Select-Object -First 1 } else { $null }
    $corePrivate = if ($corePerf) { $corePerf.WorkingSetPrivate } else { 0 }
    $popupPrivate = if ($popupPerf) { $popupPerf.WorkingSetPrivate } else { 0 }
    $samples += [pscustomobject]@{
        sample = $i + 1
        core_working_set = $coreNow.WorkingSet64
        popup_working_set = if ($popup) { $popup.WorkingSet64 } else { 0 }
        core_private_ws = $corePrivate
        popup_private_ws = $popupPrivate
        total_private_ws = $corePrivate + $popupPrivate
    }
}
$samples | Format-Table -AutoSize | Out-String | Write-Output
$samples | Export-Csv -LiteralPath (Join-Path $resultDir "idle-memory.csv") -NoTypeInformation
Write-Output ("IDLE_TOTAL_PRIVATE_BYTES={0}" -f $samples[1].total_private_ws)

# --- popup latency summary ------------------------------------------------
if (Test-Path -LiteralPath $perfLog) {
    $rows = Get-Content -LiteralPath $perfLog | ForEach-Object {
        $parts = $_ -split ","
        [pscustomobject]@{
            timestamp_ms = [int64]$parts[0]
            latency_us = [int64]$parts[1]
            dpi = [double]$parts[2]
            provider = $parts[3]
        }
    }
    $ipcRows = $rows | Where-Object { $_.provider -eq "ipc" }
    $rows | Export-Csv -LiteralPath (Join-Path $resultDir "popup-latency-raw.csv") -NoTypeInformation
    if ($ipcRows.Count -gt 0) {
        $sorted = ($ipcRows.latency_us | Sort-Object)
        $p50 = $sorted[[int][Math]::Floor($sorted.Count * 0.50)]
        $p95 = $sorted[[Math]::Min($sorted.Count - 1, [int][Math]::Floor($sorted.Count * 0.95))]
        Write-Output ("POPUP_IPC_FRAMES={0}" -f $ipcRows.Count)
        Write-Output ("POPUP_FIRST_FRAME_P50_US={0}" -f $p50)
        Write-Output ("POPUP_FIRST_FRAME_P95_US={0}" -f $p95)
    } else {
        Write-Output "POPUP_LATENCY_UNVERIFIED=no ipc rows"
    }
} else {
    Write-Output "POPUP_LATENCY_UNVERIFIED=perf log missing"
}

# --- teardown -------------------------------------------------------------
Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
    Where-Object { $_.StartTime -ge $startedAt } |
    Stop-Process -Force -ErrorAction SilentlyContinue
Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
Write-Output "QA_ROOT=$Root"
Write-Output "QA_RESULT=completed"
