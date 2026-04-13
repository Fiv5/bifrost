# SOCKS5 代理功能测试用例

## 功能模块说明

验证 Bifrost 作为 SOCKS5 代理服务器的核心功能，包括基本 SOCKS5 代理转发、DNS 解析、HTTPS 流量透传等。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录，启用 SOCKS5 端口）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --socks5-port 1180
   ```
2. 确保端口 8800 和 1180 均未被占用
3. SOCKS5 代理监听在 `127.0.0.1:1180`

---

## 测试用例

### TC-PSK-01：SOCKS5 基本 HTTP 代理（curl --socks5）

**操作步骤**：
1. 执行命令：
   ```bash
   curl --socks5 127.0.0.1:1180 http://httpbin.org/get
   ```

**预期结果**：
- 返回 HTTP 200 状态码
- 响应体为 JSON 格式，包含 `"url": "http://httpbin.org/get"`
- 请求通过 SOCKS5 代理成功转发

---

### TC-PSK-02：SOCKS5 代理 DNS 解析（--socks5-hostname）

**操作步骤**：
1. 使用 `--socks5-hostname` 让代理服务器负责 DNS 解析：
   ```bash
   curl --socks5-hostname 127.0.0.1:1180 http://httpbin.org/get
   ```

**预期结果**：
- 返回 HTTP 200 状态码
- 响应体为 JSON 格式，包含 `"url": "http://httpbin.org/get"`
- DNS 解析由 Bifrost 代理服务器完成（而非客户端本地解析）
- 与 TC-PSK-01 的区别在于域名解析发生在代理端

---

### TC-PSK-03：SOCKS5 代理 HTTPS 流量

**操作步骤**：
1. 执行命令：
   ```bash
   curl --socks5-hostname 127.0.0.1:1180 https://httpbin.org/get
   ```

**预期结果**：
- 返回 HTTP 200 状态码
- 响应体为 JSON 格式，包含 `"url": "https://httpbin.org/get"`
- HTTPS 流量通过 SOCKS5 隧道正确传输
- TLS 握手在客户端与目标服务器之间完成，代理仅做透传

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
