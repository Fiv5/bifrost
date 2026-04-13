# Web UI Groups 页面测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保已开启 Sync 功能（Groups 页面仅在 `syncStatus.enabled` 为 `true` 时显示）
3. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/`

---

## 测试用例

### TC-WGR-01：访问 Groups 页面

**操作步骤**：
1. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/groups`

**预期结果**：
- 页面正常加载，显示 Groups 页面
- 页面顶部显示蓝色提示信息："Groups allow team members to share proxy rules and collaborate efficiently. Create or join a group to sync rules across your team."
- 页面中间显示搜索框，placeholder 为 "Search groups by name..."
- 页面包含 "Managed" 分区标题

---

### TC-WGR-02：Group 列表显示

**前置条件**：已创建至少一个 Group

**操作步骤**：
1. 打开 `http://127.0.0.1:8800/_bifrost/groups`
2. 观察 Group 卡片列表

**预期结果**：
- Group 以卡片网格形式展示（响应式布局：xs=24, sm=12, md=8, lg=6）
- 每个 Group 卡片包含：
  - 左侧圆形头像（显示 Group 名称首字母大写）
  - Group 名称（单行省略显示）
  - 可见性标签：Public（绿色）或 Private（默认色）
  - 角色标签：Owner（金色）、Master（蓝色）、Member（默认色）
  - 描述文本（单行省略显示，无描述时显示 "No description"）
- "Managed" 分区的第一个位置显示 "Create Group" 虚线卡片（带 + 图标）
- 如有 Joined 分组的 Group，显示 "Joined" 分区
- 如有可发现的公开 Group，显示 "Discover" 分区
- 如果没有任何 Group，显示 "No groups yet, create one to get started" 空状态提示

---

### TC-WGR-03：搜索 Group

**前置条件**：已创建多个 Group

**操作步骤**：
1. 在 Groups 页面的搜索框中输入一个已存在 Group 的名称
2. 点击搜索按钮或按回车

**预期结果**：
- 搜索结果仅显示名称匹配的 Group 卡片
- 搜索模式下不显示 "Managed"、"Joined"、"Discover" 分区标题
- 无匹配结果时显示 "No groups found" 空状态
- 清空搜索框后恢复完整列表（含分区标题）

---

### TC-WGR-04：创建 Group

**操作步骤**：
1. 在 Groups 页面点击 "Create Group" 虚线卡片
2. 在弹出的 "Create Group" 对话框中：
   - Name 输入 "Test Group"
   - Description 输入 "A test group for testing"
   - Visibility 选择 "Private"
3. 点击 "OK" 按钮

**预期结果**：
- 显示 Toast 消息 "Group created"
- 自动跳转到新创建的 Group 详情页（URL 为 `/groups/{group_id}`）
- Group 详情页显示正确的名称、描述和可见性

---

### TC-WGR-05：查看 Group 详情

**前置条件**：已通过 TC-WGR-04 创建 Group

**操作步骤**：
1. 在 Groups 列表页点击一个 Group 卡片

**预期结果**：
- 跳转到 Group 详情页（URL 为 `/groups/{group_id}`）
- 页面顶部显示 "Back" 返回按钮
- Group 信息卡片包含：
  - 圆形头像（名称首字母大写）
  - Group 名称
  - 可见性标签（Public 绿色 / Private 橙色）
  - 角色标签（Owner / Master / Member）
  - 描述文本（无描述时显示 "No description"）
  - 创建时间和更新时间（格式 YYYY-MM-DD HH:mm:ss）
- 如果当前用户是 Master 或 Owner，显示 "Edit" 按钮
- 如果当前用户是 Owner，显示 "Delete" 按钮
- 下方显示 "Members (N)" 标题和成员列表

---

### TC-WGR-06：查看 Group 规则（通过 Rules 页面）

**前置条件**：已创建 Group 且该 Group 中有规则

**操作步骤**：
1. 打开 `http://127.0.0.1:8800/_bifrost/rules?group={group_id}`
2. 观察 Rules 页面的 Group 切换器

**预期结果**：
- Rules 页面顶部显示 Group 切换下拉框
- 下拉框中包含 "My Rules" 和已加入的 Group 列表
- 选择目标 Group 后，规则列表切换为该 Group 的规则
- 如果当前用户对该 Group 有写权限（Master/Owner），可以编辑规则
- 如果是只读成员（Member），规则列表为只读模式

---

### TC-WGR-07：创建 Group 规则

**前置条件**：当前用户是 Group 的 Master 或 Owner

**操作步骤**：
1. 在 Rules 页面切换到目标 Group
2. 点击创建规则按钮
3. 输入规则名称和规则内容
4. 保存规则

**预期结果**：
- 规则创建成功
- 规则列表中显示新创建的规则
- 规则带有正确的 Group 归属

---

### TC-WGR-08：编辑 Group 规则

**前置条件**：当前用户是 Group 的 Master 或 Owner，Group 中已有规则

**操作步骤**：
1. 在 Rules 页面切换到目标 Group
2. 选择一条已有规则
3. 修改规则内容
4. 保存修改

**预期结果**：
- 规则更新成功
- 规则列表中显示更新后的内容

---

### TC-WGR-09：启用/禁用 Group 规则

**前置条件**：当前用户是 Group 的 Master 或 Owner，Group 中已有规则

**操作步骤**：
1. 在 Rules 页面切换到目标 Group
2. 对一条已启用的规则执行禁用操作
3. 确认规则状态变更
4. 再次对该规则执行启用操作

**预期结果**：
- 禁用操作后，规则状态变为已禁用
- 启用操作后，规则状态变为已启用
- 规则列表中的状态指示器正确反映当前状态

---

### TC-WGR-10：删除 Group 规则

**前置条件**：当前用户是 Group 的 Master 或 Owner，Group 中已有规则

**操作步骤**：
1. 在 Rules 页面切换到目标 Group
2. 选择一条规则，执行删除操作
3. 在确认对话框中确认删除

**预期结果**：
- 规则从列表中移除
- 规则被成功删除，刷新后不再出现

---

### TC-WGR-11：编辑 Group 信息

**前置条件**：当前用户是 Group 的 Master 或 Owner

**操作步骤**：
1. 在 Group 详情页点击 "Edit" 按钮
2. 在弹出的 "Edit Group" 对话框中：
   - 修改 Name 为 "Updated Group Name"
   - 修改 Description 为 "Updated description"
   - 修改 Visibility 为 "Public"
3. 点击 "OK" 按钮

**预期结果**：
- 显示 Toast 消息 "Group updated"
- Group 详情页信息更新为新的名称、描述和可见性
- 可见性标签从 "Private"（橙色）变为 "Public"（绿色）

---

### TC-WGR-12：删除 Group

**前置条件**：当前用户是 Group 的 Owner

**操作步骤**：
1. 在 Group 详情页点击 "Delete" 按钮
2. 在确认弹窗中点击 "Delete"

**预期结果**：
- 显示 Toast 消息 "Group deleted"
- 自动跳转回 Groups 列表页（`/groups`）
- 已删除的 Group 不再出现在列表中

---

### TC-WGR-13：只读成员无法编辑 Group 信息

**前置条件**：当前用户是 Group 的 Member（level=0）

**操作步骤**：
1. 在 Groups 列表页点击一个角色为 Member 的 Group 卡片
2. 观察 Group 详情页

**预期结果**：
- 详情页不显示 "Edit" 按钮
- 详情页不显示 "Delete" 按钮
- 显示 "Leave" 按钮

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
