# CLI 白名单与访问控制命令测试用例

## 功能模块说明

本文档覆盖 Bifrost CLI 中 `whitelist`（别名 `wl`）子命令的所有功能，包括白名单的增删查、LAN 访问控制、访问模式切换、待审批请求管理以及临时白名单管理。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被其他进程占用
3. CLI 命令均在另一个终端窗口中执行，设置相同的数据目录：
   ```bash
   export BIFROST_DATA_DIR=./.bifrost-test
   ```
4. 初始状态下白名单为空，访问模式为默认值（`interactive`），LAN 访问为 `disabled`

---

## 测试用例

### TC-CWL-01：查看空白名单列表

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist list
   ```

**预期结果**：
- 输出标题 `Client IP Whitelist`
- 输出分隔线 `===================`
- 显示 `No entries in whitelist.`
- 显示 `LAN (private network) access: disabled`

---

### TC-CWL-02：添加单个 IP 到白名单

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add 192.168.1.100
   ```

**预期结果**：
- 输出 `Added '192.168.1.100' to whitelist.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

### TC-CWL-03：添加 CIDR 到白名单

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add 10.0.0.0/24
   ```

**预期结果**：
- 输出 `Added '10.0.0.0/24' to whitelist.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

### TC-CWL-04：验证添加后白名单列表正确显示

**前置条件**：已执行 TC-CWL-02 和 TC-CWL-03

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist list
   ```

**预期结果**：
- 输出标题 `Client IP Whitelist`
- 列表中包含 `192.168.1.100`
- 列表中包含 `10.0.0.0/24`
- 每个条目前有 `  - ` 前缀
- 底部显示 `LAN (private network) access: disabled`

---

### TC-CWL-05：添加重复 IP 时提示已存在

**前置条件**：已执行 TC-CWL-02，白名单中已包含 `192.168.1.100`

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add 192.168.1.100
   ```

**预期结果**：
- 输出 `'192.168.1.100' is already in the whitelist.`
- 不输出 "Restart" 提示

---

### TC-CWL-06：添加无效 IP 地址时返回错误

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add not_an_ip
   ```

**预期结果**：
- 命令执行失败，输出包含 `Invalid IP address: not_an_ip`

---

### TC-CWL-07：添加无效 CIDR 时返回错误

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add 192.168.1.0/99
   ```

**预期结果**：
- 命令执行失败，输出包含 `Invalid CIDR notation: 192.168.1.0/99`

---

### TC-CWL-08：从白名单中移除 IP

**前置条件**：白名单中已包含 `192.168.1.100`（TC-CWL-02）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist remove 192.168.1.100
   ```

**预期结果**：
- 输出 `Removed '192.168.1.100' from whitelist.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

### TC-CWL-09：移除不存在的 IP 时提示未找到

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist remove 172.16.0.1
   ```

**预期结果**：
- 输出 `'172.16.0.1' is not in the whitelist.`

---

### TC-CWL-10：启用 LAN 访问

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist allow-lan true
   ```

**预期结果**：
- 输出 `LAN (private network) access enabled.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

### TC-CWL-11：禁用 LAN 访问

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist allow-lan false
   ```

**预期结果**：
- 输出 `LAN (private network) access disabled.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

### TC-CWL-12：allow-lan 传入无效参数时返回错误

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist allow-lan maybe
   ```

**预期结果**：
- 命令执行失败，输出包含错误提示（clap 参数校验拒绝非 `true`/`false` 的值）

---

### TC-CWL-13：查看访问控制状态

**前置条件**：已执行 TC-CWL-03（白名单中包含 `10.0.0.0/24`），LAN 访问已禁用

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist status
   ```

**预期结果**：
- 输出标题 `Access Control Settings`
- 输出分隔线 `=======================`
- 显示 `Mode: interactive`（默认模式）
- 显示 `LAN access: disabled`
- 显示 `Whitelist entries:` 后跟当前白名单条目数
- 如有白名单条目，逐条列出
- 显示四种访问模式的说明：
  - `local_only - Only allow connections from localhost`
  - `whitelist - Allow localhost + whitelisted IPs/CIDRs`
  - `interactive - Prompt for confirmation on unknown IPs (default)`
  - `allow_all - Allow all connections (not recommended)`

---

### TC-CWL-14：查看当前访问模式（不带参数）

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode
   ```

**预期结果**：
- 输出 `Current access mode: interactive`（或当前实际模式）

---

### TC-CWL-15：设置访问模式为 whitelist

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode whitelist
   ```

