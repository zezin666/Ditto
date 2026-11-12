# SECURITY — threat model, IPC/UAC hardening, privacy, and supply chain

- Status: **T0 baseline, frozen for T1–T8**
- Date: 2026-11-12
- Scope: local IPC, elevation helper, clipboard data, settings webview,
  migration/import parsers, logging/privacy, packaging, and supply chain.
- Owners: T1 (host/IPC skeleton), T2 (data/clipboard), T4 (paste/UAC), T5
  (settings webview), T6 (migration/import), T7 (verification), T8 (independent
  review).
- Non-goals: network service security (no network in v1), DRM, multi-user
  isolation beyond the Windows user boundary.

---

## 1. Security goals and non-goals

### 1.1 Goals

1. A local user's clipboard history is readable only by that user (and
   SYSTEM/Administrators as required by Windows), and is never sent anywhere.
2. No other local user or remote client can command Ditto through IPC.
3. Elevation is used only for the narrow "paste into an elevated window"
   action, only when the user triggers it, and never as a general command
   executor.
4. The settings webview cannot read/write arbitrary files, spawn processes, or
   load remote content.
5. Legacy add-ins, ChaiScript, theme/language XML, and DLLs are never loaded as
   code.
6. Parsing untrusted clipboard/migration/import data cannot execute code or
   crash the core permanently.
7. No telemetry, analytics, or hidden network calls.
8. Logs, crash dumps, and diagnostics never contain clip content by default.

### 1.2 Non-goals

- Protecting clipboard data from a same-user process that can read the user's
  files or debug the process. The OS user boundary is the boundary.
- Encrypting the database at rest in v1. This is a documented, deliberate
  deferral; the threat model is a local same-user attacker.
- Network/friends security (feature deferred).
- Anti-virus evasion. Unsigned binaries may be flagged; we report it and do not
  change Defender or system security settings.

---

## 2. Assets and trust boundaries

| Asset | Where | Sensitivity |
|---|---|---|
| Clipboard history | `ditto.db`, blob store | High: passwords, tokens, personal text, images |
| Settings | `settings` table, `config.json` | Medium: paths, hotkeys, app filters |
| IPC endpoint secret | `run/endpoint.json` | High for session integrity |
| Elevation token | one-shot pipe/handle | High |
| Logs/crash dumps | `logs/`, `crashes/` | Medium: paths, ids; must not contain content |
| Legacy DB copy | temp migration dir | High: full history |
| Export/backup files | user-chosen path | High: unencrypted by default |

Trust boundaries:

```
same user, medium integrity
  ├── Ditto.exe (core)  ── trusted application code
  ├── DittoPopup.exe    ── trusted client, no privileged operations
  ├── settings webview  ── least-privileged view, allow-listed commands only
  └── other same-user processes ── untrusted; cannot use the pipe without the secret
elevated integrity
  ├── DittoUacHelper.exe ── one-shot, token-gated, SendInput only
  └── elevated target apps ── may receive Ctrl+V, never arbitrary commands
SYSTEM/Administrators ── OS boundary; pipe ACL grants read-only to Administrators
```

The pipe is **not** a privilege boundary against the same user; it is a
correctness and anti-accident boundary. The elevation helper is the only
privilege boundary and is deliberately minimal.

---

## 3. IPC security

### 3.1 Endpoint and ACL

- Pipe name: `\\.\pipe\ditto-<current-user-sid>` (see `ARCHITECTURE.md` §4.1).
- SDDL: `D:P(A;;GA;;;SY)(A;;GA;;;<user_sid>)(A;;GR;;;BA)`.
  - Owner (current user): full control.
  - SYSTEM: full control.
  - Administrators: read-only, so an elevated admin process cannot inject
    commands without the user's secret. (An administrator can still debug the
    process; that is out of scope.)
  - No `Everyone`, `Users`, `Authenticated Users`, or anonymous access.
