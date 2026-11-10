# QA REPORT — T5 Integration + Full QA (Ditto UI Modernization)

Status: **Integration PASS with known limitations.**
Branch: `taohy/ui-integration` (worktree `D:\orca\workspaces\Ditto\ui-integration`).
Base: `taohy/ui-foundation` @ `f6e5118`.
No merge to `master`, no push.

Date: 2026-11-11
QA host: Windows 11 Home China 10.0.26200, system scaling **150 %** (`GetDpiForWindow` = 144),
Windows app theme = **light** during all runs.
Toolchain: MSBuild 18.11.1, MSVC 19.51.36260, MFC v145, Windows SDK 26100.0.

---

## 1. Merges (all clean, no conflicts)

| Order | Source branch | Source HEAD | Merge commit | Conflicts |
|---|---|---|---|---|
| T2 | `taohy/ui-shell` | `6c0d2c4` | `0beb80bcaf77f7934fd750d7f58df9b0cbb94cdd` | none |
| T3 | `taohy/ui-list` | `5ff555e` | `3f0712053d048b56c8ee87699eb6a9ec8d1ee36d` | none |
| T4a | `taohy/ui-options-core` | `828f928` | `3c1d9812f19a600e5d9407f01e954c97b7cfb3e4` | none |
| T4b | `taohy/ui-dialogs` | `735e8b8` | `80f7abf98a210aa830b20d2ed7d4791484b84239` | none |

All four merges were textual clean merges (`git merge --no-ff`, merge commits preserved).
A clean merge is not a semantic proof; runtime QA below exercises the seams.

## 2. Build

| Configuration | Command | Exit | Log | Errors | Warnings |
|---|---|---|---|---|---|
| NuGet restore | `nuget restore CP_Main_10.sln` | 0 | `docs/ui/t5-restore.log` | — | — |
| Release\|x64 (full) | `MSBuild CP_Main_10.sln -p:Configuration=Release -p:Platform=x64` | 0 | `docs/ui/t5-build-release.log` | 0 | 107 warning lines |
| Debug\|x64 (full) | `MSBuild CP_Main_10.sln -p:Configuration=Debug -p:Platform=x64` | 0 | `docs/ui/t5-build-debug.log` | 0 | 95 warning lines |
| Release\|x64 (final incremental, after fixes) | same | 0 | `docs/ui/t5-build-release-3.log` | 0 | 0 |
| Debug\|x64 (final incremental, after fixes) | same | 0 | `docs/ui/t5-build-debug-2.log` | 0 | 0 |

Warning triage (full build):
- Third-party / vendored: `sqlite3mc_amalgamation.c` (C4996), QRCode, zlib-adjacent code, `EncryptDecrypt\NewRandom.cpp`.
- Pre-existing first-party: `Misc.cpp`, `DrawHTML.C`, `Options.cpp`, `DeleteClipData.cpp`, `SendKeys.cpp`, `Client/Server`, etc.
- New UI code: only `src/ToolTipEx.cpp` lines 283 and 1649 (`C4244 double→int`, T3). No new warnings from `src/Ui/**`, `QPasteWnd.cpp`, `SymbolEdit.cpp`, or the theme layer.

Binaries:
- `Release64\Ditto.exe` 6,059,520 bytes (final build 2026-11-11 02:05).
- `Debug64\Ditto.exe` 14,633,472 bytes, SHA-256 `9c4eafc5c11d852a401a6d2de27838717a4103b7440e9bdd7b24af7b3141f5e1`.

**Defender note:** Windows Defender (threat `2147737394`, the known unsigned-Release false positive) blocked launching freshly copied `Release64\Ditto.exe` at 02:11–02:12 and currently holds the source file for scanning. Defender was **not** disabled or modified. Result:
- Release runtime is verified on the same integration code before the final two-line menu wiring, and on the final Release binary in the light pass at 02:09 (before the first block).
- Final **dark** and **German** runtime passes were executed with `Debug64`; **final-binary Release dark runtime is UNVERIFIED**.

## 3. New strings merged (English only)

`Debug/Language/English.xml` (only this language file was touched; no existing ID changed):

| Key | English default |
|---|---|
| `BackTooltip` | Back |
| `SystemMenuTooltip` | Menu |
| `OptionsTooltip` | Options |
| `NoResultsTitle` | No matching clips |
| `NoResultsHint` | Try a different search or clear it to see all clips. |
| `ClearSearchAction` | Clear search |
| `NoStarredClipsHint` | Star clips to keep them handy here. |
| `ClearFilterAction` | Clear filter |

T3, T4a and T4b declared no new strings. Other languages fall back to these English defaults (verified visually with Deutsch).

## 4. Integration fixes (only changes made by T5)

1. **T2 × T3 layout seam — native horizontal scrollbar exposed** (`src/QPasteWnd.cpp`, `CQPasteWnd::MoveControls`).
   T3 hides the native list scrollbars by moving the list window wider/taller than the window region that clips it. T2's new footer calculation replaced T3's `+ extraSize + 1` list extent with `bottomBarTop - topOfListBox`, so the native (light) horizontal scrollbar became visible in dark mode (captured before the fix).
   Fix: keep T2's footer top but restore T3's extent — visible height plus `extraSize + 1` when native scrollbars are hidden.
   Evidence: `artifacts/ui/after/after-dark-scrollbar.png` (before) vs `after-dark-debug-scrollbar.png` (after).

