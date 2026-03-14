# Bifrost 文件协议 (.bifrost)

## 现状结论

协议已经实现，但旧文档保留了很多理想化描述，没有完全反映 parser / writer / 管理端导入导出的真实行为。

## 当前已实现格式

- 头部：`01 <type>`
- 支持类型：
  - `rules`
  - `network`
  - `script`
  - `values`
  - `template`
- 基本结构：
  - TOML `[meta]`
  - TOML `[options]`
  - `---`
  - 正文内容

## 关键现状

- `rules` 的正文仍是原始规则文本，且带 `enabled`、`sort_order`、`created_at`、`updated_at` 等元信息。
- `network` / `script` / `values` / `template` 的正文是 JSON。
- `script` 导出已不只 request/response，当前还会包含 `decode` 脚本。
- `template` 已支持 `groups + requests` 结构。
- parser 具备一定 tolerant 能力，但容错主要体现在原始/规则解析路径，不应泛化成所有类型都能随意降级恢复。

## 与旧文档的差异

- `.bifrost` 目前既用于规则文件，也用于管理端导入导出；“全面替代所有旧 JSON 存储”不宜写成已经完成的事实。
- 旧文档未覆盖 decode script、template group 等当前真实字段。

## 建议

- 如果继续维护本文件，应该直接以以下实现为准补全字段说明：
  - [`crates/bifrost-core/src/bifrost_file/types.rs`](../crates/bifrost-core/src/bifrost_file/types.rs)
  - [`crates/bifrost-core/src/bifrost_file/parser.rs`](../crates/bifrost-core/src/bifrost_file/parser.rs)
  - [`crates/bifrost-core/src/bifrost_file/writer.rs`](../crates/bifrost-core/src/bifrost_file/writer.rs)
