# Agent Skill 安装说明

仓库根目录已经提供可直接复用的 `SKILL.md`。

## 在当前仓库中使用

如果 Agent 会读取仓库根目录的 `AGENTS.md`，通常不需要额外安装，`SKILL.md` 会被自动发现。

## 复制到其他仓库

最简单的方式是直接复制根目录的 `SKILL.md` 到目标仓库。

## 安装到全局 Codex skills 目录

```bash
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills/bifrost-cli-agent"
cp ./SKILL.md "${CODEX_HOME:-$HOME/.codex}/skills/bifrost-cli-agent/SKILL.md"
```

如果更喜欢软链接：

```bash
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills/bifrost-cli-agent"
ln -sf "$(pwd)/SKILL.md" "${CODEX_HOME:-$HOME/.codex}/skills/bifrost-cli-agent/SKILL.md"
```

## 安装后检查

```bash
ls "${CODEX_HOME:-$HOME/.codex}/skills/bifrost-cli-agent"
```

目录中至少应包含 `SKILL.md`。

## 使用约定

- 在当前仓库中直接让 Agent 执行与 `bifrost` CLI 相关的启动、配置、排查任务即可触发
- 如果 Agent 检测到本机未安装 `bifrost`，会优先使用官方安装脚本安装
- 如果已安装且用户未禁止升级，通常会优先执行 `bifrost upgrade -y`
