# T4 report - paste, target focus, UAC, and drag/drop

## STATUS

Implementation and automated verification are complete. The worktree is
**uncommitted** because repository policy requires the user/coordinator to run
all git operations.

The T4 implementation covers:

- foreground target capture, focus/caret capture, minimized-window restore,
  own-window rejection, secure-desktop rejection, and integrity/elevation
  detection;
- full-format clipboard materialization, plain-text paste, format aliases and
  fallback, PNG-to-DIB compatibility fallback, CF_HDROP path validation,
  self-write suppression, and sequential multi-paste;
- additive `Paste` IPC fields and a `PasteResult` response/event;
- a one-shot UAC helper protocol where the helper receives no clip payload and
  only sends `Ctrl+V`;
- OLE drag-out using `OleGetClipboard` + `DoDragDrop`, and bounded CF_HDROP
  receive parsing;
- synthetic unit coverage for payloads, fallback, batch order, target classes,
  error mapping, token expiry/replay/binding, and helper argument hardening.

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-paste`

## BRANCH

`taohy/rewrite-paste` (read from worktree metadata; no git command was run).

## HEAD

Base `taohy/rewrite-core` @ `7069fcc8ef14fa16f3d4f779886c1de9c9ca0009`.
T4 changes are uncommitted on top of that base.

## FILES

- `Cargo.lock`
- `crates/ditto-ipc/src/lib.rs`
- `crates/ditto-paste/Cargo.toml`
- `crates/ditto-paste/src/lib.rs`
- `crates/ditto-paste/src/error.rs`
- `crates/ditto-paste/src/model.rs`
- `crates/ditto-paste/src/payload.rs`
- `crates/ditto-paste/src/clipboard.rs`
- `crates/ditto-paste/src/target.rs`
- `crates/ditto-paste/src/input.rs`
- `crates/ditto-paste/src/engine.rs`
- `crates/ditto-paste/src/elevation.rs`
- `crates/ditto-paste/src/drag.rs`
- `crates/ditto-uac-helper/Cargo.toml`
- `crates/ditto-uac-helper/src/lib.rs`
- `crates/ditto-uac-helper/src/error.rs`
- `crates/ditto-uac-helper/src/protocol.rs`
- `crates/ditto-uac-helper/src/token.rs`
- `crates/ditto-uac-helper/src/win32.rs`
- `crates/ditto-uac-helper/src/helper.rs`
- `apps/ditto-uac-helper/src/main.rs`
- `apps/ditto/src-tauri/Cargo.toml`
- `apps/ditto/src-tauri/src/core_service.rs`
- `docs/rewrite/T4-REPORT.md`

`crates/ditto-win` was not changed. No storage, popup, UI, legacy C++, or
`LICENSE` file was changed.

## BUILD

Passed:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh -NoProfile -File scripts/prepare-sidecars.ps1 -Profile debug
npm ci
npm run build
```

The Tauri crate requires `apps/ditto/ui/dist` and the staged sidecars. Both are
ignored build inputs; they were generated locally only to run the full
workspace checks.

## API

### Paste request

`Request::Paste` is an additive extension of the existing T2 message:

```json
{
  "type": "Paste",
  "clip_id": 7,
  "plain_text": false,
  "formats": ["html", "text"],
  "clip_ids": [7, 8, 9]
}
```

- `clip_id` and `plain_text` remain compatible with existing clients.
- `formats` defaults to an empty array and `clip_ids` defaults to an empty
  array.
- If `clip_ids` is empty, `clip_id` is used.
- IDs must be positive; duplicates and invalid IDs are removed while preserving
  order.
- When `plain_text` is true it takes precedence over `formats` and produces
  one NUL-terminated `CF_UNICODETEXT` payload.

Supported format selectors:

| Selector | Stored formats |
|---|---|
| `text`, `plain`, `plain_text` | `CF_UNICODETEXT`, `CF_TEXT`, `CF_OEMTEXT` |
| `html` | `HTML Format` |
| `rtf`, `rich_text` | `Rich Text Format` |
| `image` | `PNG`, `CF_DIB`, `CF_DIBV5` |
| `files`, `file` | `CF_HDROP` |
| exact name | Exact stored format name |

