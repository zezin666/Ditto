param(
  [string]$ExePath = (Join-Path $PSScriptRoot "..\..\..\..\target\release\Ditto.exe"),
  [string]$OutputPath = (Join-Path $PSScriptRoot "..\..\..\..\docs\rewrite\screenshots\T5\webview-lifecycle.json"),
  [string]$ScreenshotPath = (Join-Path $PSScriptRoot "..\..\..\..\docs\rewrite\screenshots\T5\tauri-settings-window-100.png")
)

$ErrorActionPreference = "Stop"
$resolvedExe = (Resolve-Path -LiteralPath $ExePath).Path
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$resolvedScreenshot = [System.IO.Path]::GetFullPath($ScreenshotPath)

Add-Type -AssemblyName System.Drawing.Common
Add-Type -AssemblyName System.Private.Windows.GdiPlus
Add-Type -AssemblyName System.Private.Windows.Core
$drawingReferences = @(
  [System.Drawing.Bitmap].Assembly.Location,
  ([System.Reflection.Assembly]::Load("System.Private.Windows.GdiPlus")).Location,
  ([System.Reflection.Assembly]::Load("System.Private.Windows.Core")).Location
)
Add-Type -ReferencedAssemblies $drawingReferences @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class DittoWindowCapture {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    private static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint flags);

    [DllImport("user32.dll")]
    private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr dpiContext);

    public static void Save(IntPtr handle, string path) {
        SetThreadDpiAwarenessContext(new IntPtr(-4));
        RECT rect;
        if (!GetWindowRect(handle, out rect)) {
            throw new InvalidOperationException("GetWindowRect failed");
        }
        int width = rect.Right - rect.Left;
        int height = rect.Bottom - rect.Top;
        if (width <= 0 || height <= 0) {
            throw new InvalidOperationException("Settings window has no capture area");
        }
        using (var bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb))
        using (var graphics = Graphics.FromImage(bitmap)) {
            IntPtr hdc = graphics.GetHdc();
            try {
                if (!PrintWindow(handle, hdc, 2)) {
                    throw new InvalidOperationException("PrintWindow failed");
                }
            } finally {
                graphics.ReleaseHdc(hdc);
            }
            bitmap.Save(path, ImageFormat.Png);
        }
    }
}
"@

function Get-ProcessSnapshot([int]$RootPid) {
  $all = Get-CimInstance Win32_Process |
    Select-Object ProcessId, ParentProcessId, Name, ExecutablePath
  $descendants = [System.Collections.Generic.HashSet[int]]::new()
  $queue = [System.Collections.Generic.Queue[int]]::new()
  $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) {
    $parent = $queue.Dequeue()
    foreach ($child in $all | Where-Object ParentProcessId -eq $parent) {
      if ($descendants.Add([int]$child.ProcessId)) {
        $queue.Enqueue([int]$child.ProcessId)
      }
    }
  }
  $rows = foreach ($pidValue in $descendants) {
    $row = $all | Where-Object ProcessId -eq $pidValue | Select-Object -First 1
    if ($row) {
      $process = Get-Process -Id $pidValue -ErrorAction SilentlyContinue
      [pscustomobject]@{
        pid = [int]$row.ProcessId
        parentPid = [int]$row.ParentProcessId
        name = $row.Name
        executablePath = $row.ExecutablePath
        workingSetBytes = if ($process) { $process.WorkingSet64 } else { $null }
        privateBytes = if ($process) { $process.PrivateMemorySize64 } else { $null }
      }
    }
  }
  @($rows)
}

$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ditto-t5-lifecycle-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $testRoot | Out-Null
$previousDataDir = $env:DITTO_DATA_DIR
$core = $null

