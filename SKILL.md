---
name: "bifrost"
description: "使用 bifrost 命令行工具管理代理生命周期、规则、Group 规则、证书、脚本、系统代理、运行时配置与流量查询；当需要通过命令行启动/停止代理、配置 TLS 拦截、调试规则、管理 Group 规则、管理脚本、查看 traffic/search、修改 values/config 时使用。"
---

# Bifrost

该技能用于指导 Agent 直接使用 `bifrost` CLI 完成代理启动、配置修改、规则调试、Group 规则管理、脚本管理和流量排查，而不是绕过 CLI 直接改底层数据文件。

## 何时调用

- 需要启动、停止、检查 Bifrost 代理
- 需要通过命令行添加或调试规则
- 需要管理 Group（查询 group、管理 group 规则）
- 需要启用或排查 TLS 拦截
- 需要管理 CA、系统代理、访问白名单、变量值
- 需要管理脚本（request/response/decode）
- 需要查询运行中的代理配置、流量记录或搜索请求

## 启动时必须执行的自检流程

每次技能被触发后，Agent 在正式执行任何 `bifrost` 命令前，都必须先完成下面的启动检查：

1. 检查 `bifrost` 是否存在
2. 如果不存在，自动安装最新版本
3. 如果存在，检查是否可执行
4. 如果可执行，优先升级到最新版本，再继续后续任务
5. 安装或升级完成后，再次验证 `bifrost --version`

除非用户明确禁止联网或禁止改动本机环境，否则不要跳过这个流程。

## 1. 检查 bifrost 是否存在

优先检查当前环境是否已经安装并可执行：

```bash
command -v bifrost
bifrost --version
```

- `command -v bifrost` 有输出路径，说明二进制已在 `PATH` 中
- `bifrost --version` 成功返回，说明 CLI 可以直接使用
- 如果仓库源码在本地但 `bifrost` 尚未加入 `PATH`，可退回源码方式检查：

```bash
cargo run -p bifrost -- --version
```

## 2. 如果 bifrost 不存在，自动安装最新版本

优先使用官方安装脚本直接安装最新版本：

```bash
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash
```

安装完成后，必须重新执行：

```bash
command -v bifrost
bifrost --version
```

如果安装脚本失败，再按环境降级处理：

```bash
# macOS / Homebrew
brew tap bifrost-proxy/bifrost
brew install bifrost
```

如果用户不希望安装系统级二进制，才退回源码构建；注意这里应从官方仓库拉取源码，而不是假设当前工作区就是 Bifrost 仓库：

```bash
git clone https://github.com/bifrost-proxy/bifrost.git /tmp/bifrost
cd /tmp/bifrost
cd web && pnpm install && pnpm build && cd ..
cargo build --release -p bifrost
./target/release/bifrost --version
```

## 3. 如果 bifrost 已存在，自动升级到最新版本

只要用户没有明确要求"固定当前版本"或"禁止升级"，都应在首次检查通过后继续执行：

```bash
bifrost upgrade -y
bifrost --version
```

- `bifrost upgrade -y` 会跳过确认提示
- 若升级失败，再回退到官方安装脚本重新安装最新版本
- 如果用户只允许最小改动，至少要告知"当前跳过升级，后续行为基于现有版本"

## 4. 完成自检后再进入正式任务

1. 先确认目标是"运行代理"还是"管理已有代理"。
2. 始终使用默认数据目录（`~/.bifrost`），禁止使用临时 `BIFROST_DATA_DIR`，以确保证书、规则、配置等数据正确可用。
3. 先用 `bifrost --help` 或 `bifrost <command> --help` 补充具体参数，再执行高影响命令。
4. 会改本机网络环境的命令必须谨慎：`system-proxy enable/disable`、`start --system-proxy`、`start --cli-proxy`。

## 关键约束

- `bifrost` 不带子命令时，等价于 `bifrost start`
- `config`、`traffic`、`search`、`group`、多数 `status` 相关能力依赖"已有运行中的代理"
- `rule`、`value`、`script`、`ca` 主要操作本地数据目录，不一定要求代理正在运行
- `system-proxy` 会修改操作系统代理设置；除非用户明确要求，不要主动启用
- `--unsafe-ssl` 只适合测试排查，不要默认开启
- TLS 拦截默认关闭；要抓 HTTPS 内容，通常需要 `--intercept` 且准备好 CA

