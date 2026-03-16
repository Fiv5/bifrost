# 桌面端启动期日志与故障可观测性补强

## 功能模块详细描述

这份文档只记录当前已经落地的桌面端启动期可观测性补强，不再混入尚未实现的启动 UI 草案。

当前已落地的重点是：

- 桌面端与 CLI 共用 `~/.bifrost` / `BIFROST_DATA_DIR` 作为默认数据目录
- 启动 bootstrap、sidecar stdout、sidecar stderr 都会落盘
- backend 启动失败原因会同步写入桌面 runtime 状态，前端可见

它和 [`design/desktop-launcher-startup.md`](./desktop-launcher-startup.md) 的关系是：

- `desktop-launcher-startup.md` 关注窗口 handoff / launcher overlay
- 本文只关注日志、数据目录与失败定位

## 实现逻辑

### 1. 桌面壳层 bootstrap 日志与默认目录统一

- 当前实现会在 `desktop/src-tauri/src/main.rs` 中写入：
  - `logs/desktop-bootstrap.log`
- 数据目录与 CLI 保持一致：
  - 若设置 `BIFROST_DATA_DIR`，使用该目录
  - 否则默认使用 `~/.bifrost`
  - 目录解析直接复用 `bifrost_storage::data_dir()`，避免桌面端单独维护默认路径逻辑
- `desktop-config.json` 也位于同一数据目录下，而不是 Tauri 私有目录。
- 记录关键启动步骤：
  - 使用的 bifrost 二进制路径
  - 目标数据目录
  - 端口尝试过程
  - sidecar 启动/停止结果
  - 等待 ready 超时的原因
- 即使 core 尚未成功初始化 tracing，桌面壳层自身也能留下轨迹。

### 2. sidecar 标准输出落盘

- 启动内嵌 `bifrost` core 时，不再将 `stdout` / `stderr` 丢弃。
- 改为分别追加写入：
  - `logs/desktop-sidecar.out.log`
  - `logs/desktop-sidecar.err.log`
- 这样 core 在 tracing 初始化前的 `println!` / `eprintln!`、panic 或配置错误都能被保留下来。

### 3. 启动失败信号会回传到前端

- backend bootstrap 在后台线程中执行。
- 成功时会把 `startup_ready` 标为 `true`。
- 失败时会记录：
  - `desktop-bootstrap.log`
  - runtime 内的 `startup_error`
- 前端桌面壳层会通过 `get_desktop_runtime` 读到这两个状态，并在错误弹窗里展示失败信息。

### 4. 当前未落地的部分

- 目前等待 backend ready 仍是固定超时轮询，并没有“子进程提前退出立即短路”的快速失败分支。
- Web 侧也还没有独立的 `#startup-splash` 首屏骨架；桌面启动视觉仍以现有的原生 launcher / host window handoff 为准。
- 因此这份文档不再把“快速失败”和“统一 web 骨架”写成已完成事实。

## 依赖项

- `desktop/src-tauri/src/main.rs`

## 测试方案（含 e2e）

1. 启动桌面端开发模式，确认正常情况下能拉起内嵌 core。
2. 检查数据目录下是否生成：
   - `logs/desktop-bootstrap.log`
   - `logs/desktop-sidecar.out.log`
   - `logs/desktop-sidecar.err.log`
3. 使用自定义 `BIFROST_DATA_DIR` 启动，确认桌面配置与日志都写入该目录。
4. 构造启动失败场景，确认前端错误弹窗能拿到 `startup_error`，同时日志可定位失败阶段。

## 校验要求（含 rust-project-validate）

- 先执行与本次修复相关的端到端或启动链路验证
- 再执行 `rust-project-validate` 要求的格式、lint、测试和构建校验

## 文档更新要求

- README 中桌面端默认数据目录说明需要与当前行为保持一致
