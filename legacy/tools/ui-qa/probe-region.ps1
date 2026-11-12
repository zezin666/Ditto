param([Parameter(Mandatory = $true)][string]$RunDir)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class ProbeRgn
{
	public delegate bool EP(IntPtr h, IntPtr l);
	[DllImport("user32.dll")] public static extern bool EnumWindows(EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr h, IntPtr r);
	[DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int a, int b, int c, int d);
	[DllImport("gdi32.dll")] public static extern int GetRgnBox(IntPtr r, out RECT box);
	[DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr h);
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
	[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	public struct RECT { public int Left, Top, Right, Bottom; }
	public struct POINT { public int X, Y; }
	public static IntPtr FindPopup(int pid)
	{
		IntPtr result = IntPtr.Zero;
		EnumWindows((h, l) => {
			uint p; GetWindowThreadProcessId(h, out p);
			if (p == pid && IsWindowVisible(h)) {
				var sb = new StringBuilder(128); GetClassName(h, sb, 128);
				if (sb.ToString() == "QPasteClass") { result = h; return false; }
			}
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
	public static string Region(IntPtr h)
	{
		IntPtr r = CreateRectRgn(0, 0, 0, 0);
		RECT box; int type = GetWindowRgn(h, r); GetRgnBox(r, out box); DeleteObject(r);
		RECT wr; GetWindowRect(h, out wr);
		POINT topLeft = new POINT(); topLeft.X = box.Left; topLeft.Y = box.Top; ClientToScreen(h, ref topLeft);
		POINT bottomRight = new POINT(); bottomRight.X = box.Right; bottomRight.Y = box.Bottom; ClientToScreen(h, ref bottomRight);
		return "type=" + type + " win=" + wr.Left + "," + wr.Top + "," + wr.Right + "," + wr.Bottom + " rgnScreen=" + topLeft.X + "," + topLeft.Y + "," + bottomRight.X + "," + bottomRight.Y;
	}
}
"@
$exe = Join-Path $RunDir 'Ditto.exe'
$process = Start-Process -FilePath $exe -WorkingDirectory $RunDir -PassThru
Start-Sleep -Seconds 4
& $exe -open | Out-Null
Start-Sleep -Seconds 2
$popup = [ProbeRgn]::FindPopup($process.Id)
if ($popup -eq [IntPtr]::Zero) { Write-Output 'popup not found' }
else
{
	$list = [ProbeRgn]::ChildById($popup, 0x201)
	$search = [ProbeRgn]::ChildById($popup, 0x202)
	Write-Output ('list ' + [ProbeRgn]::Region($list))
	$sr = New-Object ProbeRgn+RECT
	[ProbeRgn]::GetWindowRect($search, [ref]$sr) | Out-Null
	Write-Output ("search=" + $sr.Left + "," + $sr.Top + "," + $sr.Right + "," + $sr.Bottom)
}
& $exe -exit | Out-Null
Start-Sleep -Seconds 1
if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