## 全局参数

所有子命令继承的全局参数：

```
-p, --port <PORT>                 HTTP 代理端口（默认 9900）
-H, --host <HOST>                 监听地址（默认 0.0.0.0）
    --socks5-port <PORT>          独立 SOCKS5 端口（默认共享主端口）
-l, --log-level <LEVEL>           日志级别 [trace|debug|info|warn|error]（默认 info）
    --log-output <TARGETS>        日志输出目标：console, file 或组合（默认 console,file）
    --log-dir <PATH>              日志文件目录（默认 <data_dir>/logs）
    --log-retention-days <DAYS>   日志保留天数（默认 7）
-v, -V, --version                 打印版本号
```

## 命令能力映射

### 1. 生命周期

```bash
bifrost start -p 9900
bifrost start -p 9900 --daemon
bifrost status
bifrost status --tui
bifrost stop
```

- 前台调试优先用普通 `start`
- 需要后台运行时才用 `--daemon`
- 若未指定端口，默认 `9900`

### 2. Start 完整参数

```bash
bifrost start [OPTIONS]
  -p, --port <PORT>                   代理端口（覆盖全局 -p）
  -H, --host <HOST>                   监听地址（覆盖全局 -H）
      --socks5-port <PORT>            独立 SOCKS5 端口（覆盖全局）
  -d, --daemon                        后台守护模式运行
      --skip-cert-check               跳过 CA 证书安装检查
      --access-mode <MODE>            访问模式：local_only|whitelist|interactive|allow_all
      --whitelist <IPS>               客户端 IP 白名单（逗号分隔，支持 CIDR）
      --allow-lan                     允许局域网（私有网络）客户端
      --proxy-user <USER:PASS>        代理认证凭据（USER:PASS 格式，可重复指定）
      --intercept                     启用 TLS/HTTPS 拦截
      --no-intercept                  禁用 TLS/HTTPS 拦截（默认）
      --intercept-exclude <DOMAINS>   排除域名不拦截（逗号分隔，支持通配符）
      --intercept-include <DOMAINS>   强制拦截域名（最高优先级，即使全局关闭也生效）
      --app-intercept-exclude <APPS>  排除应用不拦截（逗号分隔，支持通配符）
      --app-intercept-include <APPS>  强制拦截应用（最高优先级）
      --unsafe-ssl                    跳过上游 TLS 证书校验（危险，仅测试用）
      --no-disconnect-on-config-change  TLS 配置变更时不自动断开受影响连接
      --rules <RULE>                  代理规则（可重复指定）
      --rules-file <PATH>             规则文件路径
      --system-proxy                  启用系统代理
      --proxy-bypass <LIST>           系统代理绕行列表（逗号分隔）
      --cli-proxy                     代理运行期间启用 CLI 代理环境变量
      --cli-proxy-no-proxy <LIST>     CLI 代理 no-proxy 列表（逗号分隔）
```

TLS 拦截优先级（从高到低）：

1. 规则级别（`tlsIntercept://`、`tlsPassthrough://`）
2. `--intercept-include` / `--app-intercept-include`：强制拦截
3. `--intercept-exclude` / `--app-intercept-exclude`：强制不拦截
4. `--intercept` / `--no-intercept`：全局开关（默认关闭）

### 3. TLS / CA

```bash
bifrost ca generate
bifrost ca generate -f          # 强制重新生成
bifrost ca install
bifrost ca info
bifrost ca export -o ./bifrost-ca.pem

bifrost start --intercept
bifrost start --no-intercept
bifrost start --intercept-exclude '*.apple.com,*.microsoft.com'
bifrost start --intercept-include '*.api.local'
bifrost start --app-intercept-exclude '*Safari'
bifrost start --app-intercept-include '*Chrome'
```

- 需要解密 HTTPS 时，先处理 `ca`，再启用 `--intercept`
- 精确控制优先用 `--intercept-include` / `--intercept-exclude`
- 应用级别控制用 `--app-intercept-include` / `--app-intercept-exclude`
- 若只是转发 HTTPS 而非查看明文，可保持 `--no-intercept`

