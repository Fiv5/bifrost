# Desktop Window Chrome Strategy

## 现状结论

旧文档描述的平台分配置与前端自定义标题栏方案目前并未落地。仓库中只有一个 [`desktop/src-tauri/tauri.conf.json`](../desktop/src-tauri/tauri.conf.json)，没有 `tauri.macos.conf.json` / `tauri.windows.conf.json`。

## 当前实现

- 窗口装饰策略主要由 [`desktop/src-tauri/src/main.rs`](../desktop/src-tauri/src/main.rs) 在运行时控制。
- macOS 启动阶段：
  - `host` 窗口以 `decorations(false)` 创建，用于承载透明启动态与原生 overlay。
  - handoff 到主界面时再执行 `set_decorations(true)`，恢复标准窗口外观。
- 非 macOS 平台：
  - 当前直接使用带系统装饰的普通窗口启动。
  - 仓库里没有已启用的 Windows 自绘标题栏实现，也没有前端标题栏组件接管窗口控制。

## 与旧方案的差异

- 没有平台级 Tauri 配置拆分。
- 没有统一的前端标题栏抽象层。
- `startDragging()` / `toggleMaximize()` 类型定义存在于前端 runtime bridge 中，但当前仓库没有对应的桌面标题栏 UI 作为主路径使用。

## 建议

- 如果后续真的要做跨平台窗口 chrome 统一，建议重新立项，单独定义：
  - 启动态和正常态的装饰切换规则；
  - macOS / Windows / Linux 的平台差异；
  - 前端是否需要接管标题栏交互。
