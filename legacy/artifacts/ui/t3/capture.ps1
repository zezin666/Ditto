param(
	[string]$Run,
	[string]$Theme,
	[string]$Prefix
)

$ErrorActionPreference = 'Stop'
$exe = Join-Path $Run 'Ditto.exe'

Get-Process | Where-Object { $_.Path -eq $exe } | ForEach-Object {
	Stop-Process -Id $_.Id -Force
	$_.WaitForExit()
}

Copy-Item (Join-Path (Get-Location) 'Release64\Ditto.exe') $exe -Force

$settings = @"
[Ditto]
Theme2=$Theme
DBPath3=$(Join-Path $Run 'Ditto.db')
LanguageFile=English
EnableDebugLogging=1
CheckForMaxEntries=1
SimpleTextSearch=0
LinesPerRow=1
ShowScrollBar=1
UseModernScrollBar=1
ShowPersistent=1
DoNotHideOnDeactivate=1
QuickPasteCX=420
QuickPasteCY=480
MainhWnd=0
PastSearchXml=<PastSearches />
"@
Set-Content -Path (Join-Path $Run 'Ditto.Settings') -Value $settings -Encoding Ascii

$process = Start-Process -FilePath $exe -WorkingDirectory $Run -PassThru
Start-Sleep -Seconds 4
$process.Refresh()
if ($process.HasExited)
{
	throw 'Ditto exited during startup'
}

$script:targetPid = [uint32]$process.Id
$main = [IntPtr][long]([regex]::Match(
	(Get-Content -Raw (Join-Path $Run 'Ditto.Settings')),
	'MainhWnd=([^\r\n]+)').Groups[1].Value)

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class T3W
{
	public delegate bool EnumProc(IntPtr h, IntPtr l);

	[DllImport("user32.dll")]
	public static extern bool EnumWindows(EnumProc p, IntPtr l);
	[DllImport("user32.dll")]
	public static extern bool EnumChildWindows(IntPtr p, EnumProc cb, IntPtr l);
	[DllImport("user32.dll")]
	public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
	[DllImport("user32.dll")]
	public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)]
	public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")]
	public static extern IntPtr GetParent(IntPtr h);
	[DllImport("user32.dll")]
	public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)]
	public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, string l);
	[DllImport("user32.dll")]
	public static extern bool GetWindowRect(IntPtr h, out RECT r);
	[DllImport("user32.dll")]
	public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
	[DllImport("user32.dll")]
	public static extern bool ShowWindow(IntPtr h, int c);
	[DllImport("user32.dll")]
	public static extern bool SetForegroundWindow(IntPtr h);
	[DllImport("user32.dll")]
	public static extern bool UpdateWindow(IntPtr h);
	[DllImport("user32.dll")]
	public static extern bool RedrawWindow(IntPtr h, IntPtr rect, IntPtr region, uint flags);
	public struct RECT
	{
		public int L;
		public int T;
		public int R;
		public int B;
	}
}
'@

function FindTopWindow([uint32]$processId, [string]$className)
{
	$script:found = [IntPtr]::Zero
	[T3W]::EnumWindows({
		param($h, $l)
		$windowPid = 0
		[T3W]::GetWindowThreadProcessId($h, [ref]$windowPid) | Out-Null
		if ($windowPid -eq $processId)
		{
			$class = New-Object Text.StringBuilder 256
			[T3W]::GetClassName($h, $class, 256) | Out-Null
			if ($class.ToString() -eq $className)
			{
				$script:found = $h
			}
		}
		return $true
	}, [IntPtr]::Zero) | Out-Null
	return $script:found
}

function FindChildWindow([IntPtr]$parent, [string]$className)
{
	$script:found = [IntPtr]::Zero
	[T3W]::EnumChildWindows($parent, {
		param($h, $l)
		$class = New-Object Text.StringBuilder 256
		[T3W]::GetClassName($h, $class, 256) | Out-Null
		if ($class.ToString() -eq $className)
		{
			$script:found = $h
		}
		return $true
	}, [IntPtr]::Zero) | Out-Null
	return $script:found
}

