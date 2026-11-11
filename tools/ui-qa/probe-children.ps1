param([Parameter(Mandatory = $true)][string]$RunDir)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ChildProbe
{
	public delegate bool EP(IntPtr h, IntPtr l);
	[DllImport("user32.dll")] public static extern bool EnumWindows(EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EP c, IntPtr l);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	public static string T(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
	public static string C(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
	public static IntPtr FindMain(int pid) { IntPtr r = IntPtr.Zero; EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p); if(p==pid && C(h)=="Ditto" && T(h)=="Ditto"){r=h;return false;} return true;},IntPtr.Zero); return r; }
	public static IntPtr ChildById(IntPtr parent, int id) { IntPtr r=IntPtr.Zero; EnumChildWindows(parent,(h,l)=>{if(GetDlgCtrlID(h)==id){r=h;return false;} return true;},IntPtr.Zero); return r; }
	public static IntPtr Popup(int pid) { IntPtr r = IntPtr.Zero; EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p); if(p==pid && IsWindowVisible(h) && C(h)=="QPasteClass"){r=h;return false;} return true;},IntPtr.Zero); return r; }
	public static string DumpTop(int pid) { var sb=new StringBuilder(); EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p); if(p==pid){sb.AppendLine("TOP id="+GetDlgCtrlID(h)+" class=["+C(h)+"] text=["+T(h)+"] vis="+IsWindowVisible(h));} return true;},IntPtr.Zero); return sb.ToString(); }
	public static string DumpChildren(IntPtr parent) { var sb=new StringBuilder(); EnumChildWindows(parent,(h,l)=>{sb.AppendLine("  id="+GetDlgCtrlID(h)+" class=["+C(h)+"] text=["+T(h)+"] vis="+IsWindowVisible(h)); return true;},IntPtr.Zero); return sb.ToString(); }
}
"@
$exe = Join-Path $RunDir 'Ditto.exe'
$p = Start-Process -FilePath $exe -WorkingDirectory $RunDir -PassThru
Start-Sleep -Seconds 4
Write-Output ([ChildProbe]::DumpTop($p.Id))
$main = [ChildProbe]::FindMain($p.Id)
Write-Output ("main=" + $main)
if ($main -ne [IntPtr]::Zero) { [ChildProbe]::PostMessage($main, 0x04DE, [IntPtr]1, [IntPtr]::Zero) | Out-Null }
Start-Sleep -Seconds 2
$popup = [ChildProbe]::Popup($p.Id)
Write-Output ("popup=" + $popup)
if ($popup -ne [IntPtr]::Zero) { Write-Output ([ChildProbe]::DumpChildren($popup)) }
$groupsButton = if ($popup -ne [IntPtr]::Zero) { [ChildProbe]::ChildById($popup, 0x205) } else { [IntPtr]::Zero }
Write-Output ("groupsButton=" + $groupsButton)
if ($groupsButton -ne [IntPtr]::Zero)
{
	[ChildProbe]::PostMessage($groupsButton, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Seconds 2
	Write-Output ("after-click popup=" + [ChildProbe]::Popup($p.Id))
	Write-Output ([ChildProbe]::DumpChildren($popup))
	Write-Output ([ChildProbe]::DumpTop($p.Id))
}
& $exe -exit | Out-Null
Start-Sleep -Seconds 2
if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }
