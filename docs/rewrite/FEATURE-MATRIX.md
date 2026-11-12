# FEATURE-MATRIX — v1 scope, priorities, acceptance, and stage ownership

- Status: **T0 baseline, frozen for T1–T8**
- Date: 2026-11-12
- Scope: what ships in v1, what is explicitly deferred, and how each item is
  accepted.
- Priorities: **Must** (blocks v1), **Should** (ship if it does not threaten the
  core budget; may slip with coordinator approval), **Later** (designed for,
  not built), **Out** (explicit non-goal for v1).
- Stages: T1 foundation, T2 clipboard/storage, T3 popup, T4 paste/UAC/drag,
  T5 settings, T6 migration, T7 integration/QA, T8 review.
- The coordinator's prompt appendices B/C are the source of truth for the v1
  cut line. This matrix adds acceptance criteria and stage ownership.

---

## 1. v1 definition of done

A feature is **Done** only when all of the following are true:

1. Implemented in the owning stage's crate/app, merged into the integration
   branch without unresolved conflicts.
2. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` pass.
3. The acceptance evidence in the table exists: automated test, manual QA
   script, or measured artifact.
4. English and Simplified Chinese strings exist; no hardcoded user-visible
   English in the popup or settings.
5. Light, dark, and high-contrast paths are checked; 100/150/200% DPI is
   checked for the surface that changed.
6. Keyboard-only operation is possible for the feature; focus is visible.
7. No clip content is logged; no telemetry; no arbitrary code/path execution.
8. The performance budget in `PERFORMANCE.md` is not regressed (measured in
   T7 for the affected scenario).

---

## 2. Must — v1 blockers

### 2.1 Capture and data

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| C1 | Capture `CF_UNICODETEXT`, `CF_TEXT` | T2 | Copy text in Notepad, browser, VS Code and an elevated app; each appears once with correct text, source app, and timestamp | Integration test with synthetic clipboard data + manual QA |
| C2 | Capture `HTML Format`, `Rich Text Format` | T2 | Copy rich content from Word/browser; raw HTML/RTF bytes are preserved and the text preview is correct | Test comparing raw format SHA-256; manual paste back |
| C3 | Capture images (`CF_DIB`, `CF_DIBV5`, `PNG`) | T2 | Copy a screenshot and a PNG; thumbnail + full image preview; pasting reproduces the image | Thumbnail test + manual paste into Paint/Word |
| C4 | Capture files (`CF_HDROP`) | T2 | Copy files in Explorer; clip shows file names, paste reproduces the file list | Manual Explorer test; raw DROPFILES test |
| C5 | Capture custom/unknown formats | T2 | Copy a custom format (test app); raw bytes are stored and can be pasted back | Synthetic custom format test |
| C6 | Multi-format clip as one item | T2 | Copying from Word produces one clip with text+HTML+RTF, not three items | DB inspection: one `clips` row, multiple `clip_formats` |
| C7 | Dedupe by content + formats | T2 | Copying the same content twice does not create a second item by default; `allow_duplicates` creates one | Unit test on `content_hash`/`formats_hash`; manual |
| C8 | Self-write suppression | T2 | Pasting from Ditto does not re-capture the clip or create a loop | Integration test with expected-hash suppression |
| C9 | Max clip size + ignore rules | T2 | A clip above `max_clip_bytes` is ignored with a logged reason; excluded apps are not captured | Unit test + manual |
| C10 | Source app/title | T2 | Each clip records the foreground app and window title; no content in logs | DB inspection; log scan |
| C11 | Retention: max entries and expiry | T2/T9 | Core runs retention at startup and every 5 minutes; beyond the limit/age, unpinned unprotected clips are removed; pinned/dont-auto-delete/copy-buffer/global-shortcut clips are kept | Unit + core maintenance pass tests |
| C12 | Delete, soft delete, undo, purge | T2/T9 | Delete hides the clip; undo restores; clear history soft-deletes then purges rows and GCs blobs; periodic purge removes expired tombstones | Unit tests + maintenance pass test |

### 2.2 Search and history

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| S1 | Browse history, group filter, ungrouped | T2/T3 | The list shows recent clips, a group shows only its clips, ungrouped shows clips with no group | Query tests + manual |
| S2 | Search description/preview | T2/T3 | Substring search matches preview text; case-insensitive for ASCII; CJK substrings of ≥3 characters work | FTS + LIKE tests with EN/ZH fixtures |
| S3 | Full-text search | T2/T3 | Matches text inside a clip, not just the preview; 10k dataset search p95 < 50 ms | Benchmark in `PERFORMANCE.md` |
| S4 | Quick-paste text search | T2/T3 | `QuickPasteText` is searchable; `/q ` prefix works | Test + manual |
| S5 | Search modes: contains, wildcard, regex | T2/T3/T9 | Contains is default; wildcard `*`/`?` works; safe linear-time regex works at the storage/API layer with pattern-size and compiled-size limits; the v1 popup UI continues to send contains | Unit tests for regex validity/limits + real 10k paging |
| S6 | Search no-results/loading states | T3 | Empty state and loading skeleton are shown; no stale results flash | UI QA |
| S7 | Result count and latency indicator | T3 | Count is correct; a slow query shows a progress indicator after 150 ms | UI QA |
| S8 | Clear search / close behavior | T3 | Esc clears query first, then closes; the list returns to the group view | Keyboard QA |

### 2.3 Popup and keyboard

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| P1 | Global hotkey opens/closes popup (default Ctrl+`) | T1/T3/T11 | Hotkey works from any app; Show activates the popup for direct typing/Enter; the pre-Show foreground is snapshotted for paste; second press hides; conflict is reported | T11 keyboard-first QA + latency harness |
| P2 | Pre-warmed, hidden popup, <50 ms first frame | T1/T3/T7 | p95 < 50 ms over 100 samples; no process start on hotkey | `PERFORMANCE.md` harness |
| P3 | Bottom search with IME | T3 | Chinese/Japanese IME composition, commit, cancel, candidate window; no duplicate/lost characters | Manual IME QA (Microsoft Pinyin/Japanese) |
| P4 | Keyboard navigation | T3 | Up/Down/PgUp/PgDn/Home/End; Tab order; visible focus ring; no focus trap | Keyboard-only QA script |
| P5 | Enter paste, Shift+Enter plain text | T3/T4 | Both modes paste the selected clip; plain text writes only `CF_UNICODETEXT` | Manual into Word/Notepad/browser |
| P6 | Multi-select and paste all | T3/T4 | Ctrl/Shift select, Space toggles, Ctrl+Enter pastes all selected in order | Manual with 3 clips; separator setting |
| P7 | Special paste menu | T3/T4 | Plain text, HTML-as-text, and format-specific paste entries are available and correct | Manual |
| P8 | First-ten badges and Ctrl+1..0 | T3/T4/T11 | Badges show 1–10; configurable chords paste the corresponding visible item while the popup is visible; first-ten can be disabled | Unit index mapping + T11 settings round-trip |
| P9 | Copy-buffer badges and Alt+1..5 | T3/T4 | Badge shows B1–B5; Alt+num pastes the slot; slot contents persist | Manual + restart test |
| P10 | Pinned and pasted indicators | T3 | Pin glyph, pasted check/left bar; state is not color-only | Screenshot + a11y check |
| P11 | Empty, loading, error, disconnected states | T3 | Each state has text + action; no blank window | UI QA |
| P12 | Multi-monitor, caret/cursor/previous placement | T3 | Popup appears on the correct monitor, clamped to the work area, at 100/150/200% | Manual mixed-DPI dual monitor |
| P13 | Window styles: activation policy | T3/T11 | `WS_EX_NOACTIVATE`/`TOOLWINDOW` while hidden; hotkey Show clears NOACTIVATE and takes keyboard focus (keyboard-first trade-off); click-away hides without returning focus to the old target; Esc restores it | Window-style inspection + T11 keyboard-first QA |
| P14 | Resize and persist popup size | T3 | Drag edges; size persists; clamped to min/max/work area | Manual |
| P15 | Group breadcrumb + flyout | T3 | Path is correct; navigation works; flyout supports All/Ungrouped/tree/New group | Manual + tree fixture |
| P16 | Text/image/color/files preview | T3 | F3 toggles; text wraps; image fits; color shows values; files show names | UI QA with fixtures |
| P17 | Context menu | T3 | Paste, paste plain, copy, pin, move, copy to buffer, edit, delete; disabled states correct | Manual |
| P18 | Performance: 10k scroll 60 fps | T3/T7/T11 | No steady-state frame over 16.7 ms while scrolling 10k rows in a real SQLite/IPC session; 10 warm-up frames excluded and first-frame latency measured separately; no memory growth over 60 s | `tools/qa/scroll-10k-ipc.ps1` (300 frames) |
| P19 | Fluent light/dark/HC visuals | T3 | Tokens match `DESIGN-SYSTEM.md`; HC uses system colors; no hardcoded colors | Screenshot review + token grep |
| P20 | AccessKit semantics (Should, not Must) | T3 | Rows expose role/name/state; Narrator can read the list | Accessibility Insights/Narrator |

