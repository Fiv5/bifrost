# TLS 不信任域名交互式 Passthrough

## 功能模块说明

当 TLS 拦截检测到客户端不信任 Bifrost CA 证书时，系统会通过两个入口提供交互式操作：

1. **Toast 弹窗通知**：在管理端右上角弹出带操作按钮的 Toast，用户可直接点击「Passthrough」将域名加入 TLS 拦截排除列表，或点击「Ignore」忽略该通知。
2. **通知列表操作**：在 Notifications 页面的 tls_trust_change 通知行中显示「Passthrough」和「Ignore」操作按钮，操作后显示结果标签。

## 前置条件

```bash
# 启动 Bifrost 代理服务（TLS 拦截启用）
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

- 管理端 Web UI 地址：`http://localhost:8800/_bifrost/`
- 确保 TLS 拦截已启用（Settings → TLS 标签页 → Enable TLS Interception = true）
- 准备一个未信任 Bifrost CA 的客户端（或使用 curl 不带 `--cacert` 访问 HTTPS 站点触发不信任）

## 测试用例

### TC-TPI-01：TLS 不信任时 Toast 弹出域名级通知

**步骤**：
1. 打开管理端 Web UI `http://localhost:8800/_bifrost/`
2. 使用代理访问任意 HTTPS 站点（如 `curl -x http://localhost:8800 https://httpbin.org/get`），不安装 CA 证书使其 TLS 握手失败
3. 等待最多 10 秒观察管理端右上角 Toast

**预期结果**：
- 弹出 warning 级别的 Toast 通知，标题为「TLS Certificate Not Trusted」
- 通知内容包含域名信息（如 `Domain httpbin.org is not trusted by the client.`）
- Toast 中显示两个按钮：「Passthrough」（蓝色主按钮）和「Ignore」（默认按钮）
- Toast 不会自动关闭（duration: 0），需用户操作

### TC-TPI-02：Toast 中点击 Passthrough 将域名加入排除列表

**步骤**：
1. 执行 TC-TPI-01 触发 Toast 弹出
2. 点击 Toast 中的「Passthrough」按钮
3. 检查 Settings → TLS 配置中的 intercept_exclude 列表

**预期结果**：
- Toast 关闭
- 域名（如 `httpbin.org`）被添加到 TLS 拦截排除列表（intercept_exclude）
- 通知状态更新为 `read`，action_taken 为 `passthrough`
- 侧边栏 Notify Badge 数字减少
- 后续对该域名的 HTTPS 请求会直接 passthrough（不拦截）

### TC-TPI-03：Toast 中点击 Ignore 忽略通知

**步骤**：
1. 触发另一个域名的 TLS 不信任通知（如 `curl -x http://localhost:8800 https://example.com/`）
2. 在 Toast 中点击「Ignore」按钮

**预期结果**：
- Toast 关闭
- 通知状态更新为 `dismissed`，action_taken 为 `ignored`
- 域名不会被添加到 intercept_exclude 列表
- 侧边栏 Notify Badge 数字减少

### TC-TPI-04：Notifications 表格中显示 Passthrough / Ignore 操作按钮

**步骤**：
1. 触发多个域名的 TLS 不信任通知
2. 打开 Notifications 页面（侧边栏 Notify）
3. 查看 tls_trust_change 类型通知的 Actions 列

**预期结果**：
- 对于未处理的 tls_trust_change 通知，Actions 列显示「Passthrough」（蓝色）和「Ignore」两个按钮
- Domain 列正确显示对应域名
- Type 列显示 TLS Trust 标签（橙色）

### TC-TPI-05：Notifications 表格中点击 Passthrough 操作

**步骤**：
1. 在 Notifications 表格中找到一条未处理的 tls_trust_change 通知
2. 点击该行 Actions 列的「Passthrough」按钮
3. 检查 Settings → TLS 配置

**预期结果**：
- 列表刷新，该通知的 Actions 列变为绿色标签「Passthrough ✓」
- 域名被添加到 intercept_exclude 列表
- 通知状态变为 `read`

### TC-TPI-06：Notifications 表格中点击 Ignore 操作

**步骤**：
1. 在 Notifications 表格中找到一条未处理的 tls_trust_change 通知
2. 点击该行 Actions 列的「Ignore」按钮

**预期结果**：
- 列表刷新，该通知的 Actions 列变为灰色标签「Ignored」
- 域名不会被添加到 intercept_exclude 列表
- 通知状态变为 `dismissed`

### TC-TPI-07：已处理通知不重复弹出 Toast

**步骤**：
1. 先执行 TC-TPI-02 或 TC-TPI-03 处理某个通知
2. 等待下一次轮询周期（5 秒）

**预期结果**：
- 已处理的通知不会再次弹出 Toast
- 只有新的未处理 TLS 通知才会触发 Toast

### TC-TPI-08：非 TLS 通知仍显示默认 Actions

**步骤**：
1. 在 Notifications 表格中查看非 tls_trust_change 类型的通知（如 pending_authorization）

**预期结果**：
- 非 TLS 类型通知的 Actions 列仍显示默认的 Mark as read（眼睛图标）和 Dismiss（勾号图标）按钮
- TLS 类型通知显示 Passthrough / Ignore 按钮

## 清理步骤

```bash
# 停止测试服务
kill $(lsof -ti:8800)

# 清理测试数据
rm -rf .bifrost-test
```
