# Desktop Launcher Startup Flow

## 现状结论

旧文档描述的是“独立 launcher 窗口 + 主窗口”双窗口方案；当前实现已经改成“单个 host window + 原生启动遮罩 + 内嵌 webview handoff”。

## 实现入口速查

- 原生启动遮罩：`desktop/src-tauri/src/native_launcher.rs`
- 启动状态与 handoff：`desktop/src-tauri/src/main.rs` 的 `BackendState` 与 `notify_main_window_ready`

## 当前实现

- 桌面端只创建一个 Tauri `host` 窗口，不再创建独立的 `launcher` 窗口。
- macOS 上启动时：
  - `host` 窗口先以较小尺寸、透明、无边框方式显示。
  - 通过 [`desktop/src-tauri/src/native_launcher.rs`](../desktop/src-tauri/src/native_launcher.rs) 在宿主窗口内容区安装原生 overlay，承担启动器视觉层。
  - 主业务 webview 通过 `create_main_webview()` 预先创建，但会先停放在不可见位置，待切换时再 reveal。
- 后端 core 仍在后台线程中并行启动，状态写入 `BackendState.startup_ready` / `startup_error`。
- 前端完成首屏准备后调用 `notify_main_window_ready`，Rust 侧执行：
  - 放大 `host` 窗口到主界面尺寸；
  - 恢复背景与装饰；
  - 显示主 webview；
  - 淡出并移除原生 launcher overlay。
- 非 macOS 平台没有原生 launcher overlay，直接进入普通 webview 启动路径。

## 与旧方案的主要差异

- 没有独立 `launcher` label/window。
- handoff 的主体是同一个 `host` 窗口，而不是两个窗口之间的切换。
- 启动页不是前端页面，而是 macOS 原生视图 overlay。
- 回退路径是“无原生 launcher 时直接进入主 webview”，而不是超时后显示第二个窗口。

## 维护建议

- 后续若重新引入独立启动窗口，需要重写本文件，不应继续沿用当前描述。
- 当前设计的关键状态字段集中在 [`desktop/src-tauri/src/main.rs`](../desktop/src-tauri/src/main.rs) 的 `BackendState` 与 handoff 逻辑。
