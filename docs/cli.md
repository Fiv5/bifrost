# CLI 详细命令

本文档集中说明 `bifrost` CLI 的常用参数与命令。

## 全局参数

```txt
bifrost [OPTIONS] [COMMAND]
```

| 参数 | 说明 | 默认值 |
| --- | --- | --- |
| `-p, --port <PORT>` | HTTP 代理端口 | `9900` |
| `-H, --host <HOST>` | 监听地址 | `0.0.0.0` |
| `--socks5-port <PORT>` | SOCKS5 端口 | 无 |
| `-l, --log-level <LEVEL>` | 日志级别 | `info` |
| `-h, --help` | 显示帮助 | - |
| `-V, --version` | 显示版本号 | - |

## `start` 命令

常见示例：

```bash
bifrost start
bifrost start --daemon
bifrost -p 9000 start
bifrost -p 9900 --socks5-port 1080 start
bifrost start --skip-cert-check
bifrost start --no-intercept
bifrost start --intercept
bifrost start --intercept-exclude "*.example.com,internal.corp.com"
bifrost start --rules "example.com host://127.0.0.1:3000"
bifrost start --rules-file ./my-rules.txt
bifrost start --access-mode whitelist --whitelist "192.168.1.100,10.0.0.0/8"
bifrost start --allow-lan
bifrost start --system-proxy
bifrost start --unsafe-ssl
bifrost start --disable-badge-injection
bifrost start --enable-badge-injection
```

当检测到已有 Bifrost 进程在运行时，`bifrost start` 会在终端提示是否重启：输入 `y/yes` 将停止旧进程并重新启动；输入 `n/no` 将取消本次启动。

如果需要在脚本/CI 中跳过交互，可以使用 `-y/--yes` 自动确认重启。

参数摘要：

| 参数 | 说明 |
| --- | --- |
| `-d, --daemon` | 守护进程模式 |
| `--skip-cert-check` | 跳过 CA 证书安装检查 |
| `--access-mode <MODE>` | `local_only` / `whitelist` / `interactive` / `allow_all` |
| `--whitelist <IPS>` | 客户端 IP 白名单，支持 CIDR |
| `--allow-lan` | 允许局域网访问 |
| `--intercept` | 启用 TLS 拦截 |
| `--no-intercept` | 禁用 TLS 拦截 |
| `--intercept-exclude <DOMAINS>` | TLS 拦截排除域名 |
| `--unsafe-ssl` | 跳过上游证书校验，仅建议测试环境使用 |
| `--enable-badge-injection` | 强制启用 HTML 页面注入 Bifrost 小圆点（会持久化到配置） |
| `--disable-badge-injection` | 禁用 HTML 页面注入 Bifrost 小圆点（会持久化到配置） |
| `--rules <RULE>` | 直接传入规则，可多次指定 |
| `--rules-file <PATH>` | 从文件加载规则 |
| `--system-proxy` | 启动后自动设置系统代理 |
| `--proxy-bypass <LIST>` | 系统代理绕过列表 |
| `--cli-proxy` | 运行期间写入命令行代理环境变量 |
| `--cli-proxy-no-proxy <LIST>` | 命令行代理 no-proxy 列表 |

## 常用命令

### 服务管理

```bash
bifrost status
bifrost stop
```

### 流量查看与搜索

```bash
bifrost traffic list
bifrost traffic list --method GET --status_min 400 --limit 100
bifrost traffic get <id> --request-body --response-body
bifrost traffic search "keyword"
bifrost search "keyword"
bifrost search "keyword" --method POST --host api.openai.com --path /v1/responses
bifrost search "keyword" --req-header
bifrost search "keyword" --res-body
```

`bifrost search` 与 `bifrost traffic search` 等价，支持关键词搜索、基础过滤器与搜索范围控制。

基础过滤器：

| 参数 | 说明 |
| --- | --- |
| `--method <METHOD>` | 按 HTTP 方法过滤，如 `GET`、`POST` |
| `--host <TEXT>` | 按 Host 包含匹配过滤 |
| `--path <TEXT>` | 按 Path 包含匹配过滤 |
| `--status <FILTER>` | 按状态段过滤，如 `2xx`、`4xx`、`5xx`、`error` |
| `--protocol <PROTO>` | 按协议过滤，如 `HTTP`、`HTTPS`、`WS`、`WSS` |
| `--domain <PATTERN>` | 按域名模式过滤 |
| `--content-type <TYPE>` | 按内容类型过滤，如 `json`、`html`、`form` |

