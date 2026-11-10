param(
	[Parameter(Mandatory = $true)][ValidateSet('Modern Light', 'Modern Dark')][string]$Theme,
	[Parameter(Mandatory = $true)][string]$Language,
	[Parameter(Mandatory = $true)][string]$Prefix,
	[string]$Build = 'Release64',
	[switch]$SkipDeleteProgress,
	[switch]$StopAfterDelete
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$log = Join-Path $repo 'docs\ui\t4b-runtime-evidence.log'
$outDir = Join-Path $repo 'artifacts\ui\t4b'
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

if (-not ('T4bNative' -as [type])) {
	Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class T4bNative
{
	public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

	[DllImport("user32.dll")]
	static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")]
	public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")]
	static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)]
	static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)]
	static extern int GetClassName(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll")]
	public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
	[DllImport("user32.dll")]
	public static extern bool IsWindowVisible(IntPtr hWnd);
	[DllImport("user32.dll")]
	public static extern bool IsWindow(IntPtr hWnd);
	[DllImport("user32.dll")]
	public static extern bool PostMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll")]
	public static extern IntPtr SendMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll")]
	public static extern int GetDlgCtrlID(IntPtr hWnd);
	[DllImport("user32.dll")]
	public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")]
	public static extern IntPtr SetFocus(IntPtr hWnd);
	[DllImport("user32.dll")]
	public static extern bool BringWindowToTop(IntPtr hWnd);
	[DllImport("user32.dll")]
	public static extern bool ShowWindow(IntPtr hWnd, int command);
	[DllImport("user32.dll")]
	public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
	[DllImport("user32.dll")]
	public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern IntPtr OpenProcess(uint access, bool inheritHandle, uint processId);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern IntPtr VirtualAllocEx(IntPtr process, IntPtr address, UIntPtr size,
		uint allocationType, uint protect);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool VirtualFreeEx(IntPtr process, IntPtr address, UIntPtr size,
		uint freeType);
	[DllImport("kernel32.dll", SetLastError = true)]
	static extern bool WriteProcessMemory(IntPtr process, IntPtr address,
		IntPtr buffer, UIntPtr size, out UIntPtr written);
	[DllImport("kernel32.dll")]
	static extern bool CloseHandle(IntPtr handle);
	[DllImport("dwmapi.dll")]
	public static extern int DwmGetWindowAttribute(IntPtr hWnd, int attribute, out RECT rect, int size);

	[StructLayout(LayoutKind.Sequential)]
	public struct RECT
	{
		public int Left;
		public int Top;
		public int Right;
		public int Bottom;
	}

	[StructLayout(LayoutKind.Sequential)]
	public struct LVITEM
	{
		public uint mask;
		public int iItem;
		public int iSubItem;
		public uint state;
		public uint stateMask;
		public IntPtr pszText;
		public int cchTextMax;
		public int iImage;
		public IntPtr lParam;
		public int iIndent;
		public int iGroupId;
		public uint cColumns;
		public IntPtr puColumns;
		public IntPtr piColFmt;
		public int iGroup;
	}

	[StructLayout(LayoutKind.Sequential)]
	public struct NMHDR
	{
		public IntPtr hwndFrom;
		public IntPtr idFrom;
		public uint code;
	}

	public static string Text(IntPtr hWnd)
	{
		StringBuilder text = new StringBuilder(512);
		GetWindowText(hWnd, text, text.Capacity);
		return text.ToString();
	}

	public static string ClassName(IntPtr hWnd)
	{
		StringBuilder text = new StringBuilder(128);
		GetClassName(hWnd, text, text.Capacity);
		return text.ToString();
	}

	public static IntPtr[] TopLevel(int processId)
	{
		List<IntPtr> windows = new List<IntPtr>();
		EnumWindows((hWnd, lParam) =>
		{
			uint process;
			GetWindowThreadProcessId(hWnd, out process);
			if (process == processId)
			{
				windows.Add(hWnd);
			}
			return true;
		}, IntPtr.Zero);
		return windows.ToArray();
	}

	public static IntPtr[] Children(IntPtr parent)
	{
		List<IntPtr> windows = new List<IntPtr>();
		EnumChildWindows(parent, (hWnd, lParam) =>
		{
			windows.Add(hWnd);
			return true;
		}, IntPtr.Zero);
		return windows.ToArray();
	}

	public static IntPtr TopByProcessTitle(int processId, string title)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
		{
			if (Text(hWnd) == title)
			{
				return hWnd;
			}
		}
		return IntPtr.Zero;
	}

	public static IntPtr TopVisibleByTitle(int processId, string titlePrefix)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
		{
			if (IsWindowVisible(hWnd) &&
				Text(hWnd).StartsWith(titlePrefix, StringComparison.OrdinalIgnoreCase))
			{
				return hWnd;
			}
		}
		return IntPtr.Zero;
	}

	public static IntPtr TopVisibleByClassTitle(int processId, string className, string title)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
		{
			if (IsWindowVisible(hWnd) &&
				ClassName(hWnd) == className &&
				Text(hWnd) == title)
			{
				return hWnd;
			}
		}
		return IntPtr.Zero;
	}

	public static IntPtr ChildById(IntPtr parent, int id)
	{
		foreach (IntPtr hWnd in Children(parent))
		{
			if (GetDlgCtrlID(hWnd) == id)
			{
				return hWnd;
			}
		}
		return IntPtr.Zero;
	}

	public static IntPtr ChildByText(IntPtr parent, string text)
	{
		foreach (IntPtr hWnd in Children(parent))
		{
			if (Text(hWnd) == text)
			{
				return hWnd;
			}
		}
		return IntPtr.Zero;
	}

	public static IntPtr ChildByClass(IntPtr parent, string className)
	{
		foreach (IntPtr hWnd in Children(parent))
		{
			if (ClassName(hWnd) == className)
			{
				return hWnd;
			}
		}
		return IntPtr.Zero;
	}

	public static int SelectAllListItems(IntPtr list, int processId)
	{
		const uint PROCESS_VM_OPERATION = 0x0008;
		const uint PROCESS_VM_WRITE = 0x0020;
		const uint PROCESS_VM_READ = 0x0010;
		const uint MEM_COMMIT = 0x1000;
		const uint MEM_RESERVE = 0x2000;
		const uint MEM_RELEASE = 0x8000;
		const uint PAGE_READWRITE = 0x04;

		IntPtr process = OpenProcess(
			PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ,
			false, (uint)processId);
		if (process == IntPtr.Zero)
		{
			return -1;
		}

		LVITEM item = new LVITEM();
		item.stateMask = 0x0003;
		item.state = 0x0003;
		item.iItem = -1;
		int size = Marshal.SizeOf(item);
		IntPtr local = Marshal.AllocHGlobal(size);
		IntPtr remote = IntPtr.Zero;
		try
		{
			Marshal.StructureToPtr(item, local, false);
			remote = VirtualAllocEx(process, IntPtr.Zero, (UIntPtr)size,
				MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
			if (remote == IntPtr.Zero)
			{
				return -1;
			}

			UIntPtr written;
			if (!WriteProcessMemory(process, remote, local, (UIntPtr)size, out written))
			{
				return -1;
			}

			SendMessage(list, 0x102B, new IntPtr(-1), remote);
			return (int)SendMessage(list, 0x1032, IntPtr.Zero, IntPtr.Zero);
		}
		finally
		{
			if (remote != IntPtr.Zero)
			{
				VirtualFreeEx(process, remote, UIntPtr.Zero, MEM_RELEASE);
			}
			Marshal.FreeHGlobal(local);
			CloseHandle(process);
		}
	}
}
"@
}

