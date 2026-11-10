# CURRENT-STATE — Ditto UI audit (T0 baseline)

- Date: 2026-11-10
- Worktree: `C:\Users\zezin\orca\workspaces\Ditto\ui-design`
- Branch: `taohy/ui-design`
- Base / HEAD: `a80fd3536bc407cac8cce43b5682022282ace983` (`master`)
- Method: source audit of the exact worktree. **No build, no run, no screenshots** — see `docs/ui/ENVIRONMENT.md` (`BLOCKED: build toolchain missing`).
- Every current-state claim below cites `file:line`. Contrast ratios are calculated from the cited RGB values, not measured on screen.

---

## 0. Executive summary

Ditto's UI is a native MFC/Win32/GDI application with a strong, functional quick-paste popup and a large set of stock MFC dialogs. It is already DPI-aware and partially theme-driven; `CTheme` supports 35 XML keys, and the modern scrollbar is a good GDI+ reference implementation. The modernization gap is not architectural — it is visual and systemic:

1. **Dark mode is only skin-deep.** The popup's own fills follow `CTheme`, but native controls, options pages, menus, and raster icons do not. The default dark theme (`DarkerDitto.xml`) is incomplete, so the scrollbar stays light, the search highlight stays pure red, and the pasted indicator stays neon green on black.
2. **The default light theme has severe contrast failures.** Caption text defaults to `RGB(191,191,191)` on a white/`RGB(204,204,204)` caption (~1.9:1), and the search focus border defaults to white on white (invisible focus).
3. **Icon assets are black rasters.** `Close_Black_*`, `Chevron*_Black_*`, and footer PNGs are black; on the black dark caption they disappear.
4. **The compact popup layout is hand-positioned** with unscaled magic numbers (`searchRowStart = 33`) and a documented 2px top-gap hack. Search is 25px high, footer buttons 24px, and the caption font is 19px — all below/above modern targets.
5. **The search box is a fully custom-painted `CEdit`.** It draws text manually; this is a high risk for IME composition, caret, selection, and accessibility. Any redesign must verify Chinese/Japanese IME before claiming success.
6. **Options and dialogs are stock control panels** at `8pt MS Shell Dlg`, with no dark/high-contrast path and no runtime layout pass for long localized strings.
7. **High contrast and UI Automation are absent** from the custom-drawn controls.

The target design system is in `docs/ui/DESIGN-SYSTEM.md`; the file-level task plan is in `docs/ui/IMPLEMENTATION-PLAN.md`; reference mockups are in `artifacts/ui/mockups/`.

---

## 1. UI surface map

| # | Surface | Class / file | Kind | Priority |
|---|---|---|---|---|
| 1 | Invisible main frame, tray icon, hotkeys, timers | `CMainFrame` / `src/MainFrm.cpp` | `CFrameWnd` (hidden) | P0 (tray/menu only) |
| 2 | Quick-paste popup shell | `CQPasteWnd` / `src/QPasteWnd.cpp` (~8.7k lines) | `CWndEx` (`WS_POPUP`) | P0 |
| 3 | Popup caption / title bar | `CDittoWindow` / `src/DittoWindow.cpp` | custom non-client paint | P0 |
| 4 | Search field | `CSymbolEdit` / `src/SymbolEdit.cpp` | `CEdit` subclass, custom paint | P0 |
| 5 | Clip list | `CQListCtrl` / `src/QListCtrl.cpp` | `CListCtrl` owner-data + custom draw | P0 |
| 6 | Modern scrollbars (V/H) | `CModernScrollBar` / `src/ModernScrollBar.cpp` | GDI+ overlay child window | P0 |
| 7 | Group tree | `CGroupTree` / `src/GroupTree.cpp` | native `SysTreeView32` | P0/P1 |
| 8 | Group breadcrumb / back button | `CGroupStatic`, `CGdipButton` in `QPasteWnd` | owner-draw | P1 |
| 9 | Bottom command bar | `CGdipButton` (groups, system menu) | owner-draw | P0 |
| 10 | Empty / no-results state | `CGroupStatic m_noSearchResultsStatic` | native static | P0 |
| 11 | Always-on-top warning strip | `CGroupStatic m_alwaysOnToWarningStatic` | native static (currently hidden) | P1 |
| 12 | Description / tooltip window | `CToolTipEx` / `src/ToolTipEx.cpp` | `WS_POPUP` + rich edit | P0/P2 |
| 13 | Options property sheet | `COptionsSheet` + 8 pages | `CPropertySheet` | P1 |
| 14 | Advanced options grid | `CAdvGeneral` / `src/AdvGeneral.cpp` | dialog | P1 |
| 15 | Script editor | `CScriptEditor` / `src/ScriptEditor.cpp` | dialog + rich edit | P1 (opened from AdvGeneral) |
| 16 | About / group name / move-to-group / copy properties / delete clip data / select DB / friends / copy buffers / stats / types / utilities / add type / progress / remote file | `src/*.cpp` + `IDD_*` | MFC dialogs | P2 |
| 17 | Clip editor | `CEditWnd`, `CEditFrameWnd` (`src/EditWnd.cpp`, `src/EditFrameWnd.cpp`) | frame + toolbar + rich edit | P2 |
| 18 | QR code viewer | `src/QRCodeViewer.cpp` | popup | P2 |
| 19 | Progress window | `CProgressWnd` / `src/ProgressWnd.cpp` | `WS_POPUP` | P2 |
| 20 | No-database fallback frame | `CNoDbFrameWnd` / `src/NoDbFrameWnd.cpp` | `CFrameWnd` | P2 |
| 21 | Tray context menu / popup menus | `CSystemTray`, `CMenu` | native Win32 menus | P0/P1 (readability) |
| 22 | Image viewer in description | `CImageViewer` / `src/ImageViewer.cpp` | child window | P2/P3 |

