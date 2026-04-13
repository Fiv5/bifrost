# CLI Admin 管理命令测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确认服务启动成功，端口 8800 可用
3. 以下命令均需在另一个终端窗口中执行，且设置相同的数据目录：
   ```bash
   export BIFROST_DATA_DIR=./.bifrost-test
   ```

---

## 测试用例

### TC-CAD-01：查看远程访问状态（初始状态）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote status
   ```

**预期结果**：
- 输出包含 `Remote admin access: disabled`
- 输出包含 `Admin username: admin`
- 输出包含 `Admin password: not set`
- 输出包含 `Audit DB:` 后跟审计数据库文件路径

---

### TC-CAD-02：启用远程访问（未设置密码时自动提示设置）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote enable
   ```
2. 在提示 `New admin password` 时输入 `test123456`
3. 在提示 `Confirm password` 时输入 `test123456`

**预期结果**：
- 因未设置密码，先提示 `Admin password not set yet. Please set a password to enable remote access.`
- 出现密码输入提示 `New admin password`
- 出现确认密码提示 `Confirm password`
- 密码设置成功后输出 `Remote admin access: enabled`

---

### TC-CAD-03：确认远程访问已启用

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote status
   ```

**预期结果**：
- 输出包含 `Remote admin access: enabled`
- 输出包含 `Admin username: admin`
- 输出包含 `Admin password: set`

---

### TC-CAD-04：禁用远程访问

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote disable
   ```

**预期结果**：
- 输出 `Remote admin access: disabled`

---

### TC-CAD-05：确认远程访问已禁用

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote status
   ```

**预期结果**：
- 输出包含 `Remote admin access: disabled`
- 密码状态仍为 `set`（禁用远程访问不会清除已设置的密码）

---

### TC-CAD-06：交互式设置管理员密码（默认用户名 admin）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin passwd
   ```
2. 在提示 `New admin password` 时输入 `newpass123`
3. 在提示 `Confirm password` 时输入 `newpass123`

**预期结果**：
- 出现密码输入提示（密码输入时不显示字符）
- 出现确认密码提示
- 输出 `Admin password updated.`

---

### TC-CAD-07：交互式设置管理员密码（指定用户名）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin passwd --username admin
   ```
2. 在提示 `New admin password` 时输入 `admin888`
3. 在提示 `Confirm password` 时输入 `admin888`

**预期结果**：
- 出现密码输入提示
- 出现确认密码提示
- 输出 `Admin password updated.`
- 通过 `admin remote status` 验证用户名仍为 `admin`

---

### TC-CAD-08：通过 stdin 设置密码（非交互式）

**操作步骤**：
1. 执行命令：
   ```bash
   echo "pipepass123" | BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin passwd --password-stdin
   ```

**预期结果**：
- 无交互式提示
- 直接输出 `Admin password updated.`
- 密码被设置为 `pipepass123`

---

### TC-CAD-09：通过 stdin 设置密码（空密码应报错）

**操作步骤**：
1. 执行命令：
   ```bash
   echo "" | BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin passwd --password-stdin
   ```

**预期结果**：
- 输出错误信息，包含 `Password cannot be empty`
- 命令以非零退出码退出

---

### TC-CAD-10：吊销所有管理员会话

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin revoke-all
   ```

**预期结果**：
- 输出包含 `All admin sessions revoked`
- 输出包含 `revoke_before=` 后跟一个 Unix 时间戳

---

### TC-CAD-11：查看审计日志（默认参数）

**前置条件**：已通过远程 IP 登录管理端至少一次，产生审计记录

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin audit
   ```

**预期结果**：
- 如果有审计记录：
  - 输出标题行包含 `Admin login audit (total: N, showing: M):`
  - 输出分隔线 `====================================================`
  - 每条记录格式为 `- id=N ts=<RFC3339时间> user=<用户名> ip=<IP> ua=<UserAgent>`
  - 输出末尾显示 `Audit DB:` 后跟数据库路径
- 如果没有审计记录：
  - 输出 `No audit records.`
  - 输出 `Audit DB:` 后跟数据库路径

---

### TC-CAD-12：查看审计日志（指定 limit 和 offset）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin audit --limit 100 --offset 0
   ```

**预期结果**：
- 输出格式与 TC-CAD-11 相同
- `showing` 数量不超过 100
- 分页起始位置从第 0 条开始

---

### TC-CAD-13：查看审计日志（JSON 格式输出）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin audit --json
   ```

**预期结果**：
- 输出为格式化的 JSON 字符串
- JSON 包含以下顶层字段：`total`（数字）、`limit`（数字）、`offset`（数字）、`items`（数组）
- `items` 数组中每个元素包含：`id`、`ts`、`username`、`ip`、`ua` 字段
- JSON 可被 `jq` 等工具正确解析

---

### TC-CAD-14：查看审计日志（JSON 格式，带 limit 和 offset）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin audit --limit 100 --offset 0 --json
   ```

**预期结果**：
- 输出为格式化的 JSON
- `limit` 字段值为 100
- `offset` 字段值为 0
- `total` 字段为实际记录总数
- `items` 数组长度不超过 100

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
