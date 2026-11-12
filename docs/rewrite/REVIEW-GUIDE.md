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

T7 改为：
- 窗口创建后以 `WS_EX_LAYERED`、alpha 0、`WS_EX_TRANSPARENT` 保持“可见但不可见”；
- 不再发送 `ViewportCommand::Visible(false)`；
- 核心在 `PipeServer` 中记录 popup 可见状态，popup 连接后发送 `GetPopupVisibility` 重放错过的 Show。

审查点：alpha=0 窗口是否仍可能抢占鼠标/Z 序；多显示器与 DPI 下位置是否正确；wl 层是否影响截图工具。
相关证据：`artifacts/rewrite/after/qa/ipc-smoke.log`（12 次 Show 全部产生 first-frame 记录）。

### 3.2 单实例快速路径
`claim_single_instance` 在 Tauri 初始化前创建 `Local\Ditto.v2.<SID>.{settings,toggle,instance}` 三个事件。第二个实例只发信号并 `exit(0)`，实测 1.1-2.3ms。
审查点：事件命名是否与旧 Ditto 冲突；异常退出后事件是否被系统回收；`--settings` 与普通启动的分流。
回退路径：`tauri-plugin-single-instance` 仍保留。

### 3.3 导入/恢复的延迟应用
`import_data`/`restore_db` 只调用 `ditto_migration::verify_package`，把包复制到 `<data_root>.pending-import.zip` 并写 marker；下次启动在 `Storage::open_data_root` 之前由 `pending::apply_staged` 替换数据根。迁移同理。
审查点：marker/包路径位于数据根之外，替换数据根不会误删；失败时 marker 保留；备份目录 `ditto-backups`。

### 3.4 设置映射
T5 `SettingsState` 与 T2 JSON 不完全同构：
- 原生键：`clipboard.max_entries`、`appearance.popup_width`、`groups.*`、`copy_buffers.play_sound` 等；
- 扩展键（持久化但核心暂未消费）：`hotkeys.open_chord`、`hotkeys.first_ten_chords`、`copy_buffers.enabled`、`clipboard.capture_files/images`、`general.startup_message`。
`general.language` 以 `en` / `zh-CN` 存储，UI 层映射 `en-US`。
审查点：是否接受扩展键命名；热键字符串尚未注册为全局热键（T1 dev hotkey 仍为 `Ctrl+Alt+D`）。

### 3.5 存储增量
`ClipQuery` 新增 `kind`、`pinned_only`；`ClipSummary` 新增 `size_bytes`；新增 `count_clips`、`update_clip_text`。
审查点：`summary_columns` 末尾新增列后，所有行解析索引是否一致（已有测试覆盖搜索/详情/pin/copy buffer）；`update_clip_text` 不更新 content_hash，去重语义是否可接受。

## 4. 复现步骤

### 4.1 构建与质量门
```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run build --prefix apps/ditto/ui
cargo build --release -p ditto-popup-app -p ditto-uac-helper-app --locked
pwsh scripts/prepare-sidecars.ps1 -Profile release
cd apps/ditto; npx --no-install tauri build
pwsh scripts/package-portable.ps1 -Profile release
```

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

1. 真实热键 `Ctrl+Alt+D` → popup 首帧（脚本通过 IPC `ShowPopup` 触发，非物理按键路径）。物理按键路径会走同一 `toggle_popup` → `PipeServer::set_popup_visible`。
2. 粘贴到 Notepad/Word/Explorer：文本/HTML/RTF/图片/文件、纯文本、多选；自写抑制；UAC 提权目标。
3. 拖出 `drag_out_clip`（命令已注册，需要鼠标按下并保持，自动化环境无法安全模拟）。
4. IME 中文/日文组合、Narrator、高对比度主题、150%/200% 系统 DPI、多显示器。
5. 托盘菜单 Show/Settings/Exit 的实际点击（代码路径与单实例一致）。
6. Defender/SmartScreen 对未签名二进制的实际提示；未修改任何安全设置。
7. 迁移 UI 的源路径选择：当前为文本框输入，未接原生文件对话框。

## 6. 回滚策略

- 数据层：所有替换数据根的操作都会先通过 `ditto_migration::backup` 备份到 `<data_root 父目录>\ditto-backups`。
- 代码层：`git revert` 单实例事件或 popup 分层窗口改动即可回退到 T3 的 `SW_HIDE` 行为（但会重新引入 popup 不唤醒问题）。
- 打包：NSIS 与 portable 均未签名，回滚只需删除安装目录/解压目录。

## 7. 已知风险摘要

1. Popup 冷启动约 5.9s（本机 release 实测）；期间热键会被 `GetPopupVisibility` 重放，但用户感知为首次热键延迟。
2. 设置窗口标题出现耗时约 42-70ms；WebView2 前端实际渲染完成时间未单独测量（窗口销毁后 WebView2 进程归零沿用 T5 证据）。
3. 设置中的热键字符串尚未实际注册为全局热键；核心仍使用 T1 开发热键。
4. 导入/恢复需要重启应用才生效（UI 文案已说明）。
5. 所有二进制未签名。
