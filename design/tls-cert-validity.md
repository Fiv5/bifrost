# TLS 证书有效期与 CA 一致性修复

## 功能模块详细描述

- 修复 TLS MITM 动态叶子证书默认使用异常有效期，导致浏览器即使信任了根证书仍可能报证书错误的问题。
- 修复运行时加载 CA 时重新自签一张新根证书的问题，保证代理实际签发叶子证书时使用的 CA 与磁盘上、用户已安装信任的 CA 保持一致。

## 实现逻辑

### 1. 为新生成的证书设置合理有效期

- Root CA：
  - `not_before = now - 1 day`
  - `not_after = now + 3650 days`
- 动态叶子证书：
  - `not_before = now - 1 day`
  - `not_after = now + 90 days`

### 2. 加载 CA 时保留原始证书字节

- `load_root_ca()` 不再根据 subject 和私钥重新生成一张新自签名证书。
- 直接保留磁盘上的 PEM / DER 内容，确保运行时 CA 指纹与已安装到系统信任链中的 CA 一致。

### 3. 保存与签发路径统一使用原始 CA 内容

- `save_root_ca()` 使用 `CertificateAuthority` 内保存的原始 PEM。
- 动态叶子证书签发与链拼装统一使用原始 CA DER。

## 依赖项

- 复用现有 `rcgen` / `rustls` / `x509-parser`
- 新增 `time` 依赖用于生成相对当前时间的证书有效期

## 测试方案（含 e2e）

- 单元测试：
  - 验证新生成 root CA 的有效期处于预期范围
  - 验证动态叶子证书有效期处于浏览器可接受范围
  - 验证 `load_root_ca()` 会保留原始证书 DER，不再重签
- E2E：
  - 通过本地代理抓取 MITM 证书，确认目标域名和有效期正确
  - 执行现有 TLS 相关测试回归

## 校验要求（含 rust-project-validate）

- 先执行本次修改相关测试和 TLS 证书检查
- 再执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - 按修改范围执行 `cargo test`
  - `cargo build --all-targets --all-features`

## 文档更新要求

- 本次不新增用户配置项
- 若需要提示用户重新生成 / 重新安装旧 CA，可在后续 README 或证书使用文档中补充排障说明
