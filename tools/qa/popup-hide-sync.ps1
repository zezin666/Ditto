param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [Parameter(Mandatory = $true)][string]$DataRoot,
    [string]$PipeSuffix = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")
$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$DataRoot = (Resolve-Path -LiteralPath $DataRoot).Path
$env:DITTO_DATA_DIR = $DataRoot
if ([string]::IsNullOrWhiteSpace($PipeSuffix)) {
    $PipeSuffix = New-DittoPipeSuffix
}
$PipeSuffix = Assert-DittoPipeSuffix $PipeSuffix
$env:DITTO_PIPE_SUFFIX = $PipeSuffix
$env:DITTO_POPUP_TRACE = Join-Path $DataRoot "..\popup-hide-trace.log"
$env:DITTO_HOST_TRACE = Join-Path $DataRoot "..\host-trace.log"
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$pipeName = "ditto-$sid-$PipeSuffix"
$pipePath = "//./pipe/$pipeName"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class HideQa {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
"@

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

$core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
$pipe = $null
try {
    for ($i = 0; $i -lt 200; $i++) {
        Start-Sleep -Milliseconds 50
        if (Test-Path $pipePath) { break }
        if ($core.HasExited) { throw "core exited during startup" }
    }
    $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
    $pipe.Connect(5000)
    $script:requestId = 0
    $script:requestId++
    Send-Frame $pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":true}'
    $null = Receive-Response $pipe $script:requestId

    function Call([string]$type) {
        $script:requestId++
        Send-Frame $pipe 1 $script:requestId ('{"type":"' + $type + '"}')
        Receive-Response $pipe $script:requestId
    }

    # Wait for the resident popup to complete its IPC handshake; process
    # creation alone can take several seconds on an unsigned cold binary.
    $trace = Join-Path $DataRoot "..\popup-hide-trace.log"
    for ($i = 0; $i -lt 300; $i++) {
        Start-Sleep -Milliseconds 100
        if ((Test-Path $trace) -and (Select-String -Path $trace -Pattern "hello-ack-prewarm|using-prewarmed-ipc" -Quiet)) { break }
    }
    Start-Sleep -Seconds 1
    [HideQa]::SetCursorPos(420, 420) | Out-Null
    $show = Call "ShowPopup"
    if (-not $show.visible) { throw "ShowPopup did not become visible" }
    Start-Sleep -Milliseconds 800

    # A primary click far outside the popup must hide it locally and sync the
    # core's popup_visible flag.
    [HideQa]::SetCursorPos(20, 20) | Out-Null
    Start-Sleep -Milliseconds 100
    [HideQa]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [HideQa]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Seconds 1
    $afterClick = Call "GetPopupVisibility"
    Write-Output ("POPUP_VISIBLE_AFTER_OUTSIDE_CLICK={0}" -f $afterClick.visible)
    if ($afterClick.visible) { throw "outside click did not sync HidePopup" }

    # The next hotkey must show on the first press, not toggle a stale true.
    Start-Sleep -Milliseconds 300
    $showAgain = Call "ShowPopup"
    Write-Output ("POPUP_VISIBLE_AFTER_NEXT_SHOW={0}" -f $showAgain.visible)
    if (-not $showAgain.visible) { throw "next ShowPopup did not become visible" }

    # Simulate a popup crash while visible; the core must clear visibility when
    # its popup clients disconnect.
    Start-Sleep -Milliseconds 500
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "*rewrite-fixes\target\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    for ($i = 0; $i -lt 100; $i++) {
        Start-Sleep -Milliseconds 50
        if (-not (Call "GetPopupVisibility").visible) { break }
    }
    $afterCrash = Call "GetPopupVisibility"
    Write-Output ("POPUP_VISIBLE_AFTER_CRASH={0}" -f $afterCrash.visible)
    if ($afterCrash.visible) { throw "popup crash left stale visibility" }
    Write-Output "POPUP_HIDE_SYNC_RESULT=completed"
} finally {
    if ($null -ne $pipe) { $pipe.Dispose() }
    Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "*rewrite-fixes\target\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}
