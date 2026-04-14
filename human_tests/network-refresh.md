# 网络变化自动刷新子网信息

## 功能模块说明

服务启动时获取本机子网快照用于访问控制（`is_in_local_subnet`）判定。后台任务每 30 秒检测网络接口变化，自动刷新子网信息，确保 VPN 连接/断开、WiFi 切换等场景下访问控制策略和 WebUI IP 列表始终准确。

## 前置条件

```bash
BIFROST_DATA_DIR=./.bifrost-test RUST_LOG=bifrost::network=debug,info cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

## 测试用例

### TC-NR-01: 启动时子网初始化日志

**操作步骤**：
1. 启动服务，观察启动日志

**预期结果**：
- 日志中出现 `Updating local subnets:` 字样，包含本机网络接口的子网列表（如 `192.168.x.0/24`）

### TC-NR-02: 网络不变时无刷新日志

**操作步骤**：
1. 启动服务后等待 60 秒以上
2. 观察日志输出

**预期结果**：
- 启动后不再出现 `Local network changed, refreshing subnets` 日志（网络未变化时不触发更新）

### TC-NR-03: VPN 连接后子网自动刷新

**操作步骤**：
1. 启动服务（不连接 VPN）
2. 连接 VPN（如 WireGuard、Tailscale 等）
3. 等待最多 30 秒

**预期结果**：
- 日志出现 `Local network changed, refreshing subnets`，`new` 字段包含 VPN 分配的子网
- 调用 `GET http://localhost:8800/_bifrost/api/proxy/address`，返回的 `local_ips` 包含 VPN 分配的 IP
- 如果 WebUI Settings 页面已打开，页面上的代理地址列表和证书下载地址会自动更新（无需手动刷新），新增 VPN IP 对应的地址

### TC-NR-04: VPN 断开后子网自动刷新

**操作步骤**：
1. 在 TC-NR-03 基础上断开 VPN
2. 等待最多 30 秒

**预期结果**：
- 日志出现 `Local network changed, refreshing subnets`，`new` 字段不再包含 VPN 子网
- 调用 `GET http://localhost:8800/_bifrost/api/proxy/address`，返回的 `local_ips` 不再包含 VPN IP

### TC-NR-05: WiFi 切换后 IP 列表更新

**操作步骤**：
1. 启动服务（连接 WiFi A）
2. 切换到 WiFi B（不同网段）
3. 等待最多 30 秒

**预期结果**：
- 日志出现子网刷新日志
- `GET http://localhost:8800/_bifrost/api/proxy/address` 返回新 WiFi 的 IP
- 如果 WebUI Settings 页面已打开，代理地址列表自动更新为新 WiFi 的 IP

### TC-NR-06: 子网刷新后访问控制策略更新

**操作步骤**：
1. 启动服务，设置 `allow_lan=true`
2. 连接 VPN，等待子网刷新
3. 从 VPN 同网段的另一台设备访问 `http://<bifrost-ip>:8800/_bifrost/`

**预期结果**：
- VPN 同网段设备能正常访问（被识别为局域网设备），无需授权弹窗

### TC-NR-07: 单元测试 test_subnet_hot_update_changes_access_decision 通过

**操作步骤**：
1. 执行命令：`cargo test -p bifrost-core test_subnet_hot_update_changes_access_decision`

**预期结果**：
- 测试通过，输出 `test result: ok. 1 passed`

### TC-NR-08: WebUI Settings 页面网络变化后自动更新 IP 列表

**操作步骤**：
1. 启动服务
2. 在浏览器中打开 `http://localhost:8800/_bifrost/`，进入 Settings 页面
3. 记录当前显示的代理地址列表和证书下载地址
4. 连接 VPN 或切换 WiFi 网络
5. 等待最多 30 秒，观察 Settings 页面

**预期结果**：
- Settings 页面上的代理地址列表自动更新，无需手动刷新页面
- 证书下载地址列表自动更新，包含新网络的 IP 地址
- 旧网络的 IP 地址从列表中消失（如果该网络已断开）

## 清理步骤

```bash
rm -rf ./.bifrost-test
```
