# T0 handoff report — architecture, design system, data model, mockups

## STATUS

**DONE** (docs/prototypes only; no Rust/Tauri code, no build config, no source changes).

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-design`

## BRANCH

`taohy/rewrite-design` (T0 design branch; T1 renames its own branch to
`rewrite/tauri` and reports the actual name).

## HEAD

- Deliverables commit: `9e6924f21c440cc4f3812b67e7a0eb0b0a4c0301`
  (`docs(rewrite): add T0 architecture, design system, data model, and mockups`)
- This report is a follow-up commit on the same branch; resolve the final SHA
  with `git rev-parse HEAD` before starting T1.

## FILES

Docs (9 required):

| File | Contents |
|---|---|
| `docs/rewrite/ARCHITECTURE.md` | Process model (Tauri no-window host + egui popup + UAC helper), Plan B fallback, IPC (pipe name/ACL/framing/messages/events/errors/version), startup/exit/crash recovery, single instance, config/portable/data roots, logging, autostart, acceptance criteria |
| `docs/rewrite/DESIGN-SYSTEM.md` | Fluent semantic tokens (light/dark/HC), typography, spacing/radius/shadow/motion/icons, popup spec (420×480, bottom search, top title, virtual list, breadcrumb, badges, preview), settings spec (nine pages, nav/header/cards), DPI 100/150/200, HC, keyboard/focus, AccessKit, `design/tokens.json` schema + generated Rust/CSS/TS outputs |
| `docs/rewrite/DATA-MODEL.md` | SQLite schema (`clips`, `clip_formats`, `groups`, `copy_buffers`, `settings`, `schema_version`, `migration_state`, `meta`, FTS5 `clips_fts`), indexes, WAL, blob store, hashing/dedupe, ordering/pinning/groups, search modes, legacy `Main`/`Data`/`Types`/`CopyBuffers` and `Ditto.Settings` mapping, migration phases/dry-run/backup/rollback/read-only source |
| `docs/rewrite/FEATURE-MATRIX.md` | v1 Must/Should/Later/Out, per-item acceptance + stage owner, dependency graph, test map, cut-line rules |
| `docs/rewrite/PERFORMANCE.md` | PF-1..PF-11 budgets, cold-boot idle protocol (3 samples, median), popup p95 <50 ms protocol, 10k search/scroll, startup, settings WebView2 lifecycle, migration throughput, tools and evidence format |
| `docs/rewrite/SECURITY.md` | Threat model, IPC ACL/auth/allow-list/no-remote, UAC one-shot token/user-triggered helper, clipboard privacy, no telemetry, DB at rest, logs/dumps redaction, Tauri capabilities/CSP, legacy code/add-ins not loaded, migration/import parser limits, supply chain, SEC-1..SEC-15 |
| `docs/rewrite/RISK-REGISTER.md` | 40 risks with impact/probability/mitigation/owner/status, top-10 summary, review cadence, escalation triggers |
| `docs/rewrite/OWNERSHIP.md` | T0–T8 path ownership, dependencies/base branches, shared-file procedure, prohibited edits, cross-ownership risks, merge policy |
| `docs/rewrite/DECISIONS.md` | Confirmed decisions D-001..D-027, frozen decisions D-101..D-124, delegated D-201..D-217, open questions Q-01..Q-20 with defaults, prompt deviations/clarifications, assumptions, change control |

Mockups (offline, self-contained; shared `mockup.css`):

| File | Contents |
|---|---|
| `artifacts/rewrite/mockups/index.html` | Overview, dimensions, token values, DPI table, links |
| `artifacts/rewrite/mockups/popup-light.html` | 420×480 popup, 10-row list, title 40, breadcrumb 32, row 36, search 48, preview state, empty/warning states, annotations, DPI table |
| `artifacts/rewrite/mockups/popup-dark.html` | Same geometry, dark tokens |
| `artifacts/rewrite/mockups/settings-light.html` | 900×640 settings, nav 208, header 64, Appearance page, Data-page history table inset, annotations, DPI table |
| `artifacts/rewrite/mockups/settings-dark.html` | Same geometry, dark tokens |
| `artifacts/rewrite/mockups/mockup.css` | Fluent light/dark mockup styles; no external assets |

## BUILD

Not applicable: T0 was restricted to docs/prototypes. No Rust/Tauri project,
no build configuration, and no source files were created or modified. The
worktree contains no Cargo project.

## TEST

- All four mockups were opened in a local Chromium/Playwright browser; the only
  console message was a `favicon.ico` 404 (not a mockup error).
- Verified: light/dark theme classes, links between light/dark/index, local
  stylesheet only, no external asset or script, annotations/DPI tables present.
- `git diff --cached --check` passed before the deliverables commit.
- No automated doc tests exist; Markdown code fences were checked for balance.

## EVIDENCE

Docs: `docs/rewrite/*.md` (9 files).
Mockups: `artifacts/rewrite/mockups/*.html` + `mockup.css` (6 files).
Commit: `9e6924f21c440cc4f3812b67e7a0eb0b0a4c0301`.

Key legacy sources read (read-only, from the base worktree/master tree):

- `CLAUDE.md` (repository root, untracked in the base worktree) — build,
  lifecycle, data layer, add-ins, localization/themes, friends, scripting.
- `src/CP_Main.cpp` — `InitInstance`, command-line switches, single-instance
  mutex, UAC helper launch, hotkeys/tray/copy-thread startup, exit.
- `src/MainFrm.cpp` — hidden main frame, tray commands, copy-buffer hotkeys
  (0-based slots 0..4).
- `src/CopyThread.cpp`, `src/ClipboardViewer.cpp` — clipboard listener,
  capture pipeline, `AddClipboardFormatListener`/`SetClipboardViewer`.
- `src/Clip.cpp` — `AddToDataTable`, `LoadFormats`, CRC generation, format
  mapping/description extraction.
- `src/ClipIds.cpp` — export DB schema, id-based delete.
- `src/DittoCopyBuffer.cpp` — copy-buffer 0-based slot behavior.
- `src/UAC_Thread.cpp`, `src/UAC_Helper.cpp` — legacy `/uacpaste:<pid>`,
  elevation detection and SendInput path.
- `src/Options.cpp` / `src/Options.h` — `Ditto.Settings`, registry fallback,
  `DBPath3`, defaults (hotkey 704, max entries 500/portable 100, expiry 5d),
  copy-buffer keys, language/theme keys.
- `src/MultiLanguage.cpp`, `Debug/Language/English.xml`,
  `Debug/Language/Chinese Simplified.xml` — language file model (`LanguageCode`
  CHN) and 30 checked-in languages.
- `src/Theme.cpp`, `Debug/Themes/Modern Light.xml`, `Modern Dark.xml` — theme
  XML model and 14 checked-in themes.
- `src/DatabaseUtilities.cpp` — `CreateDB`, `ValidDB`, indexes/triggers,
  backup/compact/repair, retention.
- `src/QPasteWnd.cpp`, `src/QListCtrl.cpp` — popup/list/search/group/first-ten
  behavior.
- `src/Misc.cpp` — stable `GetFormatName`/`GetFormatID` names.
- `Shared/DittoDefines.h`, `Shared/IClip.h` — legacy add-in ABI and format
  constants (not ported).
- `DittoSetup/DittoSetup_10.iss`, `DittoSetup/BuildPortableZIP.bat`,
  `DittoSetup/Portable DittoSetup.iss` — install layout, HKCU Run value,
  `portable` marker, `Ditto.Settings`, language/theme asset layout.
- `docs/ui/CURRENT-STATE.md`, `docs/ui/DESIGN-SYSTEM.md` — prior approved UI
  audit/tokens used as the continuity baseline.
- Coordinator prompt `ORCA_TAURI_REWRITE_PROMPT.md` — confirmed decisions,
  defaults, DAG, ownership, performance/security requirements.

## UNVERIFIED

- No Rust/Tauri/egui build or runtime test was performed (T0 is docs-only).
- Rust edition (2024 vs 2021) is for T1 to decide.
- Tauri no-window idle memory and egui popup first-frame are not measured yet;
  T1 must run the spike and record it in `docs/rewrite/PHASE0-NOTES.md`, and T7
  re-verifies. Plan A/B is not selected yet.
- egui IME/accessibility, FTS5 trigram availability, WebView2 release on close,
  UAC clipboard isolation, NSIS sidecar packaging, and Defender behavior are
  assumptions to validate in T1–T7.
- Mockups are design references, not screenshots; runtime fonts, DWM,
  WebView2/egui rasterization, and HC system colors may differ.
- No real user `%APPDATA%\Ditto` or clipboard history was read.

## RISKS

Top open risks (details in `RISK-REGISTER.md`):

1. R-01 Tauri no-window host >100 MB → Plan B.
2. R-02 egui IME for Chinese/Japanese → native `EDIT` fallback.
3. R-06 migration data loss/corruption → read-only copy, counts/hashes,
   dry-run, backup/rollback, fixtures.
4. R-04 popup first frame >50 ms → pre-warm, snapshot, no blocking work.
5. R-05 10k search/scroll budget → trigram FTS, indexes, virtualization.
6. R-10 UAC helper security/compatibility → one-shot token, user-triggered,
   minimal helper.
7. R-08 clipboard format fidelity → raw bytes + synthetic tests.
8. R-18 scope creep → feature matrix cut line and coordinator gate.
9. R-15 Rust/Tauri toolchain install failure → report BLOCKED, do not fake.
10. R-17 unsigned binaries/Defender → report, do not change security settings.

## OPEN QUESTIONS / DECISIONS NEEDED

Full list with defaults: `docs/rewrite/DECISIONS.md` §4. The ones worth the
coordinator's attention before T2/T6:

- **Q-01 / X-01:** keep `Main.lDontAutoDelete` as `clips.dont_auto_delete`
  (recommended) or follow Appendix A's simplified `clips.pinned` mapping.
- **Q-02 / X-02:** normalized `copy_buffers` table (recommended) vs a
  `clips.copy_buffer` column.
- **X-03:** core writes the clipboard and the UAC helper only sends Ctrl+V
  (recommended); allow the helper to write the exact payload only if T4 proves
  clipboard isolation requires it.
- Data root `%LOCALAPPDATA%\Ditto\v2` (recommended) vs legacy `%APPDATA%`.
- FTS5 trigram (recommended) vs unicode61 + CJK `LIKE` fallback.
- EN/ZH locale source under `apps/ditto/ui/locales/` (T5 aggregates).
- Whether regex/wildcard search, drag-to-reorder groups, clipboard restore,
  and sound ship as Should in v1.

## NEXT

1. Start T1 with base `taohy/rewrite-design`; T1 renames its own branch to
   `rewrite/tauri` and reports the actual branch/SHA.
2. Coordinator resolves Q-01/Q-02/X-03 (and any other open question) before
   T2/T6 encode the schema/mapping.
3. T1 runs the Plan A/B spike and records raw memory/process data in
   `docs/rewrite/PHASE0-NOTES.md`.
4. T2/T3/T4/T5/T6 use `OWNERSHIP.md`; cross-ownership edits go through the
   coordinator.
5. No push, no merge to `master`; the user decides integration.