### 2.4 Paste, UAC, drag/drop, files

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| X1 | Target-window tracking and focus restore | T4 | Paste goes to the window that was foreground when the popup opened; no accidental paste into Ditto | Manual with Notepad/browser/Explorer |
| X2 | Normal paste via `SendInput` | T4 | Ctrl+V is delivered; clipboard formats are written in the original order | Manual + clipboard-format inspection |
| X3 | Elevated-target paste | T4 | Pasting into an elevated app works after the one-time helper handshake; no prompt loop; timeout produces a clear error | Manual elevated Notepad/Admin console |
| X4 | UAC helper security | T4 | One-time token, current-user only, no arbitrary commands/paths; helper exits; failure is logged without content | Security test + code review |
| X5 | `CF_HDROP` file clipboard | T4 | Files copied in Explorer paste into Explorer/apps; paths are correct | Manual |
| X6 | Drag out (text/image/files) | T4 | Dragging a clip to Explorer/Word produces the expected data | Manual drag/drop |
| X7 | Drop into the popup (Should) | T4/T3 | Dropping text/image/files adds a clip; no execution | Manual |
| X8 | Paste plain text | T4 | Only Unicode text is on the clipboard; rich formats are absent | Clipboard inspector |
| X9 | Clipboard restore (Should) | T4 | The previous clipboard is restored after the configured delay; failure is non-blocking | Manual |
| X10 | Paste failure paths | T4 | Target gone, clipboard busy, helper timeout, access denied: each returns a stable error and a user-visible message | Fault-injection tests |

