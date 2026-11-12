# T7 - 集成、全量 QA 与打包报告

## STATUS

集成与自动化 QA 完成。五个分支已按 T2 → T4 → T6 → T3 → T5 顺序以 `--no-ff` 合并；T5 UI 不再包含 MockBackend；真实存储、迁移、IPC、打包链路已接通。交互式桌面项目（真实粘贴、UAC 提权、IME、系统 DPI、拖出、托盘点击）在本次自动化环境中无法可靠执行，按 UNVERIFIED 列明。没有 Blocker。

## WORKTREE

`D:\orca\workspaces\Ditto\rewrite-integration`

## BRANCH

`taohy/rewrite-integration`（未 push、未合并 master）

## HEAD

- 代码与工具链 HEAD：`8fa9ad14b29d9d0c1e4145d2637f51b5bb7a3d89`
- 提交说明：`fix: popup IPC wake, single-instance fast path, and T7 QA tooling`
- 合并历史：

| 顺序 | 分支 | 合并前 SHA | 合并提交 |
|---|---|---|---|
| 1 | `taohy/rewrite-core` (T2) | `7069fcc8ef14fa16f3d4f779886c1de9c9ca0009` | `4ddf0d2` |
| 2 | `taohy/rewrite-paste` (T4) | `e7c30c33282d2997a1c5e422a9a7ed69d7c92500` | `2259e8c` |
| 3 | `taohy/rewrite-migration` (T6) | `a056434cddda7f62f73eec6200f008d5fa9bcbe2` | `71a5461` |
| 4 | `taohy/rewrite-popup` (T3) | `0bae4ac24f5239709998e3a42b49d985849700bd` | `f7322c6` |
| 5 | `taohy/rewrite-settings` (T5) | `221ca435273401f96ff1c1a1790e0a452284061c` | `8fd03fa` |

工作区在最终提交后为干净状态。

## 环境

| 项目 | 值 |
|---|---|
| OS | Windows 11 家庭中文版 10.0.26200 build 26200 |
| Locale | zh-CN |
| Rust | rustc 1.99.1 (58cf8f9c3 2026-10-13), MSVC |
| Cargo | 1.99.1 |
| Node / npm | v24.13.0 / 11.6.2 |
| PowerShell | 7.6.6 |
| Tauri CLI | 2.12.4（`apps/ditto` 本地 `@tauri-apps/cli`） |
| 前端 | React 19.3.1, Fluent UI 9.75.2, Vite 7.3.8, TypeScript 5.9.3 |

## BUILD

全部通过：

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo fmt --all --check` | PASS | `artifacts/rewrite/after/build/cargo-fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | `artifacts/rewrite/after/build/cargo-clippy.log` |
| `cargo test --workspace --locked` | PASS | `artifacts/rewrite/after/build/cargo-test.log` |
| `npm run build --prefix apps/ditto/ui` | PASS | `artifacts/rewrite/after/build/npm-build.log` |
| `cargo build --release -p ditto-popup-app -p ditto-uac-helper-app --locked` | PASS | `artifacts/rewrite/after/build/release-build.log` |
| `pwsh scripts/prepare-sidecars.ps1 -Profile release` | PASS | 同上 |
| `npx tauri build` | PASS | 同上 |
| `pwsh scripts/package-portable.ps1 -Profile release` | PASS | 同上 |

前端 bundle 从 790.33 kB 拆分为 505.39 kB 主包 + 9 个页面 chunk（Vite 仍提示 505 kB > 500 kB，仅为警告）。

### 产物与哈希

| 产物 | 字节 | SHA-256 | 签名 |
|---|---:|---|---|
| `target/release/Ditto.exe` | 10,066,944 | `0708D0B8E62E100A23D6547D64F75B46B5414D401DF70AE63D41702D106F3C1F` | NotSigned |
| `target/release/DittoPopup.exe` | 4,963,328 | `34AA478ADFEE60FACC649E646D9F71969429BE2B3C856E0FF044A31ED4382E28` | NotSigned |
| `target/release/DittoUacHelper.exe` | 271,872 | `ECC37631F2257954886BF5224C7EEF61BFAC4B3EE22DF68B9630480B2DB56BF7` | NotSigned |
| `target/release/bundle/nsis/Ditto_0.1.0_x64-setup.exe` | 4,490,969 | `EEB3A198CD06374434CA756488D615916F016CB3CAC3A982BC49320B6229C15D` | NotSigned |
| `target/release/bundle/portable/Ditto-0.1.0-x64-portable.zip` | 6,333,881 | `CEE8DC709056E76719B14BA6F7657F833F5249C9E625B9DAA9A2F67816ED3B7A` | 不适用 |

