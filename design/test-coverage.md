# 测试覆盖率检测方案

## 概述

为 Bifrost 项目引入基于 LLVM source-based instrumentation 的测试覆盖率检测，覆盖单元测试和端到端测试两个维度。

## 工具选型

| 工具 | 方案 | 选择原因 |
|------|------|----------|
| **cargo-llvm-cov** | LLVM source-based | ✅ 精度最高（Region 级别），稳定 Rust 即可运行，支持 `cargo test` / `cargo run` / nextest，原生支持多次运行合并 |
| cargo-tarpaulin | ptrace-based | ❌ 仅 Linux x86_64，macOS/Windows 不支持 |
| grcov | GCNO/GCDA | ❌ 依赖 nightly，配置复杂 |

**最终选择：cargo-llvm-cov v0.8+**

## 架构设计

### 两层覆盖率体系

```
┌─────────────────────────────────────────────────────┐
│                  Coverage Pipeline                   │
├─────────────────────┬───────────────────────────────┤
│  单元测试覆盖率      │  E2E 测试覆盖率               │
│  coverage.sh         │  coverage-e2e.sh              │
├─────────────────────┼───────────────────────────────┤
│  cargo llvm-cov      │  RUSTFLAGS=-C instrument-     │
│  --workspace         │    coverage                   │
│  --all-features      │  + llvm-profdata merge        │
│                      │  + llvm-cov report            │
├─────────────────────┼───────────────────────────────┤
│  验证：函数/模块逻辑  │  验证：运行时代码路径          │
│  覆盖粒度：Region     │  覆盖粒度：Region             │
└─────────────────────┴───────────────────────────────┘
```

### 单元测试覆盖率

直接使用 `cargo llvm-cov` 对 workspace 内所有 `#[test]` 函数运行并统计覆盖率。

**关键特性：**
- 支持 text / HTML / LCOV / JSON 多种输出格式
- 支持 `--fail-under-lines` 门禁，覆盖率低于阈值则失败
- 支持 `-p` 参数对单个 crate 做精细分析
- LCOV 格式可对接 Codecov / Coveralls / IDE 插件

### E2E 测试覆盖率

E2E 测试覆盖率需要对 **被测二进制**（bifrost server）进行插桩，而非测试框架本身。

**核心流程：**

1. **插桩编译**：使用 `RUSTFLAGS="-C instrument-coverage"` 编译 bifrost 和 bifrost-e2e 二进制
2. **收集 profraw**：通过 `LLVM_PROFILE_FILE` 环境变量，运行期间每个进程生成 `.profraw` 文件
3. **合并数据**：使用 `llvm-profdata merge -sparse` 合并所有 `.profraw` 文件
4. **生成报告**：使用 `llvm-cov report/show/export` 生成覆盖率报告

**忽略规则：**
- `.cargo/registry` — 第三方依赖
- `rustc/` — 标准库
- `crates/bifrost-e2e/` — 测试框架本身的代码（在 E2E 覆盖率中只关注被测代码）

## 文件清单

| 文件 | 用途 |
|------|------|
| `scripts/ci/coverage.sh` | 单元测试覆盖率脚本 |
| `scripts/ci/coverage-e2e.sh` | E2E 测试覆盖率脚本 |
| `.github/workflows/ci.yml` (coverage job) | CI 自动覆盖率检测（PR 触发） |
| `scripts/ci/local-ci.sh` (--coverage 选项) | 本地 CI 集成覆盖率 |

## 使用方式

### 本地快速检查

```bash
# 全 workspace 单元测试覆盖率（终端输出）
bash scripts/ci/coverage.sh

# 生成 HTML 报告并在浏览器打开
bash scripts/ci/coverage.sh --open

# 单个 crate 覆盖率
bash scripts/ci/coverage.sh -p bifrost-core

# 设置门禁：低于 70% 则失败
bash scripts/ci/coverage.sh --fail-under 70

# 生成 LCOV 格式（可导入 IDE）
bash scripts/ci/coverage.sh --lcov

# 通过 local-ci 运行（包含 fmt/clippy/test + 覆盖率）
bash scripts/ci/local-ci.sh --skip-e2e --coverage
bash scripts/ci/local-ci.sh --skip-e2e --coverage-html
```

### E2E 覆盖率

```bash
# 全量 E2E 覆盖率
bash scripts/ci/coverage-e2e.sh --html

# 指定 suite
bash scripts/ci/coverage-e2e.sh --suite rules --open

# 设置门禁
bash scripts/ci/coverage-e2e.sh --fail-under 50
```

### CI 集成

PR 自动触发 `coverage` Job：
1. 使用 `taiki-e/install-action@cargo-llvm-cov` 安装工具
2. 生成 LCOV 格式覆盖率数据
3. 在 Job Summary 中显示覆盖率摘要
4. 上传 `lcov.info` 作为 artifact（7 天有效期）

## 当前覆盖率基线

| Crate | Lines | Regions | Functions |
|-------|-------|---------|-----------|
| bifrost-core | 77.5% | 79.7% | 76.8% |

## 渐进式门禁策略

建议采用"只升不降"策略：
1. 首次测量当前覆盖率作为基线
2. CI 中设置 `--fail-under-lines` 为当前值
3. 每次覆盖率提升后，上调门禁阈值
4. 目标：核心库 crate（bifrost-core, bifrost-proxy）达到 80%+

## 依赖项

- `cargo-llvm-cov` >= 0.8（`cargo install cargo-llvm-cov`）
- `llvm-tools-preview` rustup component（`rustup component add llvm-tools-preview`）
- 脚本自动检测并安装缺失依赖
