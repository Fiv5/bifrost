# CLI Config 配置管理命令测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确认服务启动成功，端口 8800 可用
3. 以下命令均在另一个终端窗口中执行

---

## 测试用例

### TC-CCF-01：显示流量配置（位置参数形式）

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config show traffic
   ```

**预期结果**：
- 输出 traffic 分区的配置信息
- 包含 `max_records`、`max_db_size_bytes`、`max_body_memory_size`、`max_body_buffer_size`、`file_retention_days` 等字段
- 输出为人类可读的表格/文本格式

---

### TC-CCF-02：显示流量配置（--section 参数形式）

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config show --section traffic
   ```

**预期结果**：
- 输出与 TC-CCF-01 一致
- 包含 traffic 分区的所有配置项
- 格式为人类可读的文本格式

---

### TC-CCF-03：显示全部配置（JSON 格式）

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config show --json
   ```

**预期结果**：
- 输出为格式化的 JSON 字符串
- JSON 包含顶层字段：`server`、`tls`、`traffic`、`access`
- `server` 包含 `timeout_secs`、`http1_max_header_size`、`http2_max_header_list_size`、`websocket_handshake_max_header_size`
- `tls` 包含 `enable_tls_interception`、`unsafe_ssl`、`disconnect_on_config_change`、`intercept_exclude`、`intercept_include`
- `traffic` 包含 `max_records`、`max_db_size_bytes` 等
- `access` 包含 `mode`、`allow_lan`
- JSON 可被 `jq` 等工具正确解析

---

### TC-CCF-04：获取单个配置值（tls.enabled）

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config get tls.enabled
   ```

**预期结果**：
- 输出 TLS 拦截的启用状态
- 值为 `true` 或 `false`
- 输出为人类可读格式，包含键名和值

---

### TC-CCF-05：获取单个配置值（tls.enabled，JSON 格式）

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config get tls.enabled --json
   ```

**预期结果**：
- 输出为 JSON 格式的布尔值（`true` 或 `false`）
- 可被 `jq` 正确解析

---

### TC-CCF-06：设置流量最大记录数

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config set traffic.max-records 10000
   ```

**预期结果**：
- 输出 `✓ max-records set to 10000`

---

### TC-CCF-07：验证设置生效

**前置条件**：已执行 TC-CCF-06

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config get traffic.max-records
   ```

**预期结果**：
- 输出的值为 `10000`

---

### TC-CCF-08：向 TLS 排除列表添加域名

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config add tls.exclude '*.example.com'
   ```

**预期结果**：
- 输出 `✓ Added '*.example.com' to tls.exclude`

---

### TC-CCF-09：验证添加的域名已在排除列表中

**前置条件**：已执行 TC-CCF-08

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config get tls.exclude
   ```

**预期结果**：
- 输出的列表中包含 `*.example.com`

---

### TC-CCF-10：重复添加相同域名应提示已存在

**前置条件**：已执行 TC-CCF-08

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config add tls.exclude '*.example.com'
   ```

**预期结果**：
- 输出 `⚠ '*.example.com' already exists in tls.exclude`
- 列表不会出现重复项

---

### TC-CCF-11：从 TLS 排除列表移除域名

**前置条件**：已执行 TC-CCF-08，列表中包含 `*.example.com`

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config remove tls.exclude '*.example.com'
   ```

**预期结果**：
- 输出 `✓ Removed '*.example.com' from tls.exclude`

---

### TC-CCF-12：验证域名已从排除列表中移除

**前置条件**：已执行 TC-CCF-11

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config get tls.exclude
   ```

**预期结果**：
- 输出的列表中不再包含 `*.example.com`

---

### TC-CCF-13：重置单个配置项为默认值

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config reset tls.enabled -y
   ```

**预期结果**：
- 输出包含 `✓ tls.enabled reset to`
- `tls.enabled` 被重置为默认值

---

### TC-CCF-14：清除所有缓存

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config clear-cache -y
   ```

