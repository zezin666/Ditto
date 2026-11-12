# CLAUDE.md

## Project

Ditto is a Windows-only, local-first clipboard manager rewritten in Rust with a
Tauri 2 host and a native egui popup. Do not load or compile legacy C++ code.

## Commands

```powershell
cargo build -p ditto-popup-app -p ditto-uac-helper-app
pwsh scripts/prepare-sidecars.ps1 -Profile debug
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Build the settings placeholder before invoking the Tauri CLI:

```powershell
cd apps/ditto/ui
npm ci
npm run build
cd ../..
cargo build --release -p ditto-tauri -p ditto-popup-app -p ditto-uac-helper-app
pwsh scripts/prepare-sidecars.ps1 -Profile release
cd apps/ditto
npx tauri build
```

Run `target/debug/Ditto.exe --background`; the development hotkey is
`Ctrl+Alt+D`, with `Ctrl+Alt+Shift+D` as the available-key fallback.

## Architecture

- `apps/ditto/src-tauri` is a thin Tauri host. It must create no window at
  startup. Settings is built on demand and destroyed on close.
- `apps/ditto-popup` is resident but hidden. It owns no database handle,
  clipboard listener, or hotkey registration.
- Domain crates must not depend on Tauri.
- `ditto-ipc` is the single source of protocol messages and framing. Protocol
  v1 uses the `DTP1` 24-byte header and UTF-8 JSON control payloads.
- The pipe is `\\.\pipe\ditto-<current-user-sid>` and is ACL-restricted to the
  current user. Never weaken or bypass the ACL.
- The core writes clipboard payloads. The UAC helper only sends `Ctrl+V` unless
  a reviewed T4 result proves an OS limitation.

## Conventions

- Rust edition 2024, rustfmt defaults, and clippy warnings denied.
- All user-visible colors, dimensions, radii, and motion values come from
  generated `design/tokens.json` outputs.
- User-visible strings belong in `apps/ditto/ui/locales/`; no hardcoded
  production English in T3/T5 UI code.
- No clip text, bytes, search query, window title, or secret in logs.
- Do not read real `%APPDATA%\Ditto`, `%LOCALAPPDATA%\Ditto`, or clipboard
  history in tests. Use synthetic fixtures and `DITTO_DATA_DIR`.
- `legacy/` and `LICENSE` are read-only.
- Do not change Defender, SmartScreen, firewall, or system security settings.
- Do not add a framework outside Tauri 2, egui, React 19 + Fluent UI v9,
  SQLite, and small support crates.

The frozen design and decisions are under `docs/rewrite/`.

