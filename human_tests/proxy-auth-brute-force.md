# 代理认证暴力破解防护测试

## 功能模块说明

验证 HTTP 代理和 SOCKS5 代理的用户名/密码认证暴力破解防护机制（SEC-05），包括：
- 每 IP 失败计数追踪
- 达到 10 次失败后临时封禁 5 分钟
- HTTP 代理返回 429 Too Many Requests
- SOCKS5 代理断开连接
- 认证成功后计数重置
- 封禁期过后自动解除（自动清理机制）

## 前置条件

1. 启动 Bifrost 服务：

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

2. 通过 API 配置代理认证并启用 loopback 认证要求（`--proxy-user` 启动参数会硬编码 `loopback_requires_auth=false`，所以必须通过 API 配置）：

```bash
curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/userpass \
  -H "Content-Type: application/json" \
  -d '{"enabled":true,"accounts":[{"username":"testuser","password":"TestPass123","enabled":true}],"loopback_requires_auth":true}'
```

3. 验证代理认证已生效：

```bash
curl -s -o /dev/null -w "%{http_code}" -x http://127.0.0.1:8800 http://httpbin.org/get
# 预期：407（需要代理认证）
```

> **注意**：每个涉及封禁的测试用例执行前需要重启服务以重置 rate limiter 计数器。重启后配置会自动从持久化存储恢复。

## 测试用例列表

### TC-PAB-01: HTTP 代理 — 正确凭证可正常通过

**操作步骤：**
1. 使用正确的代理凭证发起请求：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" \
     -x http://testuser:TestPass123@127.0.0.1:8800 \
     http://httpbin.org/get
   ```

**预期结果：**
- 返回 HTTP 200

### TC-PAB-02: HTTP 代理 — 错误凭证返回 407

**操作步骤：**
1. 使用错误的代理凭证发起请求：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" \
     -x http://testuser:WrongPass@127.0.0.1:8800 \
     http://httpbin.org/get
   ```

**预期结果：**
- 返回 HTTP 407

### TC-PAB-03: HTTP 代理 — 连续失败不超过阈值仍可正常认证

**操作步骤：**
1. 连续发送 5 次错误凭证请求：
   ```bash
   for i in $(seq 1 5); do
     echo "--- Attempt $i ---"
     curl -s -o /dev/null -w "HTTP %{http_code}\n" \
       -x http://testuser:WrongPass@127.0.0.1:8800 \
       http://httpbin.org/get
   done
   ```
2. 然后使用正确凭证请求：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" \
     -x http://testuser:TestPass123@127.0.0.1:8800 \
     http://httpbin.org/get
   ```

**预期结果：**
- 5 次错误请求均返回 HTTP 407
- 正确凭证请求返回 HTTP 200（未达 10 次阈值，未被封禁）

### TC-PAB-04: HTTP 代理 — 达到 10 次失败后返回 429

**操作步骤：**
1. 先重启服务以重置计数器（或等待清理周期后执行）：
   ```bash
   # 先停止前一个服务，重新启动（配置已持久化，无需再通过 API 配置）
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 连续发送 10 次错误凭证请求：
   ```bash
   for i in $(seq 1 10); do
     echo "--- Attempt $i ---"
     curl -s -o /dev/null -w "HTTP %{http_code}\n" \
       -x http://testuser:WrongPass@127.0.0.1:8800 \
       http://httpbin.org/get
   done
   ```
3. 发送第 11 次请求（无论凭证正确与否）：
   ```bash
   curl -s -w "\nHTTP %{http_code}\n" \
     -x http://testuser:TestPass123@127.0.0.1:8800 \
     http://httpbin.org/get
   ```

**预期结果：**
- 前 10 次错误请求返回 HTTP 407
- 第 11 次请求返回 HTTP 429
- 响应头包含 `Retry-After: 300`
- 响应体包含 "Too many failed authentication attempts"

### TC-PAB-05: HTTP 代理 — 封禁期间即使正确凭证也被拒绝（429）