---

## 2. Quick-paste popup (`CQPasteWnd`)

### 2.1 Creation and window class

- `CWndEx::Create` registers class `"QPasteClass"` with `CS_DBLCLKS | CS_HREDRAW | CS_VREDRAW`, background `WHITE_BRUSH`, and creates `WS_POPUP` (`src/WndEx.cpp:73-93`). On dark themes the white class brush can flash before the first `WM_ERASEBKGND`.
- `CQPasteWnd::OnCreate` creates all children: search, header list, two modern scrollbars, native group tree, group/back/system buttons, group static, warning static, no-results static (`src/QPasteWnd.cpp:402-529`).
- Window title is the literal `"Ditto"` (`src/QPasteWnd.cpp:38,414`), updated to include localized status in `UpdateStatus` (`src/QPasteWnd.cpp:1392-1458`).
- Default size is **300×300** and default position **300,300** (`src/Options.cpp:969-992`); `ShowQPasteWnd` also falls back to 300×300 if the saved rect is invalid (`src/QuickPaste.cpp:214-222`). The window can be resized and the size is persisted unscaled via `CDPI::UnScale` (`src/QPasteWnd.cpp:972-982`).
- `OnEraseBkgnd` fills the client with `CTheme::MainWindowBG()` (`src/QPasteWnd.cpp:6729-6743`).
- `CQPasteWnd::CtlColor` sets `SetBkColor(RGB(255,0,0))` and returns `NULL_BRUSH` (`src/QPasteWnd.cpp:389-395`). Its purpose is unclear and it is a reflected `WM_CTLCOLOR` handler; treat as suspect before changing.

### 2.2 Caption / title bar (`CDittoWindow`)

- Default caption position is `CAPTION_RIGHT` (`src/Options.cpp:1304-1306`), i.e. a **vertical strip on the right**, not a top title bar. All four positions are supported: right/top/bottom/left (`src/DittoWindow.cpp:236-461`).
- Caption thickness comes from `CTheme::GetCaptionSize()`; theme default is **25** logical px (`src/Theme.cpp:70-71`), scaled via `m_dpi.Scale` (`src/DittoWindow.cpp:733-734`).
- Caption fonts are hard-coded `Segoe UI`: 19px default, weight 400 with `-900` escapement for vertical text, weight 500 horizontal (`src/DittoWindow.cpp:51-57`; recreated in `SetCaptionOn` at `719-727` and `OnDpiChanged` at `890-899`).
- Caption title color defaults to `RGB(191,191,191)` on caption fills `RGB(255,255,255)`/`RGB(204,204,204)` (`src/Theme.cpp:25-38`) — roughly **1.9:1** on white and **1.1:1** on the grey half. This is a WCAG failure in the default light theme.
- Non-client border is drawn as **2 passes of `Draw3dRect`** (`src/DittoWindow.cpp:253-260`), i.e. a 2px 3D frame. No DWM corner preference, no DWM border/caption color, no rounded corners, no native shadow.
- Caption fill is two solid rectangles (`CaptionLeft`/`CaptionRight`) with no gradient (`src/DittoWindow.cpp:403-410`).
- Buttons are 16×16 images placed in 32px slots (`src/DittoWindow.cpp:269-288`), loaded from black PNGs: `Close_Black_*` and `Chevron*_Black_*` (`src/DittoWindow.cpp:61-65`). On `DarkerDitto` (`CaptionLeft=RGB(0,0,0)`) these icons are black-on-black.
- Top-most / not-connected states replace the whole caption color via `Caption*TopMost` / `Caption*NotConnected` (`src/WndEx.cpp:125-148`), so status is conveyed by a full-bar color change (color-only, and jarring in dark mode).
- Hit testing has hard-coded `m_borderSize * 4` / `* 2` corner/side zones (`src/DittoWindow.cpp:118-150`); `m_borderSize` is 2 then scaled on DPI change (`src/DittoWindow.cpp:11,888`).
- The roll-up/chevron (`MinMaxWindow`) resizes the window to caption thickness (`src/DittoWindow.cpp:774-881`). Persistence, auto-expand, and hit-test behavior must be preserved.

### 2.3 Search field (`CSymbolEdit`)

- `CSymbolEdit` derives from `CEdit` (`src/SymbolEdit.h:35`) but **fully custom-paints its text** in `OnPaint`: clears with `GetSysColor(COLOR_WINDOW)`, then `FillSolidRect` with `SearchTextBoxFocusBG` or `MainWindowBG`, then `DrawText` of `GetWindowText` (`src/SymbolEdit.cpp:459-588`).
- Focus/text state is indicated by changing the whole field fill; the focus border is commented out (`src/SymbolEdit.cpp:508-527`). `OnNcPaint` draws a 1px frame from `SearchTextBoxFocusBorder` (`src/SymbolEdit.cpp:931-983`). The default `SearchTextBoxFocusBorder` is `RGB(255,255,255)` (`src/Theme.cpp:52`), so the default focus border is invisible on a white surface.
- Prompt text is drawn separately with `m_fontPrompt` (`src/SymbolEdit.cpp:534-547`); the search history drop-down and close button are owner-drawn 16px images (`src/SymbolEdit.cpp:549-574`, loaded at `836-840`).
- `SetSearchImages()` in `QPasteWnd` is entirely commented out (`src/QPasteWnd.cpp:624-651`), so the old DPI-specific bitmap selection is dead code; icons are loaded by `CSymbolEdit` itself.
- The search field is positioned at the **bottom** of the popup: `MoveWindow(Scale(34), cy - Scale(searchRowStart - 5), cx - Scale(70), Scale(25))` with `searchRowStart = 33` (`src/QPasteWnd.cpp:693,769`). It is 25px high, not DPI-independent in its row offset (the constant 33 is unscaled).
- Search behavior is extensive and must be preserved: past searches (`LoadPastSearches`), autocomplete (`SHAutoComplete`), prompt text, find-as-you-type, regex, last-search accelerator, F3 description search, and clear button (`src/QPasteWnd.cpp:416-421,531-622,3046-3584`).
- **IME/caret risk:** manually drawing `GetWindowText` bypasses the native edit's composition rendering and caret placement. Chinese/Japanese IME, selection, horizontal scrolling, and caret blink must be tested before accepting any redesign. See `IMPLEMENTATION-PLAN.md` T2 risk R-T2-1.

