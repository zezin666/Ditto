param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")
$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-grouped-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value

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
function Run-Case([bool]$showGrouped) {
    $caseRoot = Join-Path $Root ("case-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
    $dataRoot = Join-Path $caseRoot "data"
    New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
    $suffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
    $env:DITTO_PIPE_SUFFIX = $suffix
    $env:DITTO_DATA_DIR = $dataRoot
    $trace = Join-Path $caseRoot "popup-trace.log"
    $env:DITTO_POPUP_TRACE = $trace
    $pipeName = "ditto-$sid-$suffix"
    $settings = if ($showGrouped) {
        '{"groups":{"show_grouped_in_main":true}}'
    } else {
        '{"groups":{"show_grouped_in_main":false}}'
    }
    & cargo run --quiet --manifest-path (Join-Path $repoRoot "Cargo.toml") -p ditto-storage --example seed_qa -- $dataRoot 3 $settings
    if ($LASTEXITCODE -ne 0) { throw "seed failed" }
    $core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    $pipe = $null
    try {
        for ($i = 0; $i -lt 600; $i++) {
            Start-Sleep -Milliseconds 50
            if ((Test-Path -LiteralPath $trace) -and
                (Select-String -Path $trace -Pattern "using-prewarmed-ipc|connected" -Quiet)) { break }
            if ($core.HasExited) { throw "core exited" }
        }
        $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
        $pipe.Connect(5000)
        $id = 1
        Send-Frame $pipe 1 $id '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
        $null = Receive-Response $pipe $id
        $id++
        Send-Frame $pipe 1 $id '{"type":"GetSettings"}'
        $settingsResponse = Receive-Response $pipe $id
        $storedGrouped = $settingsResponse.values.groups.show_grouped_in_main
        Write-Output ("PIPE_SETTINGS_SHOW_GROUPED={0}" -f $storedGrouped)
        $id++
        Send-Frame $pipe 1 $id '{"type":"ShowPopup"}'
        $null = Receive-Response $pipe $id
        Start-Sleep -Milliseconds 1200
        $loads = Select-String -Path $trace -Pattern "load group=" | ForEach-Object { $_.Line }
        Write-Output ("GROUPED_MAIN_{0}_LOADS={1}" -f $showGrouped, ($loads -join " | "))
        if ($showGrouped) {
            $ok = $loads -match "group=All" -and -not ($loads -match "group=Ungrouped")
            Write-Output ("SHOW_GROUPED_TRUE_USES_ALL={0}" -f $ok)
        } else {
            $ok = $loads -match "group=Ungrouped"
            Write-Output ("SHOW_GROUPED_FALSE_USES_UNGROUPED={0}" -f $ok)
        }
        if (-not $ok) { throw "grouped-main case $showGrouped failed" }
    } finally {
        if ($null -ne $pipe) { $pipe.Dispose() }
        Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
            Where-Object { $_.StartTime -ge $core.StartTime } |
            Stop-Process -Force -ErrorAction SilentlyContinue
        if (-not $core.HasExited) { Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Milliseconds 400
    }
}

Run-Case $true
Run-Case $false
Write-Output "GROUPED_MAIN_QA=PASS"
Write-Output "QA_ROOT=$Root"