### 4. 规则管理

```bash
bifrost rule list
bifrost rule add demo -c "example.com host://127.0.0.1:3000"
bifrost rule add demo -f ./rules/demo.txt
bifrost rule update demo -c "example.com host://127.0.0.1:4000"
bifrost rule update demo -f ./rules/demo-v2.txt
bifrost rule show demo                     # 别名：get
bifrost rule enable demo
bifrost rule disable demo
bifrost rule delete demo
bifrost rule sync                          # 与远端服务器同步规则
```

- 新增/更新规则时，`--content` 和 `--file` 至少提供一个
- 单次验证可直接用 `start --rules "..."`
- 多条或长期规则优先放入规则文件，再用 `--rules-file`

### 5. Group 管理

```bash
# Group 查询
bifrost group list                            # 列出所有 groups
bifrost group list -k "team" -l 20            # 按关键词搜索，限制结果数
bifrost group show <group_id>                 # 查看 group 详情

# Group 规则查询
bifrost group rule list <group_id>            # 列出 group 下所有规则
bifrost group rule show <group_id> <name>     # 查看 group 规则详情

# Group 规则增删改
bifrost group rule add <group_id> <name> -c "example.com host://127.0.0.1:3000"
bifrost group rule add <group_id> <name> -f ./rules/demo.txt
bifrost group rule update <group_id> <name> -c "new content"
bifrost group rule update <group_id> <name> -f ./rules/demo-v2.txt
bifrost group rule delete <group_id> <name>

# Group 规则启用/禁用
bifrost group rule enable <group_id> <name>
bifrost group rule disable <group_id> <name>
```

- **需要代理运行中**：`group` 命令通过 admin API 通信，需先 `bifrost start`
- Group 规则新增/更新时，`--content` 和 `--file` 至少提供一个（add 可以不带，默认空内容）
- `group rule show` 别名：`get`
- `group list` 支持 `-k/--keyword` 模糊搜索和 `-l/--limit` 限制最大结果数（默认 50）

### 6. 脚本管理

```bash
bifrost script list
bifrost script list -t request             # 按类型过滤：request, response, decode
bifrost script add request demo -c 'module.exports = ...'
bifrost script add response demo -f ./scripts/demo.js
bifrost script update request demo -c '...'
bifrost script update response demo -f ./scripts/demo-v2.js
bifrost script show demo                   # 模糊匹配，跨类型查找
bifrost script show request demo           # 精确指定类型
bifrost script run demo                    # 使用内置 mock 数据测试脚本
bifrost script run request demo            # 精确指定类型运行
bifrost script delete request demo
```

- 脚本类型：`request`（请求修改）、`response`（响应修改）、`decode`（解码）
- 类型别名：`req`→request、`res`→response、`dec`→decode
- `show` 和 `run` 支持只传名称进行模糊匹配；如有歧义需指定类型
- `run` 会使用内置 mock 请求/响应数据执行脚本，输出修改结果和日志

### 7. 变量值

```bash
bifrost value list
bifrost value set LOCAL_SERVER 127.0.0.1:3000    # 别名：add
bifrost value get LOCAL_SERVER                    # 别名：show
bifrost value update LOCAL_SERVER 127.0.0.1:4000
bifrost value import ./values.json               # 支持 .txt/.kv/.json
bifrost value delete LOCAL_SERVER
```

- 规则中可使用 `${NAME}` 和 `${env.VAR_NAME}`
- 需要复用环境相关地址或 token 时，优先用 `value set` 而不是把值硬编码到规则里

### 8. 访问控制

```bash
bifrost whitelist status
bifrost whitelist list
bifrost whitelist add 192.168.1.0/24
bifrost whitelist remove 192.168.1.0/24
bifrost whitelist allow-lan true
```

- 默认应偏向最小暴露面
- 只有明确需要局域网访问时，再配合 `allow-lan` 或白名单

### 9. 代理认证

```bash
bifrost start --proxy-user admin:password123
bifrost start --proxy-user user1:pass1 --proxy-user user2:pass2
```

- 通过运行时配置管理：