- `PIPE_REJECT_REMOTE_CLIENTS` and `FILE_FLAG_FIRST_PIPE_INSTANCE`.
- One server instance; max 32 clients; 15 s idle timeout; 5 s control timeout.
- The endpoint file is user-only ACL, atomic write, deleted on clean exit.

### 3.2 Authentication

- Primary: inherited pipe handle passed by the core to the popup at spawn.
- Fallback: endpoint file secret (32 bytes CSPRNG, base64url), constant-time
  compare, per boot.
- After connect, verify `GetNamedPipeClientProcessId` and the image path.
  Allow-list: `Ditto.exe`, `DittoPopup.exe`, `DittoSettings.exe`,
  `DittoUacHelper.exe`, or a path under the install directory.
- Reject and log without payload on failure. Do not reveal whether the name or
  the secret was wrong (single `DITTO_E_AUTH`).
- Optional signature verification of the client image in release builds; if
  signing is unavailable, path allow-list plus user-only ACL is the baseline.
  T8 reviews the residual risk.

### 3.3 Protocol hardening

- Fixed message catalogue only (`ARCHITECTURE.md` §4.5). Unknown types are
  rejected, not dispatched.
- Max frame 16 MiB; reject oversized lengths before allocating. JSON depth and
  array sizes are bounded.
- All requests are correlated by `request_id`; responses are ignored if the id
  is unknown. Duplicate side-effect requests (`Paste`, `Delete`) are
  deduplicated for 5 s by id.
- **No arbitrary commands, shell strings, executable paths, SQL, or URLs cross
  the pipe.** Identifiers are typed integers or fixed enums. File paths are
  accepted only as the result of a native file dialog initiated by the core,
  and are validated as absolute paths for the intended operation.
- The core never trusts a client-supplied foreground window handle for
  privileged actions without re-checking that it is still the foreground
  window and belongs to the expected process.
- Rate-limit `Paste`/`Delete`/`Search` per client; backpressure events rather
  than unbounded queues.
- Log request metadata (id, method, duration, code, peer PID), never payloads.

### 3.4 Squatting and stale endpoints

- The single-instance mutex plus `FILE_FLAG_FIRST_PIPE_INSTANCE` prevents a
  second pipe server.
- If the mutex exists but the pipe is unresponsive, the second process reports
  a stale core; it must not kill it or delete the mutex without an explicit
  `--repair` confirmation.
- If the pipe name is owned by a process that is not Ditto, startup fails with
  an explicit error. Never fall back to an unauthenticated or differently named
  pipe.

---

## 4. UAC helper

### 4.1 Scope

`DittoUacHelper.exe` exists only to deliver `Ctrl+V` to an elevated foreground
window when the core is not elevated. It is not a general helper, service, or
command runner.

### 4.2 Launch and authorization

- The core detects that the foreground target is elevated (`TokenElevation`)
  and the core is not.
- Only a user action (paste/Enter on the popup) can trigger the helper. No
  clipboard event, timer, hotkey-at-login, or background retry may elevate.
- The core launches the helper with `ShellExecuteEx` + `runas`; the helper path
  is the signed/installed `DittoUacHelper.exe` resolved from the install
  directory, not from the data directory or `PATH`.
- The helper is single-instance per request; it exits after the paste or
  timeout (10 s).

### 4.3 Token and channel

- Token: 32 bytes from the OS CSPRNG (BCrypt/`RtlGenRandom`), 10 s TTL,
  single use, bound to:
  - the core PID and session id,
  - the user SID,
  - the target foreground window handle,
  - a random nonce.
- Delivery: an inherited pipe handle or an anonymous pipe duplicated into the
  elevated child. Do not put the token on the command line (visible in the
  process list) or in a world-readable file.
- The helper validates:
  - the parent PID and its image path,
  - the user SID matches the current user,
  - the token matches and is unexpired/unused,
  - the target window still matches the request.
  On failure it exits non-zero without sending input.
- The helper writes a single audit log line (request id, result, no content) to
  the user's log via the core or its own user-writable log; it must not require
  admin to log.

### 4.4 Behavior

