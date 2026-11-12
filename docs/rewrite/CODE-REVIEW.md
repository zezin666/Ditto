# T8 - Ditto Rust/Tauri 独立代码审查

## STATUS

**结论：需修复。**

独立构建、测试、NSIS 构建、Portable 打包与安装/便携冒烟均通过。T7 的“无 Blocker”结论不成立：弹窗键盘与粘贴目标链路、弹窗/核心可见性同步、全局热键配置、保留策略接线存在 Blocker；另有若干 Major 的可用性、安全和数据一致性问题。

本次审查只新增本文件，没有修改源码、配置、测试、旧 C++ 或 `LICENSE`。没有读取真实 `%APPDATA%\Ditto`、`%LOCALAPPDATA%\Ditto` 或剪贴板历史。

## VERDICT

**需修复**

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-review`

## BRANCH

`taohy/rewrite-review`

## HEAD

`b05a28dc7a2cb1c9889b3f8b63b950df7144159e`

基线为 `taohy/rewrite-integration` @ `b05a28d`。本文件未提交，遵循仓库的 Git 操作约束，由编排者决定是否提交。

## FILES

- `docs/rewrite/CODE-REVIEW.md`（本文件）

构建产生的 `target/`、`node_modules/`、`dist/` 和 sidecar staging 均为忽略产物，不属于源码改动。

## BUILD

环境：Windows 11、Rust 1.99.1 MSVC、Cargo 1.99.1、Node 24.13.0、npm 11.6.2、Tauri CLI 2.12.4。

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo fmt --all --check` | PASS | 无输出 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS（准备 sidecar 后） | 全新 worktree 首次执行失败，原因见 Minor-1 |
| `cargo test --workspace --locked` | PASS | 全部 workspace、集成和 doc tests 通过 |
| `npm run build --prefix apps/ditto/ui` | PASS | locales 397+77 key 校验、tsc、Vite 均通过；主 chunk 505.39 kB 警告 |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | release sidecar 构建成功 |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | 生成 target triple 文件名 |
| `npx --no-install tauri build` | PASS | 生成 NSIS 4.28 MiB |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | 生成 Portable ZIP |
| `tools/qa/install-smoke.ps1` | PASS | NSIS 安装/卸载、Portable 启动和 `Data\ditto.db` 均通过 |

全新 worktree 的首次 clippy 失败不是 lint 失败：

```text
resource path `binaries\DittoPopup-x86_64-pc-windows-msvc.exe` doesn't exist
```

执行 debug popup/helper 构建和 `prepare-sidecars.ps1 -Profile debug` 后，clippy 无 warning 通过。

fresh worktree 也未安装 `node_modules`；在 `apps/ditto/ui` 和 `apps/ditto` 分别执行锁文件 `npm ci` 后，前端和 Tauri CLI 命令通过。

## TEST

`cargo test --workspace --locked` 的关键结果：

- `ditto-clipboard`: 7 passed
- `ditto-ipc`: 6 passed
- `ditto-migration`: 10 unit + 3 legacy fixture passed；10k migration benchmark ignored
- `ditto-paste`: 16 passed
- `ditto-popup-app`: 17 passed
- `ditto-storage`: 12 unit + 10k concurrency/bench passed
- `ditto-tauri-lib`: 7 passed
- `ditto-uac-helper`: 5 passed
- `ditto-win`: 2 passed

独立 10k 搜索抽查：

```text
browse      p95 1.425 ms
english     p95 2.033 ms
chinese     p95 1.486 ms
two_char_en p95 1.358 ms
two_char_zh p95 2.299 ms
quick_paste p95 22.966 ms
```

全部低于 50 ms。10k migration benchmark 仍为 ignored，T7 也没有单独给出该基准的 release 数据。

## EVIDENCE

