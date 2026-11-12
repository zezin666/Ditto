# Ditto

Ditto is a local-first Windows clipboard manager. This branch is the Tauri
rewrite foundation: a resident Tauri 2 host with no startup WebView, a
pre-warmed egui popup, and an on-demand settings webview.

## v1 Scope

- Windows 10 1809+ and Windows 11, x64.
- Text, rich text, image, file, and unknown clipboard formats.
- Search, groups, pinning, five copy buffers, and multi-paste.
- English and Simplified Chinese.
- Portable ZIP and NSIS packaging.
- Read-only migration from legacy Ditto data.
- No legacy add-ins, scripts, themes, language packs, P2P, telemetry, or cloud.

The full scope and acceptance criteria live in
[`docs/rewrite/FEATURE-MATRIX.md`](docs/rewrite/FEATURE-MATRIX.md).

## Build And Run

Prerequisites:

- Rust stable MSVC x64 with `rustfmt` and `clippy`.
- Visual Studio Build Tools with MSVC and a Windows SDK.
- Node 24 and npm 11.
- WebView2 Evergreen runtime.
- Tauri CLI 2.x: `cargo install tauri-cli --version "^2" --locked`, or the
  npm fallback `npm install -D "@tauri-apps/cli@^2"` in `apps/ditto`.

```powershell
cd apps/ditto/ui
npm ci
npm run build

cd ../..
cargo build -p ditto-popup-app -p ditto-uac-helper-app
pwsh scripts/prepare-sidecars.ps1 -Profile debug

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Run the resident host and popup:

```powershell
.\target\debug\Ditto.exe --background
```

The development hotkey is `Ctrl+Alt+D`, with `Ctrl+Alt+Shift+D` as a startup
probe fallback if the primary is already owned. Tray menu entries are Show,
Settings, and Exit. Settings is created only when requested; no WebView2 window
exists at startup.

The token source is [`design/tokens.json`](design/tokens.json). Regenerate
checked-in Rust, CSS, and TypeScript outputs with:

```powershell
node scripts/generate-tokens.mjs
```

For a bundled NSIS build:

```powershell
cargo build --release -p ditto-tauri -p ditto-popup-app -p ditto-uac-helper-app
pwsh scripts/prepare-sidecars.ps1 -Profile release
cd apps/ditto
npx tauri build
```

Sidecars are generated because Tauri requires target-triple-suffixed files for
`externalBin`. Do not commit the generated executables.

## Portable Mode

Installed builds use `%LOCALAPPDATA%\Ditto\v2\`. A `portable` marker beside
`Ditto.exe` or `--portable` selects `<exe_dir>\Data\`. Tests may override the
root with `DITTO_DATA_DIR`; development never reads the real legacy
`%APPDATA%\Ditto` store.

## Repository Layout

| Path | Purpose |
|---|---|
| `apps/ditto` | Tauri 2 core host and on-demand settings webview |
| `apps/ditto-popup` | Resident egui/winit popup |
| `apps/ditto-uac-helper` | One-shot elevated paste helper |
| `crates/ditto-model` | Shared types, errors, and configuration |
| `crates/ditto-ipc` | Versioned named-pipe protocol and framing |
| `crates/ditto-win` | Narrow Win32 helpers |
| `crates/ditto-clipboard` | Clipboard boundary (T2) |
| `crates/ditto-storage` | SQLite boundary (T2) |
| `crates/ditto-paste` | Paste boundary (T4) |
| `crates/ditto-migration` | Legacy migration boundary (T6) |
| `design` | Fluent token source and generators |
| `legacy` | Read-only C++ reference; never built or loaded |
| `docs/rewrite` | T0 architecture, design, risks, and phase notes |

Architecture: [`docs/rewrite/ARCHITECTURE.md`](docs/rewrite/ARCHITECTURE.md).
Design system: [`docs/rewrite/DESIGN-SYSTEM.md`](docs/rewrite/DESIGN-SYSTEM.md).
Performance protocol: [`docs/rewrite/PERFORMANCE.md`](docs/rewrite/PERFORMANCE.md).

