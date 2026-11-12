# T10 - T9 修复独立复审

## STATUS

**结论：需修复。**

T9 的 4 个 Blocker 中，B-2/B-3/B-4 的主要代码路径已修复，B-1 的
paste-target 快照部分已修复，但 hotkey 后的直接键盘输入仍不可用，因此
B-1 为 **PARTIAL，仍构成 Blocker**。另有 5 类、7 条残余 Major：A4 热键 Must
未完整交付、M-1 仍有多个运行时设置语义缺失、M-6 的 ANSI/OEM 编辑 payload
不合法、M-14 冷启动仅部分改善、Q7 真实 10k 滚动证据不可由当前源码复现。

本次审查只新增本文件，没有修改源码、配置、测试、`legacy/` 或 `LICENSE`。
没有读取真实 `%APPDATA%\Ditto`、`%LOCALAPPDATA%\Ditto` 或剪贴板历史。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-review2`

## BRANCH

`taohy/rewrite-review2`

## HEAD

`85d81437a3a1057844c3e7a99ff6eddf9a40aadf`

该值由 worktree 的 `.git` 指针和 `refs/heads/taohy/rewrite-review2` 文件只读确认。
仓库约束禁止 agent 自行执行任何 git 命令，因此本文件未提交；由编排者决定提交。

## FILES

- `docs/rewrite/CODE-REVIEW-2.md`（本次唯一允许编辑的文件）

构建还生成了被 `.gitignore` 忽略的 `target/`、`node_modules/`、UI `dist/`
和 sidecar staging。没有产生源码改动。

## BUILD

环境：

- Windows 11 26200，PowerShell 7.6.6，Rust/Cargo 1.99.1 MSVC
- Node 24.13.0，npm 11.6.2
- fresh worktree：无 `target/`、`node_modules/`、sidecar staging、UI `dist/`

独立执行顺序和结果：

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo build -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | 1m20s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile debug` | PASS | 生成 debug sidecar 文件 |
| `npm ci --prefix apps/ditto/ui` | PASS | 181 packages，0 vulnerabilities |
| `npm ci --prefix apps/ditto` | PASS | 2 packages，0 vulnerabilities |
| `npm run build --prefix apps/ditto/ui` | PASS | locales 415+77、tsc、Vite；主 chunk 507.78 kB 警告 |
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | 56.19s |
| `cargo test --workspace --locked` | PASS | 112 passed；migration 10k benchmark ignored |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS | 2m55s |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | 生成 release sidecar 文件 |
| `npx --no-install tauri build` | PASS | NSIS 4,870,091 B |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | Portable ZIP 6,856,367 B |
| `cargo test -p ditto-migration --test legacy_fixture --locked -- --nocapture` | PASS | 3/3 |
| `cargo test -p ditto-storage --test bench_10k --release --locked -- --nocapture` | PASS | 插入 10k、搜索 p95、keyset 预算 |
| `tools/qa/install-smoke.ps1` | STRUCTURE PASS / RUNTIME INVALID | 安装文件、卸载、ZIP 结构通过；pipe 检查命中了审查前已有的 Ditto 进程 |

fresh-worktree 顺序有效：先构建 sidecar 并 staging 后，clippy/test 才能通过。
这说明 Minor-1 的文档/CI 前置顺序修复成立。

## TEST

`cargo test --workspace --locked` 的关键 suite：

- `ditto-clipboard`: 10 passed，含 CP936/CP1252/CP437、HTML/RTF charset、capture budget
- `ditto-ipc`: 6 passed
- `ditto-migration`: 11 unit + 3 legacy fixture passed；10k benchmark ignored
- `ditto-paste`: 18 passed，含 explicit target、multi order/failure、restore suppression
- `ditto-popup-app`: 17 passed
- `ditto-storage`: 14 unit + 10k bench + concurrency passed
- `ditto-tauri-lib`: 19 passed，含 hotkey swap、retention pass、1200-row real SQLite cursor
- `ditto-uac-helper`: 5 passed
- `ditto-win`: 5 passed，含 ACL、slow client、popup event

独立 10k storage release 抽查：

```text
browse      p95 0.514 ms
english     p95 0.912 ms
chinese     p95 0.592 ms
two_char_en p95 0.750 ms
two_char_zh p95 1.254 ms
quick_paste p95 20.217 ms
```

