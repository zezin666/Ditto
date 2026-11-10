# T3 runtime captures

All captures use the final `Release64\Ditto.exe` in a unique temporary
portable directory with a scratch database. No user `%APPDATA%\Ditto` database
or settings were used.

`capture.ps1` starts the portable test app, opens the quick-paste window,
drives search/hover/scrollbar/description states, and composites visible child
windows so overlay scrollbars and rich-edit content appear in the PNG output.

## Light

- `t3-light-selected-image-color.png`
- `t3-light-hover.png`
- `t3-light-search-highlight.png`
- `t3-light-scrollbar-hover.png`
- `t3-light-description.png`

## Dark

- `t3-dark-selected-image-color.png`
- `t3-dark-hover.png`
- `t3-dark-search-highlight.png`
- `t3-dark-scrollbar-hover.png`
- `t3-dark-description.png`

The headless `PrintWindow` pass captures the description card shell but does
not reliably include the RichEdit text layer. Runtime instrumentation confirmed
the dark text color was `0x00F5F3F2`; light text uses `TextPrimary`.

## T6 review rerun

The `t3-fix-light-*.png` and `t3-fix-dark-*.png` captures were regenerated from
the final post-review Release binary after moving the translucent brush cache
into `UiDraw` and sharing one `Graphics` object per list paint pass.
