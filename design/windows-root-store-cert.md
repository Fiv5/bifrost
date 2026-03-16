# Windows Root Store 证书安装与检测

## 功能模块详细描述

- 修复 Windows 下 CA 证书检测仅按证书名称匹配，可能被同名旧证书误判为“已安装且已信任”的问题。
- 优化 Windows 安装路径：优先安装到 `CurrentUser\Root`，失败时再通过 UAC 提权安装到机器级 `Root`。
- 保持桌面端和 CLI 共用同一套 Windows 证书安装与检测逻辑。

## 实现逻辑

### 1. Windows 状态检测改为按当前证书指纹核对

- 读取当前 `ca.crt`，解析 PEM 后计算 SHA-1 thumbprint。
- 分别检查：
  - `CurrentUser\Root`
  - `LocalMachine\Root`
- 只有当证书库中的 thumbprint 与当前 `ca.crt` 一致时，才判定为已安装。
- 若发现同名 `Bifrost CA` 但 thumbprint 不匹配，则返回 `fingerprint_match = false`，避免旧证书误导状态页和 CLI。

### 2. Windows 安装优先当前用户根证书库

- `install_windows()` 改为：
  - 先执行 `certutil -user -addstore Root <ca.crt>`
  - 若失败，再通过 UAC 提权执行机器级 `certutil -addstore Root <ca.crt>`
- 这样桌面端启动时更容易无感完成安装，CLI 交互也更顺畅。

### 3. 桌面端沿用统一安装入口

- 桌面端 `install_and_trust_gui()` 在 Windows 下继续复用 `install_and_trust()`。
- 因为 Windows 已有 UAC 提权逻辑，所以无需额外的桌面专用安装实现。

## 依赖项

- `crates/bifrost-tls/src/install.rs`
- `desktop/src-tauri/src/main.rs`
- `crates/bifrost-cli/src/commands/ca.rs`

## 测试方案（含 e2e）

- 单元测试：
  - Windows certutil thumbprint 解析
  - thumbprint 规范化
- 行为验证：
  - 仅存在同名旧证书时，状态检查应报告 fingerprint mismatch
  - 安装时优先写入 `CurrentUser\Root`
  - 当前用户安装失败时，确认 UAC 提权路径仍可用
- E2E：
  - Windows 桌面端启动后触发证书安装
  - Windows CLI `bifrost ca install` 成功安装后，状态页与 CLI 均显示已信任

## 校验要求（含 rust-project-validate）

- 先执行本次修改相关测试
- 再执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - 按修改范围执行 `cargo test`
  - `cargo build --all-targets --all-features`

## 文档更新要求

- README 如涉及平台安装说明，应补充 Windows 优先写入 `CurrentUser\Root`，失败时回退到 UAC 提权安装
