# T14 - T13 修复最终独立复审

## STATUS

**结论：需修复。**

T14 在独立 fresh worktree 中完成构建、135 项测试、UI、NSIS、Portable、
安装/卸载和主要运行时 QA。R2-1、R2-2、QIS-1 的 T13 修复成立，
keyboard-first、10k 滚动/分页、IPC smoke、存储 10k、主题、分组和
`GlobalUnlock` 真实文本粘贴也未发现回归。

仍有两个需要修复的 Major：

- **P-1（Major，T13 引入的设置覆盖风险）**：启动设置快照先于 pending
  import/migration 读取；T13 随后用该旧快照做 hotkey reconcile/write-back。
  独立复现中，导入包内的 `open_chord=Ctrl+Alt+Q` 被旧值
  `Ctrl+Alt+K` 覆盖，启动提示仍显示旧库的 `OLD-MESSAGE`，而数据库已经是
  导入包的 `NEW-MESSAGE`。
- **P-2（Major，R7-1 PARTIAL）**：隐藏 tray 的 preparing 提示只在
  `popup_client_count==0` 时出现。popup 在 eframe 初始化前已完成 IPC Hello，
  因此真实的约 1.7 s 窗口初始化期间按热键没有反馈。独立复测
  `HINT_VISIBLE_AFTER_SIGNAL=False`，1,724.5 ms 后 popup 才激活。

另有 QA/文档 Minor：`popup-hide-sync.ps1` 仍硬编码
`rewrite-fixes\target`，fresh worktree 无法按 T13 证据运行；
`QA-REPORT.md` 的测试数仍写 131，实际为 135。

本次没有读取真实 `%APPDATA%\Ditto`、`%LOCALAPPDATA%\Ditto` 或剪贴板历史；
所有运行 QA 使用临时 `DITTO_DATA_DIR` 和唯一合法 `DITTO_PIPE_SUFFIX`。
用户 `D:\Programs\Ditto\Ditto.exe` 的 PID 35024 审查前后保持不变。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-review4`

## BRANCH

`taohy/rewrite-review4`

基线 `taohy/rewrite-fixes3` @ `f8d70912324d67284ea93318d397a1379d48d7b4`。

## HEAD

`f8d70912324d67284ea93318d397a1379d48d7b4`

该值由 worktree `.git` 指针、worktree `HEAD` 和
`refs/heads/taohy/rewrite-review4` 只读确认。仓库约束禁止 agent 执行 git
命令，因此未运行 `git status`、`git diff`、`git push` 或网络请求。

## FILES

- `docs/rewrite/CODE-REVIEW-4.md`（本次唯一人工编辑文件）

构建生成了 `target/`、`node_modules/`、UI `dist/`、sidecar staging 和
`apps/ditto/src-tauri/gen/`；这些均为构建产物。未修改源码、配置、测试、
`legacy/` 或 `LICENSE`。

## BUILD

环境：Windows 11 26200、Rust/Cargo 1.99.1 MSVC、Node 24.13.0、
npm 11.6.2、PowerShell 7.6.6。fresh worktree 开始时无 `target/`、
`node_modules/`、UI `dist/` 或 sidecar staging。

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | 1m 06s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile debug` | PASS | x64 staging |
| `npm ci --prefix apps/ditto/ui` | PASS | 181 packages，0 vulnerabilities |
| `npm ci --prefix apps/ditto` | PASS | 2 packages，0 vulnerabilities |
| `npm run build --prefix apps/ditto/ui` | PASS | 424 shared + 77 popup keys；tsc、Vite PASS |
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | 42.33s |
| `cargo test --workspace --locked` | PASS | **135 passed，0 failed，1 ignored**（migration 10k benchmark） |
| `cargo test -p ditto-storage --test bench_10k --release --locked -- --nocapture` | PASS | insert 1,591.916 ms；quick-paste p95 17.254 ms |
| `cargo test -p ditto-migration --test legacy_fixture --locked -- --nocapture` | PASS | 3/3 |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS | 2m 13s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | x64 staging |
| `npx --no-install tauri build` | PASS | NSIS 生成成功 |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | ZIP 6,899,805 B |
| `tools/qa/install-smoke.ps1` | PASS | NSIS 安装/卸载 + Portable 数据根/pipe |

