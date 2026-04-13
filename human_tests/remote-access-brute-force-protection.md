# 远程访问暴力破解防护测试

## 功能模块说明

验证远程访问登录暴力破解防护机制，包括：
- 登录失败计数与剩余次数提示
- 达到最大失败次数后自动锁定（停用远程访问 + 删除密码）
- 密码强度校验（至少 6 字符，必须包含字母和数字）
- 本机恢复流程

## 前置条件

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

在另一终端执行：

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote enable
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- admin remote set-password
# 输入密码：Test123 （满足 ≥6 字符+字母+数字）
```

## 测试用例列表

### TC-BF-01: 登录失败返回剩余尝试次数

**操作步骤：**
1. 使用错误密码调用登录接口：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username":"admin","password":"wrongpass"}'
   ```

**预期结果：**
- 返回 HTTP 401
- 响应体 JSON 包含 `remaining_attempts` 字段，值为 4
- 响应体 JSON 包含 `failed_attempts` 字段，值为 1
- 响应体 JSON 包含 `max_attempts` 字段，值为 5

### TC-BF-02: 连续 5 次失败后触发锁定

**操作步骤：**
1. 继续使用错误密码调用登录接口 4 次（总计 5 次）：
   ```bash
   for i in 2 3 4 5; do
     echo "--- Attempt $i ---"
     curl -s -w "\nHTTP Status: %{http_code}\n" \
       -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
       -H "Content-Type: application/json" \
       -d '{"username":"admin","password":"wrongpass"}'
   done
   ```

**预期结果：**
- 第 2-4 次返回 HTTP 401，`remaining_attempts` 依次为 3、2、1
- 第 5 次返回 HTTP 403
- 第 5 次响应体包含 `"locked_out": true`
- 第 5 次响应体包含锁定说明信息

### TC-BF-03: 锁定后远程访问被禁用，密码被清除

**操作步骤：**
1. 查询认证状态：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .
   ```

**预期结果：**
- `remote_access_enabled` 为 `false`
- `has_password` 为 `false`
- `locked_out` 为 `true` 或 `failed_attempts` ≥ 5

### TC-BF-04: 锁定后再次登录被拒绝

**操作步骤：**
1. 尝试用正确密码登录：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username":"admin","password":"Test123"}'
   ```

**预期结果：**
- 返回非 200 状态码（远程访问已停用，密码已清除）

### TC-BF-05: 密码强度校验 — 拒绝过短密码

**操作步骤：**
1. 从本机设置短密码：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/passwd \
     -H "Content-Type: application/json" \
     -d '{"password":"ab1"}'
   ```

**预期结果：**
- 返回 HTTP 400
- 错误信息包含 "at least 6 characters"

### TC-BF-06: 密码强度校验 — 拒绝纯数字密码

**操作步骤：**
1. 从本机设置纯数字密码：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/passwd \
     -H "Content-Type: application/json" \
     -d '{"password":"123456"}'
   ```

**预期结果：**
- 返回 HTTP 400
- 错误信息包含 "letters and digits"

### TC-BF-07: 密码强度校验 — 拒绝纯字母密码

**操作步骤：**
1. 从本机设置纯字母密码：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/passwd \
     -H "Content-Type: application/json" \
     -d '{"password":"abcdef"}'
   ```

**预期结果：**
- 返回 HTTP 400
- 错误信息包含 "letters and digits"

### TC-BF-08: 本机恢复 — 重新设置密码并启用远程访问

**操作步骤：**
1. 从本机设置合规密码：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/passwd \
     -H "Content-Type: application/json" \
     -d '{"password":"NewPass1"}'
   ```
2. 从本机启用远程访问：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/remote-access \
     -H "Content-Type: application/json" \
     -d '{"enabled":true}'
   ```
3. 确认状态已恢复：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .
   ```

**预期结果：**
- 步骤 1 返回 HTTP 200
- 步骤 2 返回 HTTP 200，`remote_access_enabled` 为 `true`
- 步骤 3 显示 `remote_access_enabled: true`，`has_password: true`，`failed_attempts: 0`

### TC-BF-09: 恢复后正确密码可正常登录

**操作步骤：**
1. 使用新密码登录：
   ```bash
   curl -s -w "\nHTTP Status: %{http_code}\n" \
     -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"username":"admin","password":"NewPass1"}'
   ```

**预期结果：**
- 返回 HTTP 200
- 响应体包含 `token` 字段

### TC-BF-10: 成功登录后失败计数重置

**操作步骤：**
1. 先用错误密码登录 3 次
2. 再用正确密码登录 1 次
3. 查询认证状态

```bash
for i in 1 2 3; do
  curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
    -H "Content-Type: application/json" \
    -d '{"username":"admin","password":"wrong"}'
done

curl -s -X POST http://127.0.0.1:8800/_bifrost/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"NewPass1"}'

curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .failed_attempts
```

**预期结果：**
- 前 3 次返回 401
- 第 4 次返回 200
- 查询 `failed_attempts` 为 0

### TC-BF-11: 前端 Login 页面显示锁定提示

**操作步骤：**
1. 重复 TC-BF-01 和 TC-BF-02 使账户锁定
2. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/`
3. 观察 Login 页面

**预期结果：**
- 页面显示红色锁定提示："由于多次登录失败，远程访问已被禁用"
- 登录按钮处于禁用状态

### TC-BF-12: 前端 Settings 页面显示锁定状态

**操作步骤：**
1. 从本机（localhost）访问 `http://127.0.0.1:8800/_bifrost/` 进入 Settings
2. 切换到 Remote Access 标签

**预期结果：**
- 显示 "Brute-Force Lockout Active" 错误提示
- 显示 Failed Attempts 计数

### TC-BF-13: Auth Status API 返回完整锁定字段

**操作步骤：**
```bash
curl -s http://127.0.0.1:8800/_bifrost/api/auth/status | jq .
```

**预期结果：**
- 响应包含字段：`locked_out`, `failed_attempts`, `max_attempts`, `min_password_length`
- `max_attempts` 值为 5
- `min_password_length` 值为 6

## 清理步骤

```bash
rm -rf ./.bifrost-test
```