### 2.5 Groups, pinning, copy buffers

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| G1 | Create/rename/delete groups | T2/T3/T5 | Tree updates immediately; duplicate names per parent are rejected; deleting moves clips to Ungrouped after confirmation | Unit + UI QA |
| G2 | Move clips to groups | T2/T3/T5 | Single and multi-select move; breadcrumb/list refresh; `state_rev` increments | Integration test |
| G3 | Group tree and breadcrumb | T3 | Parent/child navigation; counts; no cycles | Tree fixture + manual |
| G4 | Pin/unpin | T2/T3 | Pinned clips sort first within their group; pin order persists; pin is not color-only | Unit + UI QA |
| G5 | Copy buffers 1–5 | T2/T3/T4/T5 | Assign, paste, cut, clear; persist across restart; at least 3 slots accepted | Integration + restart test |
| G6 | Show grouped clips in main list (setting) | T2/T3/T5/T11 | Toggle changes the main list; `false` maps All to the ungrouped inbox so grouped clips are excluded; explicit group navigation is unchanged; default matches legacy | Unit provider/storage tests + `tools/qa/grouped-main.ps1` |

### 2.6 Settings, tray, lifecycle

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| A1 | Tray icon + menu | T1/T5/T7/T9 | Left click toggles; right-click menu has Open, Settings, Groups, Copy Buffers, Connect/Disconnect (pause/resume capture), Exit; labels follow EN/ZH locale files | Locale unit test + real settings screenshot; physical tray click UNVERIFIED |
| A2 | Run at startup | T1/T5 | HKCU Run value written/removed; stale path repair; portable default off | Registry inspection |
| A3 | Single instance | T1 | Second launch forwards and exits <150 ms; no second core | Process test |
| A4 | Global hotkey configuration | T1/T5/T11 | Rebind open/open-alt/plain/copy-buffer hotkeys; open/open-alt/plain/copy-buffer are global, first-ten is popup-scoped; conflicts show an error and roll back with no silent hook; copy-buffer `cut` has no global v1 action (UI disabled with explicit help) | T11 `hotkey-rebind.ps1` + swap/rollback unit tests |
| A5 | Settings window on demand | T1/T5 | Opens on demand; closes and frees WebView2; no WebView2 at startup | Process-tree snapshot + memory |
| A6 | Settings pages | T5 | General/Appearance/Hotkeys/Clipboard/Groups/Copy Buffers/Data/Languages/About, all controls persist | UI QA + settings round-trip |
| A7 | Theme/accent/density | T5/T11 | System/light/dark/HC; UI values `system/light/dark/highContrast` (legacy `hc/high_contrast`) map to popup; `system` re-resolves Windows light/dark/HC on Show and settings changes; HC uses system colors; accent validation; density changes popup and settings | `tools/qa/theme-modes.ps1` + mapping unit tests |
| A8 | EN/ZH localization | T3/T5 | All user-visible strings switch; no clipped long Chinese labels | Screenshot + string scan |
| A9 | History management in Data page | T5 | Search/filter/preview/edit/delete/group/pin/buffer actions work | UI QA |
| A10 | Diagnostics/log level | T1/T5/T11 | Log level changes at runtime through a reloadable filter (info→debug/trace emits new records); log path; diagnostics bundle without content | `tests/log_level.rs` + wiring review |
| A11 | No telemetry / no network | T1/T7 | No outbound connections except user-initiated (none in v1); no analytics | Network capture + code review |
| A12 | WebView2-missing fallback | T1/T5/T9 | Host checks the Evergreen registry/install directory before opening Settings; on failure it shows a native MessageBox with the download URL and keeps running. NSIS uses the bootstrapper; Portable README documents the requirement. | Unit detection test; missing-runtime path UNVERIFIED |

