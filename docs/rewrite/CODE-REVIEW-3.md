# T12 - T11 修复独立复审

## STATUS

**结论：需修复。**

T11 的主体修复成立：R-1、R-3、R-4、R-5、R-6、R-8、R-9、R-10
均通过独立源码复核或运行抽查，`GlobalUnlock` 修复也通过真实粘贴路径。
fresh worktree 的 fmt、clippy、128 tests、UI、NSIS、Portable、安装/卸载
全部独立通过。

仍有以下残余问题，暂不能判定“可合并”：

- **R-2 PARTIAL（Major）**：运行时 Apply 的热键 swap/rollback 正确；但启动时
  若任一非 open chord 冲突，只回退并注册 open chord，不回写失败的
  plain/copy-buffer 绑定。Settings 仍显示未注册的 chord。已独立复现。
- **R-2 范围一致性（Major）**：copy-buffer `cut` 的全局热键已明确禁用，但
  FEATURE-MATRIX G5 仍把 `cut` 列为 Must，UI 帮助又称“需要在弹窗内选中后
  剪切”，而 popup/provider 没有对应 cut action。功能、UI、矩阵三者不一致。
- **QA 隔离（Major）**：`DITTO_PIPE_SUFFIX` 非空但清洗后为空时，
  `instance_suffix()` 返回 `None`，应用静默落回生产 pipe/单实例命名。格式错误
  的 QA override 可能触发用户实例，违反隔离约束。
- **R-7 PARTIAL（Major）**：同 binary host→Show 仍为 3.57-3.60 s，新拷贝首启
  4.68 s；tray tooltip 只在 tray 存在时提供反馈。现有证据能区分新拷贝进程创建
  前的额外约 0.35 s，但不能证明其为 Defender 首扫。

没有重新发现 R-1 的 Blocker，也没有发现 R-3 至 R-6、R-8 至 R-10 的阻断性回归。

本次只新增本文件，没有修改源码、配置、测试、`legacy/` 或 `LICENSE`。
没有读取真实 `%APPDATA%\Ditto`、`%LOCALAPPDATA%\Ditto` 或历史数据库；
所有运行 QA 使用临时 `DITTO_DATA_DIR` 和唯一合法后缀。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-review3`

## BRANCH

`taohy/rewrite-review3`

基线 `taohy/rewrite-fixes2` @ `408ea2ab86bc75a1d8d445346406e40dbc46316a`。

## HEAD

`408ea2ab86bc75a1d8d445346406e40dbc46316a`

该值由 worktree `.git` 指针和 `refs/heads/taohy/rewrite-review3` 只读确认。
仓库约束禁止 agent 执行任何 git 命令，因此未运行 `git status`、`git diff`
或提交；本文件由编排者决定是否提交。

## FILES

- `docs/rewrite/CODE-REVIEW-3.md`（本次唯一人工编辑文件）

构建生成或更新的 `target/`、`node_modules/`、UI `dist/`、sidecar staging、
`apps/ditto/src-tauri/gen/` 和 QA 临时目录均为构建/运行产物，不属于源码修改。

## BUILD

环境：

- Windows 11 26200，PowerShell 7.6.6
- Rust/Cargo 1.99.1 MSVC，Node 24.13.0，npm 11.6.2
- fresh worktree：开始时无 `target/`、`node_modules/`、`dist/`、sidecar staging

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | 1m21s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile debug` | PASS | x64 sidecar staging |
| `npm ci --prefix apps/ditto/ui` | PASS | 181 packages，0 vulnerabilities |
| `npm ci --prefix apps/ditto` | PASS | 2 packages，0 vulnerabilities |
| `npm run build --prefix apps/ditto/ui` | PASS | 418 shared + 77 popup keys；tsc、Vite PASS |
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | 50.41s |
| `cargo test --workspace --locked` | PASS | 128 passed，0 failed |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS | 2m43s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | x64 sidecar staging |
| `npx --no-install tauri build` | PASS | 2m01s，NSIS 生成成功 |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | ZIP 6,874,467 B |

UI 主 chunk 为 508.21 kB，Vite 仍报告大于 500 kB 的 warning，不影响构建。

