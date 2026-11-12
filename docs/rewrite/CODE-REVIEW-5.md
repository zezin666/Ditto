# T16 - T15 修复独立复审（最终门）

## STATUS

**结论：可合并。**

T15 自报的 P-1、P-2、Minor-1、Minor-2 和 DOC-2 全部 **CONFIRMED**。
本次在 fresh worktree 中独立完成构建、137 项测试、release 打包、NSIS
安装/卸载、Portable 冒烟，以及 P-1/P-2 的运行复现。没有发现 Blocker 或
Major；只有一项不影响生产的并行 QA 隔离 Minor，未发现它影响最终合并。

冷启动结论为 **DEFERRED-ACCEPTED**。正式 PF-6 的 core-ready 峰值约为
1.97 s，仍在 max < 2 s 内；用户可见的 host→popup show 仍为 3.41 s
（同一 binary 中位）和 4.81 s（新拷贝首次），但首次启动是一次性成本，
热键按下后约 24 ms 即显示非激活提示，popup ready 后提示关闭，prewarm
完成后的首帧 p95 为 3.49 ms。该取舍不改变本次 merge verdict。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-review5`

## BRANCH

`taohy/rewrite-review5`

任务指定 base：`taohy/rewrite-fixes4` @ `550782fc...`。

## HEAD

工作树 `HEAD` 指向 `refs/heads/taohy/rewrite-review5`；该 ref 的只读值为：

```text
550782fec65cb3c621436f009fc359d84ea881a1
```

仓库约束禁止 agent 自行执行 git 命令，因此本审查只读取 `.git` 指针、ref 文件
和文件 mtime/hash，没有执行 `git status`、`git diff`、`git push`、merge 或
网络操作。

## FILES

- `docs/rewrite/CODE-REVIEW-5.md`：本次唯一人工编辑文件。

构建生成或更新的 `target/`、UI `node_modules/`、`dist/`、sidecar staging
和临时 QA 根目录均为生成产物。没有修改源码、配置、测试、`legacy/` 或
`LICENSE`。

审查期间一个 QA 脚本向旧的 mock CSV 追加了数据；已用同一 base worktree
中的原文件恢复，恢复后 SHA-256 一致，未把该生成改动留在工作区。

## BUILD

环境：Windows 11 26200，Rust/Cargo 1.99.1 MSVC，Node 24.13.0，
npm 11.6.2，PowerShell 7.6.6。

fresh worktree 开始时没有 `target/`、UI `node_modules/`、UI `dist/`、
sidecar staging 或 `apps/ditto/src-tauri/binaries/`。

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | debug sidecar 前置 |
| `pwsh scripts/prepare-sidecars.ps1 -Profile debug` | PASS | x64 staging |
| `npm ci --prefix apps/ditto/ui` | PASS | 181 packages，0 vulnerabilities |
| `npm ci --prefix apps/ditto` | PASS | 2 packages，0 vulnerabilities |
| `npm run build --prefix apps/ditto/ui` | PASS | 424 shared + 77 popup keys；tsc/Vite PASS |
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | 无 warning |
| `cargo test --workspace --locked` | PASS | **137 passed，0 failed，1 ignored** |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS | release |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | x64 staging |
| `npx --no-install tauri build` | PASS | NSIS 生成 |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | Portable ZIP |
| `tools/qa/install-smoke.ps1` | PASS | NSIS 安装/卸载 + Portable |

UI 主 chunk 仍为 509.16 kB，Vite 保留 >500 kB warning，不影响构建。

### 独立产物

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `target/release/Ditto.exe` | 11,487,744 | `8603BC3DE3904CCD6C6B98BA54685F8377DF7491DCD69D0242A79F85E5B6E1D7` | NotSigned |
| `target/release/DittoPopup.exe` | 5,023,744 | `47E356EB33F4B8E637435D7F42BEF5C1EDBD3201A792A51A06165745445342E6` | NotSigned |
| `target/release/DittoUacHelper.exe` | 272,896 | `16C919E4A1BA519537B2EB6BFD70A8739F659F3DB1A0A05B988998BA7CE45792` | NotSigned |
| `target/release/bundle/nsis/Ditto_0.1.0_x64-setup.exe` | 4,899,292 | `BAA21F5F5BF4ACACAF423EBF6749951583EC1D0F21BA3A91E3F926A4CD3B6458` | NotSigned |
| `target/release/bundle/portable/Ditto-0.1.0-x64-portable.zip` | 6,902,013 | `ABC961CAFA1EE08D5C02DF3C82CB658D8DC00C98F7D243F8C8BB5D2D4500DCD8` | 不适用 |

Portable `BUILD-INFO.json`：`git_sha=550782fec65cb3c621436f009fc359d84ea881a1`，
`data_root=<exe_dir>\Data`，`signature=unsigned`。README 中的 hotkey
文本为单反引号 ``Press Ctrl+` for the popup``。

