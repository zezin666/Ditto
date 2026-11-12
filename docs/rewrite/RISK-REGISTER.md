# RISK-REGISTER — Ditto Rust/Tauri rewrite

- Status: **T0 baseline, reviewed at every stage boundary**
- Date: 2026-11-12
- Scale: Impact (I) and Probability (P) are High / Medium / Low.
- Owner: the stage that must mitigate or escalate; the coordinator owns
  cross-stage decisions.
- Rule: a risk that becomes an issue is escalated with evidence; it is not
  silently absorbed by changing scope or lowering a budget.

---

## 1. Top risks (summary)

| Rank | ID | Risk | I | P | Mitigation | Owner |
|---:|---|---|---|---|---|---|
| 1 | R-01 | Tauri no-window host exceeds 100 MB idle | High | Medium | T1 spike; Plan B headless core | T1/T7 |
| 2 | R-02 | egui IME (Chinese/Japanese) breaks search input | High | Medium | Early IME test; native `EDIT` fallback | T3 |
| 3 | R-06 | Migration loses or corrupts history | High | Medium | Read-only copy, counts/hashes, dry-run, backup, rollback, fixtures | T6 |
| 4 | R-04 | Popup first frame misses 50 ms p95 | High | Medium | Pre-warm, snapshot show, no blocking, measure | T3/T7 |
| 5 | R-05 | 10k search/scroll misses budget | High | Medium | Trigram FTS, indexes, virtualization, benchmark | T2/T3/T7 |
| 6 | R-10 | UAC helper security or compatibility failure | High | Medium | One-shot token, user-triggered, minimal code, tests | T4 |
| 7 | R-08 | Clipboard format fidelity loss | High | Medium | Raw bytes, synthetic format tests, DIB→PNG plus original | T2 |
| 8 | R-18 | Scope creep (friends, add-ins, 30 languages) | High | High | Feature matrix cut line; coordinator gate | Coordinator |
| 9 | R-15 | Rust/Tauri toolchain install/network failure | High | Medium | winget/direct install; report BLOCKED; no fake build | T1 |
| 10 | R-17 | Unsigned binaries flagged by Defender | Medium | High | Report; sign later; never change security settings | T7/T8 |

---

## 2. Full register

### R-01 — Tauri no-window host memory

| Field | Value |
|---|---|
| Category | Architecture / performance |
| Impact | High (fails PF-1, forces redesign) |
| Probability | Medium |
| Trigger | T1 spike median idle private working set ≥ 100 MB, or any WebView2 process at idle |
| Mitigation | `windows: []` at startup; no hidden webview; measure core+popup only; keep domain crates Tauri-free; Plan B = `DittoCore.exe` headless + on-demand `DittoSettings.exe` |
| Contingency | Switch to Plan B in T1; re-measure; if Plan B also ≥100 MB, escalate to user for a budget/architecture decision |
| Owner | T1 (decision), T7 (re-verify), T8 (review) |
| Status | Open; decision gate in `PHASE0-NOTES.md` |

### R-02 — egui IME and text input

| Field | Value |
|---|---|
| Category | UX / platform |
| Impact | High (search unusable for Chinese/Japanese) |
| Probability | Medium |
| Trigger | Composition duplication/loss, candidate window misplacement, Enter during composition pastes |
| Mitigation | Early IME test in T3; handle winit/egui IME events; test Microsoft Pinyin/Japanese; fallback to a native Win32 `EDIT` for the search field while keeping the Fluent frame |
| Contingency | Native search field; document the deviation and re-test keyboard/accessibility |
| Owner | T3 |
| Status | Open |

### R-03 — egui accessibility / AccessKit

| Field | Value |
|---|---|
| Category | Accessibility |
| Impact | Medium (Narrator users cannot use the popup) |
| Probability | Medium |
| Trigger | Custom rows expose no name/role/state; focus not announced; no list item count |
| Mitigation | Enable AccessKit; expose list/listitem nodes with index, preview, state; keyboard full path; test with Narrator/Accessibility Insights |
| Contingency | Ship keyboard-only with a documented a11y gap only with coordinator approval; plan a follow-up |
| Owner | T3/T7 |
| Status | Open |

