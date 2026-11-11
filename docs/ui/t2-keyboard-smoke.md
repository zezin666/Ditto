# T2 Keyboard And Command Smoke

The following paths were exercised against the final portable Release build
using a scratch database under `%TEMP%`.

| Path | Evidence |
|---|---|
| Search focus | Synthetic click focused the native `CSymbolEdit`; focus ring and history/clear controls rendered |
| Unicode text | `中文 検索テスト` rendered through the native edit control |
| Selection/caret path | `EM_SETSEL(0, 2)` followed by `EM_GETSEL` returned `0, 2` |
| Clear search | Clicking the custom clear region left `WM_GETTEXTLENGTH` at zero |
| List navigation | `Down`, `PageDown`, `Home`, and `Up` were posted to the owner-data list without a crash |
| Escape | The popup handled Escape without a crash; while in a group it returned toward normal popup handling |
| Group open/back | Group state, breadcrumb title, and back button rendered; `ESC` back behavior was exercised |
| Caption positions | `ID_VIEWCAPTIONBARON_TOP/BOTTOM/LEFT/RIGHT` all relaid out and captured |
| Clipboard refresh | Three clips were captured and displayed in each theme |
| Theme refresh | Modern Light and Modern Dark were launched from separate portable profiles |
## T6 Review Follow-up

Source changes now keep `WS_TABSTOP` on the groups, back, system-menu, and
options buttons and add `WS_TABSTOP`, `WM_GETDLGCODE`, focus rendering, and
`Space`/`Enter` handling to the empty-state action.

The Debug portable harness verified:

- `WS_TABSTOP=true` for groups, back, system menu, options, and empty state.
- Base `GetNextDlgTabItem` chain: system menu, options, groups, list.
- Group-state chain adds the visible back button.
- Empty-state chain adds the empty-state action.
- Focusing groups/back/empty state paints the themed focus ring.
- `Space` on the focused empty-state action clears the search
  (`WM_GETTEXTLENGTH=0`).

Targeted screenshots:

- `artifacts/ui/t2/t2-light-review-footer-tab.png`
- `artifacts/ui/t2/t2-light-review-empty-keyboard.png`
- `artifacts/ui/t2/t2-light-review-back-tab.png`
- `artifacts/ui/t2/t2-dark-review-footer-tab.png`
- `artifacts/ui/t2/t2-dark-review-empty-keyboard.png`
- `artifacts/ui/t2/t2-dark-review-back-tab.png`

Unchanged paths retained by code inspection: global hotkey dispatch, search
history menu, autocomplete setup, Enter search/paste, list context menu,
special-paste commands, add-in commands, and existing menu command IDs.

Not automated: actual IME candidate/composition UI, every user-configurable hotkey,
and every context-menu action. These remain integration QA items.
