# Auth API 测试用例

## 功能模块说明

本文件覆盖 Bifrost 鉴权相关的 REST API 接口测试，包括认证状态查询、登录、密码管理、远程访问开关及会话吊销。

涉及的 API 端点：
- `GET /api/auth/status` — 查询当前鉴权状态
- `POST /api/auth/login` — 用户登录
- `POST /api/auth/change-password` — 设置/修改密码与用户名
- `POST /api/auth/remote` — 启用/禁用远程访问
- `POST /api/auth/revoke-all` — 吊销所有 JWT 会话

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被其他进程占用
3. 所有请求均以 `http://127.0.0.1:8800/_bifrost/` 为基础路径

---

## 测试用例

### TC-AAU-01：GET /api/auth/status 初始状态（未启用远程访问）

**操作步骤**：
1. 执行以下命令查询鉴权状态：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体 JSON 包含以下字段：
  - `remote_access_enabled`: `false`
  - `auth_required`: `false`
  - `username`: `"admin"`
  - `has_password`: `false`

---

### TC-AAU-02：POST /api/auth/remote 启用远程访问

**前置条件**：需先设置密码，否则启用会失败；此处先验证无密码时启用远程访问被拒绝，再设置密码后启用。

**操作步骤**：
1. 在未设置密码的情况下尝试启用远程访问：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/remote \
     -H "Content-Type: application/json" \
     -d '{"enabled": true}' | jq .
   ```
2. 先设置密码（参考 TC-AAU-04），再重新执行：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/remote \
     -H "Content-Type: application/json" \
     -d '{"enabled": true}' | jq .
   ```

**预期结果**：
- 步骤 1：返回错误，提示需先设置密码才能启用远程访问
- 步骤 2：返回 HTTP 200，远程访问成功启用

---

### TC-AAU-03：GET /api/auth/status 确认已启用远程访问

**前置条件**：已通过 TC-AAU-02 成功启用远程访问

**操作步骤**：
1. 执行以下命令查询鉴权状态：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体 JSON 包含以下字段：
  - `remote_access_enabled`: `true`
  - `auth_required`: `false`（本地请求不需要鉴权）
  - `username`: `"admin"`
  - `has_password`: `true`

---

### TC-AAU-04：POST /api/auth/change-password 设置密码

**操作步骤**：
1. 执行以下命令设置密码：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/change-password \
     -H "Content-Type: application/json" \
     -d '{"password": "test123456"}' | jq .
   ```
2. 查询状态确认密码已设置：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .has_password
   ```

**预期结果**：
- 步骤 1：返回 HTTP 200，提示密码设置成功
- 步骤 2：输出 `true`

---

### TC-AAU-05：POST /api/auth/login 正确凭证登录成功

**前置条件**：已通过 TC-AAU-04 设置密码，已通过 TC-AAU-02 启用远程访问

**操作步骤**：
1. 执行以下命令使用正确凭证登录：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin", "password": "test123456"}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体 JSON 包含以下字段：
  - `token`: 非空字符串（JWT token）
  - `expires_at`: 有效的过期时间戳
  - `username`: `"admin"`

---

### TC-AAU-06：POST /api/auth/login 错误密码返回 401

**前置条件**：已设置密码，已启用远程访问

**操作步骤**：
1. 执行以下命令使用错误密码登录：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin", "password": "wrongpassword"}'
   ```
2. 查看完整响应体：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin", "password": "wrongpassword"}' | jq .
   ```

**预期结果**：
- 步骤 1：返回 HTTP 状态码 `401`
- 步骤 2：响应体包含错误信息，提示用户名或密码不正确

---

### TC-AAU-07：使用返回的 JWT token 访问受保护 API

**前置条件**：已通过 TC-AAU-05 获取有效 JWT token

**操作步骤**：
1. 先登录获取 token：
   ```bash
   TOKEN=$(curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin", "password": "test123456"}' | jq -r .token)
   echo $TOKEN
   ```
2. 使用 token 访问需要鉴权的 API：
   ```bash
   curl -s -H "Authorization: Bearer $TOKEN" \
     http://127.0.0.1:8800/_bifrost/api/auth/status | jq .
   ```