UI 主 chunk 为 509.16 kB，Vite 仍有大于 500 kB 的 warning，不影响构建。

### 独立产物

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `Ditto.exe` | 11,482,624 | `EBF784E8056F262D8FE2E206B9EB5B5C4D6BBCFB5645DC29A23650DD2F7B64ED` | NotSigned |
| `DittoPopup.exe` | 5,023,744 | `76809EB79B39222597DF296ED2197B50486866A9535852B233BC8F5E329D0D1E` | NotSigned |
| `DittoUacHelper.exe` | 272,896 | `B6E4944664B562C5A50CEB22B877AEFF8DAE633E53C13DF5F9BF874CED8C5205` | NotSigned |
| `Ditto_0.1.0_x64-setup.exe` | 4,898,010 | `366065B199AA259264AA662DC0AB2A04F56FB4529224622CAA749E8BC546A950` | NotSigned |
| `Ditto-0.1.0-x64-portable.zip` | 6,899,805 | `5D63583685B5B424610CE7CEF862AD7533686904A23D441267DBF42B8F3E3233` | 不适用 |

Portable `BUILD-INFO.json` 的 `git_sha` 为
`f8d70912324d67284ea93318d397a1379d48d7b4`，`data_root` 为
`<exe_dir>\Data`；README 为 ``Press Ctrl+` for the popup``。

## TEST

`cargo test --workspace --locked` 独立结果：

- `ditto-clipboard`: 11 passed
- `ditto-ipc`: 6 passed
- `ditto-migration`: 11 unit + 3 legacy fixture passed
- `ditto-model`: 2 passed
- `ditto-paste`: 20 passed
- `ditto-popup-app`: 21 passed
- `ditto-storage`: 16 unit + 10k bench + concurrency passed
- `ditto-tauri-lib`: 30 unit + runtime log integration passed
- `ditto-uac-helper`: 5 passed
- `ditto-win`: 7 passed
- 合计 **135 passed，0 failed，1 ignored**（explicit migration 10k benchmark）

独立运行补充：

| 场景 | 结果 |
|---|---|
| suffix-validation（非法、合法、生产 pipe 不变） | PASS |
| 非 Unicode suffix（unpaired UTF-16 surrogate） | exit 2、无 DB、无生产 pipe |
| hotkey-rebind（正常 rebound/Apply rollback/startup conflict） | PASS |
| keyboard-first（Show→键入→Enter/Shift+Enter→受控 EDIT） | PASS |
| real 10k SQLite + IPC scroll | p50 0.663 / p95 1.287 / max 3.681 ms；0 帧 >16.7 ms |
| 10k cursor paging | 20 页 / 10,000 条 / 495.6 ms |
| IPC smoke 100 show | first-frame p50 2.105 / p95 3.521 ms |
| 空闲 private（settings 关闭） | 3 次中位 40,321,024 B |
| 常驻 popup private | 92,565,504 B < 100 MB |
| Settings 打开 / 二次实例退出 | 81.6 ms / 65.1 ms |
| grouped-main false→Ungrouped | PASS |
| theme dark→HC→system | PASS |
| retention 首轮延迟 | 12s 后按 max_entries 执行；pinned clip 按规则保留 |
| pending import + startup reconcile | **发现 P-1** |
| hidden-tray cold-start feedback | **发现 P-2** |
| NSIS 安装/卸载 + Portable | PASS |

## EVIDENCE

### R2-1 启动热键冲突回写

**CONFIRMED。**

- `bindings_with_issues` 逐项解析，`register_all_collecting` 逐项注册；
- 失败的 open_alt/plain/copy/paste 字段被清空，open chord 写回实际回退值；
- `hotkeys.startup_conflicts` 保存 action/chord/reason/effective 并在 Settings 显示；
- `set_settings` 使用严格 `bindings_from_settings`；运行时 swap 失败会恢复旧绑定并回滚存储。

独立 `hotkey-rebind.ps1` 复现：

```text
CONFLICT_EFFECTIVE_OPEN_CHORD=Ctrl+`
CONFLICT_FELL_BACK_WITHOUT_SILENT_HOOK=True
STARTUP_COPY_CONFLICT_CLEARED=True
STARTUP_COPY_CONFLICT_OPEN_KEPT=True
STARTUP_COPY_CONFLICT_RECORDED=True
STARTUP_COPY_CONFLICT_OPEN_REGISTERED=True
STARTUP_COPY_CONFLICT_PIPE_ALIVE=True
HOTKEY_REBIND_QA=PASS
```

