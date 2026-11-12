# FOUNDATION NOTES - T1

- Status: implemented and built for `Release|x64` and `Debug|x64`
- UI stack: C++17, MFC, Win32, GDI, GDI+
- Thread affinity: all `CUiTheme`, `UiDraw`, `UiWindow`, and `CDarkMode`
  functions are for the UI thread
- Toolchain: MSBuild 18.11.1, MSVC 19.51.36260, MFC v145, Windows SDK 26100

## Downstream API

### Semantic tokens

Use the `CUiTheme` singleton through `UiTheme()` or
`CUiTheme::Instance()`. Legacy `CGetSetOptions::m_Theme` accessors remain
source-compatible.

```cpp
const CUiTheme& theme = UiTheme();
COLORREF canvas = theme.Canvas();
int captionHeight = UiDraw::Scale(theme.CaptionHeight(), dpi);
HFONT font = UiDraw::Font(
    theme.FontFamilyUi(),
    theme.FontSizeBody(),
    FW_NORMAL,
    dpi);
```

`CTheme::Load()` synchronizes `CUiTheme` automatically after a successful or
failed load, including the default light path. Downstream code may call:

```cpp
CUiTheme::Instance().Refresh(CGetSetOptions::m_Theme);
CUiTheme::Instance().OnSystemThemeChanged();
CUiTheme::Instance().OnDpiChanged();
```

`OnDpiChanged()` purges draw caches. It does not move or repaint controls;
the owning control must relayout and invalidate after `WM_DPICHANGED`.

### Drawing

```cpp
UiDraw::RoundedRect(
    graphics, rect, scaledRadius, fillColor, borderColor, borderWidth);
UiDraw::FocusRing(dc, rect, scaledRadius, UiTheme().FocusRing(), ringWidth);
UiDraw::DrawTextEllipsis(
    dc, text, rect, UiTheme().FontFamilyUi(), UiTheme().FontSizeBody(),
    FW_NORMAL, UiTheme().TextPrimary(), dpi);
UiDraw::DrawGlyph(
    graphics, glyphRect, UiDraw::Glyph::Close, UiTheme().TextSecondary(), dpi);
UiDraw::DrawIcon(dc, bitmap, targetRect, true, UiTheme().TextSecondary());
```

Use the `Graphics&` overloads when painting several items in one paint pass.
The `CDC*` overloads create a stack `Graphics` wrapper for convenience.

`UiDraw::DrawIcon()` preserves aspect ratio and centers the bitmap in the
target rect. Tinted bitmaps preserve source alpha. Simple icons should use
`DrawGlyph`, which renders once into a cached ARGB bitmap and then blits it.

### Window effects

```cpp
UiWindow::ApplyWindowEffects(hwnd);
UiWindow::ApplyWindowEffects(hwnd, effects);
UiWindow::SetDarkTitleBar(hwnd, dark);
UiWindow::DrawFallbackBorder(dc, rect, UiTheme().Border());
```

`ApplyWindowEffects(HWND)` derives colors and dark state from `CUiTheme`.
The explicit `Effects` overload is available for popup shells that need
custom border/caption colors.

### Native dark mode

```cpp
CDarkMode::Initialize();
CDarkMode::ApplyToWindow(hwnd);
CDarkMode::ApplyToControl(child);
CDarkMode::ApplyToMenus(owner);
CDarkMode::IsHighContrast();
```

`ApplyDarkMode(HWND)` is an alias for `ApplyToWindow(HWND)`. In high contrast,
the helper restores native themes instead of forcing dark mode.

## Token resolution

`CTheme::LoadDefaults()` is now modern light. It keeps every legacy accessor
and changes its values to the v1.0 light palette. New color, metric, font-size,
and motion keys are optional.

An optional key is considered explicit only when it came from theme XML.
When resolving an old theme:

1. An explicit new semantic key wins.
2. Otherwise `CUiTheme` maps the semantic token from its documented legacy
   source.
3. If neither exists, the modern light default is used.