2. **Best-effort dark menus wired** (`src/QPasteWnd.cpp`).
   `CDarkMode::ApplyToMenus(GetSafeHwnd())` is now called before the two popup `TrackPopupMenu` calls (list right-click and system menu). It is a safe no-op when uxtheme dark mode is unsupported or the Windows app theme is light; command IDs and menu contents are unchanged.
   With Windows in **light** mode the native menus still render light (documented P0 best-effort behavior). Readability is preserved.

## 5. Portable run setup and data safety

- Every run used a fresh `%TEMP%\ditto-ui-t5-*` directory with `Ditto.exe`, `ICU_Loader.dll`, `Addins\`, `Debug\Language`, `Debug\Themes`, a `portable` marker, a scratch `Ditto.db` and a local `Ditto.Settings`.
- A real user Ditto instance was running from `D:\Programs\Ditto`. Its configuration has `SaveClipboard=0`, and `%APPDATA%\Ditto\Ditto.db` mtime stayed `2026-02-25 13:18:27` across all runs — QA clipboard traffic did **not** reach the user database.
- The QA harness snapshots and restores `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Ditto`. One Options Apply pass wrote the portable path during testing; it was restored. Final value: `D:\Programs\Ditto\Ditto.exe`.
- No leftover QA processes: only the user's `D:\Programs\Ditto\Ditto.exe` remains.
- Harness: `tools/ui-qa/run-integration-qa.ps1` (logs to `docs/ui/t5-runtime-evidence.log`).

## 6. Screenshots (real captures, not mockups)

Light (Release, final build, 450×450 px = 300×300 logical @ 150 %):
`artifacts/ui/after/after-light-{popup,search,empty,group,nav,scrollbar,description,contextmenu,options-general,options-keyboard,options-types,options-copybuffers,options-friends,options-stats,options-about,dialog-addtype,dialog-advanced,dialog-movetogroup,dialog-groupname}.png`

Dark (Debug64, final build, 450×450 px) — final run has `LAYOUT PASS`, Enter paste PASS, ΔGDI/ΔUSER = 0 over 12 open/close cycles:
`artifacts/ui/after/after-dark-debug-{popup,search,empty,group,nav,scrollbar,description,contextmenu,options-general,options-keyboard,options-types,options-copybuffers,options-friends,options-stats,options-about,dialog-addtype,dialog-advanced,dialog-movetogroup,dialog-groupname}.png`

German / Deutsch (Debug64, Modern Light, popup and list):
`artifacts/ui/after/after-de-light-{popup,search,empty,group,nav,scrollbar,description,contextmenu}.png`

Additional Release dark popup/list captures from the penultimate final build:
`artifacts/ui/after/after-dark-{popup,search,empty,group,nav,scrollbar,description,contextmenu}.png`

Observed:
- Dark/light hover, selection, search focus ring, empty state, group breadcrumb and ellipsis all render correctly.
- German labels (“Suchen”, long menu/tab labels) do not clip; new T2 empty-state strings intentionally fall back to English.
- Options OK/Cancel captions are OS/MFC property-sheet captions (Chinese on this host), reproduced identically in T4a's own artifacts — pre-existing, not an integration regression.

## 7. Regression checklist (§6.2)

| Item | Result | Evidence / note |
|---|---|---|
| Hotkey open / hide | **PASS** | Ctrl+` round trip: `visible=True`, second press `visible=False` (Release light/dark, Debug dark, Deutsch). |
| `-open` / `-close` | **PASS** | `visible=True` / `visible=False` in every run. |
| `-exit` | **PASS** | `exit clean=True`; no QA process left. |
| Search input, clear, empty state | **PASS** (clear button from T2 evidence) | Text length 10, result selected; no-match text length 16; T2 harness verified the clear action. |
| Search history / find-as-you-type / regex settings | **UNVERIFIED** | Not exercised by the T5 harness. |
| List Up/Down/PageUp/PageDown/Home/End | **PASS** | End/Home/PageUp/PageDown moved selection to 19, top index 1; captured. |
| Enter paste | **PASS** (Release) | Popup hides, clipboard receives the selected clip (21 chars), 3/3 Release runs. One Debug64 run hit a harness clipboard race (`OpenClipboard 失败`); the next Debug64 run passed. |
| Shift+Enter plain text | **UNVERIFIED** | Not exercised. |
| Ctrl+number quick paste | **UNVERIFIED** | Not exercised. |
| F3 description | **UNVERIFIED** | Key delivered; no description overlay appeared in this harness. T3 artifacts cover the description window separately. |
| Multi-select | **UNVERIFIED** | Not exercised. |
| Right-click menu / Add-Ins entry | **PASS (menu), UNVERIFIED (invoke)** | Full context menu captured, including `Add-Ins >`; command invocation not exercised. |
| Group navigation / breadcrumb / back | **PASS** | Seeded long-name group opened and captured. |
| Group create/rename/move/delete/pin | **UNVERIFIED** | `Move to Group` and `Group Name` dialogs opened and cancelled successfully. |
| Clipboard formats: text, Unicode, HTML, RTF, image, files, color | **PASS (capture)** | Seeded all seven; list populated (20 rows), no crash. Paste-back verified for text only. |
| Duplicate / de-duplication | **UNVERIFIED** | Not exercised. |
| Options: open, switch pages, Apply/OK, Cancel, theme | **PASS** | Pages 0/1/2/3/5/6/7 rendered; Apply+OK closed the sheet; Cancel closed dialogs; theme shown as Modern Light/Dark. |
| Options: language switch in UI, font change | **UNVERIFIED** | Language was set through portable settings (English/Deutsch); font not changed. |
| Tray icon / menu / connect / disconnect | **UNVERIFIED** | Tray menu not automated; Exit path covered by `-exit`. |
| Add-in invocation (PasteAnyAsText, ConvertPathToHtmlImageTag) | **UNVERIFIED** | Menu entry present; add-in command not invoked. |
| Drag-drop / copy buffers / UAC paste | **UNVERIFIED** | Not exercised. |
| Dark/light restart persistence | **PASS** | Separate processes with `Theme2=Modern Light` / `Modern Dark` loaded the expected theme. |
| Follow Windows theme | **UNVERIFIED** | Registry theme was not changed. |
| Language: English + Deutsch, no clipping | **PASS** | Light and Deutsch captures; no clipped labels. |
| High contrast | **UNVERIFIED** | Not enabled; code path reviewed only (`CDarkMode::IsHighContrast` / theme fallbacks). |
| DPI 150 % (144 dpi) | **PASS** | 300×300 logical popup measured 450×450 physical; controls DPI-scaled; no clipping. |
| DPI 100 % / 200 % | **UNVERIFIED** | This host runs at 150 %. T2 exercised 96 dpi for the shell; T4a/T4b captured at 96/144. |

