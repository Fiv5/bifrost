# Web Admin Rules 列表树状视图（按 `/` 分组）

## 背景
Web Admin 的 Rules 页面左侧列表当前为扁平列表。当规则名包含 `/`（如 `A/B/C`）时，用户希望像文件夹一样按层级折叠/展开，提升管理体验。

## 目标
- 将 Rules 左侧列表从扁平展示升级为**树状（文件夹）结构**。
- 规则名按 `/` 分割：`A/B/C` 展示为文件夹 `A` → 子文件夹 `B` → 叶子节点 `C`。
- 保持原有交互在树状结构下依然可用：选中、上下键切换、搜索、排序、启用/禁用、右键菜单（导出/重命名/删除）、新建/刷新等。

## 非目标
- 不改变规则名称本身的语义与存储（仅 UI 展示层分组）。
- 不新增新的规则重命名/移动能力（例如拖拽修改路径）。

## 方案概述
### 1) 数据结构转换
新增纯前端转换逻辑：将 `RuleFile[]`（已按当前 sort/search 计算后的 `filteredRules`）转换为树形结构。

- 新增文件：`web/src/pages/Rules/RuleList/ruleTree.ts`
- 核心规则：
  - `splitRulePath(name)`：按 `/` 拆分并过滤空 segment（如 `A//B` 视为 `A/B`）。
  - `buildRuleTree(rules)`：按 `filteredRules` 的顺序构建 tree，保证列表顺序与原有排序策略一致。

### 2) 展示与交互
- Rules 列表渲染时递归渲染 folder/leaf 两类节点。
- **默认展开**所有 folder（尽量保持“打开即看全”的体验），用户可点击 folder 行进行折叠/展开。
- 当选中某个叶子规则时，自动展开其父路径，确保选中项可见。
- 当用户输入搜索关键词时，自动展开所有 folder，保证匹配结果可见。

### 3) 可访问性/键盘导航
继续保留原 `listbox` 语义，并将上下键导航的目标从 `filteredRules` 改为“当前可见叶子节点”的线性序列（受折叠状态影响）。

## 影响范围
- `web/src/pages/Rules/RuleList/index.tsx`
- `web/src/pages/Rules/RuleList/index.module.css`
- 新增：`web/src/pages/Rules/RuleList/ruleTree.ts`
- E2E：`web/tests/ui/admin-rules-values.spec.ts`

## 验证计划
### 单元测试
当前 `web/` 子项目未引入单元测试框架（如 Vitest/Jest），本次不新增测试框架以避免扩大工程面；tree 构建逻辑通过 E2E + 手动验证覆盖。

### E2E 测试
- 新增用例：`Rules 列表支持按 / 分组的树状展开/折叠`
  - 创建规则：`{folder}/a-child`、`{folder}/b-child`、`top`
  - 断言 folder 行存在
  - 断言折叠后子规则不可见，展开后恢复可见

### 真实场景测试（手动）
- 进入 Rules 页面，创建若干带 `/` 的规则名：
  - `teamA/serviceA/login`
  - `teamA/serviceA/logout`
  - `teamB/serviceB/*`
- 验证：
  - 默认展开、折叠/展开行为正确
  - 点击叶子节点可加载右侧 RuleEditor
  - 右键菜单（导出/删除/重命名）可用
  - Switch 启用/禁用可用
  - 搜索时树自动展开且结果可见

## 回滚方案
本次变更仅涉及前端 UI 展示逻辑；回滚方式为恢复 `RuleList` 渲染为原扁平 `map(filteredRules)` 实现，并删除新增 `ruleTree.ts` 与对应样式/测试。