try {
  $env:DITTO_DATA_DIR = $testRoot
  $core = Start-Process -FilePath $resolvedExe -ArgumentList "--background" -PassThru -WindowStyle Hidden
  $startDeadline = [DateTime]::UtcNow.AddSeconds(12)
  while (-not $core.HasExited -and [DateTime]::UtcNow -lt $startDeadline) {
    Start-Sleep -Milliseconds 250
    $core.Refresh()
    if ($core.MainWindowHandle -ne 0) {
      break
    }
  }
  if ($core.HasExited) {
    throw "Ditto core exited during startup with code $($core.ExitCode)"
  }

  Start-Sleep -Seconds 2
  $before = Get-ProcessSnapshot $core.Id
  $beforeWebview = @($before | Where-Object name -eq "msedgewebview2.exe")
  $coreBefore = Get-Process -Id $core.Id

  $secondTimer = [System.Diagnostics.Stopwatch]::StartNew()
  $second = Start-Process -FilePath $resolvedExe -ArgumentList "--settings" -PassThru -WindowStyle Hidden
  $settingsTitle = ""
  $settingsWindowObservedMs = $null
  $windowDeadline = [DateTime]::UtcNow.AddSeconds(10)
  while ([DateTime]::UtcNow -lt $windowDeadline) {
    $current = Get-Process -Id $core.Id -ErrorAction Stop
    if ($current.MainWindowTitle -eq "Ditto Settings") {
      $settingsTitle = $current.MainWindowTitle
      $settingsWindowObservedMs = [math]::Round($secondTimer.Elapsed.TotalMilliseconds)
      break
    }
    Start-Sleep -Milliseconds 250
  }
  if (-not $settingsTitle) {
    throw "Settings window title was not observed"
  }

  while (-not $second.HasExited -and $secondTimer.Elapsed.TotalSeconds -lt 20) {
    Start-Sleep -Milliseconds 100
    $second.Refresh()
  }
  $secondTimer.Stop()
  $secondExited = $second.HasExited
  if (-not $secondExited) {
    Stop-Process -Id $second.Id -Force -ErrorAction SilentlyContinue
    throw "Second instance did not exit within 20 seconds"
  }

  Start-Sleep -Seconds 2
  $windowHandle = (Get-Process -Id $core.Id).MainWindowHandle
  [DittoWindowCapture]::Save([IntPtr]$windowHandle, $resolvedScreenshot)
  $open = Get-ProcessSnapshot $core.Id
  $openWebview = @($open | Where-Object name -eq "msedgewebview2.exe")

  $windowProcess = Get-Process -Id $core.Id
  $closeSent = $windowProcess.CloseMainWindow()
  if (-not $closeSent) {
    throw "Could not send close to the settings window"
  }

  Start-Sleep -Seconds 7
  $after = Get-ProcessSnapshot $core.Id
  $afterWebview = @($after | Where-Object name -eq "msedgewebview2.exe")
  $coreAfter = Get-Process -Id $core.Id
  $coreAliveAfterClose = -not $coreAfter.HasExited
  $remainingOpenWebview = @(
    $afterWebview | Where-Object { $_.pid -in $openWebview.pid }
  )

  $result = [pscustomobject]@{
    status = if ($coreAliveAfterClose -and $remainingOpenWebview.Count -eq 0) { "PASS" } else { "FAIL" }
    generatedAt = [DateTime]::UtcNow.ToString("o")
    executable = $resolvedExe
    syntheticDataDir = $testRoot
    corePid = $core.Id
    secondInstanceExitCode = $second.ExitCode
    secondInstanceExitDurationMs = [math]::Round($secondTimer.Elapsed.TotalMilliseconds)
    secondInstanceUnder150Ms = $secondTimer.Elapsed.TotalMilliseconds -lt 150
    settingsWindowObservedMs = $settingsWindowObservedMs
    settingsWindowTitle = $settingsTitle
    settingsScreenshot = $resolvedScreenshot
    coreAliveAfterClose = $coreAliveAfterClose
    webviewBeforeCount = $beforeWebview.Count
    webviewOpenCount = $openWebview.Count
    webviewAfterCloseCount = $afterWebview.Count
    newWebviewPidsWhileOpen = @($openWebview.pid | Where-Object { $_ -notin $beforeWebview.pid })
    remainingOpenWebviewAfterCloseCount = $remainingOpenWebview.Count
    remainingOpenWebviewAfterClosePids = ($remainingOpenWebview.pid -join ",")
    corePrivateBytesBefore = $coreBefore.PrivateMemorySize64
    corePrivateBytesAfterClose = $coreAfter.PrivateMemorySize64
    corePrivateDeltaBytes = $coreAfter.PrivateMemorySize64 - $coreBefore.PrivateMemorySize64
    before = $before
    open = $open
    afterClose = $after
  }

  $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $resolvedOutput -Encoding utf8
  $result | ConvertTo-Json -Depth 5
  if ($result.status -ne "PASS" -or $result.secondInstanceExitCode -ne 0) {
    throw "Settings lifecycle check failed"
  }
}
finally {
  if ($core -and -not $core.HasExited) {
    $coreProcesses = Get-ProcessSnapshot $core.Id
    foreach ($row in $coreProcesses | Where-Object name -in @("DittoPopup.exe", "DittoUacHelper.exe")) {
      Stop-Process -Id $row.pid -Force -ErrorAction SilentlyContinue
    }
    Stop-Process -Id $core.Id -Force -ErrorAction SilentlyContinue
  }
  $env:DITTO_DATA_DIR = $previousDataDir
  if ($testRoot -like "*ditto-t5-lifecycle-*") {
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
  }
}
