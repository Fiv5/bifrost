# Bifrost 文件协议 (.bifrost) 技术方案

## 1. 背景与目标

### 1.1 需求概述

设计一个统一的文件格式 `.bifrost`，用于 Bifrost 项目中各类数据的存储和导入导出：

1. **规则 (rules)** - 代理规则，支持启用状态和排序
2. **网络请求 (network)** - 流量记录详情，支持批量导出
3. **脚本 (script)** - 请求/响应脚本，支持批量导出
4. **变量 (values)** - 环境变量键值对
5. **请求模板 (template)** - Replay 请求集合

### 1.2 设计目标

| 目标         | 说明                                          |
| ------------ | --------------------------------------------- |
| **统一格式** | 一种协议支持所有数据类型                      |
| **人可读**   | 文本格式，便于查看和编辑                      |
| **扩展性强** | 版本号机制支持协议演进                        |
| **批量支持** | 同一文件可包含多条同类型数据                  |
| **存储格式** | 替代现有 JSON 存储，规则文件直接使用 .bifrost |

### 1.3 关键设计决策

| 决策项     | 方案       | 理由                           |
| ---------- | ---------- | ------------------------------ |
| 文件扩展名 | `.bifrost` | 项目专有格式，易识别           |
| 元信息格式 | TOML       | 简洁、可读、Rust 生态支持好    |
| 正文格式   | 类型相关   | rules 用原始文本，其他用 JSON  |
| 分隔符     | `---`      | 清晰分隔元信息和正文           |
| 存储位置   | 数据目录   | 统一使用 `$BIFROST_DATA_DIR`   |
| 容错解析   | 渐进式降级 | 部分信息丢失时仍可恢复核心数据 |

---

## 2. 文件格式定义

### 2.1 文件基本结构

```
VV TYPE

[meta]
元信息键值对

[options]
类型特定选项

---
正文内容
```

### 2.2 文件头（第一行）

格式：`VV TYPE`

| 字段 | 描述                     | 取值                                               |
| ---- | ------------------------ | -------------------------------------------------- |
| VV   | 两位数版本号，前导零填充 | `01`                                               |
| TYPE | 类型标识符               | `rules`, `network`, `script`, `values`, `template` |

**示例**:

```
01 rules
01 network
01 script
01 values
01 template
```

### 2.3 元信息区域（TOML 格式）

#### 2.3.1 通用 meta 字段

```toml
[meta]
name = "规则/数据名称"
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
updated_at = "2026-03-02T10:00:00Z"
description = "可选描述"
```

#### 2.3.2 rules 专用字段

```toml
[meta]
name = "api-rules"
enabled = true
sort_order = 0
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
updated_at = "2026-03-02T10:00:00Z"
description = "API 代理规则"

[options]
rule_count = 15
```

### 2.4 内容分隔符

使用 `---` 独占一行作为元信息和正文内容的分隔符。

### 2.5 正文内容区域

根据不同类型采用不同格式：

| 类型     | 正文格式     |
| -------- | ------------ |
| rules    | 原始规则文本 |
| network  | JSON Array   |
| script   | JSON Array   |
| values   | JSON Object  |
| template | JSON Object  |

---

## 3. 各类型详细设计

### 3.1 Rules（规则）- 存储格式

**类型标识**: `rules`

**用途**: 替代现有 JSON 格式，作为规则文件的标准存储格式

**存储路径**: `$BIFROST_DATA_DIR/rules/{name}.bifrost`

**完整示例**:

```
01 rules

[meta]
name = "api-rules"
enabled = true
sort_order = 0
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
updated_at = "2026-03-02T15:30:00Z"
description = "API 接口代理规则"

[options]
rule_count = 5

---
# API 代理规则
# 将 API 请求转发到本地开发服务器

^api\.example\.com$ proxy://localhost:3000
^api\.example\.com/v2/(.*)$ proxy://localhost:3001/$1

# Mock 响应
^api\.example\.com/users$ file://./mock/users.json
^api\.example\.com/products$ resBody://{\"products\":[]}

# 添加调试头
*.example.com resHeaders://x-debug=true
```

**meta 字段说明**:

| 字段        | 类型   | 必填 | 说明                     |
| ----------- | ------ | ---- | ------------------------ |
| name        | string | ✅   | 规则名称，用于显示和引用 |
| enabled     | bool   | ✅   | 启用状态                 |
| sort_order  | i32    | ✅   | 排序顺序，数值越小越靠前 |
| version     | string | ❌   | 版本号                   |
| created_at  | string | ✅   | 创建时间 (ISO 8601)      |
| updated_at  | string | ✅   | 更新时间 (ISO 8601)      |
| description | string | ❌   | 描述信息                 |

---

### 3.2 Network（网络请求）- 导出格式

**类型标识**: `network`

**用途**: 导出流量记录，便于分享或导入重放

**正文格式**: JSON Array，每个元素为精简的 TrafficRecord

**完整示例**:

```
01 network

[meta]
name = "api-traffic-export"
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
description = "API 请求样本"

[options]
count = 2
include_body = true
include_response = true

---
[
  {
    "id": "traffic-001",
    "method": "GET",
    "url": "https://api.example.com/users",
    "status": 200,
    "request_headers": [
      ["Accept", "application/json"],
      ["User-Agent", "Mozilla/5.0"]
    ],
    "response_headers": [
      ["Content-Type", "application/json"],
      ["X-Request-Id", "abc123"]
    ],
    "request_body": null,
    "response_body": "{\"users\":[{\"id\":1,\"name\":\"test\"}]}",
    "duration_ms": 150,
    "timestamp": 1709366400000,
    "matched_rules": [
      {
        "pattern": "api.example.com",
        "protocol": "proxy",
        "value": "localhost:3000"
      }
    ]
  },
  {
    "id": "traffic-002",
    "method": "POST",
    "url": "https://api.example.com/users",
    "status": 201,
    "request_headers": [
      ["Content-Type", "application/json"]
    ],
    "request_body": "{\"name\":\"newuser\"}",
    "response_body": "{\"id\":2,\"name\":\"newuser\"}",
    "duration_ms": 200,
    "timestamp": 1709366401000
  }
]
```

**NetworkRecord 字段**:

| 字段             | 类型               | 必填 | 说明       |
| ---------------- | ------------------ | ---- | ---------- |
| id               | string             | ✅   | 记录 ID    |
| method           | string             | ✅   | HTTP 方法  |
| url              | string             | ✅   | 完整 URL   |
| status           | u16                | ✅   | 响应状态码 |
| request_headers  | [[string, string]] | ❌   | 请求头     |
| response_headers | [[string, string]] | ❌   | 响应头     |
| request_body     | string             | ❌   | 请求体     |
| response_body    | string             | ❌   | 响应体     |

|
 duration_ms | u64 | ✅ | 请求耗时 |
| timestamp | u64 | ✅ | 时间戳 |
| matched_rules | [MatchedRule] | ❌ | 匹配的规则 |

---

### 3.3 Script（脚本）- 导出格式

**类型标识**: `script`

**用途**: 导出脚本，支持批量导入导出

**正文格式**: JSON Array

**完整示例**:
```
01 script

[meta]
name = "script-export"
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
description = "常用脚本集合"

[options]
count = 2

---
[
  {
    "name": "add-auth-header",
    "script_type": "request",
    "description": "添加认证头",
    "content": "const token = ctx.values.API_TOKEN;\nif (token) {\n  request.headers['Authorization'] = 'Bearer ' + token;\n  log.info('Added auth header');\n}"
  },
  {
    "name": "transform-response",
    "script_type": "response",
    "description": "转换响应格式",
    "content": "if (response.headers['Content-Type']?.includes('json')) {\n  const data = JSON.parse(response.body);\n  data._meta = { processedAt: Date.now() };\n  response.body = JSON.stringify(data);\n}"
  }
]
```

**ScriptItem 字段**:

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | ✅ | 脚本名称 |
| script_type | string | ✅ | 类型: `request` / `response` |
| description | string | ❌ | 描述 |
| content | string | ✅ | 脚本内容 |

---

### 3.4 Values（变量）- 导出格式

**类型标识**: `values`

**用途**: 导出环境变量，便于不同环境切换

**正文格式**: JSON Object（键值对）

**完整示例**:
```
01 values

[meta]
name = "production-env"
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
description = "生产环境变量"

[options]
count = 5

---
{
  "API_BASE_URL": "https://api.example.com",
  "AUTH_TOKEN": "your-secret-token",
  "DEBUG_MODE": "false",
  "TIMEOUT_MS": "30000",
  "MAX_RETRIES": "3"
}
```

---

### 3.5 Template（请求模板）- 导出格式

**类型标识**: `template`

**用途**: 导出 Replay 请求集合，便于分享 API 测试用例

**正文格式**: JSON Object，包含 groups 和 requests

