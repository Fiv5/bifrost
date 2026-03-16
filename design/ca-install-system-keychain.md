# CA Install System Keychain

## 功能模块详细描述

- 新增 `bifrost ca install` 子命令，用于显式安装并信任 CA 证书。
- macOS 下将 CA 安装流程固定为 `System.keychain`，避免仅写入登录钥匙串导致部分浏览器/辅助进程仍提示 HTTPS 不安全。

## 实现逻辑

### 1. 新增 `ca install` 命令

- 在 CLI 的 `CaCommands` 中新增 `Install`。
- 运行时先确保本地 CA 文件存在；不存在时自动生成。
- 然后调用统一的 `CertInstaller::install_and_trust()` 完成安装。

### 2. 调整 macOS 安装策略

- `CertInstaller::install_macos()` 改为：
  - 仅安装到 `System.keychain`
  - 不再回退到登录钥匙串
- `start` 里的证书安装提示和 `bifrost ca install` 都走同一套系统级安装逻辑。

## 依赖项

- 复用现有 `bifrost-tls::CertInstaller`
- 复用现有 CA 生成与保存逻辑

## 测试方案（含 e2e）

- 命令级验证：
  - `bifrost ca install --help`
  - `cargo test -p bifrost-cli`
- 行为验证：
  - macOS 下确认安装仅写入 `System.keychain`
  - 仅存在 `login keychain` 证书时，状态检查仍判定为未安装
  - `start` 交互路径复用同一安装逻辑

## 校验要求（含 rust-project-validate）

- 先执行本次修改相关测试
- 再执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - 按修改范围执行 `cargo test`
  - `cargo build --all-targets --all-features`

## 文档更新要求

- CLI 帮助文案需新增 `ca install`
- 如 README 后续维护 CLI 示例，可补充该命令