**操作步骤：**
1. 紧接 TC-PAB-04 之后（IP 已被封禁），使用正确凭证发起请求：
   ```bash
   curl -s -w "\nHTTP %{http_code}\n" \
     -x http://testuser:TestPass123@127.0.0.1:8800 \
     http://httpbin.org/get
   ```

**预期结果：**
- 返回 HTTP 429（IP 被封禁，直接拒绝，不进入凭证验证）

### TC-PAB-06: SOCKS5 代理 — 正确凭证可正常通过

**操作步骤：**
1. 使用正确的 SOCKS5 凭证发起请求：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" \
     --socks5 127.0.0.1:8800 \
     --proxy-user testuser:TestPass123 \
     http://httpbin.org/get
   ```

**预期结果：**
- 返回 HTTP 200

### TC-PAB-07: SOCKS5 代理 — 错误凭证返回连接错误

**操作步骤：**
1. 使用错误的 SOCKS5 凭证发起请求：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" \
     --socks5 127.0.0.1:8800 \
     --proxy-user testuser:WrongPass \
     http://httpbin.org/get 2>&1
   ```

**预期结果：**
- curl 返回连接错误（SOCKS5 认证失败断开连接）
- 不返回 HTTP 200

### TC-PAB-08: SOCKS5 代理 — 达到 10 次失败后封禁

**操作步骤：**
1. 先重启服务重置计数器
2. 连续发送 10 次错误 SOCKS5 凭证：
   ```bash
   for i in $(seq 1 10); do
     echo "--- Attempt $i ---"
     curl -s --socks5 127.0.0.1:8800 \
       --proxy-user testuser:WrongPass \
       http://httpbin.org/get 2>&1 | head -1
   done
   ```
3. 第 11 次用正确凭证：
   ```bash
   curl -s --socks5 127.0.0.1:8800 \
     --proxy-user testuser:TestPass123 \
     http://httpbin.org/get 2>&1
   ```

**预期结果：**
- 前 10 次认证失败（连接错误）
- 第 11 次即使凭证正确也被拒绝（IP 被封禁）

### TC-PAB-09: 认证成功后失败计数重置

**操作步骤：**
1. 重启服务重置计数器
2. 发送 5 次错误凭证：
   ```bash
   for i in $(seq 1 5); do
     curl -s -o /dev/null -w "HTTP %{http_code}\n" \
       -x http://testuser:WrongPass@127.0.0.1:8800 \
       http://httpbin.org/get
   done
   ```
3. 发送 1 次正确凭证：
   ```bash
   curl -s -o /dev/null -w "HTTP %{http_code}\n" \
     -x http://testuser:TestPass123@127.0.0.1:8800 \
     http://httpbin.org/get
   ```
4. 再发送 9 次错误凭证（不应触发封禁，因为计数已重置）：
   ```bash
   for i in $(seq 1 9); do
     echo "--- Attempt $i ---"
     curl -s -o /dev/null -w "HTTP %{http_code}\n" \
       -x http://testuser:WrongPass@127.0.0.1:8800 \
       http://httpbin.org/get
   done
   ```
5. 发送 1 次正确凭证：
   ```bash
   curl -s -o /dev/null -w "HTTP %{http_code}\n" \
     -x http://testuser:TestPass123@127.0.0.1:8800 \
     http://httpbin.org/get
   ```

**预期结果：**
- 步骤 2：5 次返回 407
- 步骤 3：返回 200
- 步骤 4：9 次返回 407（不是 429，因为成功认证后计数已重置）
- 步骤 5：返回 200

### TC-PAB-10: 不同 IP 独立计数（非 loopback 验证）

**操作步骤：**
1. 此用例验证 ProxyAuthRateLimiter 的单元测试中的 IP 隔离逻辑
2. 使用 cargo test 验证：
   ```bash
   cargo test -p bifrost-core test_rate_limiter
   ```

**预期结果：**
- 单元测试通过，验证不同 IP 的失败计数互不影响

## 清理步骤

```bash
# 停止 Bifrost 服务（Ctrl+C）
rm -rf ./.bifrost-test
```