**完整示例**:
```
01 template

[meta]
name = "user-api-collection"
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
description = "用户 API 测试集合"

[options]
request_count = 3
group_count = 1

---
{
  "groups": [
    {
      "id": "group-001",
      "name": "User API",
      "parent_id": null,
      "sort_order": 0,
      "created_at": 1709366400000,
      "updated_at": 1709366400000
    }
  ],
  "requests": [
    {
      "id": "req-001",
      "group_id": "group-001",
      "name": "获取用户列表",
      "request_type": "http",
      "method": "GET",
      "url": "https://api.example.com/users",
      "headers": [
        {
          "id": "h1",
          "key": "Accept",
          "value": "application/json",
          "enabled": true
        }
      ],
      "body": null,
      "is_saved": true,
      "sort_order": 0,
      "created_at": 1709366400000,
      "updated_at": 1709366400000
    },
    {
      "id": "req-002",
      "group_id": "group-001",
      "name": "创建用户",
      "request_type": "http",
      "method": "POST",
      "url": "https://api.example.com/users",
      "headers": [
        {
          "id": "h1",
          "key": "Content-Type",
          "value": "application/json",
          "enabled": true
        }
      ],
      "body": {
        "type": "raw",
        "raw_type": "json",
        "content": "{\"name\": \"newuser\", \"email\": \"user@example.com\"}"
      },
      "is_saved": true,
      "sort_order": 1,
      "created_at": 1709366400000,
      "updated_at": 1709366400000
    },
    {
      "id": "req-003",
      "group_id": "group-001",
      "name": "SSE 订阅",
      "request_type": "sse",
      "method": "GET",
      "url": "https://api.example.com/events",
      "headers": [],
      "body": null,
      "is_saved": true,
      "sort_order": 2,
      "created_at": 1709366400000,
      "updated_at": 1709366400000
    }
  ]
}
```

---

## 4. 数据结构定义（Rust）

### 4.1 模块结构

```
crates/bifrost-core/src/
└── bifrost_file/
    ├── mod.rs          # 模块入口，re-export
    ├── types.rs        # 核心类型定义
    ├── parser.rs       # 文件解析器
    ├── writer.rs       # 文件生成器
    └── rules.rs        # rules 类型特化处理
```

### 4.2 核心类型

```rust
// crates/bifrost-core/src/bifrost_file/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const BIFROST_FILE_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BifrostFileType {
    Rules,
    Network,
    Script,
    Values,
    Template,
}

impl std::fmt::Display for BifrostFileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BifrostFileType::Rules => write!(f, "rules"),
            BifrostFileType::Network => write!(f, "network"),
            BifrostFileType::Script => write!(f, "script"),
            BifrostFileType::Values => write!(f, "values"),
            BifrostFileType::Template => write!(f, "template"),
        }
    }
}

impl std::str::FromStr for BifrostFileType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rules" => Ok(BifrostFileType::Rules),
            "network" => Ok(BifrostFileType::Network),
            "script" => Ok(BifrostFileType::Script),
            "values" => Ok(BifrostFileType::Values),
            "template" => Ok(BifrostFileType::Template),
            _ => Err(format!("Unknown file type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BifrostFileHeader {
    pub version: u8,
    pub file_type: BifrostFileType,
}

#[derive(Debug, Clone)]
pub struct BifrostFile<M, T> {
    pub header: BifrostFileHeader,
    pub meta: M,
    pub options: toml::Value,
    pub content: T,
}

pub struct BifrostFileRaw {
    pub header: BifrostFileHeader,
    pub meta_raw: String,
    pub content_raw: String,
}
```

### 4.3 Rules 专用类型

```rust
// crates/bifrost-core/src/bifrost_file/rules.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFileMeta {
    pub name: String,
    pub enabled: bool,
    pub sort_order: i32,
    #[serde(default = "default_version")]
    pub version: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleFileOptions {
    #[serde(default)]
    pub rule_count: usize,
}

pub type RuleFile = BifrostFile<RuleFileMeta, String>;

impl RuleFileMeta {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name,
            enabled: true,
            sort_order: 0,
            version: "1.0.0".to_string(),
            created_at: now.clone(),
            updated_at: now,
            description: None,
        }
    }
}
```

### 4.4 其他类型定义

```rust
// crates/bifrost-core/src/bifrost_file/types.rs (续)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMeta {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRecord {
    pub id: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_headers: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    pub duration_ms: u64,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_rules: Option<Vec<MatchedRuleExport>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedRuleExport {
    pub pattern: String,
    pub protocol: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptItem {
    pub name: String,
    pub script_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub content: String,
}

pub type ValuesContent = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateContent {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ReplayGroupExport>,
    pub requests: Vec<ReplayRequestExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayGroupExport {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub sort_order: i32,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRequestExport {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub request_type: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValueItemExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<ReplayBodyExport>,
    pub is_saved: bool,
    pub sort_order: i32,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValueItemExport {
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBodyExport {
    #[serde(rename = "type")]
    pub body_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_data: Vec<KeyValueItemExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_file: Option<String>,
}

fn default_true() -> bool {
    true
}
```

---

## 5. 解析器实现

### 5.1 核心解析流程

```
┌─────────────────┐
│   读取文件内容   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  解析文件头     │  → 提取版本号和类型
│  (第一行)       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  跳过空行       │
│  (第二行)       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  查找分隔符     │  → 定位 "---"
│  分割元信息/正文 │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  解析元信息     │  → TOML 解析
│  (meta + options)│
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  解析正文内容   │  → 根据类型选择解析器
│  (类型相关)     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  构建返回结构   │
└─────────────────┘
```

### 5.2 解析器代码

```rust
// crates/bifrost-core/src/bifrost_file/parser.rs

use super::*;
use anyhow::{anyhow, Context, Result};

pub struct BifrostFileParser;

impl BifrostFileParser {
    pub fn parse_header(first_line: &str) -> Result<BifrostFileHeader> {
        let trimmed = first_line.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid header format. Expected 'VV TYPE', got: '{}'",
                trimmed
            ));
        }

        let version: u8 = parts[0]
            .parse()
            .with_context(|| format!("Invalid version number: '{}'", parts[0]))?;

        let file_type: BifrostFileType = parts[1]
            .parse()
            .map_err(|e| anyhow!("Invalid file type: {}", e))?;

        Ok(BifrostFileHeader { version, file_type })
    }

    pub fn parse_raw(content: &str) -> Result<BifrostFileRaw> {
        let mut lines = content.lines();

        let header_line = lines
            .next()
            .ok_or_else(|| anyhow!("Empty file"))?;
        let header = Self::parse_header(header_line)?;

        lines.next();

        let remaining: String = lines.collect::<Vec<&str>>().join("\n");

        let separator = "\n---\n";
        let (meta_raw, content_raw) = if let Some(pos) = remaining.find(separator) {
            let (meta, content) = remaining.split_at(pos);
            (meta.to_string(), content[separator.len()..].to_string())
        } else if remaining.ends_with("\n---") {
            (remaining[..remaining.len() - 4].to_string(), String::new())
        } else {
            return Err(anyhow!("Missing content separator '---'"));
        };

        Ok(BifrostFileRaw {
            header,
            meta_raw,
            content_raw,
        })
    }

    pub fn parse_rules(content: &str) -> Result<RuleFile> {
        let raw = Self::parse_raw(content)?;

        if raw.header.file_type != BifrostFileType::Rules {
            return Err(anyhow!(
                "Expected rules type, got {:?}",
                raw.header.file_type
            ));
        }

        #[derive(Deserialize)]
        struct MetaWrapper {
            meta: RuleFileMeta,
            #[serde(default)]
            options: toml::Value,
        }

        let parsed: MetaWrapper = toml::from_str(&raw.meta_raw)
            .with_context(|| "Failed to parse meta section")?;

        Ok(BifrostFile {
            header: raw.header,
            meta: parsed.meta,
            options: parsed.options,
            content: raw.content_raw,
        })
    }

    pub fn parse_network(content: &str) -> Result<BifrostFile<ExportMeta, Vec<NetworkRecord>>> {
        let raw = Self::parse_raw(content)?;

        if raw.header.file_type != BifrostFileType::Network {
            return Err(anyhow!(
                "Expected network type, got {:?}",
                raw.header.file_type
            ));
        }

        #[derive(Deserialize)]
        struct MetaWrapper {
            meta: ExportMeta,
            #[serde(default)]
            options: toml::Value,
        }

        let parsed: MetaWrapper = toml::from_str(&raw.meta_raw)
            .with_context(|| "Failed to parse meta section")?;

        let records: Vec<NetworkRecord> = serde_json::from_str(&raw.content_raw)
            .with_context(|| "Failed to parse network content as JSON")?;

        Ok(BifrostFile {
            header: raw.header,
            meta: parsed.meta,
            options: parsed.options,
            content: records,
        })
    }

    pub fn parse_script(content: &str) -> Result<BifrostFile<ExportMeta, Vec<ScriptItem>>> {
        let raw = Self::parse_raw(content)?;

        if raw.header.file_type != BifrostFileType::Script {
            return Err(anyhow!(
                "Expected script type, got {:?}",
                raw.header.file_type
            ));
        }

        #[derive(Deserialize)]
        struct MetaWrapper {
            meta: ExportMeta,
            #[serde(default)]
            options: toml::Value,
        }

        let parsed: MetaWrapper = toml::from_str(&raw.meta_raw)
            .with_context(|| "Failed to parse meta section")?;

        let scripts: Vec<ScriptItem> = serde_json::from_str(&raw.content_raw)
            .with_context(|| "Failed to parse script content as JSON")?;

        Ok(BifrostFile {
            header: raw.header,
            meta: parsed.meta,
            options: parsed.options,
            content: scripts,
        })
    }

    pub fn parse_values(content: &str) -> Result<BifrostFile<ExportMeta, ValuesContent>> {
        let raw = Self::parse_raw(content)?;

        if raw.header.file_type != BifrostFileType::Values {
            return Err(anyhow!(
                "Expected values type, got {:?}",
                raw.header.file_type
            ));
        }

        #[derive(Deserialize)]
        struct MetaWrapper {
            meta: ExportMeta,
            #[serde(default)]
            options: toml::Value,
        }

        let parsed: MetaWrapper = toml::from_str(&raw.meta_raw)
            .with_context(|| "Failed to parse meta section")?;

        let values: ValuesContent = serde_json::from_str(&raw.content_raw)
            .with_context(|| "Failed to parse values content as JSON")?;

        Ok(BifrostFile {
            header: raw.header,
            meta: parsed.meta,
            options: parsed.options,
            content: values,
        })
    }

    pub fn parse_template(content: &str) -> Result<BifrostFile<ExportMeta, TemplateContent>> {
        let raw = Self::parse_raw(content)?;

        if raw.header.file_type != BifrostFileType::Template {
            return Err(anyhow!(
                "Expected template type, got {:?}",
                raw.header.file_type
            ));
        }

        #[derive(Deserialize)]
        struct MetaWrapper {
            meta: ExportMeta,
            #[serde(default)]
            options: toml::Value,
        }

        let parsed: MetaWrapper = toml::from_str(&raw.meta_raw)
            .with_context(|| "Failed to parse meta section")?;

        let template: TemplateContent = serde_json::from_str(&raw.content_raw)
            .with_context(|| "Failed to parse template content as JSON")?;

        Ok(BifrostFile {
            header: raw.header,
            meta: parsed.meta,
            options: parsed.options,
            content: template,
        })
    }

    pub fn detect_type(content: &str) -> Result<BifrostFileType> {
        let first_line = content.lines().next().ok_or_else(|| anyhow!("Empty file"))?;
        let header = Self::parse_header(first_line)?;
        Ok(header.file_type)
    }
}
```

