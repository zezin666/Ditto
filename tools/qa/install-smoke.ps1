param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$PortableZip,
    [string]$PipeSuffix = ""
)

$ErrorActionPreference = "Stop"

function Wait-ForPipe([string]$name, [int]$timeoutMs = 15000) {
    for ($i = 0; $i -lt ($timeoutMs / 100); $i++) {
        $names = [System.IO.Directory]::GetFiles("//./pipe/") |
            ForEach-Object { [System.IO.Path]::GetFileName($_) }
        if ($names -contains $name) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
if ([string]::IsNullOrWhiteSpace($PipeSuffix)) {
    $PipeSuffix = [Guid]::NewGuid().ToString("N").Substring(0, 10)
}
$env:DITTO_PIPE_SUFFIX = $PipeSuffix
$pipeName = "ditto-$sid-$PipeSuffix"
$base = Join-Path $env:TEMP ("ditto-package-qa-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
$installDir = Join-Path $base "installed"
$portableDir = Join-Path $base "portable"
New-Item -ItemType Directory -Force -Path $base, $installDir, $portableDir | Out-Null

$results = [ordered]@{}

# --- NSIS silent install ---------------------------------------------------
$setupPath = (Resolve-Path -LiteralPath $Setup).Path
$install = Start-Process -FilePath $setupPath -ArgumentList "/S", "/D=$installDir" -Wait -PassThru
$results.installer_exit = $install.ExitCode
Start-Sleep -Seconds 2

foreach ($name in @("Ditto.exe", "DittoPopup.exe", "DittoUacHelper.exe", "uninstall.exe")) {
    $present = Test-Path -LiteralPath (Join-Path $installDir $name)
    $results["install_$name"] = $present
    if (-not $present) { throw "installed file missing: $name" }
}

$env:DITTO_DATA_DIR = Join-Path $base "installed-data"
$core = Start-Process -FilePath (Join-Path $installDir "Ditto.exe") -ArgumentList "--background" -PassThru
$results.install_pipe = Wait-ForPipe $pipeName
$results.install_running = -not $core.HasExited
Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -like "$installDir*" } |
    Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

$uninstall = Start-Process -FilePath (Join-Path $installDir "uninstall.exe") -ArgumentList "/S" -Wait -PassThru
$results.uninstaller_exit = $uninstall.ExitCode
Start-Sleep -Seconds 2

# --- Portable ZIP ----------------------------------------------------------
$zipPath = (Resolve-Path -LiteralPath $PortableZip).Path
Expand-Archive -LiteralPath $zipPath -DestinationPath $portableDir -Force
foreach ($name in @("Ditto.exe", "DittoPopup.exe", "DittoUacHelper.exe", "portable", "README.txt", "BUILD-INFO.json", "locales\en-US.json", "locales\zh-CN.json")) {
    $present = Test-Path -LiteralPath (Join-Path $portableDir $name)
    $results["portable_$name"] = $present
    if (-not $present) { throw "portable entry missing: $name" }
}

Remove-Item Env:DITTO_DATA_DIR -ErrorAction SilentlyContinue
$portableCore = Start-Process -FilePath (Join-Path $portableDir "Ditto.exe") -ArgumentList "--background" -PassThru
$results.portable_pipe = Wait-ForPipe $pipeName
$portableData = Join-Path $portableDir "Data"
$results.portable_marker_data = Test-Path -LiteralPath (Join-Path $portableData "ditto.db")
Stop-Process -Id $portableCore.Id -Force -ErrorAction SilentlyContinue
Get-Process -Name DittoPopup -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -like "$portableDir*" } |
    Stop-Process -Force -ErrorAction SilentlyContinue

$results | ConvertTo-Json | Tee-Object -FilePath (Join-Path $base "install-smoke.json")
Write-Output "INSTALL_QA_ROOT=$base"