**预期结果**：
- 步骤 1：成功获取非空 JWT token
- 步骤 2：返回 HTTP 200，正常返回鉴权状态 JSON

---

### TC-AAU-08：远程 IP 不带 token 访问 API 返回 401

**前置条件**：已启用远程访问，使用局域网 IP（如 `192.168.x.x`）模拟远程请求

**操作步骤**：
1. 获取本机局域网 IP：
   ```bash
   LAN_IP=$(ifconfig | grep "inet " | grep -v 127.0.0.1 | awk '{print $2}' | head -1)
   echo $LAN_IP
   ```
2. 使用局域网 IP 不带 token 访问 API：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://$LAN_IP:8800/_bifrost/api/rules
   ```
3. 使用 localhost 访问同一 API（验证本地不需要鉴权）：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/rules
   ```

**预期结果**：
- 步骤 2：返回 HTTP 状态码 `401`（远程 IP 未携带 token，拒绝访问）
- 步骤 3：返回 HTTP 状态码 `200`（本地请求免鉴权）

---

### TC-AAU-09：POST /api/auth/revoke-all 吊销所有会话

**前置条件**：已通过 TC-AAU-05 登录获取 token

**操作步骤**：
1. 先登录获取 token：
   ```bash
   TOKEN=$(curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin", "password": "test123456"}' | jq -r .token)
   ```
2. 确认 token 有效：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" \
     http://127.0.0.1:8800/_bifrost/api/auth/status
   ```
3. 执行吊销所有会话：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/revoke-all | jq .
   ```
4. 使用之前的 token 再次访问：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" \
     http://$LAN_IP:8800/_bifrost/api/auth/status
   ```

**预期结果**：
- 步骤 2：返回 `200`（token 有效）
- 步骤 3：返回 HTTP 200，提示所有会话已吊销
- 步骤 4：返回 `401`（token 已失效，远程 IP 请求被拒绝）

---

### TC-AAU-10：POST /api/auth/remote 禁用远程访问

**前置条件**：远程访问当前处于启用状态

**操作步骤**：
1. 执行以下命令禁用远程访问：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/remote \
     -H "Content-Type: application/json" \
     -d '{"enabled": false}' | jq .
   ```
2. 查询状态确认已禁用：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .remote_access_enabled
   ```

**预期结果**：
- 步骤 1：返回 HTTP 200，提示远程访问已禁用
- 步骤 2：输出 `false`

---

### TC-AAU-11：POST /api/auth/change-password 修改用户名

**操作步骤**：
1. 执行以下命令同时修改用户名和密码：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/change-password \
     -H "Content-Type: application/json" \
     -d '{"username": "superadmin", "password": "newpass789"}' | jq .
   ```
2. 查询状态确认用户名已更新：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .username
   ```
3. 重新启用远程访问后使用新凭证登录验证：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/remote \
     -H "Content-Type: application/json" \
     -d '{"enabled": true}' | jq .

   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "superadmin", "password": "newpass789"}' | jq .
   ```
4. 使用旧凭证登录验证失败：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin", "password": "test123456"}'
   ```

**预期结果**：
- 步骤 1：返回 HTTP 200，提示凭证更新成功
- 步骤 2：输出 `"superadmin"`
- 步骤 3：使用新用户名和密码登录成功，返回有效 token
- 步骤 4：使用旧凭证登录返回 HTTP 状态码 `401`

---

### TC-AAU-12：未启用远程访问时 POST /api/auth/login 返回 403

**前置条件**：远程访问处于禁用状态（如需要，先执行 TC-AAU-10 禁用远程访问）

**操作步骤**：
1. 确认远程访问已禁用：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .remote_access_enabled
   ```
2. 尝试登录：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "superadmin", "password": "newpass789"}'
   ```
3. 查看完整响应体：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username": "superadmin", "password": "newpass789"}' | jq .
   ```

**预期结果**：
- 步骤 1：输出 `false`
- 步骤 2：返回 HTTP 状态码 `403`
- 步骤 3：响应体包含错误信息，提示远程访问未启用，无法登录

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
