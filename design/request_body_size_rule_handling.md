# 请求体大小判断规则

## 现状结论

这项改造已经实现，HTTP 与 tunnel 两条链路都已经把“body override”和“需要读取原 body 才能处理”的情况拆开。

## 当前实现

- 请求侧：
  - `has_req_body_override = resolved_rules.req_body.is_some()`
  - 只有在没有 override 且确实需要 body 规则/脚本时，才会读取请求体。
- 响应侧：
  - `has_res_body_override = resolved_rules.res_body.is_some()`
  - 只有需要处理响应体且没有 override 时，才会走 bounded read / probe read。
- 对大体积或疑似流式 body：
  - 通过 `max_body_buffer_size` 和 `max_body_probe_size` 控制预读；
  - 超限时跳过 body 规则与脚本，改走流式转发。

## 当前语义

- `req_body` / `res_body` 这类直接替换规则，不会因为原 body 太大而失效。
- 依赖读取原 body 的 replace / prepend / append / merge / scripts 仍会在超限时被跳过。

## 适用范围

- 普通 HTTP 请求处理链路。
- HTTPS tunnel / H3 相关响应处理链路。