### R-04 — Popup first-frame latency

| Field | Value |
|---|---|
| Category | Performance |
| Impact | High (core UX promise) |
| Probability | Medium |
| Trigger | p95 ≥ 50 ms or any sample > 150 ms |
| Mitigation | Resident hidden popup; pre-warmed fonts/textures; core sends a snapshot; no DB/thumbnail/font work on show; optional fade after first frame; measure p50/p95/p99 |
| Contingency | Pre-rendered first frame, smaller page, or native first paint; do not hide the regression |
| Owner | T1/T3/T7 |
| Status | Open |

### R-05 — 10k search and scroll

| Field | Value |
|---|---|
| Category | Performance / data |
| Impact | High (v1 acceptance) |
| Probability | Medium |
| Trigger | Search p95 ≥ 50 ms; scroll < 60 fps; memory growth |
| Mitigation | FTS5 trigram; keyset pagination; indexes for sorts/groups; virtualized rows; bounded thumbnail cache; fixture benchmark |
| Contingency | Optimize query/index; reduce initial page; add a deferred count; document any Should-mode cost |
| Owner | T2/T3/T7 |
| Status | Open |

### R-06 — Migration data loss/corruption

| Field | Value |
|---|---|
| Category | Data integrity |
| Impact | High (user history loss) |
| Probability | Medium |
| Trigger | Count mismatch; sample hash mismatch; source checksum changes; partial import |
| Mitigation | Read-only source copy; checksum before/after; one clip per legacy row (no dedupe); idempotent `legacy_id`; dry-run; counts/hash sampling; backup; rollback; synthetic fixtures |
| Contingency | Abort, preserve source, restore backup, report; never "repair" the original |
| Owner | T6 |
| Status | Open |

### R-07 — Legacy schema variations

| Field | Value |
| ---- | --- |
| Category | Data / compatibility |
| Impact | Medium (some users cannot migrate) |
| Probability | High |
| Trigger | Missing columns/tables in older DBs; corrupt WAL; unusual group parent values |
| Mitigation | `PRAGMA table_info` introspection; defaults for missing columns; cycle/depth limits; WAL copy; salvage mode with warnings |
| Contingency | Report unsupported variants; offer export/salvage; do not silently drop rows |
| Owner | T6 |
| Status | Open |

### R-08 — Clipboard format fidelity

| Field | Value |
|---|---|
| Category | Core functionality |
| Impact | High (pasted content differs) |
| Probability | Medium |
| Trigger | HTML/RTF/CF_DIB/HDROP/custom formats are altered or dropped |
| Mitigation | Store raw bytes; preserve format order; DIB→PNG plus original; synthetic multi-format tests; paste-back checks |
| Contingency | If a conversion is unsafe, preserve raw and skip the preview; report |
| Owner | T2 |
| Status | Open |

### R-09 — Clipboard races and `OpenClipboard` failures

| Field | Value |
| --- | --- |
| Category | Platform |
| Impact | Medium (missed clips) |
| Probability | High |
| Trigger | Another app holds the clipboard; rapid copies; explorer restart |
| Mitigation | Debounce 20–50 ms; retry 5×/10 ms; capture worker off UI; re-add listener on `TaskbarCreated`; self-write suppression |
| Contingency | Log the failure (no content); do not crash or block; user can recopy |
| Owner | T2 |
| Status | Open |

### R-10 — UAC helper security/compatibility

| Field | Value |
| --- | --- |
| Category | Security / platform |
| Impact | High (privilege escalation or paste failure) |
| Probability | Medium |
| Trigger | Token replay, wrong parent, arbitrary command, paste into the wrong window, AV/EDR block |
| Mitigation | One-shot 32-byte token, TTL, PID/SID/window binding, inherited channel, no command line secret, minimal helper, user-triggered only, timeout; test normal/elevated targets |
| Contingency | Fail closed with `DITTO_E_ELEVATION`; document backend limitations; never auto-elevate |
| Owner | T4/T8 |
| Status | Open |

