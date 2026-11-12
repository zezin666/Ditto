# DESIGN-SYSTEM — Ditto UI modernization (frozen for T1–T6)

- Version: **1.0 (T0 frozen baseline)**
- Date: 2026-11-10
- Scope: native Windows UI, C++17 / MFC / Win32 / GDI / GDI+. No web runtime, no new dependency.
- Units: **logical pixels at 96 DPI** unless stated otherwise. Every value must be passed through `CDPI::Scale()` at draw/layout time.
- Status: T1 may refine *implementation details* but must not change token names, token roles, or component states without asking the coordinator. T2–T6 consume this document as-is.
- Mockups: `artifacts/ui/mockups/` (HTML/SVG). They are design references, not screenshots.

---

## 1. Principles

1. **Content first, chrome last.** The clipboard text is the product. Surfaces are neutral; accent is reserved for focus, selection, primary actions, and one status indicator.
2. **Compact, not cramped.** Preserve Ditto's small-footprint popup. Prefer 4/8/12/16/24 spacing and 30px rows over large touch targets.
3. **Light and dark are peers.** Dark is not an inversion; it uses its own surfaces and accent contrast.
4. **One accent, derived from Windows.** The accent can come from the OS, but it must pass contrast checks. If it does not, fall back to the default accent.
5. **State is never color-only.** Selected, pasted, pinned, connected, error, and focus each get a shape/glyph/weight cue in addition to color.
6. **Native behavior wins.** Keyboard paths, IME, selection, drag/drop, tooltips, and OS accessibility are more important than pixel perfection.
7. **No per-row GDI+ allocation.** Cache brushes, pens, fonts, paths, and tinted icons.

Non-goals: no Fluent rewrite, no WinUI/WebView2/Qt, no animated page transitions, no drop shadows drawn per row, no change to command IDs, hotkeys, language IDs, database, or add-in ABI.

---

## 2. Token model

```
Theme XML / defaults
        │
        ▼
     CTheme  ── legacy keys (CaptionLeft, ListBoxOddRowsBG, …) still readable
        │
        ▼
    CUiTheme (T1)  ── semantic tokens: Canvas, TextPrimary, AccentFill, ListRowHeight, …
        │            resolves legacy → modern fallbacks, high contrast, Windows accent
        ▼
   UiDraw / UiWindow (T1) ── cached brushes/pens/fonts/paths, DPI-scaled primitives
        │
        ▼
 T2/T3/T4 controls ── consume tokens; never hard-code a new color or size
```

Rules:

- A control must ask for a **semantic token**, never for an RGB literal.
- A token may be backed by a legacy theme key (see §9) so old theme XML keeps working.
- High contrast bypasses the theme (see §13).
- The design system must not override the user's clip font (`DisplayFont6`) or first-ten font size.

---

## 3. Color tokens

### 3.1 Light

| Token | Value | Role / contrast |
|---|---|---|
| `Canvas` | `#F7F8FA` | window background; maps from `MainWindowBG` |
| `Surface` | `#FFFFFF` | search field, cards, list base |
| `SurfaceAlt` | `#F1F3F6` | bottom bar, caption, inactive surfaces |
| `SurfaceSunken` | `#EEF1F5` | read-only fields, inset areas |
| `Border` | `#E3E6EC` | 1px subtle separators/fields; maps from `Border` |
| `BorderStrong` | `#CFD3DB` | hover borders, focus-adjacent |
| `TextPrimary` | `#1B1F24` | 15.58:1 on Canvas, 16.56:1 on Surface |
| `TextSecondary` | `#5C6470` | 5.63:1 on Canvas, 5.98:1 on Surface |
| `TextDisabled` | `#7E8692` | 3.46:1 on Canvas (disabled text is exempt but kept legible) |
| `Accent` | `#2563EB` | text/icon/underline; 5.17:1 on Surface, 4.86:1 on Canvas |
| `AccentHover` | `#1D4ED8` | accent text hover |
| `AccentPressed` | `#1E40AF` | accent text pressed |
| `AccentFill` | `#2563EB` | primary button fill; white text 5.17:1 |
| `AccentFillHover` | `#1D4ED8` | primary hover; white 6.70:1 |
| `AccentFillPressed` | `#1E40AF` | primary pressed; white 8.72:1 |
| `AccentOn` | `#FFFFFF` | text/icon on `AccentFill` |
| `Selection` | `#E8F0FF` | selected row/input selection; `TextPrimary` 14.46:1 |
| `SelectionText` | `#1B1F24` | text on Selection |
| `Hover` | `#F0F3F8` | hover fill |
| `FocusRing` | `#2563EB` | 2px ring; same as Accent by default |
| `Danger` | `#C43C3C` | destructive text/icon; 5.17:1 on Surface |
| `DangerFill` | `#C43C3C` | destructive fill; white 5.17:1 |
| `DangerOn` | `#FFFFFF` | text/icon on DangerFill |
| `Pinned` | `#8A5A00` | pin icon/text; 5.93:1 on Surface |
| `Pasted` | `#1F7A4D` | pasted indicator; 5.32:1 on Surface |
| `HighlightBg` | `#E8F0FF` | search match background (maps from Selection) |
| `HighlightText` | `#1B1F24` | search match text |
| `CloseHoverBg` | `#C42B1C` | close-button hover (Windows convention); white 5.66:1 |
| `CloseHoverOn` | `#FFFFFF` | close glyph on hover |
| `ScrollBarTrack` | `#F7F8FA` | modern scrollbar track (drawn with Canvas/alpha) |
| `ScrollBarThumb` | `#C4C9D2` | scrollbar thumb |
| `ScrollBarThumbHover` | `#9AA1AC` | scrollbar thumb hover |

### 3.2 Dark