## TEST

### 全量测试

`cargo test --workspace --locked`：**137 passed，0 failed，1 ignored**。

- `ditto-clipboard`: 11
- `ditto-ipc`: 6
- `ditto-migration`: 11 unit + 3 legacy fixture
- `ditto-model`: 2
- `ditto-paste`: 20
- `ditto-popup-app`: 21
- `ditto-storage`: 16 unit + 10k bench + concurrency
- `ditto-tauri-lib`: 31 unit + runtime log integration
- `ditto-uac-helper`: 5
- `ditto-win`: 8

忽略项是显式 migration 10k benchmark；本次没有把它冒充为已执行。

### 独立补充

| 场景 | 结果 |
|---|---|
| `cargo test -p ditto-storage --test bench_10k --release --locked -- --nocapture` | PASS；insert 1,718.965 ms，quick-paste p95 16.945 ms，其余 search p95 <= 1.097 ms |
| `cargo test -p ditto-migration --test legacy_fixture --locked -- --nocapture` | PASS；3/3 |
| real 10k SQLite + IPC scroll | PASS；p50 0.669 ms，p95 1.147 ms，max 1.933 ms，0 帧 >16.7 ms |
| 10k cursor paging | PASS；20 页 / 10,000 条 / 535.4 ms |
| IPC smoke 100 show | first-frame p50 2,297 us，p95 3,490 us |
| idle private（core+popup） | 93,319,168 B；3 次 cold samples 中位 40,407,040 B，设置关闭 |
| Settings open / second instance exit | 48.4 ms / 33.4 ms |

## EVIDENCE

### P-1 pending import/migration 先于启动设置

**CONFIRMED。**

源码顺序明确：

1. `run()` 先校验 suffix、claim 单实例，再在读取启动设置前调用
   `pending::apply_staged(&root.path)`：
   `apps/ditto/src-tauri/src/lib.rs:233-250`。
2. `apply_staged` 依次应用 migration 和 import：
   `apps/ditto/src-tauri/src/pending.rs:92-121`。
3. `setup()` 在打开 Storage 后从新的 data root 重读 settings，再构造
   hotkey/tray/language/startup message 并使用该快照 reconcile：
   `apps/ditto/src-tauri/src/lib.rs:351-431`。
4. 导入/迁移成功后才删除 marker，旧快照不参与正常路径 reconcile：
   `apps/ditto/src-tauri/src/pending.rs:154-207`。

独立 import QA：

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

同一独立 trace 中，`pending-applied: data import/restore applied` 先于
`startup-settings-loaded`，之后再出现 `hotkey-active=Ctrl+Alt+Q` 和
`tray-hidden`。因此不是“导入后下一轮才生效”，且旧 `Ctrl+Alt+K` 没有被
写回。

另做了一次合成 legacy migration 启动：

```text
host:run-enter
host:pending-applied: legacy migration applied
host:startup-settings-loaded
host:hotkey-active=Ctrl+`
host:tray-hidden
host:startup-complete