### 2.7 Data, migration, packaging

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| D1 | New SQLite schema + migrations | T2 | `user_version`, WAL, FTS5, indexes; clean install and upgrade from v1 schema | Schema tests |
| D2 | Images/large formats on disk | T2 | Blob paths, hashes, GC; DB stays small for image-heavy histories | Fixture + size report |
| D3 | Backup/restore/optimize | T2/T6 | Backup is verified; restore requires confirmation and restart; optimize reclaims space | Test + manual |
| D4 | Import/export new format | T6 | Manifest + DB + blobs; import validates hashes; merge policy documented | Round-trip test |
| D5 | Legacy DB migration | T6 | Counts/content/groups/pins/copy buffers match the synthetic fixture; old DB unchanged (SHA-256) | `cargo test` + `MIGRATION-REPORT.md` |
| D6 | Legacy settings migration | T6 | Mappable keys match; unsupported keys reported; no secrets/network/add-in keys | Mapping test |
| D7 | Dry-run + progress + rollback | T6/T9 | **v1 product decision:** migration is one-shot and staged for the next start. Dry-run reports source counts; the pending UI shows the stored total and a warning that progress is only available after restart; there is no cancel. Every apply keeps a pre-migration backup for rollback. | Integration tests + T9 report |
| D8 | Portable mode | T1/T7 | Runs from a read-only binary dir with writable `Data`; no installer/registry/admin | Portable QA |
| D9 | NSIS installer | T1/T6/T7 | Installs/uninstalls; start menu; optional autostart; no VC++ dependency surprise | Installer QA log |
| D10 | Portable ZIP | T7 | ZIP contains binaries/assets; runs after extraction; hash recorded | `cargo tauri build` + ZIP QA |
| D11 | Windows 10/11 x64 | T7 | Both OS versions tested; Windows 10 fallback visuals | QA matrix |
| D12 | WebView2 bootstrapper policy | T1/T6 | Evergreen requirement documented; installer behavior explicit | Installer QA |

### 2.8 Quality, performance, security