| Token | Value | Role / contrast |
|---|---|---|
| `Canvas` | `#1B1C1F` | window background |
| `Surface` | `#24262A` | search field, cards, list base |
| `SurfaceAlt` | `#2C2F34` | bottom bar, caption, inactive surfaces |
| `SurfaceSunken` | `#202226` | read-only fields, inset areas |
| `Border` | `#383B41` | 1px subtle separators |
| `BorderStrong` | `#4A4E56` | hover borders |
| `TextPrimary` | `#F2F3F5` | 15.35:1 on Canvas |
| `TextSecondary` | `#A7ADB7` | 7.55:1 on Canvas, 6.7:1 on Surface |
| `TextDisabled` | `#6C727C` | 3.52:1 on Canvas |
| `Accent` | `#6C9BFF` | text/icon/underline; 6.30:1 on Canvas, 5.61:1 on Surface |
| `AccentHover` | `#82AAFF` | accent text hover |
| `AccentPressed` | `#5A87E6` | accent text pressed |
| `AccentFill` | `#4C8DFF` | primary button fill; dark text 5.85:1 |
| `AccentFillHover` | `#6BA0FF` | primary hover; dark text 7.22:1 |
| `AccentFillPressed` | `#3C7AEA` | primary pressed; dark text 4.60:1 |
| `AccentOn` | `#0B1220` | text/icon on `AccentFill` (dark, not white) |
| `Selection` | `#2E3A52` | selected row; TextPrimary 10.26:1 |
| `SelectionText` | `#F2F3F5` | text on Selection |
| `Hover` | `#2F3238` | hover fill |
| `FocusRing` | `#6C9BFF` | 2px ring |
| `Danger` | `#FF6B6B` | 6.14:1 on Canvas |
| `DangerFill` | `#FF6B6B` | destructive fill; dark text `#1B1C1F` 6.14:1 |
| `DangerOn` | `#1B1C1F` | text/icon on DangerFill |
| `Pinned` | `#E0A94A` | 8.07:1 on Canvas |
| `Pasted` | `#4CC38A` | 7.69:1 on Canvas |
| `HighlightBg` | `#2E3A52` | search match background |
| `HighlightText` | `#F2F3F5` | search match text |
| `CloseHoverBg` | `#C42B1C` | close hover; white 5.66:1 |
| `CloseHoverOn` | `#FFFFFF` | close glyph |
| `ScrollBarTrack` | `#1B1C1F` | track |
| `ScrollBarThumb` | `#4E535C` | thumb |
| `ScrollBarThumbHover` | `#6C727C` | thumb hover |

### 3.3 Accent derivation from Windows

- If `Theme2` is empty (follow Windows), read `HKCU\Software\Microsoft\Windows\DWM\ColorizationColor`.
- Validate the candidate accent for contrast:
  - Light: `Accent` on `Surface` ≥ 4.5:1 **and** white on `AccentFill` ≥ 4.5:1.
  - Dark: `Accent` on `Canvas` ≥ 4.5:1 **and** `AccentOn` on `AccentFill` ≥ 4.5:1.
- If validation fails, fall back to the default accent (`#2563EB` light / `#6C9BFF` text + `#4C8DFF` fill dark). Log the fallback at debug level, not as an error.
- Never use a raw OS accent as a large fill without validating the text-on-fill contrast.

### 3.4 Forbidden colors

- No new `RGB(...)` literals in UI `.cpp` files. Existing literals must be replaced by tokens as their owner task touches the file.
- No pure red (`#FF0000`) for search highlight; no neon green (`#00FF00`) for pasted; no pure black icons on dark surfaces.
- No gradients in the popup shell, list, buttons, or dialogs.

---

## 4. Metric tokens (96 DPI logical px)

| Token | Value | Notes |
|---|---|---|
| `RadiusWindow` | 12 | Win11 DWM round; Win10 square fallback |
| `RadiusPanel` | 10 | cards, group tree overlay, tooltip |
| `RadiusControl` | 8 | search field, tabs container |
| `RadiusListItem` | 6 | list row, menu items, small buttons |
| `BorderWidth` | 1 | subtle border; 1 physical px preferred at all DPI |
| `FocusRingWidth` | 2 | logical; scaled |
| `CaptionHeight` | 34 | height for top/bottom, width for left/right |
| `CaptionButtonSize` | 28 | square hit target |
| `CaptionIconSize` | 16 | glyph inside button |
| `SearchHeight` | 34 | search field |
| `BottomBarHeight` | 40 | contains 34 search + 28 buttons (3/6px insets) |
| `BottomBarPadding` | 6 | around buttons; 8 between controls |
| `ListRowHeight` | 30 | minimum pitch; may grow for 2–5 lines |
| `ListRowInsetX` | 8 | horizontal inset of the row pill |
| `ListRowInsetY` | 1 | top/bottom inset of the row pill |
| `ListIndexColumnWidth` | 18 | first-ten digit + divider |
| `ListIconSize` | 16 | status icons |
| `IconSizeSmall` | 16 | inline glyphs |
| `IconSizeMedium` | 20 | toolbar/button glyphs |
| `IconSizeLarge` | 24 | empty state / prominent |
| `ScrollBarThumbWidth` | 6 | rest state |
| `ScrollBarThumbHoverWidth` | 10 | hover state |
| `ScrollBarInset` | 2 | from window edge |
| `ScrollBarRadius` | 3 | rest; 5 when wide |
| `ScrollBarMinThumb` | 24 | minimum length |
| `TooltipRadius` | 10 | description window |
| `TooltipPadding` | 12 | inner padding |
| `DialogPadding` | 16 | outer page/dialog padding |
| `FieldGap` | 8 | between label/field, related controls |
| `SectionGap` | 16 | between groups |
| `ButtonHeight` | 30 | dialog/empty-state buttons |
| `ButtonMinWidth` | 76 | text buttons |
| `ButtonPaddingX` | 14 | text buttons |
| `ButtonRadius` | 6 | |
| `InputHeight` | 30 | edit/combo |
| `InputRadius` | 6 | |
| `InputPaddingX` | 8 | |
| `TabHeight` | 36 | property sheet tab strip |
| `TabPaddingX` | 12 | |
| `TabUnderlineHeight` | 2 | active indicator |
| `MenuItemHeight` | 28 | if custom menus are ever drawn |
| `MenuPaddingX` | 12 | |
| `MenuGutterWidth` | 28 | check/icon column |
| `EmptyStateMaxWidth` | 240 | text wrap width |

Spacing scale: **4 / 8 / 12 / 16 / 24**. Do not invent 5, 7, 10, 13, 18, 20 for layout (10 is allowed only as a panel radius/tooltip radius). Existing magic numbers must be replaced as their owner touches the code.

---

## 5. Typography

Font family resolution order (runtime, cached):

1. `Segoe UI Variable Text` (Windows 11, if installed)
2. `Segoe UI`
3. `Tahoma`
4. `Arial`

Monospace: `Cascadia Mono` → `Consolas` → `Courier New`.