[T4bNative]::SetProcessDpiAwarenessContext([IntPtr]-4) | Out-Null

function Note([string]$text)
{
	$line = "{0} {1}" -f (Get-Date).ToString('o'), $text
	$line | Add-Content -LiteralPath $log
	Write-Output $line
}

function Read-Setting([string]$path, [string]$name)
{
	$match = Select-String -LiteralPath $path -Pattern ('^' + [regex]::Escape($name) + '=(.*)$')
	if ($match)
	{
		return $match.Matches[0].Groups[1].Value.Trim()
	}
	return $null
}

function Save-Window([IntPtr]$hWnd, [string]$path)
{
	if ($hWnd -eq [IntPtr]::Zero)
	{
		throw 'window handle is zero'
	}

	[T4bNative]::ShowWindow($hWnd, 5) | Out-Null
	[T4bNative]::BringWindowToTop($hWnd) | Out-Null
	[T4bNative]::SetForegroundWindow($hWnd) | Out-Null
	Start-Sleep -Milliseconds 450

	$rect = New-Object T4bNative+RECT
	for ($attempt = 0; $attempt -lt 40; $attempt++)
	{
		[T4bNative]::GetWindowRect($hWnd, [ref]$rect) | Out-Null
		if (($rect.Right - $rect.Left) -gt 20 -and
			($rect.Bottom - $rect.Top) -gt 20)
		{
			break
		}
		Start-Sleep -Milliseconds 25
	}
	$extended = New-Object T4bNative+RECT
	if ([T4bNative]::DwmGetWindowAttribute(
		$hWnd, 9, [ref]$extended,
		[System.Runtime.InteropServices.Marshal]::SizeOf($extended)) -eq 0)
	{
		$rect = $extended
	}

	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	if ($width -le 0 -or $height -le 0)
	{
		throw "invalid window size ${width}x${height}"
	}
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	$hdc = $graphics.GetHdc()
	$printed = [T4bNative]::PrintWindow($hWnd, $hdc, 2)
	$graphics.ReleaseHdc($hdc)
	if (-not $printed)
	{
		$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0,
			(New-Object System.Drawing.Size $width, $height))
	}
	$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
	$graphics.Dispose()
	$bitmap.Dispose()
	Note "screenshot=$path size=${width}x${height} print=$printed"
}