### 独立产物

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `Ditto.exe` | 11,418,624 | `64B0957A9D20189157856D60978481C3558D00E15D7B5B3A8C5555A2BEFE83BB` | NotSigned |
| `DittoPopup.exe` | 5,021,184 | `A80472EA24BAD08A09A68BE95235819A24C8E264886122DE82ED8ECB788A9507` | NotSigned |
| `DittoUacHelper.exe` | 272,896 | `BC8F9FECD48C422872FA898147E1B98C3C9FD46A68C12A190D1AD68BAF6F42BD` | NotSigned |
| `Ditto_0.1.0_x64-setup.exe` | 4,883,587 | `9EB2B8B360A80D8C73CE95F30B0660F9A10B9882552E03447B713B3281D959B9` | NotSigned |
| `Ditto-0.1.0-x64-portable.zip` | 6,874,467 | `7AB6B6D1A99C17FA8FE24BE990C88AE5054B329DD2899B27A6CA57ACBBBEB3D3` | 不适用 |

Portable `BUILD-INFO.json` 的 `git_sha` 为
`408ea2ab86bc75a1d8d445346406e40dbc46316a`，`data_root` 为
`<exe_dir>\Data`。解压后的 README 实际包含
``Press Ctrl+` for the popup``。

## TEST

`cargo test --workspace --locked` 独立结果：

- `ditto-clipboard`: 11 passed
- `ditto-ipc`: 6 passed
- `ditto-migration`: 11 unit + 3 legacy fixture passed
- `ditto-paste`: 20 passed
- `ditto-popup-app`: 21 passed
- `ditto-storage`: 16 unit + 10k bench + concurrency passed
- `ditto-tauri-lib`: 25 unit + runtime log integration passed
- `ditto-uac-helper`: 5 passed
- `ditto-win`: 5 passed
- 合计 128 passed，0 failed

额外独立执行：

| 命令 / 场景 | 结果 |
|---|---|
| `cargo test -p ditto-storage --test bench_10k --release --locked -- --nocapture` | PASS；10k insert 1890 ms，搜索 p95 0.703-21.672 ms |
| `cargo test -p ditto-migration --test legacy_fixture --locked -- --nocapture` | PASS；3/3 |
| `tools/qa/grouped-main.ps1` | PASS；false → Ungrouped |
| `tools/qa/theme-modes.ps1` | PASS；dark/HC/system |
| `tools/qa/scroll-10k-ipc.ps1` | PASS；真实 10k SQLite + IPC |
| `tools/qa/ipc-smoke.ps1` | PASS |
| `tools/qa/install-smoke.ps1` | PASS；NSIS 安装/卸载 + Portable |

## EVIDENCE

### R-1 keyboard-first / Show 前目标快照

**CONFIRMED。**

源码显示 `toggle_popup` 在广播 Show 前调用 `capture_paste_target()`，
`PipeServer::set_popup_visible(true)` 的 visibility hook 也在广播前再次快照；
popup Show 后清除 `WS_EX_NOACTIVATE`，通过 `AttachThreadInput`、
`SetForegroundWindow`、`SetFocus` 取得键盘焦点。Enter 和 Shift+Enter 的匹配顺序
也已修正。

直接运行 `tools/qa/keyboard-first.ps1` 在当前后台自动化桌面两次失败，原因是
QA 的 EDIT 目标无法取得前台：独立 Win32 探针得到
`FOREGROUND_IS_TARGET=False`，所以 core 捕获了错误目标，paste 断言失败。
这说明脚本缺少 Show 前前台断言，不是 popup 键盘路径失败。

使用一个仅把 QA 目标提升为前台的临时 helper 后，未修改源码，结果：

```text
POPUP_FOREGROUND_AFTER_SHOW=True
SEARCH_TYPED_WITHOUT_MOUSE=True
ENTER_PASTE_REQUEST=True
PASTE_LANDED_IN_TARGET_AFTER_ENTER=True
SHIFT_ENTER_PLAIN_PASTE_REQUEST=True
PASTE_LANDED_IN_TARGET_AFTER_SHIFT_ENTER=True
KEYBOARD_FIRST_QA=PASS
```

物理热键按键本身仍未独立验证；自动化通过 IPC Show 触发同一 Show 路径。

### R-2 全局热键注册、作用域、冲突回滚

**PARTIAL。**

正常路径 PASS：`hotkey-rebind.ps1` 独立得到 configured open/copy registration、
旧键释放、持久化重启、copy-buffer copy 全局动作和 open conflict fallback；
`HOTKEY_REBIND_QA=PASS`。

但启动时非 open chord 冲突未正确回写。独立复现：

1. 设置 `open_chord=Ctrl+Alt+K`、copy buffer 1 copy=`Ctrl+Alt+1`。
2. 审查进程先占用 `Ctrl+Alt+1`。
3. 启动隔离 core。
4. 结果 `OPEN_CHORD_HELD_BY_CORE=True`，但
   `PERSISTED_COPY_CHORD=Ctrl+Alt+1`、
   `STARTUP_CONFLICT_SETTINGS_RECONCILED=False`。

因此用户重启后可能看到 copy-buffer chord 仍已配置，但实际没有注册。

### R-3 `show_grouped_in_main=false`

**CONFIRMED。**

false 时主列表从 All 映射为 `Ungrouped`，storage 用 `group_id IS NULL` 排除
grouped clips；显式分组导航保持 Group。独立 QA 结果为
`SHOW_GROUPED_FALSE_USES_UNGROUPED=True`、`GROUPED_MAIN_QA=PASS`。

### R-4 theme 映射与 HC 系统色

**CONFIRMED。**

`system/light/dark/highContrast` 及 legacy spellings 均映射；Show 和设置变化时
重新解析 system；HC palette 使用 `GetSysColor`。独立 QA 得到
dark→HighContrast→System(Light) 全 PASS。

### R-5 runtime log level

**CONFIRMED。**

logger 以 Trace max level 安装，每条记录经过 `logging::enabled` 的 AtomicU8
判断；integration test 在同一进程内验证 info→debug 新记录出现、
debug→info 再次过滤。`cargo test` PASS。

### R-6 ANSI/OEM NUL 与 round-trip

**CONFIRMED。**

`encode_ansi_text` / `encode_oem_text` 追加 NUL；空文本也生成单个 NUL。
`rewrite_clip_text` 同时重写 CF_TEXT/CF_OEMTEXT、content/formats hash、
preview、quick-paste 和 FTS。CP936/CP1252/CP437 单测通过。

### R-7 cold start

**PARTIAL。**

独立复测：

| 场景 | host→Show | popup start→Show | popup start→IPC |
|---|---:|---:|---:|
| same #1 | 3,603.1 ms | 1,784.0 ms | 1,667.0 ms |
| same #2 | 3,569.8 ms | 1,789.0 ms | 1,702.0 ms |
| same #3 | 3,583.8 ms | 1,773.0 ms | 1,692.0 ms |
| fresh copy | 4,677.3 ms | 1,783.0 ms | 1,714.0 ms |

host trace 显示新拷贝在 popup path 到 popup PID 之间额外约 0.35 s；同 binary
主延迟主要来自 core/Tauri 初始化和 popup GL/字体初始化重叠。没有 Defender
扫描事件证据，不能把 fresh 额外延迟直接归因于 Defender。
tray “preparing” tooltip 路径存在，但隐藏 tray 时没有替代反馈，也未做真实目视验证。

### R-8 真实 10k IPC scroll

**CONFIRMED。**

`--bench-scroll` 不再强制 mock，脚本显式使用 `--provider=ipc`、临时数据根、
唯一 suffix 和合成 10k seed。独立结果：

```text
DB_TOTAL_ROWS=10000
FRAMES=300
FRAME_P50_MS=1.417
FRAME_P95_MS=2.386
FRAME_MAX_MS=2.972
FRAMES_OVER_16_7_MS=0
UPDATE_P95_MS=0.009
MAX_RENDERED_ROWS=9
MAX_SCROLL_OFFSET_PX=162000.0
P18_FRAME_BUDGET=PASS
```

### R-9 Portable README

**CONFIRMED。**

重新打包后的 README 为单反引号 ``Press Ctrl+` for the popup``。

