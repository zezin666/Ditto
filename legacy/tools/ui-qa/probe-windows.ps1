param([Parameter(Mandatory = $true)][string]$RunDir)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class ProbeWin
{
	public delegate bool EP(IntPtr h, IntPtr l);
	[DllImport("user32.dll")] public static extern bool EnumWindows(EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	public static string Dump(int pid)
	{
		var sb = new StringBuilder();
		EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) { var t = new StringBuilder(256); GetWindowText(h, t, 256); var c = new StringBuilder(256); GetClassName(h, c, 256); sb.AppendLine(h + " class=[" + c + "] title=[" + t + "] vis=" + IsWindowVisible(h)); } return true; }, IntPtr.Zero);
		return sb.ToString();
	}
}
"@
$exe = Join-Path $RunDir 'Ditto.exe'
$process = Start-Process -FilePath $exe -WorkingDirectory $RunDir -PassThru
Start-Sleep -Seconds 3
Write-Output ([ProbeWin]::Dump($process.Id))
& $exe -exit | Out-Null
Start-Sleep -Seconds 1
if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