The main legacy fallbacks are:

| Semantic token | Legacy source |
|---|---|
| `Canvas` | `MainWindowBG` |
| `Surface` | `ListBoxOddRowsBG` when distinct from canvas |
| `SurfaceAlt` | `ListBoxEvenRowsBG` when distinct from surface |
| `Border` | `Border` |
| `TextPrimary` | `ListBoxEvenRowsText` |
| `TextSecondary` | `GroupTreeText` |
| `Selection` | `ListBoxSelectedBG` |
| `SelectionText` | `ListBoxSelectedText` |
| `HighlightBg` | `Selection` |
| `HighlightText` | `SearchTextHighlight` for old themes |
| `Pasted` | `ClipPastedColor` for old themes |
| `ScrollBar*` | existing `ScrollBar*` keys |
| Caption colors | existing `Caption*` keys |

Classic and other old themes therefore retain their explicit colors. Modern
themes have equal odd/even row colors, so `CUiTheme::UsesLegacyZebra()` is
false and `ListRowBackground()` returns `Surface`.

## Parser behavior

- New keys use optional loaders and do not log errors when absent or empty.
- Legacy required keys retain their previous validation behavior.
- `rgb()`, `hsl()`, `#RRGGBB`, and numeric color values remain supported.
- `ListSmallQuickPasteIndexColor` is now loaded from XML.
- Missing-key log messages use a proper Unicode conversion for the node name.
- Unknown keys are ignored by this version and by old Ditto versions.

`Modern Dark.xml` is the canonical full dark theme. `DarkerDitto.xml` is a
byte-identical mirror because the follow-Windows dark path selects that name.
The two files must be updated together.

## Windows accent

When `Theme2` is empty, T1 still loads the Windows accent through the existing
DWM `ColorizationColor` path. The accent is accepted only when both checks
pass at 4.5:1 or better:

- accent text against the active surface/canvas;
- `AccentOn` text against the accent fill.

An invalid OS accent falls back to `#2563EB` in light mode or `#6C9BFF`
text/`#4C8DFF` fill in dark mode. Follow-Windows mode also derives hover,
pressed, focus, and fill state colors from the validated accent.

## High contrast

`CUiTheme` checks `SPI_GETHIGHCONTRAST` during resolution. When active it
bypasses theme colors and uses system `COLOR_WINDOW`, `COLOR_3DFACE`,
`COLOR_WINDOWTEXT`, `COLOR_HIGHLIGHT`, `COLOR_HIGHLIGHTTEXT`, and related
roles. Zebra striping and forced dark control themes are disabled.

## Caches and invalidation

`UiDraw` owns process-wide UI-thread caches:

| Cache | Key | Invalidation |
|---|---|---|
| GDI fonts | family, physical height, weight, italic | theme/DPI/exit |
| GDI brushes | `COLORREF` | theme/exit |
| GDI pens | `COLORREF`, width | theme/exit |
| GDI+ brushes/pens | color, width/alignment | theme/exit |
| rounded paths | width, height, radius | theme/exit |
| vector glyphs | glyph, size, color, stroke, DPI | theme/DPI/exit |
| tinted bitmaps | source, size, color, alpha | theme/exit |

Repeated rows with the same geometry reuse the same path and brush/pen
objects. Ownership is centralized in `UiDraw::PurgeCaches()`; callers must
not delete returned GDI objects. The application purges caches before
`GdiplusShutdown`.

## DWM probing and Win10 fallback

All DWM calls use `LoadLibrary("dwmapi.dll")` and `GetProcAddress`; no DWM
import library or hard link was added.

Attributes attempted:

| Attribute | Value | Purpose |
|---|---|---|
| `DWMWA_USE_IMMERSIVE_DARK_MODE` 20 | `BOOL` | dark frame on current Windows 10/11 |
| legacy dark-mode attribute 19 | `BOOL` | older Windows 10 fallback attempt |
| `DWMWA_WINDOW_CORNER_PREFERENCE` 33 | `DWMWCP_ROUND` | Win11 rounded window |
| `DWMWA_BORDER_COLOR` 34 | `COLORREF` | semantic border |
| `DWMWA_CAPTION_COLOR` 35 | `COLORREF` | DWM caption |
| `DWMWA_TEXT_COLOR` 36 | `COLORREF` | DWM caption text |

