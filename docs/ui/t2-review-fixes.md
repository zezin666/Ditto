# T6 Review Fixes

## Changes

- Keyboard access: footer group/back/system/options buttons keep
  `WS_TABSTOP`; traversal remains owned by `GetNextDlgTabItem`.
- Empty-state keyboard access: the empty static is a tab stop, returns
  `DLGC_WANTCHARS | DLGC_WANTARROWS`, paints a focus ring, and activates clear
  on `Space` or `Enter`.
- High contrast: `CQPasteWnd::OnEraseBkgnd` and `CQPasteWnd::CtlColor` now use
  `UiTheme().Canvas()` through the cached `UiDraw` brush path.
- Localization: the group tree root uses `NoGroup` with default `-No Group-`.
- Menus: group-tree and search-history popup menus call
  `CDarkMode::ApplyToMenus`.
- Caption glyphs: disabled caption buttons now use the same centered icon rect
  as enabled buttons.

## Source Evidence

- `src/QPasteWnd.cpp`: footer button creation retains `WS_TABSTOP`; empty-state
  static is created with `WS_TABSTOP`; `OnEraseBkgnd`/`CtlColor` use resolved
  `UiTheme` Canvas.
- `src/GroupStatic.cpp`: empty-state focus ring, `OnGetDlgCode`, `OnKeyDown`,
  and mouse focus path.
- `src/GroupTree.cpp`: `NoGroup` localization and menu dark-mode helper.
- `src/SymbolEdit.cpp`: search history menu dark-mode helper.
- `src/DittoWindow.cpp`: shared centered glyph geometry for disabled/enabled
  caption buttons.

## Runtime Evidence

Release|x64 and Debug|x64 both build with zero errors:

- `docs/ui/t2-review-build-release.log`
- `docs/ui/t2-review-build-debug.log`

The updated `artifacts/ui/t2/run-t2-runtime.ps1` harness now checks tab-stop
bits and `GetNextDlgTabItem` chains, captures a focused footer/back state, and
activates the empty-state action with a Space message.

The local Release copy could not start because Windows Defender reported
`Trojan:Win32/Bearfoos.A!ml` and blocked execution. No security settings were
changed.

The Debug binary, built from the same source, ran successfully in portable
mode at 144 DPI in both themes. The harness reported:

```text
tabStops=group:True,back:True,system:True,options:True,empty:True
baseTabIds=528,32805,517,513
emptyTabIds=528,32805,517,529
groupTabIds=528,32805,517,519,513
clearTextLength=0
```

Control IDs in the chain are system menu `0x210`, options `0x8025`, groups
`0x205`, list `0x201`, back `0x207`, and empty action `0x211`.

New focused-state screenshots:

- `artifacts/ui/t2/t2-light-review-footer-tab.png`
- `artifacts/ui/t2/t2-light-review-empty-keyboard.png`
- `artifacts/ui/t2/t2-light-review-back-tab.png`
- `artifacts/ui/t2/t2-dark-review-footer-tab.png`
- `artifacts/ui/t2/t2-dark-review-empty-keyboard.png`
- `artifacts/ui/t2/t2-dark-review-back-tab.png`

Release runtime remains UNVERIFIED because of the Windows Defender block.