全部低于 50 ms。该结果验证 storage/query，不替代 popup/provider 的 10k 滚动。

## EVIDENCE

### T9 数据复核

| 项目 | T9 值 | T10 判断 |
|---|---|---|
| Popup 首帧 | 100 samples, p50 1711 us, p95 3106 us | 原始 `ipc-smoke-t9.log` 与值一致；未能在本机重跑 |
| 空闲内存 | 3 samples median 40,321,024 B | 原始日志一致；约 38.45 MiB，低于 100 MB |
| 冷启动 | 1696.6 / 2167.6 / 1762.9 ms，median 1762.9 ms | 文件一致；同一 `ipc-smoke-t9.log` 的首次新构建启动为 4197.2 ms |
| Settings 打开 | 91.6 ms | 原始日志一致；只测标题出现 |
| 二次实例退出 | 76.2 ms | 脚本已改为 wall-clock；运行时未能独立重跑 |
| 10k cursor 分页 | 20 pages / 10000 rows / 483.3 ms | 日志一致；运行时未能独立重跑 |
| popup hide/crash sync | False / True / False | 日志一致；运行时未能独立重跑 |
| 迁移 fixture | 3/3 | 本次独立重跑 3/3 PASS |

### 10k 滚动证据问题

对 `artifacts/rewrite/after/perf/scroll-10k-real-sqlite.csv` 独立重算：

```text
ROWS=300
FRAME_P50_MS=0.674
FRAME_P95_MS=1.205
FRAME_MAX_MS=25.600
FRAMES_OVER_16_7_MS=2
UPDATE_P95_MS=0.004
```

该 CSV 本身有 2 帧超过 FEATURE-MATRIX P18 的 16.7 ms 上限。更关键的是，
当前 `--bench-scroll` 参数在 `apps/ditto-popup/src/options.rs:104-107`
强制 `options.mock = true`，没有任何 CLI/env 组合能让该 benchmark 使用真实
IPC provider。因此 T9 “真实 SQLite 10k + 真实 IPC provider”的结论不可由
当前源码复现。

### 独立产物

本次 release 产物：

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `Ditto.exe` | 11,382,784 | `3312253205A69E6FE91C56210FC79040450AD60C6922C22E7891924227CF07C0` | NotSigned |
| `DittoPopup.exe` | 5,007,872 | `5E09C81DF095C732039E968C16EB9E3BF879FA21EC00EC00CF39356D1FF5A41B` | NotSigned |
| `DittoUacHelper.exe` | 272,896 | `D6CD3F3A1E1DD7CF0E9BC3D1008F4C345A707A20AAA7E3CA60D0C665F6DFCFD2` | NotSigned |
| `Ditto_0.1.0_x64-setup.exe` | 4,870,091 | `5F97C2033EC7AE1D5FC2CE57F234EBDD0F10E8E4C138E2D5E0E71DA5E6998756` | NotSigned |
| `Ditto-0.1.0-x64-portable.zip` | 6,856,367 | `7535307BAA3B618E86D220EC1B22A645307073B8496C38DD61ABFC549C2B215E` | 不适用 |

与 T9 哈希不同是预期的：`BUILD-INFO.json` 包含新的构建时间。

Portable 内容独立检查为：

```text
BUILD-INFO.json
Ditto.exe
Ditto2.ico
DittoPopup.exe
DittoUacHelper.exe
locales/en-US.json
locales/zh-CN.json
portable
README.txt
```

`BUILD-INFO.json` 的 `git_sha` 为本次 HEAD，`data_root` 为 `<exe_dir>\Data`。

### Git 只读状态

- worktree HEAD ref：`refs/heads/taohy/rewrite-review2`
- worktree HEAD SHA：`85d81437a3a1057844c3e7a99ff6eddf9a40aadf`
- `master`：`4719eea3e38355473f4cbdd112defccf857bbcee`
- `origin/master`：`4719eea3e38355473f4cbdd112defccf857bbcee`
- `refs/remotes` 只有 `origin/HEAD` 和 `origin/master`，没有 rewrite 分支远跟踪引用

用户 master 引用与冻结值一致。因仓库明确禁止 agent 执行 git 操作，未运行
`git status`、`git diff`、`git ls-remote`；因此 “worktree clean” 和远端当前状态
仍是 UNVERIFIED。本文件本身是唯一预期新增文件，且未提交。