If a requested format is absent, the first primary or first available stored
format is used as the fallback. Missing text can also be synthesized from
`text_content`. An empty/unreadable clip returns a stable
`Unsupported`/`NotFound` error.

For image clips stored only as PNG, T4 derives a bounded 32-bit BI_RGB
`CF_DIB` fallback (maximum 256 MiB of DIB pixels) so applications that do not
accept registered PNG clipboard data can still paste the image.

### Paste result

The new response and event carry metadata only, never clip content:

```json
{
  "type": "PasteResult",
  "outcome": {
    "ok": false,
    "mode": "multi",
    "elevated": false,
    "target_pid": 1234,
    "pasted_clip_ids": [7, 8],
    "failed_clip_id": 9,
    "error_code": "clipboard",
    "message": "clipboard is busy"
  }
}
```

Sequential multi-paste stops at the first failure and returns the successful
prefix. Successful clip IDs are marked with `Storage::mark_pasted` and emit the
existing `ClipUpdated` event.

Current error mapping:

| Paste error | DTP1 error code |
|---|---|
| missing clip | `NotFound` |
| clipboard unavailable/busy | `Busy` |
| clipboard write/read, target unavailable, invalid files | `Clipboard` |
| UAC launch/token/helper failure | `Elevation` |
| user cancellation | `Cancelled` |
| no matching format | `Conflict` |
| unexpected internal error | `Internal` |

## Target And Focus

`WindowsTargetBackend` records:

- `GetForegroundWindow`;
- `GetWindowThreadProcessId`;
- `GetGUIThreadInfo.hwndFocus` with the foreground window as fallback;
- the class name, process ID, thread ID, and token integrity level.

Rejected targets:

- no foreground window;
- the current core process;
- `Ditto.exe`, `DittoPopup.exe`, `DittoSettings.exe`, or
  `DittoUacHelper.exe`;
- shell/desktop surfaces such as `Progman`, `WorkerW`, `Shell_TrayWnd`,
  `TrayNotifyWnd`, and `NotifyIconOverflowWindow`;
- a non-`Default` thread desktop or a `Winlogon` target.

Before input, the target is restored if minimized, attached to the current
thread for `BringWindowToTop`/`SetForegroundWindow`/`SetFocus`, and verified as
the foreground window. The configured focus delay is clamped to `0..=300 ms`.
Paste mode:

- core writes the selected clipboard formats;
- normal/medium targets are activated directly and receive `Ctrl+V` through
  `SendInput`;
- high/system targets use the elevation flow when the core is not elevated.

Multi-paste is sequential. The interval is read from optional
`clipboard.multi_paste_interval_ms` (default `180 ms`, clamped to
`180..=2000 ms`) and processing stops at the first failure.

## UAC Flow And Threat Model

### Flow

1. A user-triggered `Paste` request reaches `core_service`.
2. The core materializes and writes the clipboard payload, and calls T2
   `WriteSuppression::remember` with the exact content hash before the write.
3. The core detects a high-integrity target while running at lower integrity.
4. The core creates a one-shot named pipe and a 32-byte CSPRNG token. The token
   TTL is 10 seconds and is bound to core PID, user SID, target PID, target
   window handle, clip ID, and paste mode.
5. The core launches `DittoUacHelper.exe` with `ShellExecuteExW` + `runas`.
   Command-line arguments contain only fixed integers/enums:
   `--core-pid`, `--target-pid`, `--target-window`, `--clip-id`, `--mode`,
   `--request-id`, and `--focus-delay-ms`.
6. The named pipe is restricted to SYSTEM and the current user SID. The core
   accepts only the exact helper process PID returned by `ShellExecuteExW` and
   verifies that process token's user SID.
7. The helper verifies the pipe server PID/user SID, sends a fixed `Hello`,
   validates the authorization binding and expiry, verifies that the target
   window/process still match, restores foreground focus, and sends only
   `Ctrl+V` through `SendInput`.
