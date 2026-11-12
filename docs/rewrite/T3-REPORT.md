# T3 - egui native popup

- Date: 2026-11-12 (Asia/Shanghai)
- Status: implementation complete; runtime verification has documented gaps
- Worktree: `D:\orca\workspaces\Ditto\rewrite-popup`
- Expected base: `rewrite/tauri` @ `847f966`

## STATUS

The resident egui popup now has:

- `ClipProvider` with `IpcProvider` and `MockProvider`, executed on a background
  worker so pipe and mock work never blocks the UI thread.
- IPC reconnect/backoff, visible connection/search errors, and explicit
  unsupported-action messages instead of silent failures.
- A deterministic 10,000-clip mock fixture with text, HTML/RTF, image, file,
  pinned, grouped, first-ten, pasted, and copy-buffer states.
- Token-driven light, dark, and system-color high-contrast rendering.
- Virtualized clip rows, 75 ms search debounce, group breadcrumb/menu,
  preview, multi-selection, first-ten shortcuts, copy-buffer menu, and delete
  confirmation.
- Captive screenshot, release latency, scroll, and 20-cycle resource modes.

Strict present-time scroll behavior is not fully proven on this locked desktop.
The application-side update/draw submission work stayed below 0.4 ms, but
wall-clock frame spacing had isolated scheduler stalls above 50 ms in some
runs. This is reported below rather than hidden.

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-popup`

## BRANCH / HEAD

No git command was run because repository policy assigns all git operations to
the user. Branch and HEAD were therefore not queried. Expected base is
`rewrite/tauri` @ `847f966`; changes are uncommitted.

## FILES

Source:

- `apps/ditto-popup/Cargo.toml`
- `apps/ditto-popup/src/main.rs`
- `apps/ditto-popup/src/lib.rs`
- `apps/ditto-popup/src/app.rs`
- `apps/ditto-popup/src/options.rs`
- `apps/ditto-popup/src/provider.rs`
- `apps/ditto-popup/src/mock.rs`
- `apps/ditto-popup/src/worker.rs`
- `apps/ditto-popup/src/platform.rs`
- `apps/ditto-popup/src/theme.rs`
- `apps/ditto-popup/src/localization.rs`
- `apps/ditto-popup/src/ui.rs`
- `apps/ditto-popup/src/bin/mock-bench.rs`

Temporary T3 locale catalogs (coordinator-approved; T7 merges into the shared
T5 catalog):

- `apps/ditto-popup/locales/en-US.json`
- `apps/ditto-popup/locales/zh-CN.json`

Generated build metadata:

- `Cargo.lock` (popup-only direct dependencies `serde_json` and `image`)

Report:

- `docs/rewrite/T3-REPORT.md`

No source file under `crates/ditto-win`, `crates/ditto-storage`,
`crates/ditto-clipboard`, `apps/ditto/src-tauri`, `apps/ditto/ui`, `legacy`, or
the root `LICENSE` was edited. The documented build prerequisites generated
ignored `apps/ditto/ui/node_modules`, `apps/ditto/ui/dist`, and sidecar staging
files; these are build artifacts, not hand-edited source.

## IMPLEMENTATION

### Window and placement

- Hidden pre-warm is retained.
- `WS_EX_NOACTIVATE` and `WS_EX_TOOLWINDOW` are applied; no taskbar entry is
  requested.
- Show uses `SW_SHOWNOACTIVATE`; the popup was not foreground after show in the
  runtime probe.
- Always-on-top is enabled by default and can be toggled from the title pin.
- A user click temporarily enables activation for text input. Hide restores
  no-activate mode and returns focus to the previous foreground window when the
  popup had activated.
- An outside click while still non-activated passes to the target window before
  the popup detects and hides itself.
- Placement supports caret, cursor, and last position; candidates are clamped
  to the target monitor work area using each monitor's effective DPI.
- DWM requests rounded corners, light/dark immersive title treatment, and a
  token border color on non-high-contrast themes.

### Providers

`ClipProvider` supports list/search, groups, settings, paste modes, pin, move,
delete, copy-buffer list/assign/clear/paste. `IpcProvider` maps supported
operations to the existing T1/T2 `Request` enum. Copy-buffer mutation and
format-specific paste have no current request variant, so IPC returns a visible
`Unsupported` result. No IPC type was changed.

The IPC provider reconnects per request after transport failure. The UI worker
keeps stale snapshots visible while search/metadata errors are displayed in a
status strip.

`MockProvider` generates 10,000 deterministic clips and 50 groups. `--mock` or
`DITTO_POPUP_MOCK=1` selects it. No real clipboard history, database, or
`%APPDATA%`/`%LOCALAPPDATA%\Ditto` data is read.

### UI behavior

- The list is virtualized with `ScrollArea::show_rows`; observed maximum
  rendered rows was 9 out of 10,000.
- Search is debounced by 75 ms and is case-insensitive in the mock.
- Rows show type icon/index, preview text, source app, relative time, format,
  pin, pasted, and copy-buffer indicators.
- Preview supports text, a synthetic image checkerboard/thumbnail, `#RRGGBB`
  color swatch, file paths, and sanitized HTML/RTF summaries. HTML tags and
  script bodies are rendered as text or omitted, never executed.
