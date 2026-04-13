# 证书管理 Admin API 测试用例

## 功能模块说明

验证 Bifrost 证书管理相关的 Admin API 接口，包括证书信息查询、CA 证书下载、证书二维码生成等功能。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被占用
3. 服务启动成功后再执行测试用例

---

## 测试用例

### TC-ACE-01：获取证书信息

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/cert/info | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `available`：布尔值，表示 CA 证书文件是否存在
  - `status`：字符串，取值为 `not_installed`、`installed_not_trusted` 或 `installed_and_trusted`
  - `status_label`：人类可读的状态标签（如 `Not installed`）
  - `installed`：布尔值，表示证书是否已安装
  - `trusted`：布尔值，表示证书是否已被信任
  - `status_message`：状态描述信息
  - `local_ips`：字符串数组，包含本机局域网 IP 地址
  - `download_urls`：字符串数组，每个元素格式为 `http://<ip>:8800/_bifrost/public/cert`
  - `qrcode_urls`：字符串数组，每个元素格式为 `http://<ip>:8800/_bifrost/public/cert/qrcode`

---

### TC-ACE-02：通过 /api/cert 路径获取证书信息（别名路由）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/cert | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应与 TC-ACE-01 结果一致
- `download_urls` 和 `qrcode_urls` 数组长度与 `local_ips` 一致

---

### TC-ACE-03：下载 CA 证书 PEM 文件

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -sI http://127.0.0.1:8800/_bifrost/public/cert
   ```
2. 执行以下命令下载证书：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/public/cert -o /tmp/bifrost-ca-test.crt
   ```

**预期结果**：
- 返回 HTTP 200
- 响应头 `Content-Type` 为 `application/x-pem-file`
- 响应头 `Content-Disposition` 包含 `attachment; filename="bifrost-ca.crt"`
- 响应头包含 `Access-Control-Allow-Origin`（CORS 支持）
- 下载的文件为有效的 PEM 格式证书，以 `-----BEGIN CERTIFICATE-----` 开头

---

### TC-ACE-04：获取证书下载二维码（默认 Host）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -sI http://127.0.0.1:8800/_bifrost/public/cert/qrcode
   ```
2. 执行以下命令查看 SVG 内容：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/public/cert/qrcode | head -5
   ```

**预期结果**：
- 返回 HTTP 200
- 响应头 `Content-Type` 为 `image/svg+xml`
- 响应头包含 `Access-Control-Allow-Origin`（CORS 支持）
- 响应体为有效的 SVG XML 内容，包含 `<svg` 标签
- 二维码编码的 URL 为 `http://127.0.0.1:8800/_bifrost/public/cert`（基于请求的 Host 头）

---

### TC-ACE-05：获取证书下载二维码（指定 IP）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/public/cert/qrcode?ip=127.0.0.1" | head -5
   ```

**预期结果**：
- 返回 HTTP 200
- 响应头 `Content-Type` 为 `image/svg+xml`
- 响应体为有效的 SVG 二维码内容
- 二维码编码的 URL 为 `http://127.0.0.1:8800/_bifrost/public/cert`（使用查询参数中指定的 IP）

---

### TC-ACE-06：对证书 API 使用不支持的 HTTP 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/cert/info -w "\n%{http_code}"
   ```
2. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/public/cert -w "\n%{http_code}"
   ```

**预期结果**：
- 两个请求均返回 HTTP 405（Method Not Allowed）

---

### TC-ACE-07：访问不存在的证书 API 路径

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/cert/nonexistent -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 404（Not Found）

---

### TC-ACE-08：CORS 预检请求（OPTIONS）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X OPTIONS http://127.0.0.1:8800/_bifrost/public/cert -I
   ```
2. 执行以下命令：
   ```bash
   curl -s -X OPTIONS http://127.0.0.1:8800/_bifrost/public/cert/qrcode -I
   ```

**预期结果**：
- 两个请求均返回 HTTP 200
- 响应头包含 CORS 相关头部（如 `Access-Control-Allow-Origin`、`Access-Control-Allow-Methods`）

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
rm -f /tmp/bifrost-ca-test.crt
```
