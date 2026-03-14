# 管理端 Performance 设置

## 现状结论

“延迟提交草稿”方案已经落地，而且范围比旧文档更大。

## 当前实现

- `Settings` 页维护本地 `trafficDraft`，滑动时先更新本地显示。
- 各字段通过独立的 600ms 防抖计时器提交到 `PUT /api/config/performance`。
- 提交失败时会回退到最近一次服务端配置。
- 当前 Performance 面板包含：
  - `Max Records`
  - `Max DB Size`
  - `Max Body Inline Size (DB)`
  - `Max Body Buffer Size`
  - `Max Body Probe Size`
  - `File Retention Days`
  - `Clear Cache`

## 与旧文档的差异

- `Max Records` 上限已经不是旧文档中的 50000；当前 UI 文案与实现以 100000 为上限。
- 设计范围已经从单纯“滑条防抖”扩展为一组流量/存储参数的统一草稿编辑体验。

## 结论

- 该方案已实现，文档不应再停留在单一交互优化层面。
- 当前更准确的定位是“Performance 配置草稿态 + 分字段防抖提交”。
