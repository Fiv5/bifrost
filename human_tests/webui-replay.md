# Web UI Replay 页面测试用例

## 功能模块说明

Replay 页面是 Bifrost 管理端的 HTTP 请求重放工具（类似 Postman），支持创建和发送 HTTP/SSE/WebSocket 请求、管理请求集合、查看请求历史、从流量记录导入请求、导入 curl 命令等功能。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/replay`
3. 确保端口 8800 未被防火墙阻止

---

## 测试用例

### TC-WRP-01：访问 Replay 页面

**操作步骤**：
1. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/replay`

**预期结果**：
- 页面正常加载，显示 Replay 页面
- 左侧显示请求集合/历史面板
- 右侧显示请求编辑区域，包含 URL 输入框、Method 选择器、发送按钮
- URL 为 `http://127.0.0.1:8800/_bifrost/replay`

---

### TC-WRP-02：创建新的 HTTP 请求

**操作步骤**：
1. 在 Replay 页面中，点击新建请求按钮（"+" 或 "New Request"）

**预期结果**：
- 新建一个空白请求 Tab
- Method 默认为 `GET`
- URL 输入框为空或显示占位提示
- 请求体、Headers 等区域为空

---

### TC-WRP-03：设置请求 URL 和 Method

**操作步骤**：
1. 在 URL 输入框中输入 `https://httpbin.org/get`
2. 点击 Method 选择器，将方法从 `GET` 切换为 `POST`

**预期结果**：
- URL 输入框显示 `https://httpbin.org/get`
- Method 选择器显示 `POST`
- 可选的 Method 包括：GET、POST、PUT、DELETE、PATCH、HEAD、OPTIONS 等

---

### TC-WRP-04：设置请求 Headers

**操作步骤**：
1. 切换到 Headers 标签页
2. 添加一个新的 Header：Key 为 `X-Custom-Header`，Value 为 `test-value`
3. 再添加一个 Header：Key 为 `Accept`，Value 为 `application/json`

**预期结果**：
- Headers 列表中显示已添加的两个 Header
- 每个 Header 有启用/禁用的复选框
- 可以编辑或删除已添加的 Header

---

### TC-WRP-05：设置 JSON 请求体

**操作步骤**：
1. 将 Method 设为 `POST`
2. 切换到 Body 标签页
3. 选择 Body 类型为 `JSON`
4. 在代码编辑器中输入：
   ```json
   {"name": "test", "value": 123}
   ```

**预期结果**：
- Body 编辑区域显示代码编辑器
- 代码编辑器支持 JSON 语法高亮
- 输入的 JSON 内容正确显示
- Content-Type 自动设置为 `application/json`

---

### TC-WRP-06：发送请求并查看响应

**操作步骤**：
1. URL 输入框填入 `https://httpbin.org/get`
2. Method 设为 `GET`
3. 点击 "Send" 按钮发送请求

**预期结果**：
- 发送按钮显示加载状态
- 请求完成后，下方响应区域显示响应内容
- 显示响应状态码（如 `200 OK`）
- 显示响应耗时
- 显示响应体大小

---

### TC-WRP-07：查看响应 Body

**操作步骤**：
1. 完成 TC-WRP-06 的请求发送
2. 在响应区域切换到 Body 标签页

**预期结果**：
- 响应 Body 以格式化的方式展示（JSON 格式化显示）
- 响应内容包含 httpbin.org 返回的请求信息
- 代码编辑器支持语法高亮
- 可以复制响应内容

---

### TC-WRP-08：查看响应 Headers

**操作步骤**：
1. 完成 TC-WRP-06 的请求发送
2. 在响应区域切换到 Headers 标签页

**预期结果**：
- 显示响应头列表，包含 Key-Value 对
- 包含常见响应头如 `Content-Type`、`Date`、`Server` 等
- 响应头以表格或键值对形式展示

---

### TC-WRP-09：响应状态码高亮显示

