param(
	[Parameter(Mandatory = $true)][ValidateSet('Modern Light', 'Modern Dark')][string]$Theme,
	[Parameter(Mandatory = $true)][string]$Prefix,
	[string]$Build = 'Debug64',
	[int]$Rows = 5000
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$artifactDir = Join-Path $repo 'artifacts\ui\after'
$logPath = Join-Path $repo 'docs\ui\t6-review-runtime-evidence.log'
$runLogPath = Join-Path $repo ("docs\ui\t6-review-runtime-{0}.log" -f $Prefix)
New-Item -ItemType Directory -Path $artifactDir -Force | Out-Null

function Note([string]$message)
{
	$line = "{0} [{1}] {2}" -f (Get-Date -Format 'yyyy-MM-ddTHH:mm:ss.fff'), $Prefix, $message
	Add-Content -LiteralPath $logPath -Value $line
	Add-Content -LiteralPath $runLogPath -Value $line
	Write-Output $message
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

if (-not ('T6Native' -as [type])) {
	Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class T6Native
{
	public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

	[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageString(IntPtr hWnd, uint message, IntPtr wParam, string text);
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern IntPtr GetNextDlgTabItem(IntPtr dialog, IntPtr control, bool previous);
	[DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr hWnd, int index);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);
	[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint currentThread, uint targetThread, bool attach);
	[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
	[DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, int processId);
	[DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
	[DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
	[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
	[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
	[DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr hWnd, IntPtr rgn);
	[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
	[DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int left, int top, int right, int bottom);
	[DllImport("gdi32.dll")] public static extern int GetRgnBox(IntPtr rgn, out RECT box);
	[DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr handle);
	[DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint threadId, ref GUITHREADINFO info);

	[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
	[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
	[StructLayout(LayoutKind.Sequential)] public struct GUITHREADINFO
	{
		public int cbSize; public int flags; public IntPtr hwndActive; public IntPtr hwndFocus;
		public IntPtr hwndCapture; public IntPtr hwndMenuOwner; public IntPtr hwndMoveSize;
		public IntPtr hwndCaret; public RECT rcCaret;
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
		EnumWindows((hWnd, lParam) => { uint p; GetWindowThreadProcessId(hWnd, out p); if (p == processId) windows.Add(hWnd); return true; }, IntPtr.Zero);
		return windows.ToArray();
	}

	public static IntPtr[] Children(IntPtr parent)
	{
		List<IntPtr> windows = new List<IntPtr>();
		EnumChildWindows(parent, (hWnd, lParam) => { windows.Add(hWnd); return true; }, IntPtr.Zero);
		return windows.ToArray();
	}

	public static IntPtr TopVisibleByClass(int processId, string className)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (IsWindowVisible(hWnd) && ClassName(hWnd) == className) return hWnd;
		return IntPtr.Zero;
	}

	public static IntPtr TopVisibleByTitle(int processId, string title)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (IsWindowVisible(hWnd) && Text(hWnd).StartsWith(title, StringComparison.OrdinalIgnoreCase)) return hWnd;
		return IntPtr.Zero;
	}

	public static IntPtr ChildById(IntPtr parent, int id)
	{
		foreach (IntPtr hWnd in Children(parent))
			if (GetDlgCtrlID(hWnd) == id) return hWnd;
		return IntPtr.Zero;
	}

	public static IntPtr ChildByClass(IntPtr parent, string className)
	{
		foreach (IntPtr hWnd in Children(parent))
			if (ClassName(hWnd) == className) return hWnd;
		return IntPtr.Zero;
	}

	public static int ItemCount(IntPtr list)
	{
		return (int)SendMessage(list, 0x1004, IntPtr.Zero, IntPtr.Zero);
	}

	public static int SelectedIndex(IntPtr list)
	{
		return (int)SendMessage(list, 0x100C, new IntPtr(-1), new IntPtr(0x0002));
	}

	public static RECT VisibleRect(IntPtr hWnd)
	{
		RECT window;
		GetWindowRect(hWnd, out window);
		IntPtr rgn = CreateRectRgn(0, 0, 0, 0);
		int type = GetWindowRgn(hWnd, rgn);
		if (type == 0) { DeleteObject(rgn); return window; }
		RECT box;
		GetRgnBox(rgn, out box);
		DeleteObject(rgn);
		POINT topLeft = new POINT(); topLeft.X = box.Left; topLeft.Y = box.Top; ClientToScreen(hWnd, ref topLeft);
		POINT bottomRight = new POINT(); bottomRight.X = box.Right; bottomRight.Y = box.Bottom; ClientToScreen(hWnd, ref bottomRight);
		RECT visible; visible.Left = topLeft.X; visible.Top = topLeft.Y; visible.Right = bottomRight.X; visible.Bottom = bottomRight.Y;
		return visible;
	}
}
"@
}

[T6Native]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

function Get-Rect([IntPtr]$hWnd)
{
	$rect = New-Object T6Native+RECT
	[T6Native]::GetWindowRect($hWnd, [ref]$rect) | Out-Null
	return $rect
}

function Save-Window-Screen([IntPtr]$hWnd, [string]$path)
{
	[T6Native]::SetWindowPos($hWnd, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0040) | Out-Null
	[T6Native]::SetForegroundWindow($hWnd) | Out-Null
	Start-Sleep -Milliseconds 350
	$rect = Get-Rect $hWnd
	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	if ($width -le 0 -or $height -le 0) { throw "Invalid window rect for capture: $width x $height" }
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	try
	{
		$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
		$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
	}
	finally { $graphics.Dispose(); $bitmap.Dispose() }
	Note ("screenshot={0} size={1}x{2}" -f $path, $width, $height)
}

function Save-Region-Screen($rect, [string]$path)
{
	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	if ($width -le 0 -or $height -le 0) { Note 'menu rect empty; not captured'; return }
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	try
	{
		$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
		$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
	}
	finally { $graphics.Dispose(); $bitmap.Dispose() }
	Note ("screenshot-region={0} size={1}x{2}" -f $path, $width, $height)
}

function Focus-Window([IntPtr]$window, [IntPtr]$focusTarget)
{
	$processId = [uint32]0
	$targetThread = [T6Native]::GetWindowThreadProcessId($window, [ref]$processId)
	$currentThread = [T6Native]::GetCurrentThreadId()
	[T6Native]::AttachThreadInput($currentThread, $targetThread, $true) | Out-Null
	try
	{
		[T6Native]::SetForegroundWindow($window) | Out-Null
		[T6Native]::SetFocus($focusTarget) | Out-Null
	}
	finally { [T6Native]::AttachThreadInput($currentThread, $targetThread, $false) | Out-Null }
}

function Send-Search([IntPtr]$popup, [IntPtr]$edit, [string]$text)
{
	[T6Native]::SendMessageString($edit, 0x000C, [IntPtr]::Zero, $text) | Out-Null
	$command = [IntPtr]((0x0300 -shl 16) -bor 0x202)
	[T6Native]::SendMessage($popup, 0x0111, $command, $edit) | Out-Null
}

function Get-FocusHwnd([IntPtr]$window)
{
	$processId = [uint32]0
	$threadId = [T6Native]::GetWindowThreadProcessId($window, [ref]$processId)
	$info = New-Object T6Native+GUITHREADINFO
	$info.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($info)
	[T6Native]::GetGUIThreadInfo($threadId, [ref]$info) | Out-Null
	return $info.hwndFocus
}

function Focus-Control([IntPtr]$popup, [IntPtr]$target)
{
	for ($attempt = 0; $attempt -lt 5; $attempt++)
	{
		Focus-Window $popup $target
		Start-Sleep -Milliseconds 200
		$focus = Get-FocusHwnd $popup
		if ($focus -eq $target) { return $true }
	}
	Note ("focus-failed target={0} actual={1}" -f $target, (Get-FocusHwnd $popup))
	return $false
}

function Get-GuiCounts([int]$processId)
{
	$handle = [T6Native]::OpenProcess(0x0400, $false, $processId)
	if ($handle -eq [IntPtr]::Zero) { return @{ Gdi = -1; User = -1 } }
	try { return @{ Gdi = [int][T6Native]::GetGuiResources($handle, 0); User = [int][T6Native]::GetGuiResources($handle, 1) } }
	finally { [T6Native]::CloseHandle($handle) | Out-Null }
}

function Read-Setting([string]$path, [string]$name)
{
	foreach ($line in Get-Content -LiteralPath $path)
	{
		$trimmed = $line.Trim()
		if ($trimmed.StartsWith($name + '=')) { return $trimmed.Substring($name.Length + 1).Trim() }
	}
	return $null
}

$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$runValueBefore = $null
$runValueExisted = $false
try
{
	$runValueBefore = (Get-ItemProperty -Path $runKey -Name 'Ditto' -ErrorAction Stop).Ditto
	$runValueExisted = $true
}
catch { }

$run = Join-Path $env:TEMP ("ditto-ui-t6-" + $Prefix + "-" + [guid]::NewGuid().ToString('N'))
$dbPath = Join-Path $run 'Ditto.db'
$settingsPath = Join-Path $run 'Ditto.Settings'
$exe = Join-Path $run 'Ditto.exe'
$process = $null

Note ("BEGIN Theme={0} Build={1} Rows={2} Run={3}" -f $Theme, $Build, $Rows, $run)

try
{
	New-Item -ItemType Directory -Path $run | Out-Null
	Copy-Item (Join-Path $repo "$Build\Ditto.exe") $run
	Copy-Item (Join-Path $repo "$Build\ICU_Loader.dll") $run
	Copy-Item (Join-Path $repo "$Build\Addins") $run -Recurse
	Copy-Item (Join-Path $repo 'Debug\Language') $run -Recurse
	Copy-Item (Join-Path $repo 'Debug\Themes') $run -Recurse
	New-Item -ItemType File -Path (Join-Path $run 'portable') | Out-Null

	@"
[Ditto]
Theme2=$Theme
DBPath=$dbPath
LanguageFile=English
EnableDebugLogging=1
"@ | Set-Content -LiteralPath $settingsPath -Encoding Unicode

	$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds(25)
	while (!(Test-Path -LiteralPath $dbPath) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
	if (!(Test-Path -LiteralPath $dbPath)) { throw 'Portable test database was not created' }
	& $exe -exit | Out-Null
	$process.WaitForExit(8000) | Out-Null
	if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }

	$python = "import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); n=int(sys.argv[2]); db.execute('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(42,0,?,0,0,0,1,-1,NULL,0,0,0,0,0,0,0,0)',('T6 group',)); db.execute('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(100,0,?,0,0,0,0,-1,NULL,0,0,0,0,0,0,0,0)',('rgba(37, 99, 235, 0.65)',)); rows=[(1000+i,0,'T6 row %05d' % i,0,0,0,0,-1,None,0,0,0,0,0,0,0,0) for i in range(n)]; db.executemany('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', rows); db.commit(); db.close()"
	& python -c $python $dbPath $Rows
	if ($LASTEXITCODE -ne 0) { throw 'Failed to seed temporary test database' }

	$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds(30)
	$main = [IntPtr]::Zero
	while ([DateTime]::UtcNow -lt $deadline)
	{
		# The command-receiving HWND is the hidden class "Ditto" window written
		# to MainhWnd. The Afx "Ditto Clipboard Viewer" frame does not process
		# WM_OPEN_CLOSE_WINDOW.
		$settingMain = Read-Setting $settingsPath 'MainhWnd'
		if ($settingMain)
		{
			$candidate = [long]$settingMain
			if ($candidate -gt 0 -and [T6Native]::IsWindow([IntPtr]$candidate)) { $main = [IntPtr]$candidate; break }
		}
		foreach ($hWnd in [T6Native]::TopLevel($process.Id))
		{
			if ([T6Native]::ClassName($hWnd) -eq 'Ditto' -and [T6Native]::Text($hWnd) -eq 'Ditto') { $main = $hWnd; break }
		}
		if ($main -ne [IntPtr]::Zero) { break }
		Start-Sleep -Milliseconds 250
	}
	if ($main -eq [IntPtr]::Zero) { throw 'Main frame window was not found' }
	Note ("main={0}" -f $main)
	Start-Sleep -Seconds 1

	$popup = [IntPtr]::Zero
	for ($attempt = 0; $attempt -lt 4 -and $popup -eq [IntPtr]::Zero; $attempt++)
	{
		# WM_OPEN_CLOSE_WINDOW = WM_USER + 222; wParam=1 opens. This is more
		# reliable than the -open helper when focus is being stolen.
		[T6Native]::PostMessage($main, 0x04DE, [IntPtr]1, [IntPtr]::Zero) | Out-Null
		for ($index = 0; $index -lt 40; $index++)
		{
			$popup = [T6Native]::TopVisibleByClass($process.Id, 'QPasteClass')
			if ($popup -ne [IntPtr]::Zero) { break }
			Start-Sleep -Milliseconds 100
		}
	}
	if ($popup -eq [IntPtr]::Zero) { throw 'Quick paste popup was not found' }
	[T6Native]::SetForegroundWindow($popup) | Out-Null
	Start-Sleep -Seconds 2

	$list = [T6Native]::ChildById($popup, 0x201)
	$edit = [T6Native]::ChildById($popup, 0x202)
	$groups = [T6Native]::ChildById($popup, 0x205)
	$back = [T6Native]::ChildById($popup, 0x207)
	$system = [T6Native]::ChildById($popup, 0x210)
	$options = [T6Native]::ChildById($popup, 0x8025)
	$empty = [T6Native]::ChildById($popup, 0x211)
	$tree = [T6Native]::ChildByClass($popup, 'SysTreeView32')
	$dpi = [T6Native]::GetDpiForWindow($popup)
	$itemCount = [T6Native]::ItemCount($list)
	Note ("popup-found dpi={0} listItems={1}" -f $dpi, $itemCount)

	$styleTabStop = 0x00010000
	function TabState([IntPtr]$hWnd)
	{
		if ($hWnd -eq [IntPtr]::Zero) { return 'missing' }
		$style = [T6Native]::GetWindowLong($hWnd, -16)
		if (($style -band $styleTabStop) -ne 0) { return 'True' } else { return 'False' }
	}
	Note ("tabStops groups={0} back={1} system={2} options={3} empty={4} list={5}" -f `
		(TabState $groups), (TabState $back), (TabState $system), (TabState $options), (TabState $empty), (TabState $list))

	$chain = @()
	$current = [T6Native]::GetNextDlgTabItem($popup, [IntPtr]::Zero, $false)
	for ($index = 0; $index -lt 12 -and $current -ne [IntPtr]::Zero; $index++)
	{
		$chain += [string][T6Native]::GetDlgCtrlID($current)
		$next = [T6Native]::GetNextDlgTabItem($popup, $current, $false)
		if ($next -eq $current) { break }
		$current = $next
	}
	Note ("tabChain=" + ($chain -join ','))

	$listRect = Get-Rect $list
	$visible = [T6Native]::VisibleRect($list)
	$editRect = Get-Rect $edit
	$optionsRect = if ($options -ne [IntPtr]::Zero) { Get-Rect $options } else { $null }
	Note ("list-visible={0},{1},{2},{3} edit={4},{5},{6},{7}" -f $visible.Left, $visible.Top, $visible.Right, $visible.Bottom, $editRect.Left, $editRect.Top, $editRect.Right, $editRect.Bottom)
	$layoutPass = ($visible.Bottom -le $editRect.Top + 1)
	if ($optionsRect -ne $null -and $visible.Bottom -gt $optionsRect.Top + 1) { $layoutPass = $false }
	Note ("LAYOUT {0}" -f $(if ($layoutPass) { 'PASS' } else { 'FAIL' }))

	Save-Window-Screen $popup (Join-Path $artifactDir ("t6-{0}-popup.png" -f $Prefix))

	# Footer tab focus and empty-state keyboard path.
	Focus-Window $popup $options
	Start-Sleep -Milliseconds 300
	Save-Window-Screen $popup (Join-Path $artifactDir ("t6-{0}-footer-focus.png" -f $Prefix))

	Send-Search $popup $edit 'zzzz-t6-no-match'
	Start-Sleep -Seconds 2
	$empty = [T6Native]::ChildById($popup, 0x211)
	Note ("empty visible={0} tabStop={1}" -f ([T6Native]::IsWindowVisible($empty)), (TabState $empty))
	$focusOk = Focus-Control $popup $empty
	Note ("empty-focus ok={0} focus={1}" -f $focusOk, (Get-FocusHwnd $popup))
	Start-Sleep -Milliseconds 300
	Save-Window-Screen $popup (Join-Path $artifactDir ("t6-{0}-empty-focus.png" -f $Prefix))
	[T6Native]::PostMessage($empty, 0x0100, [IntPtr]0x20, [IntPtr]::Zero) | Out-Null
	[T6Native]::PostMessage($empty, 0x0101, [IntPtr]0x20, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Seconds 2
	$afterSpace = [T6Native]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()
	$itemsAfterSpace = [T6Native]::ItemCount($list)
	Note ("space-clear textLength={0} items={1}" -f $afterSpace, $itemsAfterSpace)

	Send-Search $popup $edit 'zzzz-t6-no-match'
	Start-Sleep -Seconds 2
	$empty = [T6Native]::ChildById($popup, 0x211)
	$focusOkEnter = Focus-Control $popup $empty
	Note ("empty-focus-enter ok={0} focus={1}" -f $focusOkEnter, (Get-FocusHwnd $popup))
	[T6Native]::PostMessage($empty, 0x0100, [IntPtr]0x0D, [IntPtr]::Zero) | Out-Null
	[T6Native]::PostMessage($empty, 0x0101, [IntPtr]0x0D, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Seconds 2
	$afterEnter = [T6Native]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()
	$itemsAfterEnter = [T6Native]::ItemCount($list)
	Note ("enter-clear textLength={0} items={1}" -f $afterEnter, $itemsAfterEnter)

	# Group-tree context menu.
	if ($tree -ne [IntPtr]::Zero)
	{
		$treeRect = Get-Rect $tree
		$x = [int](($treeRect.Right - $treeRect.Left) / 2)
		$y = [int](($treeRect.Bottom - $treeRect.Top) / 2)
		$point = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
		Focus-Window $popup $tree
		[T6Native]::PostMessage($tree, 0x0204, [IntPtr]2, $point) | Out-Null
		[T6Native]::PostMessage($tree, 0x0205, [IntPtr]2, $point) | Out-Null
		Start-Sleep -Milliseconds 700
		$menu = [T6Native]::TopVisibleByClass($process.Id, '#32768')
		if ($menu -ne [IntPtr]::Zero)
		{
			Save-Region-Screen (Get-Rect $menu) (Join-Path $artifactDir ("t6-{0}-group-menu.png" -f $Prefix))
			Note 'group-tree menu captured'
			[T6Native]::PostMessage($menu, 0x0100, [IntPtr]0x1B, [IntPtr]::Zero) | Out-Null
		}
		else { Note 'UNVERIFIED group-tree menu not found' }
		Start-Sleep -Milliseconds 400
	}

	# Search history menu: run a search, clear it, then click the chevron.
	Send-Search $popup $edit 'T6 history entry'
	Start-Sleep -Seconds 1
	Send-Search $popup $edit ''
	Start-Sleep -Milliseconds 500
	Focus-Window $popup $edit
	Start-Sleep -Milliseconds 200
	[T6Native]::SendMessageString($edit, 0x000C, [IntPtr]::Zero, 'T6 history') | Out-Null
	Start-Sleep -Milliseconds 400
	$editRect = Get-Rect $edit
	$editWidth = $editRect.Right - $editRect.Left
	$editHeight = $editRect.Bottom - $editRect.Top
	$scale = $dpi / 96.0
	$buttonSize = [int][Math]::Round(28 * $scale)
	$inset = [int][Math]::Round(8 * $scale)
	$historyX = $editWidth - $inset - $buttonSize - [int]($buttonSize / 2)
	$historyY = [int]($editHeight / 2)
	$historyPoint = [IntPtr](($historyY -shl 16) -bor ($historyX -band 0xFFFF))
	[T6Native]::PostMessage($edit, 0x0201, [IntPtr]1, $historyPoint) | Out-Null
	[T6Native]::PostMessage($edit, 0x0202, [IntPtr]::Zero, $historyPoint) | Out-Null
	Start-Sleep -Milliseconds 800
	$menu = [T6Native]::TopVisibleByClass($process.Id, '#32768')
	if ($menu -ne [IntPtr]::Zero)
	{
		Save-Region-Screen (Get-Rect $menu) (Join-Path $artifactDir ("t6-{0}-search-history-menu.png" -f $Prefix))
		Note 'search-history menu captured'
		[T6Native]::PostMessage($menu, 0x0100, [IntPtr]0x1B, [IntPtr]::Zero) | Out-Null
	}
	else { Note 'UNVERIFIED search-history menu not found' }
	Start-Sleep -Milliseconds 400
	Send-Search $popup $edit ''
	Start-Sleep -Seconds 1

	# 5k-row scroll exercise with GDI/user handle measurement.
	$countsBefore = Get-GuiCounts $process.Id
	$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
	for ($index = 0; $index -lt 200; $index++)
	{
		[T6Native]::SendMessage($list, 0x0115, [IntPtr]1, [IntPtr]::Zero) | Out-Null
	}
	for ($index = 0; $index -lt 200; $index++)
	{
		[T6Native]::SendMessage($list, 0x0115, [IntPtr]0, [IntPtr]::Zero) | Out-Null
	}
	$stopwatch.Stop()
	Start-Sleep -Milliseconds 500
	$countsAfter = Get-GuiCounts $process.Id
	Note ("scroll400 ms={0} gdiBefore={1} gdiAfter={2} deltaGdi={3} userBefore={4} userAfter={5} deltaUser={6} items={7}" -f `
		$stopwatch.ElapsedMilliseconds, $countsBefore.Gdi, $countsAfter.Gdi, ($countsAfter.Gdi - $countsBefore.Gdi), `
		$countsBefore.User, $countsAfter.User, ($countsAfter.User - $countsBefore.User), ([T6Native]::ItemCount($list)))

	# Select the first row so the translucent color swatch paints.
	Focus-Window $popup $list
	[T6Native]::PostMessage($list, 0x0100, [IntPtr]0x24, [IntPtr]::Zero) | Out-Null
	[T6Native]::PostMessage($list, 0x0101, [IntPtr]0x24, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 700
	Save-Window-Screen $popup (Join-Path $artifactDir ("t6-{0}-5k-list.png" -f $Prefix))
	# Filter to the color clip so the translucent swatch brush is created.
	Send-Search $popup $edit 'rgba(37, 99, 235, 0.65)'
	Start-Sleep -Seconds 2
	Save-Window-Screen $popup (Join-Path $artifactDir ("t6-{0}-color-swatch.png" -f $Prefix))

	& $exe -exit | Out-Null
	$process.WaitForExit(12000) | Out-Null
	$exited = $process.HasExited
	if (!$exited) { Stop-Process -Id $process.Id -Force }
	Get-Process Ditto -ErrorAction SilentlyContinue | Where-Object { $_.Path -like ($run + '*') } | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
	Note ("exit clean={0}" -f $exited)

	$logFile = Join-Path $run 'Ditto.log'
	if (Test-Path -LiteralPath $logFile)
	{
		foreach ($pattern in @('T3 list paint pass reused one Gdiplus::Graphics object', 'T3 UiDraw created translucent brush cache', 'T3 UiDraw purged translucent brush cache', 'ExitInstance'))
		{
			$found = Select-String -LiteralPath $logFile -Pattern $pattern -SimpleMatch
			Note ("log-marker [{0}] count={1}" -f $pattern, @($found).Count)
		}
		$interesting = Select-String -LiteralPath $logFile -Pattern 'exception|assert|failed|error' -CaseSensitive:$false |
			Where-Object { $_.Line -notmatch 'ErrorSaving|Error loading data|error code 0|Ditto_Options_Quick_Paste|CF_UNICODETEXT|BAD DESCRIPTION|OpenClipboard' } |
			Select-Object -First 10
		if ($interesting) { foreach ($match in $interesting) { Note ("log: " + $match.Line) } }
		else { Note 'log-scan clean (expected noise filtered)' }
	}
	else { Note 'log-scan no Ditto.log produced' }
}
catch
{
	Note ("FAIL Theme={0} error={1}" -f $Theme, $_.Exception.Message)
	throw
}
finally
{
	if ($process -ne $null -and !$process.HasExited)
	{
		try { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue } catch { }
	}
	Get-Process Ditto -ErrorAction SilentlyContinue | Where-Object { $_.Path -like ($run + '*') } | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
	try
	{
		if ($runValueExisted) { Set-ItemProperty -Path $runKey -Name 'Ditto' -Value $runValueBefore }
		else { Remove-ItemProperty -Path $runKey -Name 'Ditto' -ErrorAction SilentlyContinue }
	}
	catch { Note ("registry-restore-failed: " + $_.Exception.Message) }
}

Note 'DONE'