8. The helper sends `Done` and exits. Replay, expiry, wrong request, wrong
   token, wrong binding, wrong client process, and wrong SID fail closed.

The token is never placed on the command line and the helper never receives
clip text, HTML, RTF, image bytes, file paths, a command, an executable path, or
a pipe name. The core does not elevate.

### Integrity detection

The target token is queried with `OpenProcessToken` +
`GetTokenInformation(TokenElevation/TokenIntegrityLevel)`. The integrity RID
is mapped to Untrusted/Low/Medium/High/System. A high/system target causes the
UAC path when the core is not elevated.

### Clipboard isolation decision

The implementation follows the coordinator decision X-03: the core writes the
payload and the helper only sends `Ctrl+V`. No elevated payload-write exception
was added because a physical elevated-target test was not possible in this
environment. If T7 proves that the elevated target cannot read the
medium-integrity clipboard, that exception must be separately reviewed and
documented; it is not present in this code.

### Failure and recovery

- UAC cancellation maps to `Cancelled`.
- Helper launch/token/validation/timeout failures map to `Elevation`.
- No automatic elevation retry loop exists.
- A clipboard snapshot is taken before paste when the existing
  `clipboard.restore_clipboard` setting is true. On failure the snapshot is
  restored and the result reports `restored_clipboard`.
- With restore disabled (the default), the selected payload remains on the
  clipboard so the user can paste manually after resolving the failure.
- Logs contain IDs, mode, result codes, and error text; no clip content or
  window title is logged.

### Residual UAC risks

- Same-user processes remain inside the same trust boundary and can attempt
  UI automation or denial of service; the helper protects privilege
  escalation, not same-user isolation.
- Focus stealing after the UAC secure desktop returns is OS-policy dependent.
- The helper protocol still needs T8 independent review.

## Drag And Drop

### Drag-out

`ditto_paste::drag_out`:

- records the exact outgoing content hash with the T2 suppression API, writes
  the selected multi-format payload to the OLE clipboard;
- obtains `IDataObject` with `OleGetClipboard`;
- creates an `IDropSource` that returns copy-only feedback;
- calls `DoDragDrop(DROPEFFECT_COPY)`.

This supports the same payload families as paste: Unicode text, HTML, RTF,
PNG/DIB images, and CF_HDROP files. The current popup has no T4 IPC message for
drag-out, so T3/T7 must invoke this from the owning UI/integration layer.

### File drop

- `build_file_drop` creates a wide `DROPFILES` + `CF_HDROP` payload.
- Paths are checked for existence before write.
- At most 100,000 paths and 16 MiB of drop structure are accepted.
- File byte totals are checked with overflow protection.
- `parse_file_drop` accepts only existing paths and never executes/opens them.
- Stored `CF_HDROP` paste is validated again before it reaches the clipboard.

### Receive/import

The native parser rejects malformed CF_HDROP data and missing paths. No shell,
file-open, script, or command execution path exists. T5 can use
`parse_file_drop` for a manager/settings import surface.

## Test Matrix

### Automated

| Area | Test |
|---|---|
| IPC compatibility | legacy Paste JSON without new fields decodes |
| IPC result | PasteResult response round-trips with metadata only |
| IPC engine | synthetic clip reaches Paste and returns NotReady when engine unwired |
| Plain text | Unicode text uses content and NUL terminator |
| Format fallback | missing text synthesizes text; unknown selector falls back to primary |
| Format aliases | text/HTML/RTF/image/files mapping |
| PNG compatibility | PNG-only clip gains a `CF_DIB` fallback |
| Blob safety | stored blob path traversal rejected |
| Batch order | three clips paste in order with three activations/input events |
| Elevation routing | high-integrity target uses elevation backend, not direct SendInput |
| Target selection | shell surfaces and own process images rejected |
| Integrity mapping | low/medium/high/system RID thresholds |
| File drop | synthetic path round-trip; missing path rejected |
| Drop receive | malformed/absent path rejected |
| UAC token | valid one-shot, replay, expiry, wrong token/binding/request |
| Helper args | fixed argument set; pipe/path-like extra argument rejected |
| Helper hello | every bound field must match |
| Error mapping | Busy/Elevation/InvalidFiles map to stable IPC codes |