Settings 截图中 warning 明确显示
`Slot 1 Copy (Ctrl+Alt+1): already in use by another application`，未显示
失败的 chord 为已注册值。运行时 Apply swap/rollback 的
`swap_bindings_moves_buffers_and_rolls_back_on_conflict` 继续通过。

### R2-2 copy-buffer cut 一致性

**CONFIRMED（DEFERRED-ACCEPTED）。**

FEATURE-MATRIX G5/S14/A4、DECISIONS D-125、EN/ZH locale 和 UI 一致：

- copy/paste 为 Must；`cut` 为 Should/Deferred；
- `HotkeysPage` 的 cut 控件 disabled、value 固定为空并显示 “Coming later”；
- popup `ActionKind`/`ProviderAction` 和全局 `HotkeyAction` 均无 Cut；
- `bindings_with_issues` 明确忽略 cut，不再注册全局 cut。

截图中没有误导性的 “popup 内选中后剪切” 文案，legacy cut 字段仅保留数据。

### QIS-1 malformed suffix fail-closed

**CONFIRMED。**

独立 release QA：

```text
SUFFIX_PUNCTUATION_EXIT=2
SUFFIX_WHITESPACE_EXIT=2
SUFFIX_TOO_LONG_EXIT=2
SUFFIX_DOT_EXIT=2
VALID_SUFFIX_PIPE_CREATED=True
PRODUCTION_PIPE_BEFORE=False
PRODUCTION_PIPE_AFTER=False
PRODUCTION_PIPE_UNCHANGED=True
SUFFIX_VALIDATION_QA=PASS
```

另用 `ProcessStartInfo` 注入 unpaired UTF-16 surrogate：

```text
NONUNICODE_EXIT=2
NONUNICODE_DB_CREATED=False
NONUNICODE_TRACE_ERROR=True
PRODUCTION_PIPE_UNCHANGED=True
```

源码在单实例事件和 pipe 创建前校验；`DITTO_PIPE_SUFFIX` 和
`DITTO_INSTANCE_ID` 均走同一校验路径。合法 suffix 的 pipe、ACL、slow-client
和单实例测试继续通过。

### R7-1 冷启动、反馈与初始化延后

**PARTIAL；`<2.0 s` 未达成且反馈仍有真实空窗。**

独立同 binary host→Show：

| 样本 | host→Show |
|---|---:|
| same #1 | 3,586.3 ms |
| same #2 | 4,157.2 ms |
| same #3 | 3,633.7 ms |
| 中位 | **3,633.7 ms** |
| fresh copy | **4,797.0 ms** |

breakdown 与 T13 一致：popup IPC 就绪约 1.68–1.79 s，之后 eframe
window/GL 初始化约 1.79–1.87 s，IPC ready→activate 约 79–116 ms。

