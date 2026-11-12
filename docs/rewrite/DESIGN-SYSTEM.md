# DESIGN-SYSTEM — Ditto Fluent design system

- Status: **T0 baseline, frozen for T1–T8**
- Date: 2026-11-12
- Scope: Fluent tokens, typography, metrics, popup, settings, DPI, accessibility.
- Consumes: `design/tokens.json` (source of truth, owned by T1) and its generated
  outputs for egui and React.
- Reference: `docs/ui/DESIGN-SYSTEM.md` (approved native-UI tokens) and
  `artifacts/rewrite/mockups/` (T0 mockups).
- Units: **logical pixels at 96 DPI** unless stated. At runtime multiply by
  `dpi / 96` (egui: `pixels_per_point`; CSS: browser `devicePixelRatio`).
- Non-goals: implementation code, copied WinUI resource dictionaries, legacy
  theme XML compatibility.

---

## 1. Principles

1. **Fluent semantics, Ditto values.** Use the Fluent role names and structure,
   but keep the contrast-validated values from the approved Ditto design
   system. Raw WinUI alpha resources are not copied blindly, because composited
   control fills can fail contrast on Ditto's dense surfaces.
2. **Content first, chrome last.** Clipboard text is the product. Surfaces are
   neutral; accent is reserved for focus, selection, primary actions, and one
   status indicator.
3. **Compact, not cramped.** Preserve Ditto's small footprint. The popup is a
   keyboard-first tool, not a touch app. Spacing is 4/8/12/16/24; control
   heights are 32; rows are 36.
4. **Light, dark, and high contrast are peers.** Dark is not an inversion. HC
   uses Windows system colors and must remain fully usable.
5. **State is never color-only.** Selected, pinned, pasted, copy-buffer,
   connected, error, and focus each get a shape, glyph, or weight cue.
6. **Native behavior wins.** IME, keyboard navigation, selection, drag/drop,
   tooltips, and OS accessibility are more important than pixel perfection.
7. **One token source.** `design/tokens.json` generates Rust (egui) and
   CSS/TypeScript (React) constants. No hardcoded colors or sizes in either UI.
8. **No per-row allocation.** Cache fonts, textures, icon atlases, and brushes.
   Virtualize the list; do not create a texture per frame.

Non-goals: no web UI for the popup, no animated page transitions, no per-row
shadows, no custom-drawn settings title bar, no change to the confirmed
architecture.

---

## 2. Token architecture

```
design/tokens.json                 source of truth (T1 owns)
        │
        ├── tokens.schema.json    validation
        │
        ├── cargo xtask tokens    → crates/ditto-model/src/tokens_generated.rs
        │                           (egui: color/metric/font/motion constants)
        │
        └── npm run tokens        → apps/ditto/ui/src/styles/tokens.generated.css
                                    apps/ditto/ui/src/tokens.generated.ts
```

- `design/tokens.json` is the only file a designer edits. Generated files are
  checked in, carry a "generated — do not edit" header, and are verified in CI
  (`cargo xtask tokens --check`, `npm run tokens:check`).
- Token names are dot paths: `color.canvas`, `dimension.rowHeight`,
  `font.body.size`, `radius.control`, `shadow.flyout`, `motion.fast`.
- Generated naming:
  - Rust: `tokens::color::CANVAS`, `tokens::dimension::ROW_HEIGHT`.
  - TypeScript: `tokens.color.canvas`, `tokens.dimension.rowHeight`.
  - CSS: `--ditto-color-canvas`, `--ditto-dimension-row-height`.
- Both UIs may also consume Fluent UI React v9's own tokens for stock controls
  (`--colorNeutralBackground1`, `--colorBrandBackground`, etc.). The Ditto
  tokens are mapped onto a custom Fluent theme so stock components and custom
  surfaces agree.
- HC values are **system references**, not hex literals. The generator emits a
  runtime resolver; the settings CSS uses `forced-colors` system colors; egui
  reads `GetSysColor`/`SystemParametersInfo`.

### 2.1 Token groups

| Group | Examples | Notes |
|---|---|---|
| `color` | `canvas`, `surface`, `textPrimary`, `accentFill` | Light/dark/HC |
| `dimension` | `popup.width`, `rowHeight`, `controlHeight`, `pagePadding` | Logical px @96 |
| `font` | `family.ui`, `family.display`, `body.size`, `body.lineHeight` | Family, size, weight |
| `radius` | `window`, `panel`, `control`, `listItem`, `pill` | Logical px |
| `border` | `width`, `focusWidth` | 1 physical px border, 2 logical focus |
| `shadow` | `flyout`, `dialog`, `popup` | Fluent elevation |
| `motion` | `fast`, `normal`, `slow`, `easing.standard` | ms + cubic-bezier |
| `z` | `popup`, `flyout`, `dialog`, `tooltip` | Layering |
| `icon` | `size.small`, `size.medium`, `size.large` | 16/20/24 |

### 2.2 `design/tokens.json` schema

The file is JSON (UTF-8) with this shape. Every token is an object with
`$value`, `$type`, and optional `$description`/`$extensions`.