| Token | Size | Weight | Line height | Use |
|---|---|---|---|---|
| `FontWindowTitle` | 14px | 600 | 20 | popup caption, description caption |
| `FontTitle` | 14px | 600 | 20 | empty-state title, dialog section title |
| `FontBody` | 13px | 400 | 18 | clip text chrome, dialog body, menu |
| `FontBodyStrong` | 13px | 600 | 18 | selected labels, group breadcrumb |
| `FontSecondary` | 12px | 400 | 16 | helper text, dialog labels, tabs |
| `FontCaption` | 11px | 400 | 14 | metadata, timestamps, first-ten index |
| `FontMono` | 12px | 400 | 16 | hotkey text, GUIDs, monospace snippets |
| `FontDialogBody` | 12px | 400 | 16 | MFC dialog body (replaces 8pt MS Shell Dlg) |
| `FontDialogTitle` | 14px | 600 | 20 | dialog title |

Rules:

- GDI font height is negative logical pixels: `-dpi.Scale(px)` (or `-MulDiv(px, dpi, 96)`); keep one helper.
- The **clip list text uses `DisplayFont6`**, not `FontBody`. `FontBody` applies to chrome.
- The first-ten index uses `FontCaption`; it must not use the nonexistent `"Small Font"` face name.
- Do not use italic for group breadcrumb; use `FontBodyStrong` + secondary color.
- Dialog text 12px is slightly larger than the current 8pt template; T4a/T4b must verify no clipping and may reflow at runtime.

---

## 6. Radius, elevation, shadows

- Radii: window 12, panels/cards/tooltips 10, controls/search/tabs 8, list rows/menu items 6, small swatches 3.
- Border: 1 physical px. No 3D etched borders, no `Draw3dRect`.
- Elevation:
  - Win11: use DWM (`DWMWA_WINDOW_CORNER_PREFERENCE` = round, `DWMWA_BORDER_COLOR`, `DWMWA_CAPTION_COLOR` where appropriate, `DWMWA_USE_IMMERSIVE_DARK_MODE`) via `GetProcAddress`/`DwmSetWindowAttribute`. Do not hard-link.
  - Win10: square corners, 1px border, no synthetic drop shadow. Native DWM shadow is enough if the window has a frame; otherwise omit.
  - Do not draw per-row shadows; use `Hover`/`Selection` fills.
- Scrim/dim: existing `CDimWnd` may remain; do not add new overlays.

---

## 7. Motion

| Token | Value | Use |
|---|---|---|
| `MotionFastMs` | 120 | hover/press color changes, scrollbar fade |
| `MotionStdMs` | 160 | popup fade/slide, empty-state appearance |
| `ScrollBarAutoHideMs` | 800 | current behavior; keep |

- Easing: ease-out (`cubic-bezier(0.2, 0, 0, 1)` equivalent). In Win32, use a timer + `AlphaBlend`/layered window only if the existing window already supports it; otherwise switch states instantly.
- Disable all non-essential animation when:
  - `SystemParametersInfo(SPI_GETCLIENTAREAANIMATION)` is FALSE, or
  - `SPI_GETANIMATION` reports disabled, or
  - high contrast is active, or
  - the session is remote/`GetSystemMetrics(SM_REMOTESESSION)`.
- Never animate layout of the list rows while scrolling. Never animate the caret.

---

## 8. Iconography

- Sizes: 16 (inline/status/caption buttons), 20 (bottom bar/toolbar), 24 (empty state, prominent). Select by DPI bucket: ≤120 → 16, ≤168 → 20, else 24; use 20/24 variants where the asset exists.
- Style: monochrome, 1.5px stroke at 96 DPI (`IconStrokeTenths = 15`), round caps/joins, 2px padding inside the view box, geometric shapes. No skeuomorphism, no gradients, no multi-color glyphs except the tray icon.
- Color: `TextSecondary` default; `TextPrimary` for active/selected; `Accent` for active state; `Pinned`/`Pasted`/`Danger` only for their semantic status.
- **Prefer vector glyphs drawn by `UiDraw`** for simple shapes (search, close, chevron, folder, cog, check, pin, system menu). This solves dark-mode raster tinting and DPI crispness without adding dozens of PNGs. Cache the `GraphicsPath`/tinted bitmap per (glyph, size, dpi, color).
- Keep existing PNG IDs and `LoadStdImageDPI` compatibility for complex images and the tray icon. Do not remove resource IDs. New PNG assets are T1-only.
- The close/chevron/footer icons must never be pure black-only rasters in dark mode. If a raster is kept, generate a light/tinted variant or draw a vector.
- Icon hit target is at least 28×28 even when the glyph is 16.

---

## 9. Theme XML: new keys, legacy mapping, compatibility

### 9.1 New color keys (optional)

| Key | Light default | Dark default |
|---|---|---|
| `Canvas` | `#F7F8FA` | `#1B1C1F` |
| `Surface` | `#FFFFFF` | `#24262A` |
| `SurfaceAlt` | `#F1F3F6` | `#2C2F34` |
| `SurfaceSunken` | `#EEF1F5` | `#202226` |
| `BorderStrong` | `#CFD3DB` | `#4A4E56` |
| `TextPrimary` | `#1B1F24` | `#F2F3F5` |
| `TextSecondary` | `#5C6470` | `#A7ADB7` |
| `TextDisabled` | `#7E8692` | `#6C727C` |
| `Accent` | `#2563EB` | `#6C9BFF` |
| `AccentHover` | `#1D4ED8` | `#82AAFF` |
| `AccentPressed` | `#1E40AF` | `#5A87E6` |
| `AccentFill` | `#2563EB` | `#4C8DFF` |
| `AccentFillHover` | `#1D4ED8` | `#6BA0FF` |
| `AccentFillPressed` | `#1E40AF` | `#3C7AEA` |
| `AccentOn` | `#FFFFFF` | `#0B1220` |
| `Selection` | `#E8F0FF` | `#2E3A52` |
| `SelectionText` | `#1B1F24` | `#F2F3F5` |
| `Hover` | `#F0F3F8` | `#2F3238` |
| `FocusRing` | `#2563EB` | `#6C9BFF` |
| `Danger` | `#C43C3C` | `#FF6B6B` |
| `DangerFill` | `#C43C3C` | `#FF6B6B` |
| `DangerOn` | `#FFFFFF` | `#1B1C1F` |
| `Pinned` | `#8A5A00` | `#E0A94A` |
| `Pasted` | `#1F7A4D` | `#4CC38A` |
| `HighlightBg` | `#E8F0FF` | `#2E3A52` |
| `HighlightText` | `#1B1F24` | `#F2F3F5` |
| `CloseHoverBg` | `#C42B1C` | `#C42B1C` |
| `CloseHoverOn` | `#FFFFFF` | `#FFFFFF` |
| `ScrollBarThumb` | `#C4C9D2` | `#4E535C` |
| `ScrollBarThumbHover` | `#9AA1AC` | `#6C727C` |
| `ScrollBarTrack` | `#F7F8FA` | `#1B1C1F` |