- Empty, loading, error, disconnected, and unsupported-action states are
  visible.
- AccessKit is enabled through eframe defaults. Standard text/button widgets
  expose labels; custom rows publish a selected list-item label through
  `WidgetInfo`. Full list/listitem/Narrator coverage remains unverified.

## SHORTCUTS

| Input | Action |
|---|---|
| Up/Down | Move selection |
| PageUp/PageDown | Move by visible page |
| Home/End | First/last clip |
| Shift+Up/Down/Home/End | Extend selection |
| Ctrl+click | Toggle multi-selection |
| Shift+click | Select range |
| Space, list focus | Toggle current selection |
| Ctrl+A, list focus | Select all visible |
| Tab / Shift+Tab | Cycle title, list, search |
| Enter | Paste selected item(s) |
| Shift+Enter | Paste as plain text |
| Ctrl+P | Pin/unpin selected |
| Delete | Delete selected, with confirmation |
| F3 | Toggle preview |
| Ctrl+1..9, Ctrl+0 | Paste first-ten item 1..10 |
| Ctrl+Shift+1..5 | Assign selected clip to copy buffer |
| Alt+1..5 | Paste copy buffer 1..5 |
| Esc | Clear search, close preview, then hide |
| Right-click | Special paste/move/pin/delete/copy-buffer menu |

Title search input uses egui `TextEdit`, retaining IME composition, caret,
selection, `Ctrl+A`, and `Backspace` behavior. Enter/Esc/arrow shortcuts are
suppressed while egui reports an active IME preedit.

## BUILD