`general.startup_message` 和 “Ditto is preparing the popup…” 的
PrintWindow 非黑、自动关闭测试独立 PASS；retention 首轮延迟 10 s 后执行，
测试中 3 条 clip 在 pinned 保护下变为 2 条，符合 `max_entries=1 + pinned`
语义。

但真实 feedback 路径有缺口：

1. 隐藏 tray、无 startup message；
2. 启动 release core，等到 popup trace 出现 `hello-ack-prewarm`；
3. 向同一隔离 suffix 的 toggle event 发信号；
4. 结果：

```text
HINT_VISIBLE_AFTER_SIGNAL=False
ACTIVATION_SEEN=True
SIGNAL_TO_ACTIVATION_MS=1,724.5
HINT_TRACE_EXISTS=False
```

原因是 `popup_client_count` 在 `prewarm_ipc()` 完成 Hello 后已经非零，
但此时 eframe 窗口还没创建；`toggle_popup` 因 count>0 不显示 hint。
`cold-start-feedback.ps1` 的第二场景把 `DITTO_POPUP_PATH` 指向不存在的文件，
所以只覆盖“popup 始终没有 client”的情况，没有覆盖真实的 prewarm→window
初始化空窗。

因此 `<2.0 s` 仍应标为 **DEFERRED 且未被本轮接受**：当前可见反馈不能在
最需要它的真实冷启动窗口内保证出现。

### P-1 pending import/migration 首启设置被旧快照覆盖

**STILL BROKEN / Major。**

源码顺序：

1. `load_startup_settings()` 在 Tauri setup 前读取旧 data root 的 settings；
2. setup 内才执行 `pending::apply_staged()` 替换/导入 data root；
3. 随后用 `startup.values` 做 hotkey reconcile 并 `storage.set_settings`。

独立复现使用两个合成数据根：

- 导入包：`open_chord=Ctrl+Alt+Q`、`startup_message=NEW-MESSAGE`、
  `language=zh-CN`、`show_tray_icon=false`；
- 目标旧库：`open_chord=Ctrl+Alt+K`、`startup_message=OLD-MESSAGE`、
  `language=en`、`show_tray_icon=true`；
- 合法 staged package + 无 BOM `.pending-import.json`；
- 启动隔离 release core。

日志确认 import 成功：

```text
[ditto_tauri_lib][INFO] staged data operation: data import/restore applied
[ditto_tauri_lib][INFO] registered 1 global hotkey(s)
[ditto_tauri_lib][INFO] global hotkey active: Ctrl+Alt+K
```

导入后数据库：

```text
clipboard.max_entries=17
general.language="zh-CN"
general.show_tray_icon=false
general.startup_message="NEW-MESSAGE"
hotkeys.open_chord="Ctrl+Alt+K"
```

导入包内的 `Ctrl+Alt+Q` 被旧快照覆盖；启动提示截图实际显示
`OLD-MESSAGE`，而数据库已为 `NEW-MESSAGE`。这同时说明 tray/language/
startup message 的首启运行时仍使用旧值。

影响：用户完成 migration/import/restore 后，第一次启动不会使用新设置；
热键 reconcile 还会把导入包中的热键设置永久覆盖回旧值。该问题直接违反
D-025/D-209 的“下次启动应用”语义和 A4 的有效热键集合要求。

### keyboard-first / GlobalUnlock / 真实粘贴

**CONFIRMED，无回归。**

独立 keyboard-first：

```text
POPUP_FOREGROUND_AFTER_SHOW=True
SEARCH_TYPED_WITHOUT_MOUSE=True
ENTER_PASTE_REQUEST=True
PASTE_LANDED_IN_TARGET_AFTER_ENTER=True
SHIFT_ENTER_PLAIN_PASTE_REQUEST=True
PASTE_LANDED_IN_TARGET_AFTER_SHIFT_ENTER=True
KEYBOARD_FIRST_QA=PASS
```

`GlobalUnlock` movable/zero-length 单元测试通过；受控 Win32 EDIT 实际收到
文本，证明真实 clipboard 写入和 Enter/Shift+Enter 路径成立。

