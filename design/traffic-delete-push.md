## 背景
- 管理端删除流量数据后，前端列表未及时移除
- 删除流量数据时仅清理部分缓存文件，空间回收不充分
- 详情请求找不到时仍显示上一次详情内容

## 目标
- 服务端在删除流量数据后通过 push 通道下发被删除的 id 列表
- 删除流量数据时批量清理相关缓存文件（含 WS payload）
- 详情请求缺失时显示明确原因，不复用旧详情

## 方案
### Push 通道扩展
- 新增 push 消息类型 `traffic_deleted`，数据结构包含 `ids: string[]`
- 删除流量数据成功后向所有客户端广播 `traffic_deleted`
- 前端订阅并移除对应记录，同时清理选中态与详情状态

### 批量缓存清理
- 为 `WsPayloadStore` 增加 `delete_by_ids`，按连接 id 删除对应 payload 文件，并从 LRU writer 缓存移除
- 删除流量数据时同步调用 `ws_payload_store.delete_by_ids`，与 body/frame/traffic 同步清理

### 防止僵尸缓存文件
- 启动入口统一开启周期清理任务：body、ws_payload、frame、traffic、connection_monitor
- FrameStore 增加周期清理任务，仅清理已标记关闭且超过保留期的连接文件
- E2E 启动路径补齐清理任务，避免测试进程留下缓存文件

### 详情缺失提示
- 前端 store 增加 `detailError` 状态
- 详情请求失败时清空 `currentRecord/requestBody/responseBody` 并记录错误原因
- 详情组件在 `record` 为空且存在 `detailError` 时展示错误原因

## 数据结构
- 服务端：`TrafficDeletedData { ids: Vec<String> }`
- 前端：`TrafficDeletedData { ids: string[] }`

## 兼容性
- 新消息类型为可选扩展，旧客户端忽略不识别的消息
- 前端修改仅影响详情展示与列表移除逻辑

## 测试与验证
- 删除单条/多条流量记录，确认 push 删除通知触发列表移除
- 删除流量后检查 payload/body/frame 文件是否被清理
- 请求不存在的详情时，页面显示明确原因
- 新增用例覆盖已关闭连接清理与活跃连接保留 payload
