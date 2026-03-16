# 证书安装与信任状态统一检测

## 功能模块详细描述

- 修复管理端与设置页对 CA 证书状态的表达不一致问题。
- 避免仅根据 `ca.crt` 文件存在与否，就向用户传达“证书可用”或“状态正常”的错误信息。
- 统一全局证书状态语义，明确区分：
  - 未安装
  - 已安装但未信任
  - 已安装且已信任
  - 检测失败

## 实现逻辑

### 1. 统一证书状态语义

- 复用 `bifrost-tls` 中已有的 `CertStatus` 三态能力，作为“是否安装/是否信任”的唯一判断来源。
- 补充公共辅助方法，统一把三态映射为：
  - `is_installed`
  - `is_trusted`
- macOS 下补强本地证书指纹解析逻辑：
  - 不再严格依赖 `openssl x509 -fingerprint -sha256` 输出必须完全匹配 `SHA256 Fingerprint=...`
  - 统一提取并规范化十六进制指纹，兼容大小写、空格与分隔符差异
  - 避免因 OpenSSL / LibreSSL 输出格式差异，导致状态检测误报为 `unknown` 或直接报错

### 2. 管理端证书信息接口返回完整状态

- `crates/bifrost-admin/src/handlers/cert.rs` 的 `/api/cert/info` 不再只返回 `available`。
- 新增字段：
  - `status`
  - `status_label`
  - `installed`
  - `trusted`
  - `status_message`
- `available` 保留，仅表示“证书文件可下载”，不再代表系统已经信任。
- 当系统信任状态检测失败时，接口返回 `unknown`，避免把错误场景误报成“已安装”或“可用”。

### 3. 设置页改为展示系统信任状态

- 设置页证书卡片优先展示统一状态，而不是 `available`。
- 下载按钮、二维码仍基于 `available` 控制，因为它们描述的是“证书文件是否存在/可下载”。
- 用户文案明确区分：
  - 证书文件是否可下载
  - 当前设备是否已经安装并信任该 CA

## 依赖项

- `crates/bifrost-tls/src/install.rs`
- `crates/bifrost-admin/src/handlers/cert.rs`
- `web/src/api/cert.ts`
- `web/src/types/index.ts`
- `web/src/pages/Settings/tabs/CertificateTab.tsx`

## 测试方案（含 e2e）

- 单元测试：
  - 验证 `CertStatus` 的安装/信任辅助判断结果正确
  - 验证 macOS 指纹解析可兼容不同 `openssl` / `security` 输出格式
- 管理端验证：
  - 在无 `ca.crt` 时访问 `/api/cert/info`，确认返回 `not_installed`
  - 在有证书文件但未信任时，确认返回 `installed_not_trusted`
  - 在已安装且已信任时，确认返回 `installed_and_trusted`
  - 在检测命令失败时，确认返回 `unknown`
- 前端验证：
  - 设置页状态标签与说明文案随上述状态正确变化
- E2E：
  - 按项目要求先执行相关 e2e，再执行 rust-project-validate

## 校验要求（含 rust-project-validate）

- 先执行本次改动相关测试或 e2e 验证
- 再执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - 按修改范围执行 `cargo test`
  - `cargo build --all-targets --all-features`

## 文档更新要求

- 本次无需更新 `README.md`
