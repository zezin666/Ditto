param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [int]$Samples = 3,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$binaryDir = Split-Path -Parent $DittoExe
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-cold-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
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

function Measure-ColdStart([string]$exe, [string]$label, [int]$index) {
    $runRoot = Join-Path $Root ("$label-$index-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
    $dataRoot = Join-Path $runRoot "data"
    New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
    $suffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
    $env:DITTO_PIPE_SUFFIX = $suffix
    $env:DITTO_DATA_DIR = $dataRoot
    $hostTrace = Join-Path $runRoot "host-trace.log"
    $popupTrace = Join-Path $runRoot "popup-trace.log"
    $env:DITTO_HOST_TRACE = $hostTrace
    $env:DITTO_POPUP_TRACE = $popupTrace
    $pipeName = "ditto-$sid-$suffix"
    $start = Get-Date
    $core = Start-Process -FilePath $exe -ArgumentList "--background" -PassThru
    $pipe = $null
    try {
        $ready = $false
        for ($i = 0; $i -lt 600; $i++) {
            Start-Sleep -Milliseconds 50
            if ((Test-Path -LiteralPath $popupTrace) -and
                (Select-String -Path $popupTrace -Pattern "using-prewarmed-ipc|connected" -Quiet)) {
                $ready = $true
                break
            }
            if ($core.HasExited) { throw "core exited during startup" }
        }
        if (-not $ready) { throw "popup IPC reader did not start" }
        $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
        $pipe.Connect(5000)
        $requestId = 1
        Send-Frame $pipe 1 $requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
        $null = Receive-Response $pipe $requestId
        $requestId++
        Send-Frame $pipe 1 $requestId '{"type":"ShowPopup"}'
        $null = Receive-Response $pipe $requestId
        $deadline = (Get-Date).AddSeconds(10)
        while ((Get-Date) -lt $deadline) {
            if (Select-String -Path $popupTrace -Pattern "activate-foreground=" -Quiet) { break }
            Start-Sleep -Milliseconds 25
        }
        $elapsed = ((Get-Date) - $start).TotalMilliseconds
        $hostRun = Select-String -Path $hostTrace -Pattern "run-enter" | Select-Object -First 1
        $popupStart = Select-String -Path $popupTrace -Pattern "popup:process-start" | Select-Object -First 1
        $activate = Select-String -Path $popupTrace -Pattern "activate-foreground=" | Select-Object -First 1
        $popupMs = $null
        $popupToShowMs = $null
        if ($popupStart -and $activate) {
            $popupMs = [double]$activate.Line.Split(" ")[0] - [double]$popupStart.Line.Split(" ")[0]
        }
        if ($popupStart -and (Select-String -Path $popupTrace -Pattern "using-prewarmed-ipc|connected" -Quiet)) {
            $readyLine = Select-String -Path $popupTrace -Pattern "using-prewarmed-ipc|connected" | Select-Object -First 1
            $popupToShowMs = [double]$readyLine.Line.Split(" ")[0] - [double]$popupStart.Line.Split(" ")[0]
        }
        # Breakdown: process creation/loading, eframe init, PopupApp::new, and
        # the IPC-to-activation step. This distinguishes OS/loader time from
        # the code-controlled critical path.
        $runNative = Select-String -Path $popupTrace -Pattern "run-native-enter" | Select-Object -First 1
        $appEnter = Select-String -Path $popupTrace -Pattern "app-new-enter" | Select-Object -First 1
        $appExit = Select-String -Path $popupTrace -Pattern "app-new-exit" | Select-Object -First 1
        if ($popupStart -and $runNative) {
            Write-Host ("COLD_{0}_{1}_POPUP_START_TO_RUN_NATIVE_MS={2:N1}" -f $label, $index,
                ([double]$runNative.Line.Split(" ")[0] - [double]$popupStart.Line.Split(" ")[0]))
        }
        if ($appEnter -and $appExit) {
            Write-Host ("COLD_{0}_{1}_APP_NEW_MS={2:N1}" -f $label, $index,
                ([double]$appExit.Line.Split(" ")[0] - [double]$appEnter.Line.Split(" ")[0]))
        }
        if ($hostRun -and $popupStart) {
            Write-Host ("COLD_{0}_{1}_HOST_RUN_TO_POPUP_START_MS={2:N1}" -f $label, $index,
                ([double]$popupStart.Line.Split(" ")[0] - [double]$hostRun.Line.Split(" ")[0]))
        }
        if ($readyLine -and $activate) {
            Write-Host ("COLD_{0}_{1}_POPUP_IPC_TO_ACTIVATE_MS={2:N1}" -f $label, $index,
                ([double]$activate.Line.Split(" ")[0] - [double]$readyLine.Line.Split(" ")[0]))
        }
        Write-Host ("COLD_{0}_{1}_HOST_TO_SHOW_MS={2:N1}" -f $label, $index, $elapsed)
        if ($null -ne $popupMs) {
            Write-Host ("COLD_{0}_{1}_POPUP_START_TO_SHOW_MS={2:N1}" -f $label, $index, $popupMs)
        }
        if ($null -ne $popupToShowMs) {
            Write-Host ("COLD_{0}_{1}_POPUP_START_TO_IPC_MS={2:N1}" -f $label, $index, $popupToShowMs)
        }
        return $elapsed
    } finally {
        if ($null -ne $pipe) { $pipe.Dispose() }
        Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
            Where-Object { $_.StartTime -ge $start } |
            Stop-Process -Force -ErrorAction SilentlyContinue
        if (-not $core.HasExited) { Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Milliseconds 500
    }
}

$same = @()
for ($i = 1; $i -le $Samples; $i++) { $same += Measure-ColdStart $DittoExe "same" $i }
$sorted = $same | Sort-Object
Write-Output ("SAME_BINARY_SAMPLES_MS=" + (($same | ForEach-Object { "{0:N1}" -f $_ }) -join ","))
Write-Output ("SAME_BINARY_MEDIAN_MS={0:N1}" -f $sorted[[int][Math]::Floor($sorted.Count / 2)])

# Fresh copy: new file identity, closest local proxy for a first install/upgrade.
$freshDir = Join-Path $Root "fresh-build"
New-Item -ItemType Directory -Force -Path $freshDir | Out-Null
foreach ($name in @("Ditto.exe", "DittoPopup.exe", "DittoUacHelper.exe")) {
    Copy-Item -LiteralPath (Join-Path $binaryDir $name) -Destination (Join-Path $freshDir $name) -Force
}
$localeSource = Join-Path $repoRoot "apps\ditto\ui\locales"
if (Test-Path -LiteralPath $localeSource) {
    $localeTarget = Join-Path $freshDir "locales"
    New-Item -ItemType Directory -Force -Path $localeTarget | Out-Null
    foreach ($locale in @("en-US.json", "zh-CN.json")) {
        Copy-Item -LiteralPath (Join-Path $localeSource $locale) -Destination (Join-Path $localeTarget $locale) -Force
    }
}
$fresh = Measure-ColdStart (Join-Path $freshDir "Ditto.exe") "fresh" 1
Write-Output ("FRESH_COPY_FIRST_SHOW_MS={0:N1}" -f $fresh)
Write-Output "QA_ROOT=$Root"
