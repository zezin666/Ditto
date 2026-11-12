# T7 Review Guide - T8 独立审查入口

本文件面向 T8 审查者，汇总 T7 集成后的入口、风险点、复现步骤与仍需人工验证的项目。
所有 QA 证据位于 `artifacts/rewrite/after/`，完整结果见 `docs/rewrite/QA-REPORT.md`。

## 1. 分支与提交

| 项目 | 值 |
|---|---|
| 工作树 | `D:\orca\workspaces\Ditto\rewrite-integration` |
| 分支 | `taohy/rewrite-integration` |
| 基线 | `rewrite/tauri` @ `847f966` |
| 合并顺序 | T2 `7069fcc` → T4 `e7c30c3` → T6 `a056434` → T3 `0bae4ac` → T5 `221ca43` |
| 未推送 | 是（未 push、未合并 master） |

每个阶段均使用 `git merge --no-ff`。T5 合并时 `apps/ditto/src-tauri/src/lib.rs` 出现文本冲突，已按 Architecture 约束手工合并。

## 2. 审查入口

优先阅读：

1. `apps/ditto/src-tauri/src/lib.rs`
   - `AppState`：pipe + storage + core 共享。
   - `claim_single_instance`：Win32 命名事件快速单实例路径。
   - `setup`：先应用 pending import/migration，再打开 Storage 和 CoreService。
2. `apps/ditto/src-tauri/src/commands/mod.rs`
   - 全部 Tauri 命令的真实实现；无 `MockBackend`。
   - `settings_state` / `settings_value`：T5 设置与 T2 JSON 设置的映射。
   - `stage_package`：import/restore 仅暂存，不替换活动数据库。
3. `apps/ditto/src-tauri/src/pending.rs`
   - `apply_staged`：下次启动时在 Storage 打开前应用导入/迁移。
4. `apps/ditto/src-tauri/src/core_service.rs`
   - `CoreService::paste` / `drag_out`：设置窗口复用 T4 粘贴与拖出。
5. `apps/ditto-popup/src/provider.rs`
   - 真实 IPC provider：分页、分组、格式粘贴、copy buffer。
6. `apps/ditto-popup/src/app.rs`
   - 分层 alpha=0 常驻窗口（约 2500 行附近的 `set_popup_visible` / `enforce_hidden_unless_requested`）。
   - `connect_and_read`：Show/Hide 事件、`GetPopupVisibility` 重放。
   - `install_fonts`：CJK 字体按需加载。
7. `crates/ditto-win/src/lib.rs`
   - `SingleInstanceSignal`、`PipeServer::set_popup_visible`、`GetPopupVisibility`。
   - `make_popup_window` / `set_popup_visible`：WS_EX_LAYERED 常驻窗口。
8. `crates/ditto-storage/src/store.rs`
   - `count_clips`、`update_clip_text`、`append_kind_pin_filters`、`size_bytes` summary 列。

## 3. 高风险区域（请重点审查）

### 3.1 Popup 常驻窗口内存/可见性
T3 原本用 `with_visible(false)` + `ShowWindow(SW_HIDE)`。Windows 下 winit 不会给隐藏窗口派发 redraw，导致 IPC `ShowPopup` 事件无法唤醒 egui（实测冷启动约 5.9s 内事件会丢失，之后也永远不更新）。

T7/T11 改为：
- 窗口创建后以 `WS_EX_LAYERED`、alpha 0、`WS_EX_TRANSPARENT` 保持“可见但不可见”；
- 不再发送 `ViewportCommand::Visible(false)`；
- 核心在 `PipeServer` 中记录 popup 可见状态，popup 连接后发送 `GetPopupVisibility` 重放错过的 Show；
- **T11 hotkey 路径**：Show 时核心先快照前台窗口，popup 随后清除 `WS_EX_NOACTIVATE`、用 `AttachThreadInput` + `SetForegroundWindow`/`SetFocus` 取得键盘焦点，因此 hotkey 后可直接键入、Enter/Shift+Enter；Esc/外部选择隐藏时恢复目标焦点，失焦/外部点击隐藏则保留新前台窗口。

审查点：alpha=0 窗口是否仍可能抢占鼠标/Z 序；多显示器与 DPI 下位置是否正确；wl 层是否影响截图工具；激活是否在少数受保护桌面上失败。
相关证据：`tools/qa/keyboard-first.ps1`（Show → 无鼠标键入 → Enter/Shift+Enter → 粘贴到受控目标窗口）；`tools/qa/ipc-smoke.ps1`（100 次 Show first-frame）。