Windows 11 is detected with runtime `RtlGetVersion` (build 22000+). Corner
rounding and DWM caption colors are only attempted there. If an attribute
fails, the call is ignored; `DrawFallbackBorder()` provides a square 1px
border for custom non-client paint. The helper never forces a window style.

## Native control and menu dark mode

`DarkMode.cpp` isolates all uxtheme probing:

- `SetPreferredAppMode` by ordinal 135
- `AllowDarkModeForWindow` by ordinal 133
- `FlushMenuThemes` by ordinal 136
- `RefreshImmersiveColorPolicyState` by ordinal 104
- documented `SetWindowTheme`

Supported classes are tree, list, edit, combo, combo box ex, tab, header,
button, static, trackbar, hotkey, scrollbar, and dialogs. Combo boxes use
`DarkMode_CFD`; other controls use `DarkMode_Explorer`. If an entry point is
missing, dark theming is a no-op. High contrast always restores native themes.
Menu support is intentionally best-effort and keeps native `CMenu` commands,
accelerators, checks, and submenus unchanged.

## Icon resources

Existing resource IDs were retained. The 16/20/24/28/32 and high-DPI close,
chevron, folder, return, search, and system-menu PNGs were re-tinted from
near-black to `#6C727C` while preserving alpha. This makes the current legacy
popup readable before T2/T4 migrate drawing to semantic colors.

Simple future icons should use `UiDraw::DrawGlyph()` rather than adding more
monochrome raster variants. Complex images can continue using
`CGdiImageDrawer` and `LoadStdImageDPI`. The existing navy/white Ditto tray
mark was audited and retained because it is already legible in light and dark
tray surfaces; its icon IDs are unchanged.

## Integration points

- `CCP_MainApp::InitInstance()` initializes `CDarkMode`, refreshes
  `CUiTheme`, and `CCP_MainApp::ExitInstance()` purges draw caches.
- `CMainFrame` refreshes dark-mode/high-contrast state on theme changes and
  `ImmersiveColorSet`.
- No popup, list, scrollbar, tooltip, option, or dialog implementation file
  was changed.

## Verification

- `docs/ui/t1-restore-baseline.log`: NuGet restore succeeded.
- `docs/ui/t1-build-release.log`: Release x64 succeeded.
- `docs/ui/t1-build-debug.log`: Debug x64 succeeded.
- `docs/ui/t1-runtime-evidence.log`: final-binary portable test record.
- `artifacts/ui/t1/t1-modern-light.png`: light popup at 144 DPI.
- `artifacts/ui/t1/t1-modern-dark.png`: dark popup at 144 DPI.
- `artifacts/ui/t1/t1-default-follow-light.png`: follow-Windows default path.
- `artifacts/ui/t1/t1-darker-ditto.png`: default dark alias path.
- `artifacts/ui/t1/t1-classic-legacy.png`: legacy Classic theme at 144 DPI.
- `artifacts/ui/t1/t1-icon-contrast.png`: refreshed glyphs on both surfaces.

The runtime test used unique portable directories with scratch databases,
`portable` markers, copied language/theme assets, and explicit `DBPath`. No
real `%APPDATA%\Ditto` data was read into or written by the test instance.
All 14 bundled theme XML files parsed successfully, and the Classic load had
zero missing-key log entries for the new semantic keys.

## UNVERIFIED

- Actual high-contrast theme switching.
- 100%/200% DPI movement across monitors; runtime screenshots are 144 DPI.
- Windows 10 DWM/theme behavior on physical Windows 10.
- Native dark menu rendering across all Windows 10 builds.
- Visual quality of every `UiDraw::DrawGlyph` shape in migrated controls.
