# OWNERSHIP — T1–T8 file ownership and cross-task rules

- Status: **T0 baseline, frozen for T1–T8**
- Date: 2026-11-12
- Purpose: prevent merge conflicts and accidental edits across Orca worktrees.
- Authority: the coordinator's rewrite prompt §10 and the task specs. This file
  makes the boundaries explicit and adds cross-ownership risk notes.

---

## 1. Global rules

1. **One owner per path.** A task edits only the paths it owns (§3) or a path the
   coordinator explicitly grants for a single change.
2. **Ask before crossing ownership.** If a task needs a change outside its
   paths, send the coordinator a question; do not make the edit "just this
   once".
3. **Never edit the user's `master` worktree.** All work happens in the task's
   Orca worktree. The base branch is read-only reference.
4. **Never push or merge `master`.** The final branch is the user's decision.
5. **`legacy/` is read-only** after T1 moves the old C++ tree. Do not edit,
   compile, or reformat it.
6. **`LICENSE` is read-only.** It stays GPLv3, byte-for-byte.
7. **No real user data.** Do not read `%APPDATA%\Ditto`, the real clipboard
   history, or a real `Ditto.db` unless the user explicitly provides a copy.
8. **No Defender/security-setting changes.** Report unsigned-binary warnings.
9. **No new framework** unrelated to the confirmed architecture (Tauri 2,
   egui, React 19 + Fluent UI React v9, SQLite, small Rust crates).
10. **No fake success.** Builds/tests/evidence must be real. If blocked, report
    `BLOCKED` with the exact error.

---

## 2. Stage dependencies and base branches

| Stage | Depends on | Base branch | Worktree (per prompt) |
|---|---|---|---|
| T0 | — | `master` @ `4719eea` | `rewrite-design` |
| T1 | T0 | `T0_BRANCH` (then renames to `rewrite/tauri`) | `rewrite-foundation` |
| T2 | T1 | `rewrite/tauri` | `rewrite-core` |
| T3 | T1 (integrates T2) | `rewrite/tauri` | `rewrite-popup` |
| T4 | T2 | `rewrite/tauri` | `rewrite-paste` |
| T5 | T1 (integrates T2) | `rewrite/tauri` | `rewrite-settings` |
| T6 | T2 | `rewrite/tauri` | `rewrite-migration` |
| T7 | T2/T3/T4/T5/T6 | `rewrite/tauri` | `rewrite-integration` |
| T8 | T7 | T7 branch | `rewrite-review` |

- T1 must report `REWRITE_BRANCH` (expected `rewrite/tauri`) and its SHA.
- T7 merges T2/T3/T4/T5/T6 with `--no-ff`; semantic conflicts go back to the
  owning stage, not "fixed" silently.
- T8's branch is read-only except for `docs/rewrite/CODE-REVIEW.md`.

---

## 3. Path ownership

### T0 — design (this task)

| Path | Allowed |
|---|---|
| `docs/rewrite/ARCHITECTURE.md` | Create/update |
| `docs/rewrite/DESIGN-SYSTEM.md` | Create/update |
| `docs/rewrite/DATA-MODEL.md` | Create/update |
| `docs/rewrite/FEATURE-MATRIX.md` | Create/update |
| `docs/rewrite/PERFORMANCE.md` | Create/update |
| `docs/rewrite/SECURITY.md` | Create/update |
| `docs/rewrite/RISK-REGISTER.md` | Create/update |
| `docs/rewrite/OWNERSHIP.md` | Create/update |
| `docs/rewrite/DECISIONS.md` | Create/update |
| `artifacts/rewrite/mockups/**` | Create/update HTML/SVG mockups |

T0 must not create a Rust project, edit source/build config, or touch
`artifacts/ui/**`.

### T1 — foundation

