# IMPLEMENTATION-PLAN — Ditto UI modernization

- Version: 1.0 (T0 baseline)
- Date: 2026-11-10
- Inputs: `docs/ui/CURRENT-STATE.md`, `docs/ui/DESIGN-SYSTEM.md`, `docs/ui/ENVIRONMENT.md`
- Constraint reminder: no new framework/runtime dependency; C++17/MFC/Win32/GDI/GDI+ only. No DB schema, clipboard semantics, add-in ABI, settings-key semantics, language XML IDs, menu command IDs, or hotkeys changes. All UI sizes through `CDPI` scaling. No writes to real `%APPDATA%\Ditto`; portable temp dirs only. Build + screenshots are required evidence before claiming completion.
- **Current gate: `BLOCKED: build toolchain missing` (see `ENVIRONMENT.md`). T1–T4b must not start until the coordinator resolves it.**

---

## 1. Dependency graph

```
T0 (audit + design)
 └── T1 (foundation: tokens, draw/window/dark helpers, theme XML, icons)
      ├── T2 (popup shell: caption, search, bottom bar, group tree, empty state)
      ├── T3 (list rows, hover/focus, modern scrollbar, description window)
      └── T4a (shared dialog theming + core options pages)
           └── T4b (remaining dialogs)
                 │
T2 ── T3 ── T4a ── T4b ──► T5 (integration + QA + new strings merge)
                              └── T6 (independent review)
```

T2, T3, and T4a can run in parallel from `FOUNDATION_BRANCH`. T4b is based on T4a. T5 merges T2 → T3 → T4a → T4b into a fresh worktree based on `FOUNDATION_BRANCH`. T6 reviews T5's branch with a different agent where available.

---

## 2. Branch / worktree guidance

| Task | Suggested worktree | Base branch | Agent |
|---|---|---|---|
| T0 | `ui-design` (this worktree) | `master` | `claude` |
| T1 | `ui-foundation` | `T0_BRANCH` | `codex` (or available implementer) |
| T2 | `ui-shell` | `FOUNDATION_BRANCH` | `codex` |
| T3 | `ui-list` | `FOUNDATION_BRANCH` | `codex` |
| T4a | `ui-options-core` | `FOUNDATION_BRANCH` | `codex` |
| T4b | `ui-dialogs` | `T4A_BRANCH` | `codex` |
| T5 | `ui-integration` | `FOUNDATION_BRANCH` | `claude` |
| T6 | `ui-review` | `T5_BRANCH` | different agent if available |

Rules:

- Never run state-changing git commands in `D:\Develop\project\Ditto`. Commit only in the worker's own worktree.
- Do not push, do not delete branches, do not merge into `master`.
- Each task reports branch name, HEAD SHA, worktree path, modified files, build command/result, screenshot paths, UNVERIFIED items, and risks.

---

## 3. File ownership matrix