### 2.4 Clip list (`CQListCtrl`)

- `CListCtrl` with `LVS_OWNERDATA | LVS_OWNERDRAWFIXED | LVS_REPORT | LVS_NOCOLUMNHEADER`, one column (`src/QPasteWnd.cpp:430,491`).
- Row height: initial `m_rowHeight = 50` (`src/QListCtrl.cpp:211`); `MeasureItem` computes `(tmHeight + tmExternalLeading) * m_linesPerRow + Scale(ROW_BOTTOM_BORDER)` using a screen DC and the current font (`src/QListCtrl.cpp:414-428`). `ROW_BOTTOM_BORDER = 4`, `ROW_LEFT_BORDER = 3` (`src/QListCtrl.cpp:27-28`). 1–5 lines per row are supported by settings (`src/QPasteWnd.cpp:2096-2155`).
- Custom draw paints everything and returns `CDRF_SKIPDEFAULT` (`src/QListCtrl.cpp:430-658`):
  - Zebra rows use `ListBoxOddRowsBG` / `ListBoxEvenRowsBG` with flat `FillSolidRect` (`src/QListCtrl.cpp:497-511`).
  - Selection uses `ListBoxSelectedBG` / `ListBoxSelectedNoFocusBG` (`src/QListCtrl.cpp:483-495`).
  - Pasted indicator is a **2px vertical `ClipPastedColor` strip** at the left, default `RGB(0,255,0)` (`src/QListCtrl.cpp:519-528`; `src/Theme.cpp:47`). Color-only, neon.
  - First-ten hotkey index is a small digit plus a 1px vertical divider (`src/QListCtrl.cpp:610-647`); the font is created from `FirstTenHotKeysFontSize` (default 5) with face name `"Small Font"` and `FW_LIGHT` (`src/QListCtrl.cpp:2293-2313`).
  - Status icons (group, no-autodelete, shortcut, in-group, sticky) are drawn as PNG images and advance the text cursor (`src/QListCtrl.cpp:558-589`).
  - Search matches are highlighted by injecting an HTML color span and using `DrawHTML` (`src/QListCtrl.cpp:591-604`); highlight color defaults to pure red (`src/Theme.cpp:53`).
  - There is **no hover state** and no rounded row/selection shape. The focus rectangle is commented out (`src/QListCtrl.cpp:606-608`), so keyboard focus is only visible through selection color.
- Bitmap/RTF/colour rendering helpers exist: `DrawBitMap` (`src/QListCtrl.cpp:1169+`), `DrawRtfText` (`1120+`), `DrawCheckerboard` (`799+`), colour-code drawing (`DrawCopiedColorCode`, declared `src/QListCtrl.h:210`).
- User clip font comes from `DisplayFont6`, default `Segoe UI` (`src/Options.cpp:1495-1547`); it is DPI-scaled in `CQPasteWnd::UpdateFont` (`src/QPasteWnd.cpp:2206-2218`). This is a user setting and must not be overridden by the design system.

### 2.5 Modern scrollbars (`CModernScrollBar`)

- Overlay child window created over the list; native scrollbar is hidden by a window region when modern scrollbars are enabled (`src/QPasteWnd.cpp:438-454,703-725`).
- GDI+ anti-aliased rounded thumb: `DrawRoundedRect` builds a `GraphicsPath` and fills it (`src/ModernScrollBar.cpp:245-282`); `OnPaint` double-buffers into a compatible bitmap (`src/ModernScrollBar.cpp:284-322`).
- Colors are `ScrollBarTrack`/`ScrollBarThumb`/`ScrollBarThumbHover` from `CTheme` (`src/Theme.cpp:65-68`), pushed via `SetColors` at creation and refreshed by `RefreshScrollBarColors` (`src/QPasteWnd.cpp:8108-8120`).
- Auto-hide timer is 800ms (`src/ModernScrollBar.cpp:384,555`); hover widens the thumb (logic around `src/ModernScrollBar.cpp:330-390`).
- `DarkerDitto.xml` does not define the scrollbar keys, so dark mode falls back to **light** scrollbar colors (`RGB(240,240,240)` track / `RGB(180,180,180)` thumb).
- This is the best existing reference for GDI+ drawing, but it creates GDI+ objects per paint (acceptable for one scrollbar, not for every list row).

### 2.6 Group tree and breadcrumb