Existing keys `CaptionLeft`, `CaptionRight`, `Caption*TopMost`, `Caption*NotConnected`, `CaptionTextColor`, `ListBox*`, `GroupTree*`, `DescriptionWindow*`, `SearchTextBoxFocus*`, `SearchTextHighlight`, `MainWindowBG`, `Border*`, `ClipPastedColor`, `ListSmallQuickPasteIndexColor`, `ScrollBar*`, `CaptionSize`, `CaptionFontSize` remain valid and must still load.

### 9.2 New metric/font/motion keys (optional)

`RadiusWindow`, `RadiusPanel`, `RadiusControl`, `RadiusListItem`, `BorderWidth`, `FocusRingWidth`, `CaptionHeight`, `CaptionButtonSize`, `CaptionIconSize`, `SearchHeight`, `BottomBarHeight`, `ListRowHeight`, `ListRowInsetX`, `ListRowInsetY`, `ListIndexColumnWidth`, `ListIconSize`, `ScrollBarThumbWidth`, `ScrollBarThumbHoverWidth`, `ScrollBarInset`, `ScrollBarRadius`, `TooltipRadius`, `TooltipPadding`, `DialogPadding`, `FieldGap`, `SectionGap`, `ButtonHeight`, `ButtonMinWidth`, `ButtonPaddingX`, `ButtonRadius`, `InputHeight`, `InputRadius`, `InputPaddingX`, `TabHeight`, `TabPaddingX`, `TabUnderlineHeight`, `FontFamilyUi`, `FontFamilyMono`, `FontSizeBody`, `FontSizeSecondary`, `FontSizeCaption`, `FontSizeTitle`, `FontSizeWindowTitle`, `FontSizeDialogBody`, `IconStrokeTenths`, `MotionFastMs`, `MotionStdMs`, `ScrollBarAutoHideMs`.

### 9.3 Legacy → modern fallback map

When a new key is absent, derive it from the old keys so existing themes remain coherent:

| New token | Fallback |
|---|---|
| `Canvas` | `MainWindowBG` |
| `Surface` | `ListBoxOddRowsBG` (if it differs from `Canvas`), else `#FFFFFF`/`#24262A` |
| `SurfaceAlt` | `ListBoxEvenRowsBG` (if it differs from `Surface`), else derived |
| `Border` | existing `Border` |
| `BorderStrong` | blend(`Border`, `TextPrimary`, 18%) |
| `TextPrimary` | `ListBoxEvenRowsText` |
| `TextSecondary` | `GroupTreeText` |
| `TextDisabled` | blend(`TextSecondary`, `Canvas`, 55%) |
| `Accent` | validated Windows accent, else default |
| `AccentFill` | `Accent` if white/dark text contrast passes, else darken/lighten to pass |
| `AccentOn` | computed for max contrast against `AccentFill` |
| `Selection` | `ListBoxSelectedBG` |
| `SelectionText` | `ListBoxSelectedText` |
| `Hover` | blend(`Surface`, `TextPrimary`, 4%) |
| `FocusRing` | `Accent` |
| `HighlightBg` | `Selection` |
| `HighlightText` | existing `SearchTextHighlight` (legacy red is preserved for old themes) |
| `Pasted` | `ClipPastedColor` (legacy lime is preserved only for old themes) |
| `Pinned` | `ListSmallQuickPasteIndexColor` is not semantically pinned; use default |
| `ScrollBar*` | existing keys |
| Caption colors | existing `Caption*` keys are honored directly |
| List zebra | existing `ListBoxOdd/EvenRowsBG` honored if they differ |

**Compatibility rule:** when a user selects an old theme such as `Classic`, the popup may gain rounded chrome and modern spacing, but its explicit colors must win. Do not "correct" a user theme's colors silently. Only the *default* modern themes use the new neutral palette.

### 9.4 Parser requirements (T1)

- Add optional loaders that do **not** log an error when a new key is missing. The current `LoadElement` logs every missing key (`src/Theme.cpp:271-279`); 20+ new keys on every old theme would flood `Ditto.log`.
- Keep accepting `rgb()`, `hsl()`, `#RRGGBB`, and numeric values.
- Add string loading for `FontFamilyUi`/`FontFamilyMono`.
- New keys must be ignored by old Ditto versions? Old versions will ignore unknown XML elements (they only query known nodes), so this is safe.
- `LoadDefaults()` becomes the modern **light** theme. `DarkerDitto.xml` becomes the modern **dark** theme (auto-selected when Windows is dark). Add `Modern Light.xml` and `Modern Dark.xml` explicit themes. Keep all other bundled themes unchanged.
- Theme switch must purge cached fonts/brushes/tinted icons and repaint all children in place.

### 9.5 Follow-Windows and high contrast

- Empty `Theme2`:
  - Windows dark → load `DarkerDitto` (modern dark).
  - Windows light → `LoadDefaults()` (modern light) + validated Windows accent.
- `WM_SETTINGCHANGE`/`ImmersiveColorSet` behavior remains (1s debounce), but the refresh must update every visible surface, not just caption/scrollbars.
- High contrast: see §13. Detect on theme load and on `WM_THEMECHANGED`/`WM_SETTINGCHANGE`.

---

## 10. Component specifications

All coordinates are logical px at 96 DPI. `C` = caption thickness (34), `W/H` = window size.

### 10.1 Popup shell

- Window: `WS_POPUP`, background `Canvas`, 1px `Border`, `RadiusWindow` 12 on Win11; square on Win10.
- Minimum usable size: **280×220** logical. Recommended comfortable size: 420×480. **Do not change the default `QuickPasteCX/CY` values**; existing users' saved sizes must be preserved. The layout must work at the default 300×300.
- Content area:
  - Caption on right: content = `[0, 0, W-C, H]`.
  - Caption on top: content = `[0, C, W, H]`.
  - Left/bottom mirrored.
- Bottom command bar: height `BottomBarHeight` (40), anchored to the content bottom, fill `SurfaceAlt`, top border 1px `Border`.
- List viewport: content minus bottom bar; horizontal inset 0, vertical 0. Rows inset `ListRowInsetX`/`Y`.
- Empty state occupies the list viewport.
- No native status bar, no title bar gradient, no 2px 3D frame.
- Window resize: clamp to minimum; invalidate and relayout on `WM_SIZE`; preserve focus/selection.
- Show/hide: keep existing `CAlphaBlend` transparency setting and auto-hide behavior.

### 10.2 Caption / title bar (all four positions)