| Owner | Exclusive files |
|---|---|
| **T1** | `src/Theme.h/.cpp`; new `src/UiTheme.h/.cpp`, `src/UiDraw.h/.cpp`, `src/UiWindow.h/.cpp`, `src/DarkMode.h/.cpp`; `src/GdiImageDrawer.h/.cpp`, `src/CGdiPlusBitmap.h` (only if necessary and non-breaking); `src/CP_Main.cpp` (app init only); `src/MainFrm.cpp` (theme reload/`ImmersiveColorSet` only); `CP_Main.vcxproj`, `CP_Main.vcxproj.filters` (foundation new files); `CP_Main.rc`, `resource.h`, `res/**`, `Debug/Themes/**`; `docs/ui/FOUNDATION-NOTES.md`; `docs/ui/new-strings-T1.md` (if any) |
| **T2** | `src/QPasteWnd.h/.cpp`, `src/QuickPaste.h/.cpp`, `src/WndEx.h/.cpp`, `src/DittoWindow.h/.cpp`, `src/SymbolEdit.h/.cpp`, `src/GroupTree.h/.cpp`, `src/GroupStatic.h/.cpp`, `src/SearchEditBox.h/.cpp` (only if actually needed); `docs/ui/new-strings-T2.md` |
| **T3** | `src/QListCtrl.h/.cpp`, `src/ModernScrollBar.h/.cpp`, `src/ClipFormatQListCtrl.h/.cpp`, `src/FormattedTextDraw.h/.cpp` (only if necessary), `src/ToolTipEx.h/.cpp`, `src/ImageViewer.h/.cpp` (thumbnail only); `docs/ui/new-strings-T3.md` |
| **T4a** | new `src/Ui/ThemedDialog.h/.cpp`, `src/Ui/ThemedControls.h/.cpp`, `src/Ui/UiIds.h` (or equivalent); `src/OptionsSheet.h/.cpp`, `src/OptionsGeneral.h/.cpp`, `src/OptionsKeyBoard.h/.cpp`, `src/QuickPasteKeyboard.h/.cpp`, `src/OptionsQuickPaste.h/.cpp` (only if still compiled/re-enabled), `src/AdvGeneral.h/.cpp`, `src/ScriptEditor.h/.cpp`; `CP_Main.vcxproj/.filters` **only** to register its own `src/Ui/**` files; `docs/ui/new-strings-T4a.md` |
| **T4b** | `src/About.*`, `GroupName.*`, `MoveToGroupDlg.*`, `CopyProperties.*`, `DeleteClipData.*`, `SelectDB.*`, `FriendDetails.*`, `FriendPromptDlg.*`, `OptionFriends.*`, `OptionsCopyBuffers.*`, `OptionsStats.*`, `OptionsTypes.*`, `OptionsUtilities.*`, `AddType.*`, `FileTransferProgressDlg.*`, `ProgressWnd.*`; `docs/ui/new-strings-T4b.md` |
| **T5** | Integration fixes across files (record every file/reason); `docs/ui/QA-REPORT.md`, `docs/ui/REVIEW-GUIDE.md`; `artifacts/ui/after/**`; `tools/ui-qa/**`; `Debug/Language/English.xml` **only** to merge new strings during integration |
| **T6** | `docs/ui/CODE-REVIEW.md` only (no source changes) |

Cross-ownership rules:

- T2/T3/T4b must not add new files without owner approval. If a helper is needed:
  - Prefer a file-local static function in an owned `.cpp`.
  - If it must be shared, ask T1 to add it to `UiDraw`/`UiWindow` or T4a to add it to `src/Ui/`.
- T1 is the only task allowed to edit `CP_Main.rc`, `resource.h`, `res/`, and `Debug/Themes/`. Requests from T2/T3/T4a/T4b go through the coordinator.
- `Debug/Language/English.xml` is edited only by T5. All other tasks write `docs/ui/new-strings-<task>.md`.
- `CP_Main.vcxproj/.filters`: T1 registers foundation files; T4a registers only its `src/Ui/**` files; no one else edits these files.
- New runtime control IDs: T4a defines them in `src/Ui/UiIds.h`; T4b uses local constants in its own `.cpp`; T2/T3 must not add IDs to `resource.h`.

---

## 4. Cross-cutting workstreams

### 4.1 New strings workflow

Format for `docs/ui/new-strings-<task>.md`:

| Key | English default | Context | Task |
|---|---|---|---|
| `NoResultsClearSearch` | `Clear search` | Empty-state action button | T2 |

- Prefer reusing existing `GetString` keys. Search `Debug/Language/English.xml` first.
- Never change an existing key or ID.
- T5 merges all task files into `English.xml` during integration and records the merge in `QA-REPORT.md`.
- Other language files are not edited by anyone; untranslated keys fall back to the English default.

### 4.2 Project file registration

- T1 adds `src/UiTheme.*`, `src/UiDraw.*`, `src/UiWindow.*`, `src/DarkMode.*` to `CP_Main.vcxproj` and `.filters`.
- T4a adds `src/Ui/ThemedDialog.*`, `src/Ui/ThemedControls.*`, `src/Ui/UiIds.h` to both files. It must not reorder or change other entries.
- T2/T3/T4b add no files.

### 4.3 Theme XML and RC ownership

- T1 owns all theme XML changes and all RC/resource changes.
- T2/T3/T4a/T4b propose new resources/strings via the coordinator; they do not edit RC/resource.h/res/Themes.
- Old theme files must keep loading. The parser must treat new keys as optional.

### 4.4 Build / run / screenshot recipe

Per task, if the toolchain is available:

```bat
nuget restore CP_Main_10.sln
msbuild CP_Main_10.sln /p:Configuration=Release /p:Platform=x64
```