- The core writes the clipboard payload first. The helper sends `Ctrl+V` with
  `SendInput` and exits. It does not read the clipboard history, the database,
  or arbitrary files.
- If T4 proves that the elevated target cannot read a medium-integrity
  clipboard write, the helper may receive the exact payload over the same
  authenticated one-shot channel and write it itself. This exception must be
  documented in T4 and reviewed by T8; it still executes no arbitrary commands.
- No `/uacpaste:<pid>` command-line switch. No relaunching the full Ditto
  process elevated. No persistence of elevation.
- The helper must not accept a pipe name, path, command, or URL from an
  untrusted source.

### 4.5 Failure handling

- Timeout, token mismatch, target change, or `SendInput` failure returns
  `DITTO_E_ELEVATION` to the popup. The user sees a clear message and can
  retry; no automatic elevation loop.
- If the helper crashes, the core logs the event (no content) and does not
  retry more than once per user action.

---

## 5. Clipboard data and privacy

### 5.1 Local-only

- No network listener, no telemetry, no analytics, no cloud, no account.
- No outbound connection in v1. A future update check must be user-initiated,
  HTTPS, and signature-verified.
- The legacy friends/P2P settings and passwords are not migrated and the code
  is not ported.

### 5.2 Sensitive clipboard content

- Honor Windows clipboard exclusion signals:
  - `CanIncludeInClipboardHistory = 0`
  - `ExcludeClipboardContentFromMonitorProcessing`
  - `CF_LOCALE` is preserved, not interpreted as trust.
- Provide app include/exclude lists (process names). Default: no exclusions,
  matching legacy behavior, but the UI explains the risk and offers
  password-manager guidance.
- Do not display clip content in notifications, tray tooltips, or window
  titles.
- Do not render HTML/RTF in a live webview. Previews are escaped text or a
  decoded image. No scripts, no remote images, no active content.
- File clips show paths; do not open/execute on hover or single click. Opening
  is an explicit action with the OS shell, if implemented at all.

### 5.3 Data at rest

- `ditto.db`, blobs, backups, exports, and logs live under the user's profile
  with inherited user-only ACL. Do not grant `Everyone`.
- v1 does not encrypt the DB. This is documented in Settings → About/Privacy
  and in `RISK-REGISTER.md`.
- Secrets: none in v1. If a future feature needs one, use DPAPI
  (`CryptProtectData`, current user) and store ciphertext only.
- Backups/exports are unencrypted; the UI warns before creating them. The user
  chooses the destination.
- Temp migration copies contain full history: delete them on success/failure
  and on next startup if a crash left one behind. Never write them to a shared
  temp folder; use a user-only subdirectory.

### 5.4 Logging and diagnostics

- Never log clip text, clipboard bytes, image data, search query text, window
  titles, or file contents. Log ids, hashes, sizes, format names, and result
  codes.
- Redact `%USERPROFILE%`, `%APPDATA%`, `%LOCALAPPDATA%`, and user names in
  release logs. Debug logs may include full paths only with an explicit user
  opt-in and a warning.
- Crash dumps can contain process memory, including clipboard data. They are
  written locally and are **not** uploaded. Sharing is opt-in; the UI lists
  what will be shared. Users can delete `crashes/`.
- Diagnostics bundles exclude clip content, settings secrets, and raw database
  pages. They include versions, counts, sizes, logs, and process memory.
- Log files are user-only ACL; rotation keeps at most five files of 5 MiB.

### 5.5 Development/QA privacy

- Never read the real `%APPDATA%\Ditto` or the user's real clipboard history
  during development or QA. Use:
  - synthetic fixtures in `tests/fixtures/**`;
  - a user-provided copy that is read-only and deleted after the test;
  - `DITTO_DATA_DIR` pointing to a temp directory.
- The migration test asserts the source SHA-256 is unchanged before/after.
- Screenshots/QA artifacts must not contain real clipboard content; use
  synthetic text such as "QA sample 001".

---

## 6. Settings webview (Tauri + React)

- The settings webview is a **view**, not a trusted core. It never opens SQLite,
  reads arbitrary files, spawns processes, or makes network requests.
