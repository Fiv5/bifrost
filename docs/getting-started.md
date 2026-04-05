# 安装与启动

本文档汇总 Bifrost 的安装、启动、管理端入口与卸载方式。

## 安装 CLI

### 一键安装

```bash
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash
```

可选参数：

```bash
# 指定安装目录
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash -s -- --dir /usr/local/bin

# 安装指定版本
curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash -s -- --version v0.2.0
```

### Homebrew（macOS）

```bash
brew tap bifrost-proxy/bifrost
brew install bifrost
```

### 使用 npm 安装

```bash
npm i @bifrost-proxy/bifrost
```

### 从源码构建

环境要求：

- Rust 1.70+
- Cargo
- Node.js 22+
- pnpm

构建步骤：

```bash
git clone https://github.com/bifrost-proxy/bifrost.git
cd bifrost

./install.sh

# 或手动构建
cd web && pnpm install && pnpm build && cd ..
cargo build --release
```

### 手动下载

可直接从 [Releases](https://github.com/bifrost-proxy/bifrost/releases) 下载预编译二进制。

## 检查安装

```bash
command -v bifrost
bifrost --version
```

如果尚未加入 `PATH`，也可以在源码仓库中执行：

```bash
cargo run -p bifrost-cli -- --version
```

## 启动代理

```bash
# 默认监听 0.0.0.0:9900
bifrost start

# 自定义端口和监听地址
bifrost -p 9000 -H 127.0.0.1 start

# 启用 HTTP + SOCKS5
bifrost -p 9900 --socks5-port 1080 start

# 启用 TLS 拦截
bifrost start --intercept

# 守护进程模式
bifrost start --daemon
```

## 管理端入口

服务启动后，在浏览器访问：

```text
http://127.0.0.1:<port>/_bifrost/
```

默认端口示例：

```text
http://127.0.0.1:9900/_bifrost/
```

常用入口：

| 路径 | 说明 |
| --- | --- |
| `/_bifrost/` | Web UI |
| `/_bifrost/api/rules/*` | 规则管理 API |
| `/_bifrost/api/values/*` | Values API |
| `/_bifrost/api/traffic/*` | 流量 API |
| `/_bifrost/api/scripts/*` | Scripts API |
| `/_bifrost/api/replay/*` | 请求重放 API |

说明：

- 管理端默认仅允许通过 `127.0.0.1` 或 `localhost` 访问
- SSE 增量订阅接口为 `/_bifrost/api/traffic/{id}/sse/stream?from=begin`

## 环境变量

| 环境变量 | 说明 | 默认值 |
| --- | --- | --- |
| `BIFROST_DATA_DIR` | 数据目录路径 | `~/.bifrost` |
| `RUST_LOG` | 日志级别和过滤器 | `info` |
| `WEB_PORT` | Web UI 开发服务端口 | `3000` |

示例：

```bash
BIFROST_DATA_DIR=/tmp/bifrost bifrost start
RUST_LOG=debug bifrost start
RUST_LOG=bifrost_proxy=debug,info bifrost start
```

## 卸载

```bash
# 卸载 CLI 和桌面应用
./uninstall.sh

# 连同数据一起清理
./uninstall.sh --purge
```