- `CGroupTree` is a native `SysTreeView32` with `TVS_HASLINES | TVS_LINESATROOT | TVS_HASBUTTONS` (`src/QPasteWnd.cpp:456`). It is shown/hidden as an overlay, not a permanent pane.
- `FillTree` only sets `SetBkColor(GroupTreeBG)` and `SetTextColor(GroupTreeText)` (`src/GroupTree.cpp:176-179`). There is no dark-mode item theming, no hover state, no selected-row token, and no tooltip for long group names.
- The image list is `18×16` from `IDB_CLOSED_FOLDER`, using `RGB(255,0,0)` as the mask (`src/GroupTree.cpp:67-78`). It is not DPI-scaled.
- The root item text is the hard-coded English `"-No Group-"` (`src/GroupTree.cpp:184`) — a localization gap.
- Breadcrumb mode (inside a group) uses a back button and `CGroupStatic` title: back button 16×16 at `(2,2)`, title at `(24,2)` with a 20px top offset for the list (`src/QPasteWnd.cpp:677-691`).
- Keyboard/context behavior must be preserved: F7/Ctrl+F7 new group, Delete, Alt+Enter properties, Enter/Esc, right-click menu (`src/GroupTree.cpp:80-82,278-300,351-377`).

### 2.7 Bottom command bar

- There is no separate footer: the bottom row contains the group button, the search field, and the system menu button.
- Positions are fixed: group button `(4, cy-28, 24, 24)`, search `(34, cy-28, cx-70, 25)`, system menu `(cx-30, cy-28, 24, 24)` (`src/QPasteWnd.cpp:769-773`). All are 24/25px high — below the modern 28px hit-target target.
- Icons are black PNGs: `open_folder_*`, `system_menu_2_*` (`src/QPasteWnd.cpp:465,476`), with the same dark-mode invisibility risk as the caption icons.
- The always-on-top warning strip is currently hidden by commented-out logic (`src/QPasteWnd.cpp:775-784`).

### 2.8 Empty / no-results state

- `m_noSearchResultsStatic` is a plain `CStatic` created with the always-on-top text and then updated with `NoSearchResults` localization (`src/QPasteWnd.cpp:499-515,6415-6435`).
- It is shown only when `m_noSearchResults && (search non-empty || starred view)`; it is positioned with a 10px border (`src/QPasteWnd.cpp:727-737`).
- Its font is the 15px search font and its colors are `MainWindowBG` + `ListBoxEvenRowsText` (`src/QPasteWnd.cpp:2226-2228`). There is no icon, no hint, and no "clear search" action.
- Localized text: `NoSearchResults` = "There are no results for" (`Debug/Language/English.xml:435`). The query is appended in quotes.

### 2.9 Description / tooltip window (`CToolTipEx`)

- Custom `WS_POPUP` with `CDittoWindow` caption, a rich edit, an options button, clip-data static, folder-path static, and an image viewer (`src/ToolTipEx.cpp:90-160`).
- Fonts hard-coded: 11px clip data, 13px body (`src/ToolTipEx.cpp:136-138`), recreated on DPI change (`1485-1504`).
- `OnPaint` only fills the client with `DescriptionWindowBG` (`src/ToolTipEx.cpp:1305-1321`); there is no rounded card, border, shadow, or padding token. Child controls cover most of it.
- Preserve rich text / HTML / image / search / pin / word-wrap / auto-hide behavior (`src/ToolTipEx.cpp:1323-1387` and the `m_RichEdit` event handling).

### 2.10 Menus

- Popup menus are native `CMenu` resources: `IDR_MENU`, `IDR_QUICK_PASTE`, `IDR_MENU_SEARCH`, `IDR_DESC_OPTIONS_MENU`, `IDR_MENU_GROUPS`, `IDR_QUICK_PASTE_SYSTEM_MENU`, `IDR_MENU_NO_DB`, `IDR_MENU_DELETE_CLIP_DATA` (`CP_Main.rc:422-720`).
- `CWndEx::OnInitMenuPopupEx` only updates shortcuts; no owner-draw or dark mode (`src/WndEx.cpp:297-300`).
- Tray menu is `IDR_MENU` via `CSystemTray` (`src/SystemTray.cpp`); the tray icon itself is OS-managed.
- The app does not call `SetWindowTheme`, `SetPreferredAppMode`, or any DWM attribute today (no hits for `DwmSetWindowAttribute`/`SetWindowTheme` in `src/`). Dark native menus are therefore whatever the OS gives a non-dark-aware process (usually light, or partially dark on Win11).

---

## 3. Options and dialogs

### 3.1 Options property sheet

- `COptionsSheet` is a stock `CPropertySheet` with `EnableStackedTabs(TRUE)` (`src/OptionsSheet.cpp:26-30,40-66`).
- Page order: **General → Types → Keyboard → Copy Buffers → Quick Paste Keyboard → Friends (if allowed) → Stats → About** (`src/OptionsSheet.cpp:53-66`).
- `OptionsQuickPaste` is compiled but **not added to the sheet** (the `AddPage` is commented out, `src/OptionsSheet.cpp:29,59`). Its `IDD_OPTIONS_QUICK_PASTE` template remains in the RC. Do not invest in modernizing it unless the owner explicitly re-enables the page.
- `OptionsUtilities` is also not added to the sheet (the member exists, `AddPage(m_pUtilites)` is absent).
- Apply button is hidden (`src/OptionsSheet.cpp:111`); OK/Cancel semantics must be preserved.
- Theme change closes and recreates the quick-paste window; `WM_OPTIONS_CLOSED` carries `m_themeChanged` (`src/OptionsSheet.cpp:127-131`; `src/MainFrm.cpp:1295-1319`). In-place `RefreshThemeColors` exists (`src/OptionsGeneral.cpp:720`) but only refreshes caption + scrollbars.

### 3.2 Page/dialog inventory

