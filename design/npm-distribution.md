# npm 渠道分发

## 功能模块详细描述

通过 npm 渠道分发 bifrost CLI 二进制文件，用户可以通过 `npm install -g @bifrost-proxy/bifrost` 安装 bifrost。

采用 **平台包 + 主包** 的架构模式（与 esbuild、turbo、SWC 等项目一致）：

- **平台包**：每个 target 对应一个独立的 npm 包，包含该平台的预编译二进制文件
- **主包**（`@bifrost-proxy/bifrost`）：作为入口，根据当前平台自动选择并安装对应的平台包

### 包结构

```
@bifrost-proxy/bifrost              # 主包 - 入口 + 平台分发逻辑
@bifrost-proxy/bifrost-linux-x64    # Linux x86_64
@bifrost-proxy/bifrost-linux-arm64  # Linux aarch64
@bifrost-proxy/bifrost-linux-arm    # Linux armv7
@bifrost-proxy/bifrost-darwin-x64   # macOS Intel
@bifrost-proxy/bifrost-darwin-arm64 # macOS Apple Silicon
@bifrost-proxy/bifrost-win32-x64    # Windows x64
@bifrost-proxy/bifrost-win32-arm64  # Windows ARM64
```

### 用户安装方式

```bash
# 全局安装
npm install -g @bifrost-proxy/bifrost

# npx 直接运行
npx @bifrost-proxy/bifrost start

# 项目开发依赖
npm install --save-dev @bifrost-proxy/bifrost
```

## 实现逻辑

### 平台包

每个平台包包含：
- `package.json`：声明 `os` 和 `cpu` 字段限制安装平台
- 预编译的 bifrost 二进制文件

npm 的 `optionalDependencies` + `os`/`cpu` 机制确保只有匹配当前平台的包会被安装。

### 主包

主包通过 `optionalDependencies` 声明所有平台包，并提供：
- `bin/bifrost`：一个 Node.js 脚本，查找并执行已安装的平台包中的二进制文件
- `postinstall` 脚本（可选）：验证平台包安装成功

### 平台映射表

| Rust Target                       | npm 包名                             | os      | cpu   |
|-----------------------------------|--------------------------------------|---------|-------|
| x86_64-unknown-linux-gnu          | @bifrost-proxy/bifrost-linux-x64     | linux   | x64   |
| aarch64-unknown-linux-gnu         | @bifrost-proxy/bifrost-linux-arm64   | linux   | arm64 |
| armv7-unknown-linux-gnueabihf     | @bifrost-proxy/bifrost-linux-arm     | linux   | arm   |
| x86_64-apple-darwin               | @bifrost-proxy/bifrost-darwin-x64    | darwin  | x64   |
| aarch64-apple-darwin              | @bifrost-proxy/bifrost-darwin-arm64  | darwin  | arm64 |
| x86_64-pc-windows-msvc            | @bifrost-proxy/bifrost-win32-x64    | win32   | x64   |
| aarch64-pc-windows-msvc           | @bifrost-proxy/bifrost-win32-arm64  | win32   | arm64 |

## 依赖项

- GitHub Actions Release workflow（已有）
- npm registry（npmjs.com）
- NPM_TOKEN secret（需配置到 GitHub repo secrets）

## 文件结构

```
npm/
├── bifrost/                    # 主包 @bifrost-proxy/bifrost
│   ├── package.json
│   ├── bin/
│   │   └── bifrost             # Node.js 入口脚本
│   └── lib/
│       └── index.js            # 平台解析逻辑（供 programmatic API 使用）
├── bifrost-linux-x64/
│   └── package.json
├── bifrost-linux-arm64/
│   └── package.json
├── bifrost-linux-arm/
│   └── package.json
├── bifrost-darwin-x64/
│   └── package.json
├── bifrost-darwin-arm64/
│   └── package.json
├── bifrost-win32-x64/
│   └── package.json
└── bifrost-win32-arm64/
    └── package.json
scripts/
└── npm-publish.mjs             # 发布脚本：注入版本、复制二进制、执行 npm publish
```

## CI 流程

在 release.yml 的 `release` job 之后新增 `publish-npm` job：

1. 下载所有 CLI 构建产物
2. 解压二进制文件到对应平台包目录
3. 同步版本号到所有 package.json
4. 逐个执行 `npm publish` 发布平台包
5. 发布主包

## 测试方案

- 本地执行 `node npm/bifrost/bin/bifrost --version` 验证二进制查找逻辑
- CI 中发布到 npm 后可通过 `npx @bifrost-proxy/bifrost --version` 验证

## 校验要求

- npm publish 前验证二进制文件存在且可执行
- 版本号与 release tag 一致

## 文档更新要求

- 更新 release.yml 中的 Installation 说明，新增 npm 安装方式
