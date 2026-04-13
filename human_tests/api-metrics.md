# Metrics 管理 API 测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 服务启动成功后，确认管理端可访问：`http://127.0.0.1:8800/_bifrost/`
3. 产生一些流量数据以便验证指标统计（可通过 curl 发送若干代理请求，或直接测试空数据下的默认响应）

---

## 测试用例

### TC-AME-01：获取当前指标快照 — 基本结构验证

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含以下顶层字段：
  - `timestamp`（数字，毫秒级时间戳）
  - `memory_used`（数字，进程 RSS，单位 bytes）
  - `memory_total`（数字，系统总内存，单位 bytes）
  - `cpu_usage`（浮点数，CPU 使用率百分比）
  - `total_requests`（数字，累计请求数）
  - `active_connections`（数字，当前活跃连接数）
  - `bytes_sent`（数字）
  - `bytes_received`（数字）
  - `bytes_sent_rate`（浮点数，发送速率）
  - `bytes_received_rate`（浮点数，接收速率）
  - `qps`（浮点数，每秒请求数）
  - `max_qps`（浮点数）
  - `max_bytes_sent_rate`（浮点数）
  - `max_bytes_received_rate`（浮点数）

---

### TC-AME-02：获取当前指标快照 — 协议分类统计验证

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics | jq '{http, https, tunnel, ws, wss, h3, socks5}'
   ```

**预期结果**：
- 返回 JSON 包含 `http`、`https`、`tunnel`、`ws`、`wss`、`h3`、`socks5` 七个协议分类对象
- 每个协议对象包含以下字段：
  - `requests`（数字）
  - `bytes_sent`（数字）
  - `bytes_received`（数字）
  - `active_connections`（数字）
- 刚启动时各协议 `requests` 值为 0 或与实际流量一致

---

### TC-AME-03：获取当前指标快照 — memory 和 CPU 值合理性

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics | jq '{memory_used, memory_total, cpu_usage}'
   ```

**预期结果**：
- `memory_used` > 0（进程必须占用内存）
- `memory_total` > `memory_used`（系统总内存大于进程使用量）
- `cpu_usage` >= 0（CPU 使用率为非负数）

---

### TC-AME-04：获取指标历史记录 — 默认无 limit 参数

**操作步骤**：
1. 等待至少 5 秒以确保采集器产生了一些历史数据
2. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/history | jq 'length'
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 数组
- 数组长度 >= 1（至少有一条历史快照）
- 每条记录结构与 `/api/metrics` 返回的 `MetricsSnapshot` 一致

---

### TC-AME-05：获取指标历史记录 — 指定 limit 参数

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/api/metrics/history?limit=5" | jq 'length'
   ```

**预期结果**：
- 返回 JSON 数组长度 <= 5
- 数组中每条记录结构完整，包含 `timestamp`、`memory_used`、`qps` 等字段

---

### TC-AME-06：获取指标历史记录 — limit=1 只返回一条

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/api/metrics/history?limit=1" | jq 'length'
   ```

**预期结果**：
- 返回 JSON 数组长度为 1
- 该条记录的 `timestamp` 为最近的采集时间点

---

### TC-AME-07：获取指标历史记录 — limit=50

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/api/metrics/history?limit=50" | jq 'length'
   ```

**预期结果**：
- 返回 JSON 数组长度 <= 50
- 记录按时间顺序排列

---

### TC-AME-08：获取应用维度统计 — 基本结构验证

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/apps | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 数组
- 每个元素包含以下字段：
  - `app_name`（字符串，应用名称）
  - `requests`（数字，请求总数）
  - `active_connections`（数字，活跃连接数）
  - `bytes_sent`（数字）
  - `bytes_received`（数字）
  - `http_requests`（数字）
  - `https_requests`（数字）
  - `tunnel_requests`（数字）
  - `ws_requests`（数字）
  - `wss_requests`（数字）
  - `h3_requests`（数字）
  - `socks5_requests`（数字）

---

### TC-AME-09：获取应用维度统计 — 无流量时返回空数组

**前置条件**：刚启动的全新服务，未产生任何代理流量

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/apps | jq 'length'
   ```

**预期结果**：
- 返回 JSON 数组长度为 0（无流量时无应用统计）

---

### TC-AME-10：获取应用维度统计 — 按请求数降序排列

**前置条件**：通过代理产生了来自不同应用的流量

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/apps | jq '.[0].requests, .[-1].requests'
   ```

**预期结果**：
- 数组按 `requests` 字段降序排列
- 第一个元素的 `requests` >= 最后一个元素的 `requests`

---

### TC-AME-11：获取主机维度统计 — 基本结构验证

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/hosts | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 数组
- 每个元素包含以下字段：
  - `host`（字符串，主机名）
  - `requests`（数字，请求总数）
  - `active_connections`（数字，活跃连接数）
  - `bytes_sent`（数字）
  - `bytes_received`（数字）
  - `http_requests`（数字）
  - `https_requests`（数字）
  - `tunnel_requests`（数字）
  - `ws_requests`（数字）
  - `wss_requests`（数字）
  - `h3_requests`（数字）
  - `socks5_requests`（数字）

---

### TC-AME-12：获取主机维度统计 — 无流量时返回空数组

**前置条件**：刚启动的全新服务，未产生任何代理流量

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/hosts | jq 'length'
   ```

**预期结果**：
- 返回 JSON 数组长度为 0

---

### TC-AME-13：获取主机维度统计 — 按请求数降序排列

**前置条件**：通过代理产生了访问不同主机的流量

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/metrics/hosts | jq '.[0].requests, .[-1].requests'
   ```

**预期结果**：
- 数组按 `requests` 字段降序排列
- 第一个元素的 `requests` >= 最后一个元素的 `requests`

---

### TC-AME-14：不支持的 HTTP 方法返回 405

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/metrics
   ```

**预期结果**：
- HTTP 状态码为 405（Method Not Allowed）

---

### TC-AME-15：不存在的 metrics 子路径返回 404

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/metrics/nonexistent
   ```

**预期结果**：
- HTTP 状态码为 404（Not Found）

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