function CaptureCompositeWindow([IntPtr]$hwnd, [string]$file)
{
	$rect = New-Object T3W+RECT
	[T3W]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
	$width = $rect.R - $rect.L
	$height = $rect.B - $rect.T
	if ($width -le 0 -or $height -le 0)
	{
		throw "Invalid capture rect for $hwnd"
	}

	$bitmap = New-Object Drawing.Bitmap $width, $height
	$graphics = [Drawing.Graphics]::FromImage($bitmap)
	$dc = $graphics.GetHdc()
	$printed = [T3W]::PrintWindow($hwnd, $dc, 2)
	$graphics.ReleaseHdc($dc)
	$graphics.Dispose()
	if (-not $printed)
	{
		throw "PrintWindow failed for $hwnd"
	}

	# Child controls do not always participate in a parent WM_PRINT pass.
	# Composite visible children when PrintWindow can render them.
	$parentClass = New-Object Text.StringBuilder 256
	[T3W]::GetClassName($hwnd, $parentClass, 256) | Out-Null
	$isQuickPaste = $parentClass.ToString() -eq 'QPasteClass'
	$script:children = @()
	[T3W]::EnumChildWindows($hwnd, {
		param($child, $l)
		if ([T3W]::GetParent($child) -eq $hwnd -and
			[T3W]::IsWindowVisible($child))
		{
			$class = New-Object Text.StringBuilder 256
			[T3W]::GetClassName($child, $class, 256) | Out-Null
			if (-not $isQuickPaste -or $class.ToString().StartsWith('Afx:'))
			{
				$script:children += $child
			}
		}
		return $true
	}, [IntPtr]::Zero) | Out-Null

	foreach ($child in $script:children)
	{
		$childRect = New-Object T3W+RECT
		[T3W]::GetWindowRect($child, [ref]$childRect) | Out-Null
		$childWidth = $childRect.R - $childRect.L
		$childHeight = $childRect.B - $childRect.T
		if ($childWidth -le 0 -or $childHeight -le 0)
		{
			continue
		}

		$childBitmap = New-Object Drawing.Bitmap $childWidth, $childHeight
		$childGraphics = [Drawing.Graphics]::FromImage($childBitmap)
		$childClass = New-Object Text.StringBuilder 256
		[T3W]::GetClassName($child, $childClass, 256) | Out-Null
		$childDc = $childGraphics.GetHdc()
		$childPrinted = [T3W]::PrintWindow($child, $childDc, 2)
		$childGraphics.ReleaseHdc($childDc)
		$childGraphics.Dispose()

		if ($childPrinted)
		{
			$targetGraphics = [Drawing.Graphics]::FromImage($bitmap)
			$targetGraphics.DrawImage(
				$childBitmap,
				$childRect.L - $rect.L,
				$childRect.T - $rect.T)
			$targetGraphics.Dispose()
		}
		$childBitmap.Dispose()
	}

	$bitmap.Save($file, [Drawing.Imaging.ImageFormat]::Png)
	$bitmap.Dispose()
}