### 本 worktree 构建产物

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `target/release/Ditto.exe` | 10,066,944 | `237C7180AB312A66ABF2313BD8FB02B8AEF0BD97F7A04ADA6F4EA8F0B098F7A6` | NotSigned |
| `target/release/DittoPopup.exe` | 4,964,352 | `0F2630BCCC53E607420FBCF31C0A2F0459A27A7CF50DB7CBB1FBC978EF935EEF` | NotSigned |
| `target/release/DittoUacHelper.exe` | 271,872 | `009BAA2F0CECD1350082DCB26BF00C8D7E6684A88C86D71CE723B82C423FF8D2` | NotSigned |
| `target/release/bundle/nsis/Ditto_0.1.0_x64-setup.exe` | 4,491,659 | `30B51CDBF479546F28214DF79567D4A61D4662C41C8624FA49010C086D610964` | NotSigned |
| `target/release/bundle/portable/Ditto-0.1.0-x64-portable.zip` | 6,333,773 | `87C82E2086C9C3029D914507B026625DAE652FF7A7FE4A6BAE5BC9DCCFB0A3FE` | 不适用 |

哈希与 T7 不同，原因是重新构建及 `BUILD-INFO.json` 的 `built_at_utc` 不同。文件大小与路径结构符合预期。

Fresh Portable ZIP 内容：

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

`BUILD-INFO.json` 的 `git_sha` 为 `b05a28dc7a2cb1c9889b3f8b63b950df7144159e`，`data_root` 为 `<exe_dir>\Data`，target 为 `x86_64-pc-windows-msvc`。

### 独立 IPC/性能抽查

使用临时 `DITTO_DATA_DIR` 和合成 seed，不读取真实用户数据。

| 指标 | 独立结果 | 判断 |
|---|---|---|
| Popup 从 core 启动到 IPC ready | 6,045.9 ms；另一次 9,389.3 ms | T7 的约 6 s 风险可复现，且本次最差约 9.4 s |
| Popup 首帧，100 次 IPC Show | p50 5.220 ms，p95 10.810 ms，max 20.107 ms | PASS，低于 50 ms |
| 空闲 pre-show private bytes | 39,948,288；另一次 48,336,896 | 低于 100 MB，但为单点采样，不是 T7 所称的 3 次冷采样中位数 |
| settings 打开后的 private bytes | 92,876,800 至 93,110,272 | 低于 100 MB |
| settings 标题出现 | 50.3 ms；另一次 66.2 ms | PASS，但只测到标题，不测 WebView2 paint |
| 20 次 show/hide GDI/USER | 17/17 到 17/17 | 无增长 |
| 20 次 show/hide private bytes | 114,290,688 保持不变 | mock/10k 场景，不作为空闲预算 |
| 二次实例“退出时间” | 1.6 ms；另一次 2.1 ms | 数值不可作为真实退出延迟，见 Minor-2 |

10k mock 滚动的 T7 原始数据仍有 2 帧超过 50 ms，最大 137.551 ms；真实 SQLite 数据的 10k 滚动仍未验证。

### Git 与用户工作区

- `taohy/rewrite-review` 在审查前为 clean，HEAD `b05a28d`。
- 用户工作区 `D:\Develop\project\Ditto`：`master` = `origin/master` = `4719eea3e38355473f4cbdd112defccf857bbcee`。
- `git diff` 无 tracked 修改；只有既有未跟踪文件：`.idea/`、`CLAUDE.md`、`DittoSetup/output/`、两个 ORCA prompt 文件。
- `master` reflog 只有 clone 和 2026-11-11 的 `taohy/ui-integration` merge，没有 T8 或 rewrite 推送记录。
- `origin` 只存在本地跟踪引用 `origin/master`，没有 `taohy/rewrite-*` remote-tracking ref。
- 尝试 `git ls-remote` 时 GitHub 443 连接失败，因此远端当前状态无法在线复核；本地证据表明 rewrite 分支未 push。

## FINDINGS

### Blocker

#### B-1 弹窗键盘输入与粘贴目标链路互相冲突

`apps/ditto-popup/src/app.rs:442`

`apps/ditto-popup/src/app.rs:791`

`apps/ditto-popup/src/app.rs:1044`

`crates/ditto-paste/src/engine.rs:155`

`crates/ditto-paste/src/target.rs:50`

复现/影响：hotkey Show 使用 `SW_SHOWNOACTIVATE` 且保留 `WS_EX_NOACTIVATE`，用户不点击时窗口收不到 OS 键盘输入，无法直接输入搜索或按 Enter。用户点击后 `activate_for_user_if_needed` 才把弹窗设为前台，但 task 在真正粘贴时才调用 `TargetBackend::capture()`；此时前台是 `DittoPopup.exe`，`crates/ditto-paste/src/target.rs:65` 又会明确拒绝 Ditto 进程，因此返回 `NoTarget`。弹窗展示时记录的 `previous_foreground` 只用于恢复焦点，没有传给 core 作为粘贴目标。

