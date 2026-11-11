param(
	[Parameter(Mandatory = $true)][ValidateSet('Modern Light', 'Modern Dark')][string]$Theme,
	[Parameter(Mandatory = $true)][string]$Prefix,
	[string]$Build = 'Debug64',
	[int]$Rows = 120
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$artifactDir = Join-Path $repo 'artifacts\ui\after'
$logPath = Join-Path $repo 'docs\ui\t7-review-runtime-evidence.log'
$runLogPath = Join-Path $repo ("docs\ui\t7-review-runtime-{0}.log" -f $Prefix)
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

if (-not ('T7Native' -as [type])) {
	Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class T7Native
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
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);
	[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint currentThread, uint targetThread, bool attach);
	[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
	[DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, int processId);
	[DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
	[DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
	[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
	[DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint threadId, ref GUITHREADINFO info);
	[DllImport("kernel32.dll")] public static extern IntPtr VirtualAllocEx(IntPtr process, IntPtr address, UIntPtr size, uint allocationType, uint protect);
	[DllImport("kernel32.dll")] public static extern bool VirtualFreeEx(IntPtr process, IntPtr address, UIntPtr size, uint freeType);
	[DllImport("kernel32.dll")] public static extern bool WriteProcessMemory(IntPtr process, IntPtr address, IntPtr buffer, UIntPtr size, out UIntPtr written);
	[DllImport("kernel32.dll")] public static extern bool ReadProcessMemory(IntPtr process, IntPtr address, IntPtr buffer, UIntPtr size, out UIntPtr read);

	[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
	[StructLayout(LayoutKind.Sequential)] public struct GUITHREADINFO
	{
		public int cbSize; public int flags; public IntPtr hwndActive; public IntPtr hwndFocus;
		public IntPtr hwndCapture; public IntPtr hwndMenuOwner; public IntPtr hwndMoveSize;
		public IntPtr hwndCaret; public RECT rcCaret;
	}
	[StructLayout(LayoutKind.Sequential)] public struct TVITEMW
	{
		public uint mask; public IntPtr hItem; public uint state; public uint stateMask;
		public IntPtr pszText; public int cchTextMax; public int iImage;
		public int iSelectedImage; public int cChildren; public IntPtr lParam;
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

	public static IntPtr FindTree(int processId)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (ClassName(hWnd) == "SysTreeView32") return hWnd;
		return IntPtr.Zero;
	}

	public static IntPtr MainWindow(int processId)
	{
		foreach (IntPtr hWnd in TopLevel(processId))
			if (ClassName(hWnd) == "Ditto" && Text(hWnd) == "Ditto") return hWnd;
		return IntPtr.Zero;
	}

	public static string TreeRootText(IntPtr tree, int processId)
	{
		IntPtr hItem = SendMessage(tree, 0x110A, IntPtr.Zero, IntPtr.Zero); // TVM_GETNEXTITEM / TVGN_ROOT
		if (hItem == IntPtr.Zero) return "";
		IntPtr process = OpenProcess(0x0008 | 0x0010 | 0x0020, false, processId);
		if (process == IntPtr.Zero) return "";
		int itemSize = Marshal.SizeOf(typeof(TVITEMW));
		int textBytes = 512 * 2;
		IntPtr remote = VirtualAllocEx(process, IntPtr.Zero, (UIntPtr)(itemSize + textBytes), 0x1000 | 0x2000, 0x04);
		IntPtr local = Marshal.AllocHGlobal(itemSize);
		try
		{
			if (remote == IntPtr.Zero) return "";
			TVITEMW item = new TVITEMW();
			item.mask = 0x0001; // TVIF_TEXT
			item.hItem = hItem;
			item.pszText = (IntPtr)((long)remote + itemSize);
			item.cchTextMax = 512;
			Marshal.StructureToPtr(item, local, false);
			UIntPtr written;
			WriteProcessMemory(process, remote, local, (UIntPtr)itemSize, out written);
			SendMessage(tree, 0x113E, IntPtr.Zero, remote); // TVM_GETITEMW
			IntPtr textLocal = Marshal.AllocHGlobal(textBytes);
			try
			{
				UIntPtr read;
				ReadProcessMemory(process, (IntPtr)((long)remote + itemSize), textLocal, (UIntPtr)textBytes, out read);
				return Marshal.PtrToStringUni(textLocal);
			}
			finally { Marshal.FreeHGlobal(textLocal); }
		}
		finally
		{
			Marshal.FreeHGlobal(local);
			if (remote != IntPtr.Zero) VirtualFreeEx(process, remote, UIntPtr.Zero, 0x8000);
			CloseHandle(process);
		}
	}
}
"@
}

[T7Native]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

function Get-Rect([IntPtr]$hWnd)
{
	$rect = New-Object T7Native+RECT
	[T7Native]::GetWindowRect($hWnd, [ref]$rect) | Out-Null
	return $rect
}

function Save-Window-Screen([IntPtr]$hWnd, [string]$path)
{
	[T7Native]::SetWindowPos($hWnd, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0040) | Out-Null
	[T7Native]::SetForegroundWindow($hWnd) | Out-Null
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

function Get-GuiCounts([int]$processId)
{
	$handle = [T7Native]::OpenProcess(0x0400, $false, $processId)
	if ($handle -eq [IntPtr]::Zero) { return @{ Gdi = -1; User = -1 } }
	try { return @{ Gdi = [int][T7Native]::GetGuiResources($handle, 0); User = [int][T7Native]::GetGuiResources($handle, 1) } }
	finally { [T7Native]::CloseHandle($handle) | Out-Null }
}

function Start-TestInstance([string]$run, [string]$exe, [string]$settings)
{
	$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds(30)
	$main = [IntPtr]::Zero
	while ([DateTime]::UtcNow -lt $deadline)
	{
		$main = [T7Native]::MainWindow($process.Id)
		if ($main -ne [IntPtr]::Zero) { break }
		Start-Sleep -Milliseconds 200
	}
	if ($main -eq [IntPtr]::Zero) { throw 'Main window was not found' }
	for ($attempt = 0; $attempt -lt 4; $attempt++)
	{
		[T7Native]::PostMessage($main, 0x04DE, [IntPtr]1, [IntPtr]::Zero) | Out-Null
		for ($index = 0; $index -lt 30; $index++)
		{
			$popup = [T7Native]::TopVisibleByClass($process.Id, 'QPasteClass')
			if ($popup -ne [IntPtr]::Zero) { return @{ Process = $process; Main = $main; Popup = $popup } }
			Start-Sleep -Milliseconds 100
		}
	}
	throw 'Quick paste popup was not found'
}

function Stop-TestInstance([string]$exe, $process, [string]$run)
{
	& $exe -exit | Out-Null
	$process.WaitForExit(10000) | Out-Null
	if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
	Get-Process Ditto -ErrorAction SilentlyContinue | Where-Object { $_.Path -like ($run + '*') } | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
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

$run = Join-Path $env:TEMP ("ditto-ui-t7-" + $Prefix + "-" + [guid]::NewGuid().ToString('N'))
$dbPath = Join-Path $run 'Ditto.db'
$settingsPath = Join-Path $run 'Ditto.Settings'
$languageDir = Join-Path $run 'Language'
$exe = Join-Path $run 'Ditto.exe'
$process = $null

Note ("BEGIN Theme={0} Build={1} Rows={2} Run={3}" -f $Theme, $Build, $Rows, $run)

try
{
	New-Item -ItemType Directory -Path $run | Out-Null
	Copy-Item (Join-Path $repo "$Build\Ditto.exe") $run
	Copy-Item (Join-Path $repo "$Build\ICU_Loader.dll") $run
	Copy-Item (Join-Path $repo "$Build\Addins") $run -Recurse
	Copy-Item (Join-Path $repo 'Debug\Language') $languageDir -Recurse
	Copy-Item (Join-Path $repo 'Debug\Themes') $run -Recurse
	New-Item -ItemType File -Path (Join-Path $run 'portable') | Out-Null

	@"
[Ditto]
Theme2=$Theme
DBPath=$dbPath
DBPath3=$dbPath
LanguageFile=English
EnableDebugLogging=1
"@ | Set-Content -LiteralPath $settingsPath -Encoding Unicode

	$process = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds(25)
	while (!(Test-Path -LiteralPath $dbPath) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
	if (!(Test-Path -LiteralPath $dbPath)) { throw 'Portable test database was not created' }
	Stop-TestInstance $exe $process $run

	$python = "import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); n=int(sys.argv[2]); db.execute('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(42,0,?,0,0,0,1,-1,NULL,0,0,0,0,0,0,0,0)',('T7 group',)); rows=[]; [rows.append((1000+i,0,'rgba(%d, %d, %d, 0.%02d)' % (i % 256, (i*7) % 256, (i*13) % 256, 10+(i % 89)),65,1,0,0,42,None,0,0,0,0,0,0,0,0)) for i in range(n)]; db.executemany('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', rows); db.commit(); db.close()"
	& python -c $python $dbPath $Rows
	if ($LASTEXITCODE -ne 0) { throw 'Failed to seed temporary test database' }

	# NoGroup localization checks: real English, customized English, Deutsch fallback.
	function Read-NoGroup([string]$settingsFile, [string]$languageFile)
	{
		# Always restore clean copies first.
		Copy-Item (Join-Path $repo 'Debug\Language\English.xml') (Join-Path $languageDir 'English.xml') -Force
		Copy-Item (Join-Path $repo 'Debug\Language\Deutsch.xml') (Join-Path $languageDir 'Deutsch.xml') -Force
		$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
		if ($languageFile -eq 'DeutschCustom')
		{
			# English.xml stores only English_Text (used for command/menu mapping);
			# GetString reads the element text. Add a German entry to prove the
			# NoGroup named ID is looked up and localized.
			$path = Join-Path $languageDir 'Deutsch.xml'
			$content = Get-Content -LiteralPath $path -Raw
			$content = $content.Replace("</Ditto_String_Table>",
				"`t`t<Item English_Text = ""-No Group-"" ID = ""NoGroup"">Keine Gruppe (QA)</Item>`r`n`t</Ditto_String_Table>")
			[System.IO.File]::WriteAllText($path, $content, $utf8NoBom)
			$lang = 'Deutsch'
		}
		elseif ($languageFile -eq 'Deutsch')
		{
			$lang = 'Deutsch'
		}
		else
		{
			$lang = 'English'
		}
		$settings = @"
[Ditto]
Theme2=$Theme
DBPath=$dbPath
DBPath3=$dbPath
LanguageFile=$lang
EnableDebugLogging=1
"@
		$settings | Set-Content -LiteralPath $settingsFile -Encoding Unicode
		$instance = Start-TestInstance $run $exe $settings
		$groupsButton = [T7Native]::ChildById($instance.Popup, 0x205)
		if ($groupsButton -ne [IntPtr]::Zero) { [T7Native]::PostMessage($groupsButton, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null }
		$tree = [IntPtr]::Zero
		for ($index = 0; $index -lt 50; $index++)
		{
			$tree = [T7Native]::FindTree($instance.Process.Id)
			if ($tree -ne [IntPtr]::Zero -and [T7Native]::IsWindowVisible($tree)) { break }
			Start-Sleep -Milliseconds 100
		}
		$text = ''
		if ($tree -ne [IntPtr]::Zero) { $text = [T7Native]::TreeRootText($tree, $instance.Process.Id) }
		else { Note ("nogroup tree not found for {0}" -f $languageFile) }
		Stop-TestInstance $exe $instance.Process $run
		Copy-Item (Join-Path $repo 'Debug\Language\English.xml') (Join-Path $languageDir 'English.xml') -Force
		return $text
	}

	$rootEnglish = Read-NoGroup $settingsPath 'English'
	Note ("nogroup-english=[{0}]" -f $rootEnglish)
	$rootCustom = Read-NoGroup $settingsPath 'DeutschCustom'
	Note ("nogroup-deutsch-custom=[{0}]" -f $rootCustom)
	$rootDeutsch = Read-NoGroup $settingsPath 'Deutsch'
	Note ("nogroup-deutsch-fallback=[{0}]" -f $rootDeutsch)

	# Indicator/cache run with the real English language file.
	Copy-Item (Join-Path $repo 'Debug\Language\English.xml') (Join-Path $languageDir 'English.xml') -Force
	@"
[Ditto]
Theme2=$Theme
DBPath=$dbPath
DBPath3=$dbPath
LanguageFile=English
EnableDebugLogging=1
"@ | Set-Content -LiteralPath $settingsPath -Encoding Unicode
	$instance = Start-TestInstance $run $exe $settingsPath
	$process = $instance.Process
	$popup = $instance.Popup
	$list = [T7Native]::ChildById($popup, 0x201)
	if ($list -eq [IntPtr]::Zero) { throw 'Quick paste list was not found' }
	$groupsButton = [T7Native]::ChildById($popup, 0x205)
	$loaded = 0
	for ($index = 0; $index -lt 100; $index++)
	{
		$loaded = [T7Native]::ItemCount($list)
		if ($loaded -gt 0) { break }
		Start-Sleep -Milliseconds 100
	}
	Note ("list-load-wait items={0}" -f $loaded)
	Start-Sleep -Milliseconds 700
	$tree = [T7Native]::FindTree($process.Id)
	$itemCount = [T7Native]::ItemCount($list)
	$dpi = [T7Native]::GetDpiForWindow($popup)
	Note ("popup-found dpi={0} listItems={1}" -f $dpi, $itemCount)
	if ($tree -ne [IntPtr]::Zero)
	{
		Note ("nogroup-runtime=[{0}]" -f [T7Native]::TreeRootText($tree, $process.Id))
	}
	else { Note 'UNVERIFIED group tree window not found' }

	$countsBefore = Get-GuiCounts $process.Id
	start-Sleep -Milliseconds 500
	Save-Window-Screen $popup (Join-Path $artifactDir ("t7-{0}-indicators-top.png" -f $Prefix))

	$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
	for ($index = 0; $index -lt 240; $index++)
	{
		[T7Native]::SendMessage($list, 0x0115, [IntPtr]1, [IntPtr]::Zero) | Out-Null
	}
	for ($index = 0; $index -lt 240; $index++)
	{
		[T7Native]::SendMessage($list, 0x0115, [IntPtr]0, [IntPtr]::Zero) | Out-Null
	}
	$stopwatch.Stop()
	Start-Sleep -Milliseconds 500
	$countsAfter = Get-GuiCounts $process.Id
	Note ("scroll480 ms={0} gdiBefore={1} gdiAfter={2} deltaGdi={3} userBefore={4} userAfter={5} deltaUser={6} items={7}" -f `
		$stopwatch.ElapsedMilliseconds, $countsBefore.Gdi, $countsAfter.Gdi, ($countsAfter.Gdi - $countsBefore.Gdi), `
		$countsBefore.User, $countsAfter.User, ($countsAfter.User - $countsBefore.User), ([T7Native]::ItemCount($list)))

	Save-Window-Screen $popup (Join-Path $artifactDir ("t7-{0}-indicators-scrolled.png" -f $Prefix))

	# Show the group pane, verify the localized root label, capture it.
	if ($groupsButton -ne [IntPtr]::Zero)
	{
		[T7Native]::PostMessage($groupsButton, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 900
		$tree = [T7Native]::FindTree($process.Id)
		if ($tree -ne [IntPtr]::Zero)
		{
			Note ("nogroup-runtime-after-show=[{0}] treeVisible={1}" -f [T7Native]::TreeRootText($tree, $process.Id), [T7Native]::IsWindowVisible($tree))
			if ([T7Native]::IsWindowVisible($tree))
			{
				$treeRect = Get-Rect $tree
				$bitmap = New-Object System.Drawing.Bitmap ($treeRect.Right - $treeRect.Left), ($treeRect.Bottom - $treeRect.Top)
				$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
				try
				{
					$graphics.CopyFromScreen($treeRect.Left, $treeRect.Top, 0, 0, $bitmap.Size)
					$bitmap.Save((Join-Path $artifactDir ("t7-{0}-group-tree.png" -f $Prefix)), [System.Drawing.Imaging.ImageFormat]::Png)
				}
				finally { $graphics.Dispose(); $bitmap.Dispose() }
				Note ("screenshot={0} size={1}x{2}" -f (Join-Path $artifactDir ("t7-{0}-group-tree.png" -f $Prefix)), ($treeRect.Right - $treeRect.Left), ($treeRect.Bottom - $treeRect.Top))
			}
		}
		else { Note 'UNVERIFIED group tree not found after show' }
		[T7Native]::PostMessage($groupsButton, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
		Start-Sleep -Milliseconds 500
	}

	# Selection + context menu smoke while indicator rows are visible.
	[T7Native]::PostMessage($list, 0x0100, [IntPtr]0x24, [IntPtr]::Zero) | Out-Null
	Start-Sleep -Milliseconds 300
	[T7Native]::PostMessage($list, 0x007B, $list, [IntPtr](-1)) | Out-Null
	Start-Sleep -Milliseconds 700
	$menu = [T7Native]::TopVisibleByClass($process.Id, '#32768')
	if ($menu -eq [IntPtr]::Zero)
	{
		$point = [IntPtr]0
		[T7Native]::PostMessage($list, 0x0204, [IntPtr]2, $point) | Out-Null
		[T7Native]::PostMessage($list, 0x0205, [IntPtr]2, $point) | Out-Null
		Start-Sleep -Milliseconds 700
		$menu = [T7Native]::TopVisibleByClass($process.Id, '#32768')
	}
	if ($menu -ne [IntPtr]::Zero)
	{
		Save-Region-Screen (Get-Rect $menu) (Join-Path $artifactDir ("t7-{0}-contextmenu.png" -f $Prefix))
		Note 'context menu captured'
		[T7Native]::PostMessage($menu, 0x0100, [IntPtr]0x1B, [IntPtr]::Zero) | Out-Null
	}
	else { Note 'UNVERIFIED context menu not found' }
	Start-Sleep -Milliseconds 400

	Stop-TestInstance $exe $process $run
	Note 'exit clean'

	$logFile = Join-Path $run 'Ditto.log'
	if (Test-Path -LiteralPath $logFile)
	{
		foreach ($pattern in @(
			'T3 list paint pass reused one Gdiplus::Graphics object',
			'T3 GdiImageDrawer shared Graphics draws: 1',
			'T3 GdiImageDrawer shared Graphics draws: 100',
			'T3 UiDraw created translucent brush cache',
			'T3 UiDraw alpha brush cache cap reached; clearing',
			'T3 UiDraw purged translucent brush cache',
			'ExitInstance'))
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