### R-10 文档一致性

**CONFIRMED（限 T11 声明范围）。**

REVIEW-GUIDE、DECISIONS、FEATURE-MATRIX 的 hotkey、ACL、migration 一次性
语义已同步。`QA-REPORT.md` 仍保留 T7 历史“设置热键未注册”表格，但其后
T9/T11 章节明确声明取代旧结论，不影响当前决策入口。G5 的 `cut` 歧义另见
R-2 残余问题。

### `GlobalUnlock` 修复

**CONFIRMED。**

`GlobalUnlock` 在锁计数归零时返回 FALSE，且成功时 `GetLastError()` 为
`ERROR_SUCCESS`；当前实现按该语义判断。独立 Win32 探针也得到
`UNLOCK_RETURN=False LAST_ERROR=0`。

真实路径已由加强后的 keyboard-first QA 间接验证：clipboard payload 写入成功，
目标窗口实际收到文本。现有单测覆盖 movable block 和 zero-length allocation，
但没有失败注入单测；不构成当前阻断项。

### QA 隔离与用户进程

合法 GUID suffix 的所有脚本均使用临时 `DITTO_DATA_DIR` 和隔离 pipe；ACL 未变化，
同后缀仍由快速单实例事件保护。脚本未终止用户进程：

- 审查前后 `D:\Programs\Ditto\Ditto.exe` PID 35024 不变；
- 既有 T9 `rewrite-fixes` popup PID 40992 不变；
- 审查结束后没有残留本 worktree `Ditto.exe`/`DittoPopup.exe`。

