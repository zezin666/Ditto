# T11 - 修复 T10 残余问题 + 可复现 QA

## STATUS

**完成。** R-1…R-10 全部 FIXED（R-2 的 copy-buffer `cut` 明确列为 v1 不支持的全局动作，
UI 仅该字段 disabled 并在矩阵/本地化文案中说明；其余 hotkey Must 均已交付）。
另修复了一个在 end-to-end paste 验证中暴露的既有 Blocker 级真实缺陷：
`crates/ditto-paste/src/clipboard.rs` 的 `GlobalUnlock` 成功语义被误判为错误，
导致所有真实粘贴在写剪贴板阶段失败（T9/T10 的单元测试使用 fake backend，未覆盖）。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-fixes2`

## BRANCH

`taohy/rewrite-fixes2`（base `taohy/rewrite-review2` @ `71daf56`；未 push、未合并 master）

## HEAD

`<pending>`（本报告与全部修复在同一提交；提交后由编排者/T12 读取实际 SHA）

## FILES

源码/配置：

- `apps/ditto/src-tauri/src/{lib.rs,hotkeys.rs,logging.rs,core_service.rs,commands/mod.rs}`
- `apps/ditto/src-tauri/tests/log_level.rs`（新增）
- `apps/ditto-popup/src/{app.rs,platform.rs,provider.rs,options.rs}`
- `apps/ditto/ui/src/pages/HotkeysPage.tsx`、`apps/ditto/ui/src/api/browserMock.ts`
- `apps/ditto/ui/locales/{en-US.json,zh-CN.json}`
- `crates/ditto-win/src/lib.rs`
- `crates/ditto-clipboard/src/{decode.rs,lib.rs}`
- `crates/ditto-paste/src/clipboard.rs`
- `crates/ditto-storage/src/store.rs`、`crates/ditto-storage/examples/seed_qa.rs`
- `scripts/package-portable.ps1`
- `tools/qa/{keyboard-first.ps1,hotkey-rebind.ps1,scroll-10k-ipc.ps1,grouped-main.ps1,theme-modes.ps1,cold-start.ps1}`（新增）
- `tools/qa/{ipc-smoke.ps1,paging-check.ps1,popup-hide-sync.ps1,install-smoke.ps1}`（隔离后缀）

文档：`docs/rewrite/{T11-FIX-REPORT.md,QA-REPORT.md,REVIEW-GUIDE.md,FEATURE-MATRIX.md,DECISIONS.md}`

证据：`artifacts/rewrite/after/**`（T11 文件带 `-t11` 后缀）。

## BUILD

环境：Windows 11 26200、Rust/Cargo 1.99.1 MSVC、Node 24.13.0/npm 11.6.2、
Tauri CLI 2.12.x、Windows SDK 26100。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS（128 passed，0 failed） |
| `npm ci` + `npm run build --prefix apps/ditto/ui` | PASS（locales 417+77、tsc、Vite） |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS |
| `npx --no-install tauri build` | PASS（NSIS 4,884,177 B） |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS（ZIP 6,874,467 B） |
| `tools/qa/install-smoke.ps1`（隔离后缀） | PASS（安装/卸载/Portable/`Data\ditto.db`/pipe） |

产物：`artifacts/rewrite/after/build/t11-hashes.txt`（4 个 exe/installer 均 `NotSigned`）。

## TEST

新增/加强的自动化测试：

- `hotkeys`: `settings_bindings_include_every_global_action`、
  `unassigned_legacy_chords_are_not_registered`、`duplicate_chords_are_rejected`、
  `swap_bindings_moves_buffers_and_rolls_back_on_conflict`。
- `logging`: `runtime_level_changes_the_predicate`；集成测试
  `tests/log_level.rs` 在同一进程内 info→debug→info 重载并断言实际记录。
- `ditto-clipboard`: `legacy_text_payloads_are_nul_terminated_and_round_trip`
  （CP936/CP1252/CP437）。
- `ditto-storage`: `ungrouped_filter_excludes_grouped_clips`、
  `rewrite_clip_text_terminates_ansi_and_oem_payloads`。
- `ditto-paste`: `movable_global_blocks_unlock_successfully`、
  `zero_length_payloads_still_allocate_a_single_byte`（真实 `GlobalAlloc/GlobalUnlock`）。
- `ditto-popup`: first-ten chord 解析、hidden-group 映射、theme 值映射、
  provider group filter 转发、Enter/Shift+Enter 顺序修复。
- `ditto-win`: 既有 ACL/slow-client 测试继续通过；新增后缀命名由运行期 QA 覆盖。

## EVIDENCE

### R-1 Blocker — hotkey 后直接键盘输入

**FIXED。**

实现：

- `show_popup` 在激活前记录 `previous_foreground`；核心 pipe 的 visibility hook
  （`PipeServer::start_with_handler_and_visibility`）在 Show 广播前快照
  HWND/PID/thread/focus/integrity 到 `TargetSlot`，Hide 时清空。hotkey、tray、
  IPC `ShowPopup` 走同一 hook。
- `platform::activate_popup` 清除 `WS_EX_NOACTIVATE`、`ShowWindow(SW_SHOW)`、
  `SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW)`，用 `AttachThreadInput` 后
  `SetForegroundWindow` + `SetFocus`；Show 后 350 ms 内最多重试 4 次。
- `shown_at` 在激活后重置，避免慢首帧被误判为失焦；失焦判断同时接受
  winit focus 与 Win32 `GetForegroundWindow`。
- 粘贴仍使用 Show 前快照，绝不使用 DittoPopup。Esc/热键切换隐藏时恢复目标；
  失焦/外部点击隐藏时保留新前台窗口（`hide_popup_keep_foreground`）。
- **取舍（已写入 DECISIONS Q-08/D-119、FEATURE-MATRIX P1/P13）**：hotkey 路径
  会让 popup 抢到键盘焦点，这是 keyboard-first 的预期取舍；点击外部时由 Windows
  正常激活目标窗口并传递首次点击。

证据（release）：`tools/qa/keyboard-first.ps1` →
`SEARCH_TYPED_WITHOUT_MOUSE=True`、`POPUP_FOREGROUND_AFTER_SHOW=True`、
`ENTER_PASTE_REQUEST=True`、`PASTE_LANDED_IN_TARGET_AFTER_ENTER=True`、
`SHIFT_ENTER_PLAIN_PASTE_REQUEST=True`、
`PASTE_LANDED_IN_TARGET_AFTER_SHIFT_ENTER=True`、`KEYBOARD_FIRST_QA=PASS`。
目标为 QA 创建的受控 Win32 EDIT 窗口，实际收到粘贴文本。
证据文件：`after/qa/keyboard-first-t11.log`、`...-popup-trace.log`。

边界：物理热键按键本身仍 UNVERIFIED（自动化用 IPC `ShowPopup` 触发同一 Show 路径）；
Enter/Shift+Enter 的最终触发在自动化环境中使用 `WM_KEYDOWN` 合成消息（键入本身使用
物理 `keybd_event`）。真实 Notepad/Word/浏览器粘贴仍 UNVERIFIED。

### R-2 Major — A4 热键 Must

**FIXED（cut 为 v1 明确不支持）。**

- 启动时从设置读取并注册：`open_chord`、`open_alt_chord`、
  `plain_text_paste_chord`、每个 copy-buffer 的 `copy`/`paste`。
- 全局动作：open/open_alt = 显示/隐藏；plain = 将最近一条剪贴内容以纯文本粘贴到
  Show 前目标；copy-buffer copy = 把当前系统剪贴板内容存入 slot；paste = 粘贴 slot。
- first-ten 为 popup 可见时的作用域快捷键，chord 从设置读取、可禁用；索引映射
  `1..9,0 → 0..9`。
- Apply 使用 `swap_bindings`：保留未变项，注销删除项，注册新增项；任一冲突则注销
  新增、恢复旧项并返回错误，`set_settings` 回滚持久化。启动冲突回退到 open chord
  并写回，避免“显示未注册的 chord”。
- UI 移除 openAlt/plain/first-ten/copy/paste 的 disabled；仅 copy-buffer `cut`
  保持 disabled，并使用专用文案说明 v1 不提供全局 cut。
- 默认 plain/copy-buffer chord 为空（legacy 默认值为 0），避免全局劫持 Alt+1 等。
- 矩阵 A4/P8 与 DECISIONS 已同步。

证据：`tools/qa/hotkey-rebind.ps1` → 配置的 open/copy chord 被本进程占用、旧默认
chord 空闲；模拟 Ctrl+Alt+1 后 copy-buffer slot 1 填入当前剪贴板内容；修改持久化
chord 重启后旧键释放、新键占用；外部进程占用 Ctrl+Alt+J 时核心回退到 `Ctrl+``
且 `CONFLICT_FELL_BACK_WITHOUT_SILENT_HOOK=True`；`HOTKEY_REBIND_QA=PASS`。
文件：`after/qa/hotkey-rebind-t11.log`。

### R-3 Major — `show_grouped_in_main=false`

**FIXED。**

- 新增 `main_list_group`：`false` + All → `Ungrouped`（核心 group_id=-1），主列表
  排除 grouped clips；显式分组导航不变。
- 修复快照优先级 bug：默认 `appearance.show_grouped_in_main=true` 曾遮蔽
  `groups.show_grouped_in_main=false`；现在先读 groups。
- metadata 后到达且值变化时在可见状态下重新查询。

证据：`tools/qa/grouped-main.ps1` →
true: `GROUPED_MAIN_True_LOADS=... load group=All`；
false: `... load group=Ungrouped`；`GROUPED_MAIN_QA=PASS`。
文件：`after/qa/grouped-main-t11.log`。
单测：popup `hidden_groups_are_excluded_only_from_the_main_list`、provider
`group_filters_are_forwarded_to_the_core`、storage
`ungrouped_filter_excludes_grouped_clips`。

### R-4 Major — theme 映射

**FIXED。**

- 统一解析 `system/light/dark/highContrast` 及 `hc/high_contrast/high-contrast`。
- `system` 保留为 ThemeMode::System；每次 Show 和设置变更重新按 Windows
  light/dark/high-contrast 解析，dark→system 不粘滞。
- HC 使用 `GetSysColor` 系统色（`Palette::for_theme(HighContrast)`）。

证据：`tools/qa/theme-modes.ps1` → dark 应用、运行时切 highContrast 应用且
`palette_high_contrast=true`、切 system 后本机（`AppsUseLightTheme=1`）解析为
`Light`、持久化为 `system`；`THEME_MODE_QA=PASS`。
文件：`after/qa/theme-modes-t11.log`。单测：`theme_values_map_from_ui_and_legacy_spellings`。

### R-5 Major — runtime log level

**FIXED。**

- 新增 `logging.rs`：`AtomicU8` 可重载级别；`tauri-plugin-log` 以
  `.level(Trace)` + `.filter(logging::enabled)` 安装，所有记录逐条经过可重载判断。
- `apply_host_settings` 改调 `set_runtime_level_from_setting`；不再只改
  `log::set_max_level`（fern 已在启动时丢弃）。

证据：`apps/ditto/src-tauri/tests/log_level.rs` 在同一进程内验证
info 下 debug 被丢弃、切 debug 后新 debug 记录出现、切回 info 再次丢弃；
`cargo test` PASS。未点击真实 Settings UI 触发该路径（wiring 已由代码审查覆盖）。

### R-6 Major — ANSI/OEM 编辑 payload

**FIXED。**

- `encode_ansi_text` / `encode_oem_text` 改为 `encode_code_page_terminated`，
  按 CP_ACP/CP_OEMCP 编码并追加 NUL；空文本返回单个 NUL。
- `rewrite_clip_text` 的 hash/preview/FTS/Original paste 使用同一 payload。

证据：`legacy_text_payloads_are_nul_terminated_and_round_trip`（CP936/1252/437）、
`rewrite_clip_text_terminates_ansi_and_oem_payloads`（存储内 CF_TEXT/CF_OEMTEXT
末字节为 0 且解码回原文）。

### R-7 Major — 冷启动

**FIXED（代码/可见反馈），残余为未签名二进制的 Defender 首扫。**

- 保留 early spawn + IPC prewarm；新增 tray “Ditto is preparing the popup…”
  （EN/ZH）在 popup 尚未 Hello 时显示，避免热键无反馈。
- 首次加载 CJK 时只加载一个系统 CJK 字体（既有行为）；无新增常驻线程。

证据（release，`tools/qa/cold-start.ps1`，3 次同 binary + 1 次新拷贝）：

| 场景 | host→Show | popup start→Show | popup start→IPC ready |
|---|---:|---:|---:|
| 同 binary #1 | 3,540.4 ms | 1,774.0 ms | 1,634.0 ms |
| 同 binary #2 | 3,518.0 ms | 1,765.0 ms | 1,691.0 ms |
| 同 binary #3 | 3,506.6 ms | 1,756.0 ms | 1,672.0 ms |
| 新拷贝首启 | 4,724.6 ms | 1,834.0 ms | 1,686.0 ms |

同 binary popup 冷启动中位约 1.76 s；新拷贝的额外延迟在 host（Defender 扫描新
`Ditto.exe`），不在 popup。常驻后热键→首帧见 ipc-smoke。文件：
`after/qa/cold-start-t11.log`。发布建议仍为签名；未修改 Defender/SmartScreen。

### R-8 Major — 真实 10k scroll harness

**FIXED。**

- `--bench-scroll` 不再强制 mock；新增 `--provider=ipc|mock` 显式选择。
- 新增 `--bench-warmup`（默认 10），剔除窗口/GPU/字体首帧，单独由首帧指标衡量。
- 4 行/帧改为 40 行/帧，300 帧覆盖约 12k 行；靠近尾部自动拉取下一页。
- 新增 `tools/qa/scroll-10k-ipc.ps1`：临时数据根 + 唯一 pipe 后缀 + `seed_qa`
  真实 10k 行 + 真实 IPC provider + 300 帧 CSV，按 P18 计算。

Release 结果（`after/perf/scroll-10k-real-ipc-t11.csv`）：

```text
DB_TOTAL_ROWS=10000
FRAMES=300 (warmup 10 excluded)
FRAME_P50_MS=1.347
FRAME_P95_MS=2.624
FRAME_MAX_MS=7.246
FRAMES_OVER_16_7_MS=0
UPDATE_P95_MS=0.009
MAX_RENDERED_ROWS=9
MAX_SCROLL_OFFSET_PX=162000.0
P18_FRAME_BUDGET=PASS
```

Debug 构建（未用 warmup 前）有 3 帧 >16.7 ms、max 248 ms；release + warmup 后
0 帧超限。原始 CSV 保留，不伪造成 steady-state 数据。

### R-9 Minor — Portable README 反引号

**FIXED。** `package-portable.ps1` 使用双反引号转义；生成的 README 为
``Press Ctrl+` for the popup``（单反引号）。

### R-10 Minor — 文档一致性

**FIXED。**

- `REVIEW-GUIDE.md`：删除“设置热键未注册”旧结论；更新 hotkey/ACL/冷启动/内存
  风险；新增 T11 入口与 QA 说明。
- `DECISIONS.md`：D-025/D-209 改为一次性、无 progress/cancel；D-102 ACL 仅
  SYSTEM + 当前 SID（无 Administrators read）；D-119/Q-08 更新为 hotkey Show
  激活的 keyboard-first 取舍；X-04 增加 QA 后缀。
- `FEATURE-MATRIX.md`：P1/P8/P13/P18/G6/A4/A7/A10 与 T11 实际一致。

### QA 隔离（必须）

- `crates/ditto-win` 新增 `instance_suffix()`：`DITTO_PIPE_SUFFIX` 或
  `DITTO_INSTANCE_ID`，清洗为 `[A-Za-z0-9_-]{0,48}`；pipe 名与
  `Local\Ditto.v2.<SID>.*` 事件名追加同一后缀，ACL 不变。
- 有后缀时禁用 Tauri single-instance 插件的兜底，避免 QA 实例被用户/其他 QA
  实例拦截；快速事件路径仍强制单实例。
- 所有 T11 QA 与更新后的 ipc-smoke/paging-check/popup-hide-sync/install-smoke
  默认生成唯一后缀，使用临时 `DITTO_DATA_DIR`；未读取真实用户数据，未终止
  `D:\Programs\Ditto` 或任何用户进程。
- 遗留的 T9 `rewrite-fixes` popup 进程仍在运行；T11 QA 按 StartTime 过滤，
  未触碰它。

## UNVERIFIED

1. 物理热键按键本身（自动化使用 IPC Show + 合成键入/Enter；同一代码路径）。
2. 真实 Notepad/Word/浏览器/Explorer 的 text/HTML/RTF/image/files 粘贴。
3. 真实 UAC elevated target、取消、超时、secure desktop。
4. 真实 IME、Narrator、系统高对比度可读性、混合 DPI、多显示器。
5. 真实 tray 物理点击。
6. 缺少 WebView2 Runtime 的真实机器。
7. Windows 10、Defender/SmartScreen 实际拦截。
8. 通过真实 Settings UI 点击切换 log level 的端到端路径（代码与集成测试覆盖）。

## RISKS

- 所有发布产物未签名；新拷贝/首次安装的 host 仍可能被 Defender 扫描约 4.7 s。
- popup 首次加载 CJK 字体后 core+popup private 约 93 MB（预展示约 40 MB；
  ASCII-only 约 55 MB），低于 100 MB 预算但接近上限。
- copy-buffer `cut` 不是 v1 全局动作，UI 明确 disabled。
- 启动时若多个已配置的全局 chord 中任一不可用，核心回退为仅注册 open chord并记录
  日志；UI Apply 路径有完整冲突回滚。
- `GlobalUnlock` 修复改变了真实粘贴路径；T11 已在受控目标上验证文本粘贴，但
  富格式/多选/UAC 仍需人工。

## NEXT

1. T12 逐条复审本报告与 `artifacts/rewrite/after/**-t11*` 证据。
2. 在可交互桌面复核物理热键、真实应用粘贴、UAC、IME、托盘点击。
3. 发布前签名二进制并复测首次安装冷启动。