Then follow prompt §7: portable temp directory, copy `Ditto.exe`, `ICU_Loader.dll`, `Addins`, `Language`, `Themes`, create the `portable` marker, write a scratch `Ditto.Settings` with `Theme2` and `DBPath` inside the temp dir, start, create test clips, capture screenshots.

Never write to `%APPDATA%\Ditto` or the user's real `Ditto.db`. If there is no interactive desktop, report `BLOCKED: no interactive desktop`.

---

## 5. Task specifications

### T0 — Audit, design system, plan (this task)

**Goal:** establish the frozen baseline.

**Delivered:**

- `docs/ui/CURRENT-STATE.md` — source-referenced UI inventory and issue register.
- `docs/ui/DESIGN-SYSTEM.md` — frozen tokens, components, accessibility, compatibility rules.
- `docs/ui/IMPLEMENTATION-PLAN.md` — this file.
- `docs/ui/ENVIRONMENT.md` — `BLOCKED: build toolchain missing` + unblock options.
- `artifacts/ui/before/README.md` — blocker + reproduction recipe (no fake screenshots).
- `artifacts/ui/mockups/` — 3 annotated popup mockups + gallery.

**Acceptance evidence:** file list, mockup paths, toolchain status, explicit UNVERIFIED list. No source changes, no RC/resource changes.

**Exit criteria:** coordinator reviews the design-system summary and either freezes it or sends revisions back to T0.

---

### T1 — Foundation: tokens, draw/window/dark helpers, theme XML, icons

**Goal:** make the semantic token layer real and give every downstream task a stable API.

**Files:** see §3. No edits to `QPasteWnd`, `QListCtrl`, `ModernScrollBar`, `ToolTipEx`, `Options*`, or dialogs (except the minimal `CP_Main.cpp`/`MainFrm.cpp` integration points).

**Work items:**

1. **`CUiTheme` (new):**
   - Wrap `CTheme`; expose semantic accessors for every token in `DESIGN-SYSTEM.md §3–§7`.
   - Resolve legacy → modern fallbacks (§9.3), high contrast (§13.2), and Windows accent validation (§3.3).
   - Provide `Refresh(HWND)` / `OnDpiChanged()` / `OnSystemThemeChanged()` entry points; purge caches.
   - Keep `CTheme`'s public API source-compatible so existing call sites compile until their owner migrates them.
2. **`CTheme` extension:**
   - Add optional color/int/string loaders with no error logs for missing new keys.
   - Add all new keys from §9.1–§9.2; keep old keys.
   - `LoadDefaults()` → modern light; `DarkerDitto.xml` → modern dark; add `Modern Light.xml`/`Modern Dark.xml`.
   - Preserve `Theme2` semantics, empty = follow Windows, accent loading.
3. **`UiDraw` (new):**
   - DPI-scaled cached brushes/pens/fonts; rounded rect (GDI+), 1px border, focus ring, accent bar, text ellipsis/measure, checkbox/radio glyphs, vector icons (search/close/chevron/folder/cog/check/pin/dots), icon tint cache.
   - No per-call object churn; document cache keys.
4. **`UiWindow` (new):**
   - Win11 DWM probing: corner preference, border color, caption color, immersive dark mode; Win10 fallback.
   - Apply/remove DWM shadow if feasible; never hard-link.
   - `ApplyWindowEffects(HWND)` idempotent.
5. **`CDarkMode` (new):**
   - Detect Windows dark app mode and high contrast.
   - `ApplyToWindow(HWND)`, `ApplyToControl(HWND)`, `ApplyToMenus(HWND)` using runtime-probed uxtheme; isolate undocumented calls; safe no-op fallback.
   - Never force dark mode in high contrast.
6. **App integration:**
   - Initialize the theme/dark mode in `CCP_MainApp::InitInstance` (minimal).
   - On `ImmersiveColorSet`, reload tokens and refresh (minimal `MainFrm.cpp`; downstream tasks do the per-window repaint).
7. **Icons/resources:**
   - Provide vector glyphs or tinted raster variants for close/chevron/folder/cog/system menu; keep existing IDs and 16/20/24/32/36/40/44/48/52/56 assets.
   - Tray icon may be refreshed only if original/derived; no downloaded assets.
8. **Build:** Release x64 + Debug x64 (when toolchain available). Fix all compile errors.

**API contract for downstream (document in `FOUNDATION-NOTES.md`):**