---

## 6. 生成器实现

```rust
// crates/bifrost-core/src/bifrost_file/writer.rs

use super::*;
use anyhow::Result;
use chrono::Utc;

pub struct BifrostFileWriter;

impl BifrostFileWriter {
    fn write_header(file_type: BifrostFileType) -> String {
        format!("{:02} {}", BIFROST_FILE_VERSION, file_type)
    }

    pub fn write_rules(meta: &RuleFileMeta, rules_content: &str) -> Result<String> {
        let rule_count = rules_content
            .lines()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .count();

        let mut output = Self::write_header(BifrostFileType::Rules);
        output.push_str("\n\n");

        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(&meta.name)));
        output.push_str(&format!("enabled = {}\n", meta.enabled));
        output.push_str(&format!("sort_order = {}\n", meta.sort_order));
        output.push_str(&format!("version = \"{}\"\n", meta.version));
        output.push_str(&format!("created_at = \"{}\"\n", meta.created_at));
        output.push_str(&format!("updated_at = \"{}\"\n", meta.updated_at));
        if let Some(ref desc) = meta.description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("rule_count = {}\n", rule_count));

        output.push_str("\n---\n");
        output.push_str(rules_content);

        Ok(output)
    }

    pub fn write_network(
        name: &str,
        description: Option<&str>,
        records: &[NetworkRecord],
    ) -> Result<String> {
        let mut output = Self::write_header(BifrostFileType::Network);
        output.push_str("\n\n");

        let now = Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("count = {}\n", records.len()));
        output.push_str("include_body = true\n");
        output.push_str("include_response = true\n");

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(records)?);

        Ok(output)
    }

    pub fn write_script(
        name: &str,
        description: Option<&str>,
        scripts: &[ScriptItem],
    ) -> Result<String> {
        let mut output = Self::write_header(BifrostFileType::Script);
        output.push_str("\n\n");

        let now = Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("count = {}\n", scripts.len()));

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(scripts)?);

        Ok(output)
    }

    pub fn write_values(
        name: &str,
        description: Option<&str>,
        values: &ValuesContent,
    ) -> Result<String> {
        let mut output = Self::write_header(BifrostFileType::Values);
        output.push_str("\n\n");

        let now = Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("count = {}\n", values.len()));

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(values)?);

        Ok(output)
    }

    pub fn write_template(
        name: &str,
        description: Option<&str>,
        template: &TemplateContent,
    ) -> Result<String> {
        let mut output = Self::write_header(BifrostFileType::Template);
        output.push_str("\n\n");

        let now = Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("request_count = {}\n", template.requests.len()));
        output.push_str(&format!("group_count = {}\n", template.groups.len()));

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(template)?);

        Ok(output)
    }
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
```

---

## 7. 容错解析机制

为保证文件在部分损坏或信息丢失时仍能恢复核心数据，协议设计了多层容错机制。

### 7.1 容错设计原则

| 原则 | 说明 |
|------|------|
| **渐进式降级** | 解析失败时逐级降级，尽可能恢复有效数据 |
| **默认值填充** | 缺失的元信息字段使用合理默认值 |
| **宽松解析** | 对格式不完全规范的文件也能尽量解析 |
| **错误报告** | 返回解析警告，让用户知道哪些数据可能有问题 |
| **内容优先** | 正文内容是核心，元信息丢失不影响主体数据恢复 |

### 7.2 容错场景与处理策略

#### 7.2.1 文件头损坏或缺失

| 场景 | 处理策略 |
|------|----------|
| 第一行为空 | 尝试从后续内容推断类型 |
| 版本号缺失 | 默认使用版本 `01` |
| 版本号格式错误 | 尝试提取数字，失败则默认 `01` |
| 类型标识缺失 | 从文件扩展名或内容特征推断 |
| 类型标识无法识别 | 尝试解析为 `rules` 类型（最常用） |

**推断类型的启发式规则**:
```rust
fn infer_type_from_content(content: &str) -> BifrostFileType {
    let trimmed = content.trim();
    
    // JSON Array 开头 → network/script/template
    if trimmed.starts_with('[') {
        // 检查内容特征
        if trimmed.contains("\"script_type\"") {
            return BifrostFileType::Script;
        }
        if trimmed.contains("\"request_headers\"") || trimmed.contains("\"response_body\"") {
            return BifrostFileType::Network;
        }
        return BifrostFileType::Template;
    }
    
    // JSON Object 开头 → values
    if trimmed.starts_with('{') && !trimmed.contains("\"requests\"") {
        return BifrostFileType::Values;
    }
    
    // 默认为 rules
    BifrostFileType::Rules
}
```

#### 7.2.2 元信息缺失或损坏

| 场景 | 处理策略 |
|------|----------|
| `[meta]` 区块完全缺失 | 使用默认 meta，从文件名提取 name |
| 部分字段缺失 | 使用字段默认值 |
| TOML 解析失败 | 跳过 meta，使用默认值，继续解析正文 |
| 时间格式错误 | 使用当前时间作为默认值 |

**Rules 类型默认值**:
```rust
impl Default for RuleFileMeta {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name: "unnamed".to_string(),
            enabled: true,           // 默认启用
            sort_order: 0,           // 默认排序最前
            version: "1.0.0".to_string(),
            created_at: now.clone(),
            updated_at: now,
            description: None,
        }
    }
}
```

**导出类型默认值**:
```rust
impl Default for ExportMeta {
    fn default() -> Self {
        Self {
            name: "unnamed".to_string(),
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description: None,
        }
    }
}
```

#### 7.2.3 分隔符缺失

| 场景 | 处理策略 |
|------|----------|
| `---` 完全缺失 | 尝试以下策略依次尝试 |
| 策略 1 | 查找第一个 `[` 或 `{` 作为 JSON 内容开始 |
| 策略 2 | 查找第一个非 TOML 格式的行作为规则内容开始 |
| 策略 3 | 将整个内容视为正文（无元信息） |

```rust
fn find_content_start(content: &str) -> Option<usize> {
    // 优先查找标准分隔符
    if let Some(pos) = content.find("\n---\n") {
        return Some(pos + 5);
    }
    if let Some(pos) = content.find("\n---") {
        return Some(pos + 4);
    }
    
    // 查找 JSON 开始标记
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            return Some(content.lines().take(i).map(|l| l.len() + 1).sum());
        }
    }
    
    // 查找非 TOML 内容（规则行）
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.is_empty() 
            && !trimmed.starts_with('[')
            && !trimmed.starts_with('#')
            && !trimmed.contains('=')
            && trimmed.contains(' ')  // 规则通常包含空格
        {
            return Some(content.lines().take(i).map(|l| l.len() + 1).sum());
        }
    }
    
    None
}
```

#### 7.2.4 正文内容损坏

| 场景 | 处理策略 |
|------|----------|
| JSON 解析失败 | 尝试修复常见错误后重试 |
| JSON 数组部分损坏 | 尝试解析有效的数组元素 |
| 规则内容为空 | 返回空规则集，不报错 |

**JSON 修复策略**:
```rust
fn try_repair_json(json_str: &str) -> Option<String> {
    let mut repaired = json_str.to_string();
    
    // 1. 移除尾部逗号
    repaired = repaired.trim_end().to_string();
    if repaired.ends_with(",]") {
        repaired = repaired[..repaired.len()-2].to_string() + "]";
    }
    if repaired.ends_with(",}") {
        repaired = repaired[..repaired.len()-2].to_string() + "}";
    }
    
    // 2. 补全未闭合的括号
    let open_brackets = repaired.matches('[').count();
    let close_brackets = repaired.matches(']').count();
    if open_brackets > close_brackets {
        repaired.push_str(&"]".repeat(open_brackets - close_brackets));
    }
    
    let open_braces = repaired.matches('{').count();
    let close_braces = repaired.matches('}').count();
    if open_braces > close_braces {
        repaired.push_str(&"}".repeat(open_braces - close_braces));
    }
    
    // 3. 验证修复结果
    if serde_json::from_str::<serde_json::Value>(&repaired).is_ok() {
        Some(repaired)
    } else {
        None
    }
}
```