**操作步骤**：
1. 分别发送以下请求并观察状态码显示：
   - `https://httpbin.org/status/200`（200 成功）
   - `https://httpbin.org/status/404`（404 未找到）
   - `https://httpbin.org/status/500`（500 服务器错误）

**预期结果**：
- 200 状态码以绿色高亮显示
- 404 状态码以橙色/黄色高亮显示
- 500 状态码以红色高亮显示
- 状态码旁显示对应的状态文本

---

### TC-WRP-10：创建请求集合

**操作步骤**：
1. 在左侧面板中，点击创建集合按钮（"New Collection" 或 "+"）
2. 输入集合名称 `测试集合`
3. 确认创建

**预期结果**：
- 左侧面板中出现名为 `测试集合` 的集合项
- 集合项可以展开/折叠
- 集合初始为空

---

### TC-WRP-11：在集合中创建文件夹

**操作步骤**：
1. 右键点击 `测试集合`（或点击集合旁的菜单按钮）
2. 选择 "New Folder" 或 "新建文件夹"
3. 输入文件夹名称 `用户接口`
4. 确认创建

**预期结果**：
- `测试集合` 下出现名为 `用户接口` 的子文件夹
- 文件夹可以展开/折叠
- 文件夹初始为空

---

### TC-WRP-12：保存请求到集合

**操作步骤**：
1. 创建一个新请求，URL 设为 `https://httpbin.org/post`，Method 设为 `POST`
2. 添加 Header：`Content-Type: application/json`
3. 设置 Body 为 `{"key": "value"}`
4. 点击保存按钮（"Save" 或 Ctrl+S）
5. 选择保存到 `测试集合` > `用户接口` 文件夹
6. 输入请求名称 `创建用户`
7. 确认保存

**预期结果**：
- 请求被保存到 `测试集合` > `用户接口` 文件夹下
- 左侧面板中显示 `创建用户` 请求项
- 请求项显示 Method 标签（如绿色的 `POST`）
- 点击该请求项可以重新加载请求内容

---

### TC-WRP-13：查看请求历史

**操作步骤**：
1. 先发送几个不同的请求（如 GET、POST 各一个）
2. 切换到左侧面板的 "History" 或 "历史" 标签

**预期结果**：
- 历史列表中按时间倒序显示已发送的请求记录
- 每条记录显示 Method、URL、时间戳
- 点击历史记录可以重新加载该请求的详细信息
- 可以从历史记录中重新发送请求

---

### TC-WRP-14：从 Traffic 页面保存请求到集合

**前置条件**：Traffic 页面中已有至少一条 HTTP 请求记录

**操作步骤**：
1. 打开 `http://127.0.0.1:8800/_bifrost/traffic`
2. 在流量列表中找到一条 HTTP 请求
3. 右键点击该请求，在上下文菜单中选择 "Replay" 或 "Save to Collection" 或类似选项

**预期结果**：
- 跳转到 Replay 页面
- 请求的 URL、Method、Headers、Body 被自动填充
- 可以直接发送或修改后发送
- 可以将该请求保存到已有的集合中

---

### TC-WRP-15：SSE 请求重放

**操作步骤**：
1. 创建一个新请求
2. URL 输入一个 SSE 端点（如 `https://httpbin.org/sse` 或其他可用的 SSE 服务地址）
3. 选择请求类型为 SSE（如有专门的类型选择器）或直接发送 GET 请求
4. 点击 "Send" 发送请求

**预期结果**：
- 请求建立 SSE 连接
- 响应区域实时显示接收到的 SSE 事件
- 每个事件显示 event type、data 等字段
- 有断开连接的按钮可以主动关闭 SSE 连接
- 连接状态有相应的指示（连接中/已连接/已断开）

---

### TC-WRP-16：WebSocket 请求重放

**操作步骤**：
1. 创建一个新请求
2. 选择请求类型为 WebSocket
3. URL 输入一个 WebSocket 端点（如 `wss://echo.websocket.org` 或其他可用的 WebSocket 服务地址）
4. 点击 "Connect" 建立连接
5. 在消息输入框中输入 `Hello WebSocket`
6. 点击发送消息