```bash
bifrost config set access.userpass.enabled true
bifrost config add access.userpass.accounts 'user:pass'
bifrost config set access.userpass.loopback-requires-auth false
```

### 10. 系统代理

```bash
bifrost system-proxy status
bifrost system-proxy enable --host 127.0.0.1 --port 9900 --bypass 'localhost,127.0.0.1,*.local'
bifrost system-proxy disable
```

- 这是高影响命令，可能触发管理员权限
- 没有用户明确授权时，不要主动修改系统代理

### 11. 运行时配置

```bash
bifrost config show
bifrost config show --json
bifrost config show --section tls            # 按 section 过滤：tls, traffic, access, server
bifrost config get tls.enabled
bifrost config get tls.enabled --json
bifrost config set tls.enabled true
bifrost config add tls.exclude '*.example.com'
bifrost config remove tls.exclude '*.example.com'
bifrost config reset tls.enabled -y
bifrost config reset all -y                  # 重置所有配置
bifrost config clear-cache -y
bifrost config disconnect example.com
bifrost config export -o ./config.toml --format toml
bifrost config export --format json
```

- `config` 走的是运行中代理的管理接口，不是直接改静态文件
- 修改后若涉及 TLS 或连接行为，必要时执行 `config disconnect <domain>` 触发重连验证
- 查询前先确认目标实例端口；如有显式端口，使用同一套 `-p`

可用的配置键：

| Section | Key                                          | 类型 | 说明                                                 |
| ------- | -------------------------------------------- | -- | -------------------------------------------------- |
| server  | `server.timeout-secs`                        | 数值 | 服务器超时秒数                                            |
| server  | `server.http1-max-header-size`               | 大小 | HTTP/1.1 最大请求头大小                                   |
| server  | `server.http2-max-header-list-size`          | 大小 | HTTP/2 最大头列表大小                                     |
| server  | `server.websocket-handshake-max-header-size` | 大小 | WebSocket 握手最大头大小                                  |
| tls     | `tls.enabled`                                | 布尔 | TLS 拦截开关                                           |
| tls     | `tls.unsafe-ssl`                             | 布尔 | 跳过上游证书校验                                           |
| tls     | `tls.disconnect-on-change`                   | 布尔 | 配置变更时自动断开连接                                        |
| tls     | `tls.exclude`                                | 列表 | TLS 拦截排除域名                                         |
| tls     | `tls.include`                                | 列表 | TLS 拦截包含域名                                         |
| tls     | `tls.app-exclude`                            | 列表 | TLS 拦截排除应用                                         |
| tls     | `tls.app-include`                            | 列表 | TLS 拦截包含应用                                         |
| traffic | `traffic.max-records`                        | 数值 | 最大记录数                                              |
| traffic | `traffic.max-db-size`                        | 大小 | 最大数据库大小                                            |
| traffic | `traffic.max-body-size`                      | 大小 | 最大 body 大小                                         |
| traffic | `traffic.max-buffer-size`                    | 大小 | 最大缓冲区大小                                            |
| traffic | `traffic.retention-days`                     | 数值 | 记录保留天数                                             |
| traffic | `traffic.sse-stream-flush-bytes`             | 大小 | SSE 流刷新字节数                                         |
| traffic | `traffic.sse-stream-flush-interval-ms`       | 数值 | SSE 流刷新间隔（毫秒）                                      |
| traffic | `traffic.ws-payload-flush-bytes`             | 大小 | WebSocket 载荷刷新字节数                                  |
| traffic | `traffic.ws-payload-flush-interval-ms`       | 数值 | WebSocket 载荷刷新间隔（毫秒）                               |
| traffic | `traffic.ws-payload-max-open-files`          | 数值 | WebSocket 载荷最大打开文件数                                |
| access  | `access.mode`                                | 枚举 | 访问模式（local\_only/whitelist/interactive/allow\_all） |
| access  | `access.allow-lan`                           | 布尔 | 允许局域网访问                                            |
| access  | `access.userpass.enabled`                    | 布尔 | 代理认证开关                                             |
| access  | `access.userpass.accounts`                   | 列表 | 代理认证账户列表                                           |
| access  | `access.userpass.loopback-requires-auth`     | 布尔 | 回环地址是否需要认证                                         |

