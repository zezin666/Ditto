# QA REPORT — T5 Integration + Full QA, T6/T7 re-integrations (Ditto UI Modernization)

Status: **T7 final integration PASS with known limitations.**
T6 integration commit: `b8eb52a`; T7 re-integration merge: `ec3d0f2` (T3 fix2 `3c4f7f6`).
T7 integration commit: `6f9e245` (code + QA evidence; the documentation update commit is one ahead).
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
| T2 follow-up | `taohy/ui-shell` | `43d89ee` | `0df7108` | none |
| T3 follow-up | `taohy/ui-list` | `1a60e6f` | `78367e6` | none |
| T3 fix2 (T7) | `taohy/ui-list` | `3c4f7f6` | `ec3d0f2` | none |

The T2/T3 follow-up merges (T6/T7 review fixes) are descendants of the earlier
merge bases and also merged without textual conflicts; the only files touched
by T5 across rounds were `src/QPasteWnd.cpp` and the language XML.

All merges were textual clean merges (`git merge --no-ff`, merge commits preserved).
A clean merge is not a semantic proof; runtime QA below exercises the seams.

## 2. Build

| Configuration | Command | Exit | Log | Errors | Warnings |
|---|---|---|---|---|---|
| NuGet restore | `nuget restore CP_Main_10.sln` | 0 | `docs/ui/t5-restore.log` | — | — |
| Release\|x64 (full) | `MSBuild CP_Main_10.sln -p:Configuration=Release -p:Platform=x64` | 0 | `docs/ui/t5-build-release.log` | 0 | 107 warning lines |
| Debug\|x64 (full) | `MSBuild CP_Main_10.sln -p:Configuration=Debug -p:Platform=x64` | 0 | `docs/ui/t5-build-debug.log` | 0 | 95 warning lines |
| Release\|x64 (final incremental, after fixes) | same | 0 | `docs/ui/t5-build-release-3.log` | 0 | 0 |
| Debug\|x64 (final incremental, after fixes) | same | 0 | `docs/ui/t5-build-debug-2.log` | 0 | 0 |
| Release\|x64 (T6 re-integration) | same | 0 | `docs/ui/t6-review-build-release.log` | 0 | 0 new (ToolTipEx C4244 gone) |
| Debug\|x64 (T6 re-integration) | same | 0 | `docs/ui/t6-review-build-debug.log` | 0 | 0 new (ToolTipEx C4244 gone) |
| Release\|x64 (T7 re-integration) | same | 0 | `docs/ui/t7-review-build-release.log` | 0 | 0 |
| Debug\|x64 (T7 re-integration) | same | 0 | `docs/ui/t7-review-build-debug.log` | 0 | 0 |

Warning triage (full build):
- Third-party / vendored: `sqlite3mc_amalgamation.c` (C4996), QRCode, zlib-adjacent code, `EncryptDecrypt\NewRandom.cpp`.
- Pre-existing first-party: `Misc.cpp`, `DrawHTML.C`, `Options.cpp`, `DeleteClipData.cpp`, `SendKeys.cpp`, `Client/Server`, etc.
- New UI code in the first integration round: only `src/ToolTipEx.cpp` lines 283 and 1649 (`C4244 double→int`, T3).
  T3's follow-up build removes both; the T6 re-integration builds have **no new UI warnings**.

Binaries (T7 re-integration):
- `Release64\Ditto.exe` 6,060,544 bytes (2026-11-11 04:35).
- `Debug64\Ditto.exe` 14,642,176 bytes (2026-11-11 04:36).
- Earlier hashes remain in the T5/T6 logs above.

**Defender note:** Windows Defender (threat `2147737394`, the known unsigned-Release false positive) blocked launching freshly copied `Release64\Ditto.exe` at 02:11–02:12 and currently holds the source file for scanning. Defender was **not** disabled or modified. Result:
- Release runtime is verified on the same integration code before the final two-line menu wiring, and on the final Release binary in the light pass at 02:09 (before the first block).
- Final **dark** and **German** runtime passes were executed with `Debug64`; **final-binary Release dark runtime is UNVERIFIED**.
- The same false positive still blocks freshly copied Release binaries (latest detections 2026-11-11 03:04-03:34). The T6 targeted and full runtime passes therefore use `Debug64`; Release\|x64 still builds with 0 errors.

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
| `NoGroup` | -No Group- |

