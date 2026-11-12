param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DittoHotkeyQa {
    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool RegisterHotKey(IntPtr hwnd, int id, uint modifiers, uint vk);
    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool UnregisterHotKey(IntPtr hwnd, int id);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    public static bool CanRegister(uint modifiers, uint vk) {
        bool ok = RegisterHotKey(IntPtr.Zero, 0x7A11, modifiers, vk);
        if (ok) { UnregisterHotKey(IntPtr.Zero, 0x7A11); }
        return ok;
    }
    public static void PressCtrlAlt(byte vk) {
        keybd_event(0x11, 0, 0, UIntPtr.Zero);
        keybd_event(0x12, 0, 0, UIntPtr.Zero);
        keybd_event(vk, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(40);
        keybd_event(vk, 0, 0x0002, UIntPtr.Zero);
        keybd_event(0x12, 0, 0x0002, UIntPtr.Zero);
        keybd_event(0x11, 0, 0x0002, UIntPtr.Zero);
    }
}
"@

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-hotkey-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$dataRoot = Join-Path $Root "data"
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null

$suffix = [Guid]::NewGuid().ToString("N").Substring(0, 10)
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
function Call($stream, [string]$type, [string]$fields = "") {
    $script:requestId++
    $json = if ($fields) { "{`"type`":`"$type`",$fields}" } else { "{`"type`":`"$type`"}" }
    Send-Frame $stream 1 $script:requestId $json
    return Receive-Response $stream $script:requestId
}
function Wait-Pipe([int]$timeoutMs = 20000) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        $names = [System.IO.Directory]::GetFiles("//./pipe/") | ForEach-Object { [System.IO.Path]::GetFileName($_) }
        if ($names -contains $pipeName) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}
function Start-Core {
    $script:core = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    if (-not (Wait-Pipe)) { throw "core did not create pipe $pipeName" }
    $script:pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
    $script:pipe.Connect(5000)
    $script:requestId = 1
    Send-Frame $script:pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
    $null = Receive-Response $script:pipe $script:requestId
}
function Stop-Core {
    if ($null -ne $script:pipe) { $script:pipe.Dispose(); $script:pipe = $null }
    if ($null -ne $script:core -and -not $script:core.HasExited) {
        Stop-Process -Id $script:core.Id -Force -ErrorAction SilentlyContinue
        $script:core.WaitForExit(5000) | Out-Null
    }
    Start-Sleep -Milliseconds 500
}
function Seed([string]$settingsJson) {
    $cargoArgs = @("run", "--quiet", "--manifest-path", (Join-Path $repoRoot "Cargo.toml"), "-p", "ditto-storage", "--example", "seed_qa", "--", $dataRoot, "3")
    if ($settingsJson) { $cargoArgs += $settingsJson }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "failed to seed QA settings" }
}

$core = $null
$pipe = $null
$heldConflict = $false
$ok = $true
try {
    # --- configured startup registration ---------------------------------
    Seed '{"hotkeys":{"open_chord":"Ctrl+Alt+K","plain_text_paste_chord":"","copy_buffers":[{"copy":"Ctrl+Alt+1","paste":""}]}}'
    Start-Core
    $newOpenTaken = -not [DittoHotkeyQa]::CanRegister(0x0003, 0x4B)
    $copyTaken = -not [DittoHotkeyQa]::CanRegister(0x0003, 0x31)
    $oldDefaultFree = [DittoHotkeyQa]::CanRegister(0x0002, 0xC0)
    Write-Output ("CONFIGURED_OPEN_REGISTERED={0}" -f $newOpenTaken)
    Write-Output ("CONFIGURED_COPY_BUFFER_REGISTERED={0}" -f $copyTaken)
    Write-Output ("OLD_DEFAULT_FREE_WHILE_CONFIGURED={0}" -f $oldDefaultFree)
    if (-not ($newOpenTaken -and $copyTaken -and $oldDefaultFree)) { $ok = $false }

    # Copy-buffer copy global action: place text, fire Ctrl+Alt+1, read slot 1.
    Set-Clipboard -Value "DITTO-QA-COPY-BUFFER"
    [DittoHotkeyQa]::PressCtrlAlt(0x31)
    Start-Sleep -Milliseconds 1200
    $buffers = Call $pipe "GetCopyBuffers"
    $slot1 = $buffers.slots | Where-Object { $_.slot -eq 1 } | Select-Object -First 1
    $slotFilled = ($null -ne $slot1 -and $null -ne $slot1.clip_id)
    Write-Output ("COPY_BUFFER_GLOBAL_ACTION_FILLED_SLOT={0}" -f $slotFilled)
    if ($slotFilled) {
        $clip = Call $pipe "GetClip" "`"clip_id`":$($slot1.clip_id)"
        $matches = $clip.clip.summary.preview_text -like "*DITTO-QA-COPY-BUFFER*"
        Write-Output ("COPY_BUFFER_CONTENT_MATCHES_CLIPBOARD={0}" -f $matches)
        if (-not $matches) { $ok = $false }
    } else { $ok = $false }
    Stop-Core

    # --- persisted chord change takes effect at the next start -----------
    Seed '{"hotkeys":{"open_chord":"Ctrl+Alt+D","plain_text_paste_chord":"","copy_buffers":[]}}'
    Start-Core
    $changedTaken = -not [DittoHotkeyQa]::CanRegister(0x0003, 0x44)
    $oldKFree = [DittoHotkeyQa]::CanRegister(0x0003, 0x4B)
    Write-Output ("CHANGED_OPEN_REGISTERED={0}" -f $changedTaken)
    Write-Output ("PREVIOUS_OPEN_UNREGISTERED={0}" -f $oldKFree)
    Write-Output ("COPY_BUFFER_UNREGISTERED_WHEN_CLEARED={0}" -f ([DittoHotkeyQa]::CanRegister(0x0003, 0x31)))
    if (-not ($changedTaken -and $oldKFree)) { $ok = $false }
    Stop-Core

    # --- startup conflict: another app owns the configured chord ---------
    $heldConflict = [DittoHotkeyQa]::RegisterHotKey([IntPtr]::Zero, 0x7A22, 0x0003, 0x4A)
    if (-not $heldConflict) { throw "QA could not hold Ctrl+Alt+J for the conflict test" }
    Seed '{"hotkeys":{"open_chord":"Ctrl+Alt+J","plain_text_paste_chord":"","copy_buffers":[]}}'
    Start-Core
    $settings = Call $pipe "GetSettings"
    $effectiveOpen = $settings.values.hotkeys.open_chord
    Write-Output ("CONFLICT_EFFECTIVE_OPEN_CHORD={0}" -f $effectiveOpen)
    $fellBack = ($effectiveOpen -ne "Ctrl+Alt+J") -and ($effectiveOpen -ne "")
    Write-Output ("CONFLICT_FELL_BACK_WITHOUT_SILENT_HOOK={0}" -f $fellBack)
    if (-not $fellBack) { $ok = $false }

    Write-Output ("HOTKEY_REBIND_QA={0}" -f $(if ($ok) { "PASS" } else { "FAIL" }))
    if (-not $ok) { throw "hotkey QA assertions failed" }
} finally {
    if ($heldConflict) { $null = [DittoHotkeyQa]::UnregisterHotKey([IntPtr]::Zero, 0x7A22) }
    Stop-Core
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "$repoRoot\target\*" } |
        Where-Object { $_.StartTime -ge (Get-Date).AddMinutes(-5) } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Write-Output "QA_ROOT=$Root"
}