```json
{
  "$schema": "./tokens.schema.json",
  "version": "1.0.0",
  "meta": {
    "name": "Ditto Fluent",
    "units": "logical-px-96dpi",
    "modes": ["light", "dark", "hc"],
    "generated": ["rust", "css", "ts"]
  },
  "tokens": {
    "color.canvas": {
      "$type": "color",
      "$value": { "light": "#F7F8FA", "dark": "#1B1C1F", "hc": "system:Window" },
      "$description": "Window and popup background"
    },
    "dimension.rowHeight": {
      "$type": "dimension",
      "$value": { "light": 36, "dark": 36, "hc": 36 },
      "$description": "One-line clip row pitch"
    },
    "font.family.ui": {
      "$type": "fontFamily",
      "$value": {
        "light": ["Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI", "Tahoma", "Arial"],
        "dark": ["Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI", "Tahoma", "Arial"],
        "hc": ["Segoe UI", "Microsoft YaHei UI", "Tahoma", "Arial"]
      }
    },
    "shadow.popup": {
      "$type": "shadow",
      "$value": {
        "light": { "x": 0, "y": 8, "blur": 16, "color": "#00000024" },
        "dark": { "x": 0, "y": 8, "blur": 16, "color": "#00000066" },
        "hc": "none"
      }
    }
  }
}
```

Schema rules:

| Rule | Value |
|---|---|
| Token key | `group.name` or `group.name.variant`; lowercase camelCase last segment |
| `$type` | `color`, `dimension`, `fontFamily`, `fontWeight`, `fontSize`, `lineHeight`, `radius`, `shadow`, `duration`, `cubicBezier`, `number` |
| Color value | `#RRGGBB`, `#RRGGBBAA`, or `system:<WindowsSystemColor>` in HC |
| Dimension value | Number = logical px at 96 DPI; no unit strings |
| Font family | Ordered array; first installed wins |
| Shadow | Object `{x, y, blur, color}` or `none`; spread is 0 in v1 |
| Duration | Integer milliseconds |
| Aliases | A token may use `"$value": {"$ref": "color.accent"}`; the generator resolves aliases and rejects cycles |
| Alpha | Allowed for shadows/scrims; for text/fill tokens use opaque values and validate contrast |
| HC | `system:` references only for color; dimensions/typography may stay numeric |
| Unknown fields | Ignored by generators; schema validation rejects unknown `$type` |
| Mode completeness | Every token must define all three modes, or the generator falls back to `light` and emits a warning |

Generated files:

| Output | Contents |
|---|---|
| `crates/ditto-model/src/tokens_generated.rs` | `ThemeTokens` struct, one constant per token, `ThemeMode` enum, HC resolver hooks |
| `apps/ditto/ui/src/styles/tokens.generated.css` | `:root[data-theme="light"]`, `[data-theme="dark"]`, `@media (forced-colors: active)` blocks |
| `apps/ditto/ui/src/tokens.generated.ts` | Typed `tokens` object and `ThemeMode` union |
| `design/tokens.schema.json` | JSON Schema (draft 2020-12) for editors/CI |
| `design/tokens.lock.json` | Generator version + SHA-256 of `tokens.json`, for drift detection |

No generated file is edited by hand. T3/T5 consume the outputs; T1 owns the
source and generator.

---

## 3. Color tokens

The values below are the approved Ditto-validated Fluent palette. Contrast
ratios are against the stated background; text targets 4.5:1, large text 3:1,
non-text UI 3:1. The mockups use the same values.

### 3.1 Fluent role mapping

| Fluent role | Ditto token | Light | Dark | HC |
|---|---|---|---|---|
| `SolidBackgroundFillColorBase` | `color.canvas` | `#F7F8FA` | `#1B1C1F` | `system:Window` |
| `CardBackgroundFillColorDefault` | `color.surface` | `#FFFFFF` | `#24262A` | `system:Window` |
| `LayerFillColorDefault` | `color.surfaceAlt` | `#F1F3F6` | `#2C2F34` | `system:ButtonFace` |
| `ControlFillColorDefault` | `color.surfaceSunken` | `#EEF1F5` | `#202226` | `system:Window` |
| `ControlStrokeColorDefault` | `color.border` | `#E3E6EC` | `#383B41` | `system:WindowText` |
| `ControlStrokeColorSecondary` | `color.borderStrong` | `#CFD3DB` | `#4A4E56` | `system:WindowText` |
| `TextFillColorPrimary` | `color.textPrimary` | `#1B1F24` | `#F2F3F5` | `system:WindowText` |
| `TextFillColorSecondary` | `color.textSecondary` | `#5C6470` | `#A7ADB7` | `system:WindowText` |
| `TextFillColorDisabled` | `color.textDisabled` | `#7E8692` | `#6C727C` | `system:GrayText` |
| `AccentTextFillColorPrimary` | `color.accent` | `#2563EB` | `#6C9BFF` | `system:Highlight` |
| `AccentFillColorDefault` | `color.accentFill` | `#2563EB` | `#4C8DFF` | `system:Highlight` |
| `TextOnAccentFillColorPrimary` | `color.onAccent` | `#FFFFFF` | `#0B1220` | `system:HighlightText` |
| `SubtleFillColorSecondary` | `color.hover` | `#F0F3F8` | `#2F3238` | `system:Highlight` (hover via border/weight) |
| `SubtleFillColorSelected` | `color.selection` | `#E8F0FF` | `#2E3A52` | `system:Highlight` |
| `FocusStrokeColorOuter` | `color.focus` | `#2563EB` | `#6C9BFF` | `system:Highlight` |
| `SystemFillColorCritical` | `color.danger` | `#C43C3C` | `#FF6B6B` | `system:WindowText` + error glyph |
| `SystemFillColorSuccess` | `color.pasted` | `#1F7A4D` | `#4CC38A` | `system:WindowText` + check glyph |
| `SystemFillColorCaution` | `color.pinned` | `#8A5A00` | `#E0A94A` | `system:WindowText` + pin glyph |

### 3.2 Extended tokens

