param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"
$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-theme-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
$dataRoot = Join-Path $Root "data"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
$suffix = [Guid]::NewGuid().ToString("N").Substring(0, 10)
$env:DITTO_PIPE_SUFFIX = $suffix
$env:DITTO_DATA_DIR = $dataRoot
$trace = Join-Path $Root "popup-trace.log"
$env:DITTO_POPUP_TRACE = $trace
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
function Call($stream, [string]$type, [string]$fields = "") {
    $script:requestId++
    $json = if ($fields) { "{`"type`":`"$type`",$fields}" } else { "{`"type`":`"$type`"}" }
    Send-Frame $stream 1 $script:requestId $json
    return Receive-Response $stream $script:requestId
}
function Wait-Theme([string]$name, [int]$timeoutMs = 5000) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        if (Select-String -Path $trace -Pattern "theme-apply $name " -Quiet) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

& cargo run --quiet --manifest-path (Join-Path $repoRoot "Cargo.toml") -p ditto-storage --example seed_qa -- $dataRoot 3 '{"appearance":{"theme":"dark"}}'
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
    $script:requestId = 1
    Send-Frame $pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
    $null = Receive-Response $pipe $script:requestId
    $null = Call $pipe "ShowPopup"

    $dark = Wait-Theme "Dark"
    Write-Output ("THEME_DARK_APPLIED={0}" -f $dark)
    if (-not $dark) { throw "dark theme was not applied" }

    $setHc = Call $pipe "SetSettings" '"values":{"appearance":{"theme":"highContrast"}}'
    if ($setHc.type -ne "Settings") { throw "SetSettings highContrast failed" }
    $hc = Wait-Theme "HighContrast"
    $hcPalette = (Select-String -Path $trace -Pattern "theme-apply HighContrast palette_high_contrast=true" -Quiet)
    Write-Output ("THEME_HIGH_CONTRAST_APPLIED={0}" -f $hc)
    Write-Output ("HIGH_CONTRAST_USES_SYSTEM_PALETTE={0}" -f $hcPalette)
    if (-not ($hc -and $hcPalette)) { throw "high contrast did not apply system colors" }

    $setSystem = Call $pipe "SetSettings" '"values":{"appearance":{"theme":"system"}}'
    if ($setSystem.type -ne "Settings") { throw "SetSettings system failed" }
    Start-Sleep -Milliseconds 1200
    $lines = Select-String -Path $trace -Pattern "theme-apply " | ForEach-Object { $_.Line }
    Write-Output ("THEME_TRACE_TAIL=" + (($lines | Select-Object -Last 3) -join " | "))
    $lightKey = Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize" -Name AppsUseLightTheme -ErrorAction SilentlyContinue
    $osLight = ($null -ne $lightKey -and $lightKey.AppsUseLightTheme -eq 1)
    Write-Output ("OS_APPS_USE_LIGHT_THEME={0}" -f $osLight)
    $lastTheme = ($lines | Select-Object -Last 1)
    $systemOk = $true
    if ($osLight) {
        $systemOk = $lastTheme -match "theme-apply Light "
        Write-Output ("SYSTEM_FOLLOWS_LIGHT_OS={0}" -f $systemOk)
    } else {
        $systemOk = $lastTheme -match "theme-apply Dark "
        Write-Output ("SYSTEM_FOLLOWS_DARK_OS={0}" -f $systemOk)
    }
    $stored = Call $pipe "GetSettings"
    Write-Output ("STORED_THEME={0}" -f $stored.values.appearance.theme)
    if (-not $systemOk) { throw "system theme did not re-resolve from the OS" }
    if ($stored.values.appearance.theme -ne "system") { throw "system theme was not persisted" }
    Write-Output "THEME_MODE_QA=PASS"
} finally {
    if ($null -ne $pipe) { $pipe.Dispose() }
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $core.StartTime } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    if (-not $core.HasExited) { Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue }
    Write-Output "QA_ROOT=$Root"
}