- Tauri capabilities: allow only the settings command namespace and window
  controls. Do not enable the shell, HTTP, filesystem, process, or updater
  plugins for the webview. If a file dialog is needed, expose a narrow
  `pick_import_file`/`pick_export_path` command that validates the result and
  returns a token/id, not an arbitrary path-backed API.
- `tauri.conf.json`: no remote URLs, no `devUrl` in release, no external
  navigation. Block new-window/navigation handlers; deny `window.open`.
- CSP: `default-src 'self'; img-src 'self' blob: data:; style-src 'self'
  'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost`
  (adjust to the Tauri 2 IPC scheme; no remote hosts). No `unsafe-eval`.
- Render clip previews as escaped text or `<img>` from a validated blob MIME
  (PNG/JPEG/WebP/GIF). Never use `dangerouslySetInnerHTML` with clip content.
- Validate all inputs again in the Rust command layer. The webview is not a
  security boundary; a compromised webview must still not be able to run
  arbitrary commands.
- Dialog focus management, `aria-live`, and error messages must not leak file
  contents.
- WebView2: use the Evergreen runtime; do not enable remote debugging in
  release; disable the devtools in release; do not load extensions.

---

## 7. Legacy code, add-ins, and updates

### 7.1 Legacy code

- The old C++ tree moves to `legacy/` and is **read-only reference**. It is not
  compiled, linked, or loaded.
- Do not `LoadLibrary` from `Addins/`, the data directory, the install
  directory's writable subfolders, or `%TEMP%`. The only loaded modules are the
  app's own binaries and OS/WebView2 libraries.
- Do not port or embed ChaiScript. Do not execute legacy scripts or XML as
  code.
- Migration reads data only: `Ditto.db` tables and INI/registry values. It
  never loads legacy theme/language XML as executable content.

### 7.2 Unsigned binaries and Defender

- v1 binaries may be unsigned. Windows SmartScreen/Defender may warn or
  quarantine them. We report this honestly and do not modify Defender,
  exclusions, or system security settings.
- If signing becomes available, sign all three binaries and the installer;
  verify signatures where practical. T8 records the signing status.
- Code signing must not change the GPLv3 license or add telemetry.

### 7.3 Updates

- No auto-updater in v1.
- A future updater must:
  - use HTTPS with certificate validation,
  - fetch a signed manifest (Ed25519 or Authenticode) and verify before
    download/run,
  - verify the installer signature and hash,
  - support rollback and never run an unsigned update,
  - be user-initiated or explicitly opted in.
- The installer must stop the running core gracefully and restart only if the
  user chose auto-start.

---

## 8. Migration and import parsing

- The legacy DB is untrusted input. Open a **copy**, read-only for the
  original; bound all parsing:
  - max DB size (e.g., 4 GiB) and blob size (e.g., 256 MiB per format);
  - max group depth (e.g., 64) and cycle detection;
  - max paths in CF_HDROP (e.g., 100k) and max string length;
  - CF_HTML/RTF parsing bounds; no external entity/URL fetch;
  - DIB dimensions bounded; decompression bombs rejected.
- Never use the legacy DB path as a working directory or executable path.
- Import ZIPs: validate entries, reject absolute paths/`..` (zip slip), enforce
  uncompressed size limits, verify manifest SHA-256 for the DB and blobs, and
  import into a temp DB before swapping.
- On any parse failure, preserve the source and report; do not "repair" the
  original.
- Migration reports contain counts, hashes, and paths; not clip text.

---

## 9. Supply chain and build

- Rust dependencies: commit `Cargo.lock`; pin Tauri 2 and plugin major
  versions; prefer the standard crates (`rusqlite`, `windows`, `serde`,
  `image`, `tracing`) over ad-hoc ones. Run `cargo audit` where available; triage
  findings. No new framework unrelated to the architecture.
- Frontend: commit `package-lock.json`; React 19, Vite, Fluent UI React v9 only;
  no remote CDN at runtime; no analytics packages.