原始哈希记录：`artifacts/rewrite/after/build/hashes.txt`。

Portable ZIP 内容：`Ditto.exe`、`DittoPopup.exe`、`DittoUacHelper.exe`、`Ditto2.ico`、`locales/{en-US,zh-CN}.json`、`portable` 标记、`README.txt`、`BUILD-INFO.json`。

### 安装 / 卸载 / Portable 冒烟

`pwsh -File tools/qa/install-smoke.ps1 -Setup ... -PortableZip ...`（PASS，原始输出 `artifacts/rewrite/after/qa/install-smoke.log`）：

| 检查 | 结果 |
|---|---|
| NSIS 静默安装到临时目录，退出码 0 | PASS |
| 安装目录包含 Ditto/DittoPopup/DittoUacHelper/uninstall | PASS |
| 已安装 core 启动并创建当前用户 pipe | PASS |
| 静默卸载退出码 0，安装目录移除 | PASS |
| Portable 解压包含全部必需文件与 `portable` 标记 | PASS |
| Portable 启动创建 pipe，并在 `<exe_dir>\Data\ditto.db` 建立数据根 | PASS |

## 集成改动清单

以下改动均为集成修复，记录在案：

1. `apps/ditto/src-tauri/src/lib.rs`
   - 解决 T5 与 T2/T4 的 `lib.rs` 文本冲突：`AppState` 同时持有 `PipeServer`、`Storage`、`CoreService`。
   - 启动顺序：解析数据根 → 应用 pending migration/import → 打开 Storage → 启动 CoreService → 托盘/热键/popup。
   - 新增 Win32 命名事件快速单实例路径（`settings`/`toggle`/`instance`），第二个实例 1-2ms 退出；保留 single-instance 插件作为回退。
   - `spawn_popup` 按设置语言传递 `--lang`。
2. `apps/ditto/src-tauri/src/commands/mod.rs`（整文件重写）
   - 移除 `MockBackend` 与所有 `TODO(T7)`；全部命令改走 `ditto-storage` / `ditto-migration`。
   - 设置双向映射（原生键 + 扩展键），`SettingsChanged` 广播，autostart 实际启用/禁用。
   - 备份、导出、优化、migration dry-run/apply；导入/恢复改为“验证 + 暂存 + 下次启动应用”。
   - 新增 `paste_clip`、`paste_copy_buffer`、`drag_out_clip`、`clear_history`、`migration_dry_run`、`migration_apply`。
   - 文件夹打开仅限数据根与日志目录。
3. `apps/ditto/src-tauri/src/pending.rs`（新增）
   - 活动数据库打开前应用迁移/导入，marker 与包存放在数据根之外。
4. `apps/ditto/src-tauri/src/core_service.rs`
   - 新增 `CoreService::paste`（复用 T4 formats/clip_ids 路径）与 `drag_out`。
5. `crates/ditto-storage/src/store.rs`
   - `count_clips`（真实总数）、`update_clip_text`、kind/pinned 过滤、summary `size_bytes`。
6. `crates/ditto-model/src/types.rs`
   - `ClipQuery.kind` / `pinned_only`，`ClipSummary.size_bytes`。
7. `crates/ditto-ipc/src/lib.rs`
   - 新增 `Request::GetPopupVisibility`（迟启动 popup 重放 Show）。
8. `crates/ditto-win/src/lib.rs`
   - `SingleInstanceSignal`；`PipeServer` 记录 popup 可见状态并提供 `set_popup_visible`。
   - Popup 改为 `WS_EX_LAYERED` alpha=0 + `WS_EX_TRANSPARENT` 常驻窗口，替代 `SW_HIDE`。
9. `apps/ditto-popup/src/provider.rs`
   - 真实 IPC：分组参数、分页、`formats`/`clip_ids` 粘贴、PasteResult 错误、copy buffer 分配/清除/粘贴。
10. `apps/ditto-popup/src/app.rs` / `main.rs`
    - 修复隐藏窗口收不到 redraw 导致 Show 事件永不生效的问题。
    - 连接后查询 `GetPopupVisibility`；CJK 字体按需加载；`DITTO_POPUP_TRACE` 诊断（默认关闭）。
11. `apps/ditto-popup/src/mock.rs`
    - 适配 T2 `ClipKind`（Html/Rtf）与 `pin_order: Option<f64>`。
12. `apps/ditto-popup/src/localization.rs` + `apps/ditto/ui/locales/*`
    - 77 个 `popup.*` 键并入共享 locale；popup 以共享目录为基础再叠加本地 fallback。
13. `apps/ditto/ui/scripts/check-locales.mjs`
    - 同时校验共享与 popup 目录的键集合、值和占位符。