| Path | Allowed |
|---|---|
| Root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.gitignore`, `README.md`, `CLAUDE.md` | Create/update |
| `legacy/**` | Move old C++ files only; no edits after |
| `crates/ditto-model/**`, `crates/ditto-ipc/**`, `crates/ditto-win/**`, `crates/ditto-clipboard/**`, `crates/ditto-storage/**`, `crates/ditto-migration/**`, `crates/ditto-paste/**`, `crates/ditto-uac-helper/**` | Create compilable skeletons |
| `apps/ditto/**` (including `src-tauri/src/lib.rs`, `tauri.conf.json`, `capabilities/**`) | Create skeleton; T5 edits UI/commands/config later under its ownership |
| `apps/ditto-popup/**`, `apps/ditto-uac-helper/**` | Create skeletons |
| `design/**` | `tokens.json`, schema, generator, generated outputs |
| `.github/workflows/rewrite-*.yml` | Create/update |
| `docs/rewrite/PHASE0-NOTES.md` | Create (spike/decision) |
| `apps/ditto/ui/locales/**` (T1 creates the skeleton; T5 aggregates) | Create skeleton |

T1 must also:
- move the old C++ tree to `legacy/` with `git mv` and preserve history;
- keep `LICENSE` untouched;
- prove the workspace builds, the tray/hotkey/IPC/popup placeholder works, and
  the Tauri idle/WebView2 spike is measured;
- create `rewrite/tauri` and report the SHA.

### T2 — clipboard + storage + settings

| Path | Allowed |
|---|---|
| `crates/ditto-clipboard/**` | Full |
| `crates/ditto-storage/**` | Full |
| `crates/ditto-model/**` | Type additions only; do not restructure other stages' types |
| `crates/ditto-ipc/**` | Message/codec extensions only; keep v1 compatibility |
| Colocated unit/integration tests in the owned crates | Full |
| `docs/rewrite/DATA-MODEL.md` | Only if the coordinator approves a schema change; otherwise propose via message |

T2 must not edit `tests/fixtures/**` (T6 owns it); create test data inside its
crate's tests or coordinate with T6/T7 for a shared fixture.

### T3 — egui popup

| Path | Allowed |
|---|---|
| `apps/ditto-popup/**` | Full |
| `crates/ditto-popup-ui/**` (if created) | Full |
| Colocated tests | Full |

T3 consumes:
- `design/tokens.json` generated outputs (read-only);
- `crates/ditto-ipc` types (read-only unless coordinated);
- locale keys (request through coordinator/T5);
- the mock provider until T2 lands.

T3 must not edit `crates/ditto-win` (T1/T4), `crates/ditto-model` (T2), or
`apps/ditto/**` (T1/T5).

### T4 — paste/UAC/drag

| Path | Allowed |
|---|---|
| `crates/ditto-paste/**` | Full |
| `crates/ditto-uac-helper/**` | Full |
| `apps/ditto-uac-helper/**` | Full |
| `crates/ditto-win/**` | Only after coordinator approval; prefer a narrow PR |
| Colocated tests | Full |

T4 must not edit `crates/ditto-clipboard`/`ditto-storage`; if it needs a storage
API, request T2.

### T5 — Tauri settings

| Path | Allowed |
|---|---|
| `apps/ditto/ui/**` | Full (React 19 + TS + Vite + Fluent UI v9) |
| `apps/ditto/src-tauri/src/commands/**` | Full (new command modules) |
| `apps/ditto/src-tauri/tauri.conf.json` | Update window/security config; keep T1's skeleton intent |
| `apps/ditto/src-tauri/capabilities/**` | Add new capability files |
| Locale keys for settings | Add through the shared catalog; coordinate with T3 |
| Colocated tests | Full |

T5 must not:
- edit `apps/ditto/src-tauri/src/lib.rs` registration (T1 owns it; ask for a
  one-line registration or coordinator authorization);
- edit `design/tokens.json` (T1 owns it; consume generated outputs);
- edit `crates/ditto-*` domain crates (request the API);
- edit the popup.

### T6 — migration/import/export

| Path | Allowed |
|---|---|
| `crates/ditto-migration/**` | Full |
| `tools/ditto-migrate/**` | Full |
| `tests/fixtures/**` | Full (synthetic fixtures only) |
| `docs/rewrite/MIGRATION-REPORT.md` | Create/update |
| Colocated tests | Full |

T6 must not:
- modify the legacy DB or the user's real data;
- edit `crates/ditto-storage` schema directly; propose changes to T2/coordinator;
- edit `docs/rewrite/DATA-MODEL.md` without approval (the mapping is frozen).

### T7 — integration/QA/packaging

| Path | Allowed |
|---|---|
| Any file | **Only integration fixes**, recorded in `QA-REPORT.md` |
| `tools/qa/**` | Full |
| `docs/rewrite/QA-REPORT.md` | Create/update |
| `docs/rewrite/REVIEW-GUIDE.md` | Create/update |
| `artifacts/rewrite/after/**` | Create/update (screenshots, perf CSV, logs) |
| Build/packaging config | Fix integration issues; record the change and notify the owner |

T7 must not:
- silently rewrite a stage's feature; send it back to the owner;
- change the v1 cut line without the coordinator;
- edit `docs/rewrite/ARCHITECTURE.md`/`DESIGN-SYSTEM.md`/`DATA-MODEL.md` without
  a recorded decision;
- merge `master` or push.

### T8 — independent review

| Path | Allowed |
|---|---|
| `docs/rewrite/CODE-REVIEW.md` | Create/update |
| Everything else | **Read-only** |

T8 must build/test independently, inspect the branch, and report
Blocker/Major/Minor/Nit with file:line and reproduction. It must not fix code;
fixes go back to the owner (T9 if needed).

---

## 4. Shared files and cross-ownership procedure

| Shared file/path | Owner | Consumers | Change procedure |
|---|---|---|---|
| Root `Cargo.toml` / `Cargo.lock` | T1 | All | Ask T1/coordinator; T1 adds members/deps |
| `crates/ditto-model` types | T2 (T1 skeleton) | T3/T4/T5/T6 | Propose the type; T2 or T1 adds it; do not duplicate |
| `crates/ditto-ipc` messages | T2 (T1 skeleton) | T3/T4/T5/T6 | Additive change; keep protocol v1; coordinate |
| `crates/ditto-win` | T1 | T4 | T4 asks for a narrow helper; T1/coordinator grants |
| `apps/ditto/src-tauri/src/lib.rs` | T1 | T5 | T5 asks for command registration; do not edit directly unless granted |
| `tauri.conf.json` / capabilities | T1 initial, T5 updates | T7 | T5 owns the settings window/security config; T7 verifies |
| `design/tokens.json` + generated | T1 | T3/T5 | T3/T5 propose token needs; T1 edits/regenerates |
| Locale catalogs (`apps/ditto/ui/locales/**`) | T5 aggregates; T1 creates the skeleton | T3/T5 | T3 requests keys; do not create a second store |
| `tests/fixtures/**` | T6 | T2/T7 | T2 requests fixtures via coordinator; T6 adds |
| `tools/qa/**` | T7 | All (read) | T7 owns |
| `legacy/**` | T1 move; read-only thereafter | All (read) | No edits |
| `LICENSE` | User/base | All | Never edit |
| `docs/ui/**` | Prior work | All (read) | Never edit; reference only |
| `docs/rewrite/*.md` | T0 baseline | All (read) | Changes require coordinator; T6/T7/T8 add their own report files |

**Cross-ownership request format** (to the coordinator):

```
Need: <file/path>
Stage: <T#>
Change: <one sentence>
Why: <blocked feature/test>
Size: <lines/scope>
Fallback if denied: <alternative>
```

The coordinator either grants a one-time edit, reassigns the file, or asks the
owner to make the change. A grant is scoped to that change; it does not transfer
ownership.

---

## 5. Prohibited modifications (all stages)

- The user's `master` worktree, any file outside the task's Orca worktree.
- `LICENSE` (GPLv3) — no relicense, no header removal.
- `legacy/**` after T1's move — no edits, builds, or "small fixes".
- `docs/ui/**` — prior approved work; reference only.
- `src/**`, `CP_Main*`, `Shared/**`, `DittoSetup/**`, `Debug/**`, `res/**`,
  solution/project files — only T1's move to `legacy/`; no content edits.
- Real `%APPDATA%\Ditto` / `%LOCALAPPDATA%\Ditto` / clipboard history during
  development or QA.
- Defender/security settings, firewall exclusions, or system policy changes.
- New framework dependencies outside the confirmed stack.
- Network listeners, telemetry, auto-update, friends/P2P, add-in/script
  loaders — out of v1.
- Push, force-push, merge `master`, rebase shared branches, or rewrite history.
- Reading or logging clip content/query text/window titles in logs or test
  artifacts.

---

## 6. Cross-ownership risks

| Risk | Where | Mitigation |
|---|---|---|
| Root manifest conflicts | `Cargo.toml`/`Cargo.lock` | T1 owns; downstream requests members/deps; T7 resolves integration |
| IPC type drift | `crates/ditto-ipc` | Additive-only rule; T2 owns codec; consumers use types; T7 checks round-trip |
| Duplicate model types | `crates/ditto-model` | One owner (T2) for types; no shadows in apps |
| Win32 helper edits | `crates/ditto-win` | T4 asks T1; narrow helpers; review |
| `lib.rs` registration | `apps/ditto/src-tauri/src/lib.rs` | T1 registers; T5 adds modules under `commands/**`; coordinator grants one-line edits |
| Locale split | popup vs settings | Single shared catalog; T5 aggregates; T3 requests keys |
| Token drift | `design/tokens.json` vs hardcoded values | T1 generator + CI drift check; T3/T5 consume generated |
| Tauri config/security | `tauri.conf.json`/capabilities | T5 owns settings window/security; T7 verifies; T8 reviews |
| Fixture collisions | `tests/fixtures/**` | T6 owns; T2 requests via coordinator; deterministic seeds |
| Integration "fixes" | T7 across files | Record every cross-file fix in `QA-REPORT.md`; semantic conflicts go back to the owner |
| Docs divergence | `docs/rewrite/*.md` | T0 frozen; changes require coordinator; reports are separate files |
| Legacy move | `legacy/**` | T1 only; everyone treats it as read-only; no build |
| Package lock churn | `package-lock.json` | T5 owns frontend deps; T1 owns Rust; T7 integrates |

---

## 7. Merge and conflict policy

1. Each stage works on its own Orca worktree and branch.
2. T7 merges `--no-ff` in dependency order: T2 → T3 → T4 → T5 → T6 (or a
   coordinator-approved order that respects the DAG).
3. A textual conflict is resolved by the file owner or T7 with the owner's
   confirmation. A semantic conflict is sent back to the owner.
4. No stage rebases or merges another stage's branch without the coordinator.
5. T8 reviews the integrated branch read-only; fixes go back to the owner
   (T9 if needed).
6. The final branch is `rewrite/tauri` (or the coordinator's integration
   branch). It is **not** pushed and **not** merged to `master` by the agents.

---

## 8. Ownership acceptance checklist

For every stage handoff:

- [ ] `git status` is clean (or the report lists uncommitted files).
- [ ] HEAD SHA and branch are reported.
- [ ] Only owned paths changed; cross-owned changes are listed with the
      coordinator's grant.
- [ ] `legacy/` and `LICENSE` are unchanged after T1's move.
- [ ] No real user data in tests/artifacts.
- [ ] Build/test evidence is attached.
- [ ] New files are registered where required (workspace members, `lib.rs`,
      capabilities, locale catalog).
- [ ] The next stage's base branch/SHA is included.

---

## 9. Evidence

This matrix is derived from the coordinator's rewrite prompt §10 and task
specs. The ownership boundaries were checked against the current tree layout
at `master` @ `4719eea` (`src/`, `Shared/`, `DittoSetup/`, `Debug/`,
`Addins/`, solution files) and the target layout in `ARCHITECTURE.md` §10.