## T8 FINDINGS 逐条复核

### Blocker

| ID | 状态 | 判断 |
|---|---|---|
| B-1 | **PARTIAL / residual Blocker** | Show 前捕获 target、显式 target paste、自窗口拒绝均已实现并有测试。Popup 仍以 `WS_EX_NOACTIVATE` 显示，只有鼠标 pointer press 才激活；hotkey 后直接输入和 Enter 仍不可用。 |
| B-2 | **CONFIRMED FIXED** | 本地 hide 发短连接 `HidePopup`；core 记录 visibility；最后一个 popup client 断开时清空 visibility。保留证据与代码一致；本次未独立重跑。 |
| B-3 | **CONFIRMED FIXED** | 默认 open chord 为 `Ctrl+\``；启动注册；设置变更使用 unregister/register swap，失败恢复旧 chord 并回滚存储；未实现 chord 在 UI disabled。A4 的其余 Must 另列为残余 Major。 |
| B-4 | **CONFIRMED FIXED** | Core 启动即执行维护并每 300s 再执行；retention SQL 保护 pinned/dont_auto_delete/global_shortcut/copy buffer；clear history 立即 purge + GC。测试未显式覆盖 `global_shortcut` 列，但生产 SQL 已覆盖。 |

### Major

| ID | 状态 | 判断 |
|---|---|---|
| M-1 | **PARTIAL / residual Major** | capture flags、limits、source app、tray、default group、density/line/font/preview/animations/on-top 已接线；no-op 控件已 disabled。仍有三个语义缺口，见 R-3/R-4/R-5。 |
| M-2 | **CONFIRMED FIXED** | Popup 一次发送 `clip_ids`；IpcProvider 发单个 `Request::Paste`；core 去重后调用 `paste_many_with_target`。 |
| M-3 | **CONFIRMED FIXED** | CF_TEXT/CF_OEMTEXT 使用 `CP_ACP/CP_OEMCP`；HTML/RTF 处理声明 codepage；tests 覆盖 CP936/1252/437。 |
| M-4 | **CONFIRMED FIXED** | `GlobalSize` 在 `GlobalLock/copy` 前检查；单格式、总字节、格式数均有上限。 |
| M-5 | **CONFIRMED FIXED** | 失败恢复写回 snapshot 前 remember snapshot hash；bounded suppression 与测试存在。 |
| M-6 | **PARTIAL / residual Major** | Unicode payload、hash、preview、大小、FTS 均重写；rich/binary 拒绝。ANSI/OEM 重写缺少 clipboard 要求的 NUL 终止符，见 R-6。 |
| M-7 | **CONFIRMED FIXED** | marker temp+fsync+rename；坏 marker/包 quarantine 后继续启动；Settings 提供 failed marker 清理。 |
| M-8 | **CONFIRMED FIXED** | pipe 为 overlapped；写 250 ms 超时；慢客户端断开；fault-injection test PASS。 |
| M-9 | **CONFIRMED FIXED** | SDDL 仅 SYSTEM + current SID；无 `BA`；不同 SID open 失败有测试。`DECISIONS.md` 仍写 Administrators 只读，见 Minor 文档项。 |
| M-10 | **CONFIRMED FIXED** | 安全 regex 限制与缓存存在；popup 500/page cursor auto-load；`count_clips` 返回真实 total；1200-row real SQLite test PASS。 |
| M-11 | **CONFIRMED FIXED** | Tray 含 Open/Settings/Groups/Copy Buffers/Connect/Disconnect/Exit，EN/ZH locale test PASS。真实 tray click 仍 UNVERIFIED。 |
| M-12 | **CONFIRMED FIXED** | Settings 前检测 Evergreen registry/install dir；缺失时 native MessageBox；NSIS bootstrapper 与 Portable README 已更新。真实缺 runtime 机器未验证。 |
| M-13 | **CONFIRMED FIXED** | `%WINDIR%\explorer.exe` 绝对路径；单测覆盖。 |
| M-14 | **PARTIAL / residual Major** | early spawn + IPC prewarm 与首帧指标有效；同一 binary 的后续冷启动 1.70-2.17s。T9 自己的首次新构建样本仍为 4197.2 ms，不能称为稳定消除。 |
| M-15 | **DEFERRED-ACCEPTED** | FEATURE-MATRIX D7 与 T9 报告一致：一次、下次启动、不可取消、先备份。`DECISIONS.md` 的 D-025/D-209 仍保留 progress/cancel，见 Minor 文档项。 |