**部分数组恢复**:
```rust
fn parse_partial_json_array<T: DeserializeOwned>(json_str: &str) -> (Vec<T>, Vec<String>) {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    
    // 尝试完整解析
    if let Ok(parsed) = serde_json::from_str::<Vec<T>>(json_str) {
        return (parsed, errors);
    }
    
    // 尝试逐个元素解析
    // 简化处理：按 },{ 分割尝试
    let content = json_str.trim();
    if content.starts_with('[') && content.ends_with(']') {
        let inner = &content[1..content.len()-1];
        
        // 使用状态机分割 JSON 对象
        let objects = split_json_objects(inner);
        
        for (i, obj_str) in objects.iter().enumerate() {
            match serde_json::from_str::<T>(obj_str) {
                Ok(item) => items.push(item),
                Err(e) => errors.push(format!("Item {}: {}", i, e)),
            }
        }
    }
    
    (items, errors)
}
```

### 7.3 解析结果类型

定义带警告信息的解析结果，让调用方知道解析过程中发生了什么：

```rust
#[derive(Debug, Clone)]
pub struct ParseResult<T> {
    pub data: T,
    pub warnings: Vec<ParseWarning>,
}

#[derive(Debug, Clone)]
pub struct ParseWarning {
    pub level: WarningLevel,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningLevel {
    Info,    // 信息性提示
    Warning, // 警告，数据可能不完整
    Error,   // 严重问题，部分数据丢失
}

impl<T> ParseResult<T> {
    pub fn ok(data: T) -> Self {
        Self { data, warnings: vec![] }
    }
    
    pub fn with_warning(data: T, warning: ParseWarning) -> Self {
        Self { data, warnings: vec![warning] }
    }
    
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
    
    pub fn has_errors(&self) -> bool {
        self.warnings.iter().any(|w| w.level == WarningLevel::Error)
    }
}
```

### 7.4 容错解析器实现

```rust
impl BifrostFileParser {
    /// 容错解析 - 尽可能恢复数据
    pub fn parse_tolerant(content: &str) -> ParseResult<BifrostFileRaw> {
        let mut warnings = Vec::new();
        
        // 1. 解析文件头（容错）
        let (header, header_warnings) = Self::parse_header_tolerant(content);
        warnings.extend(header_warnings);
        
        // 2. 查找内容分隔点（容错）
        let (meta_raw, content_raw, sep_warnings) = 
            Self::split_meta_content_tolerant(content, &header);
        warnings.extend(sep_warnings);
        
        ParseResult {
            data: BifrostFileRaw {
                header,
                meta_raw,
                content_raw,
            },
            warnings,
        }
    }
    
    fn parse_header_tolerant(content: &str) -> (BifrostFileHeader, Vec<ParseWarning>) {
        let mut warnings = Vec::new();
        let first_line = content.lines().next().unwrap_or("");
        
        // 尝试标准解析
        if let Ok(header) = Self::parse_header(first_line) {
            return (header, warnings);
        }
        
        // 容错解析
        let parts: Vec<&str> = first_line.trim().split_whitespace().collect();
        
        // 尝试提取版本号
        let version = parts.get(0)
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or_else(|| {
                warnings.push(ParseWarning {
                    level: WarningLevel::Warning,
                    message: "Version number missing or invalid, using default: 01".into(),
                    field: Some("version".into()),
                });
                1
            });
        
        // 尝试提取类型
        let file_type = parts.get(1)
            .and_then(|s| s.parse::<BifrostFileType>().ok())
            .unwrap_or_else(|| {
                let inferred = Self::infer_type_from_content(content);
                warnings.push(ParseWarning {
                    level: WarningLevel::Warning,
                    message: format!(
                        "File type missing or invalid, inferred as: {}",
                        inferred
                    ),
                    field: Some("file_type".into()),
                });
                inferred
            });
        
        (BifrostFileHeader { version, file_type }, warnings)
    }
    
    fn split_meta_content_tolerant(
        content: &str,
        header: &BifrostFileHeader,
    ) -> (String, String, Vec<ParseWarning>) {
        let mut warnings = Vec::new();
        
        // 跳过文件头
        let after_header: String = content
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");
        
        // 尝试标准分隔符
        if let Some(pos) = after_header.find("\n---\n") {
            let meta = after_header[..pos].trim().to_string();
            let body = after_header[pos + 5..].to_string();
            return (meta, body, warnings);
        }
        
        // 尝试其他分隔符变体
        if let Some(pos) = after_header.find("\n---") {
            let meta = after_header[..pos].trim().to_string();
            let body = after_header[pos + 4..].trim().to_string();
            warnings.push(ParseWarning {
                level: WarningLevel::Info,
                message: "Separator found without trailing newline".into(),
                field: None,
            });
            return (meta, body, warnings);
        }
        
        // 无分隔符 - 尝试推断
        warnings.push(ParseWarning {
            level: WarningLevel::Warning,
            message: "Content separator '---' not found, attempting to infer".into(),
            field: None,
        });
        
        match header.file_type {
            BifrostFileType::Rules => {
                // 规则类型：查找第一个看起来像规则的行
                Self::split_rules_content(&after_header, &mut warnings)
            }
            _ => {
                // JSON 类型：查找 [ 或 { 开始
                Self::split_json_content(&after_header, &mut warnings)
            }
        }
    }
    
    fn split_rules_content(
        content: &str,
        warnings: &mut Vec<ParseWarning>,
    ) -> (String, String) {
        let mut meta_end = 0;
        let mut in_toml = false;
        
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            // TOML 特征：[section] 或 key = value
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_toml = true;
                meta_end = content.lines()
                    .take(i + 1)
                    .map(|l| l.len() + 1)
                    .sum();
                continue;
            }
            
            if trimmed.contains('=') && !trimmed.starts_with('#') {
                in_toml = true;
                meta_end = content.lines()
                    .take(i + 1)
                    .map(|l| l.len() + 1)
                    .sum();
                continue;
            }
            
            // 规则特征：pattern protocol://value
            if !trimmed.is_empty() 
                && !trimmed.starts_with('#')
                && (trimmed.contains("://") || trimmed.contains(" "))
                && !trimmed.contains('=')
            {
                // 找到规则开始
                let content_start: usize = content.lines()
                    .take(i)
                    .map(|l| l.len() + 1)
                    .sum();
                
                return (
                    content[..meta_end].trim().to_string(),
                    content[content_start..].to_string(),
                );
            }
        }
        
        // 没有找到规则，整个内容可能就是规则（无元信息）
        if !in_toml {
            warnings.push(ParseWarning {
                level: WarningLevel::Warning,
                message: "No meta section found, treating entire content as rules".into(),
                field: None,
            });
            return (String::new(), content.to_string());
        }
        
        // 有 TOML 但没有规则
        (content.to_string(), String::new())
    }
    
    fn split_json_content(
        content: &str,
        warnings: &mut Vec<ParseWarning>,
    ) -> (String, String) {
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') || trimmed.starts_with('{') {
                let content_start: usize = content.lines()
                    .take(i)
                    .map(|l| l.len() + 1)
                    .sum();
                
                return (
                    content[..content_start].trim().to_string(),
                    content[content_start..].to_string(),
                );
            }
        }
        
        // 没找到 JSON，整个内容可能就是 JSON
        if content.trim().starts_with('[') || content.trim().starts_with('{') {
            warnings.push(ParseWarning {
                level: WarningLevel::Warning,
                message: "No meta section found, treating entire content as JSON".into(),
                field: None,
            });
            return (String::new(), content.to_string());
        }
        
        (content.to_string(), String::new())
    }
    
    /// 容错解析规则文件
    pub fn parse_rules_tolerant(content: &str) -> ParseResult<RuleFile> {
        let ParseResult { data: raw, mut warnings } = Self::parse_tolerant(content);
        
        // 解析元信息（容错）
        let meta = Self::parse_rules_meta_tolerant(&raw.meta_raw, &mut warnings);
        
        ParseResult {
            data: BifrostFile {
                header: raw.header,
                meta,
                options: toml::Value::Table(toml::map::Map::new()),
                content: raw.content_raw,
            },
            warnings,
        }
    }
    
    fn parse_rules_meta_tolerant(
        meta_raw: &str,
        warnings: &mut Vec<ParseWarning>,
    ) -> RuleFileMeta {
        if meta_raw.is_empty() {
            warnings.push(ParseWarning {
                level: WarningLevel::Warning,
                message: "Meta section is empty, using defaults".into(),
                field: None,
            });
            return RuleFileMeta::default();
        }
        
        // 尝试完整解析
        #[derive(Deserialize, Default)]
        struct MetaWrapper {
            #[serde(default)]
            meta: PartialRuleFileMeta,
        }
        
        #[derive(Deserialize, Default)]
        struct PartialRuleFileMeta {
            name: Option<String>,
            enabled: Option<bool>,
            sort_order: Option<i32>,
            version: Option<String>,
            created_at: Option<String>,
            updated_at: Option<String>,
            description: Option<String>,
        }
        
        let parsed: MetaWrapper = toml::from_str(meta_raw).unwrap_or_else(|e| {
            warnings.push(ParseWarning {
                level: WarningLevel::Error,
                message: format!("Failed to parse meta as TOML: {}", e),
                field: None,
            });
            MetaWrapper::default()
        });
        
        let now = chrono::Utc::now().to_rfc3339();
        let partial = parsed.meta;
        
        // 为缺失字段添加警告
        if partial.name.is_none() {
            warnings.push(ParseWarning {
                level: WarningLevel::Info,
                message: "Field 'name' missing, using default".into(),
                field: Some("name".into()),
            });
        }
        
        RuleFileMeta {
            name: partial.name.unwrap_or_else(|| "unnamed".to_string()),
            enabled: partial.enabled.unwrap_or(true),
            sort_order: partial.sort_order.unwrap_or(0),
            version: partial.version.unwrap_or_else(|| "1.0.0".to_string()),
            created_at: partial.created_at.unwrap_or_else(|| now.clone()),
            updated_at: partial.updated_at.unwrap_or(now),
            description: partial.description,
        }
    }
}
```

