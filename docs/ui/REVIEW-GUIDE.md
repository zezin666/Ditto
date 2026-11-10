# REVIEW GUIDE — Ditto UI Modernization (T5 integration + T6 re-integration)

This is the entry point for reviewing the integrated UI work. It describes the
branch, how to build and run it, where the evidence lives, and what is still
unverified.

## 1. Branch and merge history

- Integration branch: `taohy/ui-integration`
- Base: `taohy/ui-foundation` @ `f6e5118`
- Worktree used: `D:\orca\workspaces\Ditto\ui-integration`
- Not merged to `master`; not pushed.

Merge commits (newest first):

| Merge | Source | Commit |
|---|---|---|
| T4b dialogs | `taohy/ui-dialogs` @ `735e8b8` | `80f7abf` |
| T4a options core | `taohy/ui-options-core` @ `828f928` | `3c1d981` |
| T3 list/scrollbars | `taohy/ui-list` @ `5ff555e` | `3f07120` |
| T2 shell | `taohy/ui-shell` @ `6c0d2c4` | `0beb80b` |
| T3 follow-up (T6 fixes) | `taohy/ui-list` @ `1a60e6f` | `78367e6` |
| T2 follow-up (T6 fixes) | `taohy/ui-shell` @ `43d89ee` | `0df7108` |

T5 integration changes on top of the merges:

- `src/QPasteWnd.cpp` — restore T3's clipped native-scrollbar extent in T2's
  footer layout; wire best-effort dark native menus; let `VK_RETURN` reach the
  focused empty-state Clear button (T2's Enter path was consumed by the paste
  accelerator).
- `Debug/Language/English.xml` — the eight T2 string keys.
- `docs/ui/QA-REPORT.md`, `docs/ui/REVIEW-GUIDE.md`, `docs/ui/t5-*.log`,
  `docs/ui/t6-review-*.log`, `artifacts/ui/after/**`, `tools/ui-qa/**`.

## 2. Build

Toolchain used: MSBuild 18.11.1, MSVC 19.51, MFC v145, Windows SDK 26100.
Do not retarget the solution.

```bat
set MSBUILD=C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\MSBuild\Current\Bin\amd64\MSBuild.exe
nuget restore CP_Main_10.sln
"%MSBUILD%" CP_Main_10.sln -p:Configuration=Release -p:Platform=x64
"%MSBUILD%" CP_Main_10.sln -p:Configuration=Debug   -p:Platform=x64
```

Expected outputs: `Release64\Ditto.exe`, `Release64\ICU_Loader.dll`,
`Release64\Addins\DittoUtil.dll`, `Release64\focus.dll` (and `Debug64\...`).

Build evidence:

- `docs/ui/t5-restore.log`
- `docs/ui/t5-build-release.log` / `docs/ui/t5-build-release-3.log`
- `docs/ui/t5-build-debug.log` / `docs/ui/t5-build-debug-2.log`
- `docs/ui/t6-review-build-release.log` / `docs/ui/t6-review-build-debug.log`

Final builds: **0 errors**. The T6 re-integration builds add no new warnings;
T3's follow-up removed the two `C4244` warnings in `ToolTipEx.cpp`.

> Defender: the unsigned Release binary triggers the known
> `Trojan:Win32/Bearfoos.A!ml` false positive when copied to `%TEMP%`.
> Do not disable Defender. Use `Debug64` for runtime QA if Release is blocked
> and mark Release runtime unverified.

## 3. Run a portable QA instance

Never run against `%APPDATA%\Ditto`.

```powershell
$run = Join-Path $env:TEMP ("ditto-ui-review-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory $run | Out-Null
Copy-Item Debug64\Ditto.exe, Debug64\ICU_Loader.dll $run
Copy-Item Debug64\Addins $run -Recurse
Copy-Item Debug\Language $run -Recurse
Copy-Item Debug\Themes $run -Recurse
New-Item -ItemType File (Join-Path $run "portable") | Out-Null
@"
[Ditto]
Theme2=Modern Dark
DBPath=$run\Ditto.db
LanguageFile=English
EnableDebugLogging=1
"@ | Set-Content (Join-Path $run "Ditto.Settings") -Encoding Unicode
Start-Process (Join-Path $run "Ditto.exe") -WorkingDirectory $run
# copy something, then:
& (Join-Path $run "Ditto.exe") -open
# close with -close, quit with -exit
```

The full automated harness is `tools/ui-qa/run-integration-qa.ps1`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -STA -File tools/ui-qa/run-integration-qa.ps1 `
  -Theme "Modern Dark" -Prefix "dark" -Build "Debug64" -SeedGroup
```

