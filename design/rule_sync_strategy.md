# Rule Sync Strategy

## 目标

在远端服务不可修改的前提下，为 rule 同步定义一套简单、稳定、可落地的策略，避免出现：

- 本地删除后又被远端同步回来
- 多端同时编辑后出现莫名其妙的创建/更新/删除失败
- 同名规则被误判为同一对象
- 状态越补越复杂，最后谁也说不清该覆盖谁

本方案明确选择：

- 不做冲突管理
- 不引入服务端版本协商
- 直接覆盖

换句话说，我们接受“最终一致 + 明确优先级”，不追求强一致。

## 总体原则

1. 远端只当对象存储，不承担并发协调职责。
2. 活规则的同步状态写在 rule 文件里。
3. 删除意图写在 `sync-state.json` 的 tombstone 里。
4. 同步主键优先使用 `remote_id`，不是 `name`。
5. 不做 conflict 状态，不做人工合并。
6. 同步优先级为：
   - 删除优先
   - 本地修改优先
   - 本地未修改时才接受远端覆盖

## 数据模型

### 规则文件

每个规则文件保存以下同步元数据：

```toml
[meta.sync]
rule_id = "rl_xxx"
status = "local_only" # local_only | synced | modified
last_synced_at = "2026-03-25T10:00:00Z"
origin_device_id = "dev_macbook_01"
remote_id = "123456"
remote_user_id = "u_abc"
last_synced_remote_updated_at = "2026-03-25T10:00:00Z"
last_synced_content_hash = "sha256:xxxx"
remote_created_at = "2026-03-25T09:00:00Z"
remote_updated_at = "2026-03-25T10:00:00Z"
```

字段说明：

- `rule_id`
  - 本地稳定 ID，创建时生成，rename 不变
- `status`
  - `local_only`：只有本地有，还没绑定远端
  - `synced`：上次同步后，本地与远端一致
  - `modified`：本地有未同步改动
- `remote_id`
  - 已绑定的远端对象 ID
- `last_synced_remote_updated_at`
  - 上次确认同步时看到的远端更新时间
- `last_synced_content_hash`
  - 上次确认同步时远端内容 hash

这里不引入 `conflict`，因为策略已经定为“直接覆盖”，不走冲突分支。

### sync-state.json

`sync-state.json` 只保存：

1. session
2. tombstone

```json
{
  "token": "...",
  "user": { "...": "..." },
  "last_sync_at": "2026-03-25T10:00:00Z",
  "last_sync_action": "bidirectional",
  "deleted_rules": {
    "rl_xxx": {
      "rule_id": "rl_xxx",
      "rule_name": "demo",
      "remote_id": "123456",
      "remote_user_id": "u_abc",
      "base_remote_updated_at": "2026-03-25T10:00:00Z",
      "base_content_hash": "sha256:xxxx",
      "deleted_at": "2026-03-25T10:01:00Z",
      "device_id": "dev_macbook_01"
    }
  }
}
```

为什么 tombstone 还必须存在：

- rule 文件删掉后，活对象元数据也一起没了
- 不保留 tombstone，就无法区分：
  - 这是本地明确删除
  - 这是本地暂时还没拉到远端对象

## 远端限制

远端服务不可修改，意味着：

- 没有 version
- 没有 CAS
- 不能做 `If-Match`
- 不能做服务端冲突判断

所以客户端只能依赖：

- `remote_id`
- `name`
- `rule`
- `create_time`
- `update_time`

在这个前提下，客户端不能做严格并发控制，只能做保守的覆盖策略。

## 最终同步语义

### 1. 删除优先

只要某个 `rule_id` 有 tombstone：

- 优先删远端
- tombstone 未清除前，不允许把同名远端对象重新拉回本地
- tombstone 未清除前，不允许把同 `remote_id` 的对象当成新对象同步

这是为了彻底解决“本地删除后远端又被拉回来”的问题。

### 2. 本地修改优先

如果本地 rule 是 `modified`：

- 无论远端是否也更新过，都直接用本地内容覆盖远端
- update 成功后，状态回到 `synced`

这就是“不要管理冲突，直接覆盖”的落地语义。

### 3. 本地未修改时接受远端覆盖

如果本地 rule 是 `synced`，且远端内容变化：

- 直接用远端覆盖本地
- 更新本地同步快照

这意味着：

- 本地没动时，远端优先
- 本地动过时，本地优先

### 4. 本地新建直接创建远端

如果本地 rule 是 `local_only`：

- 直接 create
- 成功后写回 `remote_id` 和同步快照

如果远端因同名创建失败：

- 不做复杂冲突管理
- 当前策略直接报错给用户
- 用户改名后再创建

## 同步流程

同步必须严格按顺序处理：