### 7.5 容错解析示例

#### 示例 1：完全损坏的文件头

**输入**:
```
[meta]
name = "my-rules"
enabled = true

---
example.com proxy://localhost:3000
```

**解析结果**:
- 类型推断为 `rules`（根据内容特征）
- 版本默认为 `01`
- 警告：`Version number missing or invalid`
- 警告：`File type missing or invalid, inferred as: rules`

#### 示例 2：缺少分隔符

**输入**:
```
01 rules

[meta]
name = "my-rules"
example.com proxy://localhost:3000
```

**解析结果**:
- 正确识别规则内容开始位置
- 警告：`Content separator '---' not found`

#### 示例 3：部分 JSON 损坏

**输入**:
```
01 network

[meta]
name = "test"

---
[
  {"id": "1", "method": "GET", "url": "http://a.com", "status": 200, "duration_ms": 100, "timestamp": 1000},
  {"id": "2", "method": invalid json here},
  {"id": "3", "method": "POST", "url": "http://b.com", "status": 201, "duration_ms": 150, "timestamp": 2000}
]
```

**解析结果**:
- 成功解析第 1 和第 3 条记录
- 错误：`Item 1: expected value at line 1 column 23`
- 返回 2 条有效记录

### 7.6 API 层面的容错处理

```rust
// HTTP API 响应增加警告信息
#[derive(Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub data: ImportedData,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// 导入时的容错处理
pub async fn import_bifrost_file(content: &str) -> ImportResponse {
    let result = BifrostFileParser::parse_tolerant(content);
    
    ImportResponse {
        success: !result.has_errors(),
        data: process_import(&result.data),
        warnings: result.warnings
            .iter()
            .map(|w| format!("[{}] {}", w.level, w.message))
            .collect(),
        error: if result.has_errors() {
            Some("Some data could not be recovered".into())
        } else {
            None
        },
    }
}
```

### 7.7 容错等级配置

支持配置不同的容错等级，让用户可以选择严格模式或宽松模式：

```rust
#[derive(Debug, Clone, Copy, Default)]
pub enum ParseMode {
    /// 严格模式：任何格式问题都报错
    Strict,
    /// 标准模式：轻微问题警告，严重问题报错
    #[default]
    Standard,
    /// 宽松模式：尽可能恢复数据，只在完全无法解析时报错
    Tolerant,
}

impl BifrostFileParser {
    pub fn parse_with_mode(content: &str, mode: ParseMode) -> Result<ParseResult<BifrostFileRaw>> {
        match mode {
            ParseMode::Strict => {
                // 严格模式：使用原有的解析逻辑
                let raw = Self::parse_raw(content)?;
                Ok(ParseResult::ok(raw))
            }
            ParseMode::Standard => {
                // 标准模式：容错解析，但有 Error 级别警告时返回错误
                let result = Self::parse_tolerant(content);
                if result.has_errors() {
                    Err(anyhow!("Parse errors: {:?}", result.warnings))
                } else {
                    Ok(result)
                }
            }
            ParseMode::Tolerant => {
                // 宽松模式：返回所有解析结果
                Ok(Self::parse_tolerant(content))
            }
        }
    }
}
```

---

## 8. 存储迁移方案

### 8.1 现有格式 vs 新格式

**现有格式** (`rules/{name}.json`):
```json
{
  "name": "api-rules",
  "content": "example.com proxy://localhost:3000",
  "enabled": true,
  "metadata": {}
}
```

**新格式** (`rules/{name}.bifrost`):
```
01 rules

[meta]
name = "api-rules"
enabled = true
sort_order = 0
version = "1.0.0"
created_at = "2026-03-02T10:00:00Z"
updated_at = "2026-03-02T10:00:00Z"

[options]
rule_count = 1

---
example.com proxy://localhost:3000
```

### 8.2 RulesStorage 更新

```rust
// crates/bifrost-storage/src/rules.rs

use bifrost_core::bifrost_file::{BifrostFileParser, BifrostFileWriter, RuleFileMeta};

pub struct RulesStorage {
    rules_dir: PathBuf,
}

impl RulesStorage {
    pub fn new(rules_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&rules_dir)?;
        Ok(Self { rules_dir })
    }

    fn rule_path(&self, name: &str) -> PathBuf {
        self.rules_dir.join(format!("{}.bifrost", name))
    }

    pub fn save(&self, meta: &RuleFileMeta, content: &str) -> Result<()> {
        let file_content = BifrostFileWriter::write_rules(meta, content)?;
        let path = self.rule_path(&meta.name);
        fs::write(&path, file_content)?;
        Ok(())
    }

    pub fn load(&self, name: &str) -> Result<(RuleFileMeta, String)> {
        let path = self.rule_path(name);
        let content = fs::read_to_string(&path)?;
        let file = BifrostFileParser::parse_rules(&content)?;
        Ok((file.meta, file.content))
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.rule_path(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.rules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "bifrost").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().to_string());
                }
            }
        }
        Ok(names)
    }

    pub fn load_all(&self) -> Result<Vec<(RuleFileMeta, String)>> {
        let names = self.list()?;
        let mut rules = Vec::new();
        for name in names {
            match self.load(&name) {
                Ok(rule) => rules.push(rule),
                Err(e) => {
                    tracing::warn!(name = %name, error = %e, "Failed to load rule file");
                }
            }
        }
        rules.sort_by_key(|(meta, _)| meta.sort_order);
        Ok(rules)
    }

    pub fn load_enabled(&self) -> Result<Vec<(RuleFileMeta, String)>> {
        let all = self.load_all()?;
        Ok(all.into_iter().filter(|(meta, _)| meta.enabled).collect())
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let (mut meta, content) = self.load(name)?;
        meta.enabled = enabled;
        meta.updated_at = chrono::Utc::now().to_rfc3339();
        self.save(&meta, &content)
    }

    pub fn set_sort_order(&self, name: &str, sort_order: i32) -> Result<()> {
        let (mut meta, content) = self.load(name)?;
        meta.sort_order = sort_order;
        meta.updated_at = chrono::Utc::now().to_rfc3339();
        self.save(&meta, &content)
    }

    pub fn update_content(&self, name: &str, new_content: &str) -> Result<()> {
        let (mut meta, _) = self.load(name)?;
        meta.updated_at = chrono::Utc::now().to_rfc3339();
        self.save(&meta, new_content)
    }
}
```

### 8.3 State 文件移除

由于 `enabled` 和 `sort_order` 现在直接存储在 `.bifrost` 文件的 meta 中，`state.json` 不再需要存储规则相关的状态。

**变更点**:
- 移除 `RuntimeState.disabled_rules`
- 保留 `RuntimeState.enabled_groups`（如果有规则组功能）
- 或完全移除 `state.json`（如果不需要其他运行时状态）

---

## 9. TypeScript 类型定义

```typescript
// web/src/types/bifrost-file.ts

export type BifrostFileType = 'rules' | 'network' | 'script' | 'values' | 'template';

export interface BifrostFileHeader {
  version: number;
  file_type: BifrostFileType;
}

export interface RuleFileMeta {
  name: string;
  enabled: boolean;
  sort_order: number;
  version: string;
  created_at: string;
  updated_at: string;
  description?: string;
}

export interface ExportMeta {
  name: string;
  version: string;
  created_at: string;
  description?: string;
}

export interface NetworkRecord {
  id: string;
  method: string;
  url: string;
  status: number;
  request_headers?: [string, string][];
  response_headers?: [string, string][];
  request_body?: string;
  response_body?: string;
  duration_ms: number;
  timestamp: number;
  matched_rules?: MatchedRuleExport[];
}

export interface MatchedRuleExport {
  pattern: string;
  protocol: string;
  value: string;
}

export interface ScriptItem {
  name: string;
  script_type: 'request' | 'response';
  description?: string;
  content: string;
}

export type ValuesContent = Record<string, string>;

export interface TemplateContent {
  groups: ReplayGroup[];
  requests: ReplayRequest[];
}

export interface RuleFile {
  meta: RuleFileMeta;
  content: string;
  rule_count: number;
}

export interface RuleFileSummary {
  name: string;
  enabled: boolean;
  sort_order: number;
  rule_count: number;
  description?: string;
  updated_at: string;
}
```

---

## 10. API 设计

### 10.1 Rules API 更新

| 端点 | 方法 | 描述 |
|------|------|------|
| `GET /api/rules` | GET | 获取规则列表（含 meta 信息） |
| `GET /api/rules/:name` | GET | 获取规则详情 |
| `PUT /api/rules/:name` | PUT | 创建/更新规则 |
| `DELETE /api/rules/:name` | DELETE | 删除规则 |
| `PATCH /api/rules/:name/enabled` | PATCH | 设置启用状态 |
| `PATCH /api/rules/:name/sort` | PATCH | 设置排序 |
| `POST /api/rules/reorder` | POST | 批量重排序 |

### 10.2 导入导出 API

| 端点 | 方法 | 描述 |
|------|------|------|
| `POST /api/bifrost-file/import` | POST | 导入 .bifrost 文件 |
| `POST /api/bifrost-file/export/rules` | POST | 导出规则 |
| `POST /api/bifrost-file/export/network` | POST | 导出网络请求 |
| `POST /api/bifrost-file/export/scripts` | POST | 导出脚本 |
| `POST /api/bifrost-file/export/values` | POST | 导出变量 |
| `POST /api/bifrost-file/export/templates` | POST | 导出请求模板 |

