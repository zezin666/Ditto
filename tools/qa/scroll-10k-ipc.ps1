param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = "",
    [int]$Frames = 300
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$popupExe = Join-Path (Split-Path -Parent $DittoExe) "DittoPopup.exe"
if (-not (Test-Path -LiteralPath $popupExe)) { throw "DittoPopup.exe is missing beside $DittoExe" }
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-scroll-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$dataRoot = Join-Path $Root "data"
$csv = Join-Path $Root "scroll-10k-real-ipc.csv"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null

# Unique instance suffix: isolated pipe and single-instance events.
$suffix = Assert-DittoPipeSuffix $(if ($env:DITTO_PIPE_SUFFIX) { $env:DITTO_PIPE_SUFFIX } else { New-DittoPipeSuffix })
$env:DITTO_PIPE_SUFFIX = $suffix
$env:DITTO_DATA_DIR = $dataRoot
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
$popup = $null
$pipe = $null
$startedAt = Get-Date
try {
    & cargo run --quiet --manifest-path (Join-Path $repoRoot "Cargo.toml") -p ditto-storage --example seed_qa -- $dataRoot 10000
    if ($LASTEXITCODE -ne 0) { throw "failed to seed 10k synthetic clips" }

    $core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    $ready = $false
    for ($i = 0; $i -lt 300; $i++) {
        Start-Sleep -Milliseconds 100
        $names = [System.IO.Directory]::GetFiles("//./pipe/") | ForEach-Object { [System.IO.Path]::GetFileName($_) }
        if ($names -contains $pipeName) { $ready = $true; break }
        if ($core.HasExited) { throw "core exited during startup" }
    }
    if (-not $ready) { throw "core did not create pipe $pipeName" }

    # Confirm the real database has 10k rows through the real IPC provider.
    $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
    $pipe.Connect(5000)
    $script:requestId = 1
    Send-Frame $pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
    $null = Receive-Response $pipe $script:requestId
    $script:requestId++
    Send-Frame $pipe 1 $script:requestId '{"type":"GetClips","query":"","limit":1}'
    $page = Receive-Response $pipe $script:requestId
    Write-Output ("DB_TOTAL_ROWS={0}" -f $page.total)
    if ($page.total -lt 10000) { throw "expected 10000 rows, found $($page.total)" }
    $pipe.Dispose()
    $pipe = $null

    # Explicit real provider; --bench-scroll no longer forces mock.
    $popup = Start-Process -FilePath $popupExe -ArgumentList @(
        "--bench-scroll",
        "--provider=ipc",
        "--bench-frames=$Frames",
        "--bench-warmup=10",
        "--bench-log=$csv"
    ) -PassThru
    $exited = $popup.WaitForExit(120000)
    if (-not $exited) { throw "popup benchmark did not finish" }
    if (-not (Test-Path -LiteralPath $csv)) { throw "benchmark CSV was not written" }

    $rows = Import-Csv -LiteralPath $csv
    Write-Output ("FRAMES={0}" -f $rows.Count)
    Write-Output "WARMUP_FRAMES_EXCLUDED=10"
    if ($rows.Count -lt 300) { throw "expected at least 300 frames, found $($rows.Count)" }
    $frameMs = $rows | ForEach-Object { [double]$_.frame_ms }
    $updateMs = $rows | ForEach-Object { [double]$_.update_ms }
    $sorted = $frameMs | Sort-Object
    $p50 = $sorted[[int][Math]::Floor($sorted.Count * 0.50)]
    $p95 = $sorted[[Math]::Min($sorted.Count - 1, [int][Math]::Floor($sorted.Count * 0.95))]
    $max = ($sorted | Measure-Object -Maximum).Maximum
    $over = ($frameMs | Where-Object { $_ -gt 16.7 }).Count
    $updateSorted = $updateMs | Sort-Object
    $updateP95 = $updateSorted[[Math]::Min($updateSorted.Count - 1, [int][Math]::Floor($updateSorted.Count * 0.95))]
    $maxRendered = ($rows | ForEach-Object { [int]$_.rendered_rows } | Measure-Object -Maximum).Maximum
    $maxOffset = ($rows | ForEach-Object { [double]$_.scroll_offset } | Measure-Object -Maximum).Maximum
    Write-Output ("FRAME_P50_MS={0:N3}" -f $p50)
    Write-Output ("FRAME_P95_MS={0:N3}" -f $p95)
    Write-Output ("FRAME_MAX_MS={0:N3}" -f $max)
    Write-Output ("FRAMES_OVER_16_7_MS={0}" -f $over)
    Write-Output ("UPDATE_P95_MS={0:N3}" -f $updateP95)
    Write-Output ("MAX_RENDERED_ROWS={0}" -f $maxRendered)
    Write-Output ("MAX_SCROLL_OFFSET_PX={0:N1}" -f $maxOffset)
    $p18 = if ($over -eq 0) { "PASS" } else { "FAIL" }
    Write-Output ("P18_FRAME_BUDGET={0}" -f $p18)
} finally {
    if ($null -ne $pipe) { $pipe.Dispose() }
    if ($null -ne $popup -and -not $popup.HasExited) { Stop-Process -Id $popup.Id -Force -ErrorAction SilentlyContinue }
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $startedAt } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    if ($null -ne $core -and -not $core.HasExited) {
        Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
    }
    Write-Output "QA_ROOT=$Root"
}