**预期结果**：
- WebSocket 连接成功建立，显示连接状态
- 消息面板显示发送的消息 `Hello WebSocket`
- 如果是 echo 服务，显示接收到的回复消息
- 发送和接收的消息有不同的样式区分
- 有断开连接的按钮
- 连接状态有相应的指示（连接中/已连接/已断开）

---

### TC-WRP-17：导入 curl 命令

**操作步骤**：
1. 在 Replay 页面中找到导入功能（"Import" 或 "Import curl"）
2. 粘贴以下 curl 命令：
   ```
   curl -X POST https://httpbin.org/post -H "Content-Type: application/json" -H "Authorization: Bearer token123" -d '{"username": "admin", "password": "secret"}'
   ```
3. 确认导入

**预期结果**：
- 请求的各字段被正确解析和填充：
  - Method: `POST`
  - URL: `https://httpbin.org/post`
  - Headers 中包含 `Content-Type: application/json` 和 `Authorization: Bearer token123`
  - Body 内容为 `{"username": "admin", "password": "secret"}`
- 可以直接发送该请求

---

### TC-WRP-18：Form-Data 请求体

**操作步骤**：
1. 创建一个新请求，Method 设为 `POST`
2. URL 填入 `https://httpbin.org/post`
3. 切换到 Body 标签页
4. 选择 Body 类型为 `Form Data`（multipart/form-data）
5. 添加字段：
   - Key: `username`，Value: `testuser`
   - Key: `email`，Value: `test@example.com`
6. 点击 "Send" 发送请求

**预期结果**：
- 请求以 multipart/form-data 格式发送
- 响应中显示 httpbin.org 返回的 form 数据，包含 `username` 和 `email` 字段
- Content-Type 自动设置为 `multipart/form-data`

---

### TC-WRP-19：URL-Encoded Form 请求体

**操作步骤**：
1. 创建一个新请求，Method 设为 `POST`
2. URL 填入 `https://httpbin.org/post`
3. 切换到 Body 标签页
4. 选择 Body 类型为 `x-www-form-urlencoded`
5. 添加字段：
   - Key: `grant_type`，Value: `password`
   - Key: `username`，Value: `admin`
6. 点击 "Send" 发送请求

**预期结果**：
- 请求以 application/x-www-form-urlencoded 格式发送
- 响应中显示 httpbin.org 返回的 form 数据
- Content-Type 自动设置为 `application/x-www-form-urlencoded`

---

### TC-WRP-20：纯文本请求体

**操作步骤**：
1. 创建一个新请求，Method 设为 `POST`
2. URL 填入 `https://httpbin.org/post`
3. 切换到 Body 标签页
4. 选择 Body 类型为 `Text`（纯文本）
5. 在编辑器中输入 `Hello, this is plain text body`
6. 点击 "Send" 发送请求

**预期结果**：
- 请求以纯文本格式发送
- 响应中显示 httpbin.org 返回的 data 字段包含发送的文本内容
- Content-Type 设置为 `text/plain`

---

### TC-WRP-21：请求体代码编辑器功能

**操作步骤**：
1. 创建一个新请求
2. 切换到 Body 标签页，选择 `JSON` 类型
3. 在代码编辑器中输入以下 JSON：
   ```json
   {
     "name": "test",
     "items": [1, 2, 3],
     "nested": {
       "key": "value"
     }
   }
   ```
4. 观察编辑器功能

**预期结果**：
- 代码编辑器支持 JSON 语法高亮（关键字、字符串、数字不同颜色）
- 支持行号显示
- 支持自动缩进
- 支持括号匹配
- 编辑器内容可以正常编辑和修改

---

### TC-WRP-22：响应体代码高亮

**操作步骤**：
1. 发送 GET 请求到 `https://httpbin.org/get`
2. 查看响应 Body 区域

**预期结果**：
- 响应 Body 以 JSON 格式化显示
- JSON 内容有语法高亮（关键字、字符串、数字不同颜色）
- 响应体内容可以复制
- 支持搜索响应内容

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