All templates use `FONT 8, "MS Shell Dlg"` (or close variants) and dialog-unit sizes; at 96 DPI these are small by modern standards. Sizes below are RC dialog units, not pixels.

| RC template | Class | Size (DLU) | Caption | Notes |
|---|---|---|---|---|
| `IDD_OPTIONS_GENERAL` | `COptionsGeneral` | 375×275 | General | Theme/language/font/path, DB limits, app include/exclude. `OnCtlColor` is effectively empty (`src/OptionsGeneral.cpp:457-472`). |
| `IDD_OPTIONS_TYPES` | `COptionsTypes` | 285×275 | — | Supported clipboard types list. |
| `IDD_OPTIONS_KEYSTROKES` | `COptionsKeyBoard` | 373×278 | Keyboard Shortcuts | 10 hotkey rows + Win checkboxes. |
| `IDD_OPTIONS_COPY_BUFFERS` | `COptionsCopyBuffers` | 379×277 | — | Copy buffers. |
| `IDD_OPTIONS_QUICK_PASTE_KEYBOARD` | `CQuickPasteKeyboard` | 375×278 | — | Quick-paste shortcuts. |
| `IDD_OPTIONS_FRIENDS` | `COptionFriends` | 367×275 | — | Friends/network. |
| `IDD_OPTIONS_STATS` | `COptionsStats` | 346×207 | — | Statistics. |
| `IDD_ABOUT` | `CAbout` | 337×206 | About Ditto | Listbox + links. |
| `IDD_ADV_OPTIONS` | `CAdvGeneral` | 390×269 | — | Advanced options / property grid. |
| `IDD_SCRIPT_EDITOR` | `CScriptEditor` | 435×324 | — | Rich edit script editor. |
| `IDD_OPTIONS_QUICK_PASTE` | `COptionsQuickPaste` | 367×280 | Quick Paste | **Not in sheet**; compiled. |
| `IDD_OPTIONS_UTILITIES` | `COptionsUtilities` | 241×137 | — | **Not in sheet**. |
| `IDD_OPTIONS_TYPES` (same as above) | | | | |
| `IDD_ADD_TYPE` | `CAddType` | 219×322 | — | Add clipboard format. |
| `IDD_COPY_PROPERTIES` | `CCopyProperties` | 293×289 | — | Clip properties. |
| `IDD_GROUP_NAME` | `CGroupName` | 186×49 | Ditto | New/rename group. |
| `IDD_FRIEND_DETAILS` | `CFriendDetails` | 203×79 | — | Friend details. |
| `IDD_MOVE_TO_GROUP` | `CMoveToGroupDlg` | 254×177 | — | Move to group. |
| `IDD_DIALOG_REMOTE_FILE` | `CFileTransferProgressDlg` | 284×110 | — | Remote file transfer. |
| `IDD_GLOBAL_CLIPS` | `CGlobalClips` | 369×199 | Global Hot Keys | List view + Close. |
| `IDD_DELETE_CLIP_DATA` | `CDeleteClipData` | 789×315 | Delete Clip Data | Large search/delete dialog. |
| `IDD_DIALOG_FREIND_PROMPT` | `CFriendPromptDlg` | 237×91 | — | Send-to-friend prompt. |
| `IDD_SELECT_DB` | `CSelectDB` | 276×46 | — | DB selection. |
| `IDD_ABOUTBOX` | — | 235×55 | — | Legacy/unused template (no source reference found). |

Additional non-dialog windows: clip editor `CEditWnd`/`CEditFrameWnd`, QR viewer, progress popup, no-DB frame, image viewer. See the surface map.

### 3.3 Dialog behavior to preserve

- DDX/DDV, validation, OK/Cancel/Apply, page order, tab order.
- Modal vs modeless behavior; `COptionsSheet` is modeless with `WF_CONTINUEMODAL` and posts `WM_OPTIONS_CLOSED`.
- Localization via `theApp.m_Language.UpdateOptionGeneral(this)` and similar; never hard-code new IDs.
- File dialogs, network error paths, and the script editor's open/save flow.

---

## 4. Theme system (`CTheme`)

### 4.1 Current keys and defaults

`CTheme::LoadDefaults()` (`src/Theme.cpp:23-72`) sets:

| Key | Default | Notes |
|---|---|---|
| `CaptionLeft` | `RGB(255,255,255)` | |
| `CaptionRight` | `RGB(204,204,204)` | |
| `CaptionLeftTopMost` / `CaptionRightTopMost` | white / 204 | |
| `CaptionLeftNotConnected` / `CaptionRightNotConnected` | white / yellow | |
| `CaptionTextColor` | `RGB(191,191,191)` | low contrast on both caption halves |
| `ListBoxOddRowsBG` | white | |
| `ListBoxEvenRowsBG` | `RGB(243,243,243)` | zebra |
| `ListBoxOddRowsText` / `EvenRowsText` | black | |
| `ListBoxSelectedBG` / `SelectedNoFocusBG` | `RGB(204,204,204)` | |
| `ListBoxSelectedText` / `SelectedNoFocusText` | black | |
| `ClipPastedColor` | `RGB(0,255,0)` | neon green |
| `ListSmallQuickPasteIndexColor` | `RGB(180,180,180)` | |
| `MainWindowBG` | `RGB(240,240,240)` | |
| `SearchTextBoxFocusBG` | white | |
| `SearchTextBoxFocusText` | black | |
| `SearchTextBoxFocusBorder` | white | invisible focus on white |
| `SearchTextHighlight` | `RGB(255,0,0)` | red text highlight |
| `Border` / `BorderTopMost` / `BorderNotConnected` | 204 / 204 / 204 | |
| `GroupTreeBG` | `RGB(240,240,240)` | |
| `GroupTreeText` | `RGB(127,127,127)` | |
| `CaptionSize` | 25 | caption thickness |
| `CaptionFontSize` | 19 | px at 96 DPI |
| `DescriptionWindowBG` / `Text` | 240 / black | |
| `ScrollBarThumb` / `ThumbHover` / `Track` | 180 / 140 / 240 | modern scrollbar |