T3, T4a and T4b declared no new strings. Other languages fall back to these English defaults (verified visually with Deutsch).

## 4. Integration fixes (only changes made by T5)

1. **T2 × T3 layout seam — native horizontal scrollbar exposed** (`src/QPasteWnd.cpp`, `CQPasteWnd::MoveControls`).
   T3 hides the native list scrollbars by moving the list window wider/taller than the window region that clips it. T2's new footer calculation replaced T3's `+ extraSize + 1` list extent with `bottomBarTop - topOfListBox`, so the native (light) horizontal scrollbar became visible in dark mode (captured before the fix).
   Fix: keep T2's footer top but restore T3's extent — visible height plus `extraSize + 1` when native scrollbars are hidden.
   Evidence: `artifacts/ui/after/after-dark-scrollbar.png` (before) vs `after-dark-debug-scrollbar.png` (after).

2. **Best-effort dark menus wired** (`src/QPasteWnd.cpp`).
   `CDarkMode::ApplyToMenus(GetSafeHwnd())` is now called before the two popup `TrackPopupMenu` calls (list right-click and system menu). It is a safe no-op when uxtheme dark mode is unsupported or the Windows app theme is light; command IDs and menu contents are unchanged.
   With Windows in **light** mode the native menus still render light (documented P0 best-effort behavior). Readability is preserved.

3. **Empty-state Enter was swallowed by the paste accelerator** (`src/QPasteWnd.cpp`).
   T2's fix makes the empty-state Clear handle `Space`/`Enter`, but the popup's
   `PreTranslateMessage`/`CheckActions` consumed `VK_RETURN` as the paste
   action before the focused static received it. `PreTranslateMessage` now
   lets `VK_RETURN` through when the empty-state static has focus.
   Verified before/after: `enter-clear textLength=16` -> `textLength=0`
   (dark and light), with focus confirmed by `GetGUIThreadInfo`.
   Owner note: this is a T2 behavior fix; upstream T2 should carry it.

## 4A. T6 review-fix verification (targeted)

Harness: `tools/ui-qa/run-t6-targeted-qa.ps1` with a 5,001-row scratch DB
(`rgba(37, 99, 235, 0.65)` color clip + group + 5,000 text rows).
Runs: `t6-dark` and `t6-light`, both `Debug64` at 144 DPI.
Evidence: `docs/ui/t6-review-runtime-evidence.log`,
`docs/ui/t6-review-runtime-{dark,light}.log`.

| Check | Result | Evidence |
|---|---|---|
| Footer/back/system/options/empty tab stops | **PASS** | `tabStops groups=True back=True system=True options=True empty=True list=True` |
| Tab traversal chain | **PASS** | `tabChain=513,514,528,32805,517,...` (list, search, system, options, groups; back/empty appear when visible). |
| Empty-state Space activates Clear | **PASS** | `space-clear textLength=0 items=5001` (dark and light). |
| Empty-state Enter activates Clear | **PASS after fix** | Before the integration fix Enter was consumed by the paste accelerator (`textLength=16`); after `QPasteWnd::PreTranslateMessage` lets Enter reach the focused static: `enter-clear textLength=0 items=5001` (dark and light). |
| Empty-state focus ring | **PASS** | `t6-{dark,light}-empty-focus.png`. |
| Footer focus ring | **PASS** | `t6-{dark,light}-footer-focus.png` (cog has focus ring). |
| High-contrast source path | **PASS (source)** | `CQPasteWnd::OnEraseBkgnd` and `CQPasteWnd::CtlColor` use `UiTheme().Canvas()` via the cached `UiDraw::Brush`; runtime HC remains UNVERIFIED. |
| 5k-row list, one Graphics per paint pass | **PASS** | Debug log marker `T3 list paint pass reused one Gdiplus::Graphics object` count=1 in both themes. |
| 5k-row GDI/user stability | **PASS** | 400 `WM_VSCROLL` messages in ~790 ms: delta GDI = -2, delta USER = +1, item count stays 5001. |
| Alpha brush cache creation + purge before shutdown | **PASS** | `T3 UiDraw created translucent brush cache` count=1 and `T3 UiDraw purged translucent brush cache` count=1, with `ExitInstance` present, both themes. |
| Dark menu calls | **PASS (source)** | `QPasteWnd.cpp` (list + system), `GroupTree.cpp`, `SymbolEdit.cpp`, `ToolTipEx.cpp` all call `CDarkMode::ApplyToMenus` before `TrackPopupMenu`. Windows-light + Ditto-dark menus still render light (best-effort). |
| T2/T3 seam | **PASS** | Region-based layout: `list-visible` bottom 1253 < search edit top 1257; no overlap/clipping. |
| Options/dialogs after re-merge | **PASS** | Full harness light/dark re-ran Options pages, Add Supported Type, Advanced Options, Move to Group, Group Name: open/close/Apply OK. |
| Log scan | **PASS** | No new exceptions; expected benign lines filtered. |

