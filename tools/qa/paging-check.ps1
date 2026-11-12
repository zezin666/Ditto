param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [Parameter(Mandatory = $true)][string]$DataRoot,
    [int]$PageSize = 500,
    [string]$PipeSuffix = ""
)

$ErrorActionPreference = "Stop"
$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$DataRoot = (Resolve-Path -LiteralPath $DataRoot).Path
$env:DITTO_DATA_DIR = $DataRoot
if ([string]::IsNullOrWhiteSpace($PipeSuffix)) {
    $PipeSuffix = [Guid]::NewGuid().ToString("N").Substring(0, 10)
}
$env:DITTO_PIPE_SUFFIX = $PipeSuffix
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$pipeName = "ditto-$sid-$PipeSuffix"
$pipePath = "//./pipe/$pipeName"

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
    Send-Frame $pipe 1 $script:requestId '{"type":"Hello","proto_min":1,"proto_max":1,"client":"cli","pid":1,"nonce":"qa","subscribe":false}'
    $hello = Receive-Response $pipe $script:requestId
    if ($hello.type -ne "HelloAck") { throw "Hello failed" }

    $cursor = $null
    $pages = 0
    $seen = 0
    $total = -1
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    while ($pages -lt 100) {
        $script:requestId++
        $fields = if ($cursor) {
            $escaped = $cursor.Replace('"', '\"')
            '"query":"","limit":' + $PageSize + ',"cursor":"' + $escaped + '"'
        } else {
            '"query":"","limit":' + $PageSize
        }
        Send-Frame $pipe 1 $script:requestId ('{"type":"GetClips",' + $fields + '}')
        $page = Receive-Response $pipe $script:requestId
        if ($page.type -ne "Clips") { throw ("GetClips failed: " + ($page | ConvertTo-Json -Compress)) }
        $pages++
        $seen += $page.items.Count
        $total = $page.total
        if (-not $page.next_cursor) { break }
        $cursor = $page.next_cursor
    }
    Write-Output ("PAGING_PAGES={0} SEEN={1} TOTAL={2} ELAPSED_MS={3:N1}" -f $pages, $seen, $total, $sw.Elapsed.TotalMilliseconds)
    if ($seen -ne $total) { throw "cursor paging did not reach every row" }
    Write-Output "PAGING_RESULT=completed"
} finally {
    if ($null -ne $pipe) { $pipe.Dispose() }
    Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "*rewrite-fixes\target\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}
