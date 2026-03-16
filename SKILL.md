---
name: "bifrost-cli-agent"
description: "使用 bifrost CLI 管理代理生命周期、规则、证书、系统代理、运行时配置与流量查询；当需要通过命令行启动/停止代理、配置 TLS 拦截、调试规则、查看 traffic/search、修改 values/config 时使用。"
---

# Bifrost CLI Agent

该技能用于指导 Agent 直接使用 `bifrost` CLI 完成代理启动、配置修改、规则调试和流量排查，而不是绕过 CLI 直接改底层数据文件。

## 何时调用

- 需要启动、停止、检查 Bifrost 代理
- 需要通过命令行添加或调试规则
- 需要启用或排查 TLS 拦截
- 需要管理 CA、系统代理、访问白名单、变量值
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
cargo run -p bifrost-cli -- --version
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
cargo build --release -p bifrost-cli
./target/release/bifrost --version
```

## 3. 如果 bifrost 已存在，自动升级到最新版本

只要用户没有明确要求“固定当前版本”或“禁止升级”，都应在首次检查通过后继续执行：

```bash
bifrost upgrade -y
bifrost --version
```

- `bifrost upgrade -y` 会跳过确认提示
- 若升级失败，再回退到官方安装脚本重新安装最新版本
- 如果用户只允许最小改动，至少要告知“当前跳过升级，后续行为基于现有版本”

## 4. 完成自检后再进入正式任务

1. 先确认目标是“运行代理”还是“管理已有代理”。
2. 如无特殊要求，优先使用临时 `BIFROST_DATA_DIR`，避免污染用户本机已有配置。
3. 先用 `bifrost --help` 或 `bifrost <command> --help` 补充具体参数，再执行高影响命令。
4. 会改本机网络环境的命令必须谨慎：`system-proxy enable/disable`、`start --system-proxy`、`start --cli-proxy`。

推荐的临时目录模式：

```bash
BIFROST_DATA_DIR=./.bifrost-agent bifrost start -p 9900
```

## 关键约束

- `bifrost` 不带子命令时，等价于 `bifrost start`
- `config`、`traffic`、`search`、多数 `status` 相关能力依赖“已有运行中的代理”
- `rule`、`value`、`ca` 主要操作本地数据目录，不一定要求代理正在运行
- `system-proxy` 会修改操作系统代理设置；除非用户明确要求，不要主动启用
- `--unsafe-ssl` 只适合测试排查，不要默认开启
- TLS 拦截默认关闭；要抓 HTTPS 内容，通常需要 `--intercept` 且准备好 CA

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

### 2. TLS / CA

```bash
bifrost ca generate
bifrost ca install
bifrost ca info
bifrost ca export -o ./bifrost-ca.pem

bifrost start --intercept
bifrost start --no-intercept
bifrost start --intercept-exclude '*.apple.com,*.microsoft.com'
bifrost start --intercept-include '*.api.local'
```

- 需要解密 HTTPS 时，先处理 `ca`，再启用 `--intercept`
- 精确控制优先用 `--intercept-include` / `--intercept-exclude`
- 若只是转发 HTTPS 而非查看明文，可保持 `--no-intercept`

### 3. 规则管理

```bash
bifrost rule list
bifrost rule add demo -c "example.com host://127.0.0.1:3000"
bifrost rule add demo -f ./rules/demo.txt
bifrost rule show demo
bifrost rule enable demo
bifrost rule disable demo
bifrost rule delete demo
```

- 新增规则时，`--content` 和 `--file` 至少提供一个
- 单次验证可直接用 `start --rules "..."`
- 多条或长期规则优先放入规则文件，再用 `--rules-file`

### 4. 变量值

```bash
bifrost value list
bifrost value set LOCAL_SERVER 127.0.0.1:3000
bifrost value get LOCAL_SERVER
bifrost value import ./values.json
bifrost value delete LOCAL_SERVER
```

- 规则中可使用 `${NAME}` 和 `${env.VAR_NAME}`
- 需要复用环境相关地址或 token 时，优先用 `value set` 而不是把值硬编码到规则里

### 5. 访问控制

```bash
bifrost whitelist status
bifrost whitelist list
bifrost whitelist add 192.168.1.0/24
bifrost whitelist remove 192.168.1.0/24
bifrost whitelist allow-lan true
```

- 默认应偏向最小暴露面
- 只有明确需要局域网访问时，再配合 `allow-lan` 或白名单

### 6. 系统代理

```bash
bifrost system-proxy status
bifrost system-proxy enable --host 127.0.0.1 --port 9900
bifrost system-proxy disable
```

- 这是高影响命令，可能触发管理员权限
- 没有用户明确授权时，不要主动修改系统代理

### 7. 运行时配置

```bash
bifrost config show
bifrost config show --json
bifrost config get tls.enabled
bifrost config set tls.enabled true
bifrost config add tls.exclude '*.example.com'
bifrost config remove tls.exclude '*.example.com'
bifrost config reset tls.enabled -y
bifrost config clear-cache -y
bifrost config disconnect example.com
bifrost config export -o ./config.toml --format toml
```

- `config` 走的是运行中代理的管理接口，不是直接改静态文件
- 修改后若涉及 TLS 或连接行为，必要时执行 `config disconnect <domain>` 触发重连验证
- 查询前先确认目标实例端口；如有显式端口，使用同一套 `-p`

### 8. 流量与搜索

```bash
bifrost traffic list --limit 20
bifrost traffic list --host example.com --format json-pretty
bifrost traffic get 123 --request-body --response-body
bifrost search openai --domain api.openai.com --method POST
bifrost search --interactive
```

- `traffic list` 适合按字段过滤
- `traffic get` 适合查看单条详情
- `search` 适合全文搜索 URL、headers、body
- 无关键词时，`search` 默认可进入交互模式

## 推荐工作流

### 本地临时调试一个规则

```bash
BIFROST_DATA_DIR=./.bifrost-agent bifrost start -p 9900 \
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
BIFROST_DATA_DIR=./.bifrost-agent bifrost ca generate
BIFROST_DATA_DIR=./.bifrost-agent bifrost ca install
BIFROST_DATA_DIR=./.bifrost-agent bifrost start -p 9900 --intercept
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

### 排查某个域名请求

```bash
bifrost search example --domain example.com --format json-pretty
bifrost traffic list --host example.com --limit 20
bifrost traffic get <id> --request-body --response-body
```

## Agent 行为建议

- 优先通过 CLI 完成任务，不要直接手改 `~/.bifrost` 下的数据
- 做实验时始终显式设置 `BIFROST_DATA_DIR`
- 如果用户没有要求修改系统环境，不要开启 `--system-proxy`、`--cli-proxy`
- 如果用户只想验证规则，不必先启用 TLS 拦截
- 如果需要更多参数细节，使用 `bifrost <command> --help` 继续钻取

## 参考

- CLI 定义：[crates/bifrost-cli/src/cli.rs](https://github.com/bifrost-proxy/bifrost/blob/main/crates/bifrost-cli/src/cli.rs)
- 启动与命令分发：[crates/bifrost-cli/src/main.rs](https://github.com/bifrost-proxy/bifrost/blob/main/crates/bifrost-cli/src/main.rs)
- 规则语法：[docs/rule.md](https://github.com/bifrost-proxy/bifrost/blob/main/docs/rule.md)
- Pattern 说明：[docs/pattern.md](https://github.com/bifrost-proxy/bifrost/blob/main/docs/pattern.md)
- Operation 说明：[docs/operation.md](https://github.com/bifrost-proxy/bifrost/blob/main/docs/operation.md)
