# T4b Verification

## Status

- Task: T4b remaining dialogs modernization.
- Worktree: `D:\orca\workspaces\Ditto\ui-dialogs`
- Branch: `taohy/ui-dialogs`
- HEAD remains the T4a base,
  `828f928ad2a343daed6f61dbc43b1a96506e3fb2`.
- Git operations: none performed by the worker.

## Files Changed

- Dialog sources:
  `src/About.*`, `src/GroupName.*`, `src/MoveToGroupDlg.*`,
  `src/CopyProperties.*`, `src/DeleteClipData.*`, `src/SelectDB.*`,
  `src/FriendDetails.*`, `src/FriendPromptDlg.*`, `src/OptionFriends.*`,
  `src/OptionsCopyBuffers.*`, `src/OptionsStats.*`, `src/OptionsTypes.*`,
  `src/OptionsUtilities.*`, `src/AddType.*`,
  `src/FileTransferProgressDlg.*`, `src/ProgressWnd.*`
- Shared T4a files, resources, themes, project files, `resource.h`, T2/T3
  files, and user workspace files were not modified.
- Evidence: `docs/ui/t4b-build-release.log`,
  `docs/ui/t4b-build-debug.log`, `docs/ui/t4b-restore.log`,
  `docs/ui/t4b-runtime-evidence.log`, `artifacts/ui/t4b/`.

## Implementation

- Remaining dialogs/pages derive from `CThemedDialog` or
  `CThemedPropertyPage`; property-sheet pages use the T4a sheet renderer.
- Existing `CCopyProperties` and `CFriendPromptDlg` color handlers delegate to
  `CThemedControls::OnCtlColor`.
- Post-localization layout uses `CThemedControls::FinishLayout` plus measured
  fitting for radio buttons, checkboxes, statics, and their adjacent controls.
  Measurements use `UiTheme` body/dialog fonts and `CDPI`.
- Theme-painted group boxes are placed behind sibling controls with
  `WS_CLIPSIBLINGS`, preventing their canvas fill from obscuring the row
  controls in Stats, Copy Buffers, Friends, and Delete Clip Data.
- List columns are DPI-scaled. Friends columns use the current client width.
- The custom `CProgressWnd` uses theme canvas/text/button painting, DPI-scaled
  geometry, and a small native progress subclass that draws track, value, and
  border from `CUiTheme` semantic colors.
- `CFileTransferProgressDlg` applies the same progress-bar rendering and
  theme/dark-mode preparation; its network workflow was not executable in the
  smoke environment.
- No control IDs, menu command IDs, localization XML IDs, settings keys,
  schema, clipboard semantics, add-in ABI, or hotkeys were changed.

## Builds

- NuGet restore: PASS (`docs/ui/t4b-restore.log`).
- `Release|x64`: PASS, 0 warnings, 0 errors
  (`docs/ui/t4b-build-release.log`).
- `Debug|x64`: PASS, 2 warnings, 0 errors
  (`docs/ui/t4b-build-debug.log`). Both warnings are existing `C4244`
  conversions in `DeleteClipData.cpp` near the database-size formatting path.
- Final binary hashes:
  - Release: `59C453D6D69E5961F2E82FDA4DE3FEF4067B5CC1A9E092DEFD67DFBD60E2D97F`
  - Debug: `3DB22A87F501F96AB850F998CFA3A57A916427C6C76B11ED9717576BC1FADE91`

## Runtime Smoke

Runtime used new `%TEMP%\ditto-ui-t4b-*` directories, scratch databases,
`portable` markers, and copied `Debug\Language`/`Debug\Themes`. No real
`%APPDATA%\Ditto` data was read or written.

| Surface | Open | Interact | Close | Light | Dark |
| --- | --- | --- | --- | --- | --- |
| Options / Supported Types | PASS | page switch | PASS | PASS | PASS |
| Add Supported Type | PASS | list selection, Add default | Cancel PASS | PASS | PASS |
| Options / Copy Buffers | PASS | hotkey/checkbox/group rows rendered | PASS | PASS | PASS |
| Options / Friends | PASS | list loaded | PASS | PASS | PASS |
| Friend Details | PASS | fields/checkbox visible | Cancel PASS | PASS | PASS |
| Copy Properties | PASS | fields/list visible | Cancel PASS | PASS | PASS |
| Send to Friend | PASS | fields/checkbox visible | Cancel PASS | PASS | PASS |
| Options / Stats | PASS | values populated | PASS | PASS | PASS |
| Options / About | PASS | list populated, tab title visible | PASS | PASS | PASS |
| Move to Group | PASS | tree populated | Cancel PASS | PASS | PASS |
| Group Name | PASS | field focused | Cancel PASS | PASS | PASS |
| Delete Clip Data | PASS | 242 rows loaded, filters/delete controls visible | Close PASS | PASS | PASS |
| Custom progress window | PASS | 242 selected rows, Cancel visible | PASS | PASS | PASS |

The final Release light/dark smoke runs completed with
`processAlive=True`. The transient progress window was captured from the final
Debug binaries after selecting all 242 rows; the Release delete loop completed
too quickly for a stable screenshot.

## Screenshots

- Contact sheets:
  `artifacts/ui/t4b/t4b-dialogs-light-contact-sheet.png`
  and `artifacts/ui/t4b/t4b-dialogs-dark-contact-sheet.png`
- Per-dialog Release screenshots are in `artifacts/ui/t4b/` with `light` and
  `dark` suffixes.
- Direct-screen variants use `-screen` and cover transparent statics,
  checkboxes, group boxes, custom progress painting, and other surfaces that
  `PrintWindow` can omit.
- Final progress captures:
  `t4b-progress-wnd-debug-progress-light-screen.png` and
  `t4b-progress-wnd-debug-progress-screen.png`.

## Tab Order and Defaults

No `WS_TABSTOP` order, control ID, or command ID was changed. Native template
order remains in effect. Add Supported Type now promotes its Add button to the
default push button at runtime; existing OK/Cancel DEFPUSHBUTTON templates are
unchanged.

## Unverified

- Actual Windows high-contrast mode switching. All painting resolves through
  `CUiTheme`, and `CDarkMode` restores native themes in high contrast.
- Runtime DPI movement across 100%, 150%, and 200% monitors. Screenshots were
  captured at the current 144 DPI desktop.
- `CSelectDB`: the class has no call sites in the current source, so it could
  be compiled and audited but not opened through the product UI.
- `COptionsUtilities`: intentionally not added to `COptionsSheet` per T0/P3;
  compiled and themed but not reachable.
- `CFileTransferProgressDlg`: remote-file transfer was not exercised. It
  compiles and uses the shared themed progress renderer.
- IME and screen-reader behavior.

## Risks

- Native `SysDateTimePick32` fields in Delete Clip Data retain their OS-drawn
  light date field in Modern Dark on this host. Text, surrounding form,
  buttons, list, headers, group boxes, and scrollbars are themed, but full
  custom date-picker painting would require a broader common-control owner and
  was not added inside T4b.
- Native list scrollbar theming remained OS-dependent in some `PrintWindow`
  captures; direct-screen captures were used to distinguish capture artifacts
  from rendering.

## Next

- Coordinator review and commit from this worktree.
- T5 can consolidate the final screenshots and decide whether to fund a
  dedicated native date-picker/common-scrollbar follow-up outside T4b.