### 3.2 单实例快速路径
`claim_single_instance` 在 Tauri 初始化前创建 `Local\Ditto.v2.<SID>.{settings,toggle,instance}` 三个事件。第二个实例只发信号并 `exit(0)`，实测 1.1-2.3ms。
T11 起支持 `DITTO_PIPE_SUFFIX`/`DITTO_INSTANCE_ID`：pipe 变为 `\\.\pipe\ditto-<SID>-<suffix>`，事件名追加相同后缀；QA 默认使用唯一后缀，不会命中用户的 Ditto 进程。
T13 起 suffix 采用 fail-closed：变量存在时必须是 1–48 位 `[A-Za-z0-9_-]`；空、仅空白、超长或含非法字符时核心/弹窗以退出码 2 拒绝启动，绝不回退生产 pipe 或生产单实例事件。
审查点：事件命名是否与旧 Ditto 冲突；异常退出后事件是否被系统回收；`--settings` 与普通启动的分流。
回退路径：`tauri-plugin-single-instance` 在无后缀的正常实例中保留；QA 实例使用后缀事件。

### 3.3 导入/恢复的延迟应用
`import_data`/`restore_db` 只调用 `ditto_migration::verify_package`，把包复制到 `<data_root>.pending-import.zip` 并写 marker；下次启动在 `Storage::open_data_root` 之前由 `pending::apply_staged` 替换数据根。迁移同理。
审查点：marker/包路径位于数据根之外，替换数据根不会误删；失败时 marker 保留；备份目录 `ditto-backups`。

### 3.4 设置映射
T5 `SettingsState` 与 T2 JSON 不完全同构：
- 原生键：`clipboard.max_entries`、`appearance.popup_width`、`groups.*`、`copy_buffers.play_sound` 等；
- 已消费的扩展键（T9/T11/T13）：`hotkeys.open_chord`、`hotkeys.open_alt_chord`、`hotkeys.plain_text_paste_chord`、`hotkeys.first_ten_enabled/chords`、`hotkeys.copy_buffers[*].copy/paste`、`hotkeys.startup_conflicts`、`clipboard.capture_files/images`、`general.show_tray_icon`、`general.startup_message`、`groups.show_grouped_in_main`、`diagnostics.log_level`；
- 仍仅为持久化/占位：`copy_buffers.enabled` 等（对应控件 disabled，矩阵注明）；`general.startup_message` 自 T13 起由原生非激活提示实现。
`general.language` 以 `en` / `zh-CN` 存储，UI 层映射 `en-US`。
审查点：hotkey 的全局/弹窗作用域（open/open_alt/plain/copy-buffer 为全局；first-ten 仅在 popup 可见时生效）；`show_grouped_in_main=false` 时主列表映射为 ungrouped；主题值 `system/light/dark/highContrast` 与旧 `hc/high_contrast` 的兼容。

### 3.5 存储增量
`ClipQuery` 新增 `kind`、`pinned_only`；`ClipSummary` 新增 `size_bytes`；新增 `count_clips`、`update_clip_text`。
审查点：`summary_columns` 末尾新增列后，所有行解析索引是否一致（已有测试覆盖搜索/详情/pin/copy buffer）；`update_clip_text` 不更新 content_hash，去重语义是否可接受。

## 3.6 T9 修复入口

- `docs/rewrite/T9-FIX-REPORT.md`：CODE-REVIEW B-1…Nit 的逐条状态与证据索引。
- `apps/ditto/src-tauri/src/hotkeys.rs`：chord 解析、冲突探测、原子 swap/回滚。
- `apps/ditto-popup/src/app.rs`：Show 前目标快照、本地 Hide 同步、后台外部点击 watcher、cursor 分页。
- `apps/ditto/src-tauri/src/core_service.rs`：显式目标粘贴、retention timer、暂停/捕获重配置。
- `crates/ditto-win/src/lib.rs`：overlapped IPC 写超时、慢客户端断开、popup 崩溃可见性清理、ACL。
- `tools/qa/popup-hide-sync.ps1`、`tools/qa/paging-check.ps1`：T9 新增的可重复 QA。

## 3.7 T11 修复入口