- `CUiTheme& UiTheme()` or `CGetSetOptions::m_Theme` accessor pattern, with exact signatures.
- `UiDraw::RoundedRect`, `UiDraw::DrawTextEllipsis`, `UiDraw::DrawGlyph`, `UiDraw::FocusRing`, `UiDraw::IconSizeForDpi`.
- `UiWindow::ApplyWindowEffects`, `UiWindow::SetDarkTitleBar`.
- `CDarkMode::ApplyToWindow/Control/Menus`, `CDarkMode::IsHighContrast`.
- Cache invalidation rules and thread affinity (UI thread only).

**Risks:**

| ID | Risk | Mitigation |
|---|---|---|
| R-T1-1 | Theme XML backward compatibility breaks old themes | Defaults + optional loaders; test all 12 bundled themes; never log-as-error for new keys |
| R-T1-2 | Undocumented uxtheme dark APIs crash on some Win10 builds | Runtime build check + `GetProcAddress`; try/catch-free safe no-op fallback; isolate in `DarkMode.*` |
| R-T1-3 | DWM rounded corners do not apply to custom-NC popup | Probe `DwmSetWindowAttribute`; fall back to square + 1px border; verify on Win10/11 |
| R-T1-4 | Accent validation changes user theme colors | Only derived tokens are validated; explicit legacy colors win |
| R-T1-5 | Vector icons look worse than existing PNGs | Provide a per-glyph fallback to existing PNG; screenshot compare |
| R-T1-6 | GDI/GDI+ cache leaks | Cache owned by theme singleton; purge on DPI/theme; `GetGuiResources` check |

**Acceptance evidence:**

- Branch/SHA, build log Release x64 (+ Debug x64 if available), no compiler errors.
- All bundled theme XML files load without crash; old themes still readable.
- Default light/dark theme screenshot of the popup (or a test window if the popup is not yet restyled), showing tokens applied.
- `FOUNDATION-NOTES.md` with the API contract, cache keys, DWM/uxtheme probing, and fallback behavior.
- Explicit UNVERIFIED list (e.g., high contrast, 200% DPI, Win10 build) if not tested.

---

### T2 — Quick-paste shell: caption, search, bottom bar, group tree, empty state

**Goal:** restyle the popup shell using T1 tokens without changing any behavior.

**Files:** `QPasteWnd.*`, `QuickPaste.*`, `WndEx.*`, `DittoWindow.*`, `SymbolEdit.*`, `GroupTree.*`, `GroupStatic.*`; `SearchEditBox.*` only if needed.

**Work items:**

1. **Layout (`MoveControls`):**
   - Replace `searchRowStart = 33`, `cy - Scale(...)`, and the 2px top-gap hack with token-based formulas (`CaptionHeight`, `BottomBarHeight`, `SearchHeight`, `BottomBarPadding`).
   - Support all four caption positions and the compact 300×300 size; clamp to a 280×220 minimum.
   - Do not change stored size units or default `QuickPasteCX/CY`.
2. **Caption (`DittoWindow`):**
   - 34px thickness; title `FontWindowTitle` 14 semibold; vertical/horizontal rotated text measured correctly (remove the `m_titleTextHeight`/4px hack).
   - 28×28 buttons, 16px vector/tinted icons; hover/pressed/focus/disabled; red close hover.
   - Apply `UiWindow::ApplyWindowEffects` (Win11 round/dark/border; Win10 fallback); no 2px `Draw3dRect`.
   - Status dot + persistent accent edge; legacy `Caption*TopMost`/`NotConnected` colors honored if explicitly set.
   - Keep roll-up/chevron, auto-expand, hit testing, `sendWMClose`, transparency.
3. **Search (`SymbolEdit`):**
   - 34px height, radius 8, `Surface`/`Border`, focus ring, left glyph, prompt, clear/history buttons, hover states.
   - **IME-safe implementation:** prefer parent-drawn chrome + native edit text rendering. If custom paint remains, render composition/caret or prove IME works. Test Chinese Pinyin + Japanese IME, selection, Backspace, Esc, horizontal scrolling.
   - Keep `SHAutoComplete`, past searches, last-search accelerator, prompt text, and keyboard behavior.
4. **Bottom bar:**
   - 40px; group button 28 at left, search flexible center, system menu 28 at right; 8px gaps; hover/pressed/focus; localized tooltips.
   - Keep existing actions and z-order.
