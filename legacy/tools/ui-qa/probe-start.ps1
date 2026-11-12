param([Parameter(Mandatory = $true)][string]$Build, [int]$Rows = 0)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$run = Join-Path $env:TEMP ("ditto-start-probe-" + $Build + "-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $run | Out-Null
Copy-Item (Join-Path $repo "$Build\Ditto.exe") $run
Copy-Item (Join-Path $repo "$Build\ICU_Loader.dll") $run
Copy-Item (Join-Path $repo "$Build\Addins") $run -Recurse
Copy-Item (Join-Path $repo 'Debug\Language') $run -Recurse
Copy-Item (Join-Path $repo 'Debug\Themes') $run -Recurse
New-Item -ItemType File (Join-Path $run 'portable') | Out-Null
$db = Join-Path $run 'Ditto.db'
@"
[Ditto]
Theme2=Modern Dark
DBPath=$db
LanguageFile=English
EnableDebugLogging=1
"@ | Set-Content (Join-Path $run 'Ditto.Settings') -Encoding Unicode
$exe = Join-Path $run 'Ditto.exe'
$p = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
Start-Sleep -Seconds 3
$dbCreated = Test-Path $db
if (!$p.HasExited -and $dbCreated -and $Rows -gt 0)
{
	& $exe -exit | Out-Null
	$p.WaitForExit(8000) | Out-Null
	if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }
	$python = "import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); n=int(sys.argv[2]); db.execute('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(100,0,?,0,0,0,0,-1,NULL,0,0,0,0,0,0,0,0)',('#3366CC',)); rows=[(1000+i,0,'T6 row %05d' % i,0,0,0,0,-1,None,0,0,0,0,0,0,0,0) for i in range(n)]; db.executemany('INSERT INTO Main(lID,lDate,mText,lShortCut,lDontAutoDelete,CRC,bIsGroup,lParentID,QuickPasteText,clipOrder,clipGroupOrder,globalShortCut,lastPasteDate,stickyClipOrder,stickyClipGroupOrder,MoveToGroupShortCut,GlobalMoveToGroupShortCut) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', rows); db.commit(); db.close()"
	& python -c $python $db $Rows
	$p = Start-Process -FilePath $exe -WorkingDirectory $run -PassThru
	Start-Sleep -Seconds 5
}
$aliveBeforeExit = !$p.HasExited
& $exe -exit | Out-Null
Start-Sleep -Seconds 2
if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }
Write-Output ("build=$Build rows=$Rows dbCreated=$dbCreated aliveBeforeExit=$aliveBeforeExit exited=$($p.HasExited) run=$run")
