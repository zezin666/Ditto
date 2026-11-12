param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $root "target\$Profile"
$sidecars = Join-Path $root "apps\ditto\src-tauri\binaries"
$triple = "x86_64-pc-windows-msvc"
New-Item -ItemType Directory -Force -Path $sidecars | Out-Null

foreach ($name in @("DittoPopup", "DittoUacHelper")) {
    $source = Join-Path $target "$name.exe"
    if (-not (Test-Path -LiteralPath $source)) {
        throw "Missing sidecar build output: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $sidecars "$name-$triple.exe") -Force
}

Write-Output "Prepared sidecars in $sidecars for $triple"

