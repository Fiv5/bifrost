# 网络地址智能识别

## 功能描述

解决当机器存在多个网络接口（物理网卡、VPN 隧道、Docker 桥接、虚拟机网络等）时，代理地址和证书下载 URL 展示了过多无效 IP 的问题。

## 问题分析

原有实现（`proxy.rs`、`cert.rs`、`push.rs` 各自独立的 `get_local_ips` 函数）只过滤了"是否为私有 IP"，未考虑：
1. 虚拟网络接口（docker0、veth、utun、vmnet 等）的 IP 对用户无意义
2. 接口状态（未激活、未运行的接口不应展示）
3. 用户无法区分哪个 IP 是主要可用的

## 实现方案

### 统一模块：`bifrost-admin/src/network.rs`

提取公共 IP 获取逻辑为独立模块，三处调用方统一复用。

### 三层过滤策略

1. **接口 flags 过滤**（仅 Unix）
   - 要求 `IFF_UP` + `IFF_RUNNING`：接口必须已激活且正在运行
   - 排除 `IFF_LOOPBACK`：排除回环接口

2. **接口名称过滤**
   - 过滤已知虚拟接口前缀：`docker`、`br-`、`veth`、`vnet`、`virbr`、`cni`、`flannel`、`calico`、`weave`、`cilium`、`lxc`、`lxd`、`podman`、`tun`、`tap`、`wg`、`tailscale`、`utun`、`ipsec`、`ppp`、`vmnet`、`vmware`、`vboxnet`、`bridge`、`dummy`
   - 大小写不敏感匹配

3. **IP 地址过滤**
   - 仅保留 IPv4 私有地址（`10.x`、`172.16-31.x`、`192.168.x`）
   - 排除回环（`127.x`）、链路本地（`169.254.x`）、IPv6

### Preferred IP 检测

通过 UDP socket 连接 `8.8.8.8:80`（不实际发送数据），获取操作系统路由表选择的默认出口 IP，标记为 `is_preferred`。

### API 变更

`ProxyAddress` 结构体新增 `is_preferred: bool` 字段：

```json
{
  "ip": "10.71.149.76",
  "address": "10.71.149.76:8800",
  "qrcode_url": "/_bifrost/public/proxy/qrcode?ip=10.71.149.76",
  "is_preferred": true
}
```

结果列表按 preferred 优先排序。

### 前端展示

preferred IP 的地址卡片上展示绿色 "Recommended" 标签。

## 关键文件

- `crates/bifrost-admin/src/network.rs` — 核心实现（IP 获取、接口过滤、preferred 检测）
- `crates/bifrost-admin/src/handlers/proxy.rs` — 代理地址 API
- `crates/bifrost-admin/src/handlers/cert.rs` — 证书信息 API
- `crates/bifrost-admin/src/push.rs` — WebSocket 推送
- `web/src/api/proxy.ts` — 前端类型定义
- `web/src/pages/Settings/tabs/ProxyTab.tsx` — 前端展示

## 测试方案

### 单元测试（16 个）
- `test_is_virtual_interface_name_filters_*` — 验证各类虚拟接口名被正确识别
- `test_is_virtual_interface_name_allows_physical` — 验证物理接口不被误过滤
- `test_is_routable_private_ip_*` — 验证 IP 地址分类正确性
- `test_get_local_ips_*` — 验证返回值非空、preferred 排序、无重复

### E2E 测试
- `admin_api_proxy_address_with_preferred_ip` — 验证 API 返回含 `is_preferred` 字段、preferred IP 排在首位

### 真实场景测试
- 启动服务后调用 `GET /api/proxy/address`，对比 `ifconfig` 输出验证虚拟接口被正确过滤