Owner：T3/T4/T7

建议：在 Show 时捕获并持久化目标 HWND/PID/focus HWND，粘贴时使用该快照；明确设计 hotkey 后直接输入焦点的方案，并增加物理 hotkey、点击、Enter、Shift+Enter 的端到端测试。

#### B-2 弹窗自行隐藏后 core 的可见状态不会同步

`apps/ditto-popup/src/app.rs:474`

`apps/ditto/src-tauri/src/lib.rs:299`

`crates/ditto-win/src/lib.rs:371`

复现/影响：Esc、失焦、外部点击和弹窗自身错误路径只调用本地 `hide_popup()`，没有发送 `Request::HidePopup`，也没有更新 `PipeServer.popup_visible`。下一次 hotkey/tray toggle 根据 stale `true` 计算出 `show=false`，第一次按键只会再次 Hide，用户必须按第二次才看见弹窗。弹窗崩溃后同样可能留下 stale `true`。

Owner：T3/T7

建议：popup 本地隐藏时发送带 request-id 的 Hide 同步，或由 core 监听连接断开/窗口状态并清理；增加 Esc、点击外部、popup 崩溃后下一次 hotkey 的测试。

#### B-3 设置中的全局热键没有注册，默认键也不符合冻结规格

`apps/ditto/src-tauri/src/lib.rs:26`

`apps/ditto/src-tauri/src/lib.rs:91`

`apps/ditto/src-tauri/src/commands/mod.rs:440`

`apps/ditto/src-tauri/src/commands/mod.rs:547`

`apps/ditto/ui/src/pages/HotkeysPage.tsx:109`

复现/影响：host 始终选择一个硬编码开发键 `Ctrl+Alt+D`，不可用时只是启动时改为 `Ctrl+Alt+Shift+D`。Settings 持久化并展示 `Ctrl+\``、plain、first-ten、copy-buffer 等 chord，但 `SettingsChanged` 后没有任何注册/注销逻辑。普通用户修改热键并看到 Apply 成功后，新键永远不会生效；T7 已将其标成 Major，但按冻结的 P1/A4 Must 条款应视为 Blocker。

Owner：T1/T5/T7

建议：由 host 读取 `hotkeys.open_chord` 并注册 `tauri-plugin-global-shortcut`，变更时原子地 unregister/register，失败时回滚设置并向 UI 返回明确冲突错误；未实现的 chord 必须在 UI 禁用并标注。

#### B-4 max entries、expiry 和 purge 只有存储 API，没有运行时接线

`crates/ditto-storage/src/store.rs:552`

`crates/ditto-storage/src/store.rs:568`

`apps/ditto/src-tauri/src/core_service.rs:23`

`apps/ditto/src-tauri/src/lib.rs:156`

复现/影响：`purge_deleted_before` 和 `cleanup_retention` 只被单元测试调用。全 workspace 的 `rg` 没有生产调用点。默认 `max_entries=500`、`expire_days=5` 永远不执行；`clear_history` 只是 soft delete，相关 blob 仍被 `data_path` 引用，因此 `gc_orphan_blobs` 也不会删除。用户清空历史或长期复制后，DB/blob 持续增长，C11/C12 Must 不成立。

Owner：T2/T7

建议：在 core 启动后和定期维护 timer 中执行 retention/purge；对 `dont_auto_delete`、pinned、copy-buffer、global-shortcut 的保护规则保留测试；增加“超过上限、超期、清空后重启”的端到端测试。

### Major

#### M-1 设置页展示的多个开关只持久化，不会改变运行行为

`apps/ditto/src-tauri/src/commands/mod.rs:822`

`apps/ditto/src-tauri/src/core_service.rs:23`

`apps/ditto/src-tauri/src/core_service.rs:584`

`apps/ditto-popup/src/app.rs:596`

`apps/ditto/src-tauri/src/lib.rs:223`

`apps/ditto/ui/src/pages/ClipboardPage.tsx:79`

`apps/ditto/ui/src/pages/AppearancePage.tsx:42`

