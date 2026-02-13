# Bifrost E2E 测试框架文档

Bifrost E2E 是 Whistle 代理服务器的端到端测试框架，用于验证各种代理规则的正确性。

## 目录结构

```
docs/
├── README.md                    # 本文档
├── rules/                       # 规则使用手册
│   ├── README.md               # 规则总览与分类
│   ├── routing.md              # 路由与转发规则
│   ├── request-modification.md # 请求修改规则
│   ├── response-modification.md# 响应修改规则
│   ├── url-manipulation.md     # URL 操作规则
│   ├── body-manipulation.md    # Body 操作规则 (含 HTML/CSS/JS 注入)
│   ├── status-redirect.md      # 状态码与重定向规则
│   ├── timing-throttle.md      # 延迟与限速规则
│   ├── filters.md              # 过滤器规则
│   ├── patterns.md             # 匹配模式详解
│   ├── scripts.md              # 脚本规则 (reqScript/resScript)
│   └── websocket.md            # WebSocket 规则 (ws/wss)
└── testing/
    └── test-cases.md           # 测试用例清单
```

## 快速开始

### 运行所有测试

```bash
cd rust/crates/bifrost-e2e
cargo run
```

### 按类别运行测试

```bash
# 运行路由相关测试
cargo run -- --category routing

# 运行协议相关测试
cargo run -- --category protocols
```

### 运行特定测试

```bash
cargo run -- --test host_redirect
```

### 列出所有测试

```bash
cargo run -- --list
```

### 生成测试报告

```bash
cargo run -- --output report.json
```

## 测试框架架构

```
                    ┌──────────────┐
                    │   TestCase   │
                    │   Runner     │
                    └──────┬───────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
    ┌──────▼──────┐ ┌──────▼──────┐ ┌──────▼──────┐
    │   Proxy     │ │    Mock     │ │    Curl     │
    │  Instance   │ │   Server    │ │   Command   │
    └─────────────┘ └─────────────┘ └─────────────┘
```

### 核心组件

| 组件 | 说明 |
|------|------|
| `TestCase` | 测试用例定义，包含名称、分类、规则和测试函数 |
| `ProxyInstance` | 代理服务实例，支持自定义规则配置 |
| `EnhancedMockServer` | Mock 服务器，用于模拟后端响应 |
| `CurlCommand` | Curl 命令封装，用于发送 HTTP 请求 |
| `ProxyClient` | 代理客户端，通过代理发送请求 |

### 测试模式

#### 1. 标准模式
适用于简单的规则测试，框架自动管理代理实例：

```rust
TestCase::new(
    "test_name",
    "category",
    vec!["pattern host://target"],
    |client: ProxyClient| async move {
        let resp = client.get("http://example.com").await?;
        assert_status_ok(&resp)?;
        Ok(())
    },
)
```

#### 2. 独立模式
适用于复杂场景，完全自主控制测试流程：

```rust
TestCase::standalone(
    "test_name",
    "description",
    "category",
    async || {
        let mock = EnhancedMockServer::start().await;
        let proxy = ProxyInstance::start(port, rules).await?;
        
        let result = CurlCommand::with_proxy(proxy_url, target_url)
            .execute()
            .await?;
        
        result.assert_success()?;
        Ok(())
    },
)
```

## 规则文档

详细的规则使用说明请参阅 [rules/README.md](./rules/README.md)。

## 测试用例

完整的测试用例清单请参阅 [testing/test-cases.md](./testing/test-cases.md)。