1. 加载本地 rule 文件
2. 加载 tombstone
3. 拉取远端规则
4. 优先处理 tombstone
5. 再处理活规则
6. 持久化状态

### 第一步：处理 tombstone

对每个 tombstone：

1. 如果远端对象（按 `remote_id` 精确匹配）不存在，立即移除 tombstone
2. 如果远端对象存在，直接请求删除
3. 删除失败则保留 tombstone，等待下次重试
4. 超过 7 天的 tombstone 自动过期清除

注意：tombstone 按 `remote_id` 精确匹配，不再按 `rule_name` 宽泛匹配所有同名远端对象。

也就是说：

- 只要 tombstone 在，就持续按 `remote_id` 删除目标远端对象
- 直到删成功或远端消失或过期

### 第二步：处理活规则

#### 本地存在，远端不存在

- `status = local_only`
  - 直接 create 到远端
- `status = modified` 且已有 `remote_id`
  - 本地有未同步修改，远端对象消失
  - 直接重新 create（保留本地修改优先语义）
- `status = synced` 且已有 `remote_id`
  - 远端对象消失，说明被其他端删除
  - **直接删除本地副本**（尊重远端删除意图）
  - 这是解决多端删除冲突的关键：synced 状态表示本地未修改，远端消失即意味着应同步删除

#### 本地存在，远端也存在

- `status = modified`
  - 直接 update 远端
- `status = synced`
  - 如果远端 hash 或更新时间变化，直接 pull 覆盖本地
  - 否则不动
- `status = local_only`
  - 若未绑定 `remote_id`，优先 create
  - 若因异常已有 `remote_id`，按 `modified` 处理，直接 update

#### 本地不存在，远端存在

- 若无 tombstone
  - 直接 pull 到本地
- 若有 tombstone
  - 跳过 pull，继续删远端

## 多端边界策略

### A 删除，B 修改

策略：

- A 写 tombstone，持续删远端
- B 若稍后同步，会把本地修改重新推上去
- 最终结果取决于谁最后一次同步成功

这是“直接覆盖”策略下的自然结果，不做额外冲突治理。

### A 与 B 同时修改

策略：

- 谁后同步，谁覆盖
- 不做人工干预

### A 本地删除，B 本地新建同名

策略：

- A 由 tombstone 删除旧远端对象
- B 若创建同名规则成功，则绑定新对象
- 若因远端同名限制失败，则提示用户改名

### A rename，B edit

策略：

- rename 视为一次普通 update
- 谁后同步，谁覆盖

### 远端被其他端删掉

策略：

- 若本地是 `synced` 状态（未修改），直接删除本地副本
- 若本地是 `modified` 状态（有未同步修改），重新 create 到远端
- 若本地也删了且有 tombstone，继续保持删除语义

### Tombstone 精确匹配

策略：

- Tombstone 按 `remote_id` 精确匹配远端对象，不再按 `rule_name` 宽泛匹配
- 避免误删其他端创建的同名但不同 ID 的新规则
- Tombstone 的 `rule_name` 仅用于阻止从远端拉取同名规则

### Tombstone 生命周期管理

策略：

- 远端对象按 `remote_id` 查找不到时，立即清除 tombstone（目的已达成）
- Tombstone 超过 7 天自动过期清除，避免永久累积
- 远端删除成功后立即清除对应 tombstone

## 失败处理

### 创建失败

- 常见原因：同名冲突、网络错误
- 处理：
  - 网络错误：保留 `local_only`，下次重试
  - 同名冲突：直接报错，不做自动合并

### 更新失败

- 网络错误：保留 `modified`
- 下次继续重试

### 删除失败

- 保留 tombstone
- 下次继续删

### 远端返回异常数据

- 跳过本轮对象
- 保留本地状态
- 记录日志

## UI 建议

rule 编辑页顶部展示：

- Sync status
- Created at
- Updated at
- Last synced at
- Remote updated at

Sync 面板展示：

- pending tombstone 数量
- 最近一次同步时间
- 最近一次同步动作

不展示 conflict，因为没有 conflict。

## 最小落地顺序

建议按这个顺序做：

1. rule 文件补 `rule_id`
2. rule 文件补 `last_synced_content_hash`
3. tombstone 补 `base_remote_updated_at` 和 `base_content_hash`
4. 同步逻辑明确成：
   - tombstone 永远优先
   - modified 永远 push
   - synced 才允许 pull

## 最终结论

在远端不可改、且你明确要求“不做冲突管理，直接覆盖”的前提下，最佳策略就是：

- 删除优先
- 本地修改优先
- 本地未修改才接受远端覆盖
- tombstone 持续重试删除远端
- 所有并发问题都用“最后一次成功同步覆盖”解决

这不是最强一致的方案，但它是当前约束下最简单、最稳定、最不容易出现“莫名其妙卡死状态”的方案。