clipboard.max_entries=123
general.show_tray_icon=false
general.language="zh-CN"
DB_CLIPS=1
MIGRATION_STARTUP_QA=PASS
```

该路径使用临时 `DITTO_DATA_DIR`、唯一合法 suffix、合成旧 DB 和合成
`Ditto.Settings`，没有读取真实用户数据库或剪贴板历史。

### P-2 PopupWindowReady 覆盖 prewarm 到窗口 ready 空窗

**CONFIRMED。**

- `prewarm_ipc()` 只完成 Hello，不设置 ready：
  `apps/ditto-popup/src/app.rs:3288-3306`。
- `PopupApp::new` 在 eframe 提供 native window 且完成 popup HWND 配置后
  才启动 IPC reader：
  `apps/ditto-popup/src/app.rs:294-332`。
- reader 在连接后发送 `PopupWindowReady`，并先等待 core 的
  `GetPopupVisibility` 再进入事件循环：
  `apps/ditto-popup/src/app.rs:3435-3455`。
- `PipeServer` 用独立的 `window_ready` 状态区分 IPC client 与可显示窗口；
  popup 断开时同时清除 visible 和 window_ready：
  `crates/ditto-win/src/lib.rs:828-948`、`:1244-1255`。
- `PopupWindowReady` 的 dispatcher 路径：
  `crates/ditto-win/src/lib.rs:1343-1349`。
- 隐藏 tray 时，core 在 ready 前显示 no-activate native hint，并在 ready
  后关闭：`apps/ditto/src-tauri/src/lib.rs:678-736`。

独立 release QA：

```text
PREWARM_SEEN=True
HINT_VISIBLE_AFTER_SIGNAL=True
FOREGROUND_IS_HINT=False
PREPARING_HINT_SCREENSHOT=True
POPUP_WINDOW_READY_SEEN=True
HINT_VISIBLE_BEFORE_WINDOW_READY=True
HINT_DISMISSED_AFTER_WINDOW_READY=True
POPUP_ACTIVATION_SEEN=True
SIGNAL_TO_HINT_MS=24
SIGNAL_TO_WINDOW_READY_MS=1555
SIGNAL_TO_ACTIVATION_MS=1600
POPUP_READY_FEEDBACK_QA=PASS
```

PrintWindow 截图肉眼可见 “Ditto is preparing the popup...”；hint 的
foreground check 为 false。独立值与 T15 的 29 ms / 1,508 ms /
1,558 ms 同量级，属于机器时序差异，不是不可复现。

补充测试：

`popup_window_readiness_follows_the_ready_message_not_the_handshake`
验证 Hello 后 ready 仍为 false，收到 `PopupWindowReady` 后才为 true，
client 断开后恢复 false；`native_hint_window_is_created_and_closes`
验证提前 dismiss 会关闭 hint。

### T14 残余逐条复核

| ID | 状态 | 独立结果 |
|---|---|---|
| P-1 | **CONFIRMED** | import 和 migration 都在 settings 读取前应用；导入值未被旧快照覆盖；失败/无 marker 路径由 pending 单元测试覆盖 |
| P-2 | **CONFIRMED** | hello/prewarm 不再等同于 ready；hint 非激活、ready 前可见、ready 后关闭；signal→ready/activation 数据可重复 |
| Minor-1 | **CONFIRMED** | `popup-hide-sync.ps1` 用 `$DittoExe` 目录和启动时间筛选 popup；fresh worktree 独立得到 outside click false、next show true、crash false |
| Minor-2 | **CONFIRMED** | `QA-REPORT.md` T13=135、T15=137；T15 证据索引存在，和实际测试数一致 |
| DOC-2 | **CONFIRMED** | `FEATURE-MATRIX.md:54` 明确 C11 首次 retention 延后 10 s，之后每 5 分钟 |

### 回归与安全抽查

| 项目 | 状态 | 独立证据 |
|---|---|---|
| R-1 keyboard-first | **CONFIRMED** | Show 后无需鼠标输入，Enter 和 Shift+Enter 都落到受控 EDIT；`KEYBOARD_FIRST_QA=PASS` |
| R2-1 startup hotkey conflict | **CONFIRMED** | open fallback、失败的非 open chord 清空、`startup_conflicts` 记录和 pipe 存活均 PASS |
| R2-2 copy-buffer cut | **CONFIRMED（DEFERRED）** | `FEATURE-MATRIX.md:118,194`、`DECISIONS.md:61` 与 `HotkeysPage.tsx:210-225` 一致；无 global/popup cut action |
| QIS-1 suffix fail-closed | **CONFIRMED** | punctuation/whitespace/overlong/dot 4 类均 exit 2、无 DB、无生产 pipe；合法 suffix 创建隔离 pipe |
| R-3 grouped main | **CONFIRMED** | false→Ungrouped，true→All；`GROUPED_MAIN_QA=PASS` |
| R-4 theme/HC | **CONFIRMED** | dark→HighContrast→system(Light)，HC 使用系统 palette |
| R-5 runtime log level | **CONFIRMED** | 全量 `log_level` integration test PASS |
| R-6 ANSI/OEM round-trip | **CONFIRMED** | `ditto-clipboard` 11 tests 包含 CP936/CP1252/CP437、NUL 和 hash/preview/FTS |
| R-8 real 10k IPC scroll | **CONFIRMED** | 300 帧，p95 1.147 ms，0 帧 >16.7 ms，10,000 行真实 SQLite |
| R-9 Portable README | **CONFIRMED** | 单反引号 ``Ctrl+` ``，无 Markdown 双反引号泄漏 |
| R-10 docs consistency | **CONFIRMED** | T13/T15 数目、C11 retention、suffix、cut deferral 一致 |
| GlobalUnlock | **CONFIRMED** | unit tests 通过；keyboard-first 实际文本粘贴落地；源码按 FALSE+NO_ERROR 语义处理 |
| popup hide/crash sync | **CONFIRMED** | outside click false、next Show true、popup crash false |