复现/影响：以下可见控件没有按产品契约生效，或只在 Settings shell 生效：`capture_files`、`capture_images`、`record_source_app`、`show_tray_icon`、`single_instance`、`density`、`lines_per_clip`、`font_scale`、`preview`、`animations`、`always_on_top`、`default_group_id`、`prompt_for_name`、`show_grouped_in_main`、`log_level`。CaptureConfig 只在启动时读取部分 clipboard 字段；popup 的 settings snapshot 只消费 width/height/hide/position/confirm/theme；tray 和 single-instance 路径无条件建立。用户看到 Apply 成功，却没得到行为变化。

Owner：T2/T3/T5/T7

建议：按冻结的 A6/A7/A10 和 C3/C4/C10 逐项接线。未在 v1 实现的控件应 disabled/hidden 并写“coming later”，不能保留 silent no-op。

#### M-2 多选粘贴绕过 T4 的 batch 接口

`apps/ditto-popup/src/app.rs:1044`

`apps/ditto-popup/src/provider.rs:278`

`apps/ditto/src-tauri/src/core_service.rs:203`

复现/影响：`paste_selected` 为每个选中 ID 单独发送 `Paste`，没有把一个 `clip_ids` 数组交给 `PasteEngine::paste_many`。因此 T4 的 `multi_interval_ms`、有序结果、失败停止和中止前缀逻辑不会在真实弹窗路径生效；连续写入还会反复覆盖单一 `WriteSuppression`，可能导致自身写入被重新捕获。

Owner：T3/T4/T7

建议：扩展 provider action 一次发送全部 clip IDs，core 使用 `Request::Paste { clip_ids, .. }`，并在真实 SQLite 上验证三项、失败中止和 suppression。

#### M-3 CF_TEXT/CF_OEMTEXT 使用 UTF-8 lossy，非 ASCII 文本会损坏

`crates/ditto-clipboard/src/decode.rs:25`

`crates/ditto-clipboard/src/lib.rs:350`

复现/影响：Windows `CF_TEXT`/`CF_OEMTEXT` 是 ANSI/OEM code page 数据，不是 UTF-8。当前实现 `String::from_utf8_lossy`，例如 CP936 的 `D6 D0 CE C4` 会变成替换字符，导致预览、hash、去重和粘贴回退文本错误。现有测试只覆盖 ASCII。

Owner：T2

建议：按 `CF_TEXT=CP_ACP`、`CF_OEMTEXT=CP_OEMCP` 使用 Windows code page API 解码，增加 CP936/CP1252/CP437 fixture；HTML/RTF 也要按各自声明的 charset/codepage 处理。

#### M-4 clipboard 上限在完整复制后才检查，且只按单个格式限制

`crates/ditto-clipboard/src/windows.rs:425`

`crates/ditto-clipboard/src/windows.rs:78`

`crates/ditto-clipboard/src/lib.rs:202`

复现/影响：`read_global_format` 先按 `GlobalSize` 把所有字节复制到 `Vec`，之后 `build_captured_clip` 才比对 `max_clip_bytes`/`max_image_bytes`。最多允许 1024 个格式，且限制是逐格式而非整次 capture 总量，因此大量大格式可造成峰值内存爆炸，限制并不能阻止 OOM。

Owner：T2

建议：读取前检查 `GlobalSize` 和剩余总预算；设置整次 capture 的总字节上限、总格式数上限和单格式上限；超限直接跳过且不分配大 buffer。

#### M-5 失败恢复旧剪贴板会重新捕获原内容

`crates/ditto-paste/src/engine.rs:166`

`crates/ditto-paste/src/engine.rs:207`

`crates/ditto-clipboard/src/lib.rs:151`

复现/影响：`restore_clipboard_on_failure` 时先把原 clipboard 存快照，随后 remember 目标 payload hash 并写入 payload；失败后再写回原快照，但 suppression 仍指向 payload hash。下一次 capture 的 hash 是原快照，不匹配，原内容会被重新插入历史。这会影响 C8/X9/X10 的自写抑制和用户数据一致性。

Owner：T4/T2

建议：写回快照前调用 `suppression.remember(snapshot_hash)`，或让一次 paste transaction 维护可匹配的 write sequence；增加失败恢复后不新增 clip 的测试。

