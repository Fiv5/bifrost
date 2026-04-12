# Web UI 搜索模式功能测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 在浏览器中打开 `http://127.0.0.1:8800/_bifrost/traffic`
3. 确保已有若干流量记录（可通过配置系统代理或手动发起 HTTP 请求产生流量）

---

## 测试用例

### TC-WSE-01：进入搜索模式

**操作步骤**：
1. 在 Traffic 页面的 FilterBar 区域，点击右侧 "Fuzzy Search" 按钮

**预期结果**：
- 流量列表区域切换为搜索模式界面
- 搜索模式顶部显示搜索输入框，placeholder 为 "Enter keyword to search all content..."
- 搜索输入框右侧显示 "Search" 按钮和 "Exit" 按钮
- 搜索框下方显示搜索范围复选框行，包含：All、URL、Req Headers、Res Headers、Req Body、Res Body、WS Messages、SSE Events
- "All" 复选框默认选中
- "Fuzzy Search" 按钮变为激活状态（type="primary"）

---

### TC-WSE-02：关键词搜索

**前置条件**：已产生包含特定关键词的流量（例如访问过 `http://example.com`）

**操作步骤**：
1. 进入搜索模式
2. 在搜索输入框中输入关键词 "example"
3. 点击 "Search" 按钮（或按回车键）

**预期结果**：
- 搜索过程中显示 "Searching..." 加载状态和已搜索记录数
- 搜索完成后，统计栏显示 "Found N matches (searched M records)"
- 搜索结果列表中显示匹配的流量记录
- 每条搜索结果包含流量摘要信息（URL、方法、状态码等）
- 如果匹配数量较多且有更多结果，显示 "Load More" 按钮

---

### TC-WSE-03：搜索范围 — 使用 Method 过滤

**前置条件**：已产生多种 HTTP 方法的流量（GET、POST 等）

**操作步骤**：
1. 进入搜索模式
2. 退出搜索模式，在 FilterBar 中添加一个过滤条件：Field 选择 "Method"，Operator 选择 "Equals"，Value 输入 "POST"
3. 重新点击 "Fuzzy Search" 按钮进入搜索模式
4. 输入关键词并点击 "Search"

**预期结果**：
- 搜索结果仅包含 HTTP 方法为 POST 的流量记录
- FilterBar 中的过滤条件在搜索模式下仍然生效
- 搜索请求中包含 Method 过滤条件

---

### TC-WSE-04：搜索范围 — 使用 Host 过滤

**前置条件**：已产生针对不同 Host 的流量

**操作步骤**：
1. 进入搜索模式
2. 退出搜索模式，在 FilterBar 中添加一个过滤条件：Field 选择 "Host"，Operator 选择 "Contains"，Value 输入目标域名（如 "example.com"）
3. 重新点击 "Fuzzy Search" 按钮进入搜索模式
4. 输入关键词并点击 "Search"

**预期结果**：
- 搜索结果仅包含 Host 匹配的流量记录
- Host 过滤条件作为搜索请求的 conditions 参数传递

---

### TC-WSE-05：搜索结果列表显示

**前置条件**：已执行搜索并有匹配结果

**操作步骤**：
1. 执行一次有结果的关键词搜索
2. 观察搜索结果列表

**预期结果**：
- 搜索结果以列表形式展示
- 每条结果包含流量的基本信息（HTTP 方法、URL、状态码等）
- 每条结果显示匹配位置的预览文本片段
- 统计栏显示匹配总数和已搜索记录数
- 列表支持滚动浏览
- 无结果时显示 "No results found. Try a different keyword."

---

### TC-WSE-06：点击搜索结果跳转到流量详情

**前置条件**：已执行搜索并有匹配结果

**操作步骤**：
1. 在搜索结果列表中单击一条结果

**预期结果**：
- 右侧详情面板（如果展开）显示被点击流量的完整详情
- 被点击的结果在列表中高亮显示（selectedId 匹配）
- 双击一条结果时，如果详情面板已折叠，会自动展开详情面板

---

### TC-WSE-07：搜索结果关键词高亮

**前置条件**：已执行关键词搜索并有匹配结果

**操作步骤**：
1. 输入关键词（如 "example"）执行搜索
2. 观察搜索结果列表中的匹配预览文本

**预期结果**：
- 匹配的关键词在预览文本中以高亮背景色标注（backgroundColor 为 `token.colorWarningBg`）
- 高亮匹配不区分大小写
- 多处匹配时每处都有高亮

---

### TC-WSE-08：退出搜索模式

**前置条件**：当前处于搜索模式

**操作步骤**：
1. 点击搜索区域右上角的 "Exit" 按钮

**预期结果**：
- 搜索模式关闭，恢复为正常的流量列表视图
- "Fuzzy Search" 按钮恢复为非激活状态（type="default"）
- 之前设置的 FilterBar 过滤条件和 Toolbar 过滤标签仍然保留
- URL 中的 `search` 参数被清除

---

### TC-WSE-09：搜索范围复选框切换

**操作步骤**：
1. 进入搜索模式
2. 取消勾选 "All" 复选框
3. 勾选 "URL" 和 "Req Body" 复选框
4. 输入关键词并执行搜索

**预期结果**：
- "All" 取消后，搜索范围缩小至所选范围
- 仅搜索 URL 和请求体中的内容
- 搜索结果仅反映所选范围的匹配
- URL 中的 `search` 参数正确记录当前搜索范围

---

### TC-WSE-10：搜索状态持久化到 URL

**操作步骤**：
1. 进入搜索模式
2. 输入关键词 "test"
3. 勾选特定搜索范围
4. 观察浏览器地址栏 URL
5. 复制 URL 并在新标签页中打开

**预期结果**：
- URL 中包含 `search=...` 参数，编码了搜索模式、关键词和搜索范围
- 在新标签页中打开该 URL 后，自动恢复搜索模式、关键词和搜索范围设置
- 搜索界面状态与原始页面一致

---

### TC-WSE-11：取消正在进行的搜索

**操作步骤**：
1. 进入搜索模式
2. 输入关键词并点击 "Search"
3. 在搜索进行过程中（"Searching..." 状态），点击 "Stop" 按钮

**预期结果**：
- 搜索立即停止
- 已获取的搜索结果保留在列表中
- "Searching..." 状态消失
- "Stop" 按钮消失

---

### TC-WSE-12：空关键词搜索提示

**操作步骤**：
1. 进入搜索模式
2. 不输入任何关键词
3. 观察搜索区域

**预期结果**：
- 结果区域显示空状态提示 "Enter a keyword to search all traffic content."
- "Search" 按钮在无关键词且无其他过滤条件时为禁用状态（disabled）

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