Common:
- Thickness `CaptionHeight` = 34 (width for left/right, height for top/bottom).
- Fill `SurfaceAlt`; 1px `Border` separating from content; no gradient.
- Title: `FontWindowTitle` 14px semibold, `TextPrimary`, single line, ellipsis (`DT_END_ELLIPSIS`), 12px padding.
- Buttons: `CaptionButtonSize` 28×28, radius 6, icon `CaptionIconSize` 16, 2px inset within the strip, 3px from the outer edge, 4px gap.
- Button default: transparent; hover `Hover`; pressed `Selection`; focus ring `FocusRing` 2px inside; disabled `TextDisabled`.
- Close button hover: `CloseHoverBg` fill, `CloseHoverOn` glyph. Pressed: `#A5261A`.
- Chevron toggles roll-up/expand; no maximize/minimize on the popup (existing flags).
- Status:
  - Connected: 6px `Accent` dot near the title or at the caption's inner end; tooltip explains.
  - Not connected: 6px `Danger` dot + tooltip. Do **not** recolor the whole caption in modern themes. Legacy themes that set `Caption*NotConnected` still get their explicit color.
  - Always-on-top/persistent: 3px `Accent` bar along the outer edge of the caption + localized title suffix (existing `top_window` string). Legacy `Caption*TopMost` colors are honored if the theme defines them distinctly.
- Right caption (default):
  - Close at `(W-31, 3, 28, 28)`, chevron at `(W-31, 35, 28, 28)`; additional buttons continue downward at 32px pitch.
  - Vertical title rotated −90°, centered between the last button and the bottom status area, 12px end padding, ellipsis. Do not use the current manual `m_titleTextHeight` offset hack; compute the rotated text rect from `GetTextExtentPoint32`.
  - Status dot at `(W-17, H-14, 6, 6)`; persistent bar at `(W-3, 80, 3, H-100)`.
- Top caption:
  - Title at `(12, 0, W-? , 34)`, buttons to the right: close `(W-32,3)`, chevron `(W-64,3)`, optional others leftward.
  - Status dot at `(8,14,6,6)` and title starts at `(20,...)`; persistent bar along the top edge.
- Left/bottom mirror the above.
- Roll-up: caption-only size; chevron direction reflects state; auto-expand timer and hover behavior preserved.

### 10.3 Search field

- Height `SearchHeight` (34); radius `RadiusControl` (8); fill `Surface`; border 1px `Border`.
- Hover: border `BorderStrong`.
- Focus: 2px `FocusRing` ring drawn inside the field; do not rely on a 1px border color change.
- Disabled: `SurfaceAlt` fill, `TextDisabled` text, no ring.
- Left glyph: 16×16 `TextSecondary` at x+10, vertically centered.
- Text: `FontBody` 13px (or the existing search font size 15px? Freeze at 13px; T2 may keep 15px only if it preserves layout. The design target is 13–15px; use 14px for the search input to match the current 15px visual weight). **Decision:** search text `FontBody` 13px, prompt `FontBody` 13px. This is a visible change; T2 may use 14px if 13px feels too small, but must not exceed 15px.
- Text left inset: 34px; right inset: clear button (20) + history button (20) + 8px padding = 48px.
- Clear button: 20×20, `TextSecondary`, hover `Hover`, radius 6, visible only when text is non-empty; clicking clears and keeps focus. Keyboard: Esc clears (existing behavior; verify `DoActionForceCloseWindow`/`DoActionCloseWindow` semantics).
- History/drop-down button: 20×20 to the left of clear or at the far right depending on existing behavior; preserve past-search interaction.
- Prompt: `TextSecondary`, same font, left aligned at text inset; hidden when text non-empty.
- Selection: `Selection`/`SelectionText`; caret remains the system caret. **Do not custom-draw text if it breaks IME composition/caret.** Preferred implementation: parent-drawn rounded chrome + a native `CEdit` with matching background brush (`WM_CTLCOLOREDIT`) and no border; the parent draws the icon/buttons. If custom painting is retained, it must explicitly render the composition string and caret, or T2 must prove IME works.
- Placeholder and clear button retain tooltips/accessibility names.

### 10.4 Clip list row

- Pitch: `ListRowHeight` 30 minimum. For 2–5 lines, pitch = `lines * lineHeight + 10`, rounded up to the next even value; never less than 30.
- Row visual rect: `x = ListRowInsetX`, `y = rowTop + ListRowInsetY`, `width = viewportW - 2*ListRowInsetX`, `height = pitch - 2*ListRowInsetY`, radius 6.
- Background:
  - Normal: `Surface` (modern themes). Legacy themes that set `ListBoxOddRowsBG`/`EvenRowsBG` distinctly keep their zebra.
  - Hover (unselected): `Hover`.
  - Selected: `Selection`; text `SelectionText`.
  - Selected + focused: `Selection` + 2px `FocusRing` outline inset 1px (or a 1px inner ring if 2px feels heavy) + the left accent bar.
  - Selected without focus: `Selection` + `SelectionText`; the ring is omitted but the accent bar remains.
  - Pressed: `Selection`.
- Left accent bar (selected): 3×14, radius 1.5, `Accent`, at `x+4`, vertically centered.
- First-ten index: column `ListIndexColumnWidth` 18 at x+12; digit `FontCaption` 11px semibold `TextSecondary`; 1px divider `Border` at x+30, y+7..rowH-7.
  - If the user disables first-ten text, use the 12px indent instead of 30px.
- Status icons: 16×16, 2px gap, after the index; `TextSecondary` tint; `Pinned` for sticky; tooltips for each. Preserve the existing symbol order from `GetDisplayText` (`<group>`, `<noautodelete>`, `<shortcut>`, `<ingroup>`, `<sticky>`).
- Pasted indicator: 3×14 `Pasted` bar at the left inside edge + a 12px check glyph or a localized "pasted" tooltip; not color-only. Do not use a 2px neon strip.
- Text: user clip font (`DisplayFont6`), `TextPrimary`; top/bottom padding 4; left padding after icons; `DT_VCENTER`, `DT_END_ELLIPSIS`, `DT_EXPANDTABS`, `DT_NOPREFIX` for plain text.
- Search highlight: `HighlightBg` background + `HighlightText` text; bold optional. Do not use red text.
- Image thumbnail: 40×28, radius 4, 1px `Border`, checkerboard for transparency, vertically centered; cache decoded/thumbnailed images (existing caches).
- Colour swatch: 16×16, radius 3, 1px `Border`; hex text `FontBody`.
- Row separators: none in modern themes. Legacy zebra is the separator.
- Empty space below the last row: `Canvas`/`Surface` (same as list background).
- Hit testing: keep the full row selectable; do not make only the text clickable. Keep drag selection and right-click behavior.