14. `apps/ditto/ui/src/App.tsx` / `pages/DataPage.tsx` / `types.ts` / `api/*`
    - 移除 MockBackend 徽标与文案；页面懒加载；Data 页接入 migration 路径、dry-run/apply、计数/警告/备份路径。
15. `scripts/package-portable.ps1`、`tools/qa/ipc-smoke.ps1`、`tools/qa/install-smoke.ps1`、`crates/ditto-storage/examples/seed_qa.rs`（新增）
    - 可复现的合成 QA 与打包工具。

## TEST

`cargo test --workspace --locked`：全部通过（T2/T3/T4/T5/T6 各 crate 与集成测试）。重点：

| 测试 | 结果 | 证据 |
|---|---|---|
| `ditto-migration` legacy fixture（3 项） | PASS | `artifacts/rewrite/after/qa/migration-fixture.log` |
| `ditto-storage` 10k bench | PASS | `artifacts/rewrite/after/qa/storage-10k.log` |
| T5 前端 `npm run build`（含 locales:check / tsc） | PASS | `artifacts/rewrite/after/build/npm-build.log` |
| T5 `visual:check`（10 张截图、22 项检查） | PASS | `docs/rewrite/screenshots/T5/visual-check.json` |

迁移 fixture 覆盖：dry-run、apply、clips/groups/formats/copy buffers/pins 计数与内容对齐、旧库 SHA-256 不变、导出/导入往返、失败迁移不破坏目标数据根。

## E2E QA（合成数据）

数据根均为临时目录（`DITTO_DATA_DIR`），fixture 由 `seed_qa` 生成；未读取真实 `%APPDATA%\Ditto`、`%LOCALAPPDATA%\Ditto` 或用户剪贴板历史。

| # | 项目 | 结果 | 证据/说明 |
|---|---|---|---|
| D1 | IPC `ShowPopup` → popup 首帧，12 次 | PASS | p50 5.1ms，p95 15.4ms；`artifacts/rewrite/after/qa/ipc-smoke.log` |
| D1 | 物理热键 `Ctrl+Alt+D` 路径 | UNVERIFIED | 自动化未注入物理按键；热键回调与 IPC 共用 `toggle_popup`/`set_popup_visible` |
| D1 | Esc / 失焦 / 外部点击 / 无任务栏 / 不抢焦点 | UNVERIFIED | 代码路径存在；需交互桌面复核 |
| D2 | 粘贴到 Notepad 等：文本/HTML/RTF/图片/文件、纯文本、多选 | UNVERIFIED | 需真实目标窗口；T4 单元/集成测试覆盖 payload |
| D2 | 自写抑制（粘贴不重复入历史） | UNVERIFIED | T4 `WriteSuppression` 已接入；端到端需交互 |
| D3 | 分组/置顶/复制缓冲区/搜索/删除（IPC） | PASS | Create/Rename/DeleteGroup、SetPinned、MoveToGroup、CopyBufferAssign、Search、Delete 全部 OK |
| D3 | 前十/快速粘贴/特殊粘贴键盘 | UNVERIFIED | 需交互桌面 |
| D4 | 托盘 Show/Settings/Exit | UNVERIFIED（点击）；代码与单实例路径一致 | — |
| D4 | 开机启动开关 | PASS（命令调用）/ UNVERIFIED（注册表效果） | `set_settings` 调用 autostart enable/disable |
| D4 | 单实例 `--settings` 打开设置 | PASS | 设置窗口标题 53.7-803.1ms；进程退出 1.1-2.3ms |
| D4 | 设置关闭后 WebView2 归零 | UNVERIFIED（未重测） | T5 证据：`docs/rewrite/screenshots/T5/webview-lifecycle.json` |
| D5 | 浅/深色 100/150/200% DPI、EN/ZH、高对比度 | PASS（浏览器渲染 10 张截图） | `docs/rewrite/screenshots/T5/*.png`；原生 DPI/IME UNVERIFIED |
| D6 | UAC helper 只发 Ctrl+V、取消/超时 | UNVERIFIED（端到端） | T4 helper/token 测试通过 |
| D7 | CF_HDROP 文件复制/粘贴、拖出 | PARTIAL（payload 测试 PASS；交互 UNVERIFIED） | `drag_out_clip` 命令已注册并文档化 |
| D8 | 迁移 fixture 实跑 | PASS | 见 TEST 表 |
| D9 | IPC 仅当前用户/ACL | PASS（代码审查） | `crates/ditto-win` SDDL 限制当前用户；未放宽 |
| D9 | UAC token 一次性 | PASS（T4 测试） | `cargo test --workspace` |
| D9 | 无 telemetry、无任意命令 | PASS（代码审查） | 无网络依赖；命令白名单；文件夹打开限定数据根/日志 |
| D9 | 未签名 Defender/SmartScreen | UNVERIFIED | 未修改任何安全设置；产物 `NotSigned` |
| D10 | 不读取真实用户数据/历史 | PASS | 全部 QA 使用临时数据根与合成 fixture；未改动/恢复用户剪贴板 |

