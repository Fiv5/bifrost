# 桌面安装脚本改为直接复制 App Bundle

## 功能模块详细描述

调整 macOS 下的源码安装流程：`./install.sh` 在安装桌面端时不再依赖 DMG 打包产物，而是仅构建 `Bifrost.app`，随后直接复制到 macOS 的 Applications 目录。

## 实现逻辑

- 新增 `package.json` 脚本 `desktop:build:app`
- 该脚本沿用现有桌面构建前置步骤，只在 Tauri 打包阶段显式传入 `--bundles app`
- `install.sh` 的桌面安装流程改为调用 `pnpm run desktop:build:app`
- `install.sh` 默认桌面安装目录从 `~/Applications` 调整为 `/Applications`
- 在复制 `Bifrost.app` 前检查目标目录可写性；若不可写，则给出明确提示，建议使用 `sudo` 或通过 `--app-dir` 改到用户目录

## 依赖项

- `pnpm`
- `@tauri-apps/cli`
- macOS Tauri 构建环境
- `ditto` 或 `cp -R` 用于复制 `.app`

## 测试方案（含 e2e）

- 手动执行 `pnpm run desktop:build:app`，确认产出 `desktop/src-tauri/target/release/bundle/macos/Bifrost.app`
- 手动执行 `./install.sh --desktop-only --app-dir /tmp/bifrost-install-test-apps`，确认脚本可直接复制 `.app`，且不会尝试生成 DMG
- 本次变更只涉及安装/打包脚本，不涉及代理转发链路，因此不新增代理 E2E 用例；仍按项目要求执行现有校验命令

## 校验要求（含 rust-project-validate）

- 先执行与本次改动相关的脚本验证
- 再执行 `rust-project-validate` 技能要求的格式、lint、测试、构建检查
- 若校验耗时较长，优先按改动范围执行，确保最终结果可复现

## 文档更新要求

- 更新 `README.md` 中源码安装说明，明确 `./install.sh` 在 macOS 上会直接安装 `Bifrost.app` 到 `/Applications`
- 更新桌面构建说明，补充仅构建 `.app` 的命令与产物位置