### 10.3 请求响应示例

**获取规则列表**:
```json
GET /api/rules

Response:
{
  "rules": [
    {
      "name": "api-rules",
      "enabled": true,
      "sort_order": 0,
      "rule_count": 15,
      "description": "API 代理规则",
      "updated_at": "2026-03-02T10:00:00Z"
    },
    {
      "name": "mock-rules",
      "enabled": false,
      "sort_order": 1,
      "rule_count": 5,
      "updated_at": "2026-03-01T08:00:00Z"
    }
  ]
}
```

**获取规则详情**:
```json
GET /api/rules/api-rules

Response:
{
  "meta": {
    "name": "api-rules",
    "enabled": true,
    "sort_order": 0,
    "version": "1.0.0",
    "created_at": "2026-03-01T00:00:00Z",
    "updated_at": "2026-03-02T10:00:00Z",
    "description": "API 代理规则"
  },
  "content": "^api\\.example\\.com$ proxy://localhost:3000\n...",
  "rule_count": 15
}
```

**更新规则**:
```json
PUT /api/rules/api-rules
{
  "content": "^api\\.example\\.com$ proxy://localhost:3000",
  "description": "Updated API rules"
}

Response:
{
  "success": true,
  "meta": { ... }
}
```

---

## 11. 实施计划

### Phase 1: 核心模块 (1-2 天)

- [ ] 创建 `bifrost_file` 模块结构
- [ ] 实现类型定义
- [ ] 实现解析器
- [ ] 实现生成器
- [ ] 添加单元测试

### Phase 2: Rules 存储迁移 (1-2 天)

- [ ] 更新 `RulesStorage` 使用新格式
- [ ] 移除 `state.json` 中的规则状态
- [ ] 更新 `ConfigManager` 接口
- [ ] 添加排序支持

### Phase 3: API 更新 (1 天)

- [ ] 更新 Rules API
- [ ] 添加导入导出 API
- [ ] 更新前端类型定义

### Phase 4: 前端更新 (1-2 天)

- [ ] 更新规则列表组件（显示 meta 信息）
- [ ] 添加排序功能
- [ ] 添加导入导出 UI

### Phase 5: 测试与文档 (1 天)

- [ ] E2E 测试
- [ ] 更新 README

---

## 12. 前端技术方案

### 12.1 功能概述

#### 12.1.1 导出功能

支持多选右键导出，覆盖以下资源类型：

| 资源类型 | 页面位置 | 导出格式 |
|----------|----------|----------|
| Rules | `/rules` 规则列表 | `.bifrost` (rules) |
| Network | `/traffic` 流量表 | `.bifrost` (network) |
| Script | `/scripts` 脚本列表 | `.bifrost` (script) |
| Values | `/values` 变量列表 | `.bifrost` (values) |
| Replay | `/replay` 请求集合 | `.bifrost` (template) |

#### 12.1.2 导入功能

支持两种导入方式：

1. **拖拽导入**: 拖入 `.bifrost` 文件自动识别类型并导入
2. **按钮导入**: 每个页面提供导入按钮，选择文件导入

---

### 12.2 组件设计

#### 12.2.1 全局拖拽导入组件

创建全局拖拽监听组件，监听文件拖入事件：

```typescript
// web/src/components/BifrostFileDropZone/index.tsx

import React, { useCallback, useState, useEffect } from 'react';
import { Modal, message, Spin } from 'antd';
import { useNavigate } from 'react-router-dom';
import { bifrostFileApi } from '@/api/bifrost-file';

interface DropZoneProps {
  children: React.ReactNode;
}

export const BifrostFileDropZone: React.FC<DropZoneProps> = ({ children }) => {
  const [isDragging, setIsDragging] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const navigate = useNavigate();

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    if (e.dataTransfer?.types.includes('Files')) {
      setIsDragging(true);
    }
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    // 只在离开窗口时重置状态
    if (e.relatedTarget === null) {
      setIsDragging(false);
    }
  }, []);

  const handleDrop = useCallback(async (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    const files = Array.from(e.dataTransfer?.files || []);
    const bifrostFiles = files.filter(f => f.name.endsWith('.bifrost'));

    if (bifrostFiles.length === 0) {
      message.warning('请拖入 .bifrost 格式的文件');
      return;
    }

    setIsImporting(true);
    
    try {
      for (const file of bifrostFiles) {
        const content = await file.text();
        const result = await bifrostFileApi.import(content);
        
        if (result.warnings?.length > 0) {
          message.warning(`导入 ${file.name} 完成，但有警告`);
        } else {
          message.success(`导入 ${file.name} 成功`);
        }
        
        // 根据类型导航到对应页面
        navigateToPage(result.file_type);
      }
    } catch (error) {
      message.error(`导入失败: ${error}`);
    } finally {
      setIsImporting(false);
    }
  }, [navigate]);

  const navigateToPage = (fileType: string) => {
    const routes: Record<string, string> = {
      rules: '/rules',
      network: '/traffic',
      script: '/scripts',
      values: '/values',
      template: '/replay',
    };
    const route = routes[fileType];
    if (route) {
      navigate(route);
    }
  };

  useEffect(() => {
    window.addEventListener('dragover', handleDragOver);
    window.addEventListener('dragleave', handleDragLeave);
    window.addEventListener('drop', handleDrop);

    return () => {
      window.removeEventListener('dragover', handleDragOver);
      window.removeEventListener('dragleave', handleDragLeave);
      window.removeEventListener('drop', handleDrop);
    };
  }, [handleDragOver, handleDragLeave, handleDrop]);

  return (
    <>
      {children}
      
      {/* 拖拽遮罩层 */}
      {isDragging && (
        <div className="bifrost-drop-overlay">
          <div className="bifrost-drop-content">
            <UploadIcon />
            <span>释放以导入 .bifrost 文件</span>
          </div>
        </div>
      )}
      
      {/* 导入加载状态 */}
      <Modal
        open={isImporting}
        footer={null}
        closable={false}
        centered
        width={200}
      >
        <div style={{ textAlign: 'center', padding: 20 }}>
          <Spin size="large" />
          <p style={{ marginTop: 16 }}>正在导入...</p>
        </div>
      </Modal>
    </>
  );
};
```

**样式**:
```css
/* web/src/components/BifrostFileDropZone/style.css */

.bifrost-drop-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.bifrost-drop-content {
  background: var(--color-bg-elevated);
  border: 2px dashed var(--color-primary);
  border-radius: 12px;
  padding: 48px 64px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  color: var(--color-primary);
  font-size: 18px;
}
```

#### 12.2.2 导入按钮组件

通用的导入按钮组件：

```typescript
// web/src/components/ImportBifrostButton/index.tsx

import React, { useRef, useCallback } from 'react';
import { Button, message, Upload } from 'antd';
import { ImportOutlined } from '@ant-design/icons';
import { bifrostFileApi, BifrostFileType } from '@/api/bifrost-file';

interface ImportBifrostButtonProps {
  expectedType?: BifrostFileType;
  onImportSuccess?: (result: ImportResult) => void;
  buttonText?: string;
  buttonType?: 'default' | 'primary' | 'text' | 'link';
  size?: 'small' | 'middle' | 'large';
  icon?: React.ReactNode;
}

export const ImportBifrostButton: React.FC<ImportBifrostButtonProps> = ({
  expectedType,
  onImportSuccess,
  buttonText = '导入',
  buttonType = 'default',
  size = 'middle',
  icon = <ImportOutlined />,
}) => {
  const handleBeforeUpload = useCallback(async (file: File) => {
    if (!file.name.endsWith('.bifrost')) {
      message.error('请选择 .bifrost 格式的文件');
      return Upload.LIST_IGNORE;
    }

    try {
      const content = await file.text();
      
      // 先检测文件类型
      const detected = await bifrostFileApi.detectType(content);
      
      if (expectedType && detected.file_type !== expectedType) {
        message.error(
          `文件类型不匹配: 期望 ${expectedType}，实际为 ${detected.file_type}`
        );
        return Upload.LIST_IGNORE;
      }
      
      // 执行导入
      const result = await bifrostFileApi.import(content);
      
      if (result.warnings?.length > 0) {
        message.warning(`导入完成，有 ${result.warnings.length} 条警告`);
      } else {
        message.success('导入成功');
      }
      
      onImportSuccess?.(result);
    } catch (error) {
      message.error(`导入失败: ${error}`);
    }

    return Upload.LIST_IGNORE;
  }, [expectedType, onImportSuccess]);

  return (
    <Upload
      accept=".bifrost"
      showUploadList={false}
      beforeUpload={handleBeforeUpload}
      multiple
    >
      <Button type={buttonType} size={size} icon={icon}>
        {buttonText}
      </Button>
    </Upload>
  );
};
```

#### 12.2.3 导出菜单组件

右键菜单中的导出选项组件：

```typescript
// web/src/components/ExportBifrostMenu/index.tsx

import React, { useCallback } from 'react';
import { Menu, message } from 'antd';
import { ExportOutlined } from '@ant-design/icons';
import { bifrostFileApi, BifrostFileType } from '@/api/bifrost-file';

interface ExportBifrostMenuProps {
  fileType: BifrostFileType;
  selectedIds: string[];
  getExportData: () => Promise<ExportData>;
  exportFileName?: string;
  disabled?: boolean;
}

export const useExportBifrost = () => {
  const exportFile = useCallback(async (
    fileType: BifrostFileType,
    data: ExportData,
    fileName?: string,
  ) => {
    try {
      const content = await bifrostFileApi.export(fileType, data);
      
      // 生成文件名
      const defaultName = `bifrost-${fileType}-${formatDate(new Date())}.bifrost`;
      const finalName = fileName || defaultName;
      
      // 下载文件
      downloadFile(content, finalName);
      
      message.success(`已导出为 ${finalName}`);
    } catch (error) {
      message.error(`导出失败: ${error}`);
    }
  }, []);

  return { exportFile };
};

function downloadFile(content: string, filename: string) {
  const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function formatDate(date: Date): string {
  return date.toISOString().slice(0, 19).replace(/[:-]/g, '');
}
```