### R-3..R-6、R-8、R-9

| ID | 状态 | 独立结果 |
|---|---|---|
| R-3 | CONFIRMED | `false` 主列表加载 `Ungrouped`；`true` 使用 `All` |
| R-4 | CONFIRMED | dark→HighContrast→system(Light)，HC 使用系统 palette |
| R-5 | CONFIRMED | runtime log integration test PASS |
| R-6 | CONFIRMED | ANSI/OEM NUL、HTML/RTF/CP936/CP1252/CP437 tests PASS |
| R-8 | CONFIRMED | 真实 10k SQLite + IPC，300 帧，0 帧 >16.7 ms |
| R-9 | CONFIRMED | Portable README 单反引号正确 |
| R-10 | PARTIAL | 主要文档已同步；QA-REPORT 测试数仍为 131 |

### 证据抽查

- T13 hotkey conflict 截图：真实 React/Fluent Settings，warning 文案与
  action/chord/reason 正确。
- T13 cut 截图：cut 控件 disabled，逐槽显示 “Coming later”，无误导文案。
- T13 startup/preparing hint 截图：PrintWindow 非黑，文字可读。
- `scroll-10k-real-ipc-t13.csv` 重算：300 帧、0 帧 >16.7 ms、
  max 3.743 ms。
- `popup-latency-t13.csv` 是 mock 历史数据；本次未把它当作真实 IPC 证据，
  真实 IPC 结论来自独立 `scroll-10k-ipc.ps1` 和 `ipc-smoke.ps1`。

### QA 工具问题

`tools/qa/popup-hide-sync.ps1` 仍使用：

```powershell
$_.Path -like "*rewrite-fixes\target\*"
```

在 `rewrite-review4` fresh worktree 中不会杀掉当前测试 popup，首次独立运行得到
`POPUP_VISIBLE_AFTER_CRASH=True`。这是测试脚本路径缺陷，不是本次确认的产品
崩溃清理回归；修正为当前 worktree 路径后仍需处理自动化桌面点击时序。

### 用户进程、真实数据与 master

- 审查开始和结束都没有终止或信号用户
  `D:\Programs\Ditto\Ditto.exe`；PID 35024 保持不变。
- 所有 QA 使用临时数据根和唯一合法 suffix；未读取真实用户 Ditto DB 或历史。
- 只读 refs：
  - `refs/heads/master = 4719eea3e38355473f4cbdd112defccf857bbcee`
  - `refs/remotes/origin/master = 4719eea3e38355473f4cbdd112defccf857bbcee`
  - rewrite 分支没有本地 remote-tracking ref。
- 当前 `LICENSE` 与用户 master `LICENSE` 的 SHA-256 均为
  `230184F60BAE2FEAF244F10A8BAC053C8FF33A183BCC365B4D8B876D2B7F4809`。
- 未执行 git/network 操作；远端 push 状态不能独立确认。

## 残余问题