5. **Group tree / breadcrumb:**
   - Apply tokens to the native tree via `CDarkMode` and/or owner-draw; selected/hover/focus states; 16px icons; ellipsis + tooltip.
   - Localize the `-No Group-` root string via a new key.
   - Keep F7/Delete/Alt+Enter/Enter/Esc/right-click flows.
6. **Empty state:**
   - Icon + title + body + "Clear search" button; use existing `NoSearchResults` key and add `Clear search` in `docs/ui/new-strings-T2.md`.
   - Center in the list viewport; keyboard reachable; works at 300×300.
7. **Optional motion:**
   - 120/160ms fade/slide only if `CAlphaBlend` already supports it and reduced-motion/high-contrast checks pass. Otherwise skip; do not regress show/hide.

**Risks:**

| ID | Risk | Mitigation |
|---|---|---|
| R-T2-1 | IME composition/caret regressions from custom-painted edit | Prefer native text rendering; mandatory Chinese/Japanese IME test; if no build, mark UNVERIFIED and do not claim |
| R-T2-2 | Long group names/title strings clip | Ellipsis + tooltip; German/Russian screenshot |
| R-T2-3 | Caption positions (left/bottom) break layout | Test all four positions at 300×300 and 420×480 |
| R-T2-4 | Focus/selection lost when relayout/repaint | Keep child window handles and focus; verify Tab/arrow paths |
| R-T2-5 | Roll-up/auto-expand/transparency regressions | Manual regression list in the report |
| R-T2-6 | New empty-state button steals Enter/Esc semantics | Only focusable via Tab; Enter on button clears; Esc still closes/hides |

**Acceptance evidence:**

- Branch/SHA, build log, no errors.
- Light/dark screenshots: main popup (300×300 and 420×480), search focused/empty, group tree open, empty state, right and top caption.
- Keyboard path list verified: hotkey, Tab, arrows, Enter, Shift+Enter, Esc, Ctrl+1..0, F3, Delete, group flows.
- Long German/Russian string screenshot or explicit UNVERIFIED.
- IME test evidence or explicit UNVERIFIED with the chosen implementation explained.

---

### T3 — List rendering, scrollbar, description window

**Goal:** modern, dense, readable rows and a polished overlay scrollbar; no performance regression.

**Files:** `QListCtrl.*`, `ModernScrollBar.*`, `ClipFormatQListCtrl.*`, `FormattedTextDraw.*` (only if needed), `ToolTipEx.*`, `ImageViewer.*` (thumbnail only).

**Work items:**

1. **Row layout:** 30px minimum pitch; token padding; 6px radius row pill; correct 1–5 line modes; preserve `MeasureItem` correctness with the user font.
2. **States:** hover (new), selected, selected+focus, selected without focus, pressed; visible focus ring; keyboard navigation; multi-select; hover must not disturb selection.
3. **Pasted indicator:** 3px `Pasted` bar + check/tooltip; remove neon strip; keep the setting behavior.
4. **First-ten index:** `FontCaption` 11px, 18px column, 1px divider; no `"Small Font"`/`FW_LIGHT`; keep show/hide setting.
5. **Status icons:** 16px, 2px gaps, vector/tinted; keep symbol order and tooltips; group/shortcut/sticky/in-group/no-autodelete.
6. **Content:** text 13px user font, ellipsis, multi-line; search highlight `HighlightBg`/`HighlightText`, not red; image thumbnail 40×28 radius 4 + checkerboard; colour swatch 16×16 radius 3.
7. **Scrollbar:** transparent/Canvas track, 6→10px thumb, radius, min length, fade 120ms, auto-hide 800ms, vertical+horizontal, drag/hit areas; keep `CModernScrollBar` interface stable (coordinate with T2 if not).
8. **Description window:** radius 10, Surface, 1px border, padding 12, typography 12/13/11, DWM shadow; preserve rich text/HTML/image/search/pin/wrap/auto-hide.
9. **Performance:** cache fonts/brushes/pens/paths/tinted icons per DPI; no per-row GDI+ objects; measure `GetGuiResources` before/after 5k-row scroll; invalidate only changed rows.

**Risks:**