It creates a scratch DB, seeds clips and a group, captures all screenshots,
measures window layout via `GetWindowRgn`, runs GDI cycles, opens Options and
the main dialogs, restores the HKCU Run value, and logs to
`docs/ui/t5-runtime-evidence.log`.

The T6 review-fix checks live in `tools/ui-qa/run-t6-targeted-qa.ps1`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -STA -File tools/ui-qa/run-t6-targeted-qa.ps1 `
  -Theme "Modern Dark" -Prefix "dark" -Build "Debug64" -Rows 5000
```

It seeds a 5,000-row DB plus an alpha-color clip, checks tab stops and the tab
chain, activates the empty-state Clear with Space and Enter, captures focus
rings, measures GDI/user handles across 400 `WM_VSCROLL` messages, verifies the
`T3 list paint pass reused one Gdiplus::Graphics object` and translucent-brush
cache create/purge markers, and logs to
`docs/ui/t6-review-runtime-evidence.log`.

## 4. What to review

1. `docs/ui/QA-REPORT.md` — status, merges, build, regression matrix,
   limitations.
2. Screenshots in `artifacts/ui/after/`:
   - `after-light-*` — Release, Modern Light, 150 % DPI.
   - `after-dark-debug-*` — Debug64, Modern Dark, 150 % DPI.
   - `after-de-light-*` — Deutsch, Modern Light.
   - `after-dark-*` — Release, Modern Dark (penultimate build; popup/list only).
   - `t6-{dark,light}-*` — T6 targeted checks: tab focus, empty-state focus
     ring, 5k list, alpha color swatch (Debug64, 5,001 rows).
3. Diff scope: `git diff f6e5118...HEAD -- src/`. Focus on:
   - `src/QPasteWnd.cpp::MoveControls` — the T2/T3 layout seam.
   - `src/Ui/**` — shared themed controls/dialogs (T4a).
   - `src/Theme.cpp` / `src/UiDraw.cpp` / `src/UiTheme.cpp` — T1 tokens.
4. Compatibility invariants: no schema, clipboard semantics, add-in ABI,
   settings-key, language-ID, command-ID or hotkey changes. No new runtime
   dependency; C++17/MFC/Win32/GDI/GDI+ only.

## 5. Known limitations to keep in mind

- Native context menus and native list-view scrollbars stay light when Ditto
  uses Modern Dark while the Windows app theme is light. Best-effort P0/P3
  behavior; text remains readable and commands are intact.
- Options OK/Cancel captions follow the OS/MFC locale on this host (Chinese),
  independent of `LanguageFile`; pre-existing.
- `OptionsQuickPaste` / `OptionsUtilities` stay out of the property sheet.
- Unverified: 100 %/200 % DPI, high contrast runtime, tray menu, IME, UAC,
  drag/drop, copy buffers, multiselect, Shift+Enter, Ctrl+number,
  history/regex search, add-in invocation, final-binary Release runtime
  (Defender block).
- **Pre-existing list-view defect:** programmatic `LVM_SCROLL` on a
  5,001-row list crashes in `COMCTL32.dll`; reproduces on the T1 foundation
  `f6e5118` and T3-only `1a60e6f`, so it is not a UI-modernization
  regression. User scrolling (`WM_VSCROLL`, mouse wheel) is stable. Repro:
  `docs/ui/t6-lvm-scroll-crash.log`.

## 6. Re-running the evidence

```powershell
# Light (Release)
powershell -File tools/ui-qa/run-integration-qa.ps1 -Theme "Modern Light" -Prefix light -SeedGroup
# Dark (Debug64 if Release is blocked)
powershell -File tools/ui-qa/run-integration-qa.ps1 -Theme "Modern Dark" -Prefix dark-debug -Build Debug64 -SeedGroup
# German
powershell -File tools/ui-qa/run-integration-qa.ps1 -Theme "Modern Light" -Prefix de-light -Build Debug64 -Language Deutsch -SeedGroup -SkipOptions
# T6 targeted checks (tab path, empty-state keyboard, 5k rows, cache markers)
powershell -File tools/ui-qa/run-t6-targeted-qa.ps1 -Theme "Modern Dark" -Prefix dark -Build Debug64 -Rows 5000
```

Each run appends to `docs/ui/t5-runtime-evidence.log` and overwrites the
matching `artifacts/ui/after/` screenshots. The harness restores the HKCU Run
value and kills only processes started from its own `%TEMP%` directory.
