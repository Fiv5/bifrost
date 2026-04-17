# StatusBar Proxy 状态 Hover 弹出面板

## 功能模块说明

底部状态栏的 "Proxy: Running/Stopped" 区域支持 hover 弹出 Popover 面板，展示系统代理开关（Switch）和代理地址信息，用户可快速切换系统代理开启/关闭状态。

## 前置条件

1. Bifrost 服务已启动：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 前端 dev server 已启动（或直接访问嵌入的管理端）：
   ```bash
   cd web && BACKEND_PORT=8800 npx vite
   ```
3. 浏览器打开管理端页面

## 测试用例

### TC-SPP-01: Proxy 状态区域显示 cursor pointer

- **操作步骤**：在浏览器中打开管理端，观察底部状态栏 "Proxy:" 区域
- **预期结果**：鼠标移入时光标变为 pointer 手型

### TC-SPP-02: Hover 弹出 Popover 面板

- **操作步骤**：将鼠标悬停在底部状态栏 "Proxy: Running" 或 "Proxy: Stopped" 区域
- **预期结果**：
  - 向上弹出 Popover 面板（无箭头）
  - 面板内包含 "System Proxy" 文字和一个 Switch 开关
  - Switch 状态与当前系统代理状态一致

### TC-SPP-03: 通过 Popover 开启系统代理

- **操作步骤**：
  1. 确保系统代理处于关闭状态（底部状态栏显示 "Proxy: Stopped"）
  2. Hover "Proxy: Stopped" 区域
  3. 点击弹出面板中的 Switch 开关
- **预期结果**：
  - Switch 切换为开启状态
  - 底部状态栏更新为 "Proxy: Running"，状态圆点变为绿色
  - Popover 面板中显示代理地址（如 `127.0.0.1:8800`）

### TC-SPP-04: 通过 Popover 关闭系统代理

- **操作步骤**：
  1. 确保系统代理处于开启状态（底部状态栏显示 "Proxy: Running"）
  2. Hover "Proxy: Running" 区域
  3. 点击弹出面板中的 Switch 开关
- **预期结果**：
  - Switch 切换为关闭状态
  - 底部状态栏更新为 "Proxy: Stopped"，状态圆点变为灰色
  - Popover 面板中不再显示代理地址

### TC-SPP-05: 系统代理不支持时的 Popover 显示

- **操作步骤**：在不支持系统代理的平台上 hover "Proxy:" 区域
- **预期结果**：Popover 面板显示 "System proxy is not supported on this platform"，无 Switch 开关

### TC-SPP-06: Popover 与 Settings 页面状态同步

- **操作步骤**：
  1. 通过 Popover 开启系统代理
  2. 打开 Settings 页面的 Proxy Tab
  3. 检查 "Enable System Proxy" Switch 状态
- **预期结果**：Settings 页面的 Switch 状态与 Popover 中的操作结果一致

## 清理步骤

1. 关闭系统代理（如已开启）
2. 停止 Bifrost 服务
3. 清理临时数据目录：`rm -rf .bifrost-test`