| ID | Risk | Mitigation |
|---|---|---|
| R-T3-1 | Per-row GDI+ allocation causes scroll stutter | Cache in `UiDraw`; no `Graphics`/`SolidBrush` per item |
| R-T3-2 | `CDRF_SKIPDEFAULT` change breaks selection/drag/tooltips | Keep state flags; regression list; test mouse+keyboard |
| R-T3-3 | Row height breaks 2–5 line modes or DPI | Use text metrics + token padding; test 1/2/3/5 lines |
| R-T3-4 | Hover repaint flicker | Double-buffer the list if needed; invalidate row rects only |
| R-T3-5 | Scrollbar interface changes break T2 | Keep public API; coordinate before changing |
| R-T3-6 | Tooltip window regressions (pin/wrap/search) | Preserve behavior; test F3/Shift+F3, links, images |

**Acceptance evidence:**

- Branch/SHA, build log.
- Light/dark screenshots: list with normal/hover/selected/focus, multi-line rows, image/colour clips, first-ten badges, empty list, description window.
- Performance numbers (GDI objects before/after, scroll observation) or UNVERIFIED with explanation.
- Keyboard/mouse selection, paste, search, scroll, thumbnail verification.

---

### T4a — Shared dialog theming + core options pages

**Goal:** a reusable themed dialog layer and modern General/Keyboard/QuickPasteKeyboard/AdvGeneral/ScriptEditor pages.

**Files:** new `src/Ui/ThemedDialog.*`, `src/Ui/ThemedControls.*`, `src/Ui/UiIds.h`; `OptionsSheet.*`, `OptionsGeneral.*`, `OptionsKeyBoard.*`, `QuickPasteKeyboard.*`, `OptionsQuickPaste.*` (only if compiled/re-enabled), `AdvGeneral.*`, `ScriptEditor.*`; `CP_Main.vcxproj/.filters` only for `src/Ui/**`.

**Work items:**

1. **ThemedDialog/ThemedControls:**
   - Handle `WM_CTLCOLOR`, `WM_ERASEBKGND`, `WM_DRAWITEM`, `NM_CUSTOMDRAW`, `WM_THEMECHANGED`.
   - Theme statics, group boxes, buttons, checkboxes/radios, edits, combos, tabs, lists/trees, property grids.
   - Dark mode via T1 `CDarkMode`; high contrast fallback to system colors.
2. **Buttons/inputs:**
   - Token heights/radii/padding; hover/pressed/focus/disabled; primary/secondary/destructive; default-button accent border; auto-size for long localized text.
3. **Property sheet:**
   - Owner-drawn tabs or visual manager (justify choice); page background/title; preserve page order, DDX/DDV, validation, OK/Cancel (Apply hidden).
4. **Core pages:**
   - General: theme/language/font/path/limits/app filters; ensure theme switch persists and refresh path works.
   - Keyboard: 10 hotkey rows + Win checkboxes; no behavior changes.
   - Quick Paste Keyboard: shortcuts.
   - AdvGeneral: property grid/list; ScriptEditor: rich edit + buttons.
5. **Layout:**
   - Runtime reflow in `OnInitDialog`/`OnSize`; no RC edits; new controls via `src/Ui/UiIds.h`.
   - Long German/Russian strings must not clip.
6. **No settings changes:** preserve reads/writes, defaults, Apply/OK/Cancel.

**Risks:**

| ID | Risk | Mitigation |
|---|---|---|
| R-T4a-1 | Native control dark mode fails on some Windows builds | `CDarkMode` fallback to light native; never crash |
| R-T4a-2 | Owner-drawn tabs break page switching/keyboard | Keep native tab control messages; test Tab/Ctrl+Tab/arrow keys |
| R-T4a-3 | Runtime reflow breaks DDX or control IDs | Do not recreate controls; move/resize existing; test save/cancel |
| R-T4a-4 | Long strings clip in fixed dialogs | Measure + reflow; screenshot German/Russian |
| R-T4a-5 | Theme preview/apply path regressions | Test theme change, OK/Cancel, restart persistence |
| R-T4a-6 | `OptionsQuickPaste` scope creep | It is not in the sheet; do not modernize unless explicitly re-enabled |

**Acceptance evidence:**

- Branch/SHA, build log.
- Light/dark screenshots for General, Keyboard, QuickPasteKeyboard, AdvGeneral, ScriptEditor.
- Long localized string screenshots (German and/or Russian) or UNVERIFIED.
- Open/switch/save/cancel tests for every core page; settings persistence verified.
- Tab order documentation.

---

### T4b — Remaining dialogs

**Goal:** apply T4a's shared layer to all remaining dialogs; fix layout/padding/alignment; verify smoke paths.