### 4.2 Loading and compatibility

- `CTheme::Load` parses `<Ditto_Theme_File>` and 35 color/int values through `LoadColor`/`LoadInt` (`src/Theme.cpp:74-200`). Missing keys keep the defaults, but `LoadElement` logs an error for every missing node (`src/Theme.cpp:271-279`). Adding many optional keys will spam `Ditto.log` unless T1 adds optional loaders.
- `ListSmallQuickPasteIndexColor` has a default and an accessor (`src/Theme.h:35`) but is **never read from theme XML** — the load list at `src/Theme.cpp:154-192` omits it. No theme can override the first-ten index color today.
- Color values accept `rgb(r,g,b)`, `hsl(h,s,l)`, `#RRGGBB`, or a numeric `COLORREF` (`src/Theme.cpp:271-377`).
- Unknown/old theme files must keep loading; missing keys fall back to `LoadDefaults()`.
- Empty `Theme2` follows Windows: dark app setting → `DarkerDitto`, light → defaults + Windows accent (`src/Theme.cpp:74-103,202-223`). `DarkAppWindows10Setting` reads `AppsUseLightTheme` (`src/Misc.cpp:1561-1583`); `Windows10AccentColor` reads DWM `ColorizationColor` (`src/Misc.cpp:1585-1608`).
- `LoadWindowsAccentColor` applies the accent only to `ClipPastedColor`, `SearchTextBoxFocusBorder`, and `SearchTextHighlight` (`src/Theme.cpp:202-223`). It does not drive a global accent token today.
- Theme reload on Windows theme change is handled by `CMainFrame::OnWinIniChange` (`ImmersiveColorSet`) → 1s timer → reload only when `Theme2` is empty; the quick-paste window is closed/recreated if visible (`src/MainFrm.cpp:849-866,1503-1512`). For an explicit theme, no reload occurs on OS theme change (expected).

### 4.3 Bundled themes

`Debug/Themes/` contains 12 files: `Classic`, `DarkerDitto`, `MonoDark`, `Monolight`, `Nord`, `Selenized Black`, `Selenized Dark`, `Selenized Light`, `Selenized Strong`, `Selenized White`, `Selenized-Dark`, `Terminal`.

- `DarkerDitto.xml` (dark default) is incomplete: `CaptionSize` and `CaptionFontSize` are empty; `SearchTextBoxFocusBorder`, `SearchTextHighlight`, `ScrollBarThumb`, `ScrollBarThumbHover`, `ScrollBarTrack`, `ClipPastedColor`, and `ListSmallQuickPasteIndexColor` are absent. Result: light scrollbar, red highlight, lime pasted strip, and default index color on a black popup.
- `Classic.xml` uses blue captions and `RGB(30,116,211)` selection; old themes like this must remain usable even if the new design prefers neutral surfaces.
- Some themes have empty values for `GroupTreeBG`, `MainWindowBG`, `SearchTextBoxFocusBG`, etc. — those fall back to defaults, not to the theme's other colors.

---

## 5. Fonts

| Surface | Family / size / weight | Source |
|---|---|---|
| Popup caption | `Segoe UI`, 19px, 400 vertical / 500 horizontal | `src/DittoWindow.cpp:51-57` |
| Search | `Segoe UI`, 15px, 400 | `src/QPasteWnd.cpp:2214` |
| Group breadcrumb | `Segoe UI`, 12px, 400, italic | `src/QPasteWnd.cpp:2219-2221` |
| Clip list | `DisplayFont6` (default `Segoe UI`) | `src/Options.cpp:1535`; `src/QPasteWnd.cpp:2206-2218` |
| First-ten index | `"Small Font"` (nonexistent family), size from setting (default 5), `FW_LIGHT` | `src/QListCtrl.cpp:2293-2313` |
| Description body | `Segoe UI`, 13px, 400 | `src/ToolTipEx.cpp:138` |
| Description metadata | `Segoe UI`, 11px, 400 | `src/ToolTipEx.cpp:136` |
| Dialogs/property pages | `MS Shell Dlg`, 8pt | `CP_Main.rc` all `DIALOGEX` templates |

There is no Segoe UI Variable usage, no shared font factory, and repeated `CreateFont` calls on DPI change. `CDPI::Scale` is used for most popup sizes but not for dialog templates (MFC scales dialogs via DS_SETFONT/DPI virtualization; this must be verified with the toolchain).

---

## 6. DPI behavior

- Manifest declares `permonitorv2,permonitor` (`DeclareDPIAware.manifest`).
- `CDPI` uses `GetDpiForWindow` (Win10), falls back to `GetDpiForMonitor` (Win8), then `GetDeviceCaps(LOGPIXELSX)` (`src/DPI.h:27-80`).
- Popup sizes are mostly `m_DittoWindow.m_dpi.Scale(...)` (`src/QPasteWnd.cpp:667-785`), including reported DPI changes (`DittoWindow::OnDpiChanged`, `src/DittoWindow.cpp:883-924`).
- Exceptions/unscaled constants: `searchRowStart = 33` (`src/QPasteWnd.cpp:693`); `m_borderSize = 2` initialized before the window exists (`src/DittoWindow.cpp:11`); `CSymbolEdit` margins use `SM_CXSMICON` and `Scale` mixed (`src/SymbolEdit.cpp:441-456`); group tree image list 18×16 (`src/GroupTree.cpp:68`); dialog templates use DLUs.
- The popup remembers size in unscaled units (`SaveWindowSize`, `src/QPasteWnd.cpp:972-982`) and rescales on show (`src/QuickPaste.cpp:139-143,250-280`). Changing minimum/comfortable sizes must not corrupt saved sizes.
- No actual 150%/200% validation was possible — **UNVERIFIED**.