### R-11 — UAC helper blocked by AV/EDR

| Field | Value |
| --- | --- |
| Category | Security / distribution |
| Impact | Medium (elevated paste unavailable) |
| Probability | Medium |
| Trigger | Helper flagged/quarantined; `runas` blocked by policy |
| Mitigation | Sign binaries when possible; minimal behavior; no injection; document; test on a clean VM |
| Contingency | Report the limitation; do not disable security tools; offer a manual paste path |
| Owner | T4/T7 |
| Status | Open |

### R-12 — WebView2 missing or changed

| Field | Value |
| --- | --- |
| Category | Platform / packaging |
| Impact | Medium (Settings unavailable) |
| Probability | Medium |
| Trigger | Evergreen runtime absent/blocked; loader missing; WebView2 update breakage |
| Mitigation | Bundle the loader if required; check runtime; actionable native error; popup/core work without it; document the bootstrapper policy |
| Contingency | Settings disabled with a link to install Evergreen; core/popup unaffected |
| Owner | T1/T5/T7 |
| Status | Open |

### R-13 — WebView2 memory not released

| Field | Value |
| --- | --- |
| Category | Performance |
| Impact | Medium (idle budget creep) |
| Probability | Medium |
| Trigger | `msedgewebview2.exe` persists after Settings close; memory delta >10 MB |
| Mitigation | Destroy the window (not hide); verify process tree after 5 s; no webview at startup; measure |
| Contingency | Document the residual process if it is a shared runtime; do not kill unrelated processes |
| Owner | T5/T7 |
| Status | Open |

### R-14 — Tauri/plugin/React API churn

| Field | Value |
| --- | --- |
| Category | Build/toolchain |
| Impact | Medium (build breakage, rework) |
| Probability | Medium |
| Trigger | Major-version changes; plugin API changes; Fluent UI v9 changes |
| Mitigation | Pin versions and lockfiles; T1 spike validates the stack; minimal plugin surface; weekly build |
| Contingency | Pin to the last working version; isolate host wiring in `apps/ditto` |
| Owner | T1/T5 |
| Status | Open |

### R-15 — Rust/Tauri toolchain install/network failure

| Field | Value |
| --- | --- |
| Category | Environment |
| Impact | High (project cannot start) |
| Probability | Medium |
| Trigger | `winget`/rustup download fails; MSVC/SDK missing; proxy offline |
| Mitigation | Document winget and direct-download paths; verify versions; report BLOCKED with exact error; do not fake builds |
| Contingency | Coordinator/user resolves the environment; no source workaround claimed as success |
| Owner | T1 |
| Status | Open |

### R-16 — NSIS + sidecar packaging

| Field | Value |
| --- | --- |
| Category | Distribution |
| Impact | Medium (installer/portable broken) |
| Probability | Medium |
| Trigger | Sidecar path resolution, missing WebView2 loader, VC++ dependency, per-user install, uninstall leftovers |
| Mitigation | Tauri sidecars with install-dir-relative paths; clean-VM install/uninstall/portable tests; explicit WebView2 policy |
| Contingency | Fix the packaging script; T7 blocks the release until install/uninstall/portable pass |
| Owner | T1/T6/T7 |
| Status | Open |

### R-17 — Unsigned binaries flagged by Defender/SmartScreen

| Field | Value |
| --- | --- |
| Category | Distribution / security |
| Impact | Medium (install/run friction) |
| Probability | High |
| Trigger | SmartScreen warning, Defender quarantine, enterprise policy block |
| Mitigation | Report honestly; sign when possible; do not modify Defender/exclusions; document hashes; test on a clean VM |
| Contingency | User decides on signing; release notes state the status |
| Owner | T7/T8 |
| Status | Open |

### R-18 — Scope creep