大小类型支持单位：`B`、`KB`、`MB`、`GB`（如 `10MB`、`512KB`）。

### 12. 流量查询

```bash
# 列出流量记录
bifrost traffic list --limit 20
bifrost traffic list --host example.com --format json-pretty
bifrost traffic list --method POST --status 200
bifrost traffic list --status-min 400 --status-max 499
bifrost traffic list --protocol https --content-type json
bifrost traffic list --client-ip 192.168.1.100 --client-app Chrome
bifrost traffic list --has-rule-hit true
bifrost traffic list --is-websocket true
bifrost traffic list --is-sse true
bifrost traffic list --is-tunnel true
bifrost traffic list --cursor 100 --direction forward  # 分页

# 查看单条详情
bifrost traffic get 123
bifrost traffic get 123 --request-body --response-body

# 搜索（等同于 bifrost search）
bifrost traffic search openai --domain api.openai.com --method POST
```

`traffic list` 完整过滤参数：

```
-l, --limit <N>               最大返回数（默认 50）
    --cursor <SEQ>            分页游标（来自 next_cursor/prev_cursor）
    --direction <DIR>         分页方向：backward（默认）或 forward
    --method <METHOD>         HTTP 方法过滤
    --status <CODE>           精确状态码过滤
    --status-min <CODE>       状态码下限
    --status-max <CODE>       状态码上限
    --protocol <PROTO>        协议过滤（http/https/ws/wss/h3）
    --host <TEXT>             Host 包含过滤
    --url <TEXT>              URL 包含过滤
    --path <TEXT>             Path 包含过滤
    --content-type <TYPE>     Content-Type 过滤
    --client-ip <IP>          客户端 IP 过滤
    --client-app <APP>        客户端应用过滤
    --has-rule-hit <BOOL>     是否命中规则
    --is-websocket <BOOL>     仅 WebSocket
    --is-sse <BOOL>           仅 SSE
    --is-tunnel <BOOL>        仅隧道
-f, --format <FMT>            输出格式：table|compact|json|json-pretty
    --no-color                禁用彩色输出
```

### 13. 全文搜索

```bash
bifrost search openai --domain api.openai.com --method POST
bifrost search --interactive                    # 交互式 TUI 模式
bifrost search error --status 5xx --format json-pretty
bifrost search auth --req-header --host api.example.com
bifrost search '{"error"' --res-body --content-type json
```

`search` 完整参数：

```
[keyword]                     搜索关键词（URL/headers/body 全文搜索）
-i, --interactive             交互式 TUI 模式（无关键词时默认进入）
-l, --limit <N>               最大结果数（默认 50）
-f, --format <FMT>            输出格式：table|compact|json|json-pretty
    --url                     仅搜索 URL/path
    --headers                 仅搜索 headers（请求+响应）
    --body                    仅搜索 body（请求+响应）
    --req-header              仅搜索请求 headers
    --res-header              仅搜索响应 headers
    --req-body                仅搜索请求 body
    --res-body                仅搜索响应 body
    --status <FILTER>         状态过滤：2xx|3xx|4xx|5xx|error
    --method <METHOD>         HTTP 方法过滤
    --host <TEXT>             Host 包含过滤
    --path <TEXT>             Path 包含过滤
    --protocol <PROTO>        协议过滤：HTTP|HTTPS|WS|WSS
    --content-type <TYPE>     Content-Type 过滤（json/xml/html/form 等）
    --domain <PATTERN>        域名 pattern 过滤
    --no-color                禁用彩色输出
```

### 14. 升级

```bash
bifrost upgrade
bifrost upgrade -y            # 跳过确认
```

## 环境变量

| 变量                 | 说明                                               | 默认值                |
| ------------------ | ------------------------------------------------ | ------------------ |
| `BIFROST_DATA_DIR` | 数据目录路径（含 config/rules/values/scripts/certs/logs） | `~/.bifrost`（平台相关） |
| `RUST_LOG`         | 日志级别与过滤器                                         | `info`             |

## 推荐工作流

### 本地调试一个规则

```bash
bifrost start -p 9900 \
  --rules "example.com reqHeaders://X-Debug=1" \
  --no-intercept
```