---

## 7. Localization

- UI strings use `theApp.m_Language.GetString(key, englishDefault)`; `Debug/Language/English.xml` is the source of truth (`CLAUDE.md`). 30 language files exist under `Debug/Language/`.
- Existing IDs must not change. New IDs/keys go to `docs/ui/new-strings-<task>.md`; only T5 may merge them into `English.xml`.
- Known hard-coded English: group tree root `"-No Group-"` (`src/GroupTree.cpp:184`); various window titles such as `"Options"` / `"Ditto"` are set then localized by `MultiLanguage` update helpers. The audit did not attempt a full hard-coded-string sweep; T2/T4 should use existing `MultiLanguage` helpers and flag new gaps.
- Long-string languages (German, Russian) must not clip in buttons/group boxes/tabs. The current dialog templates are fixed-position; T4a/T4b need a runtime layout pass, not RC edits (RC is T1-only).

---

## 8. Known issues register

Severity: **Blocker** = cannot ship; **Major** = visible/accessibility defect; **Minor** = polish; **Risk** = implementation hazard.

| ID | Sev | Issue | Evidence | Likely owner |
|---|---|---|---|---|
| UI-001 | Major | Black close/chevron PNGs are invisible on dark caption | `src/DittoWindow.cpp:61-65`; `DarkerDitto.xml` caption `RGB(0,0,0)` | T1/T2 |
| UI-002 | Major | Footer group/system icons are black PNGs; same dark-mode problem | `src/QPasteWnd.cpp:465,476` | T1/T2 |
| UI-003 | Major | Default caption text `RGB(191,191,191)` on white/grey is ~1.9:1 / ~1.1:1 | `src/Theme.cpp:25-38` | T1 |
| UI-004 | Major | Default search focus border is white on a white field — focus invisible | `src/Theme.cpp:52`; `src/SymbolEdit.cpp:508-527,931-983` | T1/T2 |
| UI-005 | Major | `DarkerDitto.xml` lacks scrollbar/highlight/focus-border keys → light scrollbar, red highlight, lime pasted strip on dark | `Debug/Themes/DarkerDitto.xml`; `src/Theme.cpp:52-53,65-68` | T1 |
| UI-006 | Major | Native controls (tree, edits, combos, tabs, menus) are unthemed in dark; options pages stay light | `src/GroupTree.cpp:176-179`; `src/OptionsGeneral.cpp:457-472`; no `SetWindowTheme`/DWM calls in `src/` | T1/T2/T4a/T4b |
| UI-007 | Major | No high-contrast support; custom colors override system colors | no `SPI_GETHIGHCONTRAST` usage found | T1 |
| UI-008 | Major | In-place theme preview refreshes only caption/scrollbars; list/search/tree keep old colors (the options-close path recreates the window) | `src/QPasteWnd.cpp:8122-8134`; `src/MainFrm.cpp:1295-1319` | T2/T3 |
| UI-009 | Major | List has no hover state and no visible keyboard focus (focus rect commented out) | `src/QListCtrl.cpp:430-658`, `606-608` | T3 |
| UI-010 | Major | Search is a custom-painted `CEdit` that may break IME/caret/selection | `src/SymbolEdit.cpp:459-588` | T2 (verify) |
| UI-011 | Major | 2px unscaled magic `searchRowStart = 33` plus a documented 2px top-gap hack | `src/QPasteWnd.cpp:674-675,693,769` | T2 |
| UI-012 | Major | Pasted state is color-only (2px neon green strip) | `src/QListCtrl.cpp:519-528`; `src/Theme.cpp:47` | T3 |
| UI-013 | Major | Dialogs are 8pt `MS Shell Dlg`, fixed DLU layouts; long localized strings can clip | `CP_Main.rc:746-1281`; no runtime layout pass | T4a/T4b |
| UI-014 | Major | Default caption font 19px and caption thickness 25px do not follow an 8px grid; oversized for compact popup | `src/Theme.cpp:70-71`; `src/DittoWindow.cpp:51-57` | T1/T2 |
| UI-015 | Minor | 2px `Draw3dRect` frame; no Win11 rounding/shadow/DWM colors | `src/DittoWindow.cpp:253-260` | T1/T2 |
| UI-016 | Minor | Two flat caption halves; full-bar color changes for topmost/disconnected | `src/DittoWindow.cpp:403-410`; `src/WndEx.cpp:125-148` | T2 |
| UI-017 | Minor | Empty state is bare text with no icon/action; font 15px | `src/QPasteWnd.cpp:515,727-737,2226-2228` | T2 |
| UI-018 | Minor | Group tree root string `-No Group-` is hard-coded English | `src/GroupTree.cpp:184` | T2 |
| UI-019 | Minor | Group tree image list 18×16, mask `RGB(255,0,0)`, not DPI-scaled | `src/GroupTree.cpp:67-78` | T2 |
| UI-020 | Minor | First-ten font uses nonexistent face `"Small Font"` and `FW_LIGHT` | `src/QListCtrl.cpp:2293-2313` | T3 |
| UI-021 | Minor | Search highlight default is red text; low contrast / dated | `src/Theme.cpp:53`; `src/QListCtrl.cpp:593-599` | T3 |
| UI-022 | Minor | `CQPasteWnd::CtlColor` returns `NULL_BRUSH` and sets a red background; purpose unclear | `src/QPasteWnd.cpp:389-395` | T2 |
| UI-023 | Minor | `CSymbolEdit::SetSearchImages` is entirely commented out (dead code) | `src/QPasteWnd.cpp:624-651` | T2 |
| UI-024 | Minor | `IDD_ABOUTBOX` has no source reference (legacy template) | `CP_Main.rc:746` | T4b |
| UI-025 | Minor | `COptionsQuickPaste` / `COptionsUtilities` are compiled but not in the sheet | `src/OptionsSheet.cpp:29,53-66` | T4a (scope decision) |
| UI-026 | Minor | `WndEx` registers a white class brush; possible white flash on dark | `src/WndEx.cpp:83` | T1/T2 |
| UI-027 | Minor | `CTheme::LoadElement` logs missing keys as errors; new optional keys will spam the log | `src/Theme.cpp:271-279` | T1 |
| UI-028 | Minor | Accent color from DWM only drives pasted/highlight/border, not a global accent; may have poor contrast | `src/Theme.cpp:202-223` | T1 |
| UI-029 | Minor | Popup minimum size is not enforced; a user can shrink it until controls overlap | `src/QPasteWnd.cpp:667-785` | T2 |
| UI-030 | Minor | No animations except scrollbar auto-hide; no reduced-motion handling | `src/ModernScrollBar.cpp:384,549-584` | T2/T3 |
| UI-031 | Risk | Measured row height depends on screen DC and user font; changing tokens must not break 1–5 line modes | `src/QListCtrl.cpp:414-428` | T3 |
| UI-032 | Risk | List custom draw returns `CDRF_SKIPDEFAULT`; any drawing change must preserve selection, drag, tooltip, and accessibility behavior | `src/QListCtrl.cpp:430-658` | T3 |
| UI-033 | Risk | `CSymbolEdit` custom text rendering may bypass IME composition; must be tested with Chinese/Japanese IME | `src/SymbolEdit.cpp:459-588` | T2 |
| UI-034 | Risk | Theme XML parser is shared by all themes; new keys must be optional and must not change old-theme appearance | `src/Theme.cpp:74-200` | T1 |
| UI-035 | Risk | DWM rounded corners on a `WS_POPUP` with custom NC may require `DwmSetWindowAttribute` probing; Win10 fallback must be square | no DWM code today | T1 |
| UI-036 | Risk | Dark native menus depend on undocumented uxtheme behavior; must be isolated and optional | no dark-mode code today | T1 |
| UI-037 | Minor | `ListSmallQuickPasteIndexColor` has an accessor/default but is never loaded from theme XML | `src/Theme.h:35`; `src/Theme.cpp:154-192` | T1/T3 |