`GlobalUnlock` 的独立源码位置：
`crates/ditto-paste/src/clipboard.rs:136-163`。

### T15 证据审计

- `artifacts/rewrite/after/qa/*-t15*` 和
  `artifacts/rewrite/after/perf/*-t15*` 均存在且内容与 T15 报告一致。
- 独立重算 T15 scroll CSV：300 帧、0 帧 >16.7 ms、max 2.921 ms；
  T15 报告中的 p50/p95 与脚本统计方法略有差异，但结论和 max 一致。
- T15 的 `preparing-hint-t15.png` 和本次独立截图都非黑，且显示 preparing
  文案。
- T15 的最终 artifact hash 对应其报告中的代码提交 `5ffc129`；本次
  `BUILD-INFO` 的 `git_sha` 是任务指定 worktree ref `550782f`，因此
  binary hash 不应作为跨 worktree 的逐字节相等断言。

## UNVERIFIED

1. 物理 global-shortcut 按键到 Show 的真实事件源；本次运行走同一 toggle
   event，未注入物理键盘。
2. Notepad/Word/浏览器/Explorer 的真实 text/HTML/RTF/image/files 粘贴；
   本次 `keyboard-first` 只验证受控 Win32 EDIT 的文本落地。
3. 真实 UAC elevated target、取消、超时、secure desktop。
4. 真实 IME、Narrator、高对比度可读性、混合 DPI、多显示器。
5. 真实 tray 物理点击，以及 native hint 的人眼视觉检查。
6. 缺失 WebView2 Runtime 的真实机器路径。
7. Windows 10、Defender/SmartScreen 实际拦截行为；所有产物未签名。
8. 60 s 常驻 popup 无内存增长断言；本次只测 idle 和 100 次 Show 的工作集。
9. 旧/新 popup 与 core 混跑时的 `PopupWindowReady` 兼容性；协议消息是
   additive，但没有构造混合版本运行。
10. 远端 push 状态；仓库约束禁止 git/network 操作，无法独立确认远端。

## RISKS

- 所有可执行文件未签名；新拷贝首次启动仍可能触发 Defender/SmartScreen
  扫描。本次没有修改任何安全设置。
- 用户可见 host→popup show 仍约 3.41 s，新拷贝约 4.81 s；P-2 的
  no-activate hint 覆盖了可见反馈空窗，但用户仍需等待。
- `PF-6` 的 core-ready 独立观测约为 1.60 s（same binary）和 1.97 s
  （fresh copy），峰值贴近 2 s；机器温度、AV 扫描和首次登录会影响边界。
- popup 加载 CJK 字体后的 private working set 约 93.3 MB，距 100 MB
  预算只有约 6.7 MB 余量。
- 启动时非 open chord 冲突会按既定设计清空失败的绑定并显示 warning；
  这是 T13 产品决策，不是 T15 回归。
- `dismiss_native_hint()` 通过全局窗口类查找 hint。正常单实例不受影响，
  但两个 suffix-isolated QA core 同时运行时，理论上可能互相关闭对方的
  hint；这是 QA 并行隔离风险，不阻断本次 merge。
- `PopupWindowReady` 是 additive IPC 消息；当前 core/popup 成对发布正常，
  混合旧 popup 不会发该消息，升级/并发运行场景仍应保持成对替换。

## NEXT

1. 编排者可将本报告作为 T16 最终审查结果；本报告没有修改源码，因此
   不需要 T17 代码修复才能合并。
2. 若要并行运行多个 suffix-isolated QA 实例，可选地做一次 T17 小修，
   让 native hint 的查找带上 core/instance 标识；该事项不影响正常单实例
   产品验收。
3. 正式发布前仍应在交互式桌面完成物理热键、真实应用粘贴、UAC、IME、
   DPI/tray 点击和 Defender 行为的人工矩阵。

## VERDICT

**可合并**

P-1/P-2/Minor-1/Minor-2/DOC-2 均为 **CONFIRMED**。冷启动
**DEFERRED-ACCEPTED**，不影响本次 merge。