| Field | Value |
| --- | --- |
| Category | Delivery |
| Impact | High (v1 slips or quality drops) |
| Probability | High |
| Trigger | Friends/add-ins/30 languages/updater/encryption/editor requests during implementation |
| Mitigation | `FEATURE-MATRIX.md` cut line; Must/Should/Later/Out; coordinator gate for any change; no half-implemented controls |
| Contingency | Defer with a recorded decision; do not add dependencies or network listeners |
| Owner | Coordinator |
| Status | Open |

### R-19 — Cross-ownership conflicts

| Field | Value |
| --- | --- |
| Category | Process |
| Impact | Medium (merge conflicts, duplicated code) |
| Probability | High |
| Trigger | Two tasks edit `ditto-win`, `lib.rs`, locales, IPC types, or tokens |
| Mitigation | `OWNERSHIP.md`; one owner per directory; coordinator authorization for cross-ownership edits; small merges |
| Contingency | Revert the cross-owned edit, hand it to the owner, rebase |
| Owner | Coordinator/T7 |
| Status | Open |

### R-20 — Token drift / hardcoded design values

| Field | Value |
| --- | --- |
| Category | Design quality |
| Impact | Medium (inconsistent UI, a11y regressions) |
| Probability | Medium |
| Trigger | Hardcoded colors/sizes; generated files edited; light/dark diverge |
| Mitigation | `design/tokens.json` single source; generated Rust/CSS/TS; CI drift check; review checklist |
| Contingency | Regenerate; reject the PR if generated files are hand-edited |
| Owner | T1/T3/T5 |
| Status | Open |

### R-21 — EN/ZH string gaps and clipping

| Field | Value |
| --- | --- |
| Category | Localization |
| Impact | Medium |
| Probability | Medium |
| Trigger | Missing keys; English fallback shown; long Chinese labels clipped at 200% |
| Mitigation | Shared locale catalogs; key lint; longest-string screenshots; pseudo-localization; T5 aggregates |
| Contingency | Fix the key/layout; do not ship a raw key or blank label |
| Owner | T3/T5 |
| Status | Open |

### R-22 — DPI/multi-monitor placement

| Field | Value |
| --- | --- |
| Category | UX / platform |
| Impact | Medium |
| Probability | Medium |
| Trigger | Popup straddles monitors, is off-screen, blurry, or clipped at 150/200% |
| Mitigation | Per-monitor DPI awareness; logical coordinates; clamp to work area; `ScaleFactorChanged` relayout; test matrix |
| Contingency | Reposition, shrink to min, or center on the active monitor |
| Owner | T3 |
| Status | Open |

### R-23 — High-contrast failures

| Field | Value |
| --- | --- |
| Category | Accessibility |
| Impact | Medium |
| Probability | Medium |
| Trigger | Custom surfaces ignore forced colors; focus invisible; disabled text unreadable |
| Mitigation | System colors, `forced-colors`, 1px borders, no shadows, test HC Black/White |
| Contingency | Fix before release; HC is a v1 acceptance item |
| Owner | T3/T5 |
| Status | Open |

### R-24 — Startup/single-instance races

| Field | Value |
| --- | --- |
| Category | Lifecycle |
| Impact | Medium |
| Probability | Medium |
| Trigger | Two cores, stale mutex, pipe squatting, duplicate popup |
| Mitigation | Mutex + `FILE_FLAG_FIRST_PIPE_INSTANCE`; CLI forward; stale-core detection; backoff; tests |
| Contingency | Fail with an explicit error; `--repair` with confirmation; never kill silently |
| Owner | T1 |
| Status | Open |

### R-25 — Crash/restart loops

| Field | Value |
| --- | --- |
| Category | Reliability |
| Impact | Medium |
| Probability | Low |
| Trigger | Popup/core crash loop; watchdog fights |
| Mitigation | Exponential backoff, max 3 attempts/10 min, log, stop and show an error |
| Contingency | User can restart manually; diagnostics bundle |
| Owner | T1/T3 |
| Status | Open |

### R-26 — Logging privacy leak

