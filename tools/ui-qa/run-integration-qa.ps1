param(
	[Parameter(Mandatory = $true)][ValidateSet('Modern Light', 'Modern Dark')][string]$Theme,
	[Parameter(Mandatory = $true)][string]$Prefix,
	[string]$Build = 'Release64',
	[string]$Language = 'English',
	[switch]$SeedGroup,
	[switch]$SkipOptions,
	[switch]$SkipHotkey
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$artifactDir = Join-Path $repo 'artifacts\ui\after'
$logPath = Join-Path $repo 'docs\ui\t5-runtime-evidence.log'
New-Item -ItemType Directory -Path $artifactDir -Force | Out-Null

function Note([string]$message)
{
	$line = "{0} [{1}] {2}" -f (Get-Date -Format 'yyyy-MM-ddTHH:mm:ss.fff'), $Prefix, $message
	Add-Content -LiteralPath $logPath -Value $line
	Write-Output $message
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

if (-not ('T5Native' -as [type])) {
	Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class T5Native
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
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);
	[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
	[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint currentThread, uint targetThread, bool attach);
	[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
	[DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, int processId);
	[DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
	[DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
	[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
	[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
	[DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
	[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
	[DllImport("user32.dll")] public static extern bool RedrawWindow(IntPtr hWnd, IntPtr rect, IntPtr rgn, uint flags);
	[DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr hWnd, IntPtr rgn);
	[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
	[DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int left, int top, int right, int bottom);
	[DllImport("gdi32.dll")] public static extern int GetRgnBox(IntPtr rgn, out RECT box);
	[DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr handle);

	[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
	[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
	[StructLayout(LayoutKind.Sequential)] public struct LVITEM
	{
		public uint mask; public int iItem; public int iSubItem; public uint state; public uint stateMask;
		public IntPtr pszText; public int cchTextMax; public int iImage; public IntPtr lParam;
		public int iIndent; public int iGroupId; public uint cColumns; public IntPtr puColumns;
		public IntPtr piColFmt; public int iGroup;
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
			if (process == processId) windows.Add(hWnd);
			return true;
		}, IntPtr.Zero);
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

	public static IntPtr TopByTitle(int processId, string title)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (Text(hWnd) == title) return hWnd;
		return IntPtr.Zero;
	}

	public static IntPtr TopByTitlePrefix(int processId, string prefix)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (Text(hWnd).StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) return hWnd;
		return IntPtr.Zero;
	}

	public static IntPtr TopByClassPrefix(int processId, string prefix)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (ClassName(hWnd).StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) return hWnd;
		return IntPtr.Zero;
	}

	public static string TopSummary(int processId)
	{
		StringBuilder summary = new StringBuilder();
		foreach (IntPtr hWnd in TopLevel(processId))
			summary.Append("[" + ClassName(hWnd) + "|" + Text(hWnd) + "|vis=" + IsWindowVisible(hWnd) + "]");
		return summary.ToString();
	}

	public static IntPtr TopVisibleByClassTitle(int processId, string className, string title)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (IsWindowVisible(hWnd) && ClassName(hWnd) == className && Text(hWnd) == title) return hWnd;
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

	public static int SelectedIndex(IntPtr list)
	{
		return (int)SendMessage(list, 0x100C, new IntPtr(-1), new IntPtr(0x0002));
	}

	public static int TopIndex(IntPtr list)
	{
		return (int)SendMessage(list, 0x1013, IntPtr.Zero, IntPtr.Zero);
	}

	public static int ItemCount(IntPtr list)
	{
		return (int)SendMessage(list, 0x1004, IntPtr.Zero, IntPtr.Zero);
	}

	public static RECT VisibleRect(IntPtr hWnd)
	{
		RECT window;
		GetWindowRect(hWnd, out window);
		IntPtr rgn = CreateRectRgn(0, 0, 0, 0);
		int type = GetWindowRgn(hWnd, rgn);
		if (type == 0)
		{
			DeleteObject(rgn);
			return window;
		}
		RECT box;
		GetRgnBox(rgn, out box);
		DeleteObject(rgn);
		POINT topLeft = new POINT();
		topLeft.X = box.Left; topLeft.Y = box.Top;
		ClientToScreen(hWnd, ref topLeft);
		POINT bottomRight = new POINT();
		bottomRight.X = box.Right; bottomRight.Y = box.Bottom;
		ClientToScreen(hWnd, ref bottomRight);
		RECT visible;
		visible.Left = topLeft.X; visible.Top = topLeft.Y;
		visible.Right = bottomRight.X; visible.Bottom = bottomRight.Y;
		return visible;
	}
}
"@
}

[T5Native]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

function Find-TopWindow([int]$processId, [string]$title, [int]$tries = 40)
{
	for ($index = 0; $index -lt $tries; $index++)
	{
		$hWnd = [T5Native]::TopVisibleByTitle($processId, $title)
		if ($hWnd -ne [IntPtr]::Zero) { return $hWnd }
		Start-Sleep -Milliseconds 150
	}
	return [IntPtr]::Zero
}

function Get-Rect([IntPtr]$hWnd)
{
	$rect = New-Object T5Native+RECT
	[T5Native]::GetWindowRect($hWnd, [ref]$rect) | Out-Null
	return $rect
}

function Save-Window-Screen([IntPtr]$hWnd, [string]$path)
{
	[T5Native]::SetWindowPos($hWnd, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0040) | Out-Null
	[T5Native]::SetForegroundWindow($hWnd) | Out-Null
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
	finally
	{
		$graphics.Dispose()
		$bitmap.Dispose()
	}
	Note ("screenshot={0} size={1}x{2}" -f $path, $width, $height)
}

function Save-Full-Screen([string]$path)
{
	$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
	$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	try
	{
		$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
		$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
	}
	finally
	{
		$graphics.Dispose()
		$bitmap.Dispose()
	}
	Note ("screenshot-full={0} size={1}x{2}" -f $path, $bounds.Width, $bounds.Height)
}

function Save-Region-Screen($rect, [string]$path)
{
	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	if ($width -le 0 -or $height -le 0) { Save-Full-Screen $path; return }
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	try
	{
		$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
		$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
	}
	finally
	{
		$graphics.Dispose()
		$bitmap.Dispose()
	}
	Note ("screenshot-region={0} size={1}x{2}" -f $path, $width, $height)
}

function Focus-Window([IntPtr]$window, [IntPtr]$focusTarget)
{
	$processId = [uint32]0
	$targetThread = [T5Native]::GetWindowThreadProcessId($window, [ref]$processId)
	$currentThread = [T5Native]::GetCurrentThreadId()
	[T5Native]::AttachThreadInput($currentThread, $targetThread, $true) | Out-Null
	try
	{
		[T5Native]::SetForegroundWindow($window) | Out-Null
		[T5Native]::SetFocus($focusTarget) | Out-Null
	}
	finally
	{
		[T5Native]::AttachThreadInput($currentThread, $targetThread, $false) | Out-Null
	}
}

function Send-Search([IntPtr]$popup, [IntPtr]$edit, [string]$text)
{
	[T5Native]::SendMessageString($edit, 0x000C, [IntPtr]::Zero, $text) | Out-Null
	$command = [IntPtr]((0x0300 -shl 16) -bor 0x202)
	[T5Native]::SendMessage($popup, 0x0111, $command, $edit) | Out-Null
}

function Send-Key([IntPtr]$hWnd, [int]$key)
{
	[T5Native]::PostMessage($hWnd, 0x0100, [IntPtr]$key, [IntPtr]::Zero) | Out-Null
	[T5Native]::PostMessage($hWnd, 0x0101, [IntPtr]$key, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 180
}

function Click-Control([IntPtr]$parent, [int]$id)
{
	$control = [T5Native]::ChildById($parent, $id)
	if ($control -eq [IntPtr]::Zero) { throw "control $id was not found" }
	[T5Native]::PostMessage($control, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 350
}

function Get-ActivePage([IntPtr]$sheet)
{
	foreach ($hWnd in [T5Native]::Children($sheet))
	{
		if ([T5Native]::ClassName($hWnd) -eq '#32770' -and [T5Native]::IsWindowVisible($hWnd)) { return $hWnd }
	}
	return [IntPtr]::Zero
}

function Set-Page([IntPtr]$sheet, [int]$index)
{
	[T5Native]::SendMessage($sheet, 0x0465, [IntPtr]$index, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 350
	# TCM_SETCURSEL changes the page but does not notify the themed tab subclass;
	# force a repaint so screenshots show the real selected tab (mouse clicks
	# repaint via TCN_SELCHANGE).
	$tab = [T5Native]::ChildByClass($sheet, 'SysTabControl32')
	if ($tab -ne [IntPtr]::Zero)
	{
		[T5Native]::RedrawWindow($tab, [IntPtr]::Zero, [IntPtr]::Zero, 0x0001 -bor 0x0004 -bor 0x0100 -bor 0x0080) | Out-Null
	}
	Start-Sleep -Milliseconds 200
	return Get-ActivePage $sheet
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

function Get-GuiCounts([int]$processId)
{
	$handle = [T5Native]::OpenProcess(0x0400, $false, $processId)
	if ($handle -eq [IntPtr]::Zero) { return @{ Gdi = -1; User = -1 } }
	try
	{
		return @{ Gdi = [int][T5Native]::GetGuiResources($handle, 0); User = [int][T5Native]::GetGuiResources($handle, 1) }
	}
	finally
	{
		[T5Native]::CloseHandle($handle) | Out-Null
	}
}

function Set-HtmlClipboard
{
	$doc = '<html><body><!--StartFragment--><b>UI QA HTML clip</b><!--EndFragment--></body></html>'
	$headerTemplate = "Version:0.9`r`nStartHTML:{0:D10}`r`nEndHTML:{1:D10}`r`nStartFragment:{2:D10}`r`nEndFragment:{3:D10}`r`n"
	$probe = $headerTemplate -f 0, 0, 0, 0
	$startHtml = $probe.Length
	$startFragment = $startHtml + $doc.IndexOf('<!--StartFragment-->') + 20
	$endFragment = $startHtml + $doc.IndexOf('<!--EndFragment-->')
	$endHtml = $startHtml + $doc.Length
	$cf = ($headerTemplate -f $startHtml, $endHtml, $startFragment, $endFragment) + $doc
	[System.Windows.Forms.Clipboard]::SetData('HTML Format', $cf)
}

function Set-RtfClipboard
{
	[System.Windows.Forms.Clipboard]::SetData('Rich Text Format', '{\rtf1\ansi\deff0 {\fonttbl {\f0 Segoe UI;}}\f0\fs24 UI QA RTF clip\par}')
}

function Set-ImageClipboard
{
	$bitmap = New-Object System.Drawing.Bitmap 120, 80
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	try
	{
		$graphics.Clear([System.Drawing.Color]::FromArgb(48, 110, 200))
		$font = New-Object System.Drawing.Font('Segoe UI', 12)
		$brush = [System.Drawing.Brushes]::White
		$graphics.DrawString('UI QA', $font, $brush, 8, 20)
		$font.Dispose()
		[System.Windows.Forms.Clipboard]::SetImage($bitmap)
	}
	finally
	{
		$graphics.Dispose()
		$bitmap.Dispose()
	}
}

function Set-FileClipboard([string]$runDir)
{
	$file1 = Join-Path $runDir 'qa-file-one.txt'
	$file2 = Join-Path $runDir 'qa-file-two.txt'
	Set-Content -LiteralPath $file1 -Value 'UI QA file one'
	Set-Content -LiteralPath $file2 -Value 'UI QA file two'
	$files = New-Object System.Collections.Specialized.StringCollection
	[void]$files.Add($file1)
	[void]$files.Add($file2)
	[System.Windows.Forms.Clipboard]::SetFileDropList($files)
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

$run = Join-Path $env:TEMP ("ditto-ui-t5-" + $Prefix + "-" + [guid]::NewGuid().ToString('N'))
$dbPath = Join-Path $run 'Ditto.db'
$settingsPath = Join-Path $run 'Ditto.Settings'
$exe = Join-Path $run 'Ditto.exe'
$process = $null

Note ("BEGIN Theme={0} Language={1} Build={2} Run={3}" -f $Theme, $Language, $Build, $run)

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
LanguageFile=$Language
EnableDebugLogging=1
"@ | Set-Content -LiteralPath $settingsPath -Encoding Unicode

	# First start only creates the scratch database.
	$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds(20)
	while (!(Test-Path -LiteralPath $dbPath) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
	if (!(Test-Path -LiteralPath $dbPath)) { throw 'Portable test database was not created' }
	& $exe -exit | Out-Null
	$process.WaitForExit(8000) | Out-Null
	if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }

	if ($SeedGroup)
	{
		$python = "import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); db.execute('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(42,0,?,0,0,0,1,-1,NULL,0,0,0,0,0,0,0,0)',('Internationalization / very long group name for ellipsis QA',)); db.execute('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(43,0,?,0,0,0,1,-1,NULL,0,0,0,0,0,0,0,0)',('UI QA Group',)); db.commit(); db.close()"
		& python -c $python $dbPath
		if ($LASTEXITCODE -ne 0) { throw 'Failed to seed temporary test database' }
	}

	# Create a target notepad before Ditto opens so paste tests have somewhere to go.
	$notepad = $null
	try { $notepad = Start-Process notepad.exe -PassThru } catch { $notepad = $null }
	Start-Sleep -Milliseconds 500

	$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds(25)
	$main = [IntPtr]::Zero
	$mainValue = $null
	while ([DateTime]::UtcNow -lt $deadline)
	{
		$mainValue = Read-Setting $settingsPath 'MainhWnd'
		if ($mainValue)
		{
			$candidate = [long]$mainValue
			if ($candidate -gt 0 -and [T5Native]::IsWindow([IntPtr]$candidate))
			{
				$main = [IntPtr]$candidate
				break
			}
		}
		$candidate = [T5Native]::TopByTitle($process.Id, 'Ditto Clipboard Viewer')
		if ($candidate -eq [IntPtr]::Zero) { $candidate = [T5Native]::TopByClassPrefix($process.Id, 'Afx:') }
		if ($candidate -ne [IntPtr]::Zero) { $main = $candidate; break }
		Start-Sleep -Milliseconds 250
	}
	if ($main -eq [IntPtr]::Zero) { throw 'Main frame window was not found' }
	Note ("main={0} mainhWndSetting={1}" -f $main, $mainValue)
	Note ("top-level-windows={0}" -f [T5Native]::TopSummary($process.Id))

	Start-Sleep -Milliseconds 800
	Set-Clipboard -Value 'UI QA clip one: alpha'
	Start-Sleep -Milliseconds 250
	Set-Clipboard -Value 'UI QA clip two: beta'
	Start-Sleep -Milliseconds 250
	Set-Clipboard -Value 'https://example.com/ui-qa?theme=test'
	Start-Sleep -Milliseconds 250
	Set-Clipboard -Value 'UI QA unicode: 中文 検索テスト Ünïcode 🎨'
	Start-Sleep -Milliseconds 250
	Set-HtmlClipboard
	Start-Sleep -Milliseconds 350
	Set-RtfClipboard
	Start-Sleep -Milliseconds 350
	Set-ImageClipboard
	Start-Sleep -Milliseconds 500
	Set-FileClipboard $run
	Start-Sleep -Milliseconds 500
	Set-Clipboard -Value '#3366CC'
	Start-Sleep -Milliseconds 250
	# A single very long line exercises the horizontal overlay scrollbar.
	Set-Clipboard -Value ('UI QA long line ' + ('0123456789ABCDEF' * 40))
	Start-Sleep -Milliseconds 400
	for ($index = 0; $index -lt 40; $index++)
	{
		Set-Clipboard -Value ("UI QA row {0:D2} {1}" -f $index, [guid]::NewGuid().ToString('N').Substring(0, 8))
		Start-Sleep -Milliseconds 160
	}
	Start-Sleep -Seconds 1

	$countsBefore = Get-GuiCounts $process.Id
	Note ("gdi-before gdi={0} user={1}" -f $countsBefore.Gdi, $countsBefore.User)

	& $exe -open | Out-Null
	$popup = [IntPtr]::Zero
	for ($index = 0; $index -lt 40; $index++)
	{
		$popup = [T5Native]::TopVisibleByClass($process.Id, 'QPasteClass')
		if ($popup -ne [IntPtr]::Zero) { break }
		Start-Sleep -Milliseconds 150
	}
	if ($popup -eq [IntPtr]::Zero) { throw 'Quick paste popup was not found' }
	Start-Sleep -Milliseconds 700

	$list = [T5Native]::ChildById($popup, 0x201)
	$edit = [T5Native]::ChildById($popup, 0x202)
	$optionsButton = [T5Native]::ChildById($popup, 0x8025)
	$tree = [T5Native]::ChildByClass($popup, 'SysTreeView32')
	if ($list -eq [IntPtr]::Zero) { throw 'Quick paste list was not found' }
	if ($edit -eq [IntPtr]::Zero) { throw 'Quick paste search edit was not found' }

	$popupRect = Get-Rect $popup
	$listRect = Get-Rect $list
	$editRect = Get-Rect $edit
	$optionsRect = if ($optionsButton -ne [IntPtr]::Zero) { Get-Rect $optionsButton } else { $null }
	$treeRect = if ($tree -ne [IntPtr]::Zero) { Get-Rect $tree } else { $null }
	$dpi = [T5Native]::GetDpiForWindow($popup)
	$itemCount = [T5Native]::ItemCount($list)
	Note ("popup={0},{1},{2},{3} dpi={4} items={5} list={6},{7},{8},{9} edit={10},{11},{12},{13}" -f `
		$popupRect.Left, $popupRect.Top, $popupRect.Right, $popupRect.Bottom, $dpi, $itemCount, `
		$listRect.Left, $listRect.Top, $listRect.Right, $listRect.Bottom, `
		$editRect.Left, $editRect.Top, $editRect.Right, $editRect.Bottom)
	if ($optionsRect -ne $null)
	{
		Note ("options-button={0},{1},{2},{3}" -f $optionsRect.Left, $optionsRect.Top, $optionsRect.Right, $optionsRect.Bottom)
	}
	if ($treeRect -ne $null)
	{
		Note ("group-tree={0},{1},{2},{3}" -f $treeRect.Left, $treeRect.Top, $treeRect.Right, $treeRect.Bottom)
	}

	# Layout invariants: search above list, list above footer, all inside popup.
	$layoutPass = $true
	$listVisible = [T5Native]::VisibleRect($list)
	Note ("list-visible={0},{1},{2},{3}" -f $listVisible.Left, $listVisible.Top, $listVisible.Right, $listVisible.Bottom)
	if ($listVisible.Bottom -gt $editRect.Top + 1) { $layoutPass = $false; Note 'LAYOUT-FAIL visible list overlaps bottom search edit' }
	if ($optionsRect -ne $null -and $listVisible.Bottom -gt $optionsRect.Top + 1) { $layoutPass = $false; Note 'LAYOUT-FAIL visible list overlaps bottom options button' }
	if ($editRect.Bottom -gt $popupRect.Bottom + 1) { $layoutPass = $false; Note 'LAYOUT-FAIL search edit clipped at bottom' }
	if ($optionsRect -ne $null -and ($optionsRect.Bottom -gt $popupRect.Bottom + 1 -or $optionsRect.Right -gt $popupRect.Right + 1)) { $layoutPass = $false; Note 'LAYOUT-FAIL options button outside popup' }
	if ($editRect.Right -gt $popupRect.Right + 1) { $layoutPass = $false; Note 'LAYOUT-FAIL search edit clipped horizontally' }
	if ($treeRect -ne $null -and $treeRect.Right -gt $listRect.Left + 1) { $layoutPass = $false; Note 'LAYOUT-FAIL group tree overlaps list' }
	Note ("LAYOUT {0}" -f $(if ($layoutPass) { 'PASS' } else { 'FAIL' }))

	Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-popup.png" -f $Prefix))

	# Search interaction.
	Focus-Window $popup $edit
	Send-Search $popup $edit 'UI QA clip'
	Start-Sleep -Milliseconds 900
	Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-search.png" -f $Prefix))
	$searchTextLength = [T5Native]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()
	$searchSelected = [T5Native]::SelectedIndex($list)
	Note ("search textLength={0} selected={1}" -f $searchTextLength, $searchSelected)

	Send-Search $popup $edit 'zzzz-no-match-t5'
	Start-Sleep -Milliseconds 1000
	Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-empty.png" -f $Prefix))
	Note ("empty-state captured textLength={0}" -f ([T5Native]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()))

	Send-Search $popup $edit ''
	Start-Sleep -Milliseconds 900

	if ($SeedGroup)
	{
		[T5Native]::PostMessage($popup, 0x0510, [IntPtr]42, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 900
		Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-group.png" -f $Prefix))
		Note 'group view captured'
		[T5Native]::PostMessage($popup, 0x0510, [IntPtr](-1), [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 600
	}

	# Keyboard navigation and selection.
	Focus-Window $popup $list
	Send-Key $list 0x28
	Send-Key $list 0x28
	Send-Key $list 0x22
	Send-Key $list 0x23
	$navIndex = [T5Native]::SelectedIndex($list)
	$topIndex = [T5Native]::TopIndex($list)
	Note ("keyboard selected={0} top={1}" -f $navIndex, $topIndex)
	Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-nav.png" -f $Prefix))

	# Hover the right edge of the list to reveal the overlay scrollbar.
	$listRect = Get-Rect $list
	[T5Native]::SetCursorPos(($listRect.Right - [Math]::Max(4, [int]($dpi * 4 / 96))), ($listRect.Top + [int](($listRect.Bottom - $listRect.Top) / 2))) | Out-Null
	Start-Sleep -Milliseconds 900
	Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-scrollbar.png" -f $Prefix))

	# Description overlay (F3) / tooltip.
	Focus-Window $popup $list
	Send-Key $list 0x72
	Start-Sleep -Milliseconds 700
	Save-Window-Screen $popup (Join-Path $artifactDir ("after-{0}-description.png" -f $Prefix))
	Note 'description toggle captured (F3)'

	# Context menu incl. Add-Ins entry.
	Send-Key $list 0x5D
	Start-Sleep -Milliseconds 600
	$menu = [T5Native]::TopVisibleByClass($process.Id, '#32768')
	if ($menu -eq [IntPtr]::Zero)
	{
		$listRect = Get-Rect $list
		$x = [int](($listRect.Right - $listRect.Left) / 2)
		$y = [int](($listRect.Bottom - $listRect.Top) / 3)
		$point = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
		[T5Native]::PostMessage($list, 0x0204, [IntPtr]2, $point) | Out-Null
		[T5Native]::PostMessage($list, 0x0205, [IntPtr]2, $point) | Out-Null
		Start-Sleep -Milliseconds 800
		$menu = [T5Native]::TopVisibleByClass($process.Id, '#32768')
	}
	if ($menu -ne [IntPtr]::Zero)
	{
		$menuRect = Get-Rect $menu
		Save-Region-Screen $menuRect (Join-Path $artifactDir ("after-{0}-contextmenu.png" -f $Prefix))
		Note 'context menu captured'
		Send-Key $menu 0x1B
	}
	else
	{
		Save-Full-Screen (Join-Path $artifactDir ("after-{0}-contextmenu.png" -f $Prefix))
		Note 'UNVERIFIED context menu window not found; saved full screen'
		Send-Key $popup 0x1B
	}
	Start-Sleep -Milliseconds 400

	# Ensure popup is hidden before paste tests.
	[T5Native]::PostMessage($popup, 0x0100, [IntPtr]0x1B, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 350

	# -open / -close command line behavior.
	& $exe -open | Out-Null
	Start-Sleep -Milliseconds 700
	$visibleAfterOpen = [T5Native]::IsWindowVisible($popup)
	& $exe -close | Out-Null
	Start-Sleep -Milliseconds 700
	$visibleAfterClose = [T5Native]::IsWindowVisible($popup)
	Note ("-open visible={0} -close visible={1}" -f $visibleAfterOpen, $visibleAfterClose)

	# Global hotkey round trip (default Ctrl+`).
	if (-not $SkipHotkey)
	{
		[T5Native]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
		[T5Native]::keybd_event(0xC0, 0, 0, [UIntPtr]::Zero)
		[T5Native]::keybd_event(0xC0, 0, 2, [UIntPtr]::Zero)
		[T5Native]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 800
		$visibleAfterHotkey = [T5Native]::IsWindowVisible($popup)
		[T5Native]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
		[T5Native]::keybd_event(0xC0, 0, 0, [UIntPtr]::Zero)
		[T5Native]::keybd_event(0xC0, 0, 2, [UIntPtr]::Zero)
		[T5Native]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
		Start-Sleep -Milliseconds 800
		$visibleAfterHotkey2 = [T5Native]::IsWindowVisible($popup)
		Note ("hotkey visible={0} toggleVisible={1}" -f $visibleAfterHotkey, $visibleAfterHotkey2)
	}

	# Enter paste: clipboard should receive the selected clip and popup should hide.
	& $exe -open | Out-Null
	Start-Sleep -Milliseconds 800
	Focus-Window $popup $list
	Send-Key $list 0x28
	$pasteExpected = [T5Native]::SelectedIndex($list)
	[System.Windows.Forms.Clipboard]::Clear()
	Send-Key $list 0x0D
	Start-Sleep -Milliseconds 900
	$visibleAfterEnter = [T5Native]::IsWindowVisible($popup)
	$pasteText = ''
	try { $pasteText = [System.Windows.Forms.Clipboard]::GetText() } catch { $pasteText = '' }
	Note ("enter selected={0} visibleAfterEnter={1} clipboardLength={2}" -f $pasteExpected, $visibleAfterEnter, $pasteText.Length)

	if ($notepad -ne $null -and !$notepad.HasExited)
	{
		try { Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue } catch { }
	}

	# Open/close cycles for GDI stability.
	$countsBeforeCycles = Get-GuiCounts $process.Id
	Note ("gdi-before-cycles gdi={0} user={1}" -f $countsBeforeCycles.Gdi, $countsBeforeCycles.User)
	for ($index = 0; $index -lt 12; $index++)
	{
		& $exe -open | Out-Null
		Start-Sleep -Milliseconds 150
		& $exe -close | Out-Null
		Start-Sleep -Milliseconds 120
	}
	Start-Sleep -Milliseconds 500
	$countsAfter = Get-GuiCounts $process.Id
	Note ("gdi-after gdi={0} user={1} deltaGdi={2} deltaUser={3} cycleDeltaGdi={4} cycleDeltaUser={5}" -f `
		$countsAfter.Gdi, $countsAfter.User, ($countsAfter.Gdi - $countsBefore.Gdi), ($countsAfter.User - $countsBefore.User), `
		($countsAfter.Gdi - $countsBeforeCycles.Gdi), ($countsAfter.User - $countsBeforeCycles.User))

	if (-not $SkipOptions)
	{
		[T5Native]::PostMessage($main, 0x0111, [IntPtr]32771, [IntPtr]::Zero) | Out-Null
		$options = Find-TopWindow $process.Id 'Options'
		if ($options -eq [IntPtr]::Zero) { throw 'Options property sheet was not found' }
		Start-Sleep -Milliseconds 600
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-general.png" -f $Prefix))
		$generalPage = Get-ActivePage $options
		if ($generalPage -eq [IntPtr]::Zero) { $generalPage = $options }

		$keyboardPage = Set-Page $options 2
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-keyboard.png" -f $Prefix))
		$typesPage = Set-Page $options 1
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-types.png" -f $Prefix))
		Click-Control $typesPage 1015
		$addType = Find-TopWindow $process.Id 'Add Supported Type'
		if ($addType -ne [IntPtr]::Zero)
		{
			Save-Window-Screen $addType (Join-Path $artifactDir ("after-{0}-dialog-addtype.png" -f $Prefix))
			Click-Control $addType 2
			Note 'Add Supported Type opened and cancelled'
		}
		else { Note 'UNVERIFIED Add Supported Type not found' }

		$copyBuffersPage = Set-Page $options 3
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-copybuffers.png" -f $Prefix))
		Set-Page $options 5 | Out-Null
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-friends.png" -f $Prefix))
		Set-Page $options 6 | Out-Null
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-stats.png" -f $Prefix))
		Set-Page $options 7 | Out-Null
		Save-Window-Screen $options (Join-Path $artifactDir ("after-{0}-options-about.png" -f $Prefix))

		Set-Page $options 0 | Out-Null
		Click-Control $generalPage 2124
		$advanced = Find-TopWindow $process.Id 'Advanced Options'
		if ($advanced -ne [IntPtr]::Zero)
		{
			Save-Window-Screen $advanced (Join-Path $artifactDir ("after-{0}-dialog-advanced.png" -f $Prefix))
			Click-Control $advanced 1042
			$moveGroup = Find-TopWindow $process.Id 'Select group to reset clip order'
			if ($moveGroup -ne [IntPtr]::Zero)
			{
				Save-Window-Screen $moveGroup (Join-Path $artifactDir ("after-{0}-dialog-movetogroup.png" -f $Prefix))
				Click-Control $moveGroup 1040
				$groupName = [IntPtr]::Zero
				for ($index = 0; $index -lt 20; $index++)
				{
					$groupName = [T5Native]::TopVisibleByClassTitle($process.Id, '#32770', 'Ditto')
					if ($groupName -ne [IntPtr]::Zero) { break }
					Start-Sleep -Milliseconds 100
				}
				if ($groupName -ne [IntPtr]::Zero)
				{
					Save-Window-Screen $groupName (Join-Path $artifactDir ("after-{0}-dialog-groupname.png" -f $Prefix))
					Click-Control $groupName 2
					Note 'Group Name opened and cancelled'
				}
				else { Note 'UNVERIFIED Group Name not found' }
				Click-Control $moveGroup 2
			}
			else { Note 'UNVERIFIED Move to Group dialog not found' }
			Click-Control $advanced 2
		}
		else { Note 'UNVERIFIED Advanced Options dialog not found' }

		# Apply and OK must close and persist portable settings.
		Click-Control $options 12321
		Click-Control $options 1
		Start-Sleep -Milliseconds 800
		$optionsStillOpen = [T5Native]::IsWindowVisible($options)
		Note ("options apply/ok closed={0}" -f (!$optionsStillOpen))
	}

	$countsEnd = Get-GuiCounts $process.Id
	Note ("gdi-end gdi={0} user={1}" -f $countsEnd.Gdi, $countsEnd.User)

	& $exe -exit | Out-Null
	$process.WaitForExit(10000) | Out-Null
	$exited = $process.HasExited
	if (!$exited) { Stop-Process -Id $process.Id -Force }
	Get-Process Ditto -ErrorAction SilentlyContinue | Where-Object { $_.Path -like ($run + '*') } | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
	Note ("exit clean={0}" -f $exited)

	$logFile = Join-Path $run 'Ditto.log'
	if (Test-Path -LiteralPath $logFile)
	{
		$interesting = Select-String -LiteralPath $logFile -Pattern 'exception|assert|failed|error' -CaseSensitive:$false |
			Where-Object { $_.Line -notmatch 'ErrorSaving|Error loading data|error code 0|Ditto_Options_Quick_Paste' } |
			Select-Object -First 20
		if ($interesting)
		{
			foreach ($match in $interesting) { Note ("log: " + $match.Line) }
		}
		else { Note 'log-scan clean' }
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