### Minor / Nit

| ID | 状态 | 判断 |
|---|---|---|
| Minor-1 | **CONFIRMED FIXED** | fresh worktree 按 sidecar/UI 前置顺序独立通过。 |
| Minor-2 | **CONFIRMED FIXED（代码）** | 脚本从 `Start-Process` 前计时到实际 exit；运行时未能独立重跑。 |
| Minor-3 | **CONFIRMED FIXED（证据）** | 默认 100 show samples，空闲在 show 前取 3 次 median。 |
| Minor-4 | **CONFIRMED FIXED** | 目视检查 T9 Settings/Hotkeys 截图，真实 Tauri WebView2，无 MockBackend badge。 |
| Minor-5 | **STILL BROKEN / residual Major** | 当前 bench 参数强制 mock，真实 provider 不可复现；保留 CSV 有 2 帧超过 16.7 ms。 |
| Minor-6 | **CONFIRMED FIXED** | package verifier 拒绝 manifest 未声明文件，测试存在。 |
| Nit `webview_pid_count` | **CONFIRMED FIXED（代码）** | Toolhelp32 统计真实 child processes；无专门回归测试。 |
| Nit hidden repaint | **CONFIRMED FIXED（代码）** | hidden cadence 改为 1s；visible inactive watcher 30/100ms。 |
| Nit unsigned | **DOCUMENTED** | 本机 4 个可执行产物均 NotSigned；未修改 Defender/SmartScreen/UAC。 |

## 残余问题

| ID | 严重度 | 问题 | file:line / 复现 | 影响 | Owner |
|---|---|---|---|---|---|
| R-1 | **Blocker** | Hotkey Show 后不能直接键入，必须鼠标点击才能让 popup 获得 OS 键盘焦点 | `apps/ditto-popup/src/app.rs:514` 恢复 no-activate；`:962-969` 只有 pointer press 才调用 activate；`apps/ditto-popup/src/platform.rs:172-176` | Hotkey 后直接输入、Enter/Shift+Enter 的 keyboard-first 流程仍不可用。物理 hotkey 也未验证，但源码已经证明该分支缺口。 | T3/T4/T9 |
| R-2 | Major | A4 要求 rebind open/plain/first-ten/copy-buffer，但仅 open 注册；其余 Must 控件 disabled，且没有 coordinator 的 cut-line 变更 | `docs/rewrite/FEATURE-MATRIX.md:128`；`apps/ditto/ui/src/pages/HotkeysPage.tsx:117-191`；`commands/mod.rs:408-447` | v1 Must 仍有未交付的全局热键配置，不能以“Coming later”替代 Must，除非由编排者正式接受并更新矩阵。 | T1/T5 |
| R-3 | Major | `show_grouped_in_main=false` 不会从 All 列表排除 grouped clips | `apps/ditto-popup/src/app.rs:769-777` 只切换到 `All` 并隐藏菜单；`apps/ditto-popup/src/provider.rs:414-418` 将 `All` 映射为无 group filter | G6 控件看起来接线，但主列表语义不变。复现：设置 false，打开 popup，grouped clip 仍在主列表。 | T3/T5 |
| R-4 | Major | `highContrast` 和 `system` theme 值没有正确映射 | UI 写入 `highContrast`/`system`：`apps/ditto/ui/src/types.ts:1`、`AppearancePage.tsx:30-33`；popup 只匹配 `hc`/`high_contrast`/`light`/`dark`：`apps/ditto-popup/src/app.rs:784-788` | 选择 High Contrast 不改变 popup；从 dark 切回 system 也可能维持 dark 到重启。违反 A7。 | T3/T5 |
| R-5 | Major | runtime 提高 log level 仍被启动时 fern filter 截断 | 启动 filter：`apps/ditto/src-tauri/src/lib.rs:230-233`；运行时只 `log::set_max_level`：`:720-726` | info/error 降到更低级别可生效，但 info→debug/trace 不会产生新详细日志；A10 的运行时语义不完整。 | T1/T5 |
| R-6 | Major | 编辑 legacy ANSI/OEM text 时 payload 没有 NUL 终止符 | `crates/ditto-clipboard/src/decode.rs:80-113` 只返回 `written` 字节；`store.rs:714-718` 直接存入 CF_TEXT/CF_OEMTEXT | Windows text clipboard formats 要求 NUL 终止。旧 ANSI-only clip 编辑后可能被消费者越界读取，且 hash 与下一次真实 capture 不一致，影响 Original paste/dedupe。 | T2/T9 |
| R-7 | Major | M-14 冷启动只部分改善 | `apps/ditto/src-tauri/src/lib.rs:217-219` early spawn；T9 后续 1.70-2.17s，但新构建首启 4197.2 ms | 同一 binary 的 prewarm 有效，但首次安装/升级仍可能有数秒 hotkey 延迟；不能把 UNVERIFIED Defender 延迟完全排除。 | T1/T3/T7 |
| R-8 | Major | Q7 的真实 10k popup scroll 没有可复现路径 | `apps/ditto-popup/src/options.rs:104-107` 的 `--bench-scroll` 强制 `mock=true`；`scroll-10k-real-sqlite.csv` 有 2 帧 >16.7 ms | “真实 IPC provider”结论不可复现，且保留数据本身不满足 P18 的 no-frame-over-16.7ms。 | T3/T7 |
| R-9 | Minor | Portable README 把反引号当 PowerShell escape，实际输出变成 `Press Ctrl+` | `scripts/package-portable.ps1:74` | 用户看到的默认热键说明错误。 | T1/T7 |
| R-10 | Minor | 文档仍保留被 T9 取代的结论 | `REVIEW-GUIDE.md:73,148-149` 仍说不注册设置热键；`DECISIONS.md:46,60,98` 仍写 progress/cancel、Administrators read 和 cancel UI | 审查/决策文档互相矛盾，后续 agent 可能按旧要求回归。 | T7/T9/coordinator |

