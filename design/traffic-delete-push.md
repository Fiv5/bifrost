# Traffic 删除推送与清理

## 现状结论

该设计已经实现。

## 当前实现

- 服务端 push 已支持 `traffic_deleted`。
- 删除流量记录后会广播被删除的 `ids`。
- 前端 `useTrafficStore` 会：
  - 从列表移除对应记录；
  - 清理选中项；
  - 若当前详情正好被删，则把 `detailError` 设为 `Request was deleted`。
- 服务端删除路径已经纳入 body / frame / ws payload 等关联文件清理。

## 文档结论

- 这份设计不再是提案，可视为“当前行为说明”。
- 若后续要补充，重点应放在删除边界条件和清理失败补偿，而不是基础推送能力本身。