| ID | Feature | Stage | Acceptance | Evidence |
|---|---|---|---|---|
| Q1 | `cargo fmt --check` | T1–T7 | Passes on the integrated branch | CI log |
| Q2 | Clippy `-D warnings` | T1–T7 | Passes all targets | CI log |
| Q3 | `cargo test` | T1–T7 | Unit + integration + fixtures pass | CI log |
| Q4 | `cargo tauri build` | T1/T7 | NSIS artifact produced | Build log + artifact path |
| Q5 | Idle <100 MB | T1 spike/T7 | Median of 3 cold samples; settings closed | `PERFORMANCE.md` raw data |
| Q6 | Popup <50 ms p95 | T1/T3/T7 | 100 samples | Raw CSV/JSON |
| Q7 | 10k search <50 ms, 60 fps scroll | T2/T3/T7 | p95 search; frame times; no memory growth | Benchmark + frame log |
| Q8 | IPC ACL + allow-list | T1/T2/T8 | Wrong user/remote/bad image rejected; logs no payload | Security test |
| Q9 | UAC helper security | T4/T8 | One-time token, user-triggered only, no arbitrary execution | Code review + test |
| Q10 | No legacy DLL/script loading | T1/T6/T8 | No `LoadLibrary` of add-ins; no ChaiScript; migration imports data only | Code review + process monitor |
| Q11 | No real user data in QA | T1–T7 | Tests use synthetic fixtures or a user-provided copy; temp copies deleted | Audit of test logs |
| Q12 | GPLv3 LICENSE unchanged | T7/T8 | `LICENSE` hash matches base | `git diff` |

---

## 3. Should — ship if safe

These are valuable but can slip with coordinator approval. If they slip, the
settings UI must hide/disable the control, not show a broken promise.

| ID | Feature | Stage | Acceptance | Slip rule |
|---|---|---|---|---|
| S1 | Wildcard/regex search UI affordance | T3/T5 | A visible mode menu; invalid regex shows an error, no crash | Can ship contains+fulltext only; keep the data/API |
| S2 | Drag to reorder groups | T3/T5 | Drag/drop within the tree; order persists | Can ship move-to-group only |
| S3 | Drag/drop into popup | T3/T4 | Text/image/files dropped into the popup are captured | Can ship Explorer copy/paste only |
| S4 | Clipboard restore after paste | T4 | Previous clipboard restored after delay; opt-in | Can ship without restore |
| S5 | Sound on copy | T2/T5 | Optional system sound; off by default | Can ship muted |
| S6 | Zebra rows / transparency | T3 | Appearance options; default off; no contrast failure | Can ship without |
| S7 | Narrator live announcements and full row graph | T3 | Result counts and save/status are announced; rows read correctly | Can ship if P20 core semantics pass; record any gap |
| S8 | Settings history table virtualization | T5 | 10k rows scroll without jank | Can ship paginated 500 rows |
| S9 | Global clip hotkeys / move-to-group hotkeys | T6 | Migrated values preserved; UI deferred | Values preserved in DB, no UI |
| S10 | Multi-paste separator customization | T4/T5 | Setting changes the join; default `\r\n` | Can ship fixed `\r\n` |
| S11 | Custom accent color | T5 | Validation + fallback | Can ship System/Ditto only |
| S12 | Edit clip (F2 / settings) | T5 | Text edit updates content/hash/preview/FTS | Can ship delete/recreate workflow |
| S13 | Statistics page | T5 | Counts, DB size, top apps (no content) | Can ship in About/Data summary |

---

## 4. Later — designed for, not in v1

These must not be half-implemented or exposed as broken controls.

| ID | Feature | Why later | Data/design provision |
|---|---|---|---|
| L1 | Friends/P2P clipboard sharing | Network, encryption, discovery, firewall, abuse surface | No network columns; no settings UI; legacy password not migrated |
| L2 | Network file transfer | Depends on L1 | CF_HDROP local files only |
| L3 | Add-ins / plugins / scripts | Legacy C++ ABI + ChaiScript are security and maintenance risks | No loader; no script engine; no legacy DLL loading |
| L4 | QR code viewer | Not core; extra dependency | Can be a later clip action |
| L5 | Clip editor (rich text) | Large UI; popup/settings v1 use plain text edit only | `EditClip` API accepts plain text; raw formats preserved |
| L6 | Multiple databases | Adds lifecycle/UI complexity | Single `ditto.db`; import/export covers migration |
| L7 | 30 languages | EN/ZH first; i18n architecture supports later | Locale catalogs keyed; no hardcoded strings |
| L8 | Automatic updates | Signing/manifest/rollback complexity | Manual installer; `Check for updates` disabled with "coming later" |
| L9 | DB encryption | Key management/UX; legacy DB unencrypted | Plaintext local DB; threat model documented |
| L10 | macOS/Linux | Windows-only v1 | No cross-platform code paths |
| L11 | Per-clip global hotkeys / move-to-group hotkeys | Scope; legacy values preserved | DB columns preserved; no UI |
| L12 | Cloud sync / account | Explicit non-goal (local-first) | None |