### Pre-existing list-view defect found during targeted QA (not a modernization regression)

Sending `LVM_SCROLL` (`0x1010`) to the 5,001-row owner-data list crashes the
process on the **first message** in both Release and Debug. Fault module
`COMCTL32.dll`, exception `0xc0000005` (WER APPCRASH). It reproduces on the
T3-only branch `1a60e6f` **and on the T1 foundation build `f6e5118`** (pre-T3
list), so it predates the UI modernization and is **not** a T2/T3/T5 merge
regression.

- User-facing scroll paths are stable: 400 x `WM_VSCROLL` and 200 x
  `WM_MOUSEWHEEL` complete without a crash and with stable GDI/user counts.
- Ditto itself never sends `LVM_SCROLL` (no source references), so practical
  exposure is limited to external automation/accessibility tools.
- Reproduction log: `docs/ui/t6-lvm-scroll-crash.log`.
- Owner: pre-existing base/upstream list-view behavior. Not fixed here.

## 4B. T7 re-review verification (targeted)

Merge: T3 fix2 `taohy/ui-list` @ `3c4f7f6` -> `ec3d0f2` (clean, no conflicts).
Harness: `tools/ui-qa/run-t7-targeted-qa.ps1` (120 indicator rows with unique
RGBA + group; three NoGroup language scenarios), plus a re-run of
`run-t6-targeted-qa.ps1` (5,001 rows) and the full integration harness on the
new build. Debug64 and Release64 both exercised.
Evidence: `docs/ui/t7-review-runtime-evidence.log`,
`docs/ui/t7-review-runtime-{dark,light,release-dark}.log`.

