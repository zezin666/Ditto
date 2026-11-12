param([Parameter(Mandatory = $true)][string]$RunDir)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class OpenProbe
{
	public delegate bool EP(IntPtr h, IntPtr l);
	[DllImport("user32.dll")] public static extern bool EnumWindows(EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	public static string Text(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
	public static string Cls(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
	public static IntPtr FindMain(int pid)
	{
		IntPtr r = IntPtr.Zero;
		EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid && Text(h) == "Ditto Clipboard Viewer") { r = h; return false; } return true; }, IntPtr.Zero);
		return r;
	}
	public static IntPtr Popup(int pid)
	{
		IntPtr r = IntPtr.Zero;
		EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid && IsWindowVisible(h) && Cls(h) == "QPasteClass") { r = h; return false; } return true; }, IntPtr.Zero);
		return r;
	}
	public static string Dump(int pid)
	{
		var sb = new StringBuilder();
		EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) sb.Append("[" + h + " " + Cls(h) + "|" + Text(h) + "|vis=" + IsWindowVisible(h) + "]"); return true; }, IntPtr.Zero);
		return sb.ToString();
	}
}
"@
$exe = Join-Path $RunDir 'Ditto.exe'
$p = Start-Process -FilePath $exe -WorkingDirectory $RunDir -PassThru
Start-Sleep -Seconds 5
Write-Output ("alive=" + (!$p.HasExited))
$main = [OpenProbe]::FindMain($p.Id)
$ini = Join-Path $RunDir 'Ditto.Settings'
$settingMain = $null
foreach ($line in Get-Content $ini) { if ($line.Trim().StartsWith('MainhWnd=')) { $settingMain = $line.Trim().Substring(9) } }
Write-Output ("main=" + $main + " settingMainhWnd=" + $settingMain + " " + [OpenProbe]::Dump($p.Id))
if ($main -ne [IntPtr]::Zero)
{
	$target = $main
	if ($settingMain -ne $null -and [long]$settingMain -gt 0) { $target = [IntPtr][long]$settingMain }
	Write-Output ("post target=" + $target)
	[OpenProbe]::PostMessage($target, 0x04DE, [IntPtr]1, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Seconds 2
	$popup = [OpenProbe]::Popup($p.Id)
	Write-Output ("after-post popup=" + $popup)
	if ($popup -eq [IntPtr]::Zero)
	{
		[OpenProbe]::SendMessage($main, 0x04DE, [IntPtr]1, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Seconds 2
		Write-Output ("after-send popup=" + [OpenProbe]::Popup($p.Id))
	}
}
& $exe -open | Out-Null
Start-Sleep -Seconds 2
Write-Output ("after-exe-open popup=" + [OpenProbe]::Popup($p.Id))
& $exe -exit | Out-Null
Start-Sleep -Seconds 2
if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }
Write-Output "done"