#### M-6 edit_clip 只改文本列，原始格式仍是旧内容

`apps/ditto/src-tauri/src/commands/mod.rs:990`

`crates/ditto-storage/src/store.rs:643`

`crates/ditto-paste/src/payload.rs:62`

复现/影响：Settings 的历史编辑更新 `text_content/preview/quick_paste_text/description`，但不更新 `content_hash`、`formats_hash`，也不更新 `CF_UNICODETEXT` 原始 format。用户搜索看到新文本，但 Original paste 仍从旧 raw format materialize；去重仍按旧 hash。

Owner：T2/T5/T7

建议：将 edit 设计为明确的 payload rewrite，同步 text format、hash 和 FTS；或者隐藏编辑控件，直到 v1 支持该语义。

#### M-7 staged import/migration 失败会阻止后续启动且无恢复入口

`apps/ditto/src-tauri/src/lib.rs:177`

`apps/ditto/src-tauri/src/pending.rs:51`

`apps/ditto/src-tauri/src/pending.rs:64`

`apps/ditto/src-tauri/src/commands/mod.rs:1495`

复现/影响：marker 直接 `fs::write`，不是 temp+rename；marker 或包损坏、迁移目标临时不可用、替换失败时，`apply_staged` 返回 Err，Tauri setup 直接失败，marker 又被保留。应用没有 Settings/恢复 UI 可清除失败 marker，用户只能手工删除文件。崩溃/断电也可能留下半写 marker。

Owner：T6/T7

建议：原子写 marker；启动失败时将其改名为 `.failed` 并继续使用旧数据，记录可恢复状态；提供命令行/UI recovery，不要用启动失败作为失败标记机制。

#### M-8 IPC broadcast 在阻塞管道上同步写所有客户端

`crates/ditto-win/src/lib.rs:351`

`crates/ditto-win/src/lib.rs:476`

复现/影响：server 使用 `PIPE_WAIT` 和普通 `WriteFile`，`broadcast` 对每个客户端逐个同步写入且无 timeout。任一客户端停止读取并填满 pipe buffer，就会阻塞当前 capture callback、settings command 或 hotkey worker；这不仅是延迟问题，还可被同用户进程用于本地 DoS。

Owner：T1/T2/T7

建议：为 pipe 使用 overlapped I/O 或独立发送队列和写超时；慢客户端应被断开而不是阻塞 core；增加不读取客户端的 fault-injection 测试。

#### M-9 主 IPC ACL 额外允许 Builtin Administrators 读取

`crates/ditto-win/src/lib.rs:397`

复现/影响：SDDL 为 `D:P(A;;GA;;;SY)(A;;GA;;;<sid>)(A;;GR;;;BA)`。`BA` 让不同用户的管理员组获得 generic read，与任务要求的“IPC 仅当前用户/SYSTEM”不一致；pipe 会广播 clip summary 和事件，可能把内容暴露给非当前用户。

Owner：T1/T2/T7

建议：移除 `BA` ACE，仅保留 SYSTEM 和当前 SID；安全测试应枚举非当前 SID 并验证 Open 失败。

#### M-10 搜索模式、结果数和真实 10k 分页不满足冻结验收

`crates/ditto-storage/src/store.rs:391`

`crates/ditto-storage/src/store.rs:455`

`apps/ditto-popup/src/provider.rs:213`

`apps/ditto-popup/src/app.rs:698`

复现/影响：Regex 明确返回 unsupported，UI/provider 始终发送 `Contains`，S5 Must 未实现；`Clips.total` 是当前页行数，并非总数，provider 丢弃 `total_is_estimate`；popup 固定请求 1000 条且不使用 `next_cursor`，超过 1000 条的真实历史不可浏览，UI 仍显示 1000 作为总数。T7 的 10k 滚动证据来自 10000 条 mock，不是这条真实 IPC 路径。

Owner：T2/T3/T7

建议：明确 v1 regex 产品决策并更新冻结矩阵，或实现安全 regex；返回真实 count 或明确 estimate；provider 实现 cursor 分页并在真实 SQLite 10k 数据上测滚动和结果数。

#### M-11 Tray 菜单不完整且未本地化

`apps/ditto/src-tauri/src/lib.rs:252`

