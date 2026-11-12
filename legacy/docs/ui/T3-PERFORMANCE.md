# T3 performance and runtime evidence

- Date: 2026-11-11
- Binary: `Release64\Ditto.exe`, Release x64
- Runtime DPI: 144 DPI
- Test profile: unique `%TEMP%\ditto-ui-t3-*` portable directory
- Database: scratch Ditto DB, 5,134 `Main` rows
- Display: 4,480 x 1,600 virtual desktop reported by the test session

## GDI and user handle check

The process was opened against the 5,134-row scratch database. The list was
warmed with 50 line-scroll messages, then 1,000 synchronous line-scroll
messages were sent (500 down, 500 up).

| Measurement | Before | After | Delta |
|---|---:|---:|---:|
| GDI objects (`GetGuiResources(GR_GDIOBJECTS)`) | 88 | 87 | -1 |
| User objects (`GetGuiResources(GR_USEROBJECTS)`) | 63 | 62 | -1 |
| Working set | - | 32.2 MB | - |

- 1,000 synchronous scroll messages completed in 3,344 ms, approximately
  3.3 ms per message including the list update path.
- No crash, assertion, or process exit occurred during the scroll run.
- No per-scroll GDI/user handle growth was observed.

## Caching and allocation review

- Row pills and focus rings use the cached paths, brushes, and pens owned by
  `UiDraw`. `QListCtrl` does not create GDI+ paths or pens per row.
- The checkerboard pattern brush is created once per DPI and reused for every
  visible thumbnail and translucent color swatch.
- The existing per-clip DIB thumbnail cache in `QPasteWnd` /
  `CClipFormatQListCtrl` remains the image decode/cache path. T3 did not add a
  second decoder or cache.
- First-ten, caption, and tooltip fonts are rebuilt only on creation/DPI
  change. The list keeps the user's clip font.
- Hover changes invalidate only the previous and new row. Selection and
  keyboard focus still use the native owner-data list state path.
- The modern scrollbar is double-buffered through `CMemDCEx`; rounded paths,
  brushes, and pens come from `UiDraw`.

## T6 review fixes

- All translucent color swatch brushes now live in `UiDraw` beside the other
  GDI+ caches. `UiDraw::PurgeCaches()` clears them on theme/DPI changes and
  from `CCP_MainApp::ExitInstance()` before `GdiplusShutdown`.
- A final Release portable exit logged `ExitInstance` and then
  `T3 UiDraw purged translucent brush cache`, and the process exited normally.
- `CQListCtrl::OnCustomdrawList()` creates one `Gdiplus::Graphics` for
  `CDDS_PREPAINT`, reuses it for every item decoration, and releases it on
  `CDDS_POSTPAINT`. Row pills, glyphs, focus rings, swatches, and thumbnail
  borders use the graphics overloads.
- Debug runtime instrumentation logged
  `T3 list paint pass reused one Gdiplus::Graphics object`.

## T7 re-review fixes

- `UiDraw::AlphaBrush()` now uses a 64-entry UI-thread-only cache. When the
  cap is reached it clears the cache before inserting another color/alpha
  pair, keeping memory bounded during long clipboard sessions.
  `PurgeCaches()` still clears it on theme/DPI changes and before
  `GdiplusShutdown`.
- `CGdiImageDrawer` has a direct `Gdiplus::Graphics&` overload. It draws the
  cached PNG bitmap directly and does not create a temporary DC, bitmap, or
  `Graphics` object per icon.
- No-auto-delete, shortcut, and in-group list indicators now use that overload
  with the list's paint-pass `Graphics` object.
- The targeted scratch dataset had 120 rows, each with no-auto-delete,
  shortcut, and in-group indicators plus a unique translucent RGBA value.
  Release scrolling reached the 64-entry cache cap and logged cache clears;
  Debug instrumentation logged the shared drawer path at draw 1 and draw 100.
- `ModernScrollBar::Show()` intentionally keeps the `animate` parameter for
  interface compatibility but does not start a fade-in because the current
  shell calls `Show(false)`. Auto-hide fade-out remains active.

## Runtime behavior checked

- Light and dark owner-drawn list rows rendered from the final Release binary.
- Selected, focused, hover, pasted, pinned, shortcut, no-auto-delete, and
  in-group indicators rendered.
- Search filtering and pale `HighlightBg` match background rendered.
- Image thumbnail checkerboard, rounded border, and color swatches rendered.
- Modern vertical and horizontal scrollbars rendered and expanded on hover.
- The description window rendered in both themes. Dark RichEdit text color was
  verified at runtime as `0x00F5F3F2` against the dark surface. The headless
  `PrintWindow` path did not reliably capture the RichEdit text layer, so the
  description PNGs show the card chrome while text capture remains a tooling
  limitation.

## UNVERIFIED

- Subjective smoothness on a physical 10,000-row database; the automated run
  used 5,134 rows.
- Mouse drag selection and Enter/Shift+Enter paste through the full clipboard
  round trip were not exercised in the unattended capture.
- High-contrast theme, 100%/200% DPI movement, and Windows 10 DWM behavior.
- RTF/HTML/image tooltip content in dark mode; only plain-text description
  text color was instrumented.
