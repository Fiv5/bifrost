# CLI CA 证书管理命令测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保 `.bifrost-test/certs/` 目录下无残留证书文件（如有，先执行清理）：
   ```bash
   rm -rf .bifrost-test/certs
   ```
3. 所有 `bifrost` 命令均需指定临时数据目录：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost --
   ```

---

## 测试用例

### TC-CCA-01：首次生成 CA 证书

**前置条件**：`.bifrost-test/certs/` 目录下无 `ca.crt` 和 `ca.key` 文件

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca generate
   ```

**预期结果**：
- 命令输出包含 `CA certificate generated successfully.`
- 输出包含 `Certificate:` 后跟证书文件路径（指向 `.bifrost-test/certs/ca.crt`）
- 输出包含 `Private key:` 后跟私钥文件路径（指向 `.bifrost-test/certs/ca.key`）
- 输出包含提示信息 `To use HTTPS interception, install the CA certificate in your browser or system.`
- 文件 `.bifrost-test/certs/ca.crt` 已创建且非空
- 文件 `.bifrost-test/certs/ca.key` 已创建且非空

---

### TC-CCA-02：重复生成 CA 证书（不带 --force）

**前置条件**：已通过 TC-CCA-01 生成证书

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca generate
   ```

**预期结果**：
- 命令输出包含 `CA certificate already exists.`
- 输出包含 `Use --force to regenerate.`
- 原有 `.bifrost-test/certs/ca.crt` 和 `ca.key` 文件内容未被修改（可通过比对文件 MD5 验证）

---

### TC-CCA-03：强制重新生成 CA 证书（--force）

**前置条件**：已通过 TC-CCA-01 生成证书

**操作步骤**：
1. 记录当前证书指纹：
   ```bash
   openssl x509 -in .bifrost-test/certs/ca.crt -fingerprint -sha256 -noout
   ```
2. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca generate --force
   ```
3. 再次查看证书指纹：
   ```bash
   openssl x509 -in .bifrost-test/certs/ca.crt -fingerprint -sha256 -noout
   ```

**预期结果**：
- 命令输出包含 `CA certificate generated successfully.`
- 输出包含 `Certificate:` 和 `Private key:` 路径信息
- 新旧证书的 SHA-256 指纹不同，证明证书已被重新生成
- `.bifrost-test/certs/ca.crt` 和 `ca.key` 文件已更新

---

### TC-CCA-04：查看 CA 证书信息

**前置条件**：已通过 TC-CCA-01 或 TC-CCA-03 生成证书

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca info
   ```

**预期结果**：
- 输出包含标题 `CA Certificate Information`
- 输出包含 `📜 Certificate Details` 区块，显示以下字段：
  - `Subject:` — 非空
  - `Issuer:` — 非空
  - `Serial Number:` — 非空
  - `Signature Algo:` — 非空
  - `Is CA:` — 值为 `Yes`
- 输出包含 `🔑 Key Information` 区块，显示：
  - `Algorithm:` — 非空，包含密钥位数信息（如 `(2048 bits)` 或 `(256 bits)`）
- 输出包含 `📅 Validity Period` 区块，显示：
  - `Not Before:` — 合理的日期时间格式（如 `2026-04-12 ... UTC`）
  - `Not After:` — 晚于 `Not Before` 的日期
  - `Remaining:` — 显示剩余天数
- 输出包含 `🔐 Fingerprint` 区块，显示：
  - `SHA-256:` — 非空的十六进制指纹
- 输出包含 `📂 File Paths` 区块，显示：
  - `Certificate:` — 指向 `.bifrost-test/certs/ca.crt`
  - `Private Key:` — 指向 `.bifrost-test/certs/ca.key`
  - `File Modified:` — 显示文件修改时间
- 输出包含 `💻 System Trust Status` 区块，显示当前系统信任状态

---

### TC-CCA-05：证书不存在时查看信息

**前置条件**：`.bifrost-test/certs/` 目录下无 `ca.crt` 文件

**操作步骤**：
1. 删除证书文件：
   ```bash
   rm -f .bifrost-test/certs/ca.crt .bifrost-test/certs/ca.key
   ```
2. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca info
   ```

**预期结果**：
- 命令返回错误信息，包含 `CA certificate not found`
- 提示用户运行 `bifrost ca generate`

---

### TC-CCA-06：导出 CA 证书（默认路径）

**前置条件**：已生成 CA 证书

**操作步骤**：
1. 确保当前目录下无 `bifrost-ca.crt` 文件：
   ```bash
   rm -f bifrost-ca.crt
   ```
2. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca export
   ```
3. 检查文件是否存在：
   ```bash
   ls -la bifrost-ca.crt
   ```

**预期结果**：
- 命令输出包含 `CA certificate exported to: bifrost-ca.crt`
- 当前目录下生成 `bifrost-ca.crt` 文件
- 导出文件内容与源文件 `.bifrost-test/certs/ca.crt` 一致（可通过 `diff` 验证）：
  ```bash
  diff bifrost-ca.crt .bifrost-test/certs/ca.crt
  ```

---

### TC-CCA-07：导出 CA 证书到指定路径（-o）

**前置条件**：已生成 CA 证书

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca export -o /tmp/my-ca.crt
   ```
2. 检查文件是否存在：
   ```bash
   ls -la /tmp/my-ca.crt
   ```

**预期结果**：
- 命令输出包含 `CA certificate exported to: /tmp/my-ca.crt`
- `/tmp/my-ca.crt` 文件已创建
- 文件内容与 `.bifrost-test/certs/ca.crt` 一致：
  ```bash
  diff /tmp/my-ca.crt .bifrost-test/certs/ca.crt
  ```

---

### TC-CCA-08：证书不存在时导出

**前置条件**：`.bifrost-test/certs/` 目录下无 `ca.crt` 文件

**操作步骤**：
1. 删除证书文件：
   ```bash
   rm -f .bifrost-test/certs/ca.crt .bifrost-test/certs/ca.key
   ```
2. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca export
   ```

**预期结果**：
- 命令返回错误信息，包含 `CA certificate not found`
- 提示用户运行 `bifrost ca generate`
- 不会在当前目录生成任何文件

---

### TC-CCA-09：安装 CA 证书到系统信任存储

**前置条件**：已生成 CA 证书

**注意**：此操作可能需要管理员权限（macOS 需要输入系统密码或 sudo 授权），测试环境中可能弹出系统授权弹窗。

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca install
   ```
2. 如系统弹出授权提示，输入管理员密码确认

**预期结果**：
- 命令输出包含 `CA certificate installed and trusted successfully.`
- 输出包含 `Certificate:` 后跟证书文件路径
- 再次执行 `ca info` 查看信任状态：
  ```bash
  BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca info
  ```
  `System Trust Status` 区块显示 `✓ Installed and trusted`

---

### TC-CCA-10：证书不存在时自动生成后安装

**前置条件**：`.bifrost-test/certs/` 目录下无 `ca.crt` 和 `ca.key` 文件

**注意**：此操作可能需要管理员权限。

**操作步骤**：
1. 删除证书文件：
   ```bash
   rm -f .bifrost-test/certs/ca.crt .bifrost-test/certs/ca.key
   ```
2. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca install
   ```

**预期结果**：
- 输出包含 `Valid CA certificate not found. Generating...`
- 输出包含 `✓ CA certificate generated.`
- 输出包含 `CA certificate installed and trusted successfully.`
- `.bifrost-test/certs/ca.crt` 和 `ca.key` 文件已自动创建

---

### TC-CCA-11：强制重新生成后验证旧导出文件失效

**前置条件**：已通过 TC-CCA-06 导出过证书

**操作步骤**：
1. 记录当前导出文件的指纹：
   ```bash
   openssl x509 -in bifrost-ca.crt -fingerprint -sha256 -noout
   ```
2. 强制重新生成证书：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca generate --force
   ```
3. 重新导出：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca export
   ```
4. 比对新旧指纹：
   ```bash
   openssl x509 -in bifrost-ca.crt -fingerprint -sha256 -noout
   ```

**预期结果**：
- 步骤 2 输出 `CA certificate generated successfully.`
- 步骤 3 输出 `CA certificate exported to: bifrost-ca.crt`
- 步骤 1 和步骤 4 的 SHA-256 指纹不同，说明导出文件已更新为新证书

---

### TC-CCA-12：导出的证书格式验证

**前置条件**：已生成并导出 CA 证书

**操作步骤**：
1. 导出证书：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- ca export -o /tmp/verify-ca.crt
   ```
2. 使用 openssl 验证证书格式：
   ```bash
   openssl x509 -in /tmp/verify-ca.crt -text -noout
   ```

**预期结果**：
- openssl 命令成功执行，无报错
- 输出包含 `Issuer:` 字段
- 输出包含 `CA:TRUE`（表明是 CA 证书）
- 输出包含 `Validity` 区块，显示有效期

---

## 清理

测试完成后清理临时数据和导出文件：
```bash
rm -rf .bifrost-test
rm -f bifrost-ca.crt
rm -f /tmp/my-ca.crt
rm -f /tmp/verify-ca.crt
```