## UNVERIFIED

1. 物理 hotkey 到直接输入、Enter/Shift+Enter、真实应用粘贴的完整链路。
2. 真实 Notepad/Word/浏览器/Explorer 的 text/HTML/RTF/image/files 粘贴。
3. UAC elevated target、取消、超时、secure desktop。
4. OLE drag-out，IME，Narrator，高对比度系统主题，混合 DPI，多显示器。
5. 真实 tray click。
6. 缺失 WebView2 Runtime 的机器路径。
7. Windows 10、Defender/SmartScreen 实际拦截。
8. 本机 IPC smoke、100 样本首帧、3 次独立冷启动、settings/second-instance、
   crash sync、真实 10k paging/scroll 的独立重跑。原因是审查前已有
   `Ditto.exe`（`D:\Programs\Ditto`）和旧 `rewrite-fixes` popup 持有当前 SID
   的 pipe；审查没有终止或信号该进程。
9. worktree clean 和远端 push 状态。仓库约束禁止 agent 运行 git 命令。

## RISKS

- 所有可执行产物未签名；本次未修改 Defender、SmartScreen、防火墙或 UAC。
- 现有用户 Ditto 进程占用了 per-SID pipe；任何第二实例或 QA 脚本都会命中它。
  因此 install-smoke 的 pipe/Portable runtime 检查不能作为本次独立运行证据，
  只保留文件布局和卸载结构证据。
- T9 的 10k scroll CSV 与当前 `--bench-scroll` 实现不一致，不能仅凭文件名和
  CSV 推断真实 provider。
- 本次未验证远端仓库，也没有执行 git status/diff/commit。

## NEXT

1. 修复 B-1 的 keyboard-first 路径：hotkey Show 后必须能直接键入，且 click/
   keyboard 激活后仍使用 Show 前 target snapshot。
2. 补齐 A4 的 Must 热键或由编排者正式接受 cut-line 变更并同步 FEATURE-MATRIX/
   DECISIONS。
3. 修复 M-1 的 grouped-main、theme 映射和 runtime log level；修复 M-6 的
   ANSI/OEM NUL 终止。
4. 提供不强制 mock 的真实 10k popup scroll harness，并以 P18 的 16.7 ms 上限
   重新判定；同时重测新构建首次冷启动。
5. 由编排者清理/协调现有 Ditto runtime 后，重跑 IPC、冷启动、settings、
   second-instance、popup crash、paging/scroll、NSIS 和 Portable runtime smoke。

## VERDICT

**需修复**