但 malformed suffix 会在清洗为空后静默回退生产命名，见残余问题。

### master 与远端本地状态

未执行任何 git 命令。只读确认：

- 用户 master `HEAD` / `refs/heads/master` / `refs/remotes/origin/master`
  均为 `4719eea3e38355473f4cbdd112defccf857bbcee`；
- `refs/remotes` 只有 `origin/HEAD` 和 `origin/master`，没有 rewrite 分支
  远端跟踪引用；
- 用户 master 树中排除 `.git` 后，mtime 晚于本次审查开始的文件数为 0；
- master `.git/index` mtime 为 `2026-11-12 19:09:18`，早于本次审查。

这证明本地 master 引用和文件未被本次审查改动；远端当前状态仍无法在禁止
git/网络操作的前提下独立确认。

## 逐条结论

| ID | 结论 | 摘要 |
|---|---|---|
| R-1 | **CONFIRMED** | Show 后键盘焦点、输入、Enter/Shift+Enter、Show 前目标粘贴在隔离作用域 PASS；物理热键仍 UNVERIFIED |
| R-2 | **PARTIAL** | 正常注册/rebind/运行时 rollback PASS；启动时非 open 冲突不回写，cut 的 G5/UI/code 不一致 |
| R-3 | **CONFIRMED** | false → Ungrouped，true → All，显式分组不变 |
| R-4 | **CONFIRMED** | system/light/dark/HC 映射和 HC 系统 palette PASS |
| R-5 | **CONFIRMED** | runtime reloadable logger 实际生效 |
| R-6 | **CONFIRMED** | ANSI/OEM NUL、hash/preview/FTS round-trip PASS |
| R-7 | **PARTIAL** | 反馈代码存在，但首次可用仍 3.58 s / fresh 4.68 s；Defender 未观测 |
| R-8 | **CONFIRMED** | 真实 10k SQLite/IPC 300 帧，0 帧 >16.7 ms |
| R-9 | **CONFIRMED** | Portable README 反引号正确 |
| R-10 | **CONFIRMED** | T11 指定三份文档一致；G5 cut 另列 |
| GlobalUnlock | **CONFIRMED** | 成功语义正确，真实文本粘贴落地 |

## 残余问题

