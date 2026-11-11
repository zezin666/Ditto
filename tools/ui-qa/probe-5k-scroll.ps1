param(
	[Parameter(Mandatory = $true)][string]$RunDir,
	[string]$Build = 'Debug64',
	[int]$Messages = 400,
	[int]$Delta = 60,
	[ValidateSet('LVM','VSCROLL','WHEEL')][string]$Method = 'LVM'
)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ScrollProbe
{
	public delegate bool EP(IntPtr h, IntPtr l);
	[DllImport("user32.dll")] public static extern bool EnumWindows(EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
	[DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
	[DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
	[DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr p, uint flags);
	public static IntPtr FindPopup(int pid)
	{
		IntPtr result = IntPtr.Zero;
		EnumWindows((h, l) => {
			uint p; GetWindowThreadProcessId(h, out p);
			if (p == pid && IsWindowVisible(h)) { var sb = new StringBuilder(128); GetClassName(h, sb, 128); if (sb.ToString() == "QPasteClass") { result = h; return false; } }
			return true;
		}, IntPtr.Zero);
		return result;
	}
	public static IntPtr ChildById(IntPtr parent, int id)
	{
		IntPtr result = IntPtr.Zero;
		EnumChildWindows(parent, (h, l) => { if (GetDlgCtrlID(h) == id) { result = h; return false; } return true; }, IntPtr.Zero);
		return result;
	}
	public static int Items(IntPtr list) { return (int)SendMessage(list, 0x1004, IntPtr.Zero, IntPtr.Zero); }
	public static int Gdi(int pid) { IntPtr h = OpenProcess(0x0400, false, pid); if (h == IntPtr.Zero) return -1; try { return (int)GetGuiResources(h, 0); } finally { CloseHandle(h); } }
	public static int User(int pid) { IntPtr h = OpenProcess(0x0400, false, pid); if (h == IntPtr.Zero) return -1; try { return (int)GetGuiResources(h, 1); } finally { CloseHandle(h); } }
}
"@
$exe = Join-Path $RunDir 'Ditto.exe'
$process = Start-Process -FilePath $exe -WorkingDirectory $RunDir -PassThru
Start-Sleep -Seconds 4
& $exe -open | Out-Null
Start-Sleep -Seconds 3
$popup = [ScrollProbe]::FindPopup($process.Id)
if ($popup -eq [IntPtr]::Zero) { Write-Output 'FAIL popup not found'; exit 1 }
$list = [ScrollProbe]::ChildById($popup, 0x201)
Write-Output ("items=" + [ScrollProbe]::Items($list) + " gdi=" + [ScrollProbe]::Gdi($process.Id))
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$survived = $true
for ($i = 0; $i -lt $Messages; $i++)
{
	if ($process.HasExited) { $survived = $false; Write-Output ("EXITED during scroll at iteration " + $i); break }
	if ($Method -eq 'LVM') { [ScrollProbe]::SendMessage($list, 0x1010, [IntPtr]::Zero, [IntPtr]$Delta) | Out-Null }
	elseif ($Method -eq 'VSCROLL') { [ScrollProbe]::SendMessage($list, 0x0115, [IntPtr]1, [IntPtr]::Zero) | Out-Null }
	else { [ScrollProbe]::SendMessage($list, 0x020A, [IntPtr](-120 -shl 16), [IntPtr]::Zero) | Out-Null }
	if (($i % 20) -eq 0) { Start-Sleep -Milliseconds 5 }
}
$sw.Stop()
if (!$process.HasExited)
{
	for ($i = 0; $i -lt $Messages; $i++)
	{
		if ($process.HasExited) { $survived = $false; Write-Output ("EXITED during up-scroll at iteration " + $i); break }
		if ($Method -eq 'LVM') { [ScrollProbe]::SendMessage($list, 0x1010, [IntPtr]::Zero, [IntPtr](-$Delta)) | Out-Null }
		elseif ($Method -eq 'VSCROLL') { [ScrollProbe]::SendMessage($list, 0x0115, [IntPtr]0, [IntPtr]::Zero) | Out-Null }
		else { [ScrollProbe]::SendMessage($list, 0x020A, [IntPtr](120 -shl 16), [IntPtr]::Zero) | Out-Null }
		if (($i % 20) -eq 0) { Start-Sleep -Milliseconds 5 }
	}
}
Write-Output ("scrollDone ms=" + $sw.ElapsedMilliseconds + " alive=" + (!$process.HasExited) + " gdi=" + [ScrollProbe]::Gdi($process.Id))
if (!$process.HasExited)
{
	& $exe -exit | Out-Null
	Start-Sleep -Seconds 3
	if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
}
Write-Output ("finalExited=" + $process.HasExited)