- Tauri plugins: only tray, global-shortcut, autostart, single-instance, log,
  and the minimum needed. Do not enable shell/HTTP/fs/updater for the webview.
- Build artifacts are reproducible enough to identify the commit (`build SHA`
  in About). The build must not download arbitrary scripts at build time; use
  locked dependencies.
- License: GPLv3. New code is GPLv3-compatible. Do not add a dependency with an
  incompatible license. Preserve `LICENSE` unchanged.
- Secrets/keys: no signing keys or API keys in the repo or build logs.

---

## 10. Security acceptance tests

| ID | Test | Pass criteria | Owner |
|---|---|---|---|
| SEC-1 | Pipe ACL | A different user/session and a remote client are rejected; Administrators have read-only | T1/T8 |
| SEC-2 | Auth | Wrong secret, missing secret, and non-allow-listed image are rejected with `DITTO_E_AUTH`; no payload logged | T1/T8 |
| SEC-3 | Frame limits | Oversized/malformed frames close the connection; no unbounded allocation | T1/T8 |
| SEC-4 | Arbitrary command/path | No message accepts a shell command, executable path, SQL, or URL; file paths only from a core-initiated dialog | T8 |
| SEC-5 | UAC token | Replay, expired token, wrong PID/SID/window, and second use all fail; helper exits | T4/T8 |
| SEC-6 | UAC trigger | No elevation on clipboard events, startup, timers, or background retries; only user paste | T4/T8 |
| SEC-7 | WebView capabilities | Shell/HTTP/fs/process commands are unavailable; remote navigation blocked; no devtools in release | T5/T8 |
| SEC-8 | Content rendering | Clip HTML/RTF is never rendered as active content; previews are escaped text or validated images | T5/T8 |
| SEC-9 | Logs | Automated scan finds no clip text/bytes, query text, titles, or secrets in logs/diagnostics | T7/T8 |
| SEC-10 | Legacy loaders | No `LoadLibrary` of add-ins/scripts; no ChaiScript; migration reads data only | T6/T8 |
| SEC-11 | Migration source | Source SHA-256 unchanged; temp copy deleted; no real user DB in tests | T6/T7 |
| SEC-12 | Import ZIP | Zip-slip, oversized entries, bad hashes, and schema mismatch are rejected; no partial swap | T6/T8 |
| SEC-13 | Data ACL | DB/blobs/logs are user-only; no `Everyone`/`Users` write | T1/T7 |
| SEC-14 | Network | A packet capture shows no outbound connections during normal use | T7/T8 |
| SEC-15 | Defender | Unsigned-binary warnings are recorded, not worked around; no security settings changed | T7/T8 |

---

## 11. Incident response and disclosure

- Security issues are reported to the project's issue tracker/security contact
  (the About page links to the source and issues). No telemetry is used to
  detect them.
- A suspected data exposure: stop, preserve logs (no content), rotate any
  secrets (none in v1), and document in `RISK-REGISTER.md`.
- A clipboard leak via logs/dumps is a release blocker until fixed and
  verified by SEC-9.
- A pipe/UAC bypass is a release blocker until fixed and verified by SEC-1/5/6.

---

## 12. Residual risks (accepted or tracked)

| Risk | Mitigation | Status |
|---|---|---|
| Same-user process can read the DB/blobs | OS user boundary; no multi-user claim | Accepted |
| Same-user process can read the endpoint secret | Same-user trust; pipe is not a privilege boundary | Accepted |
| Unsigned binaries flagged by Defender/SmartScreen | Report; sign later; never change security settings | Tracked |
| No DB encryption | Documented; future DPAPI/SQLCipher consideration | Accepted v1 |
| Crash dumps may contain content | Local only; opt-in sharing; deletable | Tracked |
| WebView2 is a large attack surface | No remote content, strict CSP/capabilities, on-demand lifetime | Tracked |
| UAC helper is a high-value target | Minimal code, one-shot, token-gated, no arbitrary commands | Tracked |

See `RISK-REGISTER.md` for impact/probability/owner.