## 8. Special re-verification

- **T2 shell vs T3 overlay scrollbar:** PASS. The list window deliberately extends past its clip region; the *visible* region (queried with `GetWindowRgn`) ends at y=854, above the search edit at y=858 and the options button at y=863 (final Debug64 dark run). The earlier harness “list overlaps” lines were raw-window-rect measurements of the intentional clipped extension and are superseded by the region check (`probe-region.ps1`: visible bottom 804 < search top 807).
- **Dark horizontal scrollbar:** PASS after fix. The native white bar is gone; the custom overlay thumb uses the dark theme color. Captured `after-dark-debug-scrollbar.png`.
- **GDI / USER objects:** one-time growth on first popup open: 15→119 GDI, 27→103 USER (fonts, theme caches, scrollbar/overlay controls). Over 12 `-open`/`-close` cycles: **ΔGDI = 0, ΔUSER = 0** in every final run (one earlier Debug64 run measured −2/−4). No leak.
- **T4a/T4b after merge:** Options sheet, Advanced Options, Add Supported Type, Move to Group, Group Name, Friends, Stats, Copy Buffers, About all opened and closed; Apply/OK persisted to the portable settings; no crash.
- **Logs:** no new exceptions. Expected/benign lines: `Error finding the section Ditto_Options_Quick_Paste` (page intentionally disabled), `CF_UNICODETEXT ... NULL` for image-only test clips, and Debug-only harness artifacts (`OpenClipboard 失败`, `BAD DESCRIPTION` on synthetic clips).

## 9. Known limitations / UNVERIFIED

1. **Native menus and native list-view scrollbars stay light** when Ditto uses `Modern Dark` while the Windows app theme is light. This is the documented P0/P3 best-effort behavior (`CURRENT-STATE.md`, `DESIGN-SYSTEM.md §10`). Menu text remains readable and all commands are intact.
2. **Options OK/Cancel captions are OS/MFC-localized** (Chinese here) even with `LanguageFile=English`. Identical in T4a's own evidence; pre-existing.
3. `OptionsQuickPaste` and `OptionsUtilities` remain **disabled** in the property sheet (T0/P3 decision).
4. **Final-binary Release dark runtime is UNVERIFIED** because Defender blocked the copied Release64 executable; Debug64 was used for the final dark and German passes. Release light runtime passed on the final build before the block.
5. 100 %/200 % DPI, high contrast, tray menu, IME candidate window, UAC paste, drag/drop, copy buffers, multiselect, Shift+Enter, Ctrl+number, history/regex search and add-in invocation are **UNVERIFIED**.
6. The Deutsch empty state shows English T2 strings because only `English.xml` was updated, by design.

## 10. Review entry points

- Branch/commits and build/run instructions: `docs/ui/REVIEW-GUIDE.md`.
- Machine-readable run log: `docs/ui/t5-runtime-evidence.log`.
- Harness: `tools/ui-qa/run-integration-qa.ps1`, plus `probe-region.ps1` / `probe-windows.ps1` diagnostics.
