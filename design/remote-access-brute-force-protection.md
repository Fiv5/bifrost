# 远程访问暴力破解防护

## 功能模块详细描述

为远程管理访问增加暴力破解防护机制，防止攻击者通过穷举密码方式获取管理权限。

### 核心机制

1. **登录失败计数**：基于 IP 跟踪连续登录失败次数
2. **自动锁定**：达到 5 次失败后，自动停用远程登录并删除密码
3. **手动恢复**：锁定后必须从本机（loopback）重新设置密码并开启远程访问
4. **密码强度要求**：设置密码时强制至少 6 字符

### 安全增强

- 失败登录的 IP 和时间记录到审计日志
- 锁定事件记录到日志和审计
- 登录失败响应返回剩余尝试次数
- 锁定后的请求返回 HTTP 403 + 锁定原因说明
- AuthStatus API 返回锁定状态，前端可展示提示

## 实现逻辑

### 后端变更

#### 1. `admin_auth.rs` - 登录失败计数与锁定

- 新增 Values Storage key：`admin.auth.login_failed_count` 记录连续失败次数
- 常量 `MAX_LOGIN_ATTEMPTS = 5`
- 新函数：
  - `record_failed_login(state) -> Result<u32>`: 失败计数 +1，达到阈值时自动 lockout
  - `reset_failed_login_count(state) -> Result<()>`: 重置失败计数（成功登录后调用）
  - `get_failed_login_count(state) -> u32`: 获取当前失败次数
  - `execute_lockout(state) -> Result<()>`: 停用远程访问 + 删除密码 hash + 撤销所有会话 + 重置计数
  - `validate_password_strength(password) -> Result<()>`: 校验密码强度

#### 2. `handlers/auth.rs` - 登录接口变更

- `/api/auth/login`：
  - 登录失败时调用 `record_failed_login`
  - 返回 `remaining_attempts` 字段
  - 达到阈值后返回 403 + lockout 信息
  - 成功登录后调用 `reset_failed_login_count`
- `/api/auth/status`：
  - 新增 `locked_out` 布尔字段
  - 新增 `failed_attempts` 字段
- `/api/auth/passwd`：
  - 调用 `validate_password_strength` 校验密码强度
  - 返回明确错误信息

### 前端变更

#### 1. `Login.tsx`
- 登录失败时显示剩余尝试次数
- 锁定后显示锁定提示信息

#### 2. `RemoteAccessTab.tsx`
- 显示锁定状态
- 密码输入时校验强度提示

#### 3. `adminAuth.ts`
- `AdminAuthStatus` 类型新增 `locked_out` 和 `failed_attempts` 字段

## 依赖项

- 无新增外部依赖
- 使用现有 ValuesStorage 持久化失败计数

## 测试方案

### 单元测试

- `test_record_failed_login_increments_count`: 验证失败次数递增
- `test_lockout_after_max_failures`: 验证达 5 次后自动锁定
- `test_lockout_clears_password_and_disables_remote`: 验证锁定后密码被删除、远程访问被禁用
- `test_reset_failed_count_on_success`: 验证成功登录重置计数
- `test_password_strength_rejects_short`: 验证 < 6 字符被拒绝
- `test_password_strength_accepts_valid`: 验证合法密码通过
- `test_lockout_resets_after_re_enable`: 验证本机重新设置后状态恢复

### E2E 测试

- 验证 5 次错误登录后返回锁定状态（403）
- 验证密码强度校验拒绝弱密码（400）
- 验证锁定后本机可重新设置密码并启用远程访问

### 真实场景测试

- 在 `human_tests/remote-access-brute-force-protection.md` 创建测试用例
