# PHASE0-NOTES - T1 foundation and spike record

- Date: 2026-11-12 (Asia/Shanghai)
- Worktree: `D:\orca\workspaces\Ditto\rewrite-foundation`
- Task: T1 foundation
- Git state: **uncommitted**. Repository policy prohibits agent-run git
  operations, so the branch was not renamed, no commit was created, and HEAD
  was not queried. The coordinator must run `git branch -m rewrite/tauri`,
  review, and commit these files.
- Status: implementation and local evidence complete; git branch/commit and
  interactive-desktop confirmation remain for the coordinator/user.

---

## 1. Toolchain

| Tool | Version / result |
|---|---|
| rustup | `1.29.3 (edbc831ce 2026-11-02)` |
| active toolchain | `stable-x86_64-pc-windows-msvc` |
| rustc | `1.99.1 (58cf8f9c3 2026-10-13)` |
| LLVM | `23.1.1` |
| cargo | `1.99.1 (5f94df478 2026-08-27)` |
| rustfmt | installed for stable MSVC |
| clippy | installed for stable MSVC |
| Node | `v24.13.0` |
| npm | `11.6.2` |
| Tauri CLI | `tauri-cli 2.12.4` through `npx tauri` fallback |
| PowerShell | `7.6.6` |
| Visual Studio Build Tools | `18.11.3`, MSVC `14.51.36231` |
| Windows SDK | `10.0.26100.0` |
| WebView2 Evergreen | `157.0.4341.46` |
| OS | Windows 11 10.0.26200 x64 |
| CPU / RAM | Intel i7-13700HX, 24 logical processors, 31.7 GiB |

Rust was not initially present. User-level Winget installation
`Rustlang.Rustup` succeeded; no administrator install was used. `rustfmt` and
`clippy` were added to the stable MSVC toolchain.

`cargo install tauri-cli --version "^2" --locked` resolved Tauri CLI 2.12.4 but
did not complete in this environment after the package download/index step.
The task-approved fallback was used in `apps/ditto`:

```powershell
npm install -D "@tauri-apps/cli@^2"
npx tauri --version
```

The visual studio toolchain is VS 2026 Build Tools with MSVC v145 and Windows
SDK 26100.

---

## 2. Edition and layout

- Rust edition: **2024**. The installed stable compiler supports it.
- `rust-toolchain.toml` pins stable MSVC x64, `rustfmt`, `clippy`, and the
  `x86_64-pc-windows-msvc` target.
- Workspace members:
  - `crates/ditto-model`
  - `crates/ditto-ipc`
  - `crates/ditto-win`
  - `crates/ditto-clipboard`
  - `crates/ditto-storage`
  - `crates/ditto-migration`
  - `crates/ditto-paste`
  - `crates/ditto-uac-helper`
  - `crates/ditto-ui-tokens`
  - `apps/ditto-popup`
  - `apps/ditto-uac-helper`
  - `apps/ditto/src-tauri`
- `apps/ditto/ui` is the Vite + TypeScript settings placeholder. React and
  Fluent UI remain deferred to T5.
- `design/tokens.json` generates Rust, CSS, and TypeScript outputs through
  `scripts/generate-tokens.mjs`.

---

## 3. IPC choice

- Transport: current-user named pipe
  `\\.\pipe\ditto-<current-user-sid>`.
- ACL: `D:P(A;;GA;;;SY)(A;;GA;;;<sid>)(A;;GR;;;BA)`.
- Pipe flags: duplex, byte mode, wait mode, 32 instances,
  `PIPE_REJECT_REMOTE_CLIENTS`, and `FILE_FLAG_FIRST_PIPE_INSTANCE` for the
  initial instance.
- Framing: 24-byte `DTP1` header and a 16 MiB payload cap.
- Codec: UTF-8 JSON. It is debuggable and matches the T0 decision; postcard was
  not added.
- Messages implemented end to end: `Hello`, `ShowPopup`, `HidePopup`,
  `Shutdown` event. Other catalogue messages return `NotImplemented`.
- `Hello` and popup event round-trips are covered by
  `hello_and_popup_events_traverse_named_pipe`.
- The pipe handler uses `PeekNamedPipe` polling before read. A blocking
  synchronous read prevented concurrent server-side event writes; polling is a
  T1 implementation detail and should be replaced by an overlapped/async actor
  in T2 if throughput requires it.

---

## 4. Tauri and WebView2 lifecycle

`tauri.conf.json` uses:

- `app.windows: []`;
- identifier `com.ditto.clipboard`;
- NSIS target;
- sidecars `DittoPopup.exe` and `DittoUacHelper.exe`;
- icon copied from the legacy `res/Ditto2.ico`.