| Field | Value |
| --- | --- |
| Category | Privacy |
| Impact | High |
| Probability | Low |
| Trigger | Clip text/query/title/bytes appear in logs or dumps |
| Mitigation | Structured fields only; redaction; review; automated log scan (SEC-9); dumps not uploaded |
| Contingency | Release blocker; purge logs; fix and re-test |
| Owner | T7/T8 |
| Status | Open |

### R-27 — GPLv3 / license contamination

| Field | Value |
| --- | --- |
| Category | Legal |
| Impact | High |
| Probability | Low |
| Trigger | Incompatible dependency; LICENSE changed; missing source offer |
| Mitigation | Keep `LICENSE` unchanged; audit dependencies; include third-party notices; publish source |
| Contingency | Replace the dependency; restore LICENSE; legal review |
| Owner | T8/Coordinator |
| Status | Open |

### R-28 — Supply-chain compromise

| Field | Value |
| --- | --- |
| Category | Security |
| Impact | High |
| Probability | Low |
| Trigger | Malicious crate/npm package; unlocked dependency; build-time download |
| Mitigation | Lockfiles; pinned versions; `cargo audit`; minimal deps; no remote build scripts; review new deps |
| Contingency | Pin/remove the package; rebuild; incident note |
| Owner | T1/T8 |
| Status | Open |

### R-29 — Performance measurement noise

| Field | Value |
| --- | --- |
| Category | Process/performance |
| Impact | Medium |
| Probability | High |
| Trigger | VM/shared runner, debug build, AV scan, WPR overhead, different hardware |
| Mitigation | Fixed QA machine, release build, clean idle protocol, raw CSV, hardware profile, median of 3 |
| Contingency | Re-measure; do not compare across machines without a note |
| Owner | T1/T7 |
| Status | Open |

### R-30 — Memory growth / leaks

| Field | Value |
| --- | --- |
| Category | Performance |
| Impact | Medium |
| Probability | Medium |
| Trigger | Scroll/search leaks; unbounded caches; per-row textures |
| Mitigation | Virtualization, bounded LRU, no per-row allocations, 60 s soak, handle count |
| Contingency | Fix before release; regression test |
| Owner | T3/T2 |
| Status | Open |

### R-31 — Accent/theme contrast

| Field | Value |
| --- | --- |
| Category | Visual/a11y |
| Impact | Medium |
| Probability | Low |
| Trigger | OS accent fails contrast; custom accent unreadable |
| Mitigation | Validate candidate; fallback; HC system colors |
| Contingency | Fall back to Ditto Blue; document |
| Owner | T3/T5 |
| Status | Open |

### R-32 — Settings bundle/startup cost

| Field | Value |
| --- | --- |
| Category | Performance |
| Impact | Low |
| Probability | Medium |
| Trigger | Large React/Fluent bundle; slow first paint |
| Mitigation | Tree shaking, code splitting, on-demand window, no remote assets |
| Contingency | Accept a slower Settings open (still <1.5 s); not on the hotkey path |
| Owner | T5 |
| Status | Open |

### R-33 — OLE drag/drop complexity

| Field | Value |
| --- | --- |
| Category | Platform |
| Impact | Medium |
| Probability | Medium |
| Trigger | Drag-out/drop fails in Explorer/Word; UIPI blocks between integrity levels |
| Mitigation | T4 tests; CF_HDROP baseline; fallback to copy/paste; explicit error |
| Contingency | Ship file clipboard; mark drag-out Should/deferred if blocked |
| Owner | T4 |
| Status | Open |

### R-34 — Legacy settings misinterpretation

| Field | Value |
| --- | --- |
| Category | Migration |
| Impact | Medium |
| Probability | Medium |
| Trigger | Wrong key mapping, 0-based vs 1-based buffers, theme names, language names |
| Mitigation | Mapping table in `DATA-MODEL.md`; fixture tests; unknown keys reported; preserve raw values |
| Contingency | Fix mapping; do not overwrite user settings silently |
| Owner | T6 |
| Status | Open |

### R-35 — Data root permissions / portable read-only