### 10.5 Modern scrollbar

- Overlay child, width 10 (hit area 12) on hover; visible thumb 6 rest / 10 hover; inset 2 from the right/bottom; radius 3 rest / 5 hover.
- Track: transparent or `ScrollBarTrack` with the list background showing through; no solid light track on dark.
- Thumb: `ScrollBarThumb`; hover/drag `ScrollBarThumbHover`; minimum length `ScrollBarMinThumb` 24; disabled/hidden when content fits.
- Fade in/out `MotionFastMs` 120; auto-hide `ScrollBarAutoHideMs` 800 after the last interaction; keep the existing timer behavior.
- Horizontal scrollbar mirrors the vertical spec.
- Keep `CModernScrollBar`'s public interface stable unless T2 is coordinated; T3 owns the implementation.
- If modern scrollbars are disabled (`m_useModernScrollBar = false`), native scrollbars remain; theme them best-effort via `DarkMode`.

### 10.6 Group tree overlay / breadcrumb

- Overlay panel: `Surface` fill, radius 10, 1px `Border`, padding 6; DWM shadow if available; otherwise no shadow.
- Item height 28; radius 6; selected `Selection` + 3px `Accent` left bar; hover `Hover`; text `FontBody` 13px `TextPrimary`; chevrons `TextSecondary`.
- Root item: use the existing localized "No Group" concept. `"-No Group-"` is currently hard-coded; T2 must add a new string key via `docs/ui/new-strings-T2.md` or reuse an existing key.
- Long names: `DT_END_ELLIPSIS` + tooltip with the full name.
- Folder icons: 16px vector glyph tinted `TextSecondary`/`Accent`; if raster is kept, provide a DPI/tint variant.
- Breadcrumb (inside a group): back button 24×24 `TextSecondary` hover `Hover`; group title `FontBodyStrong` `TextPrimary`; 1px `Border` bottom separator; 20px top offset for the list is replaced by the 34px row/toolbar token (T2 decides within 20–28px and documents it).
- Keyboard and context menu behavior unchanged.

### 10.7 Bottom command bar

- Height 40, `SurfaceAlt`, top border 1px `Border`.
- Left: group button 28×28 at `(6, 6)`; icon 20px `TextSecondary`; active group state `Accent`.
- Center: search field (flex) from `x = 6+28+8 = 42` to `x = contentW-42`; height 34 at `y=3`.
- Right: system menu 28×28 at `(contentW-34, 6)`; icon 20px.
- Optional additional actions (if any) use the same 28px size and 4px gaps; do not add new commands in T2.
- Hover/pressed/focus states as §10.9.
- Tooltips are mandatory and localized.

### 10.8 Empty state / no results

- Center in the list viewport (excluding bottom bar); max text width 240.
- Icon: 40px search-off glyph, `TextSecondary` (or `Accent` at 40% if a tint is available; do not rely on alpha alone).
- Title: `FontTitle` 14px semibold `TextPrimary`, e.g. localized "No results".
- Body: `FontBody` 13px `TextSecondary`, includes the query: `There are no results for "<query>"` using the existing `NoSearchResults` string.
- Action: secondary button 30px high, "Clear search" (new localized string; `docs/ui/new-strings-T2.md`). If the search is empty (e.g. starred view empty), show a different body and keep the action only if meaningful.
- Vertical spacing: 16 (top), 8 (icon→title), 8 (title→body), 16 (body→button).
- Focus: the action is reachable by Tab; Enter activates; Esc still closes/clears as today.
- Do not use a custom animated illustration; a static glyph is enough. Optional 160ms fade if animations are enabled.

### 10.9 Buttons and icon buttons

| State | Text button | Primary button | Icon button |
|---|---|---|---|
| Default | `SurfaceAlt` fill, 1px `Border`, `TextPrimary` | `AccentFill`, `AccentOn` | transparent, `TextSecondary` glyph |
| Hover | `Hover` fill, `BorderStrong` | `AccentFillHover` | `Hover` fill, `TextPrimary` |
| Pressed | `Selection` fill | `AccentFillPressed` | `Selection` fill |
| Focus | 2px `FocusRing` inside/outside | 2px `FocusRing` with 1px offset | 2px `FocusRing` |
| Disabled | `SurfaceAlt`, `TextDisabled`, no hover | `SurfaceAlt`, `TextDisabled` | `TextDisabled`, no hover |

- Height 30 (text) / 28 (icon); radius 6; padding x 14 (text) / 4 (icon); minimum width 76 (text).
- Destructive text: `Danger` text on `SurfaceAlt`; hover may use `DangerFill` + `DangerOn`.
- Default dialog button: add a 1px `Accent` border even when not focused (Windows convention) in addition to the focus ring when focused.
- Icon buttons must have a tooltip and accessible name.

### 10.10 Inputs, combos, checkboxes, radios, hotkeys

- Edit/combo: height 30, radius 6, `Surface`, 1px `Border`; focus 2px `FocusRing`; padding x 8; placeholder `TextSecondary`; disabled `SurfaceAlt`/`TextDisabled`; read-only `SurfaceSunken`.
- Combo drop-down: 16px chevron `TextSecondary`; popup list `Surface`, item height 28, hover `Hover`, selected `Selection`.
- Checkbox/radio: 16×16; unchecked `Surface` + 1px `BorderStrong`; checked `AccentFill` + `AccentOn` glyph; focus ring 2px; label `FontBody` 13px; 8px gap.
- Hotkey control (`msctls_hotkey32`): keep native; 28–30px height; theme border/background; do not break Win-key checkbox behavior.
- Number edit: keep native spin/validation; theme border/background.
- Error state: `Danger` border + icon + message; not color-only.

### 10.11 Tabs (property sheet)

- Tab strip height 36; page background `Canvas`; content `Surface` if cards are used.
- Tab: padding x 12, label `FontBody` 13px; inactive `TextSecondary`; hover `SurfaceAlt`; active `TextPrimary` + 2px `Accent` underline; focus ring.
- Keep page order and switching semantics. Do not add/remove pages.
- Implementation choice: owner-draw the native `SysTabControl32` or use an MFC visual manager. **Recommendation:** owner-draw the native tab (less risk, no Feature Pack dependency). T4a must justify the choice in `docs/ui/FOUNDATION-NOTES.md` or its report.

### 10.12 Group boxes and section headers