**预期结果**：
- 输出 `Access mode set to: whitelist`

---

### TC-CWL-16：设置访问模式为 local_only

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode local_only
   ```

**预期结果**：
- 输出 `Access mode set to: local_only`

---

### TC-CWL-17：设置访问模式为 allow_all

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode allow_all
   ```

**预期结果**：
- 输出 `Access mode set to: allow_all`

---

### TC-CWL-18：设置访问模式为 interactive

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode interactive
   ```

**预期结果**：
- 输出 `Access mode set to: interactive`

---

### TC-CWL-19：设置无效的访问模式时返回错误

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode invalid_mode
   ```

**预期结果**：
- 命令执行失败，输出包含错误提示（clap 参数校验拒绝非法模式值，仅接受 `local_only`、`whitelist`、`interactive`、`allow_all`）

---

### TC-CWL-20：查看待审批请求列表（无待审批时）

**前置条件**：Bifrost 服务正在运行（端口 8800），无外部连接请求

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist pending
   ```

**预期结果**：
- 输出 `No pending access requests.`

---

### TC-CWL-21：审批待审批请求

**前置条件**：
- Bifrost 服务正在运行，访问模式为 `interactive`
- 有一个来自外部 IP（如 `192.168.8.50`）的连接触发了待审批

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist approve 192.168.8.50
   ```

**预期结果**：
- 输出 `Approved access for: 192.168.8.50`
- 该 IP 被加入临时白名单，后续连接自动放行

---

### TC-CWL-22：拒绝待审批请求

**前置条件**：
- Bifrost 服务正在运行，访问模式为 `interactive`
- 有一个来自外部 IP（如 `192.168.8.51`）的连接触发了待审批

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist reject 192.168.8.51
   ```

**预期结果**：
- 输出 `Rejected access for: 192.168.8.51`
- 该 IP 被标记为本次会话拒绝，后续连接直接拒绝

---

### TC-CWL-23：清除所有待审批请求

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist clear-pending
   ```

**预期结果**：
- 输出 `All pending access requests cleared.`

---

### TC-CWL-24：添加临时白名单条目

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add-temporary 192.168.8.60
   ```

**预期结果**：
- 输出 `Temporary access granted for: 192.168.8.60`
- 该 IP 在当前服务运行期间具有临时访问权限

---

### TC-CWL-25：移除临时白名单条目

**前置条件**：已执行 TC-CWL-24，临时白名单中包含 `192.168.8.60`

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist remove-temporary 192.168.8.60
   ```

**预期结果**：
- 输出 `Temporary access removed for: 192.168.8.60`
- 该 IP 不再具有临时访问权限

---

### TC-CWL-26：使用别名 wl 执行白名单命令

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- wl status
   ```

**预期结果**：
- 输出与 `whitelist status` 完全相同的访问控制状态信息
- 别名 `wl` 等价于 `whitelist`

---

### TC-CWL-27：验证 LAN 访问启用后 list 命令反映状态

**前置条件**：已执行 TC-CWL-10 启用 LAN 访问

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist list
   ```

**预期结果**：
- 底部显示 `LAN (private network) access: enabled`

---

### TC-CWL-28：查看有待审批请求时的 pending 列表

**前置条件**：
- Bifrost 服务正在运行，访问模式为 `interactive`
- 从另一台设备（如 IP `192.168.8.70`）通过代理发起连接，触发待审批

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist pending
   ```

**预期结果**：
- 输出 `Pending Access Requests (N):`（N 为待审批数量，至少为 1）
- 列表中包含 `192.168.8.70`，格式为 `  192.168.8.70 (requested: <时间戳>)`

---

### TC-CWL-29：设置 mode 后再查看确认模式已变更

**前置条件**：Bifrost 服务正在运行（端口 8800）

**操作步骤**：
1. 执行命令设置模式为 `whitelist`：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode whitelist
   ```
2. 执行命令查看当前模式：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist mode
   ```

**预期结果**：
- 第一步输出 `Access mode set to: whitelist`
- 第二步输出 `Current access mode: whitelist`

---

### TC-CWL-30：添加 IPv6 地址到白名单

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add ::1
   ```

**预期结果**：
- 输出 `Added '::1' to whitelist.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

### TC-CWL-31：添加 IPv6 CIDR 到白名单

**操作步骤**：
1. 执行命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- whitelist add fd00::/8
   ```

**预期结果**：
- 输出 `Added 'fd00::/8' to whitelist.`
- 输出 `Note: Restart the proxy server for changes to take effect.`

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
