param(
	[Parameter(Mandatory = $true)]
	[string]$Theme,
	[Parameter(Mandatory = $true)]
	[string]$Suffix,
	[ValidateSet("Release", "Debug")]
	[string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
$repo = "D:\orca\workspaces\Ditto\ui-shell"
$binaryDir = if ($Configuration -eq "Release") { "Release64" } else { "Debug64" }
$artifactDir = Join-Path $repo "artifacts\ui\t2"
$run = Join-Path $env:TEMP ("ditto-ui-t2-" + $Suffix + "-" + [guid]::NewGuid().ToString("N"))
$dbPath = Join-Path $run "Ditto.db"
$settingsPath = Join-Path $run "Ditto.Settings"
$exe = Join-Path $run "Ditto.exe"
$process = $null
$script:finished = $false

trap {
	Write-Output ("TRAP: " + $_.Exception.GetType().FullName + ": " + $_.Exception.Message)
	if ($process -ne $null -and !$process.HasExited -and $process.Path -like "$run\*") {
		Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
	}
	throw
}

function Save-WindowCapture {
	param(
		[IntPtr]$Window,
		[string]$Path
	)

	[T2Win32]::SetWindowPos($Window, [IntPtr](-1), 0, 0, 0, 0,
		0x0001 -bor 0x0002 -bor 0x0040) | Out-Null
	[T2Win32]::SetForegroundWindow($Window) | Out-Null
	Start-Sleep -Milliseconds 250
	$rect = New-Object T2RECT
	[T2Win32]::GetWindowRect($Window, [ref]$rect) | Out-Null
	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	try {
		$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
		$bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
	}
	finally {
		$graphics.Dispose()
		$bitmap.Dispose()
	}
}

function Send-SearchText {
	param(
		[IntPtr]$Edit,
		[IntPtr]$Popup,
		[string]$Text
	)

	[T2Win32]::SendMessageString($Edit, 0x000C, [IntPtr]::Zero, $Text) | Out-Null
	$command = [IntPtr]((0x0300 -shl 16) -bor 0x202)
	[T2Win32]::SendMessage($Popup, 0x0111, $command, $Edit) | Out-Null
}

function Focus-Control {
	param(
		[IntPtr]$Popup,
		[IntPtr]$Control
	)

	$processId = [uint32]0
	$targetThread = [T2Win32]::GetWindowThreadProcessId($Popup, [ref]$processId)
	$currentThread = [T2Win32]::GetCurrentThreadId()
	[T2Win32]::AttachThreadInput($currentThread, $targetThread, $true) | Out-Null
	try {
		[T2Win32]::SetForegroundWindow($Popup) | Out-Null
		[T2Win32]::SetFocus($Control) | Out-Null
	}
	finally {
		[T2Win32]::AttachThreadInput($currentThread, $targetThread, $false) | Out-Null
	}
}

function Focus-SearchEdit {
	param(
		[IntPtr]$Popup,
		[IntPtr]$Edit
	)

	Focus-Control $Popup $Edit
}

function Get-TabChain {
	param(
		[IntPtr]$Popup,
		[IntPtr]$Start,
		[int]$Maximum = 12
	)

	[System.Collections.Generic.List[int]]$ids = New-Object 'System.Collections.Generic.List[int]'
	[IntPtr]$current = $Start
	for ($i = 0; $i -lt $Maximum; $i++)
	{
		[IntPtr]$next = [T2Win32]::GetNextDlgTabItem($Popup, $current, $false)
		if ($next -eq [IntPtr]::Zero -or $next -eq $Start)
		{
			break
		}
		$ids.Add([T2Win32]::GetDlgCtrlID($next))
		$current = $next
	}

	return $ids
}

New-Item -ItemType Directory -Path $run | Out-Null
Copy-Item (Join-Path $repo "$binaryDir\Ditto.exe") $run
Copy-Item (Join-Path $repo "$binaryDir\ICU_Loader.dll") $run
Copy-Item (Join-Path $repo "$binaryDir\Addins") $run -Recurse
Copy-Item (Join-Path $repo "Debug\Language") $run -Recurse
Copy-Item (Join-Path $repo "Debug\Themes") $run -Recurse
New-Item -ItemType File -Path (Join-Path $run "portable") | Out-Null

@"
[Ditto]
Theme2=$Theme
DBPath=$dbPath
LanguageFile=English
"@ | Set-Content -LiteralPath $settingsPath -Encoding Unicode

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public struct T2RECT
{
	public int Left;
	public int Top;
	public int Right;
	public int Bottom;
}

public static class T2Win32
{
	public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

	[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
	[DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, System.Text.StringBuilder className, int maxCount);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out T2RECT rect);
	[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hwnd);
	[DllImport("user32.dll")] public static extern IntPtr GetNextDlgTabItem(IntPtr dialog, IntPtr control, bool previous);
	[DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr hwnd, int index);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
	[DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hwnd);
	[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint currentThread, uint targetThread, bool attach);
	[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr insertAfter,
		int x, int y, int width, int height, uint flags);
	[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
	[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
	[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);
	[DllImport("user32.dll", EntryPoint = "SendMessageW")] public static extern IntPtr SendMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageString(IntPtr hwnd, uint message, IntPtr wParam, string text);
	[DllImport("user32.dll", EntryPoint = "SendMessageW")] public static extern IntPtr GetEditSelection(IntPtr hwnd, uint message, ref int start, ref int end);
	[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
	[DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, int processId);
	[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
	[DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
	[DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);

	public static IntPtr FindTopLevel(int processId, string className)
	{
		IntPtr result = IntPtr.Zero;
		EnumWindows((hwnd, unused) =>
		{
			uint pid;
			GetWindowThreadProcessId(hwnd, out pid);
			if (pid != processId || !IsWindowVisible(hwnd))
				return true;

			var buffer = new System.Text.StringBuilder(128);
			GetClassName(hwnd, buffer, buffer.Capacity);
			if (buffer.ToString() == className)
			{
				result = hwnd;
				return false;
			}
			return true;
		}, IntPtr.Zero);
		return result;
	}

	public static IntPtr FindChildById(IntPtr parent, int id)
	{
		IntPtr result = IntPtr.Zero;
		EnumChildWindows(parent, (hwnd, unused) =>
		{
			if (GetDlgCtrlID(hwnd) == id)
			{
				result = hwnd;
				return false;
			}
			return true;
		}, IntPtr.Zero);
		return result;
	}
}
"@

[T2Win32]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while (!(Test-Path -LiteralPath $dbPath) -and [DateTime]::UtcNow -lt $deadline)
{
	Start-Sleep -Milliseconds 200
}

if (!(Test-Path -LiteralPath $dbPath))
{
	throw "Portable test database was not created"
}

& $exe -exit | Out-Null
$process.WaitForExit(5000) | Out-Null
if (!$process.HasExited)
{
	Stop-Process -Id $process.Id -Force
}

$python = @"
import sqlite3
import sys

db = sqlite3.connect(sys.argv[1])
db.execute(
    "INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,"
    "QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,"
    "stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) "
    "VALUES(42,0,?,0,0,0,1,-1,NULL,0,0,0,0,0,0,0,0)",
    ("Internationalization / \u56fd\u9645\u5316 / \u30ed\u30fc\u30ab\u30e9\u30a4\u30ba\u306e\u975e\u5e38\u306b\u9577\u3044\u30b0\u30eb\u30fc\u30d7\u540d",)
)
db.commit()
db.close()
"@
& python -c $python $dbPath
if ($LASTEXITCODE -ne 0)
{
	throw "Failed to seed temporary test database"
}

$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
Start-Sleep -Seconds 2

Set-Clipboard -Value "T2 portable $Suffix clip one"
Start-Sleep -Milliseconds 300
Set-Clipboard -Value "T2 portable $Suffix clip two"
Start-Sleep -Milliseconds 300
Set-Clipboard -Value "T2 portable $Suffix clip three"
Start-Sleep -Milliseconds 300

& $exe -open | Out-Null
Start-Sleep -Seconds 1

$popup = [T2Win32]::FindTopLevel($process.Id, "QPasteClass")
if ($popup -eq [IntPtr]::Zero)
{
	throw "Quick paste popup was not found"
}

$handle = [T2Win32]::OpenProcess(0x0400, $false, $process.Id)
$gdiBefore = if ($handle -ne [IntPtr]::Zero) { [T2Win32]::GetGuiResources($handle, 0) } else { 0 }
if ($handle -ne [IntPtr]::Zero)
{
	[T2Win32]::CloseHandle($handle) | Out-Null
}

Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-popup.png"))

$edit = [T2Win32]::FindChildById($popup, 0x202)
if ($edit -eq [IntPtr]::Zero)
{
	throw "Search edit was not found"
}

$list = [T2Win32]::FindChildById($popup, 0x201)
$groupButton = [T2Win32]::FindChildById($popup, 0x205)
$backButton = [T2Win32]::FindChildById($popup, 0x207)
$systemButton = [T2Win32]::FindChildById($popup, 0x210)
$optionsButton = [T2Win32]::FindChildById($popup, 0x8025)
$emptyAction = [T2Win32]::FindChildById($popup, 0x211)

$baseTabIds = Get-TabChain $popup $edit
$tabStopMask = 0x00010000
$groupTabStop = ([T2Win32]::GetWindowLongPtr($groupButton, -16).ToInt64() -band $tabStopMask) -ne 0
$backTabStop = ([T2Win32]::GetWindowLongPtr($backButton, -16).ToInt64() -band $tabStopMask) -ne 0
$systemTabStop = ([T2Win32]::GetWindowLongPtr($systemButton, -16).ToInt64() -band $tabStopMask) -ne 0
$optionsTabStop = ([T2Win32]::GetWindowLongPtr($optionsButton, -16).ToInt64() -band $tabStopMask) -ne 0

Focus-Control $popup $groupButton
Start-Sleep -Milliseconds 300
Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-review-footer-tab.png"))
Focus-Control $popup $edit

$editRect = New-Object T2RECT
[T2Win32]::GetWindowRect($edit, [ref]$editRect) | Out-Null
$editLocalX = [Math]::Floor(($editRect.Right - $editRect.Left) / 2)
$editLocalY = [Math]::Floor(($editRect.Bottom - $editRect.Top) / 2)
$editPoint = [IntPtr](($editLocalY -shl 16) -bor ($editLocalX -band 0xFFFF))
[T2Win32]::SendMessage($edit, 0x0201, [IntPtr]1, $editPoint) | Out-Null
[T2Win32]::SendMessage($edit, 0x0202, [IntPtr]::Zero, $editPoint) | Out-Null
Start-Sleep -Milliseconds 300
Send-SearchText $edit $popup "中文 検索テスト"
$focusTextLength = 0
for ($attempt = 0; $attempt -lt 4; $attempt++)
{
	Start-Sleep -Milliseconds 400
	$focusTextLength = [T2Win32]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()
	if ($focusTextLength -gt 0)
	{
		break
	}
	Focus-SearchEdit $popup $edit
	Send-SearchText $edit $popup "中文 検索テスト"
}
Focus-SearchEdit $popup $edit
Start-Sleep -Seconds 2
Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-search-focus.png"))
[T2Win32]::SendMessage($edit, 0x00B1, [IntPtr]::Zero, [IntPtr]2) | Out-Null
$selectionStart = 0
$selectionEnd = 0
[T2Win32]::GetEditSelection($edit, 0x00B0, [ref]$selectionStart, [ref]$selectionEnd) | Out-Null

Send-SearchText $edit $popup "zzzz-no-match-$Suffix"
$emptyTextLength = 0
for ($attempt = 0; $attempt -lt 4; $attempt++)
{
	Start-Sleep -Milliseconds 500
	$emptyTextLength = [T2Win32]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()
	if ($emptyTextLength -gt 0)
	{
		break
	}
	Send-SearchText $edit $popup "zzzz-no-match-$Suffix"
}
Focus-SearchEdit $popup $edit
Start-Sleep -Seconds 2
Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-empty.png"))

$emptyTabIds = Get-TabChain $popup $edit
$emptyTabStop = ([T2Win32]::GetWindowLongPtr($emptyAction, -16).ToInt64() -band $tabStopMask) -ne 0
Focus-Control $popup $emptyAction
Start-Sleep -Milliseconds 300
Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-review-empty-keyboard.png"))
[T2Win32]::PostMessage($emptyAction, 0x0100, [IntPtr]0x20, [IntPtr]::Zero) | Out-Null
[T2Win32]::PostMessage($emptyAction, 0x0101, [IntPtr]0x20, [IntPtr]::Zero) | Out-Null
Start-Sleep -Seconds 1
$lengthAfterClear = [T2Win32]::SendMessage($edit, 0x000E, [IntPtr]::Zero, [IntPtr]::Zero).ToInt32()

Send-SearchText $edit $popup ""
Start-Sleep -Seconds 1
[T2Win32]::PostMessage($popup, 0x0510, [IntPtr]42, [IntPtr]::Zero) | Out-Null
Start-Sleep -Seconds 2
Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-group.png"))

$groupTabIds = Get-TabChain $popup $edit
if ($backButton -ne [IntPtr]::Zero -and [T2Win32]::IsWindowVisible($backButton)) {
	Focus-Control $popup $backButton
	Start-Sleep -Milliseconds 300
	Save-WindowCapture $popup (Join-Path $artifactDir ("t2-$Suffix-review-back-tab.png"))
}

if ($list -ne [IntPtr]::Zero)
{
	foreach ($key in @(0x28, 0x22, 0x24, 0x26))
	{
		[T2Win32]::PostMessage($list, 0x0100, [IntPtr]$key, [IntPtr]::Zero) | Out-Null
		[T2Win32]::PostMessage($list, 0x0101, [IntPtr]$key, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 150
	}
}

if ($Suffix -eq "light")
{
	foreach ($caption in @(
		@{ Id = 32783; Name = "top" },
		@{ Id = 32781; Name = "bottom" },
		@{ Id = 32782; Name = "left" },
		@{ Id = 32780; Name = "right" }))
	{
		[T2Win32]::SendMessage($popup, 0x0111, [IntPtr]$caption.Id, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 400
		Save-WindowCapture $popup (Join-Path $artifactDir ("t2-light-caption-" + $caption.Name + ".png"))
	}
}

for ($i = 0; $i -lt 20; $i++)
{
	[T2Win32]::SetForegroundWindow($popup) | Out-Null
	Start-Sleep -Milliseconds 25
}

$handle = [T2Win32]::OpenProcess(0x0400, $false, $process.Id)
$gdiAfter = if ($handle -ne [IntPtr]::Zero) { [T2Win32]::GetGuiResources($handle, 0) } else { 0 }
if ($handle -ne [IntPtr]::Zero)
{
	[T2Win32]::CloseHandle($handle) | Out-Null
}

$windowRect = New-Object T2RECT
[T2Win32]::GetWindowRect($popup, [ref]$windowRect) | Out-Null
$editRect = New-Object T2RECT
[T2Win32]::GetWindowRect($edit, [ref]$editRect) | Out-Null
$options = [T2Win32]::FindChildById($popup, 0x8025)
$optionsRect = New-Object T2RECT
if ($options -ne [IntPtr]::Zero) {
	[T2Win32]::GetWindowRect($options, [ref]$optionsRect) | Out-Null
}
$dpi = [T2Win32]::GetDpiForWindow($popup)
[T2Win32]::PostMessage($popup, 0x0100, [IntPtr]0x1B, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 250
$visibleAfterEscape = [T2Win32]::IsWindowVisible($popup)
$summary = "config={0} theme={1} dpi={2} window={3}x{4} gdiBefore={5} gdiAfter={6} run={7}" -f `
	$Configuration, $Theme, $dpi, ($windowRect.Right - $windowRect.Left), `
	($windowRect.Bottom - $windowRect.Top), $gdiBefore, $gdiAfter, $run
$summary += " visibleAfterEscape={0}" -f $visibleAfterEscape
$summary += " edit={0},{1},{2},{3}" -f `
	$editRect.Left, $editRect.Top, $editRect.Right, $editRect.Bottom
$summary += " options={0},{1},{2},{3}" -f `
	$optionsRect.Left, $optionsRect.Top, $optionsRect.Right, $optionsRect.Bottom
$summary += " selection={0},{1}" -f $selectionStart, $selectionEnd
$summary += " clearTextLength={0}" -f $lengthAfterClear
$summary += " emptyTextLength={0}" -f $emptyTextLength
$summary += " focusTextLength={0}" -f $focusTextLength
$summary += " tabStops=group:{0},back:{1},system:{2},options:{3},empty:{4}" -f `
	$groupTabStop, $backTabStop, $systemTabStop, $optionsTabStop, $emptyTabStop
$summary += " baseTabIds={0}" -f ($baseTabIds -join ',')
$summary += " emptyTabIds={0}" -f ($emptyTabIds -join ',')
$summary += " groupTabIds={0}" -f ($groupTabIds -join ',')

& $exe -exit | Out-Null
$process.WaitForExit(5000) | Out-Null
if (!$process.HasExited)
{
	Stop-Process -Id $process.Id -Force
}

$script:finished = $true
Write-Output $summary
