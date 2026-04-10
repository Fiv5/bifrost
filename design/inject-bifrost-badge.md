# HTML 页面注入 Bifrost 小圆点（Badge Injection）

## 背景与目标

当用户通过 Bifrost 代理访问网页时（尤其是 HTTPS MITM 后的页面），用户需要一个**低侵入**的视觉提示，确认“该页面已被 Bifrost 接管/代理”。

本方案在**明文可编辑**的 HTML 响应中（`Content-Type` 包含 `text/html`），向页面左下角注入一个固定定位的小圆点（Bifrost badge）。

## 需求范围

- 仅对 **HTTP 响应 body 可缓冲** 的场景生效（非 streaming）。
- 仅对 `Content-Type: text/html`（包含参数如 `charset=utf-8`）生效。
- 支持开关配置：
  - 全局配置项：`traffic.inject_bifrost_badge`，默认 `true`，持久化到 `config.toml`。
  - Web UI：Settings -> Proxy 页提供开关，文案：**“注入 Bifrost 小圆点”**。
  - CLI：启动命令提供 flag（例如 `--disable-badge-injection`）用于覆盖并持久化该配置。

## 实现设计

### 1. 配置存储

- 配置位置：`UnifiedConfig.traffic.inject_bifrost_badge: bool`（默认 `true`）。
- 持久化链路：
  - `bifrost-storage`：扩展 `TrafficConfig` / `TrafficConfigUpdate` / `ConfigManager::update_traffic_config`。
  - `bifrost-proxy`：`ProxyConfig` 增加 `inject_bifrost_badge` 字段，运行时读取并在响应处理链路中生效。

### 2. 响应处理链路（Rust proxy）

注入发生在 HTTP handler 的响应 body 已经被 collect 成 bytes 后。

- 判定条件：
  - `config.inject_bifrost_badge == true`
  - `content-type` contains `text/html`
  - 非 SSE / 非 streaming
- 注入策略：
  - 将 body 解压到明文（支持 `gzip` / `br` / `deflate` / `zstd`），在 `</body>` 前插入 badge 片段；如果找不到 `</body>`，则追加到末尾。
  - 若原响应存在 `Content-Encoding`，在注入完成后按原 encoding 重新压缩（保持 header 语义不变），并通过 `normalize_res_headers` 修正 `Content-Length` / `Transfer-Encoding`。

### 3. Badge 片段

- 以一个 `div` + 内联样式注入，不依赖外部资源。
- 样式目标：左下角、固定定位、小圆点、不抢占点击（`pointer-events: none`），`z-index` 极高。

### 4. Web UI / Admin API

- Admin API：复用 `/_bifrost/api/config`，扩展 response 增加 `inject_bifrost_badge` 字段，并新增 `PUT /_bifrost/api/config` 支持更新该字段（内部调用 `update_traffic_config` 持久化）。
- Web：Settings -> Proxy tab 新增 `Switch`，默认值来自 `GET /config`，切换后调用 `PUT /config`。

## 验证计划（强制三层）

### 单元测试

- `bifrost-proxy`：
  - `test_inject_badge_before_body_end`：HTML 含 `</body>` 时插入位置正确。
  - `test_inject_badge_append_when_no_body_end`：无 `</body>` 时回退到末尾追加。
  - `test_inject_badge_gzip_roundtrip`：gzip body 解压->注入->再压缩后，解压结果包含 badge。

### E2E 测试

- 新增 e2e 用例：`badge_injection_html_response`
  - 启动本地 http server 返回 `Content-Type: text/html` 的页面。
  - 通过 Bifrost 代理请求该页面。
  - 断言响应 body 包含注入标识（例如 `__bifrost_badge__`）。

### 真实场景测试

- 按临时数据目录启动：
  - `BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl`
- 浏览器设置系统代理后访问任意网页，打开 DevTools 查看页面 HTML，确认出现 `__bifrost_badge__` 节点，且页面未被破坏。

## 校验要求

- 必须执行：`cargo test --workspace --all-features`
- 提交前必须通过：`cargo fmt`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`

## 文档更新

- `README.md`：补充配置项与 CLI 参数说明。