Tauri plugins wired in the core: global-shortcut, single-instance, autostart,
log, and tray. The tray menu contains Show, Settings, and Exit. The development
hotkey is chosen by a preflight probe: `Ctrl+Alt+D` first, then
`Ctrl+Alt+Shift+D`. In this environment `Ctrl+Alt+D` was free and was registered;
an external probe then received Win32 error 1409, proving the app owned it.

Tauri `externalBin` requires staged files with the target triple in the filename:
`binaries/DittoPopup-x86_64-pc-windows-msvc.exe` and
`binaries/DittoUacHelper-x86_64-pc-windows-msvc.exe`. The staging script creates
these from Cargo output; generated `.exe` files are ignored by git.

The settings window is created on demand by `WebviewWindowBuilder`. Closing the
last settings window is explicitly prevented from terminating the tray host;
the core remained alive after close. A `--settings` second-instance path exists
for reproducible lifecycle tests.

Release settings lifecycle:

| Check | Result |
|---|---|
| Second instance exit | 0 |
| Settings windows | `Ditto Settings` created |
| New `msedgewebview2.exe` while settings open | 6 |
| WebView2 processes 6 s after close | 0 |
| Core alive after settings close | yes |
| WebView2 at idle before settings opened | 0 |

The existing machine had 20 unrelated WebView2 processes before the test.
Comparisons use before/after PID sets, not machine-wide counts.

---

## 5. Performance method and raw data

### 5.1 Release idle memory

Command: start `target/release/Ditto.exe --background` with a synthetic
`DITTO_DATA_DIR`, wait 10 s, sample three times at 5 s intervals, settings never
opened.

Process tree:

```text
Ditto.exe      PID 19296  parent 36264
DittoPopup.exe PID 38592  parent 19296
```

| Sample | Core `PrivateMemorySize64` | Core `WorkingSet64` | Popup private | Popup working set | Total private | Total working set |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 4,050,944 | 19,542,016 | 87,339,008 | 74,674,176 | 91,389,952 | 94,216,192 |
| 2 | 3,948,544 | 19,480,576 | 87,339,008 | 74,674,176 | 91,287,552 | 94,154,752 |
| 3 | 3,948,544 | 19,480,576 | 87,191,552 | 74,600,448 | 91,140,096 | 94,081,024 |

- Median total `PrivateMemorySize64`: **91,287,552 bytes = 87.06 MiB**.
- Median total `WorkingSet64`: **94,154,752 bytes = 89.79 MiB**.
- New WebView2 PIDs: **none**.
- Raw CSV:
  `target/final-ui2-20261112-204325/idle-memory.csv` (build artifact, not
  committed).

The debug-build fresh idle run measured 90.69 MiB median total private and
98.17 MiB median total working set. Release numbers are the decision input; the
final DPI-corrected release run is lower than the pre-fix build.

These are the two PowerShell metrics requested by T1. They are not the same as
the Task Manager "private working set" counter; `GetProcessMemoryInfo`/PDH
cross-check remains useful at T7.

### 5.2 Plan A / Plan B decision

**Plan A selected.** Release Plan A is below 100 MiB by both requested totals:
87.06 MiB private and 89.79 MiB working set, with no WebView2 process at idle.
Plan B was not needed and was not run.

The margin on `WorkingSet64` is about 10.2 MiB. T3 must not add resident
WebView2, large caches, or eager thumbnail/font work. T7 must re-measure on the
integrated branch.

### 5.3 Popup first-frame smoke

The interactive desktop was locked (`LockApp.exe` owned the foreground secure
desktop), so physical key injection could not reach the desktop hotkey. The
harness posted `WM_HOTKEY` ID `589846` to the app's `global_hotkey_app` window,
the exact message produced by the registered hotkey, then verified the Tauri
handler broadcast to one IPC client.

The popup records:

- `t2`: `ShowPopup` received by the popup IPC thread;
- `t3`: end of the first subsequent egui `update` that renders the popup.

This is an update-frame approximation, not a swap-chain/PresentMon measurement.

Release 10-sample raw values, milliseconds:

```text
2.719, 2.276, 1.436, 1.121, 1.415, 2.681, 1.641, 1.439, 1.856, 1.050
```

- min: 1.050 ms
- median: 1.641 ms
- p95 (nearest rank): **2.719 ms**
- max: 2.719 ms
- popup visibility check: 10/10 visible
- raw CSV: `target/final-ui2-20261112-204325/latency.csv`
- DPI-aware `PrintWindow` evidence:
  `target/final-ui2-20261112-204325/popup-printwindow-dpi.png`

Debug final smoke after all lifecycle fixes measured p95 **4.332 ms** over 10
samples.