搜索范围：

| 参数 | 说明 |
| --- | --- |
| `--url` | 仅搜索 URL / Path |
| `--req-header` | 仅搜索请求头 |
| `--res-header` | 仅搜索响应头 |
| `--req-body` | 仅搜索请求体 |
| `--res-body` | 仅搜索响应体 |
| `--headers` | 同时搜索请求头与响应头 |
| `--body` | 同时搜索请求体与响应体 |

常见组合示例：

```bash
# 在 OpenAI 请求里搜索 Authorization 请求头
bifrost search "Bearer " --method POST --host api.openai.com --req-header

# 搜索某个接口的请求体
bifrost search "user_123" --host api.example.com --path /v1/users --req-body

# 搜索响应头中的缓存标记
bifrost search "cache-control" --res-header

# 搜索响应体中的错误信息
bifrost search "invalid_request_error" --res-body
```

### CA 证书管理

```bash
bifrost ca generate
bifrost ca generate --force
bifrost ca export
bifrost ca export -o ca.crt
bifrost ca info
```

### 规则管理

```bash
bifrost rule list
bifrost rule add <name> --content "rule"
bifrost rule add <name> --file rules.txt
bifrost rule update <name> --content "new rule"
bifrost rule update <name> --file rules.txt
bifrost rule enable <name>
bifrost rule disable <name>
bifrost rule delete <name>
bifrost rule show <name>
bifrost rule get <name>
```

### Group 管理

```bash
# 列出/搜索 groups
bifrost group list
bifrost group list --keyword "team" --limit 20

# 查看 group 详情
bifrost group show <group_id>

# 列出 group 下所有规则
bifrost group rule list <group_id>

# 查看 group 规则详情
bifrost group rule show <group_id> <rule_name>

# 添加 group 规则
bifrost group rule add <group_id> <name> --content "example.com host://127.0.0.1:3000"
bifrost group rule add <group_id> <name> --file rules.txt

# 更新 group 规则
bifrost group rule update <group_id> <name> --content "new rule"
bifrost group rule update <group_id> <name> --file rules.txt

# 启用/禁用 group 规则
bifrost group rule enable <group_id> <name>
bifrost group rule disable <group_id> <name>

# 删除 group 规则
bifrost group rule delete <group_id> <name>
```

- `group` 命令需要代理服务运行中（通过 admin API 通信）
- `group list` 支持 `--keyword` 模糊搜索和 `--limit` 限制结果数
- `group rule add/update` 通过 `--content` 或 `--file` 提供规则内容

### 白名单管理

```bash
bifrost whitelist list
bifrost whitelist add 192.168.1.100
bifrost whitelist add 10.0.0.0/8
bifrost whitelist remove 192.168.1.100
bifrost whitelist allow-lan true
bifrost whitelist allow-lan false
bifrost whitelist status
```

### Values 管理

```bash
bifrost value list
bifrost value show <name>
bifrost value get <name>
bifrost value add <name> <value>
bifrost value set <name> <value>
bifrost value update <name> <value>
bifrost value delete <name>
bifrost value import <file>
```

### Scripts 管理

```bash
bifrost script list
bifrost script list -t request
bifrost script add request demo --content 'log.info("hello")'
bifrost script update request demo --content 'log.info("updated")'
bifrost script show request demo
bifrost script show demo
bifrost script get demo
bifrost script run demo
bifrost script delete request demo
```

### 系统代理管理

```bash
bifrost system-proxy status
bifrost system-proxy enable
bifrost system-proxy enable --host 127.0.0.1 --port 9900
bifrost system-proxy enable --bypass "localhost,127.0.0.1,*.local"
bifrost system-proxy disable
```

### 配置项管理

```bash
bifrost config show traffic
bifrost config set traffic.max-records 10000
bifrost config set traffic.max-db-size 2GB
bifrost config set traffic.max-body-size 1MB
bifrost config set traffic.max-buffer-size 20MB
bifrost config set traffic.retention-days 3
bifrost config set traffic.sse-stream-flush-bytes 64KB
bifrost config set traffic.sse-stream-flush-interval-ms 200
bifrost config set traffic.ws-payload-flush-bytes 256KB
bifrost config set traffic.ws-payload-flush-interval-ms 200
bifrost config set traffic.ws-payload-max-open-files 128
```
