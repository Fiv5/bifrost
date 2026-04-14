# CGN 地址段支持与同子网局域网判定

## 功能模块说明

验证 Bifrost 代理服务的局域网判定逻辑：
1. 正确识别 CGN（Carrier-Grade NAT，RFC 6598）地址段 `100.64.0.0/10`
2. **同子网判定**：`allow_lan` 开启时，连接来源 IP 与本机任一网卡 IP 在同一子网即视为局域网（而不仅仅是硬编码私有网段）
3. 本机 IP 列表正确展示 CGN 地址

## 前置条件

1. 编译并启动 Bifrost 服务：

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

2. 确认服务正常运行：

```bash
curl -s http://127.0.0.1:8800/_bifrost/api/system/info | head -1
```

## 测试用例

### TC-CGN-01: is_private_network 识别 CGN 地址段 — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-core test_private_network_detection`

**预期结果**：
- 测试通过，`100.64.0.1`、`100.86.178.33`、`100.127.255.255` 被识别为私有网络
- `100.63.255.255`、`100.128.0.1` 不被识别为私有网络

### TC-CGN-02: allow_lan 开启时 CGN 地址直接允许 — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-core test_cgn_address_allowed_with_allow_lan`

**预期结果**：
- 测试通过，Interactive 模式 + allow_lan=true 时，CGN IP `100.86.178.33` 返回 `AccessDecision::Allow`

### TC-CGN-03: allow_lan 关闭时 CGN 地址触发 Prompt — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-core test_cgn_address_prompts_without_allow_lan`

**预期结果**：
- 测试通过，Interactive 模式 + allow_lan=false 时，CGN IP `100.86.178.33` 返回 `AccessDecision::Prompt`

### TC-CGN-04: is_routable_private_ip 识别 CGN 地址 — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-admin test_is_routable_private_ip_accepts_cgn`

**预期结果**：
- 测试通过，`100.64.0.1`、`100.86.178.33`、`100.127.255.255` 被识别为可路由私有 IP

### TC-CGN-05: is_routable_private_ip 拒绝非 CGN 的 100.x 地址 — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-admin test_is_routable_private_ip_rejects_non_cgn_100`

**预期结果**：
- 测试通过，`100.63.255.255`、`100.128.0.1` 不被识别为可路由私有 IP

### TC-CGN-06: API allow-lan 开启后 CGN 地址可访问服务

**操作步骤**：
1. 启动服务（见前置条件）
2. 通过 API 开启 allow_lan：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan -H 'Content-Type: application/json' -d '{"allow_lan": true}'
   ```
3. 验证 allow_lan 状态：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan
   ```

**预期结果**：
- allow_lan 返回 `true`
- 响应中 `allow_lan` 字段为 `true`

### TC-CGN-07: 本机 IP 列表 API 返回值验证

**操作步骤**：
1. 启动服务
2. 调用代理地址 API：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/proxy/address
   ```

**预期结果**：
- 返回的 IP 列表中所有地址均为有效的私有/CGN 地址
- 不包含公网地址（如 `8.8.8.8` 等）
- 如果本机有 CGN 地址（100.64-127.x），该地址应出现在列表中

### TC-CGN-08: 同子网判定 — 同一子网的公网 IP 视为局域网 — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-core test_local_subnet_detection_allows_same_subnet`

**预期结果**：
- 测试通过
- 当本机子网为 `203.0.113.0/24` 时，`203.0.113.50`（同子网）被视为局域网，`check_access` 返回 Allow
- `203.0.114.50`（不同子网）不被视为局域网，`check_access` 返回 Prompt

### TC-CGN-09: 同子网判定 — CGN 网段的精确子网匹配 — 单元测试验证

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-core test_local_subnet_detection_any_public_ip_in_same_subnet`

**预期结果**：
- 测试通过
- 当本机子网为 `100.86.0.0/16` 时，`100.86.178.33`（同子网）被视为局域网
- `100.87.0.1`（不同子网）不在本机子网中

## 清理步骤

1. 停止测试服务：`Ctrl+C` 或 `kill` 对应进程
2. 清理临时数据：`rm -rf ./.bifrost-test`
