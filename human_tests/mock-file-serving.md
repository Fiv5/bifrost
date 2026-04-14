# Mock File Serving 测试用例

## 功能模块说明

测试 `file://`、`tpl://`、`rawfile://` 协议规则在返回本地文件内容时的行为，特别是对二进制文件（图片、PDF 等）和文本文件（JSON、HTML 等）的正确处理，包括：

- 自动检测 Content-Type（基于文件扩展名）
- 二进制文件原始字节返回（不要求 UTF-8）
- 文本文件支持模板变量替换
- 文件不存在时的错误处理

### 回归背景

**Bug**：配置 `file://` 规则指向二进制文件（如 PNG 图片）时，报错 `Failed to read file: stream did not contain valid UTF-8`。
**根因**：`serve_mock_file` 使用 `tokio::fs::read_to_string`（要求 UTF-8），对二进制文件直接失败。
**修复**：改用 `tokio::fs::read`（二进制读取），并基于 `mime_guess` 自动检测 Content-Type。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 准备测试文件：
   ```bash
   # 创建一个 PNG 测试文件（最小有效 PNG）
   printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82' > /tmp/bifrost-test.png

   # 创建 JSON 测试文件
   echo '{"status":"ok","message":"hello"}' > /tmp/bifrost-test.json

   # 创建 HTML 测试文件
   echo '<!DOCTYPE html><html><body><h1>Test Page</h1></body></html>' > /tmp/bifrost-test.html

   # 创建模板测试文件
   echo '{"host":"${host}","method":"${method}","url":"${url}"}' > /tmp/bifrost-test-tpl.json
   ```

## 测试用例

### TC-MFS-01：二进制 PNG 文件通过 file:// 协议正确返回（回归用例）

**操作步骤**：
1. 创建规则：
   ```bash
   curl -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "test-png-file", "content": "a.com/test.png file:///tmp/bifrost-test.png", "enabled": true}'
   ```
2. 请求文件：
   ```bash
   curl -x http://127.0.0.1:8800 -sD - http://a.com/test.png -o /tmp/bifrost-resp.png
   ```
3. 清理：
   ```bash
   curl -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/test-png-file
   ```

**预期结果**：
- 响应状态码为 200
- 响应头 `Content-Type` 为 `image/png`
- 返回的文件与原始 PNG 文件字节一致（`diff /tmp/bifrost-test.png /tmp/bifrost-resp.png` 无差异）
- **不会**出现 `Failed to read file: stream did not contain valid UTF-8` 错误

---

### TC-MFS-02：JSON 文件通过 file:// 协议正确返回

**操作步骤**：
1. 创建规则：
   ```bash
   curl -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "test-json-file", "content": "a.com/test.json file:///tmp/bifrost-test.json", "enabled": true}'
   ```
2. 请求文件：
   ```bash
   curl -x http://127.0.0.1:8800 -s http://a.com/test.json
   ```
3. 清理：
   ```bash
   curl -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/test-json-file
   ```

**预期结果**：
- 响应状态码为 200
- 响应头 `Content-Type` 包含 `application/json`
- 响应体为 `{"status":"ok","message":"hello"}`

---

### TC-MFS-03：HTML 文件通过 file:// 协议正确返回

**操作步骤**：
1. 创建规则：
   ```bash
   curl -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "test-html-file", "content": "a.com/test.html file:///tmp/bifrost-test.html", "enabled": true}'
   ```
2. 请求文件：
   ```bash
   curl -x http://127.0.0.1:8800 -sD - http://a.com/test.html
   ```
3. 清理：
   ```bash
   curl -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/test-html-file
   ```

**预期结果**：
- 响应状态码为 200
- 响应头 `Content-Type` 包含 `text/html`
- 响应体包含 `<h1>Test Page</h1>`

---

### TC-MFS-04：模板文件通过 tpl:// 协议正确替换变量

**操作步骤**：
1. 创建规则：
   ```bash
   curl -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "test-tpl-file", "content": "a.com/test-tpl tpl:///tmp/bifrost-test-tpl.json", "enabled": true}'
   ```
2. 请求文件：
   ```bash
   curl -x http://127.0.0.1:8800 -s http://a.com/test-tpl
   ```
3. 清理：
   ```bash
   curl -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/test-tpl-file
   ```

**预期结果**：
- 响应状态码为 200
- 响应体中 `${host}` 被替换为 `a.com`
- 响应体中 `${method}` 被替换为 `GET`
- 响应体中 `${url}` 被替换为请求 URL

---

### TC-MFS-05：不存在的文件返回错误响应

**操作步骤**：
1. 创建规则：
   ```bash
   curl -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "test-missing-file", "content": "a.com/missing file:///tmp/nonexistent-file-12345.txt", "enabled": true}'
   ```
2. 请求文件：
   ```bash
   curl -x http://127.0.0.1:8800 -sD - http://a.com/missing
   ```
3. 清理：
   ```bash
   curl -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/test-missing-file
   ```

**预期结果**：
- 响应状态码为 404 或 500
- 不会导致服务 panic 或崩溃

---

### TC-MFS-06：HTTPS 下二进制 PNG 文件正确返回（TLS 拦截路径回归用例）

**操作步骤**：
1. 创建规则：
   ```bash
   curl -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "test-https-png", "content": "a.com/secure.png file:///tmp/bifrost-test.png", "enabled": true}'
   ```
2. 通过 HTTPS 请求（TLS 拦截）：
   ```bash
   curl -x http://127.0.0.1:8800 -sk -D - https://a.com/secure.png -o /tmp/bifrost-resp-https.png
   ```
3. 清理：
   ```bash
   curl -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/test-https-png
   ```

**预期结果**：
- 响应状态码为 200
- 响应头 `Content-Type` 为 `image/png`
- 返回的文件与原始 PNG 文件字节一致
- **不会**出现 UTF-8 相关错误

## 清理步骤

```bash
rm -f /tmp/bifrost-test.png /tmp/bifrost-test.json /tmp/bifrost-test.html /tmp/bifrost-test-tpl.json
rm -f /tmp/bifrost-resp.png /tmp/bifrost-resp-https.png
```