## 性能

方法：release 产物，`tools/qa/ipc-smoke.ps1`，临时数据根；内存为 `Win32_PerfFormattedData_PerfProc_Process.WorkingSetPrivate`（任务管理器“内存”口径）三次采样，取第二次。

| 指标 | 目标 | 实测 | 结果 |
|---|---|---|---|
| 空闲内存（core+popup，设置关闭，未显示过 popup） | <100 MB | 40.3 MB（最终轮；前几轮 48.4 MB） | PASS |
| 空闲内存（加载 CJK 内容后） | <100 MB | 93.1 MB | PASS |
| Working Set（含共享 DLL/GPU，仅参考） | — | 148-160 MB | 记录，不作为验收口径 |
| Popup 首帧（IPC Show，12 次） | p95 <50 ms | p50 5.1 ms / p95 15.4 ms（前几轮 p95 6.9-7.2 ms） | PASS |
| 10k 搜索 p95 | <50 ms | 最大 22.1 ms（quick_paste），其余 1.3-2.3 ms | PASS |
| 设置窗口出现 | <1.5 s | 53.7-803.1 ms（多次运行） | PASS |
| 二次实例退出 | <150 ms | 1.1-2.3 ms | PASS |

原始数据：

- `artifacts/rewrite/after/qa/ipc-smoke.log`
- `artifacts/rewrite/after/perf/*.csv`（T3 mock 滚动/首帧历史）
- `artifacts/rewrite/after/qa/storage-10k.log`

未测量/未达标说明：

- Popup 冷启动（进程创建到 IPC 就绪）实测 5.9-6.0 s。期间按下的热键由 `GetPopupVisibility` 重放，用户感知为首次热键延迟；这是本机软件 GL/驱动初始化耗时，非首帧渲染耗时。
- 10k 真实 popup 滚动 60fps 未在真实数据上重测；T3 mock 结果见 `artifacts/rewrite/after/perf/scroll-10k-release-final2.csv`。
- 设置窗口指标是窗口标题出现时间，不是 WebView2 内容 paint 完成时间。

## UNVERIFIED

1. 真实热键、托盘点击、Esc/失焦/外部点击。
2. 真实目标窗口粘贴（文本/HTML/RTF/图片/文件、纯文本、多选、自写抑制）。
3. UAC 提权目标、helper 取消/超时。
4. OLE 拖出、Explorer 文件拖放交互。
5. 原生 100/150/200% DPI、多显示器、IME 中文/日文、Narrator、高对比度主题。
6. 设置关闭后 WebView2 进程归零（沿用 T5 证据，未重测）。
7. Defender/SmartScreen 实际拦截行为；未修改安全设置。
8. 迁移 UI 的原生文件选择器（当前为文本框路径输入）。

## 已知限制 / 风险

| 级别 | 项 | 说明 |
|---|---|---|
| Major | Popup 冷启动约 6 s | 首次热键在 popup 就绪前会被重放，但存在可感知延迟 |
| Major | 设置热键字符串尚未注册为全局热键 | 核心仍使用 T1 开发热键 `Ctrl+Alt+D`；设置项已持久化 |
| Minor | 导入/恢复需重启 | 避免替换活动 SQLite 句柄；UI 文案已说明 |
| Minor | 迁移源路径需手工输入 | 未接原生文件对话框 |
| Minor | 设置页列表分页为 cursor 循环 | 深页码（10k/50 页）会做多次有界查询 |
| Minor | copy buffer `updatedAtMs` 返回 null | storage API 未暴露该列 |
| Minor | 前端主包 505 kB | 已按页面拆分；Fluent UI 仍占主要体积 |
| Info | 全部二进制未签名 | 可能触发 SmartScreen/Defender 警告；未绕过 |

## NEXT

1. T8 独立审查：从 `docs/rewrite/REVIEW-GUIDE.md` 开始。
2. 在解锁、可交互桌面复跑 UNVERIFIED 项（真实热键、粘贴、UAC、拖出、IME/DPI、托盘）。
3. 评估把设置热键真正注册到 `tauri-plugin-global-shortcut`。
4. 评估 popup 冷启动优化（预热/持久化 GL 缓存/切换渲染后端）。
