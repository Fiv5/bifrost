# Bifrost 规则测试脚本

本目录包含用于测试 Bifrost 代理规则的端到端测试脚本。

## 目录结构

```
scripts/
├── test_rules.sh      # 单个规则文件测试脚本
├── run_all_tests.sh   # 批量运行所有规则测试
├── rule.txt           # 示例规则配置文件
└── rules/             # 规则测试用例目录
    ├── host.txt       # Host 转发规则测试
    ├── redirect.txt   # 重定向规则测试
    ├── req_headers.txt
    ├── res_headers.txt
    ├── status_code.txt
    ├── method.txt
    ├── ua.txt
    ├── referer.txt
    ├── req_delay.txt
    ├── res_delay.txt
    ├── cors.txt
    ├── req_cookies.txt
    ├── res_cookies.txt
    ├── http.txt
    └── ws.txt
```

## 前置条件

- Rust 工具链 (用于编译 Bifrost)
- Python 3 (用于启动 Mock 服务器)
- curl (用于发送 HTTP 请求)
- lsof (用于检测端口占用)

## 测试单个规则文件

使用 `test_rules.sh` 脚本测试单个规则文件：

```bash
# 基本用法
./test_rules.sh <规则文件>

# 示例
./test_rules.sh rules/host.txt
./test_rules.sh rules/redirect.txt

# 指定代理端口
./test_rules.sh -p 9090 rules/host.txt

# 查看帮助
./test_rules.sh --help

# 列出所有可用的规则文件
./test_rules.sh --list
```

### 选项说明

| 选项 | 说明 |
|------|------|
| `-h, --help` | 显示帮助信息 |
| `-p, --port PORT` | 指定代理端口 (默认: 8080) |
| `-l, --list` | 列出所有可用的规则文件 |

## 批量运行测试

使用 `run_all_tests.sh` 脚本批量运行多个规则测试：

```bash
# 运行所有规则测试
./run_all_tests.sh

# 只运行指定的规则测试
./run_all_tests.sh host redirect

# 测试失败时继续执行后续测试
./run_all_tests.sh -c

# 指定代理端口并继续模式
./run_all_tests.sh -p 9090 -c

# 列出所有可用规则
./run_all_tests.sh --list
```

### 选项说明

| 选项 | 说明 |
|------|------|
| `-h, --help` | 显示帮助信息 |
| `-l, --list` | 列出所有可用的规则文件 |
| `-p, --port PORT` | 指定代理端口 (默认: 8080) |
| `-c, --continue` | 测试失败时继续执行后续测试 |

## 规则文件格式

规则文件使用简单的文本格式，每行一条规则：

```
# 这是注释
pattern protocol://target
```

### 支持的协议类型

| 协议 | 说明 | 示例 |
|------|------|------|
| `http://` | HTTP 转发 | `example.com http://127.0.0.1:3000` |
| `https://` | HTTPS 转发 | `example.com https://127.0.0.1:3000` |
| `host://` | 保持原协议转发 | `example.com host://localhost:8080` |
| `redirect://` | 302 重定向 | `old.com redirect://https://new.com` |
| `reqHeaders://` | 修改请求头 | `example.com reqHeaders://{...}` |
| `resHeaders://` | 修改响应头 | `example.com resHeaders://{...}` |
| `statusCode://` | 修改状态码 | `example.com statusCode://201` |
| `method://` | 修改请求方法 | `example.com method://POST` |
| `ua://` | 修改 User-Agent | `example.com ua://CustomUA` |
| `referer://` | 修改 Referer | `example.com referer://https://ref.com` |
| `reqDelay://` | 请求延迟 (ms) | `example.com reqDelay://500` |
| `resDelay://` | 响应延迟 (ms) | `example.com resDelay://1000` |
| `resCors://` | 添加 CORS 头 | `example.com resCors://*` |
| `reqCookies://` | 修改请求 Cookie | `example.com reqCookies://{...}` |
| `resCookies://` | 设置响应 Cookie | `example.com resCookies://{...}` |
| `ws://` | WebSocket 转发 | `example.com ws://127.0.0.1:3020/ws` |
| `wss://` | WebSocket (TLS) 转发 | `example.com wss://127.0.0.1:3020/ws` |

## 测试流程

脚本执行以下步骤：

1. **编译代理服务器** - 自动编译最新的 Bifrost 二进制文件
2. **初始化配置目录** - 创建测试所需的配置文件和目录
3. **启动代理服务器** - 使用指定的规则文件启动 Bifrost
4. **启动 Mock 服务器** - 为需要后端服务的规则启动 Mock 服务
5. **执行测试用例** - 根据规则类型自动选择合适的测试方法
6. **输出测试结果** - 显示通过、失败、跳过的测试数量

## 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `PROXY_PORT` | 代理服务器监听端口 | `8080` |
| `PROXY_HOST` | 代理服务器主机地址 | `127.0.0.1` |

## 示例

```bash
# 测试 host 转发规则
./test_rules.sh rules/host.txt

# 使用自定义端口测试重定向规则
PROXY_PORT=9090 ./test_rules.sh rules/redirect.txt

# 运行所有测试，失败时继续
./run_all_tests.sh -c

# 只测试 host 和 redirect 规则
./run_all_tests.sh host redirect
```

## 添加新的测试规则

1. 在 `rules/` 目录下创建新的 `.txt` 文件
2. 文件第一行以 `#` 开头作为规则描述
3. 添加测试规则，每行一条
4. 运行测试验证规则是否生效

示例：

```bash
# 创建新规则文件
cat > rules/my_rule.txt << 'EOF'
# My Custom Rule Test
my-test-domain.local http://127.0.0.1:3000
EOF

# 运行测试
./test_rules.sh rules/my_rule.txt
```
