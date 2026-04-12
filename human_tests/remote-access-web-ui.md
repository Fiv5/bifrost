# 远程访问管理 Web UI 测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 准备一台同局域网的设备，或使用本机的局域网 IP（如 `192.168.8.31`）
3. 确保端口 8800 未被防火墙阻止

---

## 测试用例

### TC-RA-01：localhost 访问管理端无需登录

**操作步骤**：
1. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/`

**预期结果**：
- 直接进入管理端 Traffic 页面，无登录提示
- URL 为 `http://127.0.0.1:8800/_bifrost/traffic`

---

### TC-RA-02：Remote Access Tab 显示及初始状态

**操作步骤**：
1. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/settings?tab=remote`

**预期结果**：
- Settings 页面显示 "Remote Access" Tab
- Remote Access Status 卡片显示：
  - Remote Access: `Disabled`（灰色标签）
  - Admin Username: `admin`
  - Password: `Not Set`（橙色标签）
- Enable Remote Admin Access 开关处于关闭状态
- 底部有蓝色提示 "Set a password to enable remote admin access"

---

### TC-RA-03：设置管理端用户名和密码

**操作步骤**：
1. 在 Remote Access Tab 中，Username 输入框填入 `admin`
2. New Password 输入框填入 `test123456`
3. Confirm Password 输入框填入 `test123456`
4. 点击 "Save Credentials" 按钮

**预期结果**：
- 显示 Toast 消息 "Password updated"
- Password 状态从 `Not Set` 变为 `Set`（绿色标签）
- 密码输入框被清空

---

### TC-RA-04：密码不匹配时拒绝保存

**操作步骤**：
1. New Password 填入 `abc123`
2. Confirm Password 填入 `abc456`
3. 点击 "Save Credentials"

**预期结果**：
- 显示警告消息 "Passwords do not match"
- 密码未被修改

---

### TC-RA-05：无密码时禁止开启远程访问

**前置条件**：新启动的服务，未设置密码

**操作步骤**：
1. 直接点击 "Enable Remote Admin Access" 开关

**预期结果**：
- 显示错误消息 "Cannot enable remote access without setting a password first"
- 开关保持关闭状态

---

### TC-RA-06：有密码后开启远程访问

**前置条件**：已通过 TC-RA-03 设置密码

**操作步骤**：
1. 点击 "Enable Remote Admin Access" 开关

**预期结果**：
- 显示 Toast 消息 "Remote access enabled"
- Remote Access 状态变为 `Enabled`（绿色标签）

---

### TC-RA-07：远程 IP 访问根路径自动重定向到登录页

**前置条件**：已开启远程访问

**操作步骤**：
1. 在浏览器中打开 `http://192.168.8.31:8800/`

**预期结果**：
- 自动重定向到 `http://192.168.8.31:8800/_bifrost/login?next=...`
- 显示登录页面，标题为 "Bifrost Admin"
- 显示 "远程管理访问 / 鉴权登录"
- 有用户名和密码输入框

---

### TC-RA-08：远程 IP 使用正确密码登录

**操作步骤**：
1. 在远程登录页面，输入用户名 `admin`，密码 `test123456`
2. 点击 "登录" 按钮

**预期结果**：
- 登录成功，跳转到 `http://192.168.8.31:8800/_bifrost/traffic`
- 显示完整的管理端 Traffic 页面
- 可以正常浏览和操作管理端所有功能（Traffic、Rules、Settings 等）

---

### TC-RA-09：远程 IP 使用错误密码登录

**操作步骤**：
1. 在远程登录页面，输入用户名 `admin`，密码 `wrongpassword`
2. 点击 "登录" 按钮

**预期结果**：
- 登录失败
- 显示错误提示（如 "Invalid username or password"）
- 停留在登录页面

---

### TC-RA-10：开启远程访问后 localhost 仍无需登录

**前置条件**：远程访问已开启

**操作步骤**：
1. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/`

**预期结果**：
- 直接进入管理端 Traffic 页面，**无需登录**
- localhost/127.0.0.1 始终免鉴权

---

### TC-RA-11：吊销所有会话

**前置条件**：远程 IP 已通过 TC-RA-08 登录

**操作步骤**：
1. 通过 localhost 打开 `http://127.0.0.1:8800/_bifrost/settings?tab=remote`
2. 在 "Session Management" 区域点击 "Revoke All Sessions" 按钮
3. 在确认弹窗中点击 "Revoke"

**预期结果**：
- 显示 Toast 消息 "All sessions revoked"

---

### TC-RA-12：吊销后远程 token 失效

**前置条件**：已通过 TC-RA-11 吊销会话

**操作步骤**：
1. 在远程 IP 浏览器中刷新管理端页面（或访问 `http://192.168.8.31:8800/_bifrost/`）

**预期结果**：
- 被重定向回登录页面
- 需要重新输入用户名密码登录
- 之前的 JWT token 已失效

---

### TC-RA-13：关闭远程访问

**操作步骤**：
1. 通过 localhost 打开 Settings → Remote Access Tab
2. 点击 "Enable Remote Admin Access" 开关关闭

**预期结果**：
- 显示 Toast 消息 "Remote access disabled"
- Remote Access 状态变为 `Disabled`
- 远程 IP 无法再访问管理端（连接被拒绝或无响应）

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
