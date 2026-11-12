# T5 - Tauri Settings and Manager UI

## STATUS

Implementation and local evidence are complete. The worktree is **uncommitted**
because repository policy prohibits agent-run Git operations.

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-settings`

## BRANCH

Expected integration branch: `rewrite/tauri`.

The branch name was not queried because Git operations are reserved for the
user/coordinator by `AGENTS.md`.

## HEAD

Not queried. Base supplied by the task: `rewrite/tauri` @ `847f966`.

## FILES

Frontend:

- `apps/ditto/ui/package.json`, `package-lock.json`, `tsconfig.json`,
  `vite.config.ts`, `index.html`
- `apps/ditto/ui/src/App.tsx`, `main.tsx`, `types.ts`, `theme.ts`, `i18n.tsx`
- `apps/ditto/ui/src/api/commands.ts`, `api/browserMock.ts`
- `apps/ditto/ui/src/components/Controls.tsx`
- `apps/ditto/ui/src/pages/*.tsx` (nine pages plus History manager)
- `apps/ditto/ui/src/styles/app.css`
- `apps/ditto/ui/locales/en-US.json`, `locales/zh-CN.json`
- `apps/ditto/ui/scripts/check-locales.mjs`, `visual-check.mjs`,
  `lifecycle-check.ps1`

Tauri:

- `apps/ditto/src-tauri/src/commands/mod.rs`
- `apps/ditto/src-tauri/src/lib.rs`
- `apps/ditto/src-tauri/capabilities/default.json`

Evidence:

- `docs/rewrite/T5-REPORT.md`
- `docs/rewrite/screenshots/T5/**`
- `docs/rewrite/logs/T5/**`

No popup, storage, clipboard, model, legacy C++, or `LICENSE` file was edited.

## BUILD

Toolchain used:

| Tool | Version |
|---|---|
| Node | 24.13.0 |
| npm | 11.6.2 |
| React | 19.3.1 |
| Fluent UI React Components | 9.75.2 |
| Fluent UI React Icons | 2.0.345 |
| Vite | 7.3.8 |
| TypeScript | 5.9.3 |
| Tauri CLI | 2.12.4 |
| Rust | 1.99.1 stable MSVC |

Commands:

```powershell
npm run build --prefix apps/ditto/ui
npx tauri build
```

Both commands passed. Vite emitted one informational warning that the main
chunk is 790.33 kB (224.77 kB gzip), above its 500 kB warning threshold.

NSIS artifact:

```text
target/release/bundle/nsis/Ditto_0.1.0_x64-setup.exe
```

| Artifact | Bytes | SHA-256 | Signature |
|---|---:|---|---|
| `Ditto.exe` | 7,404,544 | `7572BA440F422FB8E4CEF6906101921E0E725E2A5A88E4702EDFF4D68F39E30A` | NotSigned |
| `DittoPopup.exe` | 4,305,408 | `E95455BD4C3491A559A7F7AAD979E7A5B391CF1AE6C640FF0583F6EEF8320D3C` | NotSigned |
| `DittoUacHelper.exe` | 122,880 | `0061942E7754E489590FB34187EE3F7711D22CC9006B327F5B104C5E91D35F7A` | NotSigned |
| NSIS setup | 3,354,800 | `0BAD42E2F98F498469D15C0CF288509F79B28887A3655DDD5EC1128BAEF59348` | NotSigned |

Logs:

- `docs/rewrite/logs/T5/npm-build.log`
- `docs/rewrite/logs/T5/tauri-build.log`
- `docs/rewrite/logs/T5/cargo-fmt.log`
- `docs/rewrite/logs/T5/cargo-clippy.log`
- `docs/rewrite/logs/T5/cargo-test.log`
- `docs/rewrite/logs/T5/visual-check.log`
- `docs/rewrite/logs/T5/lifecycle-check.log`

## PAGES AND I18N

Navigation contains the nine frozen pages:

1. General
2. Appearance
3. Hotkeys
4. Clipboard
5. Groups
6. Copy Buffers
7. Data
8. Languages
9. About

History is the second tab on Data, matching decision D-112. It provides search,
group/kind/pinned filters, pagination (50/100/250/500 rows), preview, plain-text
edit, pin/unpin, delete with confirmation, group move, and copy-buffer
assignment.

`en-US.json` and `zh-CN.json` each contain 312 keys. `npm run locales:check`
checks:

- identical key sets;
- non-empty values;
- matching placeholders;
- literal `t("...")` references in TypeScript.

Missing keys fall back to English and produce a visible `MessageBar`; a missing
key in both catalogs is returned as the visible key.

## COMMANDS AND PERMISSIONS

Commands are registered in `apps/ditto/src-tauri/src/lib.rs` through
`tauri::generate_handler!`. The settings capability is limited to the `settings`
window and `core:window:allow-close`; no shell, filesystem, HTTP, process,
updater, or remote-content plugin permission is granted. The T1 CSP is
unchanged.

| Command | Mock behavior | Expected T7 backend |
|---|---|---|
| `get_settings` | Returns in-memory defaults, revision, restart flag | Typed settings accessor over the core/storage bridge |
| `set_settings` | Validates and replaces mock snapshot | Validate, transactionally persist, broadcast `SettingsChanged` |
| `list_clips` | Filters/sorts 620 synthetic clips with page cap 500 | Keyset-paginated storage query |
| `search_clips` | Case-insensitive synthetic search | FTS5 trigram query with bounded fallback |
| `get_clip` | Returns synthetic text detail | `GetClip` IPC/core call |
| `delete_clips` | Removes mock clips and clears slot references | Soft delete/transactional purge policy |
| `set_pinned` | Updates mock pin flags | Storage pin update with stable order |
| `move_to_group` | Updates mock group IDs | Transactional group move |
| `edit_clip` | Updates text and preview | Plain-text edit preserving raw formats |
| `get_groups` | Returns seeded tree with counts | Storage group tree |
| `create_group` | Adds a mock group | Validated group create |
| `rename_group` | Renames a mock group | Validated group rename |
| `delete_group` | Deletes descendants, moves clips to ungrouped | Transactional group delete |
| `move_group` | Prevents cycles in mock tree | Validated group move |
| `get_copy_buffers` | Returns five seeded slots | Normalized copy-buffer table |
| `set_copy_buffer` | Updates name/enabled/clip reference | One-slot transactional assignment |
| `backup_db` | Returns a synthetic destination path | Storage backup API, verify, rotate |
| `restore_db` | Acknowledges mock restore | Validate backup, restart-required restore |
| `export_data` | Acknowledges mock export | T6 manifest/DB/blob export |
| `import_data` | Acknowledges mock import | T6 validated merge/import |
| `optimize_storage` | Acknowledges mock optimization | `PRAGMA optimize` and incremental vacuum |
| `get_migration_status` | Returns mock-ready status and warning | T6 migration state/progress |
| `open_data_folder` | Returns a mock path | Narrow approved folder-open command |
| `open_log_folder` | Returns a mock path | Narrow approved folder-open command |
| `get_diagnostics` | Returns process/version fields, no content | Core diagnostics without clip content |
| `get_app_info` | Returns build/protocol/schema/source metadata | Build-stamped application info |

`MockBackend` is a process-global seeded in-memory implementation. Every
real-data boundary has a `TODO(T7)` marker. The UI always displays a
`MockBackend` badge and a synthetic-data notice.

## TEST

Passed:

```powershell
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npx tauri build
npm run visual:check
pwsh -NoProfile -File scripts/lifecycle-check.ps1
```

Tauri command tests cover deterministic search/pagination, edit/delete buffer
consistency, and group-cycle rejection. The complete workspace test suite
passed.

Browser verification:

- 22 checks, including all nine navigation targets;
- zero console errors/warnings;
- zero page errors;
- zero unlabeled interactive accessibility nodes;
- no horizontal overflow or container escape at 900x640, 720x480, DPR 1.5,
  and DPR 2.0;
- settings Apply toast, hotkey conflict warning, and destructive confirmation
  Escape path exercised;
- English, Chinese, light, dark, and high-contrast rendering inspected.

## EVIDENCE

Actual Tauri WebView2 window:

- `docs/rewrite/screenshots/T5/tauri-settings-window-100.png`

Browser-rendered screenshots:

- `light-en-general-100.png`
- `light-en-history-100.png`
- `dark-en-appearance-100.png`
- `dark-en-history-100.png`
- `light-zh-data-100.png`
- `dark-zh-general-100.png`
- `dark-zh-general-150.png`
- `dark-zh-general-200.png`
- `dark-zh-general-720x480.png`
- `high-contrast-en-general-100.png`

Structured results:

- `docs/rewrite/screenshots/T5/visual-check.json`
- `docs/rewrite/screenshots/T5/webview-lifecycle.json`

Release lifecycle result, using a synthetic `DITTO_DATA_DIR`:

| Check | Result |
|---|---|
| WebView2 descendants before Settings | 0 |
| WebView2 descendants while Settings open | 6 |
| WebView2 descendants 7 seconds after close | 0 |
| Core alive after close | yes |
| Core private-memory delta | 0 bytes |
| Second instance `--settings` exit | 0 |
| Settings window observed | `Ditto Settings` |
| Settings window observed latency | 1,848 ms |
| Second-instance process exit latency | 4,382 ms |

No real `%APPDATA%\Ditto`, `%LOCALAPPDATA%\Ditto`, clipboard history, or user
database was read.

## UNVERIFIED

- Real storage, clipboard, migration, backup/restore, import/export, file
  dialogs, and migration progress are MockBackend placeholders until T7.
- Native Windows 100/150/200% DPI changes were not applied system-wide. Browser
  device-scale factors 1.0/1.5/2.0 were captured, and the actual Tauri window
  was captured at the current desktop scale.
- Narrator, IME composition, OS keyboard focus traversal, and Windows HC
  Black/White variants were not manually exercised. The browser AX tree and
  keyboard paths were checked.
- The tray menu Settings item was not clicked. The tested second-instance path
  calls the same `open_settings` function used by the tray.
- Esc-to-close is implemented, but the lifecycle test closes the native window
  through `WM_CLOSE`/`CloseMainWindow`.
- NSIS install, upgrade, uninstall, and portable ZIP execution were not run.
- All release binaries and the installer are unsigned. Defender/SmartScreen
  warnings were not suppressed or bypassed.

## RISKS

1. **Second-instance latency:** process exit took 3.7-4.4 seconds in repeated
   release runs, above the T1 `<150 ms` single-instance acceptance target.
   The command does open the existing Settings window and exits 0. T7 should
   profile Tauri pre-plugin startup or add a lightweight preflight forwarding
   path if this budget remains mandatory.
2. **Settings open latency:** the title was observed after 1.58-2.10 seconds,
   around or above the `PF-8` 1.5-second target. The main Fluent bundle is
   790.33 kB; code splitting or a lighter initial shell should be evaluated.
3. **Mock success semantics:** file operations acknowledge success while the
   visible MockBackend badge states that no real file operation occurred.
   T7 must replace these commands before acceptance.
4. **Unsigned distribution:** Windows may warn or quarantine the binaries.
   Signing is unresolved.

## NEXT

1. Coordinator commits the uncommitted worktree and records the actual branch
   and HEAD SHA.
2. T7 replaces `MockBackend` methods with the T2/T6 storage and migration APIs,
   including native file dialogs and validation.
3. T7 reruns native DPI, Narrator/IME, tray-entry, installer/uninstall, and
   performance protocols on an unlocked desktop.
4. Evaluate bundle splitting and the single-instance fast path against the
   measured latency gaps before release sign-off.