| Field | Value |
| --- | --- |
| Category | Deployment |
| Impact | Medium |
| Probability | Medium |
| Trigger | Install dir not writable; `%LOCALAPPDATA%` blocked; portable on a read-only USB |
| Mitigation | Installed mode uses LocalAppData; portable requires a writable `Data`; clear error; ACL test |
| Contingency | Tell the user which path is not writable; offer a different data dir only with explicit user action |
| Owner | T1/T7 |
| Status | Open |

### R-36 — Backup/restore corruption

| Field | Value |
| --- | --- |
| Category | Data integrity |
| Impact | High |
| Probability | Low |
| Trigger | Backup written during a write; restore replaces a good DB with a bad one |
| Mitigation | SQLite backup API/`VACUUM INTO`; `quick_check`; rotate only after verification; restore to temp + swap |
| Contingency | Keep the previous DB; restore the backup; report |
| Owner | T2/T6 |
| Status | Open |

### R-37 — Autostart stale path

| Field | Value |
| --- | --- |
| Category | Deployment |
| Impact | Low |
| Probability | Medium |
| Trigger | App moved/uninstalled; HKCU Run points to a missing exe |
| Mitigation | Settings detects and offers repair; installer updates/removes the value |
| Contingency | User disables autostart; no repeated error dialogs |
| Owner | T1/T5 |
| Status | Open |

### R-38 — Windows 10 vs 11 differences

| Field | Value |
| --- | --- |
| Category | Platform |
| Impact | Medium |
| Probability | Medium |
| Trigger | Mica/rounded corners/icons unavailable; different DWM behavior |
| Mitigation | Solid fallback, square corners on Win10, Segoe MDL2 fallback; test both |
| Contingency | Degrade gracefully; do not require Win11 |
| Owner | T3/T5/T7 |
| Status | Open |

### R-39 — Arm64

| Field | Value |
|---|---|
| Category | Platform |
| Impact | Low |
| Probability | Low |
| Trigger | User expects native arm64 |
| Mitigation | Out of v1; x64 runs under emulation on arm64 Windows |
| Contingency | Later task |
| Owner | Coordinator |
| Status | Out of scope |

### R-40 — Legacy feature expectations

| Field | Value |
|---|---|
| Category | Product |
| Impact | High (user disappointment) |
| Probability | Medium |
| Trigger | Users expect friends/P2P, add-ins, 30 languages, auto-update, DB encryption |
| Mitigation | Release notes and Settings clearly show deferred features; no broken controls; explain the security/scope reasons |
| Contingency | Collect feedback; schedule later tasks; do not silently promise |
| Owner | Coordinator/T7 |
| Status | Open |

---

## 3. Risk review cadence

| Gate | Review |
|---|---|
| T0 → T1 | Confirm top risks and the Plan A/B trigger; no open blocker without an owner |
| T1 | R-01, R-14, R-15, R-24: spike results, toolchain, startup |
| T2 | R-05, R-07, R-08, R-09, R-30, R-36: storage/clipboard |
| T3 | R-02, R-03, R-04, R-22, R-23, R-30: popup/IME/a11y/perf |
| T4 | R-10, R-11, R-33: UAC/drag |
| T5 | R-12, R-13, R-21, R-31, R-32: settings/webview/localization |
| T6 | R-06, R-07, R-34, R-36: migration/import/backup |
| T7 | All Must risks; R-17, R-29: QA/performance/packaging |
| T8 | R-10, R-26, R-27, R-28: security/license review |

## 4. Escalation triggers

Escalate to the coordinator/user immediately when:

- R-01 triggers and Plan B also fails the budget;
- R-02 has no acceptable IME path;
- R-06 cannot guarantee data integrity;
- R-10 has a security gap that cannot be closed;
- R-15 blocks the toolchain;
- R-17 blocks QA and signing is unavailable;
- R-18 requires a scope change;
- any risk requires reading real user data or changing system security settings.

Escalation includes: risk ID, observed evidence, impact, options, recommended
option, and what is blocked.