- `docs/rewrite/T11-FIX-REPORT.md`：CODE-REVIEW-2 R-1…R-10 逐条状态、证据与残余边界。
- `apps/ditto-popup/src/{app.rs,platform.rs,provider.rs,options.rs}`：hotkey 键盘焦点、主题映射、分组主列表、真实 IPC scroll harness（`--provider=ipc`、`--bench-warmup`）。
- `apps/ditto/src-tauri/src/{hotkeys.rs,logging.rs,lib.rs,core_service.rs}`：全局 hotkey 集合注册/回滚、可重载日志过滤、Show 前目标 hook、copy-buffer 全局动作。
- `crates/ditto-paste/src/clipboard.rs`：`GlobalUnlock` 语义修复（实际写入路径此前全部失败）。
- `crates/ditto-clipboard/src/decode.rs`：ANSI/OEM 文本 NUL 终止。
- `tools/qa/{keyboard-first.ps1,hotkey-rebind.ps1,scroll-10k-ipc.ps1,grouped-main.ps1,theme-modes.ps1,cold-start.ps1}`：T11 新增/更新的隔离 QA。

## 3.8 T13 修复入口

- `docs/rewrite/T13-FIX-REPORT.md`：CODE-REVIEW-3 的 R2-1、R2-2、QIS-1、R7-1 逐条状态、证据、残余风险。
- `apps/ditto/src-tauri/src/hotkeys.rs`：`bindings_with_issues`（启动容错解析）、`register_all_collecting`（逐项注册）、`reconcile_settings`（写回 effective 集合 + `hotkeys.startup_conflicts`）。
- `apps/ditto/src-tauri/src/lib.rs`：启动时逐项注册、open chord fallback、把冲突/清空写回存储；tray 隐藏时的原生非激活提示；`general.startup_message` 提示；冷启动 trace 点。
- `apps/ditto/ui/src/pages/HotkeysPage.tsx` + locales：Settings 显示启动冲突原因；copy-buffer `cut` disabled 且文案改为“v1 暂不支持/计划后续”。
- `crates/ditto-win/src/lib.rs`：`parse_instance_suffix`/`instance_suffix` fail-closed；`show_native_hint` 原生分层提示窗口。
- `apps/ditto-popup/src/main.rs`：suffix 校验；`run-native-enter`、`ipc-prewarm-returned` trace。
- `apps/ditto/src-tauri/src/core_service.rs`：retention 首轮延后 10 s，避开冷启动窗口。
- `tools/qa/{suffix-validation.ps1,cold-start-feedback.ps1,hotkey-rebind.ps1,cold-start.ps1}` 与 `suffix-common.ps1`：T13 新增/更新的隔离 QA。

T13 的冷启动结论：代码可控部分（setup、hotkey、tray）约 60 ms；剩余时间由
未签名进程启动/扫描（约 1.8 s，host 与 popup 重叠）和 eframe 窗口/GL 初始化
（约 1.7 s）构成。`host→Show <2.0 s` 在当前未签名、独立 popup 进程架构下未达标，
已作为 DEFERRED 记录，并由原生提示覆盖隐藏 tray 的可见反馈。

## 3.9 T15 修复入口

- `docs/rewrite/T15-FIX-REPORT.md`：CODE-REVIEW-4 的 P-1、P-2、Minor-1、Minor-2
  逐条状态、证据与残余边界。
- `apps/ditto/src-tauri/src/lib.rs`：`run()` 先 claim 单实例，再应用
  `pending::apply_staged`，然后才读取启动设置；`setup` 打开 Storage 后重新读取
  设置并据此重建 hotkey/tray/language/startup message；旧快照不参与 reconcile。
  `toggle_popup` 改用 `popup_window_ready()` 判断是否需要 preparing 提示。
- `crates/ditto-ipc/src/lib.rs`：新增 `Request::PopupWindowReady`。
- `crates/ditto-win/src/lib.rs`：`PopupPresence`（visible/clients/window_ready）、
  `PipeServer::popup_window_ready`、`dismiss_native_hint`；popup 断开时重置
  window ready。
- `apps/ditto-popup/src/app.rs`：`connect_and_read` 在窗口创建后发送
  `PopupWindowReady` 并写 `window-ready-ack` trace。
- `tools/qa/pending-import-startup.ps1`：合成导出包 + 旧设置目标，验证导入值
  不被旧快照覆盖。
- `tools/qa/popup-ready-feedback.ps1`：等待 `hello-ack-prewarm` 后直接信号
  toggle event，断言 hint 在 window ready 前可见、ready 后关闭、不抢焦点。
- `tools/qa/popup-hide-sync.ps1`：popup 进程筛选改为当前 worktree 的
  `$DittoExe` 目录 + 启动时间，不再硬编码 `rewrite-fixes\target`。

## 4. 复现步骤

