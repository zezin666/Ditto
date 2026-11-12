# DECISIONS — confirmed decisions, T0 resolutions, and open questions

- Status: **T0 baseline; confirmed decisions are not to be changed by workers**
- Date: 2026-11-12
- Rule: user/coordinator-confirmed decisions are immutable for T1–T8. If a
  worker believes one is wrong, it must escalate with evidence; it must not
  silently deviate.
- Status values:
  - **Confirmed** — from the user/coordinator; do not change.
  - **Frozen** — T0 baseline for cross-stage consistency; change requires the
    coordinator.
  - **Delegated** — explicitly left to a later stage.
  - **Open** — needs a decision; a recommended default is given so work can
    continue.

---

## 1. Confirmed decisions (immutable)

| ID | Decision | Source |
|---|---|---|
| D-001 | Architecture: **Tauri 2** for the settings/manager process + **Rust native egui** popup. | Coordinator prompt §1.1 |
| D-002 | v1 scope: MVP + the specified advanced features (groups, pinning, copy buffers, first-ten/quick-paste keyboard, UAC admin paste, drag/drop, file clipboard). | §1.2, §1.6 |
| D-003 | Migrate legacy Ditto data; **discard old C++ DLL add-ins**. | §1.3 |
| D-004 | Visual language: **Windows 11 Fluent**. | §1.4 |
| D-005 | Popup is native **egui**, not a web UI. | §1.5 |
| D-006 | Advanced v1 features: groups/pin/copy buffers; first-ten/quick-paste keyboard; UAC admin paste/drag-drop/file transfer. | §1.6 |
| D-007 | Same repository, new branch (T1 creates `rewrite/tauri`). | §1.7 |
| D-008 | **Windows-only** v1 (Windows 10/11 x64). | §1.8 |
| D-009 | Root layout replacement; old C++ moves to `legacy/` after T1. | §1.9 |
| D-010 | Distribution: **Portable ZIP + NSIS**; **no auto-update in v1**. | §1.10 |
| D-011 | Languages: **English + Simplified Chinese**; i18n architecture for later languages. | §1.11 |
| D-012 | Performance budgets: idle **<100 MB**, popup **<50 ms**, 10k clips smooth. | §1.12, §5 |
| D-013 | Settings UI stack: **React 19 + TypeScript + Vite + Fluent UI React v9**. | §2 default assumptions |
| D-014 | Shared design tokens: **`design/tokens.json`** generates egui constants and CSS/TS variables. | §2, §3.5 |
| D-015 | Storage: **SQLite (`rusqlite` bundled) + FTS5**; images on disk; WAL; no encryption in v1. | §2 |
| D-016 | Migration scope: `Main`/`Data`/`Types`/`CopyBuffers` + mappable `Ditto.Settings`; no legacy themes/add-ins/ChaiScript/language XML. | §2 |
| D-017 | File transfer in v1 means **CF_HDROP file clipboard + drag-out/drop**; friends/P2P network transfer is deferred. | §2 |
| D-018 | Deferred: friends/P2P, add-ins/plugins/scripts, QR, statistics, clip editor, multi-DB, 30 languages, auto-update, DB encryption, macOS/Linux. | §2, Appendix B |
| D-019 | Popup layout: **bottom search**, **top minimal title area**, no right vertical caption, **default 420×480**. | §2 |
| D-020 | Privacy: development/QA must not read the real clipboard history or `%APPDATA%\Ditto\Ditto.db`; use synthetic fixtures or a user-provided copy. | §2 |
| D-021 | License: keep **GPLv3** `LICENSE`; new code is GPLv3. | §2 |
| D-022 | Do not modify Defender/system security settings; unsigned binaries may be flagged; report honestly. | §2, §5 |
| D-023 | Process model default: `Ditto.exe` (Tauri 2 host, no WebView2 at startup) + `DittoPopup.exe` (egui, resident hidden) + `DittoUacHelper.exe` (elevation). Fallback: headless `DittoCore.exe` + popup + on-demand `DittoSettings.exe`. | §2 |
| D-024 | IPC: named pipe, versioned protocol, request IDs, error codes, no remote access; Tauri settings uses in-process commands; popup uses the pipe. | §3.2 |
| D-025 | Migration: read-only legacy DB, dry-run, backup, rollback; v1 is one-shot and staged for the next start (no progress stream, no cancel). | §3.4 |
| D-026 | No telemetry/network in v1. | §5 |
| D-027 | The coordinator's task DAG and ownership matrix are authoritative. | §9, §10 |

