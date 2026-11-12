param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [string]$Output = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $root "target\$Profile"
$version = "0.1.0"
$stageRoot = Join-Path $root "target\portable"
$name = "Ditto-$version-x64-portable"
$stage = Join-Path $stageRoot $name

if (-not (Test-Path -LiteralPath $target)) {
    throw "Build output is missing: $target. Run the release build first."
}

foreach ($binary in @("Ditto.exe", "DittoPopup.exe", "DittoUacHelper.exe")) {
    $path = Join-Path $target $binary
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing portable binary: $path"
    }
}

if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null

foreach ($binary in @("Ditto.exe", "DittoPopup.exe", "DittoUacHelper.exe")) {
    Copy-Item -LiteralPath (Join-Path $target $binary) -Destination (Join-Path $stage $binary)
}

$localeSource = Join-Path $root "apps\ditto\ui\locales"
$localeTarget = Join-Path $stage "locales"
New-Item -ItemType Directory -Force -Path $localeTarget | Out-Null
foreach ($locale in @("en-US.json", "zh-CN.json")) {
    Copy-Item -LiteralPath (Join-Path $localeSource $locale) -Destination (Join-Path $localeTarget $locale)
}

Copy-Item -LiteralPath (Join-Path $root "apps\ditto\src-tauri\icons\Ditto2.ico") -Destination (Join-Path $stage "Ditto2.ico")

# The portable marker makes DataRoot::resolve use <exe_dir>\Data.
New-Item -ItemType File -Force -Path (Join-Path $stage "portable") | Out-Null

$gitSha = "unknown"
try {
    $gitSha = (git -C $root rev-parse HEAD 2>$null).Trim()
    if ([string]::IsNullOrWhiteSpace($gitSha)) { $gitSha = "unknown" }
} catch {
    $gitSha = "unknown"
}

$buildInfo = [ordered]@{
    product         = "Ditto"
    version         = $version
    git_sha         = $gitSha
    profile         = $Profile
    target          = "x86_64-pc-windows-msvc"
    built_at_utc    = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    data_root       = "<exe_dir>\Data"
    locales         = @("en-US.json", "zh-CN.json")
    signature       = "unsigned"
    notes           = "Portable build. No installer, no registry entries, no user data included."
}
$buildInfo | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $stage "BUILD-INFO.json") -Encoding UTF8

$readme = @"
Ditto $version portable (x86_64)

1. Extract this folder to a writable location.
2. Run Ditto.exe. The portable marker file keeps data in <exe_dir>\Data.
3. Press Ctrl+` for the popup, or use the tray icon. The chord is configurable
   in Settings; Ctrl+Alt+D / Ctrl+Alt+Shift+D are development fallbacks.
4. DittoPopup.exe and DittoUacHelper.exe must stay beside Ditto.exe.
5. Settings open from the tray menu. The settings window is created on demand
   and requires the Microsoft Edge WebView2 Evergreen Runtime:
   https://go.microsoft.com/fwlink/p/?LinkId=2124703
   If it is missing, Ditto shows a native message and keeps running headless.
6. Binaries are unsigned, so Windows SmartScreen or Defender may warn.

Privacy: this archive contains no clipboard history, no settings,
and no user data. Everything is created under <exe_dir>\Data on first run.
"@
Set-Content -LiteralPath (Join-Path $stage "README.txt") -Value $readme -Encoding UTF8

if ([string]::IsNullOrWhiteSpace($Output)) {
    $bundleDir = Join-Path $target "bundle\portable"
    New-Item -ItemType Directory -Force -Path $bundleDir | Out-Null
    $Output = Join-Path $bundleDir "$name.zip"
}
if (Test-Path -LiteralPath $Output) {
    Remove-Item -LiteralPath $Output -Force
}
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $Output -CompressionLevel Optimal

$hash = (Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash
$size = (Get-Item -LiteralPath $Output).Length
Write-Output "PORTABLE_ZIP=$Output"
Write-Output "PORTABLE_BYTES=$size"
Write-Output "PORTABLE_SHA256=$hash"