**预期结果**：
- 输出 `✓` 后跟缓存清理的结果消息
- 命令成功执行，退出码为 0

---

### TC-CCF-15：断开指定域名的连接

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config disconnect example.com
   ```

**预期结果**：
- 输出 `✓` 后跟断开连接的结果消息
- 命令成功执行，退出码为 0

---

### TC-CCF-16：按应用断开连接

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config disconnect-by-app Chrome
   ```

**预期结果**：
- 输出 `Disconnected connections for app: Chrome`
- 命令成功执行，退出码为 0

---

### TC-CCF-17：导出配置为 TOML 文件

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config export -o ./config-test.toml --format toml
   ```

**预期结果**：
- 输出 `✓ Configuration exported to ./config-test.toml`
- 文件 `./config-test.toml` 被创建
- 文件内容为合法的 TOML 格式
- 包含 `[server]`、`[tls]`、`[traffic]`、`[access]` 分区
- `[server]` 包含 `timeout_secs`、`http1_max_header_size` 等
- `[tls]` 包含 `enable_interception`、`unsafe_ssl` 等
- `[traffic]` 包含 `max_records`、`max_db_size_bytes` 等
- `[access]` 包含 `mode`、`allow_lan`

---

### TC-CCF-18：导出配置为 JSON 格式（输出到 stdout）

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config export --format json
   ```

**预期结果**：
- 输出为格式化的 JSON 字符串（直接打印到终端）
- JSON 包含 `server`、`tls`、`traffic`、`access` 顶层字段
- `server` 包含 `timeout_secs`、`http1_max_header_size`、`http2_max_header_list_size`、`websocket_handshake_max_header_size`
- `tls` 包含 `enabled`、`unsafe_ssl`、`disconnect_on_change`、`exclude`、`include`、`app_exclude`、`app_include`
- `traffic` 包含 `max_records`、`max_db_size_bytes`、`max_body_size`、`max_buffer_size`、`retention_days`
- `access` 包含 `mode`、`allow_lan`
- JSON 可被 `jq` 等工具正确解析

---

### TC-CCF-19：设置流量数据库最大大小

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config set traffic.max-db-size 2GB
   ```

**预期结果**：
- 输出 `✓ max-db-size set to 2 GB`（或等效的大小格式化表示）

---

### TC-CCF-20：验证数据库大小设置生效

**前置条件**：已执行 TC-CCF-19

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config get traffic.max-db-size --json
   ```

**预期结果**：
- 输出 JSON 数字值为 `2147483648`（2GB = 2 × 1024 × 1024 × 1024 字节）

---

### TC-CCF-21：查看活跃连接列表

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config connections
   ```

**预期结果**：
- 输出标题行 `Active Connections (N):`（N 为当前活跃连接数）
- 如果有活跃连接：
  - 显示表头包含 `REQ ID`、`HOST`、`PORT`、`INTERCEPT`、`APP`
  - 每行显示一个连接的详细信息
- 如果没有活跃连接：
  - 显示 `No active connections.`

---

### TC-CCF-22：查看内存诊断信息

**操作步骤**：
1. 执行命令：
   ```bash
   cargo run --bin bifrost -- -p 8800 config memory
   ```

**预期结果**：
- 输出标题 `Memory Diagnostics`
- 输出分隔线 `==================`
- `Process:` 区域包含：
  - `PID:` 后跟进程 ID 数字
  - `RSS:` 后跟内存占用（MiB 和 GiB 格式）
  - `Virtual:` 后跟虚拟内存（MiB 和 GiB 格式）
  - `CPU:` 后跟 CPU 使用率百分比
  - `System RAM:` 后跟系统总内存（GiB 格式）
- `Connections:` 区域包含 Tunnel active、SSE total、SSE open 等计数
- `Stores:` 区域包含 Body store、Frame store、WS payload 的文件数量和大小
- `Traffic DB:` 区域包含 Records 记录数、DB size 数据库大小、Cache entries 缓存条目数

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
rm -f ./config-test.toml
```