| Token | Light | Dark | HC | Use |
|---|---|---|---|---|
| `color.accentHover` | `#1D4ED8` | `#82AAFF` | `system:Highlight` | Accent text/icon hover |
| `color.accentPressed` | `#1E40AF` | `#5A87E6` | `system:Highlight` | Accent pressed |
| `color.accentFillHover` | `#1D4ED8` | `#6BA0FF` | `system:Highlight` | Primary button hover |
| `color.accentFillPressed` | `#1E40AF` | `#3C7AEA` | `system:Highlight` | Primary pressed |
| `color.selectionText` | `#1B1F24` | `#F2F3F5` | `system:HighlightText` | Text on selection |
| `color.highlightBg` | `#E8F0FF` | `#2E3A52` | `system:Highlight` | Search match |
| `color.highlightText` | `#1B1F24` | `#F2F3F5` | `system:HighlightText` | Search match text |
| `color.closeHoverBg` | `#C42B1C` | `#C42B1C` | `system:Highlight` | Close button hover |
| `color.closeHoverOn` | `#FFFFFF` | `#FFFFFF` | `system:HighlightText` | Close glyph hover |
| `color.scrollThumb` | `#C4C9D2` | `#4E535C` | `system:WindowText` | Scrollbar rest |
| `color.scrollThumbHover` | `#9AA1AC` | `#6C727C` | `system:WindowText` | Scrollbar hover |
| `color.scrim` | `#00000066` | `#00000099` | `none` | Modal scrim (settings dialogs) |
| `color.errorBg` | `#FDF2F2` | `#3A1F1F` | `system:Window` | Error strip background |
| `color.warningBg` | `#FFF8E5` | `#3A321F` | `system:Window` | Warning strip background |
| `color.successBg` | `#EFF9F3` | `#1F2E26` | `system:Window` | Success strip background |

Contrast notes (calculated from the values, not measured on screen):

- Light `textPrimary` on `canvas`: 15.58:1; on `surface`: 16.56:1.
- Light `textSecondary` on `canvas`: 5.63:1; on `surface`: 5.98:1.
- Light `accent` on `surface`: 5.17:1; white on `accentFill`: 5.17:1.
- Dark `textPrimary` on `canvas`: 15.35:1; `textSecondary` on `canvas`: 7.55:1.
- Dark `accent` on `canvas`: 6.30:1; `onAccent` on `accentFill`: 5.85:1.
- All states also use a glyph/shape; color alone is never the only signal.

### 3.3 Accent behavior

- Default: follow the Windows accent (`HKCU\Software\Microsoft\Windows\DWM\ColorizationColor` or the documented system accent API).
- Validate the candidate:
  - Light: accent on `surface` ≥ 4.5:1 **and** white on accent fill ≥ 4.5:1.
  - Dark: accent on `canvas` ≥ 4.5:1 **and** `onAccent` on accent fill ≥ 4.5:1.
- If validation fails, fall back to the Ditto accent (`#2563EB` light, `#6C9BFF`/`#4C8DFF` dark) and log at debug level.
- Never use a raw OS accent for large fills without validating text-on-fill.
- Settings offers System / Ditto Blue / Custom (custom is validated before apply).

### 3.4 Forbidden colors

- No pure red `#FF0000` for search highlight; no neon green `#00FF00` for
  pasted; no pure black icons on dark surfaces.
- No gradients in the popup shell, list, buttons, or settings surfaces.
- No color-only status: pinned/pasted/copy-buffer/error always have a glyph or
  text.
- No new hardcoded colors outside `design/tokens.json` (except HC system
  references and test fixtures).

---

## 4. Typography

Font family resolution order:

1. `Segoe UI Variable Text` (Windows 11).
2. `Segoe UI Variable` / `Segoe UI`.
3. `Microsoft YaHei UI` (CJK fallback).
4. `Tahoma`.
5. `Arial`.

Display text (≥ 20 px) may use `Segoe UI Variable Display` first. Monospace:
`Cascadia Mono` → `Consolas` → `Courier New`.

| Token | Size / line | Weight | Use |
|---|---|---|---|
| `font.display` | 28 / 36 | 600 | Settings empty state, About hero |
| `font.title` | 20 / 28 | 600 | Settings page title |
| `font.subtitle` | 14 / 20 | 600 | Settings section title, popup empty-state title |
| `font.body` | 14 / 20 | 400 | Settings body, labels |
| `font.bodyStrong` | 14 / 20 | 600 | Selected labels, group breadcrumb |
| `font.popupBody` | 13 / 18 | 400 | Popup clip text, menu items |
| `font.popupStrong` | 13 / 18 | 600 | Popup selected label, breadcrumb |
| `font.secondary` | 12 / 16 | 400 | Helper text, metadata line |
| `font.caption` | 11 / 14 | 400 | Timestamps, first-ten badge, counts |
| `font.mono` | 12 / 16 | 400 | Hotkey chords, hashes, diagnostics |

Rules:

- Two weights only: 400 regular and 600 semibold. 700 is reserved for the About
  version number if needed.
- Do not use italic for group breadcrumb; use `font.popupStrong` +
  `color.textSecondary`.
- Clip text uses `font.popupBody`; chrome uses `font.secondary`/`font.caption`.
- CJK text must not be clipped: allow 1.2× line height for Chinese and test at
  100/150/200% with the longest localized strings.
- Settings body is 14 px; the old 8 pt `MS Shell Dlg` is not carried forward.
- Hotkey chords use `font.mono` with a subtle `surfaceAlt` chip, not a custom
  bitmap font.

---

## 5. Metrics, radius, elevation, motion

### 5.1 Spacing scale

