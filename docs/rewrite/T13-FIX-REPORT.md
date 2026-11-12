# T13 - 修复 CODE-REVIEW-3 残余问题 + 回归

## STATUS

**完成，R7-1 的 `<2.0 s` 目标作为 DEFERRED 提请编排者接受。**

- **R2-1 FIXED**：启动时逐项注册全局 chord；失败的非 open 绑定从持久化设置中
  清空，open chord 保留/回退并写回；`hotkeys.startup_conflicts` 记录 action、
  chord、reason、effective，Settings 显示 warning。
- **R2-2 FIXED（按编排者 cut-line）**：copy-buffer `cut` 记为 Should/Deferred；
  FEATURE-MATRIX、DECISIONS、EN/ZH 文案、disabled 控件一致；popup/provider
  没有 Cut action。
- **QIS-1 FIXED**：`DITTO_PIPE_SUFFIX`/`DITTO_INSTANCE_ID` 存在但为空、超长、
  含非法字符时拒绝启动（退出码 2），不创建生产 pipe、不信号生产单实例事件。
- **R7-1 FIXED（可见反馈 + 非关键初始化延后）**，但同 binary
  `host→Show <2.0 s` 未达成：实测中位 3,701.9 ms（新拷贝 4,918.7 ms）。逐项
  breakdown、已做优化和残余原因见下；该目标 **DEFERRED**，需编排者接受。