Workspace gates, all successful:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo build --release -p ditto-popup-app --locked
```

Logs:

- `artifacts/rewrite/after/logs/cargo-fmt-check.log`
- `artifacts/rewrite/after/logs/cargo-clippy-workspace.log`
- `artifacts/rewrite/after/logs/cargo-test-workspace.log`
- `artifacts/rewrite/after/logs/cargo-build-workspace.log`
- `artifacts/rewrite/after/logs/cargo-build-popup-release.log`

The workspace host build required the documented prerequisites:

```powershell
cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh scripts/prepare-sidecars.ps1 -Profile debug
cd apps/ditto/ui
npm ci
npm run build
```

## TEST

- Popup package: 17 tests passed.
- Full workspace: 22 tests passed (including 17 popup tests).
- Mock search regression test: 10k search p95 below 50 ms.
- HTML summary test: script body is not exposed.
- IME state test: preedit blocks shortcuts until commit.
- Shortcut mapping test: Ctrl+1..9 and Ctrl+0 map to indexes 0..9.
- Mock copy-buffer assign/clear round trip passes.
- IPC request mapping uses `GetClips` for empty query and `Search` for text.
- No tests read real user data.

## EVIDENCE

### Screenshots

All screenshots are real egui framebuffer captures from the release popup, not
desktop composites or fabricated mockups. A locked desktop does not prevent
these captures.

| File | State | Pixels |
|---|---|---:|
| `artifacts/rewrite/after/screenshots/light-100-default.png` | Light default | 420x480 |
| `artifacts/rewrite/after/screenshots/dark-100-default.png` | Dark default | 420x480 |
| `artifacts/rewrite/after/screenshots/dark-200-default.png` | Dark 200% | 840x960 |
| `artifacts/rewrite/after/screenshots/light-150-default.png` | Light 150% | 630x720 |
| `artifacts/rewrite/after/screenshots/light-200-default.png` | Light 200% | 840x960 |
| `artifacts/rewrite/after/screenshots/light-100-empty.png` | Empty search | 420x480 |
| `artifacts/rewrite/after/screenshots/light-100-search.png` | Search results | 420x480 |
| `artifacts/rewrite/after/screenshots/dark-100-preview.png` | Preview | 420x480 |
| `artifacts/rewrite/after/screenshots/light-100-groups.png` | Group breadcrumb | 420x480 |
| `artifacts/rewrite/after/screenshots/dark-100-multi.png` | Multi-select | 420x480 |
| `artifacts/rewrite/after/screenshots/zh-100-search.png` | Chinese UI | 420x480 |
| `artifacts/rewrite/after/screenshots/hc-100-default.png` | System-color HC palette | 420x480 |

Hashes are in `artifacts/rewrite/after/screenshots/sha256.csv`.

### Window style

`artifacts/rewrite/after/evidence/window-style.json` records:

- visible popup: yes;
- extended style: `0x8000080`;
- `WS_EX_TOOLWINDOW`: true;
- `WS_EX_NOACTIVATE`: true;
- popup was foreground: false.

### First frame, release

30 fresh release samples. The measurement starts when the popup requests show
and ends after the first UI update/draw submission. It is not PresentMon.

| Samples | p50 | p95 | p99 | Max | >=50 ms |
|---:|---:|---:|---:|---:|---:|
| 30 | 18.927 ms | 26.738 ms | 26.966 ms | 26.966 ms | 0 |

Raw: `artifacts/rewrite/after/perf/popup-first-frame-release.csv`.

### 10k mock search

30 runs per query. Each load scans 10,000 synthetic records and clones the
returned page. Timings are microseconds.

| Query | p50 | p95 | Max | Rows |
|---|---:|---:|---:|---:|
| empty/recent | 3,273 | 3,925 | 3,940 | 10,000 |
| `meeting` | 1,236 | 1,589 | 1,822 | 0 |
| `客户项目` | 1,453 | 1,620 | 1,751 | 1,245 |
| `日本語` | 1,559 | 2,320 | 31,644 | 1,658 |
| `Synthetic clipboard` | 2,080 | 2,405 | 2,604 | 3,319 |
| `CF_UNICODETEXT` | 4,476 | 5,267 | 5,416 | 10,000 |

All p95 values are below 50 ms. One Japanese maximum sample was 31.644 ms but
the p95 remained 2.320 ms.

Raw:

- `artifacts/rewrite/after/perf/search-10k-mock.csv`
- `artifacts/rewrite/after/perf/search-10k-mock-summary.csv`

### 10k scroll, release

Final instrumented run: 3,600 automatically scrolled frames, 8 rows advanced per
frame, 9 rendered rows maximum.

| Metric | p50 | p95 | p99 | Max | Over 50 ms |
|---|---:|---:|---:|---:|---:|
| Wall frame interval | 5.556 ms | 6.431 ms | 9.068 ms | 137.551 ms | 2 |
| Popup update/draw submission | 0.005 ms | 0.007 ms | 0.009 ms | 0.388 ms | 0 |

Runs:

- Release run 1: wall p99 7.998 ms, max 111.377 ms, 2 frames >50 ms.
- Release run 2: wall p99 7.370 ms, max 69.560 ms, 1 frame >50 ms.
- Release run 3: wall p99 8.480 ms, max 34.930 ms, 0 frames >50 ms.
- Final instrumented run: wall p99 9.068 ms, max 137.551 ms, 2 frames >50 ms;
  corresponding UI work was 0.01 ms on both outliers.

Raw:

- `artifacts/rewrite/after/perf/scroll-10k-release.csv`
- `artifacts/rewrite/after/perf/scroll-10k-release-2.csv`
- `artifacts/rewrite/after/perf/scroll-10k-release-3.csv`
- `artifacts/rewrite/after/perf/scroll-10k-release-final2.csv`

### 20 show/hide cycles, release

| Metric | First | Last | Delta |
|---|---:|---:|---:|
| GDI objects | 17 | 17 | 0 |
| USER objects | 17 | 17 | 0 |
| Private bytes | 132,820,992 | 132,820,992 | 0 |
| Working set | 116,039,680 | 116,043,776 | 4,096 |

Raw: `artifacts/rewrite/after/perf/popup-show-hide-20-release.csv`.

## UNVERIFIED

- Physical hardware hotkey to visible first frame on an unlocked desktop.
- Microsoft Pinyin/Japanese candidate window placement and commit behavior in a
  real IME session. Only egui IME state handling is tested.
- Keyboard input immediately after show while the window remains
  non-activated. One click is currently required to activate text input.
- DWM corner/border rendering by visual inspection; attributes are requested
  but the screenshot is an egui framebuffer, not a desktop/DWM composite.
- Real multi-monitor mixed-DPI movement; placement is DPI-aware but no physical
  mixed-DPI monitor walkthrough was possible.
- Windows High Contrast Black/White enabled at the OS level. The HC screenshot
  uses system-color resolution while the current OS high-contrast mode is off.
- Narrator/Accessibility Insights traversal and full custom list semantics.
- Swap-chain presentation timing, PresentMon, and an unlocked 60 Hz desktop
  scroll run with zero scheduler stalls.
- Real T2 core data/search/paste/copy-buffer integration. The current T1 IPC
  server returns `NotImplemented` for those requests.

## RISKS

- `WS_EX_NOACTIVATE` prevents OS keyboard focus until the user clicks. This
  preserves the target window's first click but means hotkey-then-type without a
  click is not proven. T7 must decide the intended focus tradeoff.
- Wall-clock scroll runs show occasional desktop scheduling stalls although
  popup update work is below 0.4 ms. PresentMon on unlocked hardware is needed
  for a release verdict.
- The current IPC v1 request shape has no group filter, format-specific paste,
  preview blob, or copy-buffer mutation. Those paths are client-side,
  synthetic-only, or visibly unsupported.
- Mock preview images are synthetic checkerboards; real thumbnail decode/cache
  belongs to T2/T7 integration.
- The 10k mock fixture increases popup private memory while loaded. This is not
  an idle-memory sample and must not be compared with PF-1.

## NEXT

1. T7 merges T3 and reruns physical hotkey, focus, DWM, multi-monitor, IME,
   high-contrast, and PresentMon checks.
2. T2/T7 connect real list/search/preview/paste and decide whether a v1 IPC
   additive extension is needed for group filtering, format paste, and
   copy-buffer mutation.
3. T5 merges all `popup.*` keys from the temporary T3 catalogs into the shared
   locale directory.

## LOCALE KEYS

- `app.name`
- `popup.search.placeholder`
- `popup.search.resultCount`
- `popup.search.searching`
- `popup.search.clear`
- `popup.shortcutHint`
- `popup.shortcutHintCompact`
- `popup.group.all`
- `popup.group.ungrouped`
- `popup.group.choose`
- `popup.group.back`
- `popup.group.current`
- `popup.preview.toggle`
- `popup.preview.title`
- `popup.preview.color`
- `popup.preview.files`
- `popup.preview.image`
- `popup.preview.rich`
- `popup.preview.close`
- `popup.preview.open`
- `popup.preview.empty`
- `popup.title.alwaysOnTop`
- `popup.title.close`
- `popup.state.loading`
- `popup.state.empty.title`
- `popup.state.empty.body`
- `popup.state.empty.search`
- `popup.state.error.title`
- `popup.state.error.body`
- `popup.state.retry`
- `popup.state.disconnected`
- `popup.status.pasteSent`
- `popup.status.plainPasteSent`
- `popup.status.pinned`
- `popup.status.moved`
- `popup.status.deleted`
- `popup.status.bufferAssigned`
- `popup.status.bufferCleared`
- `popup.status.bufferPasted`
- `popup.status.unsupported`
- `popup.status.failed`
- `popup.status.searchFailed`
- `popup.status.reconnected`
- `popup.status.took`
- `popup.menu.paste`
- `popup.menu.pastePlain`
- `popup.menu.pasteHtml`
- `popup.menu.pasteRtf`
- `popup.menu.pasteFiles`
- `popup.menu.pin`
- `popup.menu.unpin`
- `popup.menu.move`
- `popup.menu.delete`
- `popup.menu.copyToBuffer`
- `popup.menu.pasteBuffer`
- `popup.menu.preview`
- `popup.menu.bufferEmpty`
- `popup.menu.bufferName`
- `popup.delete.title`
- `popup.delete.body`
- `popup.delete.confirm`
- `popup.delete.cancel`
- `popup.format.text`
- `popup.format.html`
- `popup.format.rtf`
- `popup.format.image`
- `popup.format.files`
- `popup.format.custom`
- `popup.badge.pinned`
- `popup.badge.pasted`
- `popup.badge.firstTen`
- `popup.badge.copyBuffer`
- `popup.source.unknown`
- `popup.time.now`
- `popup.time.minutes`
- `popup.time.hours`
- `popup.time.days`

## POPUP CLI EVIDENCE MODES

```powershell
DittoPopup.exe --mock --auto-show
DittoPopup.exe --mock --capture=out.png --capture-state=preview --theme=dark --dpi-scale=1.5 --lang=zh-CN
DittoPopup.exe --latency-only --perf-log=latency.csv
DittoPopup.exe --mock --bench-scroll --bench-frames=3600 --bench-log=scroll.csv
DittoPopup.exe --mock --soak=20 --soak-log=show-hide.csv
cargo run -p ditto-popup-app --bin mock-bench --locked -- search.csv
```