**Files:** the dialog list in §3. No edits to T4a helpers, T2/T3 files (including `ImageViewer.*`), resources, or themes. New runtime IDs as local constants in each `.cpp` only.

**Work items:**

1. Apply `ThemedDialog`/`ThemedControls` to each dialog: background, typography, buttons, inputs, lists/property grids, group boxes, tabs, scrollbars.
2. Fix layout: outer padding, label/field alignment, button order/default, tab order, long strings.
3. Dark mode + high contrast fallback; do not break modal/modeless behavior, file dialogs, network error paths.
4. Smoke each dialog: open/close/OK/Cancel/error path.

**Acceptance evidence:**

- Branch/SHA, build log.
- Per-dialog light/dark screenshots (can be a contact sheet) or UNVERIFIED if a dialog cannot be reached.
- Smoke results for every dialog.
- No new hard-coded user-visible strings (or recorded in `new-strings-T4b.md`).

---

### T5 — Integration + full QA

**Goal:** one buildable, runnable integration branch with visual evidence and a regression report.

**Work items:**

1. Merge in order with merge commits: `git merge --no-ff <T2>` → `<T3>` → `<T4a>` → `<T4b>`. Record merge commits.
2. Resolve simple conflicts; stop and report semantic/ownership conflicts to the coordinator; do not silently change another owner's design.
3. Build Release x64 (+ Debug x64 if possible). Run `nuget restore` first.
4. Portable test instance per prompt §7; capture before/after screenshots (light/dark, popup, list, search, groups, empty state, options, at least two dialogs).
5. Run the regression checklist (prompt §6.2) and record PASS/FAIL/UNVERIFIED.
6. Check GDI handles, memory, CPU, and logs for new errors.
7. Merge all `docs/ui/new-strings-*.md` into `Debug/Language/English.xml`; do not touch other language files.
8. Write `docs/ui/QA-REPORT.md`, `docs/ui/REVIEW-GUIDE.md`, and `artifacts/ui/after/**`.

**Acceptance:** build exit 0; screenshots complete; no unexplained FAIL; all UNVERIFIED listed; no push/merge to master.

---

### T6 — Independent review

**Goal:** independent code/design review; no source changes.

**Work items:**

1. Review `git diff <FOUNDATION_BRANCH>...HEAD` and commit history against the hard constraints, design system, and DoD.
2. Look for: functional regressions, keyboard/focus regressions, dark/high-contrast issues, DPI clipping, localization overflow, GDI/performance issues, resource/theme compatibility, dead code, hard-coded colors, per-row GDI+ allocations, error handling.
3. Build; spot-check screenshots; optionally run the app for manual verification.
4. Produce `docs/ui/CODE-REVIEW.md` with Blocker/Major/Minor/Nit, each with `file:line`, repro/verification, and suggested owner (T1/T2/T3/T4a/T4b/T5). State a clear merge recommendation.

**Acceptance:** every finding locatable; no code modified; explicit conclusion.

---

## 6. Risk register (cross-task)

| ID | Severity | Risk | Owner | Mitigation |
|---|---|---|---|---|
| R-01 | Blocker | No build toolchain | Coordinator | Resolve before T1; see `ENVIRONMENT.md` |
| R-02 | High | IME/caret regression in search | T2 | Native text rendering preferred; Chinese/Japanese IME test; UNVERIFIED if no build |
| R-03 | High | Dark native controls unsupported/undocumented | T1/T4a/T4b | Isolate uxtheme calls; runtime probe; fallback to native light |
| R-04 | High | Old themes break or change colors | T1 | Optional keys + legacy fallback map; test all bundled themes |
| R-05 | High | Long localized strings clip | T4a/T4b | Runtime measure/reflow; German/Russian screenshots |
| R-06 | High | List performance/GDI leaks | T3 | Cache; per-row invalidation; `GetGuiResources` evidence |
| R-07 | High | High contrast regression | T1/T4a/T4b | System colors + no alpha; actual high-contrast test or UNVERIFIED |
| R-08 | Medium | DWM round/dark APIs fail on Win10 | T1 | Runtime probe; square/1px fallback |
| R-09 | Medium | Merge conflicts in shared integration points | T5 | T1 owns shared files; small, well-scoped diffs; merge order |
| R-10 | Medium | RC/resource bottleneck on T1 | Coordinator | Batch resource requests; T1 delivers early |
| R-11 | Medium | DPI/size persistence corruption | T2/T5 | Keep `UnScale`; test 100/150/200%; clamp min |
| R-12 | Medium | Menu dark mode is OS-dependent | T1 | Best-effort; native fallback; P0 only requires readability |
| R-13 | Medium | UIA/screen-reader regression | T2/T3 | Accessible names/tooltips; verify with Inspect/Narrator or UNVERIFIED |
| R-14 | Low | `OptionsQuickPaste` dead UI confusion | T4a | Do not invest unless re-enabled |
| R-15 | Low | Vector icons lower quality than PNGs | T1 | Per-glyph fallback; screenshot compare |