try
{
	[T3W]::SendMessage($main, 1246, [IntPtr]1, [IntPtr]0) | Out-Null
	Start-Sleep -Seconds 4

	$popup = FindTopWindow $script:targetPid 'QPasteClass'
	if ($popup -eq [IntPtr]::Zero)
	{
		throw 'QPaste window not found'
	}
	$script:popup = $popup
	[T3W]::ShowWindow($popup, 5) | Out-Null
	[T3W]::SetForegroundWindow($popup) | Out-Null
	Start-Sleep -Milliseconds 500

	$list = FindChildWindow $popup 'SysListView32'
	$edit = FindChildWindow $popup 'Edit'
	$out = Join-Path (Get-Location) 'artifacts\ui\t3'
	New-Item -ItemType Directory -Force -Path $out | Out-Null
	$files = @()

	$file = Join-Path $out ($Prefix + '-selected-image-color.png')
	CaptureCompositeWindow $popup $file
	$files += $file

	$mousePoint = [IntPtr]((75 -shl 16) -bor 120)
	[T3W]::SendMessage($list, 0x0200, [IntPtr]0, $mousePoint) | Out-Null
	[T3W]::RedrawWindow($list, [IntPtr]::Zero, [IntPtr]::Zero, 0x0085) | Out-Null
	[T3W]::UpdateWindow($list) | Out-Null
	Start-Sleep -Milliseconds 500
	$file = Join-Path $out ($Prefix + '-hover.png')
	CaptureCompositeWindow $popup $file
	$files += $file

	[T3W]::SendMessage($edit, 0x000C, [IntPtr]0, 'portable') | Out-Null
	[T3W]::SendMessage($popup, 0x512, [IntPtr]0, [IntPtr]0) | Out-Null
	Start-Sleep -Seconds 1
	$file = Join-Path $out ($Prefix + '-search-highlight.png')
	CaptureCompositeWindow $popup $file
	$files += $file

	[T3W]::SendMessage($edit, 0x000C, [IntPtr]0, '') | Out-Null
	[T3W]::SendMessage($popup, 0x512, [IntPtr]0, [IntPtr]0) | Out-Null
	Start-Sleep -Seconds 1
	for ($i = 0; $i -lt 8; $i++)
	{
		[T3W]::SendMessage($list, 0x0115, [IntPtr]1, [IntPtr]0) | Out-Null
	}
	Start-Sleep -Milliseconds 500

	$script:scrollbars = @()
	[T3W]::EnumChildWindows($popup, {
		param($child, $l)
		if ([T3W]::GetParent($child) -eq $script:popup -and
			[T3W]::IsWindowVisible($child))
		{
			$class = New-Object Text.StringBuilder 256
			[T3W]::GetClassName($child, $class, 256) | Out-Null
			if ($class.ToString().StartsWith('Afx:'))
			{
				$script:scrollbars += $child
			}
		}
		return $true
	}, [IntPtr]::Zero) | Out-Null

	foreach ($scrollbar in $script:scrollbars)
	{
		$scrollRect = New-Object T3W+RECT
		[T3W]::GetWindowRect($scrollbar, [ref]$scrollRect) | Out-Null
		if (($scrollRect.R - $scrollRect.L) -le 20)
		{
			$hoverPoint = [IntPtr]((100 -shl 16) -bor 5)
			[T3W]::SendMessage($scrollbar, 0x0200, [IntPtr]0, $hoverPoint) | Out-Null
			break
		}
	}
	Start-Sleep -Milliseconds 500
	$file = Join-Path $out ($Prefix + '-scrollbar-hover.png')
	CaptureCompositeWindow $popup $file
	$files += $file

	[T3W]::SendMessage($popup, 0x0111, [IntPtr]32793, [IntPtr]0) | Out-Null
	Start-Sleep -Seconds 2

	$script:tops = @()
	[T3W]::EnumWindows({
		param($h, $l)
		$windowPid = 0
		[T3W]::GetWindowThreadProcessId($h, [ref]$windowPid) | Out-Null
		if ($windowPid -eq $script:targetPid -and
			$h -ne $script:popup -and
			[T3W]::IsWindowVisible($h))
		{
			$rect = New-Object T3W+RECT
			[T3W]::GetWindowRect($h, [ref]$rect) | Out-Null
			if (($rect.R - $rect.L) -gt 150 -and ($rect.B - $rect.T) -gt 80)
			{
				$script:tops += $h
			}
		}
		return $true
	}, [IntPtr]::Zero) | Out-Null

	$description = $script:tops | Select-Object -First 1
	if ($description -and $description -ne [IntPtr]::Zero)
	{
		[T3W]::RedrawWindow($description, [IntPtr]::Zero, [IntPtr]::Zero, 0x0185) | Out-Null
		Start-Sleep -Milliseconds 300
		$file = Join-Path $out ($Prefix + '-description.png')
		CaptureCompositeWindow $description $file
		$files += $file
	}

	$files | ForEach-Object { Get-Item $_ | Select-Object FullName, Length }
}
finally
{
	Get-Process | Where-Object { $_.Path -eq $exe } | ForEach-Object {
		Stop-Process -Id $_.Id -Force
		$_.WaitForExit()
	}
}
