# SQLite Cache Size 与内存优化

## 功能模块说明
优化 SQLite `cache_size` PRAGMA 配置和 `frame_store.metadata_cache` 数据结构，降低空闲状态下的内存占用。

## 前置条件
- 使用优化后的代码编译 bifrost
- 启动服务：`BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl`
- 产生一些流量记录（至少 100 条）

## 测试用例

### TC-MSC-01: 服务正常启动且功能不受影响
**操作步骤**：
1. 编译并启动 bifrost 服务
2. 访问 `http://localhost:8800/_bifrost/` 确认 Web UI 可正常加载

**预期结果**：
- 服务正常启动，无 SQLite 错误日志
- Web UI 正常加载

### TC-MSC-02: 流量记录写入与读取正常
**操作步骤**：
1. 通过代理发起 HTTP 请求产生流量
2. 调用 `curl http://localhost:8800/_bifrost/api/traffic?limit=10` 查看流量列表
3. 取一条记录 ID，调用 `curl http://localhost:8800/_bifrost/api/traffic/{id}` 查看详情

**预期结果**：
- 流量列表正常返回
- 详情接口正常返回完整记录

### TC-MSC-03: 内存诊断接口返回有效数据
**操作步骤**：
1. 调用 `curl http://localhost:8800/_bifrost/api/system/memory`

**预期结果**：
- 返回 JSON 包含 `rss_mb`、`frame_store.metadata_cache_len` 等字段
- `frame_store.metadata_cache_len` 不超过 1000（LRU 上限）

### TC-MSC-04: WebSocket 帧存储功能正常
**操作步骤**：
1. 通过代理发起 WebSocket 连接
2. 发送和接收消息
3. 关闭连接
4. 查看流量详情确认帧记录完整

**预期结果**：
- WebSocket 帧正常记录和展示
- 连接关闭后 metadata 正常标记为 closed

### TC-MSC-05: Replay 功能正常
**操作步骤**：
1. 在 Web UI 的 Replay 页面创建一个请求
2. 执行请求
3. 查看执行历史

**预期结果**：
- 请求创建、执行、历史记录功能均正常

### TC-MSC-06: RSS 内存占用合理
**操作步骤**：
1. 启动服务并产生约 500 条流量后等待稳定
2. 调用内存诊断接口或 `ps` 查看 RSS

**预期结果**：
- RSS 相比优化前应有显著降低（目标 < 200MB）

## 清理步骤
- 停止 bifrost 服务
- 删除临时数据：`rm -rf ./.bifrost-test`