Workspace test result: all tests passed, including existing T2 storage,
clipboard, IPC, pipe, and 10k benchmark tests.

### Live/manual status

| Scenario | Status |
|---|---|
| Normal Notepad paste | Not run (would attach the live T2 clipboard monitor to the user's real clipboard session) |
| Admin Notepad + UAC prompt | Not run (requires interactive UAC approval) |
| Explorer CF_HDROP copy/paste | Not run |
| Drag-out to Explorer | Not run |
| Multi-select sequential paste | Unit-tested; live UI run not available |
| Closed target / occupied clipboard | Unit/error paths covered; live race not run |
| UAC cancellation | Code path covered by error mapping; prompt not exercised |
| Secure desktop | Rejection path implemented; physical secure desktop not exercised |
| Office/browser HTML/RTF | Not run |
| Explorer restart | Not run |

No test or QA command read real `%APPDATA%\Ditto`,
`%LOCALAPPDATA%\Ditto`, a real Ditto database, or clipboard history.

## EVIDENCE

### Commands

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh -NoProfile -File scripts/prepare-sidecars.ps1 -Profile debug
npm ci
npm run build
```

### Helper argument hardening

```text
target\debug\DittoUacHelper.exe ... --pipe-name evil
helper_exit=2
```

The additional pipe/path-like argument was rejected before any pipe or input
operation.

### Self-write suppression

The paste path builds the same T2 `CapturedClip` model for the exact outgoing
formats, calls `WriteSuppression::remember(content_hash)` before
`SetClipboardData`, and relies on T2's existing suppression test. Multi-paste
uses at least the 180 ms capture debounce interval before the next write.

## UNVERIFIED

- Real ordinary-target paste to Notepad, browser, Office, and Explorer.
- Real elevated-target paste through the UAC prompt and post-UAC focus return.
- Whether a medium-integrity clipboard write is always readable by the elevated
  target in the QA environment.
- Physical secure-desktop behavior and Winlogon edge cases.
- Real drag-out `DoDragDrop` into Explorer and Explorer restart recovery.
- Delayed-render/OLE ownership behavior across Office and browser targets.
- CF_HDROP directories/large path sets and network-share availability.
- High-DPI, multi-monitor, and caret-only focus restoration.
- Tauri/NSIS placement of `DittoUacHelper.exe` beside the core in every package
  profile.
- Same-user denial-of-service, pipe squatting timing, and independent UAC
  security review.

## RISKS

- `SetForegroundWindow` can still be denied by Windows foreground-lock policy;
  the code fails with a target error rather than retrying or changing system
  settings.
- The UAC helper restores the bound foreground window as part of delivering
  `Ctrl+V`; this is a narrow window operation but still needs T8 review.
- The named-pipe helper protocol uses polling reads and fixed 10-second
  deadlines. A slow UAC approval fails closed and requires a new user action.
- OLE drag-out first changes the clipboard because it uses
  `OleGetClipboard`; the API requires the T2 `WriteSuppression` object so the
  core integration cannot omit self-write suppression.
- The popup/integration layer still needs to send `formats`/`clip_ids` and
  expose drag-out. T4 provides the core APIs but did not modify T3/T5-owned UI.
- Unsigned development binaries may still trigger SmartScreen/Defender
  warnings. No security setting was changed.
- No git commit exists; the coordinator must inspect and commit the listed
  files.

## NEXT

1. T7: run live Notepad, Explorer, Office/browser, multi-select, closed-target,
   clipboard-race, UAC-cancel, admin-target, and drag-out tests using synthetic
   fixtures.
2. T3/T5: send the additive `formats`/`clip_ids` fields and render
   `PasteResult`/`Event::PasteResult`; invoke drag-out with the T4 suppression
   policy.
3. T7/T8: verify sidecar path resolution, UAC helper token review, secure
   desktop behavior, and installer packaging.
4. Coordinator: inspect and commit the uncommitted T4 files, including this
   report and `Cargo.lock`.
