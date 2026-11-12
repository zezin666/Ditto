# T9 - 修复 T8 审查问题 + 回归

## STATUS

**完成（1 个 Major 按产品决策 DEFERRED，其余 Blocker/Major/Minor/Nit 均已 FIXED）。**

本任务逐条处理 `docs/rewrite/CODE-REVIEW.md` 的 4 个 Blocker、15 个 Major、6 个 Minor、3 个 Nit。所有改动都在本任务 worktree 内；没有修改 `legacy/`、`LICENSE`、用户 master 工作区或系统安全设置。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-fixes`

## BRANCH

`taohy/rewrite-fixes`（base `taohy/rewrite-review` @ `e7a5e2d`）

## HEAD

`dcb75d3`（T9 修复提交）；base `e7a5e2d`。

## FILES

主要源码/测试/文档：

- `apps/ditto/src-tauri/src/{lib.rs,core_service.rs,pending.rs,commands/mod.rs,hotkeys.rs}`
- `apps/ditto-popup/src/{app.rs,platform.rs,provider.rs,worker.rs,mock.rs,main.rs}`
- `apps/ditto/ui/src/{App.tsx,types.ts,components/Controls.tsx,pages/*,api/commands.ts}`
- `apps/ditto/ui/locales/{en-US.json,zh-CN.json}`
- `crates/ditto-win/src/lib.rs`
- `crates/ditto-clipboard/src/{lib.rs,windows.rs,decode.rs,error.rs}`
- `crates/ditto-paste/src/{engine.rs,target.rs,lib.rs}`
- `crates/ditto-storage/src/{store.rs,schema.rs}`
- `crates/ditto-migration/src/{package.rs,format.rs}`
- `crates/ditto-storage/examples/seed_qa.rs`
- `tools/qa/{ipc-smoke.ps1,popup-hide-sync.ps1,paging-check.ps1}`
- `scripts/package-portable.ps1`
- `docs/rewrite/{T9-FIX-REPORT.md,QA-REPORT.md,REVIEW-GUIDE.md,FEATURE-MATRIX.md}`
- 证据：`artifacts/rewrite/after/**`（被 `.gitignore` 忽略，使用 `git add -f` 纳入）

## BUILD

环境：Windows 11 26200、Rust 1.99.1 MSVC、Node 24.13.0、npm 11.6.2、Tauri CLI 2.12.4、Windows SDK 26100。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `npm run build --prefix apps/ditto/ui` | PASS（locales 校验、tsc、Vite） |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app -p ditto-tauri --locked` | PASS |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS |
| `npx --no-install tauri build` | PASS（NSIS 4.87 MB） |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS |
| `tools/qa/install-smoke.ps1` | PASS（NSIS 安装/卸载、Portable 启动、`Data\ditto.db`） |

## TEST

`cargo test --workspace --locked` 关键结果：

- `ditto-clipboard`: 10 passed（含 CP936/CP1252/CP437、HTML/RTF charset、捕获预算）
- `ditto-ipc`: 6 passed
- `ditto-migration`: 11 unit + 3 legacy fixture passed；10k benchmark 仍 ignored
- `ditto-paste`: 18 passed（含显式目标快照、失败恢复 suppression）
- `ditto-popup-app`: 17 passed
- `ditto-storage`: 14 unit + 10k bench + concurrency passed（含 regex、payload rewrite、retention）
- `ditto-tauri`: 19 passed（含 hotkey swap/回滚、pending 隔离、retention 运行期接线、真实 SQLite 1200 行 cursor 分页）
- `ditto-uac-helper`: 5 passed
- `ditto-win`: 5 passed（含 ACL、慢客户端超时、跨 SID 拒绝）

## EVIDENCE

### T8 审查项逐条状态

#### Blocker

| 编号 | 状态 | 修复与证据 |
|---|---|---|
| B-1 弹窗键盘/粘贴目标 | **FIXED** | 新增 `CoreService::capture_paste_target`：hotkey/tray Show 在 popup 激活前抓取 `TargetWindow`（HWND/PID/thread/focus/integrity）；`PasteEngine::paste_many_with_target` 使用显式快照并调用 `TargetBackend::validate`，仍拒绝 Ditto 自身窗口。设置窗口打开时也抓取前置目标。测试：`explicit_target_snapshot_is_used_without_capturing_the_foreground`。物理 hotkey→Enter 仍属 UNVERIFIED（见下）。 |
| B-2 弹窗/核心可见性同步 | **FIXED** | 本地 hide（Esc/失焦/外部点击/错误）通过短连接发送带 request-id 的 `HidePopup`；后台 watcher 轮询外部点击，不依赖 egui 帧。core 在最后一个 Popup 客户端断开时清空 `popup_visible`（崩溃场景）。测试/证据：`tools/qa/popup-hide-sync.ps1` → `POPUP_VISIBLE_AFTER_OUTSIDE_CLICK=False`、`...AFTER_NEXT_SHOW=True`、`...AFTER_CRASH=False`。 |
| B-3 设置热键注册 | **FIXED** | host 启动读取 `hotkeys.open_chord`（默认 `Ctrl+``，冲突时回退开发键）并注册 global-shortcut；`SettingsChanged` 通过 `hotkeys::swap` 原子 unregister/register，冲突时恢复旧 chord 并回滚持久化设置。真实 UI 验证：Apply `Ctrl+Alt+K` 后旧键可用/新键被占用；占用 `Ctrl+Alt+J` 后 Apply 显示 “Settings could not be applied.”，持久化仍为 `Ctrl+Alt+K`（截图 `T9-hotkey-conflict-rollback.png`）。 |
| B-4 retention/purge 运行时接线 | **FIXED** | core 启动即执行一次并每 5 分钟执行 `cleanup_retention` + `purge_deleted_before`；保护 pinned/dont_auto_delete/copy-buffer/global-shortcut。`clear_history` 在 soft delete 后立即 purge 并 GC blob。测试：`maintenance_pass_applies_retention_and_purges_deleted_clips`、`retention_protects_...`、`hard_delete_and_gc_remove_blob_files`。 |

#### Major

| 编号 | 状态 | 修复与证据 |
|---|---|---|
| M-1 设置开关 no-op | **FIXED** | capture_files/images/record_source_app/max/immediate debounce 通过监控重启接线；show_tray_icon 运行时重建 tray；default_group_id 写入新 clip；density/lines/font_scale/preview/animations/always_on_top/show_grouped_in_main 接入 popup；log_level 启动与运行时生效。single_instance/start_minimized/startup_message/prompt_for_name/play_sound/check_updates 明确 disabled + “Coming later”。真实截图 `T9-tauri-settings-real.png`、`T9-tauri-hotkeys-real.png`。 |
| M-2 多选粘贴 | **FIXED** | popup 一次发送 `clip_ids`；`IpcProvider::paste_many` 发单个 `Request::Paste { clip_ids }`；core 走 `PasteEngine::paste_many`。engine 多选顺序/失败前缀测试 + core `paste_ids` 去重测试；suppression 改为 16 项窗口以覆盖连续写入。 |
| M-3 CF_TEXT/OEMTEXT 解码 | **FIXED** | `MultiByteToWideChar` 解码 CP_ACP/CP_OEMCP；HTML 按 charset、RTF 按 `\ansicpg` 与 `\'hh` 连续字节解码。测试：CP936 `D6 D0 CE C4`、CP1252、CP437、HTML gbk、RTF ansicpg936。 |
| M-4 剪贴板上限 | **FIXED** | `read_global_format` 在 `GlobalLock/copy` 前检查 `GlobalSize`；新增单格式/总字节/格式数上限（默认 128 MiB、256 formats）。超限跳过并产生 warning，不分配大 buffer；`build_captured_clip` 二次校验。测试：`capture_flags_and_total_budget_filter_formats_before_storage`。 |
| M-5 失败恢复 suppression | **FIXED** | 写回快照前 `suppression.remember(snapshot_hash)`；`WriteSuppression` 改为有界队列。测试：`restoring_the_original_clipboard_suppresses_the_next_capture`。 |
| M-6 edit_clip 假编辑 | **FIXED** | 新增 `Storage::rewrite_clip_text`：重建 CF_UNICODETEXT/CF_TEXT/CF_OEMTEXT payload、content/formats hash、preview、size，删除旧格式并触发 FTS。图片/文件/纯富文本类型拒绝编辑并返回 conflict。测试：`rewrite_clip_text_updates_formats_hashes_and_fts`。 |
| M-7 staged import/migration | **FIXED** | marker 使用 temp+fsync+rename；损坏/失败 marker 改名 `.failed`，启动继续；`get_migration_status` 返回 `phase=failed`，设置页提供 “Clear failed staging marker”。测试：atomic write、corrupt marker、missing package。 |
| M-8 IPC broadcast 阻塞 | **FIXED** | pipe 改为 `FILE_FLAG_OVERLAPPED`；读写均使用 overlapped + event；broadcast 写超时 250 ms，失败即移除/关闭客户端。测试：`broadcast_disconnects_a_client_that_stops_reading`（60 KiB 事件、写入 <3 s、客户端数归零）。 |
| M-9 IPC ACL | **FIXED** | SDDL 仅 `SY` + 当前 SID，移除 `BA`；测试断言无 `;;;BA`，并以伪造 SID 创建 pipe，当前用户 `CreateFileW` 失败。 |
| M-10 搜索/结果数/分页 | **FIXED** | 实现安全 regex（pattern ≤1024 B、compiled/dfa size limit 1 MiB、64 KiB 匹配窗口、缓存）；`Clips.total` 使用真实 `count_clips`；popup 使用 cursor 分页（500/页）并在接近末尾自动加载。测试：regex 有效/非法/超长；真实 SQLite 10k 证据 `paging-10k-real-sqlite.log`（20 页、10000 行、483 ms）。 |
| M-11 Tray 菜单 | **FIXED** | 菜单含 Open/Settings/Groups/Copy Buffers/Connect/Disconnect/Exit；Connect/Disconnect 暂停/恢复 capture；EN/ZH 从 locale JSON 读取；Groups/Copy Buffers 深链到设置页。locale 单测 + 截图；物理 tray click UNVERIFIED。 |
| M-12 WebView2 fallback | **FIXED** | 打开 Settings 前检查 Evergreen 注册表/安装目录；缺失时原生 MessageBox 给出下载 URL，host 继续运行。NSIS 使用 `downloadBootstrapper`；Portable README 新增策略说明。缺失运行时的真实路径 UNVERIFIED。 |
| M-13 explorer.exe 路径 | **FIXED** | 使用 `%WINDIR%\explorer.exe` 绝对路径；单测断言绝对路径且文件名固定。 |
| M-14 popup 冷启动 | **FIXED** | popup 在 Tauri 初始化前启动，prewarm 在 window/renderer 前完成 Hello；app 侧 process-start→hello 0.3–0.7 ms。3 次无重建冷采样：1736.1 / 1707.7 / 1714.5 ms，中位数 **1714.5 ms（<2 s）**；热键→首帧 100 样本 P95 **3.526 ms**。首次运行新构建的未签名 exe 仍可能被 Defender 扫描到 6–8 s（根因见 RISKS）。 |
| M-15 migration progress/cancel | **DEFERRED（产品决策）** | v1 明确为一次性、重启时执行的 staged migration，不提供取消；pending 状态显示 dry-run 得到的总数与“仅重启后可见/不可取消”的 warning，应用前始终写备份。冻结矩阵 D7 已按此更新，移除了未实现的取消承诺。 |

#### Minor / Nit

| 编号 | 状态 | 修复与证据 |
|---|---|---|
| Minor-1 fresh worktree clippy 顺序 | **FIXED** | `REVIEW-GUIDE.md` 与 CI 顺序统一为先构建 sidecar/UI，再 fmt/clippy/test，并写明 `externalBin` 诊断。 |
| Minor-2 二次实例计时 | **FIXED** | `ipc-smoke.ps1` 在 `Start-Process` 前计时，轮询 `HasExited` 记录 wall-clock；release 实测 49.5–110.1 ms。 |
| Minor-3 100 样本/3 次冷采样 | **FIXED** | harness 默认 `ShowCycles=100`；Release 100 样本 P95 3.526 ms；空闲在 show 前采集 3 次并输出中位数（40,480,768 B）。 |
| Minor-4 MockBackend 截图 | **FIXED** | 用真实 Tauri WebView2 重新生成 `artifacts/rewrite/after/screenshots/T9-tauri-settings-real.png` 与 Hotkeys 页截图，无 MockBackend badge。 |
| Minor-5 真实 10k 滚动 | **FIXED** | 真实 SQLite 10k + 真实 IPC provider，300 帧：frame p50 0.674 ms、p95 1.205 ms、max 25.6 ms、>50 ms 帧 0；UI update p95 0.004 ms。文件 `perf/scroll-10k-real-sqlite.csv`。 |
| Minor-6 ZIP 额外文件 | **FIXED** | `verify_no_extra_files` 只允许 manifest.json、ditto.db、SQLite 瞬态 sidecar、manifest 列出的 blobs/thumbs；测试 `undeclared_package_files_are_rejected`。 |
| Nit webview_pid_count | **FIXED** | `ditto_win::webview_process_count()` 用 Toolhelp32 统计当前进程的 `msedgewebview2.exe` 子进程，diagnostics 返回真实值。 |
| Nit 隐藏 250 ms repaint | **FIXED** | 隐藏时改为 1 s 安全唤醒；可见且未激活时 100 ms 轮询外部点击/失焦（另有独立后台 watcher）；长 idle 噪声下降。 |
| Nit 未签名/Defender | **DOCUMENTED** | 4 个二进制仍 unsigned（signtool No signature found）；RISKS 与 QA 报告记录 SmartScreen/Defender 风险，未修改任何安全设置。 |

### 回归/QA

| 指标 | 结果 |
|---|---|
| Release IPC smoke 100 show | 全部 PASS；首帧 P50 1.813 ms / P95 3.526 ms |
| 空闲 3 次冷采样中位数（core+popup private） | 40,480,768 B（≈38.6 MiB） |
| Settings 打开（标题出现） | 79.0 ms（另一次 96.6/152.1 ms） |
| 二次实例退出 wall-clock | 49.5 ms（另一次 80.8/110.1 ms） |
| 真实 SQLite 10k cursor 分页 | 20 页 / 10,000 行 / 483 ms |
| 真实 SQLite 10k 滚动 300 帧 | frame p95 1.205 ms，max 25.6 ms，>50 ms = 0 |
| 冷启动（无重建） | 1736.1 / 1707.7 / 1714.5 ms，中位数 1714.5 ms |
| 本地隐藏/下一次显示/popup 崩溃 | False / True / False（`popup-hide-sync.log`） |
| NSIS/Portable | 安装/卸载/启动/`Data\ditto.db` PASS |

产物（`artifacts/rewrite/after/build/t9-hashes.txt`）：

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `Ditto.exe` | 11,382,272 | `3bf1e6e629ac5027b5b529dd48a93dd399b5552d13e6f35ef2fa8b0372ba66dc` | NotSigned |
| `DittoPopup.exe` | 5,007,872 | `4466e5d903eaae62b6d7219f10d635eedd6642cd98e6180cf6b801384b018323` | NotSigned |
| `DittoUacHelper.exe` | 272,896 | `ec5b7eaf2dd18d6626d939d0d545fcad6475c0963674b85857a53ddf86b52705` | NotSigned |
| `Ditto_0.1.0_x64-setup.exe` | 4,870,054 | `35ee43d7e8b7dfa20204b3b5272b9ed5035d0338bc7c6fb000932f5535c984d5` | NotSigned |
| `Ditto-0.1.0-x64-portable.zip` | 6,855,700 | `c6523ae1f14ff373227ae5d41dca2e0e732c82093bca5d36f9cf4a7cdc0b9304` | 不适用 |

### 迁移 fixture

`cargo test -p ditto-migration` 的 synthetic legacy fixture、失败回滚、导出/导入往返全部 PASS；10k migration benchmark 仍为 ignored。T9 未新增真实旧库读取。

### 安全

- IPC ACL：仅 SYSTEM + 当前 SID；伪造 SID pipe 的 Open 测试失败。
- 慢客户端：60 KiB 事件写入在 3 s 内返回并断开客户端。
- ZIP：拒绝 manifest 未声明文件。
- Explorer：绝对路径，无 PATH/CWD 解析。
- UAC helper 仍只发送 `Ctrl+V`；UAC 提权目标未实测（UNVERIFIED）。

## UNVERIFIED

1. 物理 hotkey → 输入 → Enter/Shift+Enter 的完整链路（自动化只覆盖 IPC/引擎与设置 UI）。
2. 真实 Notepad/Word/浏览器/Explorer 的文本/HTML/RTF/图片/文件粘贴。
3. 真实 UAC 提权目标、取消、超时、secure desktop。
4. 物理 tray 点击与菜单项（代码/本地化已测，未做真实点击录屏）。
5. 缺失 WebView2 Runtime 的真实机器路径（只验证检测函数）。
6. 真实 IME、Narrator、高对比度、混合 DPI、多显示器。
7. Windows 10、真实 Defender/SmartScreen 行为。

## RISKS

- 所有二进制未签名；首次运行新构建的 `DittoPopup.exe` 在 Defender 实时扫描下可延迟 6–8 s。连续冷启动同一二进制约 1.71 s；热键时 popup 已常驻，首帧 P95 3.5 ms。发布前应签名。
- 单实例由 per-user 命名 pipe 强约束；设置中的单实例开关按产品决策 disabled（避免宣称支持第二个 core）。
- M-15 迁移无取消/进度事件，是按冻结矩阵更新的 v1 产品决策；失败通过 `.failed` marker + UI 清理恢复。
- 测试与 QA 只使用临时数据根和合成 fixture；未读取真实 `%APPDATA%\Ditto`、`%LOCALAPPDATA%\Ditto` 或剪贴板历史。
- 未修改 Defender/SmartScreen/防火墙/UAC 或任何系统安全设置。

## NEXT

1. T10 按本文件逐条复审 B-1…Nit，并复核 release 打包与 QA 证据。
2. T10 重点复核：B-1 物理链路、M-11 tray 点击、M-12 缺失 WebView2、M-15 产品决策。
3. 发布前签名二进制，消除 Defender 冷启动方差。