| Check | Result | Evidence |
|---|---|---|
| `NoGroup` key in English.xml | **PASS** | `<Item English_Text = "-No Group-" ID = "NoGroup"></Item>` added; only `Debug/Language/English.xml` changed, no existing ID touched. |
| Group tree root, English | **PASS** | Runtime root text `-No Group-` (Debug64 and Release64, light and dark). |
| Group tree root, localized | **PASS** | Temporary Deutsch.xml with `<Item ... ID="NoGroup">Keine Gruppe (QA)</Item>` -> root `Keine Gruppe (QA)`. |
| Group tree root, fallback | **PASS** | Real Deutsch.xml (no key) -> root `-No Group-` (code default). |
| Indicator rows with unique RGBA | **PASS** | 120 rows carrying `<noautodelete><shortcut><ingroup>` icons plus unique `rgba(r,g,b,a)` text; rendered in light/dark, `items=120`. |
| Shared paint-pass Graphics for indicators | **PASS** | Debug markers `T3 GdiImageDrawer shared Graphics draws: 1` and `: 100` in the main 120-row process; `CGdiImageDrawer::Draw(Gdiplus::Graphics&)` only calls `DrawImage` (no temp CDC/MemDC/Graphics), and `CQListCtrl` passes the paint-pass `*graphics`. |
| One Graphics per list paint pass | **PASS** | `T3 list paint pass reused one Gdiplus::Graphics object` (Debug). |
| Alpha brush cache bounded at 64 | **PASS** | `T3 UiDraw alpha brush cache cap reached; clearing` twice in the main process; cache re-created; `T3 UiDraw purged translucent brush cache` before exit. |
| GDI/user handles | **PASS** | 120-row/480 `WM_VSCROLL`: Debug 78->76 GDI, 70->71 USER; Release 78->76, 70->71. 5,001-row/400 messages: Debug 83->81, 71->72; Release 83->81, 71->72. |
| Debug direct-`WM_VSCROLL` abort | **PASS after T3 fix** | Root cause was a Debug-only `ASSERT(FALSE)` in `CQListCtrl::PreTranslateMessage` for `WM_VSCROLL`, removed in `3c4f7f6`. The Debug 5,000-row `WM_VSCROLL` harness now completes. It was a debug assertion/message-box block, not a Release/product failure. |
| T2/T3 seam | **PASS** | Region layout: visible list bottom 1207 < search edit top 1211; no clipping/overlap. |
| Footer/empty tab stops + chain | **PASS** | `groups/back/system/options/empty/list = True`; chain `513,514,528,32805,517,...`. |
| Empty-state Space and Enter | **PASS** | Both clear the search (`textLength=0`) and restore 5,001 rows. |
| List Enter-to-paste | **PASS** | Popup hides; clipboard receives 21 chars (full Release harness). |
| Dark menu call sites | **PASS (source)** | `QPasteWnd`, `GroupTree`, `SymbolEdit`, `ToolTipEx` call `CDarkMode::ApplyToMenus`; Windows-light + Ditto-dark menus remain light (best-effort). |
| Release runtime | **PASS this round** | T7 dark (120 indicators), 5,001-row harness and the full light harness all ran from `Release64\Ditto.exe`; Defender did not block these copies. |

Note: the group tree is created as a **top-level overlay window** (pre-existing
`CreateEx` without `WS_CHILD`), so its root text is read with `TVM_GETITEMW`;
this is pre-existing behavior, not a T2/T3/T7 regression.

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

T6 targeted captures (Debug64, 5,001 rows, 144 DPI):
`artifacts/ui/after/t6-{dark,light}-{popup,footer-focus,empty-focus,5k-list,color-swatch}.png`

T7 targeted captures (Debug64 and Release64, 120 indicator rows, 144 DPI):
`artifacts/ui/after/t7-{dark,light,release-dark}-{indicators-top,indicators-scrolled,group-tree,contextmenu}.png`

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
| Search input, clear, empty state | **PASS** | Text length 10, result selected; no-match text length 16; empty-state Clear activates on Space **and Enter**, leaving the search empty and 5,001 rows loaded. |
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
| High contrast | **UNVERIFIED (runtime)** | Not enabled. Source path verified: popup erase/CtlColor and themed controls use `UiTheme().Canvas()`; `CDarkMode::IsHighContrast()` guards native theming. |
| DPI 150 % (144 dpi) | **PASS** | 300×300 logical popup measured 450×450 physical; controls DPI-scaled; no clipping. |
| DPI 100 % / 200 % | **UNVERIFIED** | This host runs at 150 %. T2 exercised 96 dpi for the shell; T4a/T4b captured at 96/144. |

## 8. Special re-verification