- 1px `Border`, radius 8, padding 12; label `FontSecondary` 12px semibold `TextSecondary` sitting on a `Canvas` gap in the border (or a simple top rule + label).
- No `SS_ETCHEDFRAME`/3D group boxes. If owner-drawing the rounded border is too risky, use a top rule + label as a fallback; do not keep the 3D etched look.
- Section gap 16; field gap 8.

### 10.13 Dialogs and property pages

- Background `Canvas`; optional `Surface` cards with radius 10 and 1px `Border`.
- Padding 16; title `FontDialogTitle`; body/labels `FontDialogBody` 12px; statics transparent over `Canvas`.
- Buttons: height 30, right-aligned, preserve OK/Cancel order and default button; width auto-sized to text + 2×padding, minimum 76.
- Lists/trees: `Surface` bg, 28px rows, `Selection`/`Hover`, header `SurfaceAlt` 28px, `FontSecondary` semibold.
- Runtime layout: adjust in `OnInitDialog`/`OnSize`; never edit the RC template (T1 owns RC). Use runtime IDs from `src/Ui/UiIds.h` for new controls (T4a); T4b uses local constants.
- Long localized strings: measure and reflow; if a button cannot fit, grow it and shift neighbors, or wrap the label; never clip.
- Dark mode: apply `CDarkMode::ApplyToWindow` to the page and children; fall back to native colors if any call fails.
- Preserve DDX/DDV, modal behavior, validation, and file/network dialogs.

### 10.14 Menus

- P0: native `CMenu`. Text must be readable in dark mode. Best-effort dark support via isolated `CDarkMode::ApplyToMenus` (uxtheme `SetPreferredAppMode`/`AllowDarkModeForWindow`/`SetWindowTheme`, runtime-probed). If unsupported, leave native system menus; do not break commands.
- P3 (optional): fully owner-drawn menu window with `MenuItemHeight` 28, padding 12, gutter 28, radius 8. Do not do this in P0/P1.
- Keep all command IDs, accelerators, check marks, and submenus. Menu text must not be truncated; Windows handles width.
- Tray menu follows the same rule; the tray icon is OS-managed.

### 10.15 Description / tooltip window

- `RadiusPanel` 10, `Surface` fill, 1px `Border`, DWM shadow; padding `TooltipPadding` 12.
- Header: `FontSecondary` 12px semibold `TextSecondary`; body: `FontBody` 13px `TextPrimary`; metadata: `FontCaption` 11px `TextSecondary`.
- Rich text/images keep their existing behavior; image viewer gets radius 6, 1px `Border`, checkerboard for alpha.
- Options button 28×28, icon 16, hover/pressed/focus as §10.9.
- Caption keeps the 34px token; no 19px font.

### 10.16 Other windows (P2/P3)

- `CProgressWnd`, `CFileTransferProgressDlg`, `CNoDbFrameWnd`, QR viewer, clip editor: apply the same tokens/typography/background/buttons. Do not change behavior. T4b owns dialogs; editor/QR/no-DB may need coordinator approval if outside listed ownership.

---

## 11. State matrix (summary)

| State | Fill | Text/icon | Border/ring |
|---|---|---|---|
| Default | Surface/SurfaceAlt | TextPrimary | 1px Border |
| Hover | Hover | TextPrimary | BorderStrong (controls) |
| Pressed | Selection | TextPrimary | none/1px |
| Selected | Selection | SelectionText | left 3px Accent |
| Selected + focus | Selection | SelectionText | 2px FocusRing |
| Focus only | unchanged | unchanged | 2px FocusRing |
| Disabled | SurfaceAlt | TextDisabled | 1px Border |
| Read-only | SurfaceSunken | TextPrimary | 1px Border |
| Error | Surface | TextPrimary | 1px Danger + icon/message |
| Pasted | — | — | 3px Pasted bar + check glyph |
| Pinned | — | Pinned icon | — |
| Connected | — | Accent dot | — |
| Disconnected | — | Danger dot | — |

Rules:

- Hover must never steal selection or focus.
- Keyboard focus must be visible even when the mouse is elsewhere.
- Do not use a 1px border color change as the only focus indicator.

---

## 12. DPI rules

1. Every metric token is logical at 96 DPI. Layout and painting call `CDPI::Scale` (or a cached scaled value per DPI).
2. Fonts: negative pixel height = `-dpi.Scale(px)`; rebuild on `WM_DPICHANGED`; do not reuse a font from another monitor.
3. Borders: subtle `Border` uses 1 physical px at all DPI (avoids a heavy 2px line at 150%). Focus rings and accent bars use `Scale(2)`/`Scale(3)`.
4. Icons: pick the nearest asset bucket (16/20/24); vector glyphs scale exactly. Never upscale a 16px raster to 24px.
5. List row height: compute from the actual font metrics at the current DPI + token padding; do not hard-code 50.
6. Window size persistence: store unscaled logical size (`UnScale`), as today; clamp to minimum after scaling.
7. Cache scaled metrics/brushes per DPI and invalidate on `WM_DPICHANGED`; do not allocate per paint.
8. Dialog templates: MFC handles DLU scaling; after `OnInitDialog`, measure and reflow controls that would clip at 150%/200%.
9. Test matrix (when a toolchain exists): 100%, 150%, 200%; move window across monitors with different DPI; restart and verify saved size.

---

## 13. Accessibility

### 13.1 Contrast

- Body text ≥ 4.5:1; large text (≥18.66px bold / 24px regular) ≥ 3:1; icons/UI boundaries ≥ 3:1.
- The token table in §3 is the source of truth. Calculated ratios:
  - Light `TextPrimary`/Canvas 15.58:1; `TextSecondary`/Canvas 5.63:1; `Accent`/Surface 5.17:1; white/`AccentFill` 5.17:1; `Danger`/Surface 5.17:1.
  - Dark `TextPrimary`/Canvas 15.35:1; `TextSecondary`/Canvas 7.55:1; `Accent`/Canvas 6.30:1; `AccentOn`/`AccentFill` 5.85:1; `Danger`/Canvas 6.14:1.
- Disabled text is exempt but must remain legible (`TextDisabled` ≥ 3.4:1 on Canvas in both themes).
- Never use a user's raw OS accent without validation (§3.3).

### 13.2 High contrast