---

## 7. Definition of done (mapped to prompt §6)

### Build/run

- [ ] `nuget restore CP_Main_10.sln` succeeds.
- [ ] `msbuild CP_Main_10.sln /p:Configuration=Release /p:Platform=x64` exits 0.
- [ ] Debug x64 also builds (if environment allows).
- [ ] Portable instance starts; `ICU_Loader.dll`, `Language`, `Themes` in place.
- [ ] Text/image/HTML/file clips can be copied, listed, and pasted back.
- [ ] Exit leaves no test process.

### Functional regression

- [ ] Hotkey open/hide; `-open`/`-close`/`-exit`.
- [ ] Search input/clear/history/find-as-you-type/case/regex.
- [ ] List keyboard/mouse: arrows, PageUp/Down, Home/End, Enter, Shift+Enter, Ctrl+1..0, F3, multi-select, right-click.
- [ ] Groups: new/rename/move/enter/back/delete/pin.
- [ ] Clipboard formats: text, Unicode, image, HTML, RTF, files, colour; dedupe.
- [ ] Options: open, switch pages, change+save, cancel, theme switch, language switch, font.
- [ ] Tray: icon, menu, show/hide, connect/disconnect, exit.
- [ ] Add-in: DittoUtil menu appears and works.
- [ ] Drag/drop, copy buffers, UAC paste: tested or UNVERIFIED.
- [ ] Theme persists across restart; follows Windows; registry test restored.
- [ ] English + one long-string language (German/Russian) no clipping.
- [ ] High contrast: visible text/focus, or code review + UNVERIFIED.

### Visual/accessibility

- [ ] Light/dark screenshots: popup, list, search, empty state, options, ≥2 dialogs.
- [ ] Body contrast ≥ 4.5:1; focus ring visible; state not color-only.
- [ ] 100% DPI no clipping/overlap; 150%/200% code review or real test with UNVERIFIED if not.
- [ ] Old theme XML loads; missing keys fall back.
- [ ] Language IDs unchanged; new strings have English defaults.

### Code quality

- [ ] New files registered in `CP_Main.vcxproj/.filters` by the correct owner.
- [ ] No hard-coded new colors/sizes scattered in `.cpp`; tokens/helpers used.
- [ ] No per-row GDI/GDI+ object creation.
- [ ] No dead code/debug dialogs.
- [ ] No writes to user data/real config.

---

## 8. Explicit non-goals / do-not-do

- Do not merge to `master` or push any branch.
- Do not touch the user's `D:\Develop\project\Ditto` working tree with state-changing git commands.
- Do not change database schema, clipboard semantics, `WM_CLIPBOARD_COPIED` ownership, add-in ABI, settings key semantics, language IDs, menu command IDs, or hotkeys.
- Do not add Qt/Electron/WebView2/WPF/Direct2D or any new runtime dependency.
- Do not download fonts/icons/binaries of unknown origin; icons must be original or derived from repo assets.
- Do not fake build logs or screenshots; if blocked, say so.

---

## 9. Open questions for the coordinator/user

1. **Toolchain:** install VS Build Tools (MFC + v145), connect a build host, or accept code-only changes? Nothing downstream can produce acceptance evidence until this is answered.
2. **Default popup size:** keep 300×300 (plan assumption) or approve a larger default (e.g., 420×480)? Changing `QuickPasteCX/CY` defaults affects existing users' first impression but not saved sizes.
3. **Search position:** keep Ditto's bottom search bar (plan assumption) or move it to the top? Moving it is a larger behavioral change.
4. **Dark menus:** accept best-effort native menus (plan assumption) or require fully owner-drawn menus (P3, higher risk)?
5. **OptionsQuickPaste/OptionsUtilities:** keep them out of the sheet (plan assumption) or re-enable them as part of T4a?