- 全量 fmt/clippy/test/UI/NSIS/Portable/安装卸载通过；未修改 master、未 push、
  未读取真实用户数据、未终止用户 Ditto 或既有 QA 进程。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-fixes3`

## BRANCH

`taohy/rewrite-fixes3`（base `taohy/rewrite-review3` @ `d551a93`；未 push、未合并 master）

## HEAD

修复提交：`ff7c5be1e272ff48f9b8f301f96cb30554d9ac2e`

本报告、最终产物哈希和重新打包后的安装日志由紧随其后的 docs-only 提交加入
（不改变代码/测试/QA 逻辑）。供 T14 复审的代码、测试与 QA 脚本在 `ff7c5be`
中；最终 HEAD 以分支 `taohy/rewrite-fixes3` 为准，T14 应审阅
`ff7c5be..HEAD` 的全部变更（预期只有本报告和证据刷新）。
Portable `BUILD-INFO.json` 的 `git_sha` 为
`ff7c5be1e272ff48f9b8f301f96cb30554d9ac2e`。

## FILES

源码/UI：

- `apps/ditto/src-tauri/src/hotkeys.rs`：`bindings_with_issues`、
  `register_all_collecting`、`reconcile_settings`、`ToggleOpenAlt`。
- `apps/ditto/src-tauri/src/lib.rs`：启动逐项注册/回退/写回、Settings 显示所需
  数据、隐藏 tray 的原生提示、`general.startup_message`、trace 点。
- `apps/ditto/src-tauri/src/commands/mod.rs`：`HotkeyConflict`、
  `hotkeys.startup_conflicts` 映射、Apply 时清除旧冲突。
- `apps/ditto/src-tauri/src/core_service.rs`：retention 首轮延后 10 s。
- `apps/ditto-popup/src/main.rs`、`app.rs`：suffix 校验、冷启动 trace。
- `apps/ditto/ui/src/{types.ts,pages/HotkeysPage.tsx,pages/GeneralPage.tsx,api/browserMock.ts}`。
- `apps/ditto/ui/locales/{en-US.json,zh-CN.json}`。
- `crates/ditto-win/{Cargo.toml,src/lib.rs}`：fail-closed suffix、原生提示窗口。

脚本/文档/证据：

- `tools/qa/{suffix-common.ps1,suffix-validation.ps1,cold-start-feedback.ps1}`（新增）。
- `tools/qa/{cold-start.ps1,hotkey-rebind.ps1,grouped-main.ps1,theme-modes.ps1,ipc-smoke.ps1,paging-check.ps1,popup-hide-sync.ps1,scroll-10k-ipc.ps1,keyboard-first.ps1,install-smoke.ps1}`（suffix 校验/冲突场景）。
- `docs/rewrite/{FEATURE-MATRIX.md,DECISIONS.md,QA-REPORT.md,REVIEW-GUIDE.md,T13-FIX-REPORT.md}`。
- `artifacts/rewrite/after/qa/*-t13.log|png`、`artifacts/rewrite/after/perf/*-t13.csv|log`、
  `artifacts/rewrite/after/build/t13-hashes.txt`（`artifacts/` 被忽略，使用 `git add -f`）。

## BUILD

环境：Windows 11 26200、Rust/Cargo 1.99.1 MSVC、Node 24.13.0/npm 11.6.2、
Tauri CLI 2.12.x、Windows SDK 26100；worktree 起始无 `target/`、`node_modules/`。

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | sidecar 前置 |
| `pwsh scripts/prepare-sidecars.ps1 -Profile debug` | PASS | x64 staging |
| `npm ci` + `npm run build --prefix apps/ditto/ui` | PASS | locales 424/424 keys、tsc、Vite |
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | 无 warning |
| `cargo test --workspace --locked` | PASS | **135 passed，0 failed** |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS | 2m29s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | x64 staging |
| `npx --no-install tauri build` | PASS | NSIS 生成 |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | ZIP 6,899,803 B |
| `tools/qa/install-smoke.ps1` | PASS | NSIS 安装/卸载 + Portable |

UI 主 chunk 508.99 kB，Vite 仍报告 >500 kB warning，不影响构建。

### 产物

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `Ditto.exe` | 11,482,624 | `3C64337FD6936A4B63A3D8069EFB6CFF0F6D6C2113DE6AF338FD71EB7D7EF7F2` | NotSigned |
| `DittoPopup.exe` | 5,023,744 | `791E350C89570014E6DD78A62C885CAC8C8FDF7EF7BD015DE5EA51426F428297` | NotSigned |
| `DittoUacHelper.exe` | 272,896 | `7A9FF425D10A2E413E74D152B0A1FBDF2510C8E4A3EAAD3986C707D101E9A334` | NotSigned |
| `Ditto_0.1.0_x64-setup.exe` | 4,897,584 | `E438AD62CA7ACC5BC7F9CD0DC91B89387037C2543E69B1CAD5E63232FC1F638D` | NotSigned |
| `Ditto-0.1.0-x64-portable.zip` | 6,899,803 | `4F16899AC801786F34C96D383C69269AD3C5D216C4D2D58689AFD729C4750DD4` | 不适用（ZIP） |

Portable `BUILD-INFO.json`：`git_sha=ff7c5be1e272ff48f9b8f301f96cb30554d9ac2e`，
`data_root=<exe_dir>\Data`；README 仍为 ``Press Ctrl+` for the popup``。

## TEST

`cargo test --workspace --locked`：**135 passed，0 failed**（T12 为 128，T13 新增 7）。

T13 新增/加强：

- `ditto-win`：`instance_suffix_validation_is_fail_closed`、
  `native_hint_window_is_created_and_closes`。
- `ditto-tauri` hotkeys：`open_alt_has_its_own_action_for_reconciliation`、
  `tolerant_parse_reports_invalid_and_duplicate_bindings`、
  `reconciliation_clears_only_the_uneffective_bindings`、
  `independent_registration_keeps_working_chords`。
- `ditto-tauri` commands：`startup_conflicts_map_to_settings_values`。
- 既有 `swap_bindings_moves_buffers_and_rolls_back_on_conflict` 继续通过，确认
  运行时 Apply 的原子 swap/rollback 未回归。

## EVIDENCE

### R2-1 Major — 启动热键冲突回写

**FIXED。**

实现：

- `hotkeys::bindings_with_issues` 逐条解析配置：无效/重复项记录为
  `invalid`/`duplicate`，其余保留；运行时 Apply 仍使用严格
  `bindings_from_settings`，任一问题即拒绝并回滚。
- 启动时 `register_all_collecting` 逐项注册；失败的项记录 `in_use`，其余
  chord 继续生效。open chord 失败时按 configured → `Ctrl+`` → `Ctrl+Alt+D`
  → `Ctrl+Alt+Shift+D` 回退。
- `reconcile_settings` 写回：`open_chord` 设为实际生效值；失败的
  `open_alt`/`plain`/copy/paste 字段清空；`hotkeys.startup_conflicts` 保存
  `{action, chord, reason, effective}`。
- Settings GetSettings 映射 `startupConflicts`；HotkeysPage 显示 warning
  （action/chord/reason/effective）。UI 不再显示未注册的 chord。

QA（release，`after/qa/hotkey-rebind-t13.log`）：

```text
CONFIGURED_OPEN_REGISTERED=True
CONFIGURED_COPY_BUFFER_REGISTERED=True
OLD_DEFAULT_FREE_WHILE_CONFIGURED=True
COPY_BUFFER_GLOBAL_ACTION_FILLED_SLOT=True
CHANGED_OPEN_REGISTERED=True
PREVIOUS_OPEN_UNREGISTERED=True
CONFLICT_EFFECTIVE_OPEN_CHORD=Ctrl+`
CONFLICT_FELL_BACK_WITHOUT_SILENT_HOOK=True
STARTUP_COPY_CONFLICT_CLEARED=True
STARTUP_COPY_CONFLICT_OPEN_KEPT=True
STARTUP_COPY_CONFLICT_RECORDED=True
STARTUP_COPY_CONFLICT_OPEN_REGISTERED=True
STARTUP_COPY_CONFLICT_PIPE_ALIVE=True
HOTKEY_REBIND_QA=PASS
```

场景：QA 进程持有 `Ctrl+Alt+1`，配置 open=`Ctrl+Alt+K`、copy-buffer slot 1
copy=`Ctrl+Alt+1`。启动后 GetSettings 返回 copy 字段为空、open 保留
`Ctrl+Alt+K`、`startup_conflicts` 含 `copyBuffer.1.copy / in_use`，pipe 正常。

UI 证据：`after/qa/hotkeys-startup-conflict-t13.png`（真实 React 页面，warning
显示 “Slot 1 Copy (Ctrl+Alt+1): already in use by another application”）。

### R2-2 Major — copy-buffer `cut` 一致性

**FIXED（按编排者裁决：v1 `cut` 为 Should/Deferred）。**

- `FEATURE-MATRIX.md` G5：copy/paste 为 Must；`cut` 明确为 Should/Deferred，
  新增 S14 记录 deferred 理由；A4 同步启动冲突回写。
- `DECISIONS.md`：新增 D-125 记录 cut-line；D-102 更新 fail-closed suffix。
- UI：cut 控件保持 disabled，标签显示 “Coming later”，帮助文案改为
  “Cut is not available in v1; it is planned for a later release.”
  （ZH：“v1 暂不支持剪切；计划在后续版本提供。”）；删除了
  “在弹窗内选中后剪切”的误导说明，并从重复检测中移除 cut（它不是实际绑定）。
- popup/provider：`ActionKind`/`ProviderAction` 无 Cut；`hotkeys.rs` 不注册 cut；
  只有 legacy 数据字段保留。
- 证据：`after/qa/hotkeys-cut-deferred-t13.png`。

### QIS-1 Major — malformed QA suffix 必须 fail closed

**FIXED。**

实现：

- `ditto_win::parse_instance_suffix`：变量存在时，trim 后必须为 1–48 字节
  `[A-Za-z0-9_-]`；为空、空白、超长、含非法字符或非 Unicode 均返回错误。
  不再 filter/truncate，不再静默回退。
- `instance_suffix()` 返回 `Result`；`pipe_name()` 传播为配置错误。
- `Ditto.exe` 在 `run()` 第一行、任何单实例事件/pipe 创建之前校验；失败
  `eprintln!`、写 `DITTO_HOST_TRACE` 的 `invalid-suffix=`、退出码
  `INVALID_SUFFIX_EXIT_CODE=2`。`DittoPopup.exe` 同样校验。
- QA 脚本统一 dot-source `tools/qa/suffix-common.ps1`，生成/读取 suffix 后、
  启动前校验；新增 `suffix-validation.ps1`。

QA（release，`after/qa/suffix-validation-t13.log`）：

```text
SUFFIX_PUNCTUATION_EXIT=2        (!!!)
SUFFIX_WHITESPACE_EXIT=2         (仅空白)
SUFFIX_TOO_LONG_EXIT=2           (49 字符)
SUFFIX_DOT_EXIT=2                (qa.1)
-- 每个非法 case：DB_CREATED=False、TRACE_HAS_ERROR=True
VALID_SUFFIX_PIPE_CREATED=True
PRODUCTION_PIPE_BEFORE=False
PRODUCTION_PIPE_AFTER=False
PRODUCTION_PIPE_UNCHANGED=True
SUFFIX_VALIDATION_QA=PASS
```

合法 suffix 的 ACL/单实例行为不变（`ditto-win` 既有 ACL/slow-client 测试继续通过）。

### R7-1 Major — 冷启动与反馈

**FIXED（可见反馈 + 非关键初始化延后）；`<2.0 s` 目标 DEFERRED，需编排者接受。**

代码变更：

- retention 首轮延后 10 s，避免在 popup 窗口/GL 初始化窗口内争抢 SQLite/磁盘；
  迁移/待处理检查保留在 Storage 打开前（只有 marker 文件 stat，正确性要求，
  实测 setup 总计约 60 ms，不是瓶颈）。
- 保留 early popup spawn / IPC prewarm；新增 host/popup trace 点
  （`hotkey-registered`、`show-request`、`startup-complete`、
  `run-native-enter`、`app-new-enter/exit`、`ipc-prewarm-returned`）。
- 隐藏 tray 时，首次热键且 popup 尚未 Hello：显示原生非激活 hints 窗口
  “Ditto is preparing the popup…”（不抢焦点、不阻塞、6 s 自动关闭）；
  tray 存在时仍更新 tooltip。
- `general.startup_message` 从 disabled 占位改为已实现：非空时启动显示 8 s
  原生提示（可配置、默认空）。GeneralPage 解除 disabled。

同 binary release 分解（`after/qa/cold-start-t13.log`）：

| 样本 | host→Show | host run→popup process-start | popup start→Show | popup start→run_native | app-new | IPC→activate |
|---|---:|---:|---:|---:|---:|---:|
| same #1 | 3,633.5 ms | 1,768.0 ms | 1,793.0 ms | 2.0 ms | 9.0 ms | 85.0 ms |
| same #2 | 3,701.9 ms | 1,822.0 ms | 1,827.0 ms | 1.0 ms | 13.0 ms | 78.0 ms |
| same #3 | 4,107.4 ms | 1,828.0 ms | 2,224.0 ms | 403.0 ms | 12.0 ms | 79.0 ms |
| fresh copy | 4,918.7 ms | 2,160.0 ms | 1,931.0 ms | 1.0 ms | 20.0 ms | 70.0 ms |

中位 3,701.9 ms；新拷贝 4,918.7 ms。host 侧 `run-enter → startup-complete`
约 60 ms（settings 读取、storage、core、hotkey、tray、popup spawn 全部在内）。

残余原因（按证据区分，不把未观测事件归因于 Defender）：

1. **约 1.8 s**：`Command::spawn` 返回后，popup 进程进入 `main()` 前等待
   loader/扫描；host Tauri 初始化（约 1.5–1.8 s）与该等待重叠。fresh copy
   的 host 进程进入 `main()` 额外约 1.2 s。未采集到 Defender 扫描事件，
   因此只报告“未签名进程启动/扫描路径”，不断言具体扫描器。
2. **约 1.7 s**：popup 进程 `main()` 内 eframe/winit 创建窗口与 GL 上下文；
   `run-native-enter → app-new-enter` 占绝大部分，`PopupApp::new` 仅 9–20 ms，
   IPC prewarm 在窗口创建前已完成。
3. **约 0.07–0.09 s**：IPC ready → 激活/首帧。

已做优化：early spawn、IPC prewarm、retention 延后、非关键 setup 在 60 ms 内
完成；代码可控关键路径已很小。要达标需消除上述 1.7–1.8 s 的进程扫描/窗口
初始化之一（例如签名消除首扫，或改为常驻进程提前启动），不属于 T13 可安全
实施的代码改动，故 `<2.0 s` **DEFERRED**。

可见反馈证据（release，`after/qa/cold-start-feedback-t13.log`）：

```text
STARTUP_MESSAGE_HINT_VISIBLE=True
STARTUP_MESSAGE_HINT_SCREENSHOT=True
STARTUP_MESSAGE_HINT_AUTO_CLOSED=True
HOTKEY_PREPARING_HINT_VISIBLE=True
HOTKEY_PREPARING_HINT_SCREENSHOT=True
HOTKEY_PREPARING_HINT_AUTO_CLOSED=True
COLD_START_FEEDBACK_QA=PASS
```

截图 `after/qa/startup-hint-t13.png`、`after/qa/hotkey-preparing-hint-t13.png`
由 PrintWindow 捕获且非黑；窗口类 `DittoNativeHint`，无激活、无阻塞。

### 回归

| 项目 | 结果 | 证据 |
|---|---|---|
| keyboard-first（Show→键入→Enter/Shift+Enter→受控目标） | PASS | `after/qa/keyboard-first-t13.log` |
| grouped main false→Ungrouped | PASS | `after/qa/grouped-main-t13.log` |
| theme dark→HC→system | PASS | `after/qa/theme-modes-t13.log` |
| runtime log level | PASS | `cargo test`（`tests/log_level.rs`） |
| 真实 10k SQLite/IPC scroll 300 帧 | p50 1.468 / p95 2.326 / max 3.743 ms；>16.7 ms 0 | `after/perf/scroll-10k-real-ipc-t13.csv` |
| 10k 分页 | 20 页/10,000 条，477 ms | `after/qa/paging-check-t13.log` |
| popup hide 同步/崩溃清理 | PASS | `after/qa/popup-hide-sync-t13.log` |
| IPC smoke 100 show | first-frame p50 3.571 / p95 5.735 ms | `after/qa/ipc-smoke-t13.log` |
| 迁移 fixture | 3/3 PASS | `after/qa/migration-fixture-t13.log` |
| 存储 10k | insert 1,754 ms；search p95 ≤18.955 ms | `after/perf/storage-10k-t13.log` |
| NSIS 安装/卸载 + Portable | PASS | `after/qa/install-smoke-t13.log` |

### 性能

- 空闲（settings 关闭）：3 次 cold 私有工作集中位 **40,607,744 B**
  （预展示，popup 尚未 Show）；popup 常驻后合计 **93,061,120 B < 100 MB**。
- popup 首帧（100 samples）：p50 **3.571 ms**，p95 **5.735 ms < 50 ms**。
- Settings 打开 **89.2 ms**；二次实例退出 **72.8 ms**。
- 真实 10k 分页/滚动见上；无内存增长断言由 60 s sample 未覆盖，沿用 T11 边界。

## 逐条映射

| ID | 状态 | 摘要 | 证据 |
|---|---|---|---|
| R2-1 | **FIXED** | 启动逐项注册、清空失败绑定、写回 effective open、持久化冲突并在 Settings 显示 | `hotkey-rebind-t13.log`、`hotkeys-startup-conflict-t13.png`、hotkeys 单测 |
| R2-2 | **FIXED（Deferred cut）** | G5/A4/S14、DECISIONS D-125、EN/ZH 文案、disabled cut、无 popup Cut action | `hotkeys-cut-deferred-t13.png`、矩阵/DECISIONS |
| QIS-1 | **FIXED** | malformed suffix 退出码 2、无生产 pipe/DB；合法 suffix 正常 | `suffix-validation-t13.log`、`ditto-win` 单测 |
| R7-1 | **FIXED（反馈/延后）+ `<2.0 s` DEFERRED** | 原生提示、startup message、retention 延后、trace；冷启动仍 3.70 s 中位 | `cold-start-t13.log`、`cold-start-feedback-t13.log`、两张 hint PNG |

## UNVERIFIED

1. 物理热键按键到 Show 的真实 global-shortcut 事件源（QA 使用同一 toggle 路径/IPC）。
2. Notepad/Word/浏览器/Explorer 的真实 text/HTML/RTF/image/files 粘贴。
3. 真实 UAC elevated target、取消、超时、secure desktop。
4. 真实 IME、Narrator、高对比度可读性、混合 DPI、多显示器。
5. 真实 tray 物理点击；原生 hint 在交互式桌面上的目视效果（本次用 PrintWindow
   验证非黑窗口，未由人眼目视）。
6. 缺失 WebView2 Runtime 的机器路径。
7. Windows 10、Defender/SmartScreen 实际拦截行为（未观测到扫描事件，未归因）。
8. 通过真实 Settings UI 点击切换 log level（代码 + 集成测试覆盖）。
9. 远端仓库状态；未 push、未执行网络操作。

## RISKS

- 所有发布产物未签名；新拷贝首启 host→Show 约 4.9 s，首次安装/升级可能更慢。
- 空闲含 popup 约 93 MB，接近 100 MB 预算；CJK 字体加载后的上限仍需关注。
- 启动冲突会清空用户的失败 chord（有意为之，避免 UI 显示未注册值）；用户需在
  Settings 重新分配，warning 会说明原因。
- 原生 hint 是自绘 GDI 窗口；高对比度/混合 DPI 下的可读性未目视验证，当前使用
  系统窗口/文字色与 Segoe UI，自动 6–8 s 关闭。
- `cut` 在 v1 明确不可用；legacy `cut` 数据字段保留但不注册、不显示原值。
- 冷启动 `<2.0 s` 未达成；需要签名或常驻启动策略，可能改变分发/架构决策。

## NEXT

1. T14 独立复审本报告的 R2-1/R2-2/QIS-1/R7-1 与 `artifacts/rewrite/after/**-t13*`。
2. 在交互式桌面复核物理热键、真实应用粘贴、UAC、IME、tray 点击、hint 目视效果。
3. 编排者决定 R7-1 的 `<2.0 s` DEFERRED 是否接受，或另行安排签名/常驻启动优化。