`2 / 4 / 8 / 12 / 16 / 24 / 32 / 40`. Use 4/8/12 inside the popup; 8/16/24 in
settings. Do not invent 5/7/10/13/18/20 for layout. 10 is allowed only for a
panel/tooltip radius.

### 5.2 Core metrics

| Token | Value | Notes |
|---|---|---|
| `dimension.popup.width` | 420 | Default logical width |
| `dimension.popup.height` | 480 | Default logical height |
| `dimension.popup.minWidth` | 360 | |
| `dimension.popup.minHeight` | 320 | |
| `dimension.popup.maxWidth` | 720 | Also clamp to 90% work area |
| `dimension.popup.maxHeight` | 800 | Also clamp to 90% work area |
| `dimension.titleHeight` | 40 | Minimal top title area |
| `dimension.breadcrumbHeight` | 32 | Only when in a group/search |
| `dimension.searchHeight` | 48 | Bottom search container |
| `dimension.searchFieldHeight` | 32 | Inner text field |
| `dimension.rowHeight.oneLine` | 36 | Default |
| `dimension.rowHeight.twoLine` | 54 | |
| `dimension.rowHeight.threeLine` | 72 | |
| `dimension.rowHeight.maxLines` | 5 | 108 px |
| `dimension.rowInsetX` | 4 | Row pill inset |
| `dimension.rowInsetY` | 2 | |
| `dimension.indexGutter` | 28 | First-ten badge / format icon |
| `dimension.iconSmall` | 16 | |
| `dimension.iconMedium` | 20 | |
| `dimension.iconLarge` | 24 | |
| `dimension.controlHeight` | 32 | Inputs, buttons, combos |
| `dimension.buttonHeight` | 32 | |
| `dimension.toggleHeight` | 20 | |
| `dimension.scrollThumbRest` | 6 | Width |
| `dimension.scrollThumbHover` | 10 | Width |
| `dimension.scrollThumbMin` | 24 | Length |
| `dimension.settings.navWidth` | 208 | |
| `dimension.settings.headerHeight` | 64 | |
| `dimension.settings.pagePadding` | 24 | |
| `dimension.settings.cardRadius` | 8 | |
| `dimension.settings.maxContentWidth` | 720 | |
| `dimension.dialog.padding` | 24 | |
| `dimension.tooltip.maxWidth` | 320 | |

### 5.3 Radius and borders

| Token | Value | Use |
|---|---|---|
| `radius.window` | 8 | Popup/settings outer |
| `radius.panel` | 8 | Flyouts, cards, tooltips |
| `radius.control` | 4 | Inputs, buttons, tabs |
| `radius.listItem` | 4 | Rows, menu items |
| `radius.pill` | 999 | Badges/chips |
| `border.width` | 1 physical px | Use `1 / scale` logical for crispness |
| `border.focusWidth` | 2 logical px | Focus ring, drawn outside the control |

### 5.4 Elevation and shadows

| Token | Light | Dark | Use |
|---|---|---|---|
| `shadow.popup` | `0 8 16 #00000024` | `0 8 16 #00000066` | Popup window only |
| `shadow.flyout` | `0 8 16 #00000024` | `0 8 16 #00000066` | Group flyout, menus |
| `shadow.dialog` | `0 16 32 #00000029` | `0 16 32 #00000080` | Settings dialogs |
| `shadow.tooltip` | `0 4 8 #0000001F` | `0 4 8 #00000066` | Tooltip/description |
| HC | none | none | Use 1px `WindowText` border |

- No per-row shadows. Use `hover`/`selection` fills.
- Popup uses a 1px `border` plus shadow on Win11. On Win10, square corners and
  no synthetic shadow; the native frame is enough.
- Settings uses the native window frame; Mica is optional and must not reduce
  text contrast.

### 5.5 Motion

| Token | Value | Use |
|---|---|---|
| `motion.fast` | 100 ms | Hover/press color changes, scrollbar fade |
| `motion.normal` | 200 ms | Flyout open, settings section reveal |
| `motion.slow` | 300 ms | Dialog open (settings only) |
| `motion.easing.standard` | `cubic-bezier(0.33, 0, 0.67, 1)` | Standard |
| `motion.easing.decelerate` | `cubic-bezier(0.1, 0.9, 0.2, 1)` | Enter |
| `motion.scrollbarAutoHide` | 800 ms | Scrollbar fade |

- Popup show: present the first frame immediately. An optional 100 ms opacity
  fade starts **after** the first frame and must not delay input. No scale or
  slide that changes the measured first-frame time.
- Never animate list layout while scrolling. Never animate the caret.
- Disable non-essential motion when:
  - `SPI_GETCLIENTAREAANIMATION` is false,
  - high contrast is active,
  - the session is remote (`SM_REMOTESESSION`),
  - or the user chooses Reduce motion in settings.

---

## 6. Iconography

- Primary font: `Segoe Fluent Icons` (Windows 11). Fallback: `Segoe MDL2
  Assets`. Critical icons are also embedded as SVG so rendering does not depend
  on a font version.
- Sizes: 16 (inline), 20 (buttons), 24 (empty state/prominent).
- Style: filled Fluent glyphs, single color from a token; no black raster
  assets; no per-state bitmap variants (tint at draw time or use separate SVG).
- Icon color follows the text token of its context; disabled uses
  `textDisabled`; danger uses `danger`.
- Required glyph set:
  `Search`, `Close`, `ChevronLeft`, `ChevronRight`, `ChevronDown`, `Pin`,
  `Unpin`, `Delete`, `Copy`, `Cut`, `Paste`, `Folder`, `FolderOpen`, `Image`,
  `Text`, `File`, `Checkmark`, `Error`, `Info`, `Warning`, `More`, `Add`,
  `Edit`, `Filter`, `Sort`, `Refresh`, `OpenFolder`, `Backup`, `Restore`,
  `Import`, `Export`, `Language`, `Shield`, `Settings`, `Dismiss`.