---

## 5. Out — explicit non-goals for v1

- No telemetry, analytics, crash upload, or account.
- No remote IPC, no TCP listener, no named-pipe remote clients.
- No loading of legacy `Addins\*.dll`, ChaiScript, or legacy theme/language XML
  as code.
- No modification of the user's real `%APPDATA%\Ditto` or clipboard history
  during development/QA.
- No Defender/security-setting changes; unsigned binaries may be flagged and
  that is reported, not worked around.
- No change to the GPLv3 `LICENSE`.
- No merge to `master`, no push; the user decides integration.

---

## 6. Stage dependency and ownership

```
T1 foundation
 ├── T2 clipboard + storage + settings
 │     ├── T4 paste/UAC/drag
 │     └── T6 migration/import/export
 ├── T3 popup (mock provider; integrates T2)
 └── T5 settings (integrates T2)
        └── T7 integration + QA + packaging
              └── T8 independent review
```

| Stage | Owns the acceptance for |
|---|---|
| T1 | Scaffold, tokens, tray/hotkey skeleton, single instance, spike A/B, CI, first-run shell |
| T2 | C1–C12, S1–S5 data/query side, D1–D3, G1/G2/G4/G5 data side, Q7 storage benchmarks |
| T3 | P1–P20, S1/S2/S6 popup side, G3, popup performance/a11y |
| T4 | X1–X10, copy-buffer paste/cut, special paste |
| T5 | A5–A10, settings pages, history management, theme/accent/density, EN/ZH settings |
| T6 | D4–D7, D5/D6 migration, import/export/backup |
| T7 | Integration, D8–D12, Q1–Q12 end-to-end, screenshots, QA report |
| T8 | Independent review and verdict |

Cross-ownership notes:

- Tray is a T1 skeleton with T5 menu/commands; T7 verifies the integrated
  behavior. T3/T5 must request menu changes through the coordinator.
- Locale catalogs are shared by T3 and T5. T1 creates the skeleton; T5
  aggregates keys; T3 requests popup keys through the coordinator. Do not
  create two independent string stores.
- Design tokens are T1-owned; T3/T5 consume generated files and must not edit
  `design/tokens.json`.
- `crates/ditto-win` is T1-owned; T4 may modify it only after coordination.
- Tauri `lib.rs` registration is T1-owned; T5 adds commands under
  `apps/ditto/src-tauri/src/commands/**` and asks T1/coordinator for the
  one-line registration if its ownership is not extended.

---

## 7. Acceptance test map

| Test suite | Covers | Owner |
|---|---|---|
| `ditto-storage` unit/integration | schema, FTS, hashes, groups, retention, soft delete | T2 |
| `ditto-clipboard` synthetic formats | C1–C11, self-write, retries | T2 |
| `ditto-migration` fixtures | D5/D6, count/content parity, idempotency, rollback | T6 |
| `ditto-paste` unit | X1/X2/X8/X10 format order, plain text, errors | T4 |
| Manual UAC matrix | X3/X4 | T4/T7 |
| Popup keyboard/IME script | P3–P8, P12 | T3/T7 |
| Popup performance harness | P2/P18, Q6/Q7 | T3/T7 |
| Memory harness | Q5 | T1/T7 |
| Settings round-trip | A6/A7/A9 | T5 |
| Installer/portable matrix | D8–D12 | T7 |
| Security review | Q8/Q9/Q10 | T8 |

---

## 8. Cut-line rules

- v1 is cut at **Must** only. **Should** items may ship if they are complete and
  do not regress the Must budget; otherwise hide the control and defer.
- Never ship a visible control that silently does nothing. If a feature is
  deferred, remove/disable it and say "coming later" in the UI, or omit it.
- Deferred features must not add dependencies, background services, network
  listeners, or legacy loaders.
- Any change to the v1 cut line requires the coordinator's explicit decision and
  an update to this file and `DECISIONS.md`.