| ID | 严重度 | 问题 | file:line / 复现 | 影响 | Owner |
|---|---|---|---|---|---|
| R2-1 | **Major** | 启动时任一非 open chord 冲突后，仅回退注册 open chord，不回写失败的 plain/copy-buffer 绑定 | `apps/ditto/src-tauri/src/lib.rs:331-339,645-669`；`apps/ditto/src-tauri/src/hotkeys.rs:233-250`。复现：占用 Ctrl+Alt+1，配置 copy chord 为 Ctrl+Alt+1，启动后 GetSettings 仍返回该值 | Settings 显示已配置但热键不工作，用户无法从 UI 判断实际生效集合 | T1/T11 |
| R2-2 | **Major** | copy-buffer `cut` 的 Must/UI/code 不一致 | `docs/rewrite/FEATURE-MATRIX.md:118` 仍要求 cut；`apps/ditto/ui/src/pages/HotkeysPage.tsx:179-192` disabled 并提示弹窗内选中；`apps/ditto-popup/src/worker.rs:24-31,178-190` 无 Cut action | Must 是否满足取决于 coordinator 的正式 cut-line；当前文案指向不存在的 in-popup cut | T4/T5/coordinator |
| QIS-1 | **Major** | 非空但清洗后为空的 suffix 静默落回生产 pipe/单实例命名 | `crates/ditto-win/src/lib.rs:133-155`；`apps/ditto/src-tauri/src/lib.rs:78-90`。例：`DITTO_PIPE_SUFFIX=!!!` | malformed QA override 可能触发用户正在运行的实例，违反隔离硬约束 | T1/T11 |
| R7-1 | **Major** | cold start 仍明显，tray feedback 不覆盖隐藏 tray | `apps/ditto/src-tauri/src/lib.rs:229-231,563-609`；独立数据 3.57-3.60 s / 4.68 s | 首次热键仍可能长时间无可见反馈；Defender 与代码延迟未完全分离 | T1/T3/T7 |
| QA-1 | **Minor** | keyboard-first 脚本不验证 Show 前目标真的是前台 | `tools/qa/keyboard-first.ps1:257-264`；当前后台桌面 plain run 可稳定得到 `FOREGROUND_IS_TARGET=False` 并使断言失败 | QA 在不同会话中不可重复，需人工/helper 预置前台，容易把环境问题误判为产品回归 | T11/T12 QA |

## UNVERIFIED

1. 物理 hotkey 按键到 Show 的真实 global-shortcut 事件源。
2. Notepad/Word/browser/Explorer 的真实 text/HTML/RTF/image/files 粘贴。
3. 真实 UAC elevated target、取消、超时、secure desktop。
4. 真实 IME、Narrator、高对比度可读性、混合 DPI、多显示器。
5. 真实 tray 点击和 tray “preparing” 文案目视效果。
6. 缺失 WebView2 Runtime 的机器路径。
7. Windows 10、Defender/SmartScreen 实际拦截行为。
8. 通过真实 Settings UI 点击切换 log level 的端到端路径。
9. 远端仓库当前状态；未执行 git/network 操作。
10. 工作区 git clean 状态；仓库约束禁止 agent 运行 git。

## RISKS

- 所有可执行产物均未签名；本次没有修改 Defender、SmartScreen、防火墙或 UAC。
- T11 的 cold-start 证据表明 popup 已常驻后首帧仍为毫秒级，但首次可用延迟
  仍可能为 3.5-4.7 s；隐藏 tray 时没有“preparing”可见提示。
- keyboard-first 的 plain 调用依赖自动化桌面能把 QA 目标提升为前台；本审查
  必须使用外部 foreground helper 才能复现 T11 PASS。
- QA 覆盖 override 的 suffix 清洗目前不能区分“未设置”和“设置但无效”，存在
  误触生产实例的风险。
- 本次 hotkey QA 写入了合成剪贴板文本 `DITTO-QA-COPY-BUFFER`；没有读取原
  clipboard/history，也没有尝试恢复未知的用户原内容。

## NEXT

1. 修复启动时全局热键部分注册失败的持久化/UI 对账，并增加“非 open chord
   冲突”的自动化回归。
2. 明确 copy-buffer `cut` 的 v1 cut-line：实现 popup cut，或移除 G5 Must、
   修正 UI 帮助并记录 coordinator 决策。
3. malformed/empty-after-sanitize suffix 必须 hard fail，不能回退生产命名；
   同时增加 suffix 合法性单测。
4. 决定 R-7 是继续优化首次可用延迟，还是由 coordinator 正式接受并记录
   首次启动反馈风险；补充 tray 隐藏场景反馈。
5. 修复 keyboard-first 脚本的 Show 前前台断言后，创建 T13 复审。

## VERDICT

**需修复**
