# T4a Property Sheet Tabs and Keyboard Order

## Tab implementation

T4a keeps the native `SysTabControl32` created by `CPropertySheet`. The tab
strip is owner-drawn and hit-tested by `CThemedControls`; selection still goes
through `TCM_SETCURSEL`/`TCN_SELCHANGE`, so the existing property-page
activation, DDX, validation, Apply, OK, and Cancel paths remain in MFC.

`CMFCPropertySheet`/Feature Pack visual managers were not used. This avoids a
new MFC dependency and keeps the existing resource-based `CPropertySheet`
construction unchanged.

## Page order

The order is unchanged from `COptionsSheet`:

1. General
2. Supported Types
3. Keyboard Shortcuts
4. Copy Buffers
5. Quick Paste Keyboard
6. Friends, when friends are enabled
7. Stats
8. About

## Keyboard order

No `WS_TABSTOP` styles or control IDs were changed. Native MFC tab navigation
therefore follows the dialog-template order:

- General: checkboxes and numeric inputs, language/theme/position combos,
  theme/font actions, database path, environment-variable link, accepted copy
  applications, privacy link, Advanced, then property-sheet buttons.
- Keyboard Shortcuts: primary hotkey, Win checkbox, Text Only, Save Current
  Clipboard, Copy And Save Current Clipboard, global-hotkey controls, custom
  key help, then property-sheet buttons.
- Quick Paste Keyboard: report list, assigned-shortcut combo, Remove/Add,
  second-press controls, command hotkey/mouse editors, Assign, Reset, then
  property-sheet buttons.
- Advanced Options: filter, property grid, script actions, compact/repair,
  Reset Clip Order, OK, Cancel.
- Script Editor: script list, name/description, Active, script rich edit,
  sample input/output, app name/title, Run, Add/Delete, OK, Cancel.

The owner-drawn tab strip uses the same left-to-right page order and sends the
same change notification as the native tab control.