function Save-Window-Screen([IntPtr]$hWnd, [string]$path)
{
	if ($hWnd -eq [IntPtr]::Zero)
	{
		throw 'window handle is zero'
	}

	[T4bNative]::ShowWindow($hWnd, 5) | Out-Null
	[T4bNative]::BringWindowToTop($hWnd) | Out-Null
	[T4bNative]::SetForegroundWindow($hWnd) | Out-Null
	Start-Sleep -Milliseconds 650

	$rect = New-Object T4bNative+RECT
	[T4bNative]::GetWindowRect($hWnd, [ref]$rect) | Out-Null
	$extended = New-Object T4bNative+RECT
	if ([T4bNative]::DwmGetWindowAttribute(
		$hWnd, 9, [ref]$extended,
		[System.Runtime.InteropServices.Marshal]::SizeOf($extended)) -eq 0)
	{
		$rect = $extended
	}

	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	$screenBounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
	$screenBitmap = New-Object System.Drawing.Bitmap $screenBounds.Width, $screenBounds.Height
	$graphics = [System.Drawing.Graphics]::FromImage($screenBitmap)
	$graphics.CopyFromScreen($screenBounds.Location, [System.Drawing.Point]::Empty,
		$screenBounds.Size)
	$destination = New-Object System.Drawing.Rectangle (
		$rect.Left - $screenBounds.Left), ($rect.Top - $screenBounds.Top), $width, $height
	$crop = $screenBitmap.Clone($destination,
		[System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
	$crop.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
	$crop.Dispose()
	$graphics.Dispose()
	$screenBitmap.Dispose()
	Note "screen screenshot=$path size=${width}x${height}"
}

function Find-Window([int]$processId, [string]$title, [int]$tries = 40)
{
	for ($index = 0; $index -lt $tries; $index++)
	{
		$hWnd = [T4bNative]::TopVisibleByTitle($processId, $title)
		if ($hWnd -ne [IntPtr]::Zero)
		{
			return $hWnd
		}
		Start-Sleep -Milliseconds 150
	}
	return [IntPtr]::Zero
}

function Get-ActivePage([IntPtr]$sheet)
{
	foreach ($hWnd in [T4bNative]::Children($sheet))
	{
		if ([T4bNative]::ClassName($hWnd) -eq '#32770' -and
			[T4bNative]::IsWindowVisible($hWnd))
		{
			return $hWnd
		}
	}
	return [IntPtr]::Zero
}

function Set-Page([IntPtr]$sheet, [int]$index)
{
	[T4bNative]::SendMessage($sheet, 0x0465, [IntPtr]$index, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 450
	return Get-ActivePage $sheet
}

function Click-Control([IntPtr]$parent, [int]$id)
{
	$control = [T4bNative]::ChildById($parent, $id)
	if ($control -eq [IntPtr]::Zero)
	{
		throw "control $id was not found"
	}
	[T4bNative]::PostMessage($control, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
}

function Select-ListItem([IntPtr]$list, [int]$row)
{
	$item = New-Object T4bNative+LVITEM
	$item.stateMask = 0x0003
	$item.state = 0x0003
	$item.iItem = $row
	$size = [System.Runtime.InteropServices.Marshal]::SizeOf($item)
	$memory = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($size)
	try
	{
		[System.Runtime.InteropServices.Marshal]::StructureToPtr($item, $memory, $false)
		[T4bNative]::SendMessage($list, 0x102B, [IntPtr]::Zero, $memory) | Out-Null
	}
	finally
	{
		[System.Runtime.InteropServices.Marshal]::FreeHGlobal($memory)
	}
}

function Open-FriendDetails([IntPtr]$page)
{
	$list = [T4bNative]::ChildByClass($page, 'SysListView32')
	if ($list -eq [IntPtr]::Zero)
	{
		throw 'friends list was not found'
	}
	[T4bNative]::SetFocus($list) | Out-Null
	$listCount = [T4bNative]::SendMessage($list, 0x1004, [IntPtr]::Zero, [IntPtr]::Zero)
	Note "friends list=$list count=$listCount"
	$x = 16
	$y = 34
	$point = [IntPtr](($y -shl 16) -bor ($x -band 0xffff))
	[T4bNative]::PostMessage($list, 0x0201, [IntPtr]1, $point) | Out-Null
	[T4bNative]::PostMessage($list, 0x0202, [IntPtr]::Zero, $point) | Out-Null
	Start-Sleep -Milliseconds 50
	[T4bNative]::PostMessage($list, 0x0203, [IntPtr]1, $point) | Out-Null
	[T4bNative]::PostMessage($list, 0x0202, [IntPtr]::Zero, $point) | Out-Null
	Start-Sleep -Milliseconds 500
}

$source = Join-Path $repo $Build
$exe = Join-Path $source 'Ditto.exe'
if (-not (Test-Path -LiteralPath $exe))
{
	throw "binary not found: $exe"
}

$run = Join-Path $env:TEMP ("ditto-ui-t4b-$Prefix-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run | Out-Null
Copy-Item -LiteralPath (Join-Path $source 'Ditto.exe') -Destination $run
Copy-Item -LiteralPath (Join-Path $source 'ICU_Loader.dll') -Destination $run
if (Test-Path -LiteralPath (Join-Path $source 'Addins'))
{
	Copy-Item -LiteralPath (Join-Path $source 'Addins') -Destination $run -Recurse
}
Copy-Item -LiteralPath (Join-Path $repo 'Debug\Language') -Destination $run -Recurse
Copy-Item -LiteralPath (Join-Path $repo 'Debug\Themes') -Destination $run -Recurse
New-Item -ItemType File -Path (Join-Path $run 'portable') | Out-Null

$settings = Join-Path $run 'Ditto.Settings'
@"
[Ditto]
Theme2=$Theme
DBPath=$run\Ditto.db
LanguageFile=$Language
"@ | Set-Content -LiteralPath $settings -Encoding Unicode

Set-Clipboard -Value ("T4b themed dialog smoke test {0}" -f [guid]::NewGuid().ToString('N'))
$process = $null
try
{
	Note "BEGIN Theme=$Theme Language=$Language Build=$Build Run=$run"
	$process = Start-Process -FilePath (Join-Path $run 'Ditto.exe') -PassThru
	Start-Sleep -Seconds 7
	if ($process.HasExited)
	{
		throw "Ditto exited at startup with code $($process.ExitCode)"
	}

	$mainValue = Read-Setting $settings 'MainhWnd'
	if (-not $mainValue)
	{
		throw 'MainhWnd was not written'
	}
	$main = [IntPtr][long]$mainValue
	Note "main=$main"

	Set-Clipboard -Value ("T4b copy properties clip {0}" -f [guid]::NewGuid().ToString('N'))
	Start-Sleep -Seconds 2
	for ($index = 0; $index -lt 120; $index++)
	{
		Set-Clipboard -Value ("T4b delete progress row {0:D3} {1}" -f $index, [guid]::NewGuid().ToString('N'))
		[T4bNative]::PostMessage($main, 0x0111, [IntPtr]32904, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 40
	}
	Start-Sleep -Seconds 3
	[T4bNative]::PostMessage($main, 0x0111, [IntPtr]32886, [IntPtr]::Zero) | Out-Null
	$deleteData = Find-Window $process.Id 'Delete Clip Data'
	if ($deleteData -eq [IntPtr]::Zero)
	{
		throw 'Delete Clip Data dialog was not found'
	}
	Save-Window $deleteData (Join-Path $outDir "t4b-delete-clip-data-$Prefix.png") | Out-Null
	Save-Window-Screen $deleteData (Join-Path $outDir "t4b-delete-clip-data-$Prefix-screen.png") | Out-Null

	if (-not $SkipDeleteProgress)
	{
		$list = [T4bNative]::ChildById($deleteData, 1018)
		$count = [int][T4bNative]::SendMessage($list, 0x1004, [IntPtr]::Zero, [IntPtr]::Zero)
		if ($count -gt 0)
		{
			$x = 20
			$y = 35
			$point = [IntPtr](($y -shl 16) -bor ($x -band 0xffff))
			[T4bNative]::SetForegroundWindow($deleteData) | Out-Null
			[T4bNative]::SetFocus($list) | Out-Null
			[T4bNative]::PostMessage($list, 0x0201, [IntPtr]1, $point) | Out-Null
			[T4bNative]::PostMessage($list, 0x0202, [IntPtr]::Zero, $point) | Out-Null
			Start-Sleep -Milliseconds 250
			$selectedCount = [T4bNative]::SelectAllListItems($list, $process.Id)
			Note "delete rows=$count selected=$selectedCount"
			Click-Control $deleteData 2118
			$confirm = [IntPtr]::Zero
			for ($index = 0; $index -lt 30; $index++)
			{
				$confirm = [T4bNative]::TopVisibleByClassTitle($process.Id, '#32770', '')
				if ($confirm -ne [IntPtr]::Zero)
				{
					break
				}
				Start-Sleep -Milliseconds 100
			}
			if ($confirm -ne [IntPtr]::Zero)
			{
				Click-Control $confirm 1
				Start-Sleep -Milliseconds 5
				$progress = [IntPtr]::Zero
				for ($index = 0; $index -lt 100; $index++)
				{
					$progress = [T4bNative]::TopVisibleByTitle($process.Id, 'Deleting clip items')
					if ($progress -ne [IntPtr]::Zero)
					{
						break
					}
					Start-Sleep -Milliseconds 20
				}
				if ($progress -ne [IntPtr]::Zero)
				{
					try
					{
						Save-Window $progress (Join-Path $outDir "t4b-progress-wnd-$Prefix.png") | Out-Null
						Save-Window-Screen $progress (Join-Path $outDir "t4b-progress-wnd-$Prefix-screen.png") | Out-Null
						Click-Control $progress 10
					}
					catch
					{
						Note "UNVERIFIED ProgressWnd disappeared before capture: $($_.Exception.Message)"
					}
				}
				else
				{
					Note "UNVERIFIED ProgressWnd could not be captured before completion"
				}
			}
			else
			{
				Note "UNVERIFIED delete confirmation was not found"
			}
		}
		else
		{
			Note "UNVERIFIED Delete Clip Data had no rows for ProgressWnd"
		}
	}

	Click-Control $deleteData 8
	Start-Sleep -Milliseconds 600
	if ($StopAfterDelete)
	{
		if ($process.HasExited)
		{
			throw 'Ditto exited during delete dialog testing'
		}
		Note "PASS Theme=$Theme Language=$Language delete-only=True"
		return
	}
	[T4bNative]::PostMessage($main, 0x04DE, [IntPtr]1, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Seconds 2
	$quickPaste = [T4bNative]::TopVisibleByClassTitle($process.Id, 'QPasteClass', 'Ditto')
	if ($quickPaste -eq [IntPtr]::Zero)
	{
		throw 'quick paste window was not found'
	}

	[T4bNative]::PostMessage($quickPaste, 0x0525, [IntPtr]1, [IntPtr]::Zero) | Out-Null
	$copyProperties = Find-Window $process.Id 'Copy Properties'
	if ($copyProperties -eq [IntPtr]::Zero)
	{
		throw 'Copy Properties dialog was not found'
	}
	Save-Window $copyProperties (Join-Path $outDir "t4b-copy-properties-$Prefix.png") | Out-Null
	Save-Window-Screen $copyProperties (Join-Path $outDir "t4b-copy-properties-$Prefix-screen.png") | Out-Null
	Click-Control $copyProperties 2
	Start-Sleep -Milliseconds 350

	[T4bNative]::PostMessage($quickPaste, 0x0111, [IntPtr]32939, [IntPtr]::Zero) | Out-Null
	$friendPrompt = Find-Window $process.Id 'Send to Friend'
	if ($friendPrompt -eq [IntPtr]::Zero)
	{
		throw 'Send to Friend dialog was not found'
	}
	Save-Window $friendPrompt (Join-Path $outDir "t4b-friend-prompt-$Prefix.png") | Out-Null
	Save-Window-Screen $friendPrompt (Join-Path $outDir "t4b-friend-prompt-$Prefix-screen.png") | Out-Null
	Click-Control $friendPrompt 2
	Start-Sleep -Milliseconds 350

	[T4bNative]::PostMessage($main, 0x0111, [IntPtr]32771, [IntPtr]::Zero) | Out-Null
	$options = Find-Window $process.Id 'Options'
	if ($options -eq [IntPtr]::Zero)
	{
		throw 'Options sheet was not found'
	}
	Save-Window $options (Join-Path $outDir "t4b-options-types-$Prefix.png") | Out-Null

	$typesPage = Set-Page $options 1
	Save-Window $options (Join-Path $outDir "t4b-options-types-$Prefix.png") | Out-Null
	Click-Control $typesPage 1015
	$addType = Find-Window $process.Id 'Add Supported Type'
	if ($addType -eq [IntPtr]::Zero)
	{
		throw 'Add Supported Type dialog was not found'
	}
	Save-Window $addType (Join-Path $outDir "t4b-add-type-$Prefix.png") | Out-Null
	Save-Window-Screen $addType (Join-Path $outDir "t4b-add-type-$Prefix-screen.png") | Out-Null
	Click-Control $addType 2
	Start-Sleep -Milliseconds 350

	$copyBuffersPage = Set-Page $options 3
	Save-Window $options (Join-Path $outDir "t4b-options-copy-buffers-$Prefix.png") | Out-Null
	Save-Window-Screen $options (Join-Path $outDir "t4b-options-copy-buffers-$Prefix-screen.png") | Out-Null
	foreach ($child in [T4bNative]::Children($copyBuffersPage))
	{
		$rect = New-Object T4bNative+RECT
		[T4bNative]::GetWindowRect($child, [ref]$rect) | Out-Null
		Note "copy buffer child id=$([T4bNative]::GetDlgCtrlID($child)) class=$([T4bNative]::ClassName($child)) visible=$([T4bNative]::IsWindowVisible($child)) rect=$($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom) text=[$([T4bNative]::Text($child))]"
	}

	$friendsPage = Set-Page $options 5
	Save-Window $options (Join-Path $outDir "t4b-options-friends-$Prefix.png") | Out-Null
	Open-FriendDetails $friendsPage
	$friendDetails = Find-Window $process.Id 'Friend Details'
	if ($friendDetails -eq [IntPtr]::Zero)
	{
		throw 'Friend Details dialog was not found'
	}
	Save-Window $friendDetails (Join-Path $outDir "t4b-friend-details-$Prefix.png") | Out-Null
	Save-Window-Screen $friendDetails (Join-Path $outDir "t4b-friend-details-$Prefix-screen.png") | Out-Null
	foreach ($child in [T4bNative]::Children($friendDetails))
	{
		if ($child -ne [IntPtr]::Zero)
		{
			$rect = New-Object T4bNative+RECT
			[T4bNative]::GetWindowRect($child, [ref]$rect) | Out-Null
			Note "friend detail child id=$([T4bNative]::GetDlgCtrlID($child)) class=$([T4bNative]::ClassName($child)) visible=$([T4bNative]::IsWindowVisible($child)) rect=$($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom) text=[$([T4bNative]::Text($child))]"
		}
	}
	Click-Control $friendDetails 2
	Start-Sleep -Milliseconds 350

	Set-Page $options 6 | Out-Null
	Save-Window $options (Join-Path $outDir "t4b-options-stats-$Prefix.png") | Out-Null
	Save-Window-Screen $options (Join-Path $outDir "t4b-options-stats-$Prefix-screen.png") | Out-Null
	Set-Page $options 7 | Out-Null
	Save-Window $options (Join-Path $outDir "t4b-options-about-$Prefix.png") | Out-Null
	Save-Window-Screen $options (Join-Path $outDir "t4b-options-about-$Prefix-screen.png") | Out-Null

	$generalPage = Set-Page $options 0
	Click-Control $generalPage 2124
	$advanced = Find-Window $process.Id 'Advanced Options'
	if ($advanced -eq [IntPtr]::Zero)
	{
		throw 'Advanced Options dialog was not found'
	}
	Click-Control $advanced 1042
	$moveGroup = Find-Window $process.Id 'Select group to reset clip order'
	if ($moveGroup -eq [IntPtr]::Zero)
	{
		throw 'Move to Group dialog was not found'
	}
	Save-Window $moveGroup (Join-Path $outDir "t4b-move-to-group-$Prefix.png") | Out-Null
	Click-Control $moveGroup 1040
	for ($index = 0; $index -lt 30; $index++)
	{
		$groupName = [T4bNative]::TopVisibleByClassTitle($process.Id, '#32770', 'Ditto')
		if ($groupName -ne [IntPtr]::Zero)
		{
			break
		}
		Start-Sleep -Milliseconds 100
	}
	if ($groupName -eq [IntPtr]::Zero)
	{
		throw 'Group Name dialog was not found'
	}
	Save-Window $groupName (Join-Path $outDir "t4b-group-name-$Prefix.png") | Out-Null
	Save-Window-Screen $groupName (Join-Path $outDir "t4b-group-name-$Prefix-screen.png") | Out-Null
	Click-Control $groupName 2
	Start-Sleep -Milliseconds 350
	Click-Control $moveGroup 2
	Start-Sleep -Milliseconds 350
	Click-Control $advanced 2
	Start-Sleep -Milliseconds 350
	Click-Control $options 2
	Start-Sleep -Milliseconds 700

	if ($process.HasExited)
	{
		throw 'Ditto exited during dialog smoke testing'
	}
	Note "PASS Theme=$Theme Language=$Language processAlive=True"
}
catch
{
	Note "FAIL Theme=$Theme Language=$Language error=$($_.Exception.Message)"
	throw
}
finally
{
	if ($process -ne $null -and -not $process.HasExited)
	{
		Stop-Process -Id $process.Id -Force
	}
}