Because the desktop was locked, user-visible presentation and the full
hardware-key-to-present interval remain UNVERIFIED. `PrintWindow` rendered the
popup successfully, but ordinary desktop capture showed the LockApp secure
desktop; the actual screen is not valid visual evidence in this session. T3/T7
must repeat the full real-key test from an unlocked desktop.

---

## 6. FTS5 measurement

The bundled `rusqlite 0.37.0` test
`bundled_sqlite_has_fts5_trigram` passed:

- `PRAGMA compile_options` contains `ENABLE_FTS5`;
- `CREATE VIRTUAL TABLE probe USING fts5(text, tokenize = 'trigram')`
  succeeds.

T2 should use FTS5 `trigram`, with the approved 1-2 character `LIKE` fallback.
No fallback to `unicode61` is needed for the bundled build measured here.

---

## 7. Build, test, and packaging

Passed locally:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo build --release -p ditto-tauri -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh scripts/prepare-sidecars.ps1 -Profile release
cd apps/ditto
npx tauri build
```

NSIS artifact:

```text
target/release/bundle/nsis/Ditto_0.1.0_x64-setup.exe
```

Artifact hashes and signatures:

| Artifact | Bytes | SHA-256 | Signature |
|---|---:|---|---|
| `Ditto.exe` | 7,156,736 | `1436429EF6EBFF18CEE19741835FD74AE894C8DCBC10BDE08B6233FB13F58F37` | NotSigned |
| `DittoPopup.exe` | 4,320,768 | `492BC05928015A9EBDA54A1F8DC1DB61934A23628119572BA2BEAECE08081ED0` | NotSigned |
| `DittoUacHelper.exe` | 131,584 | `56BF58043B7B1FD6A3FD138FFEE82D6A92C33EBEFDD5BB9622EFBD4506E28981` | NotSigned |
| NSIS setup | 3,108,334 | `AC2A330D34FF65CA723BF5FB83822E1E8DF41ADC46139CB8A5F7EC7DCB183F3F` | NotSigned |

GitHub Actions equivalent is defined in `.github/workflows/rewrite-ci.yml`.
Remote CI was not run from this worker; the local Windows commands above are the
evidence available in T1.

---

## 8. Legacy move verification

All old C++/build/UI assets were moved to `legacy/` without content edits.
`docs/rewrite/` was preserved at the root and `LICENSE` was not moved.

Verification:

- Source vs legacy file count matched for the moved trees, including
  `src`, `res`, `Addins`, `DittoSetup`, `Shared`, `EncryptDecrypt`, `focusdll`,
  `FocusHighlight`, `ICU_Loader`, `U3Stop`, `Debug`, `packages`, `x64`,
  `Release64`, `docs/ui`, `artifacts/ui`, and `tools/ui-qa`.
- SHA-256 comparison covered **1,809 files** with **0 missing and 0 content
  mismatches**.
- `LICENSE` SHA-256 matches master:
  `230184F60BAE2FEAF244F10A8BAC053C8FF33A183BCC365B4D8B876D2B7F4809`.
- The repository policy prohibited `git mv`, so filesystem moves/copies were
  used; history preservation and the `rewrite/tauri` rename require the
  coordinator's git operation.
- No real `%APPDATA%\Ditto`, `%LOCALAPPDATA%\Ditto`, or clipboard history was
  read.

---

## 9. Defender / unsigned binaries

- Current binaries are unsigned (`NotSigned`).
- The release core, popup, and installer were built and launched without a
  current Defender block.
- `Get-MpThreatDetection` found **no detections since 2026-11-12 19:00**.
- Defender real-time protection remained enabled. No security setting,
  exclusion, firewall rule, or policy was changed.
- Historical Defender records exist for older Ditto executables from earlier
  worktrees (2026-11-10/11), so unsigned distribution risk remains real and
  must be reported in later packaging/signing work.

---

## 10. Unverified / risks

- Branch rename and commit are pending because agents may not run git.
- Full hardware hotkey -> visible popup latency was not measurable while the
  desktop was locked; the measured path starts at popup IPC receipt.
- egui IME, candidate-window placement, DWM rounded corners, multi-monitor
  placement, and high-contrast behavior remain T3 work.
- NSIS install/uninstall behavior was produced but not exercised.
- UAC helper behavior remains placeholder work for T4.
- Clipboard capture, SQLite schema, 10k search/scroll, and migration are not
  implemented in T1.
- The synchronous named-pipe read was replaced by `PeekNamedPipe` polling in
  T1; T2 should move to a bounded async/overlapped actor if needed.
- Release working-set margin is small. Avoid idle WebView2 and eager caches.
- No remote GitHub CI result is available from this worker.