- **T2 shell vs T3 overlay scrollbar:** PASS. The list window deliberately extends past its clip region; the *visible* region (queried with `GetWindowRgn`) ends at y=854, above the search edit at y=858 and the options button at y=863 (final Debug64 dark run). The earlier harness “list overlaps” lines were raw-window-rect measurements of the intentional clipped extension and are superseded by the region check (`probe-region.ps1`: visible bottom 804 < search top 807).
- **Dark horizontal scrollbar:** PASS after fix. The native white bar is gone; the custom overlay thumb uses the dark theme color. Captured `after-dark-debug-scrollbar.png`.
- **GDI / USER objects:** one-time growth on first popup open: 15→119 GDI, 27→103 USER (fonts, theme caches, scrollbar/overlay controls). Over 12 `-open`/`-close` cycles: **ΔGDI = 0, ΔUSER = 0** in every final run (one earlier Debug64 run measured −2/−4). No leak.
- **T4a/T4b after merge:** Options sheet, Advanced Options, Add Supported Type, Move to Group, Group Name, Friends, Stats, Copy Buffers, About all opened and closed; Apply/OK persisted to the portable settings; no crash.
- **T6 5k-row paint/cache:** PASS (see section 4A). One shared `Gdiplus::Graphics` per paint pass; translucent brush cache created once and purged before `GdiplusShutdown`; GDI/user counts stable over 400 scroll messages.
- **LVM_SCROLL crash:** FAIL on the T1 foundation (`f6e5118`) and T3-only builds too; pre-existing, not a T2/T3/T5 regression (see section 4A); user scroll paths pass.
- **Logs:** no new exceptions. Expected/benign lines: `Error finding the section Ditto_Options_Quick_Paste` (page intentionally disabled), `CF_UNICODETEXT ... NULL` for image-only test clips, and Debug-only harness artifacts (`OpenClipboard 失败`, `BAD DESCRIPTION` on synthetic clips).

## 9. Known limitations / UNVERIFIED

1. **Native menus and native list-view scrollbars stay light** when Ditto uses `Modern Dark` while the Windows app theme is light. This is the documented P0/P3 best-effort behavior (`CURRENT-STATE.md`, `DESIGN-SYSTEM.md §10`). Menu text remains readable and all commands are intact.
2. **Options OK/Cancel captions are OS/MFC-localized** (Chinese here) even with `LanguageFile=English`. Identical in T4a's own evidence; pre-existing.
3. `OptionsQuickPaste` and `OptionsUtilities` remain **disabled** in the property sheet (T0/P3 decision).
4. **Release runtime was verified in the T7 round** (120-indicator dark pass, 5,001-row harness and full light harness from `Release64\Ditto.exe`). Defender may still block future copies of the unsigned binary; it blocked earlier rounds. Do not disable Defender.
5. 100 %/200 % DPI, high contrast, tray menu, IME candidate window, UAC paste, drag/drop, copy buffers, multiselect, Shift+Enter, Ctrl+number, history/regex search and add-in invocation are **UNVERIFIED**.
6. The Deutsch empty state shows English T2 strings because only `English.xml` was updated, by design.
7. **LVM_SCROLL crash (pre-existing):** a programmatic `LVM_SCROLL` to a 5,001-row list crashes in `COMCTL32.dll`; reproduces on the T1 foundation `f6e5118` and T3-only `1a60e6f`, so it predates the UI work. User scrolling (wheel/WM_VSCROLL) is stable. See section 4A and `docs/ui/t6-lvm-scroll-crash.log`.
8. The group tree is a pre-existing top-level overlay window (missing `WS_CHILD`) rather than a popup child; the NoGroup check reads it via `TVM_GETITEMW`. Not changed by the UI modernization.
9. Group-tree and search-history menus were verified at source level only: the group pane is an on-demand overlay, and no search-history entry was present in the automated run.

## 10. Review entry points

- Branch/commits and build/run instructions: `docs/ui/REVIEW-GUIDE.md`.
- Machine-readable run logs: `docs/ui/t5-runtime-evidence.log`, `docs/ui/t6-review-runtime-evidence.log`, `docs/ui/t7-review-runtime-evidence.log`.
- Targeted harnesses: `tools/ui-qa/run-t6-targeted-qa.ps1` (tab/empty/5k) and `tools/ui-qa/run-t7-targeted-qa.ps1` (indicators/NoGroup/cache); LVM crash repro: `docs/ui/t6-lvm-scroll-crash.log`.
- Harness: `tools/ui-qa/run-integration-qa.ps1`, plus `probe-region.ps1` / `probe-windows.ps1` diagnostics.