- The group flyout and breadcrumb use chevrons, not triangles.
- The first-ten badge is a number chip, not an icon font glyph.

---

## 7. Popup specification

Default: **420 × 480 logical px** at 96 DPI. Minimum 360 × 320. Resizable with a
4px edge grip; size persisted in logical px per monitor-independent profile and
clamped to the work area.

### 7.1 Layout

```
┌────────────────────────────────────────────┐  0
│ [icon] Ditto              [pin] [gear] [x] │  40   title area
├────────────────────────────────────────────┤
│ ‹ All clips / Work / Project X      12     │  32   breadcrumb (optional)
├────────────────────────────────────────────┤
│  1   Lorem ipsum dolor sit amet…      📌   │  36   row 1 (selected)
│  2   https://github.com/…             ✓    │  36   row 2 (pasted)
│  3   ▣ screenshot.png                  B2  │  36   row 3 (copy buffer)
│      … virtualized list …                   │
│                                            │
├────────────────────────────────────────────┤
│  Preview (F3)                              │ 160   optional preview
├────────────────────────────────────────────┤
│  🔍 Search clips…                    1,204 │  48   bottom search
└────────────────────────────────────────────┘  480
```

- Outer: `radius.window` 8, 1px border, `shadow.popup`.
- Background: `color.canvas`; list rows sit on `color.surface` or `canvas`
  depending on density; no alternating row colors by default (legacy zebra is
  an Appearance option).
- No right-side vertical caption. The title is a minimal top strip.
- The search is always at the bottom, preserving Ditto muscle memory.

### 7.2 Title area (40)

| Element | Spec |
|---|---|
| Container | Height 40, padding X 8, drag region |
| App icon | 16×16 at x=8 |
| Title | `font.subtitle` (14/20, 600), `textPrimary`, x=32 |
| Status dot | 6px, only for connected/paused/error + tooltip; never color-only |
| Buttons | 32×32 hit target, 16px glyph, radius 4, 4px gap, right padding 4 |
| Pin button | Toggles always-on-top; pressed state uses `selection` fill + `accent` glyph |
| Settings button | Opens Settings; disabled while migration is running |
| Close button | Hides popup; hover `closeHoverBg` with white glyph |
| Drag | Drag anywhere in the strip except buttons |

### 7.3 Breadcrumb (32, optional)

- Visible when inside a group or when a search is active.
- Back button 28×28, chevron 16; path segments 13/18 semibold; separator `/` in
  `textSecondary`; current segment `textPrimary`.
- Right side: item count or search result count, 11/14, `textSecondary`.
- Clicking a segment navigates to that group. A chevron-down at the end opens
  the group flyout.
- The breadcrumb never wraps; middle segments collapse to `…` with a tooltip.

### 7.4 Clip list

- Virtualized: render only visible rows + 3 overscan. No per-row texture or
  brush allocation.
- Row pitch: 36 for one line, 54/72/90/108 for 2–5 lines (Appearance setting;
  default one line).
- Row pill: inset X 4, inset Y 2, radius 4.
- States:
  | State | Fill | Border | Text |
  |---|---|---|---|
  | default | transparent | none | `textPrimary` |
  | hover | `hover` | none | `textPrimary` |
  | selected | `selection` | 1px `accent` if focused, else none | `selectionText` |
  | keyboard focus | selected + 2px `focus` ring inset | | |
  | pasted | transparent | left 3px `pasted` bar | `textPrimary` + check |
  | pinned | transparent | none | pin glyph `pinned` |
  | disabled/loading | transparent | none | `textDisabled` |
- Gutter (28): first-ten badge 18×18 radius 4, `surfaceAlt` fill, 11/14
  semibold; if not in the first ten, a 16px format glyph.
- Content: text 13/18, ellipsis at one line; optional second line 11/14 with
  source app, relative time, format. Newlines in the clip render as `⏎` or a
  multi-line preview according to the line setting; never expand the row
  unexpectedly while scrolling.
- Badges on the right:
  - Pinned: 14px pin glyph, `pinned`.
  - Pasted: 14px check glyph, `pasted`, plus left bar.
  - Copy buffer: 18×18 chip, `B1`..`B5`, accent border; tooltip names the slot.
  - Image: 32×32 thumbnail, radius 4, 1px border.
- Selection model: single selection by default; Ctrl+click toggles, Shift+click
  ranges; Space toggles; the visual anchor is always visible.
- Double-click pastes; single click selects; right-click opens the context menu.
- Context menu: Paste, Paste plain text, Copy, Pin/Unpin, Move to group, Copy
  to buffer, Edit, Delete. Disabled items are visibly disabled, not hidden.

### 7.5 Search (bottom, 48)

- Container 48: 8px padding; inner field 32; radius 4; 1px `border`; surface.
- Search icon 16 at left, 8px inset; placeholder `Search clips…` (localized).
- Clear button 24×24 appears when non-empty.
- Focus: 2px `focus` ring outside the field; the caret is always visible.
- IME: composition text is underlined; the candidate window is positioned at
  the caret; commit/cancel must not duplicate or lose characters.
- Right side: result count `1,204`, 11/14 `textSecondary`; while searching,
  a small indeterminate progress indicator replaces the count after 150 ms.
- Modes (menu or prefix):
  - `contains` default, case-insensitive.
  - `wildcard` (`*`, `?`) via the search options menu.
  - `regex` via the search options menu.
  - `fulltext` and `quickpaste` toggles; `/q ` and `/f ` prefixes remain
    supported for upgraders.