### 4.1 构建与质量门
```powershell
# Fresh worktree: build sidecars before any clippy/test that compiles the Tauri app.
cargo build -p ditto-popup-app -p ditto-uac-helper-app
pwsh scripts/prepare-sidecars.ps1 -Profile debug
npm ci --prefix apps/ditto/ui
npm run build --prefix apps/ditto/ui
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh scripts/prepare-sidecars.ps1 -Profile release
cd apps/ditto; npx --no-install tauri build
pwsh scripts/package-portable.ps1 -Profile release
```

`apps/ditto/src-tauri/tauri.conf.json` lists `externalBin` as a required input;
`cargo clippy`/`cargo build` on a fresh worktree fail with
`resource path binaries\DittoPopup-x86_64-pc-windows-msvc.exe doesn't exist`
until the sidecar staging step above runs. CI uses the same order.

### 4.2 合成 IPC/性能冒烟
```powershell
pwsh -File tools/qa/ipc-smoke.ps1 -DittoExe target/release/Ditto.exe
```
使用临时 `DITTO_DATA_DIR`、合成 `seed_qa` fixture；不读取真实剪贴板。
脚本输出：pipe 建立、真实 Get/Search/Create/Rename/Delete、copy buffer、Show/Hide 12 次、settings 打开/二次实例退出、私有工作集、popup first-frame p50/p95。

### 4.3 迁移 fixture
```powershell
cargo test -p ditto-migration --test legacy_fixture --locked -- --nocapture
```
覆盖 dry-run、apply、计数/格式/置顶/copy buffer 对齐、旧库 SHA-256 不变、导出/导入往返、失败回滚。

### 4.4 安装/卸载与 portable
见 QA-REPORT 中的实测路径与哈希。安装使用 `target/release/bundle/nsis/Ditto_0.1.0_x64-setup.exe /S /D=<temp>`；portable 解压 ZIP 后直接运行。

## 5. 仍需人工/交互验证（UNVERIFIED）

以下项目在本次自动化环境中无法可靠执行，T8 需在解锁桌面、可交互的 Windows 会话中复核：

1. 真实物理 hotkey 按键（T11 用 IPC `ShowPopup` + 合成 WM_CHAR/WM_KEYDOWN 覆盖了同一条 Show→激活→键入→Enter/Shift+Enter→粘贴路径；物理按键只差 global-shortcut 事件源）。
2. 粘贴到 Notepad/Word/Explorer 的真实应用：T11 在受控 Win32 EDIT 目标上验证了文本落地；富文本/图片/文件/多选仍需人工。
3. 拖出 `drag_out_clip`（命令已注册，需要鼠标按下并保持，自动化环境无法安全模拟）。
4. IME 中文/日文组合、Narrator、高对比度主题的系统级可读性、150%/200% 系统 DPI、多显示器。
5. 托盘菜单 Show/Settings/Exit 的实际物理点击（代码路径与单实例一致）；T11 验证了 hotkey 注册/动作与 IPC。
6. Defender/SmartScreen 对未签名二进制的实际提示；未修改任何安全设置。
7. 迁移 UI 的源路径选择：当前为文本框输入，未接原生文件对话框。

## 6. 回滚策略

- 数据层：所有替换数据根的操作都会先通过 `ditto_migration::backup` 备份到 `<data_root 父目录>\ditto-backups`。
- 代码层：`git revert` 单实例事件或 popup 分层窗口改动即可回退到 T3 的 `SW_HIDE` 行为（但会重新引入 popup 不唤醒问题）。
- 打包：NSIS 与 portable 均未签名，回滚只需删除安装目录/解压目录。

## 7. 已知风险摘要

1. Popup 冷启动（同一 binary）：popup 进程 start→Show 约 1.76–1.77 s；host 进程 start→Show 约 3.51–3.54 s。首次新构建/新拷贝 host→Show 4.72 s（Defender 扫描未签名 host）。popup 已常驻时热键→首帧 p95 约 2.6–6.9 ms。
2. 设置窗口标题出现耗时约 84 ms；WebView2 前端实际渲染完成时间未单独测量（窗口销毁后 WebView2 进程归零沿用 T5 证据）。
3. T11 起 open/open_alt/plain/copy-buffer 热键按设置注册；first-ten 只在 popup 可见时生效。抄本/剪贴板里的 cut 不提供全局热键（v1 产品决策，见 FEATURE-MATRIX A4）。
4. 导入/恢复需要重启应用才生效（UI 文案已说明）。
5. 所有二进制未签名。
6. popup 首次加载 CJK 文本会加载一个系统 CJK 字体：预展示空闲约 40 MB，加载后 core+popup private 约 93 MB（ASCII-only 约 55 MB），仍低于 100 MB 预算但已接近上限。