---

## 2. T0 frozen decisions (consistency baseline)

These resolve cross-stage details. They are not user decisions, but changing
them requires the coordinator so T2–T7 do not diverge.

| ID | Decision | Rationale | Owner |
|---|---|---|---|
| D-101 | Data root: installed `%LOCALAPPDATA%\Ditto\v2\`; portable `<exe_dir>\Data\`; tests `DITTO_DATA_DIR`. Legacy `%APPDATA%\Ditto` is read-only migration input. | WAL/blobs should not roam; keeps portable self-contained; avoids clobbering legacy. | T1/T2/T6 |
| D-102 | Pipe name `\\.\pipe\ditto-<current-user-sid>` with an SDDL granting full control only to the current SID and SYSTEM (Administrators are **not** granted read access); `PIPE_REJECT_REMOTE_CLIENTS`; `FILE_FLAG_FIRST_PIPE_INSTANCE`. QA/development may append `-<suffix>` via `DITTO_PIPE_SUFFIX` without changing the ACL. | Matches the coordinator prompt; user boundary clear; QA isolation. | T1/T11 |
| D-103 | IPC framing: 24-byte header (`DTP1`, version, type, flags, length, request id) + UTF-8 JSON control payload; raw binary for blobs; max 16 MiB. | Debuggable, versionable, no arbitrary command surface. | T1/T2 |
| D-104 | Protocol v1 message names: `Hello`, `GetClips`, `GetClip`, `Search`, `Paste`, `SetPinned`, `MoveToGroup`, `Delete`, `GetGroups`, `GetSettings`, `SetSettings`, `ShowPopup`, `HidePopup`, `Shutdown`; events `ClipAdded`, `ClipUpdated`, `SettingsChanged`, `ShowPopup`, `HidePopup`. Extras are additive. | Matches the coordinator; avoids naming drift. | T1/T2 |
| D-105 | Settings stored in SQLite `settings` (JSON values); `config.json` is a tiny bootstrap file only. | Transactional, broadcastable, no dual source. | T2 |
| D-106 | Copy buffers are a normalized `copy_buffers` table with **1-based slots 1..5**; legacy 0-based `lCopyBuffer` maps by +1. | UI labels and legacy data are both unambiguous; one source of truth. | T2/T6 |
| D-107 | `Main.lDontAutoDelete` maps to `clips.dont_auto_delete`, **separate from** `is_pinned`; pinned comes from `stickyClipOrder`/`stickyClipGroupOrder`. | They are different legacy concepts; merging loses protection semantics. Needs coordinator confirmation because Appendix A simplifies it. | T2/T6 |
| D-108 | Content hash: SHA-256 over `format_name || 0x00 || bytes` of the primary format; `formats_hash` over sorted `(format_name, sha256)` pairs. BLAKE3 is allowed if T2 records it. | Deterministic, multi-format aware, content-addressable blobs. | T2 |
| D-109 | FTS5 tokenizer: `trigram` for EN/ZH substring search; 1–2 character queries fall back to `LIKE` with a bound. If the bundled SQLite lacks trigram, fall back to `unicode61` + CJK `LIKE` and record it. | CJK substring search needs trigram; avoids a custom tokenizer dependency. | T2 |
| D-110 | Migration inserts one clip per legacy `Main` row (no dedupe) and preserves `legacy_id`/`legacy_data_id` for idempotent reruns. | Data fidelity and count parity; dedupe is for new captures only. | T6 |
| D-111 | Popup default 420×480, min 360×320, max 720×800 and 90% work area; title 40, breadcrumb 32, row 36 (1 line), search 48. | Matches the confirmed layout; keyboard-first density. | T3 |
| D-112 | Settings default 900×640, min 720×480, left nav 208, nine pages (General/Appearance/Hotkeys/Clipboard/Groups/Copy Buffers/Data/Languages/About); history management lives on Data. | Matches the prompt; avoids a tenth page. | T5 |
| D-113 | Fluent colors use the approved Ditto-validated values (light/dark) and Windows system colors for HC; raw WinUI alpha resources are not copied. | Continuity with approved UI and contrast safety. | T1/T3/T5 |
| D-114 | Settings window is destroyed on close (not hidden); no WebView2 at startup; `windows: []`. | Meets idle memory and PF-9. | T1/T5 |
| D-115 | UAC helper: core writes the clipboard; helper only sends Ctrl+V via a one-time token. If T4 proves clipboard isolation requires it, the helper may write the exact payload over the same channel; no arbitrary commands. | Minimal privilege; satisfies the security goal; T4 documents any exception. | T4 |
| D-116 | Locale catalogs: one shared EN/ZH source under `apps/ditto/ui/locales/` (T1 skeleton, T5 aggregates), generated/loaded by both apps; T3 requests popup keys through the coordinator. | Avoids two string stores and drift. | T1/T3/T5 |
| D-117 | Settings webview capabilities: only the settings command namespace and window controls; no shell/HTTP/fs/process/updater plugins. | Least privilege. | T5 |
| D-118 | Legacy add-ins, ChaiScript, themes, and language XML are never loaded as code; migration reads data only. | Security and maintenance. | T1/T6 |
| D-119 | The new app does not create a taskbar button at idle; tray only. The popup is `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` while hidden; hotkey Show deliberately clears NOACTIVATE and takes keyboard focus (T11 keyboard-first), while the pre-Show target is snapshotted for paste. | Keyboard-first hotkey use; target remains the paste destination; matching the prompt's keyboard-first requirement. | T1/T3/T11 |
| D-120 | Data/backups/exports are unencrypted in v1; the UI warns; logs never contain content. | Explicit deferral, documented threat model. | T2/T7 |
| D-121 | No auto-update; manual installer only. Future updater must be signed and user-initiated. | Confirmed; reduces attack surface. | T1/T7 |
| D-122 | NSIS is per-user by default (no admin), with optional per-machine only if T1/T7 prove it is needed. Portable requires a writable `Data` dir. | Legacy is per-user; avoids elevation. | T1/T6/T7 |
| D-123 | Rust edition: **T1 decides**; T0 recommends 2024 if stable MSVC/cargo is green, otherwise 2021. | Explicitly delegated. | T1 |
| D-124 | Primary vs fallback (Plan A/B): T1 spike decides and records in `PHASE0-NOTES.md`; T7 re-verifies. | Matches the prompt's decision gate. | T1/T7 |

---

## 3. Decisions delegated downstream

| ID | Decision | Stage | Default / constraint |
|---|---|---|---|
| D-201 | Rust edition | T1 | 2024 if green, else 2021 |
| D-202 | Exact crate/module layout | T1 | `crates/ditto-model`, `ditto-ipc`, `ditto-win`, `ditto-clipboard`, `ditto-storage`, `ditto-migration`, `ditto-paste`, `ditto-uac-helper`; `apps/ditto`, `apps/ditto-popup`, `apps/ditto-uac-helper` |
| D-203 | Primary vs fallback | T1 spike/T7 | Primary A unless measured >100 MB |
| D-204 | Tauri plugin versions and exact wiring | T1 | Pin majors; `windows: []`; sidecars |
| D-205 | egui widget composition / font atlas / AccessKit coverage | T3 | Fluent tokens; keyboard-first; native EDIT fallback for IME if needed |
| D-206 | Fluent UI theme construction / page composition | T5 | React 19 + Fluent UI v9; nine pages; generated tokens |
| D-207 | Mica/Acrylic on Settings | T5 | Mica on Win11, solid on Win10 |
| D-208 | Settings window size | T5 | 900×640, min 720×480 |
| D-209 | Migration UI flow (welcome/dry-run) | T6/T9 | Offer migrate if legacy found; dry-run summary first; v1 applies once at the next start and cannot be cancelled |
| D-210 | Backup rotation details | T2/T6 | Last 3 verified; daily + before destructive ops |
| D-211 | Whether regex/wildcard search ships in v1 | T3/T5 | Should; if not, contains+fulltext only; keep the API |
| D-212 | Whether drag-to-reorder groups ships | T3/T5 | Should; move-to-group is Must |
| D-213 | Whether clipboard restore/sound ships | T4/T2 | Should; off/safe defaults |
| D-214 | History table pagination vs virtualization | T5 | Virtualize if feasible; 500-row pages acceptable |
| D-215 | Signing/authenticode | User/T7 | Unsigned acceptable if reported; do not change Defender |
| D-216 | Whether to ship arm64 | Coordinator | Out of v1; x64 under emulation |
| D-217 | Whether to bundle a fixed WebView2 runtime | T1/T6 | Evergreen + loader in v1; fixed runtime later if enterprise requests |

---

## 4. Open questions and recommended defaults

These need the coordinator/user's confirmation; work can proceed on the
recommended default unless it is marked blocking.

| ID | Question | Recommended default | Why / impact | Blocking? |
|---|---|---|---|---|
| Q-01 | Map `Main.lDontAutoDelete` to `pinned` (Appendix A) or keep it separate? | **Keep separate** (`dont_auto_delete`) and derive `pinned` from sticky columns. If the coordinator insists on the appendix, set both `pinned` and `dont_auto_delete`. | Avoids conflating "never delete" with "sticky to top"; preserves retention behavior. | No; needs confirmation |
| Q-02 | Normalized `copy_buffers` table vs `clips.copy_buffer` column? | **Normalized table** (1..5 slots), derive `copy_buffer` in summaries. | One source of truth; supports names/legacy ids cleanly. | No |
| Q-03 | Should v1 expose per-clip global hotkeys and move-to-group hotkeys? | **No UI; preserve data.** | Legacy values migrate; UI is deferred to keep scope. | No |
| Q-04 | Should v1 ship regex/wildcard search? | **Should**; ship if tests pass; otherwise contains+fulltext. | Legacy users may rely on it; not required by the prompt. | No |
| Q-05 | Copy buffer count: 3+ or legacy 5? | **5 slots** (legacy-compatible); acceptance requires at least 3. | No data loss; matches legacy. | No |
| Q-06 | Should migration run automatically on first launch? | **Offer with a dry-run summary; default "Migrate" if a legacy DB is found, "Start fresh" always available.** | User consent and transparency; avoids silent long operations. | No |
| Q-07 | Should `settings` be a JSON KV table or typed columns? | **JSON KV** with typed accessors and validation. | Extensible, fewer migrations; settings are not queried relationally. | No |
| Q-08 | Should the popup activate on show or remain no-activate? | **Activate on hotkey Show (keyboard-first).** Snapshot the prev-foreground target before activation and paste to it; Esc/toggle hide restores the target, click-away keeps the newly focused app. | Direct typing/Enter immediately after the hotkey; the earlier no-activate choice made keyboard-first use impossible. | No (T11) |
| Q-09 | Should the settings window be destroyed or hidden? | **Destroyed** after close. | Meets PF-9/idle memory. | No |
| Q-10 | Data root `%LOCALAPPDATA%\Ditto\v2` vs legacy `%APPDATA%\Ditto`? | **LocalAppData** for installed; legacy AppData read-only for migration. | WAL/blobs are machine-local; avoids roaming corruption. | No; document in release notes |
| Q-11 | Should the app support a per-machine install? | **Per-user NSIS by default**; per-machine only if required. | No admin; matches legacy. | No |
| Q-12 | Should WebView2 be Evergreen or fixed-version? | **Evergreen + loader**; fixed version later if enterprise requests. | Smaller installer; T1 verifies missing-runtime behavior. | No |
| Q-13 | Should signing be pursued in v1? | **User decision**; if unavailable, ship unsigned and report Defender warnings. | Signing costs/identity; not a code decision. | No |
| Q-14 | Is a 30-language catalog required at launch? | **No; EN/ZH only**, i18n keys ready. | Confirmed scope. | No |
| Q-15 | Should the DB be encrypted? | **No in v1**; document the local threat model; revisit DPAPI/SQLCipher later. | Confirmed deferral. | No |
| Q-16 | Should the UI show legacy theme names? | **No**; new Theme = System/Light/Dark/HC, with optional legacy import mapping. | Legacy theme XML is not migrated. | No |
| Q-17 | Should the popup keep zebra rows/transparency? | **Should, off by default**; no contrast failure. | Legacy options; not core. | No |
| Q-18 | Should the settings history table allow editing clip text? | **Should**; plain-text edit only; raw formats preserved. | Useful management; rich editor is deferred. | No |
| Q-19 | Should file drag-out be Must or Should? | **Must for CF_HDROP clipboard; drag-out Should**; if OLE/UIPI blocks it, ship copy/paste and document. | Prompt lists drag/drop; T4 tests. | No |
| Q-20 | Should `Ditto.exe` support a `--safe-mode`? | **Yes** (no popup/tray hotkeys? no: start with logging, disable migration/backup) | Diagnostics; useful for support. | No |

---

## 5. Deviations / clarifications vs the coordinator prompt

These are deliberate refinements, recorded so T2–T7 do not diverge. None changes
a confirmed user decision.

| ID | Prompt says | T0 resolution | Reason |
|---|---|---|---|
| X-01 | Appendix A maps `Main.lDontAutoDelete` → `clips.pinned` | Keep `dont_auto_delete` separate; derive `pinned` from sticky fields (Q-01) | Different semantics; avoids retention bugs |
| X-02 | Appendix A sketches `clips.copy_buffer` | Normalized `copy_buffers` table; derive in summaries (Q-02) | One source of truth; slots/names/legacy ids |
| X-03 | T4 spec: "helper 设置剪贴板并粘贴" | Core writes clipboard; helper only sends Ctrl+V; helper may write payload only if T4 proves clipboard isolation requires it (D-115) | Least privilege; the helper stays minimal |
| X-04 | Pipe name `ditto-<current-user-sid>` | Canonical SID string in the name; ACL is the boundary; QA adds an optional `-<suffix>` for isolation | Matches the prompt exactly; SID is safe in a pipe name |
| X-05 | "Rust 2024 edition (or 2021, T1 decides)" | T1 decides; T0 recommends 2024 | Explicitly delegated |
| X-06 | Idle budget measured by T1 spike | T1 decides A/B in `PHASE0-NOTES.md`; T7 re-verifies | The prompt's gate is at T1, not T7 |
| X-07 | Settings stack Fluent UI React v9 | Confirmed and used throughout the design system | No deviation |
| X-08 | v1 file transfer = CF_HDROP + drag/drop | Confirmed; friends/P2P deferred | No deviation |
| X-09 | 30 legacy languages | EN/ZH only; i18n ready | Confirmed |
| X-10 | Settings theme/accent | Use approved Ditto values + HC system colors; not raw WinUI alpha resources | Contrast and continuity |

---

## 6. Assumptions to validate (not decisions)

| ID | Assumption | Validation | Owner |
|---|---|---|---|
| V-01 | Tauri 2 can start with no window and no WebView2 process. | T1 spike process-tree snapshot | T1 |
| V-02 | Tauri no-window idle + egui popup is <100 MB. | T1 spike; T7 re-measure | T1/T7 |
| V-03 | egui/winit IME is sufficient for Chinese/Japanese search. | T3 IME test; native EDIT fallback | T3 |
| V-04 | rusqlite bundled SQLite has FTS5 + trigram. | `PRAGMA compile_options`; fallback to unicode61+LIKE | T2 |
| V-05 | Closing the settings window releases the WebView2 process tree. | T5 process-tree/memory test | T5/T7 |
| V-06 | A medium-integrity process can set the clipboard for an elevated target. | T4 test; helper payload-write exception if not | T4 |
| V-07 | `SOFTWARE\Microsoft\Windows\CurrentVersion\Run` autostart works per-user without admin. | T1/T5 registry test | T1/T5 |
| V-08 | NSIS can package Tauri sidecars and the WebView2 story cleanly. | T1/T7 clean-VM install | T1/T7 |
| V-09 | The legacy DB schema variants can be introspected and defaulted. | T6 fixtures + real-world variants (user-provided only) | T6 |
| V-10 | The popup can meet p95 <50 ms with a pre-warmed hidden window. | T3/T7 harness | T3/T7 |

---

## 7. Change control

- A worker may propose a change to a **Frozen** decision via the coordinator
  with:
  - decision ID, current text, proposed text, evidence, impact on T2–T7,
  - a migration/rollback plan if the change is data-related.
- The coordinator updates this file and notifies affected stages. No worker
  edits this file directly.
- A confirmed decision (D-001..D-027) requires the user's explicit approval to
  change.
- An **Open** question can be resolved by the coordinator; record the chosen
  option and date, then move it to §2 (Frozen) or §3 (Delegated).
- If a decision is unclear during implementation, use the recommended default
  and record it as an assumption in the stage report; do not block unless it is
  marked blocking.

---

## 8. Decision records for T0 deliverables

- `ARCHITECTURE.md` — D-023, D-101..D-107, D-114, D-119, D-121..D-124.
- `DESIGN-SYSTEM.md` — D-004, D-005, D-013, D-014, D-019, D-111..D-113.
- `DATA-MODEL.md` — D-015, D-016, D-101, D-105..D-110, D-120.
- `FEATURE-MATRIX.md` — D-002, D-006, D-017, D-018, D-026.
- `PERFORMANCE.md` — D-012, D-114, D-121, D-123.
- `SECURITY.md` — D-003, D-020..D-022, D-025, D-102, D-115, D-117, D-118.
- `OWNERSHIP.md` — D-007, D-009, D-027.
- `RISK-REGISTER.md` — cross-references all of the above.