- Empty query = browse current group. Esc clears the query first, then closes
  the popup.

### 7.6 Preview panel (F3)

- Docked between the list and the search; default height 160, min 96, max 45%
  of the popup content height.
- Top border 1px `border`; padding 12.
- Content:
  - Text: wrapped 12/16, max 12 lines, `textSecondary` metadata row.
  - Image: fit inside the pane, 1px border, checkerboard only for transparency
    (subtle `surfaceAlt`), no distortion.
  - Color: 48×48 swatch + hex/RGB/HSL values; copy button.
  - Files: icon + path list; no execution.
- Actions: Copy, Paste, Copy plain text, Open (files only), Close.
- On popup heights below 360, the preview replaces the list area with a back
  button; the search remains visible.
- F3 toggles; Esc closes the preview before closing the popup.

### 7.7 Group flyout

- 280×360, radius 8, `shadow.flyout`, 1px border, `surface`.
- Header 40: "Groups" + close.
- Rows 32: indentation 16 per level, folder glyph, name, count; selected uses
  `selection`; drag to reorder is a Should (see `FEATURE-MATRIX.md`).
- Top entries: All clips, Ungrouped; then the tree; footer "New group…".
- Keyboard: Up/Down/Left/Right/Enter/Esc; type-ahead by name.

### 7.8 Empty, loading, error states

- Empty: centered 240 max width; 24 icon; 14/20 title; 12/16 body; optional
  action button (Clear search / Open Settings).
- Loading: skeleton rows only for the first paint; no spinner over existing
  content.
- Error: 28px strip above the search, `warningBg`/`errorBg`, icon + text +
  "Retry"; not color-only.
- Disconnected: persistent strip "Clipboard capture is paused" + "Resume"
  action.

### 7.9 Keyboard map