- Detect via `SystemParametersInfo(SPI_GETHIGHCONTRAST, ...)` on theme load and on theme/settings changes.
- When active:
  - `Canvas = GetSysColor(COLOR_WINDOW)`, `Surface = COLOR_WINDOW`, `SurfaceAlt = COLOR_3DFACE`, `Border = COLOR_WINDOWFRAME`, `TextPrimary = COLOR_WINDOWTEXT`, `TextSecondary = COLOR_WINDOWTEXT` (use weight/size instead of a lighter color), `TextDisabled = COLOR_GRAYTEXT`, `Selection = COLOR_HIGHLIGHT`, `SelectionText = COLOR_HIGHLIGHTTEXT`, `Accent = COLOR_HIGHLIGHT`, `FocusRing = COLOR_HIGHLIGHT`, `Danger = COLOR_WINDOWTEXT` (plus icon/text).
  - No alpha, no tinted icons, no custom-drawn focus rectangle color; use `DrawFocusRect` or a solid `COLOR_HIGHLIGHT` ring.
  - Let native controls use the system high-contrast theme; do not force `SetWindowTheme(..., L"DarkMode_Explorer")` or dark mode.
  - Disable fades/slides.
- High contrast must be verified with an actual high-contrast theme; until then mark UNVERIFIED.

### 13.3 Keyboard and focus

- Preserve every existing path: hotkey open/hide, arrows, PageUp/PageDown, Home/End, Enter, Shift+Enter, Ctrl+1..0, F3/Shift+F3, Tab/Ctrl+Tab, Esc, F7, Delete, Alt+Enter, menu key, Ctrl+AppMenu.
- Tab order must follow visual order (caption buttons → search → list → bottom bar).
- Focus ring on every interactive control; never `ShowFocusRect(FALSE)` without a replacement.
- The search field must keep focus while the list is navigated per existing behavior; verify after any repaint change.

### 13.4 Screen readers / UI Automation

- Every icon-only button needs a localized tooltip **and** an accessible name (window text or `WM_GETOBJECT`).
- Owner-drawn list items must expose their display text via the list's `LVN_GETDISPINFO`/UIA provider; verify with Narrator or Inspect. If UIA is broken today, do not regress it; report as UNVERIFIED.
- The empty state's action must be a real button, not a clickable static.
- Do not use `CStatic` for interactive content; use `CButton`/owner-draw with `WS_TABSTOP`.

### 13.5 IME

- Never intercept `WM_IME_*` or draw over the composition window.
- If the search edit is custom-painted, test with Chinese (Microsoft Pinyin) and Japanese IME: composition string, candidate window position, caret, committed text, selection, Backspace, Esc.
- Prefer native edit text rendering plus parent-drawn chrome. If that is impossible, implement composition/caret rendering explicitly and document the test evidence.

---

## 14. Performance rules (GDI/GDI+)

1. **No per-row GDI+ object creation.** Cache `GraphicsPath`, `SolidBrush`, `Pen`, and fonts in `UiDraw`, keyed by (token, DPI, size). A list of 10k rows must not allocate per item.
2. Prefer GDI for flat fills/text; use GDI+ only for antialiased rounded shapes/icons. Reuse one `Graphics` per paint, not per row.
3. Cache tinted icon bitmaps per (glyph, size, DPI, color). Invalidate on theme/DPI change.
4. Invalidate only the changed row/control; do not repaint the whole list on hover.
5. Keep the existing image/RTF caches; do not decode thumbnails per paint.
6. Record `GetGuiResources(GR_GDIOBJECTS)` and `GR_USEROBJECTS` before/after a 5k-row scroll; no unbounded growth. T3 must report numbers (or mark UNVERIFIED if no build).
7. Scroll performance target: no visible stutter at 5k–10k rows on a typical machine; hover/selection must be immediate.
8. Theme/DPI switch: purge caches once, then repaint once.

---

## 15. Do / don't

**Do**

- Use tokens; use `CDPI::Scale`; use the shared font/paint helpers.
- Preserve every command, shortcut, setting, and localization ID.
- Draw focus rings and non-color state cues.
- Test 300×300, 420×480, and maximized/rolled-up states.
- Keep old theme XML loading; fall back gracefully.
- Keep the search at the bottom (Ditto's signature) unless the user explicitly asks to move it.

**Don't**

- Don't add Qt/Electron/WebView2/WPF/Direct2D or any new dependency.
- Don't change the database, clipboard semantics, add-in ABI, settings key semantics, language IDs, menu command IDs, or hotkeys.
- Don't hard-code new colors/sizes/fonts in `.cpp`.
- Don't use pure red/green/black semantic colors or neon strips.
- Don't draw per-row shadows/GDI+ objects.
- Don't break IME, caret, selection, drag/drop, or keyboard navigation.
- Don't force dark mode on high contrast.
- Don't edit `CP_Main.rc`, `resource.h`, `res/`, `Debug/Themes/` outside T1.
- Don't claim visual/DPI/high-contrast verification without screenshots.

---

## 16. Frozen decisions and delegated ranges

Frozen by T0:

1. Popup search stays in the bottom command bar (Ditto signature); bottom bar 40, search 34.
2. Default caption position remains right (vertical); all four positions are supported.
3. Default popup size remains 300×300; recommended comfortable size 420×480; minimum 280×220.
4. Modern themes disable zebra stripes; legacy themes keep their explicit odd/even colors.
5. Search highlight uses `HighlightBg`/`HighlightText`, not red text; old themes keep `SearchTextHighlight` via fallback.
6. Pasted state uses a 3px `Pasted` bar + check/tooltip, not a neon strip.
7. Dark primary buttons use `AccentFill #4C8DFF` with `AccentOn #0B1220` (not white).
8. Menus are native in P0/P1; dark menu styling is best-effort and isolated.
9. Property sheet tabs are owner-drawn native tabs (recommended), not a new MFC Feature Pack dependency.
10. No new user-visible strings outside `docs/ui/new-strings-<task>.md`; T5 merges.

Delegated to T1 (must document in `docs/ui/FOUNDATION-NOTES.md`):

- Exact DWM attribute probing and Win10 fallback.
- Whether vector glyphs or tinted rasters are used for each icon (prefer vectors for simple glyphs).
- Font-family runtime detection and fallback.
- How legacy colors map when both old and new keys exist (new keys win; old keys fill gaps).
- Parser optional-key behavior and log levels.
- Cache keying and DPI invalidation.
- Whether `DarkerDitto.xml` and `Modern Dark.xml` share values or one is canonical (they must not drift).

Delegated to T2/T3/T4 (must report deviations):

- Exact search text size (13–15px), group breadcrumb offset (20–28px), row padding for multi-line modes.
- Whether the search edit is native-painted or custom-painted, with IME evidence.
- Tab owner-draw implementation details.
- Any legacy theme that cannot be made coherent without changing its colors (report; do not silently alter).