`apps/ditto/src-tauri/src/lib.rs:371`

复现/影响：A1 Must 要求 Open/Settings/Connect/Disconnect/Groups/Copy Buffers/Pause/Exit，当前只有 Show/Settings/Exit；菜单和 Settings 标题硬编码英文，中文语言下不会切换。Connect/Disconnect/Pause 等缺失路径也没有 disabled 说明。

Owner：T1/T5/T7

建议：补齐 v1 菜单项并接 locale；未实现的项 disabled 或移除。增加真实 tray click 和 EN/ZH 截图/键盘验证。

#### M-12 WebView2 缺失时没有 actionable fallback，Portable 也没有策略说明

`apps/ditto/src-tauri/src/lib.rs:371`

`target/release/nsis/x64/installer.nsi:56`

生成的 NSIS 使用 `downloadBootstrapper`，缺 WebView2 时需要联网下载；Portable ZIP 不含 bootstrapper，缺 WebView2 时 Settings 创建失败，只会写日志，没有原生用户提示。A12 Must 未满足。

Owner：T1/T5/T7

建议：在打开 Settings 前检测 WebView2，失败时显示原生 MessageBox/下载说明；明确 NSIS 和 Portable 的 WebView2 策略并写入 README/QA。

#### M-13 `explorer.exe` 使用 PATH 查找，可被当前目录劫持

`apps/ditto/src-tauri/src/commands/mod.rs:782`

复现/影响：`Command::new("explorer.exe")` 不使用绝对路径。Windows executable search order 包含当前目录；从可写目录启动时，恶意 `explorer.exe` 可能被当作系统 Explorer 启动，违背“无任意命令/路径执行”的意图。

Owner：T5/T7

建议：使用 `%WINDIR%\explorer.exe` 绝对路径，或直接调用 shell API；不要从 CWD/PATH 解析系统工具。

#### M-14 popup 冷启动约 6 至 9.4 秒，T7 的 Major 风险成立

`apps/ditto/src-tauri/src/lib.rs:312`

`apps/ditto-popup/src/main.rs:17`

复现/影响：两次独立 IPC smoke 的 popup ready 分别为 6,045.9 ms 和 9,389.3 ms。期间首次 hotkey 会被 visibility replay 补发，但用户仍感知为明显延迟。首帧预算在 ready 后通过，因此问题不是首帧，而是冷启动就绪。

Owner：T3/T7

建议：保留 Major 等级；评估预热、持久化 shader/GL 缓存、渲染后端或启动时机；在安装后/登录后自动预热并在 UI 中避免宣称即时可用。

#### M-15 Migration progress/cancel 没有真实实现

`apps/ditto/src-tauri/src/commands/mod.rs:1419`

复现/影响：`get_migration_status` 主要返回 marker/settings 静态状态，processed/total 常为 0；D7 Must 要求的 progress/cancel/rollback UI 和事件链路没有出现在本 review 的源码中。用户只能看到 pending-restart 或 completed，长迁移没有进度，也没有取消入口。

Owner：T6/T7

建议：实现可观察的 migration state/event，至少提供 processed/total、失败原因和取消后的回滚结果；若 v1 决定只支持一次性迁移，应更新冻结矩阵并移除未实现的承诺。

### Minor

#### Minor-1 文档顺序下 clippy 在 fresh worktree 不能直接运行

`docs/rewrite/REVIEW-GUIDE.md:83` 把 clippy 放在 sidecar 准备之前，`apps/ditto/src-tauri/tauri.conf.json:20` 又把 externalBin 设为必需输入。`CLAUDE.md:10` 的顺序是正确的，但 T8 任务指定的质量门顺序仍会在 fresh worktree 失败；建议文档和 CI 统一先构建 popup/helper 并准备 sidecar，或在 build script 中提供可诊断的前置检查。

#### Minor-2 T7 的二次实例 1 ms 数字不是实际退出延迟

`tools/qa/ipc-smoke.ps1:219`

计时器在 Settings 标题被观察到后才启动 `WaitForExit`；第二个进程可能早已退出，因此 1.6/2.1 ms 只是残余等待。实现本身是 Tauri 前快速退出，但报告应改成从 `Start-Process` 前到 `HasExited` 的 wall-clock 或 Process ExitTime。

