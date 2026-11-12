param(
    [Parameter(Mandatory = $true)][string]$DittoExe,
    [string]$Root = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "suffix-common.ps1")

$DittoExe = (Resolve-Path -LiteralPath $DittoExe).Path
$binaryDir = Split-Path -Parent $DittoExe
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $env:TEMP ("ditto-suffix-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
}
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$productionPipeName = "ditto-$sid"

function Get-PipeNames {
    return [System.IO.Directory]::GetFiles("//./pipe/") |
        ForEach-Object { [System.IO.Path]::GetFileName($_) }
}

function Wait-ForPipe([string]$name, [int]$timeoutMs = 20000) {
    $deadline = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $deadline) {
        if ((Get-PipeNames) -contains $name) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

function Stop-RunProcesses([datetime]$startedAt) {
    Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "$binaryDir\*" -and $_.StartTime -ge $startedAt } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

$productionBefore = (Get-PipeNames) -contains $productionPipeName
$ok = $true
$invalidCases = @(
    @{ Name = "punctuation"; Value = "!!!"; Label = "SUFFIX_PUNCTUATION" },
    @{ Name = "whitespace"; Value = "   "; Label = "SUFFIX_WHITESPACE" },
    @{ Name = "too_long"; Value = ("a" * 49); Label = "SUFFIX_TOO_LONG" },
    @{ Name = "dot"; Value = "qa.1"; Label = "SUFFIX_DOT" }
)

foreach ($case in $invalidCases) {
    $runRoot = Join-Path $Root ($case.Name + "-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
    $dataRoot = Join-Path $runRoot "data"
    New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
    $trace = Join-Path $runRoot "host-trace.log"
    $env:DITTO_PIPE_SUFFIX = $case.Value
    $env:DITTO_DATA_DIR = $dataRoot
    $env:DITTO_HOST_TRACE = $trace
    $startedAt = Get-Date
    $process = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
    if (-not $process.WaitForExit(15000)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        Write-Output ("{0}_TIMED_OUT=True" -f $case.Label)
        $ok = $false
        continue
    }
    $exitCode = $process.ExitCode
    $dbCreated = Test-Path -LiteralPath (Join-Path $dataRoot "ditto.db")
    $traceHasError = (Test-Path -LiteralPath $trace) -and
        (Select-String -Path $trace -Pattern "invalid-suffix=" -Quiet)
    Write-Output ("{0}_EXIT={1}" -f $case.Label, $exitCode)
    Write-Output ("{0}_DB_CREATED={1}" -f $case.Label, $dbCreated)
    Write-Output ("{0}_TRACE_HAS_ERROR={1}" -f $case.Label, $traceHasError)
    if ($exitCode -ne 2 -or $dbCreated -or -not $traceHasError) { $ok = $false }
    Stop-RunProcesses $startedAt
}

# A valid suffix must still start normally and create only the isolated pipe.
$validRoot = Join-Path $Root ("valid-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
$validData = Join-Path $validRoot "data"
New-Item -ItemType Directory -Force -Path $validData | Out-Null
$validSuffix = Assert-DittoPipeSuffix (New-DittoPipeSuffix)
$env:DITTO_PIPE_SUFFIX = $validSuffix
$env:DITTO_DATA_DIR = $validData
$env:DITTO_HOST_TRACE = Join-Path $validRoot "host-trace.log"
$validStartedAt = Get-Date
$validProcess = Start-Process -FilePath $DittoExe -ArgumentList "--background" -PassThru
try {
    $validPipe = "ditto-$sid-$validSuffix"
    $validPipeCreated = Wait-ForPipe $validPipe
    Write-Output ("VALID_SUFFIX_PIPE_CREATED={0}" -f $validPipeCreated)
    if (-not $validPipeCreated) { $ok = $false }
} finally {
    if (-not $validProcess.HasExited) {
        Stop-Process -Id $validProcess.Id -Force -ErrorAction SilentlyContinue
        $validProcess.WaitForExit(5000) | Out-Null
    }
    Stop-RunProcesses $validStartedAt
}

$productionAfter = (Get-PipeNames) -contains $productionPipeName
Write-Output ("PRODUCTION_PIPE_BEFORE={0}" -f $productionBefore)
Write-Output ("PRODUCTION_PIPE_AFTER={0}" -f $productionAfter)
Write-Output ("PRODUCTION_PIPE_UNCHANGED={0}" -f ($productionBefore -eq $productionAfter))
if ($productionBefore -ne $productionAfter) { $ok = $false }

Write-Output ("SUFFIX_VALIDATION_QA={0}" -f $(if ($ok) { "PASS" } else { "FAIL" }))
if (-not $ok) { throw "suffix validation QA assertions failed" }
Write-Output "QA_ROOT=$Root"