---

## 9. Behavioral invariants (do not change)

These are not design decisions; they are compatibility surfaces.

1. `Theme2` key semantics, including empty = follow Windows. Existing theme files must load.
2. `CaptionPos` values and all four caption positions; roll-up/chevron; auto-expand timing.
3. Always-on-top / persistent state and `CAlphaBlend` transparency.
4. Search history, autocomplete, find-as-you-type, regex, last-search accelerator, F3/Shift+F3 description search.
5. List keyboard paths: arrows, PageUp/PageDown, Home/End, Enter, Shift+Enter plain text, Ctrl+1..0, F3, Delete, multi-select, drag, right-click.
6. First-ten hotkey display and `FirstTenHotKeysFontSize` setting.
7. Group tree create/rename/delete/move/enter/back and right-click menu commands.
8. `DisplayFont6` user clip font and 1–5 lines-per-row setting.
9. All menu command IDs and accelerators. New IDs only via the owning task; RC/resource changes only T1.
10. `QPASTE_TITLE` "Ditto" as the window title base.
11. Localization IDs and `GetString(key, default)` pattern; no direct `English.xml` edits outside T5.
12. Add-in menu entries and `Shared/` ABI.
13. Settings defaults and DB path semantics; no writes to real `%APPDATA%\Ditto` during tests.
14. Tray icon/menu commands and `-open`/`-close`/`-exit` behavior.

---

## 10. Verification gaps (explicit)

| Claim | Status |
|---|---|
| Code compiles | **UNVERIFIED — no toolchain** |
| App runs | **UNVERIFIED — no binary** |
| Before screenshots exist | **NO — BLOCKED** |
| Light/dark visual result | **UNVERIFIED** |
| 100/150/200% DPI layout | **UNVERIFIED** (source review only) |
| High-contrast mode | **UNVERIFIED** (no implementation today) |
| IME composition in search | **UNVERIFIED** (high risk from custom paint) |
| Contrast ratios | **CALCULATED** from token RGB, not measured |
| Long German/Russian strings | **UNVERIFIED** |

---

## 11. Modernization hot spots (ordered)

1. `CTheme` + new token layer (T1) — every other task depends on it.
2. Caption/DWM/icons (T1/T2) — highest visible win and fixes dark-mode invisibility.
3. Search field chrome + IME-safe text rendering (T2) — highest risk.
4. List row rendering + hover/focus/selection (T3) — highest visual density win.
5. Empty state + group tree (T2) — low risk, high perceived quality.
6. Options sheet + shared dialog theming (T4a) — largest surface area, most localization risk.
7. Remaining dialogs (T4b) — mechanical once T4a exists.
8. Menus/tray readability (T1/T2) — OS-dependent; best-effort with fallback.
