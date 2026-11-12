# T15 - 修复 CODE-REVIEW-4 残余问题 + 回归

## STATUS

**完成。P-1、P-2、Minor-1、Minor-2 全部 FIXED；附带修复 DOC-2。**

- **P-1 FIXED**：pending import/restore/migration 现在在读取启动设置之前应用；
  `setup` 打开 Storage 后再次从新的 data root 读取设置，并用该快照重建
  hotkey/tray/language/startup message 和 reconcile 写回。旧快照不再能覆盖
  导入的清单元数据。
- **P-2 FIXED**：新增 `PopupWindowReady` IPC 请求和
  `PipeServer::popup_window_ready`。popup 在 eframe 窗口创建后发送；core 在
  window 未 ready 时始终显示非激活 preparing 提示（即使 tray 隐藏），ready 后
  立即关闭。IPC prewarm 不再被当作“可显示”。
- **Minor-1 FIXED**：`popup-hide-sync.ps1` 改为基于当前 `$DittoExe` 目录和启动
  时间筛选 popup 进程，fresh worktree 可直接运行。
- **Minor-2 FIXED**：`QA-REPORT.md` 测试数更新为 T13=135 / T15=137，并补充
  T15 证据索引。
- **DOC-2 FIXED（附带）**：`FEATURE-MATRIX.md` C11 明确首轮 retention 延后 10 s。
- 全量 fmt/clippy/test、UI、NSIS、Portable、安装/卸载和主要运行时 QA 通过；
  未修改 master、未 push、未读取真实用户数据、未终止用户 `Ditto.exe`。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-fixes4`

## BRANCH

`taohy/rewrite-fixes4`（base `taohy/rewrite-review4` @
`65b9434`；未 push、未合并 master）

## HEAD

修复提交：`5ffc1296b5651ecd99a694aba4d3a64ee87c21a4`

本报告、最终产物哈希和证据刷新由紧随其后的 docs-only 提交加入（不改变
代码/测试/QA 逻辑）。代码、测试与 QA 脚本以上述修复提交为准；T16 应审阅
`65b9434..HEAD` 的全部变更（预期为本报告、QA 索引和证据）。

## FILES

源码/协议：

- `apps/ditto/src-tauri/src/lib.rs`：`run()` 先 claim 单实例、再
  `pending::apply_staged`、最后读取 settings；`setup` 重新读取 settings 并
  重建 hotkey/tray/language/startup message；`toggle_popup` 使用
  `popup_window_ready()`；新增 `hotkey-active` 和 `tray-created/tray-hidden`
  trace。
- `apps/ditto/src-tauri/src/pending.rs`：新增
  `staged_import_replaces_settings_before_the_next_read` 回归测试。
- `apps/ditto/src-tauri/src/core_service.rs`：`PopupWindowReady` 的
  request-method/兜底匹配。
- `crates/ditto-ipc/src/lib.rs`：新增 `Request::PopupWindowReady`。
- `crates/ditto-win/src/lib.rs`：`PopupPresence`、`popup_window_ready` 状态、
  popup 断开时重置、`dismiss_native_hint`、`show_native_hint -> Result<bool>`，
  以及 `popup_window_readiness_follows_the_ready_message_not_the_handshake`
  和 hint dismiss 测试。
- `apps/ditto-popup/src/app.rs`：`connect_and_read` 在窗口创建后发送
  `PopupWindowReady`，写 `window-ready-ack` trace。

脚本/文档/证据：

- `tools/qa/pending-import-startup.ps1`（新增）。
- `tools/qa/popup-ready-feedback.ps1`（新增）。
- `tools/qa/popup-hide-sync.ps1`（动态 worktree 路径，Minor-1）。
- `docs/rewrite/{T15-FIX-REPORT.md,QA-REPORT.md,REVIEW-GUIDE.md,FEATURE-MATRIX.md}`。
- `artifacts/rewrite/after/qa/*-t15.log|png`、`artifacts/rewrite/after/perf/*-t15.csv|log`
  （`artifacts/` 被忽略，使用 `git add -f`）。

## BUILD

环境：Windows 11 26200、Rust/Cargo 1.99.1 MSVC、Node 24 / npm 11、
PowerShell 7、Tauri CLI 2.12.x、Windows SDK 26100。

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | sidecar 前置 |
| `pwsh scripts/prepare-sidecars.ps1 -Profile debug` | PASS | x64 staging |
| `npm ci --prefix apps/ditto/ui` + `npm run build --prefix apps/ditto/ui` | PASS | locales、tsc、Vite |
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | 无 warning |
| `cargo test --workspace --locked` | PASS | **137 passed，0 failed，1 ignored** |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS | release |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | x64 staging |
| `npm ci --prefix apps/ditto` + `npx --no-install tauri build` | PASS | NSIS 生成 |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | ZIP 生成 |
| `tools/qa/install-smoke.ps1` | PASS | NSIS 安装/卸载 + Portable |

UI 主 chunk 仍为 509.16 kB，Vite 保留 >500 kB warning，不影响构建。

### 独立产物

`5ffc129` 提交后重新打包的最终产物：

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `Ditto.exe` | 11,487,744 | `039BBA6D15FED63F83083081197BA4020117C16D85F21CB30D74E84717472725` | NotSigned |
| `DittoPopup.exe` | 5,023,744 | `FB1F50642AC20610FBF7771701C6DB3FA55F63780489B7D7DB69CD3BCF381168` | NotSigned |
| `DittoUacHelper.exe` | 272,896 | `1F500BB67554E51D904081D43FBAB857F0127C0898D54F25E7CE705FA63BFA4B` | NotSigned |
| `Ditto_0.1.0_x64-setup.exe` | 4,899,808 | `2DDA969A7AEFDF51AC3B19625799E9A22878DF7B6F0E373C214AFF614B9AAA98` | NotSigned |
| `Ditto-0.1.0-x64-portable.zip` | 6,902,014 | `EDD6DB4D0F969CA04EC7D7A06F9FF7FE6808B988E58E09EE3CE7EED12B1D6800` | 不适用 |

Portable `BUILD-INFO.json`：`git_sha=5ffc1296b5651ecd99a694aba4d3a64ee87c21a4`，
`data_root=<exe_dir>\Data`，`signature=unsigned`。

## TEST

`cargo test --workspace --locked`：**137 passed，0 failed，1 ignored**
（explicit migration 10k benchmark）。

T15 新增/加强：

- `ditto-win`：`popup_window_readiness_follows_the_ready_message_not_the_handshake`
  —— Hello 后 window ready 仍为 false，收到 `PopupWindowReady` 后才为 true，
  popup 断开后重置。
- `ditto-win`：`native_hint_window_is_created_and_closes` 扩展为覆盖
  `dismiss_native_hint` 提前关闭。
- `ditto-tauri`：`pending::tests::staged_import_replaces_settings_before_the_next_read`
  —— 合成导出包写入目标根后，重新打开 Storage 读到导入值而非旧值。

各 crate：`ditto-clipboard` 11、`ditto-ipc` 6、`ditto-migration` 11+3、
`ditto-model` 2、`ditto-paste` 20、`ditto-popup-app` 21、`ditto-storage`
16+1+1、`ditto-tauri-lib` 31+1、`ditto-uac-helper` 5、`ditto-win` 8。

## EVIDENCE

### P-1 Major — pending import 首启设置被旧快照覆盖

**FIXED。**

实现：

1. `run()` 在 `claim_single_instance()` 之后、`load_startup_settings()` 之前
   调用 `pending::apply_staged()`。只有主实例能应用暂存操作。
2. `setup` 在 `Storage::open_data_root` 之后重新读取
   `storage.get_settings()`，用该快照计算 `configured_open`、tray、language、
   startup message、log level，并据此注册热键和
   `reconcile_settings` 写回。
3. 旧 `startup.values` 仅在重新读取失败时作为 fallback；正常路径不会写回。

release QA（`after/qa/pending-import-startup-t15.log`）：

```text
STARTUP_COMPLETE=True
OPEN_CHORD_AFTER_IMPORT=Ctrl+Alt+Q
STARTUP_MESSAGE_AFTER_IMPORT=DITTO-T15-NEW-MESSAGE
LANGUAGE_AFTER_IMPORT=zh-CN
SHOW_TRAY_AFTER_IMPORT=False
MAX_ENTRIES_AFTER_IMPORT=17
IMPORTED_HOTKEY_Q_REGISTERED=True
OLD_HOTKEY_K_FREE=True
HOST_TRACE_ACTIVE_OPEN_Q=True
HOST_TRACE_ACTIVE_OLD_OPEN_K=False
HOST_TRACE_TRAY_HIDDEN=True
HOST_TRACE_TRAY_CREATED=False
POPUP_LANGUAGE_ZH=True
STARTUP_MESSAGE_HINT_VISIBLE=True
PENDING_IMPORT_STARTUP_QA=PASS
```

场景：目标根含 `open_chord=Ctrl+Alt+K`、空 startup message、tray=true、
`max_entries=5`；导入包含 `open_chord=Ctrl+Alt+Q`、`NEW-MESSAGE`、`zh-CN`、
tray=false、`max_entries=17`。启动后 GetSettings 返回导入值，Q 被注册而 K
空闲，popup 以 `--lang=zh-CN` 启动，旧值未写回。

host trace（`after/qa/pending-import-t15-host-trace.log`）显示
`pending-applied: data import/restore applied` 早于
`startup-settings-loaded`，随后 `hotkey-active=Ctrl+Alt+Q`、`tray-hidden`。

### P-2 Major — 隐藏 tray 冷启动反馈空窗

**FIXED。**

实现：

- `Request::PopupWindowReady`：popup 在 eframe 窗口创建后、`connect_and_read`
  开始时发送；`PipeServer` 维护独立的 `window_ready` 原子状态，popup 进程
  断开时清零。
- `toggle_popup`：只要 `!popup_window_ready()` 就显示 preparing 提示（tray
  隐藏时用原生非激活 hint），与 IPC `popup_client_count` 无关；ready 后
  watcher 调用 `dismiss_native_hint()` 立即关闭并恢复 tooltip。
- hint 仍是 `WS_EX_NOACTIVATE`，不抢焦点、不阻塞键盘路径。

release QA（`after/qa/popup-ready-feedback-t15.log`）：

```text
PREWARM_SEEN=True
HINT_VISIBLE_AFTER_SIGNAL=True
FOREGROUND_IS_HINT=False
PREPARING_HINT_SCREENSHOT=True
POPUP_WINDOW_READY_SEEN=True
HINT_VISIBLE_BEFORE_WINDOW_READY=True
HINT_DISMISSED_AFTER_WINDOW_READY=True
POPUP_ACTIVATION_SEEN=True
SIGNAL_TO_HINT_MS=29
SIGNAL_TO_WINDOW_READY_MS=1508
SIGNAL_TO_ACTIVATION_MS=1558
POPUP_READY_FEEDBACK_QA=PASS
```

即：toggle 信号后 29 ms 出现提示，1,508 ms 后窗口才 ready，1,558 ms 后
activate；提示在 ready 前可见、ready 后消失，且未获得前台焦点。截图为
PrintWindow 捕获的 `after/qa/preparing-hint-t15.png`（非黑）。
trace：`after/qa/popup-ready-t15-{host,popup,hint}-trace.log`。

### Minor-1 — `popup-hide-sync.ps1` 硬编码 worktree 路径

**FIXED。**

脚本改为：

- `$binaryDir = Split-Path -Parent $DittoExe`
- 只处理 `Path -eq <binaryDir>\DittoPopup.exe` 且 `StartTime >= 脚本启动时间`
  的进程。

fresh `rewrite-fixes4` worktree release 运行
（`after/qa/popup-hide-sync-t15.log`）：

```text
POPUP_VISIBLE_AFTER_OUTSIDE_CLICK=False
POPUP_VISIBLE_AFTER_NEXT_SHOW=True
POPUP_VISIBLE_AFTER_CRASH=False
POPUP_HIDE_SYNC_RESULT=completed
```

### Minor-2 — QA-REPORT 测试数

**FIXED。**

- T13 段落由 `131` 改为 `135`（T13 实测）。
- 新增 T15 段落：137 passed / 0 failed / 1 ignored，并索引
  `after/qa/*-t15*`、`after/perf/*-t15*`。

### DOC-2 — retention 文档语义（附带）

**FIXED。** `FEATURE-MATRIX.md` C11 改为“首轮 retention 延后 10 s，之后每
5 分钟”，与 `MAINTENANCE_INITIAL_DELAY` 一致。

### 回归

| 项目 | 结果 | 证据 |
|---|---|---|
| keyboard-first（Show→键入→Enter/Shift+Enter→受控目标） | PASS；真实文本落地 | `after/qa/keyboard-first-t15.log` |
| startup hotkey conflict / Apply rollback | PASS | `after/qa/hotkey-rebind-t15.log` |
| grouped main false→Ungrouped | PASS | `after/qa/grouped-main-t15.log` |
| theme dark→HC→system | PASS | `after/qa/theme-modes-t15.log` |
| runtime log level | PASS | `cargo test`（`log_level` 集成测试） |
| suffix fail-closed / 合法 suffix | PASS；生产 pipe 不变 | `after/qa/suffix-validation-t15.log` |
| 真实 10k SQLite/IPC scroll（300 帧） | p50 0.665 / p95 1.235 / max 2.921 ms；>16.7 ms 0 | `after/perf/scroll-10k-real-ipc-t15.csv` |
| 10k cursor paging | 20 页 / 10,000 条 / 499.5 ms | `after/qa/paging-check-t15.log` |
| popup hide/crash sync | PASS | `after/qa/popup-hide-sync-t15.log` |
| IPC smoke 100 show | first-frame p50 2.046 / p95 3.005 ms | `after/qa/ipc-smoke-t15.log` |
| 迁移 fixture | 3/3 PASS | `after/qa/migration-fixture-t15.log` |
| 存储 10k | insert 1,766.3 ms；quick-paste p95 18.078 ms | `after/perf/storage-10k-t15.log` |
| NSIS 安装/卸载 + Portable | PASS | `after/qa/install-smoke-t15.log` |
| 冷启动反馈（startup message / popup 缺失回退） | PASS | `after/qa/cold-start-feedback-t15.log` + 两张 PNG |

### 性能

- 空闲 private（3 次）：93,155,328 / 93,282,304 / 93,282,304 B；中位
  **93,282,304 B < 100 MB**。
- popup 首帧（100 samples，IPC smoke）：p50 **2.046 ms**，p95
  **3.005 ms < 50 ms**。
- Settings 打开 **50.2 ms**；二次实例退出 **51.7 ms**。
- 冷启动同 binary 3 次：3,732.6 / 3,273.6 / 3,333.5 ms，中位
  **3,333.5 ms**；新拷贝 **4,459.2 ms**。`<2.0 s` 仍未达成，但 P-2 已保证
  该窗口内有可见、非激活的反馈。
- 真实 10k 分页/滚动与存储 10k 见上；60 s 无内存增长断言仍未覆盖。

## UNVERIFIED

1. 物理 global-shortcut 按键到 Show 的真实事件源；QA 直接 signal 同一 toggle
   事件。
2. Notepad/Word/浏览器/Explorer 的真实 text/HTML/RTF/image/files 粘贴。
3. 真实 UAC elevated target、取消、超时、secure desktop。
4. 真实 IME、Narrator、高对比度可读性、混合 DPI、多显示器。
5. 真实 tray 物理点击；hint 的人眼目视效果（本次仍用 PrintWindow 非黑）。
6. 缺失 WebView2 Runtime 的真实机器路径。
7. Windows 10、Defender/SmartScreen 实际拦截行为；所有产物均未签名。
8. 60 s 常驻 popup 无内存增长断言。
9. 远端 push 状态；未执行 push/网络操作。

## RISKS

- 所有可执行产物未签名；新拷贝首启约 4.56 s，第一次安装/升级可能更慢。
- P-2 的 ready 信号定义为“eframe 窗口创建后”，比首帧略早；ready 后
  hint 关闭、popup 通常在同一次事件循环内显示。真实激活仍取决于 Win32
  `SetForegroundWindow` 的成功（QA 中 activation 在 ready 后约 50 ms）。
- 常驻 popup 私有工作集约 93 MB，接近 100 MB 预算；CJK 字体加载后的上限仍需
  持续关注。
- 启动冲突仍会清空失败的非 open chord（T13 设计），用户需在 Settings 重新
  分配；warning 会说明原因。
- `<2.0 s` 冷启动目标仍未达成；需要签名或常驻启动策略，属于分发/架构决策。

## NEXT

1. T16 独立复审本报告的 P-1/P-2/Minor-1/Minor-2、代码改动和
   `artifacts/rewrite/after/**-t15*`。
2. 在交互式桌面复核物理热键、真实应用粘贴、UAC、IME、tray 点击和 hint 目视
   效果。
3. 编排者决定 `<2.0 s` 冷启动目标是否继续 DEFERRED，或安排签名/常驻启动
   优化。