| Key | Action |
|---|---|
| Hotkey (default Ctrl+`) | Show/hide popup |
| Type | Search (search field focused on open) |
| Up/Down | Move list selection |
| PageUp/PageDown | Move by page |
| Home/End | First/last visible item |
| Enter | Paste selected |
| Shift+Enter | Paste as plain text |
| Ctrl+Enter | Paste all selected (multi-paste) |
| Esc | Clear search → close preview → close popup |
| Ctrl+F | Focus search and select all |
| Ctrl+P | Pin/unpin selected |
| Ctrl+G | Move to group |
| Ctrl+C | Copy selected to clipboard |
| Ctrl+Shift+C | Copy plain text |
| Delete | Delete selected (confirm if enabled) |
| F2 | Edit clip (later; disabled in v1 if not implemented) |
| F3 | Toggle preview |
| Ctrl+1..0 | Paste first-ten item |
| Alt+1..5 | Paste copy-buffer slot |
| Ctrl+, | Open Settings |
| Tab/Shift+Tab | Cycle title → list → search → title |
| Space | Toggle selection |
| Ctrl+A | Select all visible |

### 7.10 Popup state matrix

| State | Visual | Input |
|---|---|---|
| Hidden/pre-warmed | Process resident, no window | Hotkey event |
| Showing | First frame immediate; optional 100 ms fade | Accept keys after first frame |
| Visible | Normal | Full |
| Searching | Count/progress, skeleton only on first load | Keys go to search |
| No results | Empty state | Esc clears |
| Database migrating | Disabled list + progress strip | Only Close/Settings |
| Disconnected | Warning strip + Resume | Search still works on cached data |
| Error | Error strip + Retry | Read-only until retry |
| Closing | Hide immediately | Release focus to target |

---

## 8. Settings specification

Window default **900 × 640 logical**, minimum 720 × 480. Native title bar,
optional Mica on Windows 11, solid `canvas` on Windows 10. Left navigation 208
px; content area fluid, max content width 720.

### 8.1 Layout

```
┌──────────────────────────────────────────────────────────────┐
│  Ditto Settings                                      — □ ×    │ 64 native/title
├───────────────┬──────────────────────────────────────────────┤
│ ⚙ General     │  Appearance                                  │ 20/28
│ 🎨 Appearance │  Theme, density, popup size, motion…         │ 12/16
│ ⌨ Hotkeys     │                                              │
│ 📋 Clipboard  │  ┌ Card ──────────────────────────────────┐  │
│ 📁 Groups     │  │ Theme          [System ▾]              │  │
│ 🧷 Copy Buf.  │  │ Density        (•) Comfortable ( ) …   │  │
│ 🗄 Data        │  └────────────────────────────────────────┘  │
│ 🌐 Languages  │                                              │
│ ℹ About       │                                              │
└───────────────┴──────────────────────────────────────────────┘
```

- Nav: 208 wide, 8 padding; item height 40, radius 4, icon 16 + label 14/20;
  selected uses `selection` + a 3px accent bar at the left; hover `hover`;
  focus ring 2px.
- Header: 64 high, padding X 24; page title `font.title` 20/28 semibold;
  description `font.secondary` 12/16.
- Content: padding 24; vertical section gap 24; cards `surface` + 1px border +
  radius 8 + padding 16; section title `font.subtitle` 14/20.
- Field rows: label 14/20, helper 12/16 `textSecondary`, control height 32;
  label column min 160, control column flexible; wrap long labels instead of
  clipping.
- Buttons: primary 32 high, radius 4, accent fill + onAccent; secondary
  surface + border; destructive danger fill with confirmation.
- Tables (history on Data page): header 32, row 40, hover, selected; virtualize
  beyond 200 rows; column sort indicators.
- Dialogs: 480 max width, radius 8, `shadow.dialog`, scrim; focus trap; Esc
  cancels; primary action is first in tab order.
- Status: a 28px `MessageBar` at the top of the content for save/reconnect
  errors; `aria-live="polite"`; do not use a toast that disappears before it
  can be read.

### 8.2 Pages

| Page | Required controls | Notes |
|---|---|---|
| **General** | Run at startup; start minimized; show tray icon; language (EN/ZH); startup message; default paste mode; check for updates (disabled "coming later") | Autostart writes HKCU Run; language is restart-required |
| **Appearance** | Theme System/Light/Dark/High contrast; accent System/Ditto/Custom; density Comfortable/Compact; popup width/height; position at caret/cursor/previous; lines per clip 1–5; font scale 100–150%; preview off/auto/F3; animations; always-on-top | Validate accent contrast; clamp popup size |
| **Hotkeys** | Open popup; paste plain text; first ten toggle; copy buffers 5 × copy/paste/cut; reset all; conflict banner | Conflict shows the conflicting app if known; no silent hook fallback |
| **Clipboard** | Capture formats (text, HTML, RTF, images, files, custom); max clip size; max entries; expire days; dedupe; ignore CF_DIB; include/exclude apps; paste delay; send paste after selection; restore clipboard; admin/elevated paste; multi-paste separator | Include/exclude uses process names, not paths |
| **Groups** | Tree add/rename/delete/move; default group; show grouped clips in main list; prompt for new group names; group order | Deleting a group moves clips to Ungrouped after confirmation |
| **Copy Buffers** | 5 slots: name, copy/paste/cut hotkeys, current clip preview, clear, assign selected clip, play sound | Slots persist; clear only unsets the reference |
| **Data** | Data root; DB size; backup now; restore; import; export; optimize; delete old; delete all; history manager (search/filter/preview/edit/delete/group/pin/buffer); open data folder; diagnostics | History manager is a management table, not the daily popup |
| **Languages** | English; 简体中文; restart note; legacy language packs are not migrated | i18n keys support future languages |
| **About** | Version, build SHA, protocol/schema version, GPLv3, source URL, third-party notices, diagnostics, check for updates (disabled) | No telemetry |

### 8.3 Fluent UI React v9 mapping

- `FluentProvider` + a custom theme created with `createLightTheme`/
  `createDarkTheme`; brand ramp derived from the validated accent.
- Stock components used: `Nav`/`TabList`, `Button`, `ToggleButton`, `Switch`,
  `Input`, `Textarea`, `Dropdown`, `Combobox`, `Slider`, `SpinButton`,
  `Table`, `Dialog`, `Tooltip`, `MessageBar`, `ProgressBar`, `Tree`, `Field`,
  `Card`, `Divider`, `Badge`, `Tag`.
- Custom surfaces use Ditto CSS variables from `tokens.generated.css`; stock
  controls use Fluent tokens mapped to the same values. Do not mix an
  unmodified default Fluent theme with the Ditto palette.
- HC: use `teamsHighContrastTheme` and `forced-colors` overrides; verify every
  custom surface has a system border.
- The webview is a pure view; it never reads SQLite or the filesystem. All
  mutations go through Tauri commands.

### 8.4 Settings state

| State | Visual | Behavior |
|---|---|---|
| Loading | Skeleton cards + disabled nav | Wait for `GetSettings` |
| Ready | Normal | Writes enabled |
| Dirty | Primary "Apply" enabled, "Discard" visible | Optional auto-save per control; never lose a value silently |
| Saving | Button busy, inputs disabled | One in-flight patch |
| Saved | Inline check + timestamp | MessageBar announces |
| Restart required | Persistent `MessageBar` with "Restart later" | Language/data root/autostart path |
| Reconnecting | Banner "Reconnecting…", writes disabled | Retry with backoff; reads from last snapshot |
| Error | `MessageBar` error + Retry | Do not drop the user's edits |

---

## 9. DPI, multi-monitor, and window placement

- All dimensions are logical px at 96 DPI. Never hardcode physical pixels.
- egui: `pixels_per_point = dpi / 96`; reload the font atlas on
  `ScaleFactorChanged`; relayout and invalidate textures. Do not scale fonts by
  drawing into a transformed layer.
- React/CSS: use CSS px; the WebView2 maps to device pixels. Test at 100%, 150%,
  200% and with `devicePixelRatio` 1.25/1.5/2.
- Borders: use 1 physical px (`1 / scale` logical) for hairlines; focus rings
  2 logical px. At 150%, round to the nearest physical pixel consistently.
- Multi-monitor:
  - Position the popup at the caret (default), cursor, or previous position.
  - Use the monitor that contains the caret/target; never straddle monitors.
  - Clamp to the monitor work area with an 8px margin; if the popup does not
    fit, shrink to min size, then move.
  - Recompute on `WM_DPICHANGED`/`ScaleFactorChanged`; do not keep stale
    physical coordinates.
- Settings window remembers its logical size/position per monitor profile; if
  the monitor is gone, center on the primary monitor.
- Test matrix: 100/150/200%, mixed-DPI dual monitor, taskbar top/bottom/left/
  right, 125% fractional scaling, RTL is out of scope for v1 but must not crash.

---

## 10. Accessibility

### 10.1 Contrast and state

- Text: 4.5:1 normal, 3:1 large (≥ 18.66 px bold or ≥ 24 px regular).
- Non-text UI (borders, focus, icons): 3:1 against adjacent colors.
- Focus ring: 2px `focus` with a 1px inner `canvas` gap for separation.
- State is never color-only: pinned/pasted/copy-buffer/error have glyphs or
  text; selected rows have a fill plus focus ring when focused.

### 10.2 High contrast

- Detect with `SystemParametersInfo(SPI_GETHIGHCONTRAST)` and
  `HIGHCONTRASTW.dwFlags & HCF_HIGHCONTRASTON`; CSS uses
  `@media (forced-colors: active)`.
- Use system colors (`Window`, `WindowText`, `Highlight`, `HighlightText`,
  `ButtonFace`, `GrayText`); do not use custom brand fills.
- Remove shadows/gradients; ensure 1px system borders around every surface.
- Icons/glyphs remain visible; disabled text uses `GrayText`.
- Test Windows HC Black, HC White, and HC #1/#2.

### 10.3 Keyboard and focus

- Every action is reachable by keyboard. No focus traps except modal dialogs,
  which trap intentionally and restore focus on close.
- Visible focus on all interactive elements; the popup opens with the search
  focused, and Tab order is title → list → search → title.
- The first-ten and copy-buffer hotkeys are global; settings must show
  conflicts and allow rebinding.
- Do not rely on hover-only affordances for critical actions; the context menu
  and keyboard shortcuts cover them.

### 10.4 Screen readers and AccessKit

- egui: enable the `accesskit` feature where the selected egui version supports
  it. Full custom-row coverage is a **Should** in v1; the Must baseline is
  keyboard/focus and readable text. Custom-painted rows must expose a node with
  role `listitem`, a label containing index, preview text, source app, and state
  (pinned/pasted/buffer), and selected/disabled state. The list exposes `list`
  with `itemCount` and `itemIndex`.
- Announce search result counts and save/error status. If AccessKit live
  regions are unavailable, update the focused node and use a short status text
  in the search row.
- Settings HTML: semantic landmarks (`nav`, `main`), real `button`/`input`
  elements, `<label for>`, `aria-describedby` for help text, `aria-live` for
  save/error, `role="dialog"` + `aria-modal` for dialogs, and a logical heading
  hierarchy.
- Test with Narrator and Accessibility Insights; record any gap in the QA
  report.

### 10.5 IME

- The search field must handle winit/egui IME events: composition start,
  preedit, commit, cancel; show the composition underline; position the
  candidate window at the caret.
- Test Chinese (Microsoft Pinyin) and Japanese (Microsoft IME) at 100/150/200%.
  Verify no duplicated characters, no lost commits, and that Enter during
  composition commits instead of pasting.
- If egui's IME proves insufficient, T3 may host a native Win32 `EDIT` control
  for the search field only. This is a documented fallback, not a redesign; it
  must keep the Fluent visual frame and keyboard behavior.

### 10.6 Reduced motion and sound

- Respect the system animation setting and the in-app Reduce motion toggle.
- The copy sound is optional and off by default; never rely on sound as the
  only feedback.

---

## 11. Component state matrix (summary)

| Component | States |
|---|---|
| Row | default, hover, selected, selected+focused, pasted, pinned, buffer, disabled |
| Icon button | rest, hover, pressed, focused, disabled, toggled |
| Text button | rest, hover, pressed, focused, disabled, loading |
| Input | rest, hover, focused, error, disabled, read-only |
| Search | empty, typing, composing, searching, no results, error |
| Toggle | off, on, hover, focused, disabled |
| Nav item | rest, hover, selected, focused, disabled |
| MessageBar | info, success, warning, error, reconnecting |
| Dialog | opening, open, saving, error, closing |
| Popup | hidden, prewarmed, showing, visible, searching, empty, migrating, disconnected, error, closing |

---

## 12. Do / don't

Do:

- Use tokens for every color, size, radius, shadow, and duration.
- Keep the popup first frame immediate; optimize the hot path.
- Use system colors in HC and validate contrast in light/dark.
- Use glyphs/shape in addition to color for state.
- Cache fonts, icon atlases, thumbnails, and row layouts.
- Test Chinese and English with the longest strings at 200% DPI.

Don't:

- Don't hardcode hex colors, pixel sizes, or font names in UI code.
- Don't use pure red/green or color-only status.
- Don't draw per-row shadows or allocate per row per frame.
- Don't animate list layout or the caret.
- Don't use a custom-drawn settings title bar; use the native frame.
- Don't load legacy theme XML or raster black icons.
- Don't let the popup steal focus from the target app on show; use
  `WS_EX_NOACTIVATE` and explicit input handling.
- Don't put a web UI in the popup.

---

## 13. Frozen decisions and delegated ranges

Frozen by T0:

- Fluent semantic roles with the Ditto-validated values in §3.
- Popup default 420×480, bottom search, 40px top title, 36px one-line rows.
- Settings 900×640, left nav 208, the nine pages in §8.2.
- `design/tokens.json` as the only token source, with generated Rust/CSS/TS.
- React 19 + TypeScript + Vite + Fluent UI React v9 for settings.
- egui + winit for the popup.
- EN/ZH in v1; i18n architecture for later.
- HC via system colors, not a separate hardcoded palette.

Delegated:

- T1: token schema implementation, generator, exact file paths, plugin
  versions, and the primary/fallback spike.
- T3: egui widget composition, font atlas strategy, IME fallback, AccessKit
  coverage, exact row rendering.
- T5: Fluent UI theme construction, page composition, table virtualization,
  settings layout details within the frozen metrics.
- T7: measurement and evidence; any deviation must be recorded in
  `QA-REPORT.md`.

Mockups: `artifacts/rewrite/mockups/index.html`, `popup-light.html`,
`popup-dark.html`, `settings-light.html`, `settings-dark.html`. They are
annotated references, not screenshots.