---

### 12.3 各页面集成方案

#### 12.3.1 Rules 页面

**导入位置**: 规则列表顶部工具栏

**导出触发**: 右键菜单，支持多选

```typescript
// web/src/pages/Rules/RuleList/index.tsx

import { ImportBifrostButton } from '@/components/ImportBifrostButton';
import { useExportBifrost } from '@/components/ExportBifrostMenu';

const RuleList: React.FC = () => {
  const [selectedRules, setSelectedRules] = useState<string[]>([]);
  const { exportFile } = useExportBifrost();
  
  // 右键菜单项
  const getContextMenuItems = (ruleName: string, enabled: boolean) => {
    const isSelected = selectedRules.includes(ruleName);
    const exportNames = isSelected ? selectedRules : [ruleName];
    
    return [
      {
        key: 'enable',
        label: enabled ? '禁用' : '启用',
        onClick: () => handleToggleEnabled(ruleName),
      },
      { type: 'divider' },
      {
        key: 'export',
        label: `导出${exportNames.length > 1 ? ` (${exportNames.length} 个)` : ''}`,
        icon: <ExportOutlined />,
        onClick: async () => {
          const rulesData = await Promise.all(
            exportNames.map(name => rulesApi.getRule(name))
          );
          await exportFile('rules', { rules: rulesData });
        },
      },
      { type: 'divider' },
      {
        key: 'delete',
        label: '删除',
        danger: true,
        onClick: () => handleDelete(ruleName),
      },
    ];
  };
  
  // 导入成功回调
  const handleImportSuccess = useCallback(() => {
    refreshRuleList();
  }, []);

  return (
    <div className="rule-list">
      {/* 顶部工具栏 */}
      <div className="rule-list-toolbar">
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
          新建规则
        </Button>
        <ImportBifrostButton
          expectedType="rules"
          onImportSuccess={handleImportSuccess}
          buttonText="导入规则"
        />
      </div>
      
      {/* 规则列表 - 支持多选 */}
      <div className="rule-items">
        {rules.map(rule => (
          <Dropdown
            key={rule.name}
            menu={{ items: getContextMenuItems(rule.name, rule.enabled) }}
            trigger={['contextMenu']}
          >
            <div
              className={cn('rule-item', {
                selected: selectedRules.includes(rule.name),
              })}
              onClick={(e) => handleSelect(rule.name, e.ctrlKey || e.metaKey)}
            >
              {/* 规则项内容 */}
            </div>
          </Dropdown>
        ))}
      </div>
    </div>
  );
};
```

#### 12.3.2 Traffic 页面

**导入位置**: 顶部工具栏（导入历史流量记录用于分析）

**导出触发**: 右键菜单，支持多选批量导出

```typescript
// web/src/components/TrafficTable/TrafficContextMenu.tsx

import { useExportBifrost } from '@/components/ExportBifrostMenu';

const TrafficContextMenu: React.FC<Props> = ({
  record,
  selectedRecords,
  onClose,
}) => {
  const { exportFile } = useExportBifrost();
  
  // 获取要导出的记录（单选或多选）
  const recordsToExport = selectedRecords.length > 0 
    ? selectedRecords 
    : record ? [record] : [];
  
  const menuItems = [
    {
      key: 'replay',
      label: '重放请求',
      onClick: handleReplay,
      disabled: record?.method === 'CONNECT',
    },
    { type: 'divider' },
    {
      key: 'copy-url',
      label: '复制 URL',
      onClick: handleCopyUrl,
    },
    {
      key: 'copy-curl',
      label: '复制为 cURL',
      onClick: handleCopyCurl,
    },
    { type: 'divider' },
    {
      key: 'export-har',
      label: `导出 HAR${recordsToExport.length > 1 ? ` (${recordsToExport.length})` : ''}`,
      onClick: () => downloadHAR(recordsToExport),
    },
    {
      key: 'export-bifrost',
      label: `导出 .bifrost${recordsToExport.length > 1 ? ` (${recordsToExport.length})` : ''}`,
      icon: <ExportOutlined />,
      onClick: async () => {
        // 获取完整的流量详情
        const details = await Promise.all(
          recordsToExport.map(r => trafficApi.getTrafficDetail(r.id))
        );
        await exportFile('network', { records: details });
      },
    },
  ];

  return <Menu items={menuItems} />;
};
```

#### 12.3.3 Scripts 页面

**导入位置**: 脚本列表顶部

**导出触发**: 右键菜单，支持多选

```typescript
// web/src/pages/Scripts/ScriptList/index.tsx

const ScriptList: React.FC = () => {
  const [selectedScripts, setSelectedScripts] = useState<string[]>([]);
  const { exportFile } = useExportBifrost();
  
  const getContextMenuItems = (scriptName: string, scriptType: ScriptType) => {
    const isSelected = selectedScripts.includes(scriptName);
    const exportNames = isSelected ? selectedScripts : [scriptName];
    
    return [
      {
        key: 'edit',
        label: '编辑',
        onClick: () => handleEdit(scriptName),
      },
      { type: 'divider' },
      {
        key: 'export',
        label: `导出${exportNames.length > 1 ? ` (${exportNames.length})` : ''}`,
        icon: <ExportOutlined />,
        onClick: async () => {
          const scriptsData = await Promise.all(
            exportNames.map(async name => {
              const info = scripts.find(s => s.name === name);
              const content = await scriptApi.getScriptContent(name, info!.script_type);
              return {
                name,
                script_type: info!.script_type,
                description: info?.description,
                content,
              };
            })
          );
          await exportFile('script', { scripts: scriptsData });
        },
      },
      { type: 'divider' },
      {
        key: 'delete',
        label: '删除',
        danger: true,
        onClick: () => handleDelete(scriptName),
      },
    ];
  };
  
  return (
    <div className="script-list">
      <div className="script-list-toolbar">
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
          新建脚本
        </Button>
        <ImportBifrostButton
          expectedType="script"
          onImportSuccess={refreshScriptList}
          buttonText="导入脚本"
        />
      </div>
      {/* 脚本列表 */}
    </div>
  );
};
```

#### 12.3.4 Values 页面

**导入位置**: 变量列表顶部

**导出触发**: 右键菜单，支持多选或全部导出

```typescript
// web/src/pages/Values/ValueList/index.tsx

const ValueList: React.FC = () => {
  const [selectedValues, setSelectedValues] = useState<string[]>([]);
  const { exportFile } = useExportBifrost();

  const getContextMenuItems = (valueName: string) => {
    const isSelected = selectedValues.includes(valueName);
    const exportNames = isSelected ? selectedValues : [valueName];
    
    return [
      {
        key: 'copy',
        label: '复制值',
        onClick: () => handleCopyValue(valueName),
      },
      { type: 'divider' },
      {
        key: 'export',
        label: `导出${exportNames.length > 1 ? ` (${exportNames.length})` : ''}`,
        icon: <ExportOutlined />,
        onClick: async () => {
          const valuesData: Record<string, string> = {};
          for (const name of exportNames) {
            const value = values.find(v => v.name === name);
            if (value) {
              valuesData[value.name] = value.value;
            }
          }
          await exportFile('values', valuesData);
        },
      },
      { type: 'divider' },
      {
        key: 'delete',
        label: '删除',
        danger: true,
        onClick: () => handleDelete(valueName),
      },
    ];
  };

  // 全部导出按钮
  const handleExportAll = async () => {
    const valuesData: Record<string, string> = {};
    values.forEach(v => {
      valuesData[v.name] = v.value;
    });
    await exportFile('values', valuesData, 'bifrost-values-all.bifrost');
  };

  return (
    <div className="value-list">
      <div className="value-list-toolbar">
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
          新建变量
        </Button>
        <ImportBifrostButton
          expectedType="values"
          onImportSuccess={refreshValueList}
          buttonText="导入变量"
        />
        <Button icon={<ExportOutlined />} onClick={handleExportAll}>
          全部导出
        </Button>
      </div>
      {/* 变量列表 */}
    </div>
  );
};
```

#### 12.3.5 Replay 页面

**导入位置**: Collection 面板顶部

**导出触发**: 右键菜单，支持导出单个请求、整个分组、或多选请求