| ID | 严重度 | 问题 | file:line / 复现 | 影响 | Owner |
|---|---|---|---|---|---|
| P-1 | **Major** | pending import/migration 在 settings 快照之后应用；T13 hotkey reconcile 用旧 settings 写回，覆盖导入热键；首启 tray/language/startup message 仍用旧值 | `apps/ditto/src-tauri/src/lib.rs:221,333,370-374`；`apps/ditto/src-tauri/src/hotkeys.rs:415-477`；复现见本文 P-1 | migration/import/restore 首次启动设置不一致，并永久覆盖导入的 open hotkey | T1/T6/T13 |
| P-2 | **Major** | 隐藏 tray 时 popup prewarm Hello 后、eframe 窗口就绪前没有 preparing 反馈 | `apps/ditto/src-tauri/src/lib.rs:637-649`；`apps/ditto-popup/src/main.rs:35-38`；`apps/ditto-popup/src/app.rs:3291-3305`；复现未见 hint，1,724.5 ms 后才 activate | 首次热键仍可能静默约 1.7 s；R7-1 `<2.0 s` 不能仅靠当前反馈接受 | T1/T3/T7 |
| QA-1 | **Minor** | `popup-hide-sync.ps1` 硬编码旧 worktree 路径，fresh review4 不能复现 T13 `POPUP_VISIBLE_AFTER_CRASH=False` | `tools/qa/popup-hide-sync.ps1:121-123,135-137` | crash cleanup 证据不可移植；容易误报产品回归 | T11/T13 QA |
| DOC-1 | **Minor** | `QA-REPORT.md` 仍写 T13 全量测试 131，实际 135 | `docs/rewrite/QA-REPORT.md:349`；`docs/rewrite/T13-FIX-REPORT.md:76,100` | 证据索引数量不一致 | T7/T13 |
| DOC-2 | **Minor** | FEATURE-MATRIX C11 仍写 retention “at startup”，T13 实际首轮延后 10 s | `docs/rewrite/FEATURE-MATRIX.md:54`；`apps/ditto/src-tauri/src/core_service.rs:28-32,310-339` | 文档语义比实现更严格，易被误解为同步执行 | T2/T9/T13 |

## UNVERIFIED

1. 物理 global-shortcut 按键到 Show 的真实事件源；QA 使用同一 toggle/signal
   路径。
2. Notepad/Word/浏览器/Explorer 的真实 text/HTML/RTF/image/files 粘贴。
3. 真实 UAC elevated target、取消、超时、secure desktop。
4. 真实 IME、Narrator、高对比度可读性、混合 DPI、多显示器。
5. 真实 tray 物理点击；原生 hint 的人眼视觉检查。
6. 缺失 WebView2 Runtime 的真实机器路径。
7. Windows 10、Defender/SmartScreen 实际拦截行为；所有产物均未签名。
8. popup-hide-sync 的产品 crash cleanup 在当前自动化桌面未获得干净独立复现；
   T13 日志存在，但脚本路径缺陷使其不能作为本 worktree 的可复现证据。
9. 60 s 无内存增长断言；本次只验证了 idle/popup 工作集。
10. 远端 push 状态；未执行 git/network 操作。

## RISKS

- 所有可执行产物未签名；新拷贝首启 host→Show 独立测得 4.80 s。
- P-1 会静默覆盖 migrated/imported hotkey 设置，且首次启动显示旧文案；
  在修复前不应把 migration/import/restore 标记为端到端通过。
- P-2 的 current feedback 只在 popup client 不存在时生效；真实 IPC prewarm
  window 内没有可见反馈。
- popup 常驻私有无增长断言未覆盖；当前 92.57 MB 已接近 100 MB 预算。
- `popup-hide-sync.ps1` 的路径问题会污染后续 QA 结论；不能据此次 fresh run
  判断产品崩溃清理回归，也不能据 T13 log 直接宣称可移植复现。

## NEXT

1. 修复 P-1：在 `pending::apply_staged()` 之后重新读取 settings，并用该快照
   重建 hotkey/tray/language/startup-message；禁止用 pre-pending 快照做
   reconcile。增加 pending import/migration 成功后的 hotkey 保留测试。
2. 修复 P-2：把 “popup ready” 定义移到窗口创建/首帧可显示之后，或在
   prewarm Hello 到窗口 ready 之间继续显示非激活 hint；补充真实 prewarm
   window 的 QA。
3. 修正 `popup-hide-sync.ps1` 的动态 worktree 路径和 QA-REPORT 的 135 计数。
4. 修复后重跑完整 T15：fmt/clippy/135+ tests/UI/NSIS/Portable/install、
   hotkey conflict、pending import、hidden-tray feedback、keyboard-first、
   real 10k scroll/paging 和 IPC smoke。

## VERDICT

**需修复**