#### Minor-3 T7 的 12 样本 p95 方法不足，而且空闲预算不是 3 次冷采样中位数

`tools/qa/ipc-smoke.ps1:196`

`tools/qa/ipc-smoke.ps1:249`

T7 只有 12 个 Show 样本，最新 harness 默认也是 12；冻结的 Q6 要求 100 样本。本审查补跑了 100 样本并得到 p95 10.810 ms，说明实现可通过，但 T7 报告的方法仍需修正。pre-show 空闲只采样一次；后续 3 次采样发生在 Settings 打开、popup 显示和 CJK 加载之后。

#### Minor-4 原生 Settings 截图仍是 T5 的 MockBackend 版本

`docs/rewrite/screenshots/T5/tauri-settings-window-100.png`

该图仍显示 `MockBackend` 和 synthetic-only 文案，与 T7 “移除 MockBackend”说法冲突。当前浏览器截图没有该 badge，源码也没有 MockBackend，但 T7 应重新生成真实的 Tauri Settings 截图，避免把旧证据当作集成证据。

#### Minor-5 10k 真实滚动仍不可判定

`artifacts/rewrite/after/perf/scroll-10k-release-final2.csv`

已有 release mock 数据有 2 帧超过 50 ms，最大 137.551 ms；真实 SQLite/provider 路径没有相同 telemetry。建议锁定桌面后用 PresentMon 或等价证据验证，至少记录帧间隔、UI update 时间和内存曲线。

#### Minor-6 ZIP 验证允许 manifest 未列出的额外文件

`crates/ditto-migration/src/package.rs:327`

`verify_package_dir` 校验 manifest 中的文件和引用，但不拒绝包根目录的额外文件。导入时会复制整个 staging，恶意或损坏包可携带未校验内容。当前 16 GiB/100k entry 上限限制了解压爆炸，但建议只允许 manifest + DB + blobs/thumbs。

### Nit

- `apps/ditto/src-tauri/src/commands/mod.rs:1564` 把 `webview_pid_count` 固定为 0，不能作为 WebView2 生命周期诊断依据。
- `apps/ditto-popup/src/app.rs:2104` 隐藏时每 250 ms 请求 repaint，功能上用于唤醒，但会增加空闲 GPU/CPU 采样噪声；应在长期 idle 测试中单独记录。
- 未签名二进制和 SmartScreen/Defender 行为仍是发布风险；本次没有修改任何安全设置。

## UNVERIFIED

1. 真实物理 hotkey 到可输入/可粘贴的完整链路。
2. Notepad/Word/浏览器/Explorer 的真实文本、HTML、RTF、图像和文件粘贴。
3. 真实 UAC 提权目标、取消、超时和 secure desktop。
4. OLE drag-out 到 Explorer/Word 的交互和失败回滚。
5. 真实 IME、Narrator、高对比度、混合 DPI、多显示器。
6. 真实 SQLite 10k popup 滚动和 frame timing。
7. Windows 10 行为、WebView2 缺失安装路径、stale autostart path 修复。
8. Defender/SmartScreen/未签名二进制的实际行为。
9. GitHub 远端当前状态；本次 `git ls-remote` 网络失败。

## RISKS

- 本次独立 smoke 只使用临时数据根和合成 fixture，没有读取真实用户剪贴板或 Ditto 数据。
- 用户 master 工作区有既有未跟踪文件，但 tracked diff 为空；本审查没有改动用户 master。
- 构建、安装和 portable smoke 使用了当前 Windows 会话；未修改防火墙、Defender、SmartScreen、UAC 或系统安全设置。
- `Ditto.exe`、popup、helper 和 NSIS 均未签名，发布前仍可能触发系统警告。

## NEXT

1. 按 Blocker 从 B-1 到 B-4 修复，并由 owner 分别提交修复证据。
2. 修复 Major 中的设置接线、CF_TEXT 解码、大对象 cap、失败恢复、edit payload、retention/purge、IPC ACL/广播和 tray 菜单。
3. 重新生成真实的 Tauri Settings 截图、100 样本 popup 指标、真实 SQLite 10k 滚动和物理交互证据。
4. 创建 T9 复审，只复查修复项和回归面；在 T9 通过前不应合并到用户 master。