```typescript
// web/src/pages/Replay/components/CollectionPanel.tsx

const CollectionPanel: React.FC = () => {
  const [selectedKeys, setSelectedKeys] = useState<string[]>([]);
  const { exportFile } = useExportBifrost();

  // 分组右键菜单
  const getGroupContextMenu = (groupId: string) => [
    {
      key: 'rename',
      label: '重命名',
      onClick: () => handleRenameGroup(groupId),
    },
    {
      key: 'export-group',
      label: '导出分组',
      icon: <ExportOutlined />,
      onClick: async () => {
        const group = groups.find(g => g.id === groupId);
        const groupRequests = requests.filter(r => r.group_id === groupId);
        await exportFile('template', {
          groups: group ? [group] : [],
          requests: groupRequests,
        });
      },
    },
    { type: 'divider' },
    {
      key: 'delete',
      label: '删除',
      danger: true,
      onClick: () => handleDeleteGroup(groupId),
    },
  ];

  // 请求右键菜单
  const getRequestContextMenu = (requestId: string) => {
    const isSelected = selectedKeys.includes(`req-${requestId}`);
    const exportIds = isSelected 
      ? selectedKeys.filter(k => k.startsWith('req-')).map(k => k.replace('req-', ''))
      : [requestId];
    
    return [
      {
        key: 'duplicate',
        label: '复制',
        onClick: () => handleDuplicateRequest(requestId),
      },
      {
        key: 'export',
        label: `导出${exportIds.length > 1 ? ` (${exportIds.length})` : ''}`,
        icon: <ExportOutlined />,
        onClick: async () => {
          const exportRequests = requests.filter(r => exportIds.includes(r.id));
          // 收集相关的分组
          const relatedGroupIds = [...new Set(exportRequests.map(r => r.group_id).filter(Boolean))];
          const relatedGroups = groups.filter(g => relatedGroupIds.includes(g.id));
          
          await exportFile('template', {
            groups: relatedGroups,
            requests: exportRequests,
          });
        },
      },
      { type: 'divider' },
      {
        key: 'delete',
        label: '删除',
        danger: true,
        onClick: () => handleDeleteRequest(requestId),
      },
    ];
  };

  // 导入成功回调
  const handleImportSuccess = useCallback(async (result: ImportResult) => {
    if (result.file_type === 'template') {
      await refreshCollection();
      message.success(`导入了 ${result.data.request_count} 个请求`);
    }
  }, []);

  return (
    <div className="collection-panel">
      <div className="collection-toolbar">
        <Button size="small" icon={<PlusOutlined />} onClick={handleCreateGroup}>
          新建分组
        </Button>
        <ImportBifrostButton
          expectedType="template"
          onImportSuccess={handleImportSuccess}
          buttonText="导入"
          size="small"
        />
      </div>
      
      <Tree
        treeData={treeData}
        selectedKeys={selectedKeys}
        onSelect={handleSelect}
        multiple
        // ... 其他属性
      />
    </div>
  );
};
```

---

### 12.4 API 客户端设计

```typescript
// web/src/api/bifrost-file.ts

export type BifrostFileType = 'rules' | 'network' | 'script' | 'values' | 'template';

export interface ImportResult {
  success: boolean;
  file_type: BifrostFileType;
  data: ImportedData;
  warnings: string[];
}

export interface ImportedData {
  // Rules
  rule_names?: string[];
  rule_count?: number;
  
  // Network
  record_count?: number;
  
  // Script
  script_names?: string[];
  script_count?: number;
  
  // Values
  value_names?: string[];
  value_count?: number;
  
  // Template
  group_count?: number;
  request_count?: number;
}

export interface DetectResult {
  file_type: BifrostFileType;
  meta: Record<string, unknown>;
}

export interface ExportRulesRequest {
  rule_names: string[];
  description?: string;
}

export interface ExportNetworkRequest {
  record_ids: string[];
  include_body?: boolean;
  description?: string;
}

export interface ExportScriptRequest {
  script_names: string[];
  description?: string;
}

export interface ExportValuesRequest {
  value_names?: string[];  // 为空表示全部导出
  description?: string;
}

export interface ExportTemplateRequest {
  group_ids?: string[];
  request_ids?: string[];
  description?: string;
}

const BASE_URL = '/_bifrost/api';

export const bifrostFileApi = {
  // 检测文件类型
  async detectType(content: string): Promise<DetectResult> {
    const res = await fetch(`${BASE_URL}/bifrost-file/detect`, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: content,
    });
    return res.json();
  },

  // 导入文件
  async import(content: string): Promise<ImportResult> {
    const res = await fetch(`${BASE_URL}/bifrost-file/import`, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: content,
    });
    return res.json();
  },

  // 导出规则
  async exportRules(request: ExportRulesRequest): Promise<string> {
    const res = await fetch(`${BASE_URL}/bifrost-file/export/rules`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return res.text();
  },

  // 导出网络请求
  async exportNetwork(request: ExportNetworkRequest): Promise<string> {
    const res = await fetch(`${BASE_URL}/bifrost-file/export/network`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return res.text();
  },

  // 导出脚本
  async exportScripts(request: ExportScriptRequest): Promise<string> {
    const res = await fetch(`${BASE_URL}/bifrost-file/export/scripts`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return res.text();
  },

  // 导出变量
  async exportValues(request: ExportValuesRequest): Promise<string> {
    const res = await fetch(`${BASE_URL}/bifrost-file/export/values`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return res.text();
  },

  // 导出请求模板
  async exportTemplates(request: ExportTemplateRequest): Promise<string> {
    const res = await fetch(`${BASE_URL}/bifrost-file/export/templates`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    return res.text();
  },
};
```

---

### 12.5 全局集成

在 App.tsx 中包裹全局拖拽组件：

```typescript
// web/src/App.tsx

import { BifrostFileDropZone } from '@/components/BifrostFileDropZone';

function App() {
  return (
    <BrowserRouter basename="/_bifrost">
      <BifrostFileDropZone>
        <Routes>
          <Route path="/" element={<AppLayout />}>
            {/* 路由配置 */}
          </Route>
        </Routes>
      </BifrostFileDropZone>
    </BrowserRouter>
  );
}
```

---

### 12.6 UI/UX 细节

#### 12.6.1 多选支持

| 页面 | 多选方式 |
|------|----------|
| Rules | Ctrl/Cmd + Click 或 Shift + Click |
| Traffic | 表格复选框 |
| Scripts | Ctrl/Cmd + Click |
| Values | Ctrl/Cmd + Click |
| Replay | Tree 组件内置多选（multiple 属性） |

#### 12.6.2 右键菜单显示规则

- 单选时：显示单个项目的操作菜单
- 多选时：显示批量操作菜单，标注选中数量
- 导出选项始终显示，根据选中数量调整文案

#### 12.6.3 拖拽导入视觉反馈

```
┌──────────────────────────────────────┐
│                                      │
│    ┌────────────────────────┐        │
│    │    📥                   │        │
│    │                        │        │
│    │  释放以导入 .bifrost   │        │
│    │        文件            │        │
│    │                        │        │
│    └────────────────────────┘        │
│                                      │
└──────────────────────────────────────┘
```

- 拖入文件时全屏半透明遮罩
- 中央显示导入提示区域
- 虚线边框强调放置区域

#### 12.6.4 导入结果反馈

| 情况 | 反馈方式 |
|------|----------|
| 成功无警告 | 绿色 toast：导入成功 |
| 成功有警告 | 黄色 toast + 可展开详情 |
| 部分成功 | 黄色 Modal 显示详情 |
| 完全失败 | 红色 Modal 显示错误 |

---

### 12.7 文件结构

```
web/src/
├── api/
│   └── bifrost-file.ts              # API 客户端
├── components/
│   ├── BifrostFileDropZone/         # 全局拖拽组件
│   │   ├── index.tsx
│   │   └── style.css
│   ├── ImportBifrostButton/         # 导入按钮组件
│   │   └── index.tsx
│   └── ExportBifrostMenu/           # 导出菜单 Hook
│       └── index.tsx
├── pages/
│   ├── Rules/
│   │   └── RuleList/index.tsx       # 集成导入导出
│   ├── Traffic/
│   │   └── ...                      # 集成导出
│   ├── Scripts/
│   │   └── ScriptList/index.tsx     # 集成导入导出
│   ├── Values/
│   │   └── ValueList/index.tsx      # 集成导入导出
│   └── Replay/
│       └── components/
│           └── CollectionPanel.tsx  # 集成导入导出
└── types/
    └── bifrost-file.ts              # TypeScript 类型
```

---

## 13. 文件结构总览

```
$BIFROST_DATA_DIR/
├── config.toml                    # 主配置文件
├── rules/                         # 规则目录
│   ├── api-rules.bifrost         # 规则文件（新格式）
│   ├── mock-rules.bifrost
│   └── ...
├── scripts/
│   ├── request/
│   │   └── *.js
│   └── response/
│       └── *.js
├── values/
├── certs/
└── traffic/

crates/bifrost-core/src/
├── bifrost_file/
│   ├── mod.rs
│   ├── types.rs
│   ├── parser.rs
│   ├── writer.rs
│   └── rules.rs
└── ...

web/src/
├── types/
│   └── bifrost-file.ts
├── api/
│   └── bifrost-file.ts
└── ...
```

---

## 14. 附录：完整文件示例

### A. 规则文件示例 (`api-rules.bifrost`)

```
01 rules

[meta]
name = "api-rules"
enabled = true
sort_order = 0
version = "1.0.0"
created_at = "2026-03-02T00:00:00Z"
updated_at = "2026-03-02T15:30:00Z"
description = "API 接口代理和 Mock 规则"

[options]
rule_count = 8

---
# ===========================================
# API 代理规则
# ===========================================

# 将 API 请求转发到本地开发服务器
^api\.example\.com$ proxy://localhost:3000
^api\.example\.com/v2/(.*)$ proxy://localhost:3001/$1

# ===========================================
# Mock 响应
# ===========================================

# 返回静态 JSON 文件
^api\.example\.com/users$ file://./mock/users.json
^api\.example\.com/config$ file://./mock/config.json

# 直接返回 JSON 内容
^api\.example\.com/health$ resBody://{"status":"ok","timestamp":${now}}

# ===========================================
# 请求修改
# ===========================================

# 添加认证头
api.example.com reqHeaders://Authorization=Bearer ${API_TOKEN}

# 添加调试头
*.example.com resHeaders://X-Proxy=bifrost

# 使用脚本处理
api.example.com/complex reqScript://transform-request
```
