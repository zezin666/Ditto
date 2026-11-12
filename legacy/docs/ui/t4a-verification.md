# T4a Verification

## Status

- Branch: `taohy/ui-options-core`
- Base HEAD: `f6e5118502b7e552f00d0127e48854bfcc70db27`
- Worktree: `D:\orca\workspaces\Ditto\ui-options-core`
- Git operations: none performed by the worker.

## Files Changed

- Project registration: `CP_Main.vcxproj`, `CP_Main.vcxproj.filters`
- Shared helpers: `src/Ui/ThemedDialog.h/.cpp`,
  `src/Ui/ThemedControls.h/.cpp`, `src/Ui/UiIds.h`
- Options sheet/pages: `src/OptionsSheet.h/.cpp`,
  `src/OptionsGeneral.h/.cpp`, `src/OptionsKeyBoard.h/.cpp`,
  `src/QuickPasteKeyboard.h/.cpp`, `src/OptionsQuickPaste.h/.cpp`
- Dialog callers: `src/AdvGeneral.h/.cpp`, `src/ScriptEditor.h/.cpp`
- Docs: `docs/ui/new-strings-t4a.md`, `docs/ui/t4a-tab-order.md`,
  `docs/ui/t4a-verification.md`
- Evidence: `docs/ui/t4a-build-release.log`,
  `docs/ui/t4a-build-debug.log`, `docs/ui/t4a-restore.log`,
  `docs/ui/t4a-runtime-evidence.log`, `docs/ui/t4a-persistence.log`,
  `artifacts/ui/t4a/*.png`

## Builds

- `Release|x64`: PASS, 0 errors
  (`docs/ui/t4a-build-release.log`)
- `Debug|x64`: PASS, 0 errors
  (`docs/ui/t4a-build-debug.log`)
- NuGet restore: `docs/ui/t4a-restore.log`

Runtime screenshots were captured from the final Debug64 binary. Windows
Defender quarantined an earlier unsigned Release64 test copy as
`Trojan:Win32/Bearfoos.A!ml`; Defender was not disabled or modified. The final
Release build completed after the layout fixes.

## Runtime verification

- Portable test instances used unique `%TEMP%\ditto-ui-t4a-*` directories,
  scratch databases, copied `Debug\Language`/`Debug\Themes`, and a `portable`
  marker. No real `%APPDATA%\Ditto` data was used.
- Options opened, tabs switched, Advanced Options opened, Script Editor opened,
  and dialogs closed without crashes in Modern Light and Modern Dark.
- Persistence: PASS. The test changed `SendPasteOnFirstTenHotKeys`, verified
  Cancel did not write it, verified OK wrote `0`, restarted the portable
  process, and verified the checkbox reloaded unchecked.
  (`docs/ui/t4a-persistence.log`)
- Evidence log: `docs/ui/t4a-runtime-evidence.log`

## Screenshots

`artifacts/ui/t4a/` contains:

- Options General: light, dark, German
- Options Keyboard Shortcuts: light, dark, German
- Options Quick Paste Keyboard: light, dark, German
- Advanced Options property grid: light, dark, German
- Script Editor: light, dark, German, plus direct-screen captures for the dark
  rich-edit background check

The German screenshots demonstrate the long localized label
`Kopieren und aktuelle Zwischenablage speichern` without clipping.

## Implementation Notes

- Shared dialogs/pages use `CThemedDialog`, `CThemedPropertyPage`,
  `CThemedPropertySheet`, and `CThemedControls`.
- The property sheet keeps the native `SysTabControl32`; its strip is
  owner-drawn and custom hit-tested, while page changes still use native
  selection and `TCN_SELCHANGE`.
- Buttons, checkboxes/radios, group boxes, hotkeys, dropdown-list combos,
  report-list headers, rich edits, tabs, and property grids receive themed
  painting while preserving native input, DDX, DDV, Apply/OK/Cancel, IME, and
  hotkey-control behavior.
- High contrast uses `CUiTheme`/`CDarkMode` system colors and does not force
  dark native themes.

## Unverified

- Actual Windows high-contrast theme switching.
- 100%/200% DPI monitor movement; runtime screenshots are at 144 DPI.
- Russian string coverage and other languages beyond English/German.
- IME composition/candidate behavior in the themed Script Editor rich edit.
- Physical Windows 10 rendering; runtime host is current Windows.
- Native MFC OK/Cancel captions follow the OS UI language (shown in Chinese in
  the English-content screenshots); this is existing behavior and their IDs
  and command flow are unchanged.
