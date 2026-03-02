# Bifrost

<p align="center">
  <strong>🌈 高性能 HTTP/HTTPS/SOCKS5 代理服务器</strong>
</p>

<p align="center">
  <a href="https://github.com/bifrost-proxy/bifrost/actions"><img src="https://github.com/bifrost-proxy/bifrost/workflows/CI/badge.svg" alt="CI Status"></a>
  <a href="https://github.com/bifrost-proxy/bifrost/releases"><img src="https://img.shields.io/github/v/release/bifrost-proxy/bifrost" alt="Release"></a>
  <a href="https://github.com/bifrost-proxy/bifrost/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
</p>

Bifrost 是一个用 Rust 编写的高性能代理服务器，灵感来源于 [Whistle](https://github.com/avwo/whistle)。它提供强大的请求拦截、修改和规则配置能力，支持 TLS 解密、插件扩展等高级功能。

## ✨ 特性

### 🚀 高性能

- 基于 **Tokio** 异步运行时，支持高并发连接
- 使用 **Hyper** HTTP 库，性能卓越
- 智能连接池，减少连接建立开销

### 🌐 全协议支持

| 协议          | 支持情况 | 说明                        |
| ------------- | -------- | --------------------------- |
| HTTP/1.1      | ✅       | 完整支持                    |
| HTTP/2        | ✅       | 帧级别处理，支持多路复用    |
| HTTP/3 (QUIC) | ✅       | 基于 Quinn 实现，支持 0-RTT |
| HTTPS         | ✅       | TLS 1.2/1.3，支持 MITM 拦截 |
| SOCKS5 TCP    | ✅       | 支持用户名/密码认证         |
| SOCKS5 UDP    | ✅       | UDP ASSOCIATE 完整支持      |
| WebSocket     | ✅       | ws:// 和 wss:// 协议        |
| CONNECT-UDP   | ✅       | MASQUE 协议 (RFC 9298)      |
| gRPC          | ✅       | 基于 HTTP/2                 |
| SSE           | ✅       | Server-Sent Events          |

### 🔒 TLS 拦截 (MITM)

- 自动生成 CA 证书
- 动态签发服务器证书（LRU 缓存优化）
- 支持 SNI 检测
- 可选择性拦截或透传特定域名

### 📝 强大的规则引擎

- **72 种规则协议** - 覆盖路由、修改、注入、控制等场景
- **多种匹配模式** - 域名、IP、正则、通配符、路径匹配
- **请求/响应修改** - Headers、Body、Cookies、状态码等
- **内容注入** - HTML/JS/CSS 注入
- **流量控制** - 延迟、限速、Mock 响应

### 🖥️ 管理界面

- 内置 Web UI 管理界面
- 实时流量监控
- 规则在线编辑
- 请求/响应查看与重放

### 📜 JavaScript 脚本引擎

- 基于 **QuickJS** 的安全沙盒执行环境
- 支持请求脚本 (`reqScript`) 和响应脚本 (`resScript`)
- 内置超时控制和内存限制
- 支持脚本在线编辑和测试

### 🔄 请求重放与管理

- **请求重放** - 支持 HTTP/HTTPS/SSE/WebSocket 请求重放
- **请求集合** - 类似 Postman 的请求管理能力
- **分组管理** - 支持嵌套文件夹组织请求
- **历史记录** - 自动记录重放历史和执行结果
- **规则集成** - 重放时可选择应用代理规则

### 🔐 安全特性

- 访问控制（本地/白名单/交互式）
- IP 白名单和 CIDR 支持
- 局域网访问控制
- 管理端口保护

## 项目结构

```
rust/
├── crates/
│   ├── bifrost-core/       # 核心库：规则解析、匹配器、协议定义
│   ├── bifrost-proxy/      # 代理服务器：HTTP/SOCKS5 代理实现
│   ├── bifrost-tls/        # TLS 处理：CA 证书管理、动态证书生成
│   ├── bifrost-storage/    # 存储层：配置和规则持久化
│   ├── bifrost-script/     # 脚本引擎：基于 QuickJS 的 JavaScript 执行
│   ├── bifrost-admin/      # 管理后台：Web UI、API、请求重放
│   ├── bifrost-cli/        # 命令行工具
│   └── bifrost-tests/      # 集成测试
└── tests/                  # 端到端测试
```

## 安装

### 方式一：一键安装（推荐）

使用 curl 一键安装脚本，自动检测平台和架构：

```bash
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash
```

安装选项：

```bash
# 指定安装目录
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash -s -- --dir /usr/local/bin

# 安装特定版本
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash -s -- --version v0.2.0
```

### 方式二：Homebrew（macOS）

```bash
brew tap bifrost-proxy/bifrost
brew install bifrost
```

### 方式三：从源码构建

#### 环境要求

- Rust 1.70+
- Cargo
- Node.js 18+ & pnpm（用于构建 Web UI）

#### 构建步骤

```bash
# 克隆仓库
git clone https://github.com/bifrost-proxy/bifrost.git
cd bifrost

# 使用安装脚本（推荐）
./install.sh

# 或手动构建
cd web && pnpm install && pnpm build && cd ..
cargo build --release
```

### 方式四：手动下载

从 [Releases](https://github.com/bifrost-proxy/bifrost/releases) 页面下载预编译的二进制文件。

**支持的平台：**

| 平台    | 架构          | 文件                                                  |
| ------- | ------------- | ----------------------------------------------------- |
| Linux   | x64           | `bifrost-vX.X.X-x86_64-unknown-linux-gnu.tar.gz`      |
| Linux   | ARM64         | `bifrost-vX.X.X-aarch64-unknown-linux-gnu.tar.gz`     |
| Linux   | ARMv7         | `bifrost-vX.X.X-armv7-unknown-linux-gnueabihf.tar.gz` |
| macOS   | Intel         | `bifrost-vX.X.X-x86_64-apple-darwin.tar.gz`           |
| macOS   | Apple Silicon | `bifrost-vX.X.X-aarch64-apple-darwin.tar.gz`          |
| Windows | x64           | `bifrost-vX.X.X-x86_64-pc-windows-msvc.zip`           |
| Windows | ARM64         | `bifrost-vX.X.X-aarch64-pc-windows-msvc.zip`          |

## 快速开始

### 运行

```bash
# 启动代理服务器（默认端口 9900）
cargo run --bin bifrost

# 指定端口和监听地址（全局参数需放在子命令前）
cargo run --bin bifrost -- -p 9000 -H 127.0.0.1

# 启用 HTTP 和 SOCKS5 代理
cargo run --bin bifrost -- -p 9900 --socks5-port 1080

# 守护进程模式
cargo run --bin bifrost -- start --daemon

# 禁用 TLS 拦截
cargo run --bin bifrost -- start --no-intercept

# 跳过证书安装检查（CI/CD 环境）
cargo run --bin bifrost -- start --skip-cert-check

# 启动时指定规则
cargo run --bin bifrost -- start --rules "example.com host://127.0.0.1:3000"

# 指定自定义数据目录
BIFROST_DATA_DIR=/tmp/bifrost cargo run --bin bifrost

# 启用系统代理
cargo run --bin bifrost -- start --system-proxy
```

### 管理端界面

启动代理服务后，可以通过浏览器访问管理端界面：

```
http://127.0.0.1:<端口>/_bifrost/
```

例如，使用默认端口 9900 启动时，访问地址为：

```
http://127.0.0.1:9900/_bifrost/
```

管理端提供以下功能：

| 路径                      | 功能            |
| ------------------------- | --------------- |
| `/_bifrost/`              | Web UI 界面     |
| `/_bifrost/api/rules/*`   | 规则管理 API    |
| `/_bifrost/api/values/*`  | Values 管理 API |
| `/_bifrost/api/traffic/*` | 流量记录 API    |
| `/_bifrost/api/metrics/*` | 指标监控 API    |
| `/_bifrost/api/system/*`  | 系统信息 API    |
| `/_bifrost/api/scripts/*` | 脚本管理 API    |
| `/_bifrost/api/replay/*`  | 请求重放 API    |

> **注意**：出于安全考虑，管理端仅允许通过 `127.0.0.1` 或 `localhost` 访问。

### 全局参数

```bash
bifrost [OPTIONS] [COMMAND]

# 全局选项
-p, --port <PORT>           HTTP 代理端口 [默认: 9900]
-H, --host <HOST>           监听地址 [默认: 0.0.0.0]
    --socks5-port <PORT>    SOCKS5 代理端口
-l, --log-level <LEVEL>     日志级别 [默认: info]
-h, --help                  显示帮助信息
-V, --version               显示版本号
```

### 启动命令 (start)

```bash
# 基本启动（前台运行）
bifrost start

# 守护进程模式启动
bifrost start --daemon

# 自定义端口启动
bifrost -p 9000 start

# 同时启用 HTTP 和 SOCKS5 代理
bifrost -p 9900 --socks5-port 1080 start

# 跳过证书安装检查（适用于无交互环境）
bifrost start --skip-cert-check

# 禁用 TLS 拦截（不解密 HTTPS 流量）
bifrost start --no-intercept

# 排除特定域名的 TLS 拦截
bifrost start --intercept-exclude "*.example.com,internal.corp.com"

# 启动时指定规则
bifrost start --rules "example.com host://127.0.0.1:3000"
bifrost start --rules "api.test.com proxy://127.0.0.1:9900" --rules "*.cdn.com tlsPassthrough://"

# 从文件加载规则
bifrost start --rules-file ./my-rules.txt

# 访问控制配置
bifrost start --access-mode local_only       # 仅本地访问（默认）
bifrost start --access-mode whitelist        # 白名单模式
bifrost start --access-mode interactive      # 交互式确认模式
bifrost start --access-mode allow_all        # 允许所有（不推荐）

# 配置 IP 白名单
bifrost start --access-mode whitelist --whitelist "192.168.1.100,10.0.0.0/8"

# 允许局域网访问
bifrost start --allow-lan

# 启用系统代理（代理启动时自动配置系统代理）
bifrost start --system-proxy
bifrost start --system-proxy --proxy-bypass "localhost,127.0.0.1,*.local"

# 跳过上游 TLS 验证（危险，仅用于测试自签名证书的后端）
bifrost start --unsafe-ssl
```

**start 命令参数详解：**

| 参数                            | 说明                                                             |
| ------------------------------- | ---------------------------------------------------------------- |
| `-d, --daemon`                  | 以守护进程模式在后台运行                                         |
| `--skip-cert-check`             | 跳过 CA 证书安装检查，适用于 CI/CD 或无交互环境                  |
| `--access-mode <MODE>`          | 访问控制模式：`local_only`/`whitelist`/`interactive`/`allow_all` |
| `--whitelist <IPS>`             | 客户端 IP 白名单，逗号分隔，支持 CIDR 表示法                     |
| `--allow-lan`                   | 允许局域网（私有网络）客户端访问                                 |
| `--no-intercept`                | 禁用 TLS/HTTPS 拦截                                              |
| `--intercept-exclude <DOMAINS>` | 排除 TLS 拦截的域名列表，逗号分隔，支持通配符                    |
| `--unsafe-ssl`                  | 跳过上游服务器 TLS 证书验证（危险，仅用于测试）                  |
| `--rules <RULE>`                | 代理规则，可多次指定                                             |
| `--rules-file <PATH>`           | 规则文件路径，每行一条规则                                       |
| `--system-proxy`                | 启用系统代理配置                                                 |
| `--proxy-bypass <LIST>`         | 系统代理绕过列表，逗号分隔                                       |

### 基本命令

```bash
# 查看状态
bifrost status

# 停止服务
bifrost stop

# CA 证书管理
bifrost ca generate           # 生成 CA 证书
bifrost ca generate --force   # 强制重新生成 CA 证书
bifrost ca export             # 导出 CA 证书到 bifrost-ca.crt
bifrost ca export -o ca.crt   # 导出 CA 证书到指定路径
bifrost ca info               # 查看 CA 证书详细信息

# 规则管理
bifrost rule list                           # 列出所有规则
bifrost rule add <name> --content "rule"    # 添加规则
bifrost rule add <name> --file rules.txt    # 从文件添加规则
bifrost rule enable <name>                  # 启用规则
bifrost rule disable <name>                 # 禁用规则
bifrost rule delete <name>                  # 删除规则
bifrost rule show <name>                    # 查看规则内容

# IP 白名单管理
bifrost whitelist list                      # 列出白名单
bifrost whitelist add 192.168.1.100         # 添加 IP 到白名单
bifrost whitelist add 10.0.0.0/8            # 添加 CIDR 网段
bifrost whitelist remove 192.168.1.100      # 移除 IP
bifrost whitelist allow-lan true            # 启用局域网访问
bifrost whitelist allow-lan false           # 禁用局域网访问
bifrost whitelist status                    # 查看访问控制状态

# Values 管理（模板变量）
bifrost value list                          # 列出所有 values
bifrost value get <name>                    # 获取指定 value 的值
bifrost value set <name> <value>            # 设置 value
bifrost value delete <name>                 # 删除 value
bifrost value import <file>                 # 从文件导入 values (KEY=VALUE 格式)

# 系统代理管理
bifrost system-proxy status                 # 查看系统代理状态
bifrost system-proxy enable                 # 启用系统代理（使用全局端口）
bifrost system-proxy enable --host 127.0.0.1 --port 9900  # 指定主机和端口
bifrost system-proxy enable --bypass "localhost,127.0.0.1,*.local"  # 配置绕过列表
bifrost system-proxy disable                # 禁用系统代理
```

### 环境变量

| 环境变量           | 说明             | 默认值       |
| ------------------ | ---------------- | ------------ |
| `BIFROST_DATA_DIR` | 数据目录路径     | `~/.bifrost` |
| `RUST_LOG`         | 日志级别和过滤器 | `info`       |

通过设置环境变量，可以自定义 Bifrost 的行为：

```bash
# 指定自定义数据目录
export BIFROST_DATA_DIR=/path/to/custom/dir
bifrost start

# 或直接在命令中指定
BIFROST_DATA_DIR=/tmp/bifrost bifrost start

# 设置日志级别
RUST_LOG=debug bifrost start

# 高级日志过滤（仅显示特定模块的 debug 日志）
RUST_LOG=bifrost_proxy=debug,info bifrost start
```

## 模块说明

### bifrost-core

核心库，提供基础功能：

- **规则解析** (`rule/`) - 解析和管理代理规则
- **匹配器** (`matcher/`) - URL 模式匹配（域名、IP、正则、通配符）
- **协议定义** (`protocol.rs`) - 71 种协议操作类型

```rust
use bifrost_core::{parse_rules, DomainMatcher, Protocol};

// 解析规则
let rules = parse_rules("example.com host://127.0.0.1");

// 创建匹配器
let matcher = DomainMatcher::new("*.example.com");
```

### bifrost-proxy

代理服务器实现：

- **HTTP 代理** - 处理 HTTP/HTTPS 请求
- **SOCKS5 代理** - SOCKS5 协议支持（可选认证）
- **WebSocket** - WebSocket 连接代理
- **隧道** - CONNECT 隧道处理

```rust
use bifrost_proxy::{ProxyConfig, ProxyServer};

let config = ProxyConfig {
    port: 9900,
    host: "0.0.0.0".to_string(),
    socks5_port: Some(1080),
    ..Default::default()
};

let server = ProxyServer::new(config);
server.run().await?;
```

### bifrost-tls

TLS 证书管理：

- **CA 证书** - 根证书生成和加载
- **动态证书** - 按需生成服务器证书
- **证书缓存** - LRU 缓存优化性能
- **SNI 处理** - 服务器名称指示支持

```rust
use bifrost_tls::{generate_root_ca, DynamicCertGenerator, CertCache};

// 生成 CA 证书
let ca = generate_root_ca()?;

// 创建动态证书生成器
let generator = DynamicCertGenerator::new(ca);
let cert = generator.generate("example.com")?;
```

### bifrost-storage

配置和状态存储：

- **规则存储** - 规则文件的持久化
- **Values 存储** - 模板变量的持久化
- **配置管理** - 代理配置
- **状态管理** - 运行时状态

```rust
use bifrost_storage::{RulesStorage, RuleFile, ValuesStorage, StateManager};

// 规则存储
let storage = RulesStorage::new()?;
let rule = RuleFile::new("my-rule", "example.com host://127.0.0.1");
storage.save(&rule)?;

// Values 存储
let mut values = ValuesStorage::new()?;
values.set_value("API_HOST", "127.0.0.1:3000")?;
let host = values.get_value("API_HOST");

// 状态管理
let state = StateManager::new()?;
let enabled_groups = state.enabled_groups();
```

## Values 变量系统

Values 是一套统一的变量管理系统，用于在规则中使用可复用的变量。变量可以通过 CLI 或 Web 界面进行管理。

### 变量定义

变量存储在 `~/.bifrost/values/` 目录下，每个变量一个文件，文件名为变量名，内容为变量值。

### 在规则中使用变量

规则支持使用 `${...}` 语法进行变量展开：

```
# 使用存储的变量（通过 bifrost value set 设置）
example.com host://${API_HOST}
*.api.com proxy://${PROXY_SERVER}
test.com reqHeaders://(Authorization=Bearer ${AUTH_TOKEN})

# 使用环境变量（${env.变量名} 语法）
api.example.com reqHeaders://(Authorization: ${env.API_TOKEN})
*.internal.com host://${env.INTERNAL_HOST}
```

### 变量类型

| 语法              | 说明                                    | 示例               |
| ----------------- | --------------------------------------- | ------------------ |
| `${name}`         | 展开为通过 `bifrost value set` 存储的值 | `${API_HOST}`      |
| `${env.VAR_NAME}` | 展开为系统环境变量                      | `${env.API_TOKEN}` |

### 变量优先级

当规则文件内定义了同名变量（块级变量）时，优先使用块级变量，其次使用 values 目录中的变量：

````
# 规则文件内的块级变量
```values
API_HOST=local.dev:3000
````

# 使用变量

example.com host://${API_HOST} # 优先使用块级变量

```

## 规则语法

Bifrost 支持类似其他代理工具的规则语法：

```

# 基本格式：pattern protocol://value

# Host 映射

example.com host://127.0.0.1
\*.api.example.com host://192.168.1.100

# 代理转发

example.com proxy://proxy.server:9900

# 请求修改

example.com reqHeaders://(content-type=application/json)
example.com reqBody://{"key": "value"}
example.com method://POST

# 响应修改

example.com resHeaders://(cache-control=no-cache)
example.com resBody://{"response": "data"}
example.com statusCode://200

# 内容注入

example.com htmlAppend://</script><script>alert(1)</script>
example.com jsAppend://console.log('injected')
example.com cssAppend://body{background:red}

# 延迟和限速

example.com reqDelay://1000
example.com resDelay://500
example.com resSpeed://10

# DNS 解析

example.com dns://8.8.8.8
\*.internal.corp dns://192.168.1.1:53
api.service.com dns://8.8.8.8,8.8.4.4

# TLS 控制

example.com tlsIntercept://
example.com tlsPassthrough://

# 脚本修改

example.com reqScript://modify-request
example.com resScript://inject-data

```

### 支持的协议（71 种）

| 分类     | 协议                                                                                                                                                                                                 |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 路由     | `host`, `xhost`, `http`, `https`, `ws`, `wss`, `proxy`, `redirect`, `file`, `tpl`, `rawfile`                                                                                                         |
| DNS      | `dns`                                                                                                                                                                                                |
| 控制     | `tlsIntercept`, `tlsPassthrough`, `passthrough`, `delete`                                                                                                                                            |
| 请求修改 | `reqHeaders`, `reqBody`, `reqPrepend`, `reqAppend`, `reqCookies`, `reqCors`, `reqDelay`, `reqSpeed`, `reqType`, `reqCharset`, `reqReplace`, `method`, `auth`, `ua`, `referer`, `urlParams`, `params` |
| 响应修改 | `resHeaders`, `resBody`, `resPrepend`, `resAppend`, `resCookies`, `resCors`, `resDelay`, `resSpeed`, `resType`, `resCharset`, `resReplace`, `statusCode`, `cache`, `attachment`, `trailers`, `resMerge`, `headerReplace`, `forwardedFor` |
| 内容注入 | `htmlAppend`, `htmlPrepend`, `htmlBody`, `jsAppend`, `jsPrepend`, `jsBody`, `cssAppend`, `cssPrepend`, `cssBody`                                                                                     |
| 脚本     | `reqScript`, `resScript` - 使用 JavaScript 脚本修改请求/响应                                                                                                          |
| 高级     | `rulesFile`, `sniCallback`, `urlReplace`                                                                                                                              |

## 脚本引擎

Bifrost 内置基于 QuickJS 的 JavaScript 脚本引擎，支持通过脚本动态修改请求和响应。

### 脚本类型

| 类型        | 协议          | 说明                     |
| ----------- | ------------- | ------------------------ |
| 请求脚本    | `reqScript`   | 在请求发送前执行         |
| 响应脚本    | `resScript`   | 在响应返回给客户端前执行 |

### 脚本存储

脚本文件存储在 `~/.bifrost/scripts/` 目录下：

```

~/.bifrost/scripts/
├── request/ # 请求脚本目录
│ ├── modify-auth.js
│ └── add-headers.js
└── response/ # 响应脚本目录
├── inject-data.js
└── transform.js

````

### 请求脚本示例

请求脚本接收 `request` 对象，返回修改后的请求数据：

```javascript
function main(request, context) {
  return {
    url: request.url,
    method: request.method,
    headers: {
      ...request.headers,
      'Authorization': 'Bearer ' + context.values.API_TOKEN,
      'X-Custom-Header': 'custom-value'
    },
    body: request.body
  };
}
````

**request 对象结构：**

| 属性      | 类型   | 说明      |
| --------- | ------ | --------- |
| `url`     | string | 请求 URL  |
| `method`  | string | HTTP 方法 |
| `headers` | object | 请求头    |
| `body`    | string | 请求体    |

**context 对象结构：**

| 属性            | 类型     | 说明                     |
| --------------- | -------- | ------------------------ |
| `values`        | object   | Values 变量（key-value） |
| `matched_rules` | string[] | 匹配到的规则列表         |

### 响应脚本示例

响应脚本接收 `response` 对象，返回修改后的响应数据：

```javascript
function main(response, context) {
  let body = response.body;

  if (response.headers["content-type"]?.includes("application/json")) {
    try {
      let data = JSON.parse(body);
      data.injected = true;
      data.timestamp = Date.now();
      body = JSON.stringify(data);
    } catch (e) {
      // 解析失败，保持原始 body
    }
  }

  return {
    status: response.status,
    headers: {
      ...response.headers,
      "X-Modified-By": "bifrost-script",
    },
    body: body,
  };
}
```

**response 对象结构：**

| 属性      | 类型   | 说明        |
| --------- | ------ | ----------- |
| `status`  | number | HTTP 状态码 |
| `headers` | object | 响应头      |
| `body`    | string | 响应体      |

### 脚本使用

在规则中引用脚本：

```
# 使用请求脚本
api.example.com reqScript://modify-auth

# 使用响应脚本
api.example.com resScript://inject-data

# 同时使用请求和响应脚本
api.example.com reqScript://add-headers resScript://transform
```

### 脚本 API

通过管理端 API 管理脚本：

| 方法   | 端点                                  | 说明         |
| ------ | ------------------------------------- | ------------ |
| GET    | `/_bifrost/api/scripts`               | 列出所有脚本 |
| GET    | `/_bifrost/api/scripts/{type}/{name}` | 获取脚本内容 |
| PUT    | `/_bifrost/api/scripts/{type}/{name}` | 保存脚本     |
| DELETE | `/_bifrost/api/scripts/{type}/{name}` | 删除脚本     |
| POST   | `/_bifrost/api/scripts/test`          | 测试脚本     |

### 安全限制

脚本引擎内置以下安全限制：

| 限制项   | 默认值 | 说明                          |
| -------- | ------ | ----------------------------- |
| 执行超时 | 10 秒  | 脚本执行超时自动终止          |
| 内存限制 | 16 MB  | 脚本内存使用上限              |
| 危险函数 | 禁用   | `eval`、`Function` 等已被移除 |

## 请求重放与管理

Bifrost 提供类似 Postman 的请求管理和重放能力，支持保存、组织和重放 HTTP 请求。

### 功能概览

- **请求重放** - 支持 HTTP/HTTPS/SSE/WebSocket 请求的重放
- **请求集合** - 保存和管理常用请求
- **分组管理** - 使用文件夹组织请求，支持嵌套
- **历史记录** - 自动记录重放历史
- **规则集成** - 重放时可选择是否应用代理规则

### 支持的请求类型

| 类型      | 说明                        |
| --------- | --------------------------- |
| HTTP      | 标准 HTTP/HTTPS 请求        |
| SSE       | Server-Sent Events 流式请求 |
| WebSocket | WebSocket 双向通信          |

### 请求体格式

| 格式        | Content-Type                        |
| ----------- | ----------------------------------- |
| JSON        | `application/json`                  |
| XML         | `application/xml`                   |
| Text        | `text/plain`                        |
| HTML        | `text/html`                         |
| JavaScript  | `application/javascript`            |
| Form Data   | `multipart/form-data`               |
| URL Encoded | `application/x-www-form-urlencoded` |
| Binary      | `application/octet-stream`          |

### 规则配置

重放请求时可以配置规则应用方式：

| 模式       | 说明                               |
| ---------- | ---------------------------------- |
| `enabled`  | 应用所有启用的规则                 |
| `selected` | 仅应用选中的规则                   |
| `none`     | 不应用任何规则（直接发送原始请求） |

### 重放 API

| 方法   | 端点                                  | 说明           |
| ------ | ------------------------------------- | -------------- |
| POST   | `/_bifrost/api/replay/execute`        | 执行 HTTP 重放 |
| POST   | `/_bifrost/api/replay/execute/stream` | 执行 SSE 重放  |
| GET    | `/_bifrost/api/replay/execute/ws`     | 执行 WebSocket |
| GET    | `/_bifrost/api/replay/groups`         | 列出分组       |
| POST   | `/_bifrost/api/replay/groups`         | 创建分组       |
| GET    | `/_bifrost/api/replay/requests`       | 列出请求       |
| POST   | `/_bifrost/api/replay/requests`       | 保存请求       |
| PUT    | `/_bifrost/api/replay/requests/{id}`  | 更新请求       |
| DELETE | `/_bifrost/api/replay/requests/{id}`  | 删除请求       |
| GET    | `/_bifrost/api/replay/history`        | 获取历史记录   |
| DELETE | `/_bifrost/api/replay/history`        | 清空历史记录   |
| GET    | `/_bifrost/api/replay/stats`          | 获取统计信息   |

### 存储限制

| 限制项       | 数量  |
| ------------ | ----- |
| 最大请求数   | 1000  |
| 最大历史记录 | 10000 |
| 最大并发重放 | 100   |

### 数据存储

请求集合数据存储在 `~/.bifrost/replay.db`（SQLite 数据库）。

## 配置

默认配置文件位于 `~/.bifrost/`：

```
~/.bifrost/
├── bifrost.pid     # 进程 PID 文件
├── bifrost.log     # 日志文件
├── bifrost.err     # 错误日志
├── rules/          # 规则文件目录
├── values/         # Values 变量目录
├── scripts/        # 脚本目录
│   ├── request/    # 请求脚本
│   └── response/   # 响应脚本
├── certs/          # 证书目录
│   ├── ca.crt      # CA 证书
│   └── ca.key      # CA 私钥
└── replay.db       # 请求集合数据库

```

## 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test --package bifrost-core
cargo test --package bifrost-proxy

# 运行集成测试
cargo test --test http_proxy_test
cargo test --test https_proxy_test
cargo test --test socks5_test
```

## 开发

### 开发环境初始化

首次克隆仓库后，请运行以下命令初始化开发环境：

```bash
make setup
```

这将配置 git hooks，确保每次提交前自动进行代码格式检查。

### 本地验证

提交代码前，请在本地运行以下验证命令确保代码质量：

```bash
# 完整验证（推荐在提交前运行）
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features

# 分步验证
cargo fmt --all -- --check       # 代码格式检查
cargo clippy -- -D warnings      # Lint 检查
cargo test                       # 运行测试

# 格式化代码（自动修复格式问题）
cargo fmt --all

# 多平台构建验证
cargo build --release --target x86_64-apple-darwin      # macOS x64
cargo build --release --target aarch64-apple-darwin     # macOS ARM64
cargo build --release --target x86_64-unknown-linux-gnu # Linux x64
```

### 添加新协议

1. 在 `bifrost-core/src/protocol.rs` 中添加新协议枚举值
2. 实现 `from_str` 和 `to_str` 方法
3. 在 `category()` 中指定协议分类
4. 如果需要多匹配支持，添加到 `MULTI_MATCH_PROTOCOLS`

### 添加新匹配器

1. 在 `bifrost-core/src/matcher/` 下创建新文件
2. 实现 `Matcher` trait
3. 在 `factory.rs` 中添加解析逻辑

## CI/CD

项目使用 GitHub Actions 进行持续集成和自动发布。

### CI 工作流

每次 Push 到 `main` 分支或创建 Pull Request 时自动运行：

- **格式检查** - `cargo fmt --check`
- **Lint 检查** - `cargo clippy -D warnings`
- **单元测试** - 多平台测试 (Ubuntu/macOS/Windows)
- **构建验证** - 多目标构建

### 发布工作流

支持两种发布方式：

**方式一：手动触发（推荐）**

1. 进入 GitHub → Actions → Release
2. 点击 "Run workflow"
3. 选择版本类型：`patch` / `minor` / `major`
4. 可选：输入预发布标识（如 `alpha`、`beta`、`rc.1`）

版本号将自动计算（基于最新 tag 递增）。

**方式二：推送 Tag**

```bash
git tag v0.1.0
git push origin v0.1.0
```

### 构建目标

| 平台    | 架构          | Target                          | 产物格式  |
| ------- | ------------- | ------------------------------- | --------- |
| Linux   | x64           | `x86_64-unknown-linux-gnu`      | `.tar.gz` |
| Linux   | ARM64         | `aarch64-unknown-linux-gnu`     | `.tar.gz` |
| Linux   | ARMv7         | `armv7-unknown-linux-gnueabihf` | `.tar.gz` |
| macOS   | Intel         | `x86_64-apple-darwin`           | `.tar.gz` |
| macOS   | Apple Silicon | `aarch64-apple-darwin`          | `.tar.gz` |
| Windows | x64           | `x86_64-pc-windows-msvc`        | `.zip`    |
| Windows | ARM64         | `aarch64-pc-windows-msvc`       | `.zip`    |

### 发布产物

每次发布会自动生成：

- CLI 二进制文件（7 个平台）
- SHA256 校验和文件
- 自动生成的 CHANGELOG
- Homebrew Formula 自动更新

## 依赖

主要依赖：

| 依赖       | 用途            |
| ---------- | --------------- |
| `tokio`    | 异步运行时      |
| `hyper`    | HTTP 库         |
| `rustls`   | TLS 实现        |
| `rcgen`    | 证书生成        |
| `clap`     | 命令行解析      |
| `serde`    | 序列化          |
| `tracing`  | 日志追踪        |
| `regex`    | 正则表达式      |
| `rquickjs` | JavaScript 引擎 |
| `rusqlite` | SQLite 数据库   |

## License

MIT