然后用 `curl` 或目标客户端走 `127.0.0.1:9900`，再执行：

```bash
bifrost traffic list --limit 20
bifrost traffic get 1
```

### 调试 HTTPS 明文请求

```bash
bifrost ca generate
bifrost ca install
bifrost start -p 9900 --intercept
```

若只想抓特定域名，优先改成：

```bash
bifrost start -p 9900 --intercept-include '*.target.local'
```

### 调试长期规则文件

```bash
bifrost rule add local-debug -f ./rules/local-debug.txt
bifrost rule enable local-debug
bifrost rule show local-debug
```

需要单独验证规则内容时，再参考：

```bash
bifrost start --rules-file ./rules/local-debug.txt
```

### 脚本开发调试

```bash
# 添加请求修改脚本
bifrost script add request add-header -f ./scripts/add-header.js

# 测试脚本（使用内置 mock 数据）
bifrost script run add-header

# 查看脚本内容
bifrost script show add-header

# 更新脚本
bifrost script update request add-header -f ./scripts/add-header-v2.js
```

### 排查某个域名请求

```bash
bifrost search example --domain example.com --format json-pretty
bifrost traffic list --host example.com --limit 20
bifrost traffic get <id> --request-body --response-body
```

### 管理 Group 规则

```bash
# 查找目标 group
bifrost group list -k "team"
bifrost group show <group_id>

# 查看/管理 group 规则
bifrost group rule list <group_id>
bifrost group rule add <group_id> api-mock -c "api.example.com host://127.0.0.1:3000"
bifrost group rule enable <group_id> api-mock
bifrost group rule show <group_id> api-mock

# 更新已有规则
bifrost group rule update <group_id> api-mock -f ./rules/api-mock-v2.txt

# 不再需要时禁用或删除
bifrost group rule disable <group_id> api-mock
bifrost group rule delete <group_id> api-mock
```

### 配置代理认证

```bash
bifrost start --proxy-user admin:secret --proxy-user viewer:pass123
# 或运行时动态配置
bifrost config set access.userpass.enabled true
bifrost config add access.userpass.accounts 'admin:secret'
```

## 特别说明

本文档仅列出常用命令和参数摘要。**CLI 的完整参数、用法说明和示例均内置于** **`--help`** **输出中**，包括协议列表、规则语法快速参考、变量展开说明等。遇到本文档未覆盖的参数或用法时，**必须**先执行以下命令获取权威信息：

```bash
bifrost -h                    # 完整帮助（含协议、规则语法、环境变量等）
bifrost <command> -h          # 子命令帮助（如 bifrost start -h、bifrost script -h）
bifrost <command> <action> -h # 子动作帮助（如 bifrost rule add -h、bifrost config set -h）
```

`-h` 输出的信息始终与当前安装版本一致，是最准确的参数参考。本文档可能因版本迭代而滞后，**以** **`--help`** **输出为准**。

## Agent 行为建议

- 优先通过 CLI 完成任务，不要直接手改 `~/.bifrost` 下的数据
- 禁止使用临时 `BIFROST_DATA_DIR`，始终使用默认数据目录，确保证书和配置正确
- 如果用户没有要求修改系统环境，不要开启 `--system-proxy`、`--cli-proxy`
- 如果用户只想验证规则，不必先启用 TLS 拦截
- 遇到不确定的参数或用法，**先执行** **`bifrost <command> -h`** **获取完整手册**，不要猜测

## 参考

- CLI 定义：[crates/bifrost/src/cli.rs](https://github.com/bifrost-proxy/bifrost/blob/main/crates/bifrost/src/cli.rs)
- 启动与命令分发：[crates/bifrost/src/main.rs](https://github.com/bifrost-proxy/bifrost/blob/main/crates/bifrost/src/main.rs)
- 规则语法：[docs/rule.md](https://github.com/bifrost-proxy/bifrost/blob/main/docs/rule.md)
- Pattern 说明：[docs/pattern.md](https://github.com/bifrost-proxy/bifrost/blob/main/docs/pattern.md)
- Operation 说明：[docs/operation.md](https://github.com/bifrost-proxy/bifrost/blob/main/docs/operation.md)

