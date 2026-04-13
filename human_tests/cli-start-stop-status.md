# CLI 服务管理（start / stop / status）测试用例

## 功能模块说明

本文档覆盖 Bifrost CLI 的核心服务管理功能，包括：
- `bifrost start`：启动代理服务（含各种参数组合）
- `bifrost stop`：停止代理服务
- `bifrost status`：查看服务状态（含 TUI 模式）
- `-v`：版本信息查看

## 前置条件

1. 确保项目已编译或可编译：
   ```bash
   cd /path/to/bifrost
   ```
2. 确保端口 8800、8801、8802 未被占用
3. 确保无正在运行的 Bifrost 测试实例（可先执行 `cargo run --bin bifrost -- stop`）
4. 准备一个规则文件用于 `--rules-file` 测试：
   ```bash
   echo "httpbin.org reqHeaders://(X-Bifrost-Test: 1)" > /tmp/bifrost-test-rules.txt
   ```
5. 所有启动命令统一使用临时数据目录，避免污染正式环境：
   ```bash
   export BIFROST_DATA_DIR=./.bifrost-test
   ```

---

## 测试用例

### TC-CSS-01：默认参数启动服务（前台模式）

**操作步骤**：
1. 执行以下命令启动服务：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 观察终端输出

**预期结果**：
- 终端输出包含启动成功信息，显示监听地址 `0.0.0.0:8800`
- 服务在前台运行，终端被占用
- 执行 `curl -x http://127.0.0.1:8800 http://httpbin.org/get` 返回正常 JSON 响应
- 按 Ctrl+C 可正常停止服务

---

### TC-CSS-02：指定自定义端口启动（-p 8801）

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8801 --unsafe-ssl
   ```
2. 使用 curl 验证代理功能

**预期结果**：
- 终端输出显示监听地址为 `0.0.0.0:8801`
- 执行 `curl -x http://127.0.0.1:8801 http://httpbin.org/get` 返回正常 JSON 响应
- 端口 8800 未被监听

---

### TC-CSS-03：后台守护进程模式启动（-d / --daemon）

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 -d --unsafe-ssl
   ```
2. 观察终端输出
3. 检查进程是否在后台运行

**预期结果**：
- 命令执行后终端立即返回（不阻塞）
- 输出包含类似 "Proxy started in daemon mode" 或显示 PID 的信息
- 执行 `curl -x http://127.0.0.1:8800 http://httpbin.org/get` 返回正常响应
- `ps aux | grep bifrost` 可以看到后台进程

**清理**：
```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
```

---

### TC-CSS-04：启用 --unsafe-ssl 跳过上游 TLS 验证

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 通过代理请求一个 HTTPS 网站：
   ```bash
   curl -x http://127.0.0.1:8800 https://httpbin.org/get -k
   ```

**预期结果**：
- 服务正常启动
- HTTPS 请求通过代理成功完成，返回正常 JSON 响应
- 不会因为上游 TLS 证书问题而报错

---

### TC-CSS-05：使用 --no-intercept 禁用 TLS 拦截

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --no-intercept
   ```
2. 通过代理请求 HTTPS 网站：
   ```bash
   curl -x http://127.0.0.1:8800 https://httpbin.org/get -k
   ```

**预期结果**：
- 服务正常启动，日志中显示 TLS 拦截已禁用
- HTTPS 请求以 CONNECT 隧道方式通过，代理不解密内容
- 请求正常返回

---

### TC-CSS-06：使用 --intercept 启用 TLS 拦截

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --intercept
   ```
2. 通过代理请求 HTTPS 网站（需要信任 CA 或使用 -k）：
   ```bash
   curl -x http://127.0.0.1:8800 https://httpbin.org/get -k
   ```

**预期结果**：
- 服务正常启动，日志中显示 TLS 拦截已启用
- HTTPS 请求通过代理时被拦截解密，代理可以看到请求内容
- 请求正常返回 JSON 响应
- `--intercept` 和 `--no-intercept` 不可同时使用（CLI 会报错并拒绝启动）

---

### TC-CSS-07：使用 --rules 指定内联规则启动

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --rules "httpbin.org reqHeaders://(X-Bifrost: hello)"
   ```
2. 通过代理请求验证规则生效：
   ```bash
   curl -x http://127.0.0.1:8800 http://httpbin.org/headers
   ```

**预期结果**：
- 服务正常启动
- curl 响应的 headers 字段中包含 `"X-Bifrost": "hello"`，说明规则已生效
- 请求 httpbin.org 以外的域名不受该规则影响

---

### TC-CSS-08：使用 --rules-file 指定规则文件启动

**前置条件**：已创建 `/tmp/bifrost-test-rules.txt`，内容为 `httpbin.org reqHeaders://(X-Bifrost-Test: 1)`

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --rules-file /tmp/bifrost-test-rules.txt
   ```
2. 通过代理请求验证规则生效：
   ```bash
   curl -x http://127.0.0.1:8800 http://httpbin.org/headers
   ```

**预期结果**：
- 服务正常启动
- curl 响应的 headers 字段中包含 `"X-Bifrost-Test": "1"`，说明规则文件已被加载并生效

---

### TC-CSS-09：使用 --socks5-port 指定独立 SOCKS5 端口

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --socks5-port 8802
   ```
2. 通过 HTTP 代理验证：
   ```bash
   curl -x http://127.0.0.1:8800 http://httpbin.org/get
   ```
3. 通过 SOCKS5 代理验证：
   ```bash
   curl -x socks5://127.0.0.1:8802 http://httpbin.org/get
   ```

**预期结果**：
- 服务正常启动，日志中显示 HTTP 代理监听 8800 端口，SOCKS5 代理监听 8802 端口
- 步骤 2 通过 HTTP 代理正常返回响应
- 步骤 3 通过 SOCKS5 代理正常返回响应

---

### TC-CSS-10：使用 --allow-lan 允许局域网访问

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --allow-lan
   ```
2. 从本机使用局域网 IP 访问代理（假设局域网 IP 为 `192.168.x.x`）：
   ```bash
   curl -x http://192.168.x.x:8800 http://httpbin.org/get
   ```

**预期结果**：
- 服务正常启动
- 从局域网 IP 访问代理时请求成功（未被拒绝）
- 不使用 `--allow-lan` 时，局域网 IP 访问会被访问控制拒绝

---

### TC-CSS-11：使用 --proxy-user 设置代理认证

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --proxy-user "testuser:testpass"
   ```
2. 不带认证访问代理：
   ```bash
   curl -x http://127.0.0.1:8800 http://httpbin.org/get
   ```
3. 带正确认证访问代理：
   ```bash
   curl -x http://testuser:testpass@127.0.0.1:8800 http://httpbin.org/get
   ```
4. 带错误认证访问代理：
   ```bash
   curl -x http://testuser:wrongpass@127.0.0.1:8800 http://httpbin.org/get
   ```

**预期结果**：
- 服务正常启动
- 步骤 2：返回 HTTP 407 Proxy Authentication Required
- 步骤 3：正常返回 httpbin.org 的 JSON 响应
- 步骤 4：返回 HTTP 407 Proxy Authentication Required

---

### TC-CSS-12：查看服务状态（status 命令）

**前置条件**：服务已通过 TC-CSS-03 以 daemon 模式启动

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- status
   ```

**预期结果**：
- 输出包含代理服务的运行状态信息
- 显示监听端口（如 `8800`）
- 显示进程 PID
- 显示运行时长或启动时间
- 显示 TLS 拦截状态、规则数量等关键配置信息

---

### TC-CSS-13：服务未运行时查看状态

**前置条件**：确保 Bifrost 服务未在运行

**操作步骤**：
1. 先停止服务：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
   ```
2. 查看状态：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- status
   ```

**预期结果**：
- 输出提示服务未在运行（如 "Proxy is not running" 或类似信息）

---

### TC-CSS-14：TUI 仪表盘模式查看状态（status --tui）

**前置条件**：服务已以 daemon 模式运行

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- status --tui
   ```
2. 观察终端输出

**预期结果**：
- 显示交互式 TUI 仪表盘界面
- 界面包含实时的代理状态信息（连接数、流量统计等）
- 按 `q` 或 Ctrl+C 可退出 TUI 界面

---

### TC-CSS-15：停止服务（stop 命令）

**前置条件**：服务已以 daemon 模式运行

**操作步骤**：
1. 确认服务正在运行：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- status
   ```
2. 执行停止命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
   ```
3. 再次检查状态：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- status
   ```

**预期结果**：
- 步骤 1：显示服务正在运行
- 步骤 2：输出停止成功的消息（如 "Proxy stopped"）
- 步骤 3：显示服务未在运行
- 代理端口 8800 不再监听

---

### TC-CSS-16：服务未运行时执行 stop

**前置条件**：确保 Bifrost 服务未在运行

**操作步骤**：
1. 执行停止命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
   ```

**预期结果**：
- 输出提示服务未在运行（如 "Proxy is not running" 或类似信息）
- 不会报错或崩溃

---

### TC-CSS-17：服务已运行时再次启动（交互式重启提示）

**前置条件**：服务已以 daemon 模式运行

**操作步骤**：
1. 先以 daemon 模式启动服务：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 -d --unsafe-ssl
   ```
2. 再次执行启动命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 -d --unsafe-ssl
   ```
3. 当出现重启提示时，输入 `n` 拒绝

**预期结果**：
- 步骤 2 检测到已有 Bifrost 进程在运行
- 终端输出类似 "Detected an existing Bifrost proxy process (PID: xxx). Restart? (y/n)" 的提示
- 输入 `n` 后，保持原有进程继续运行，不做任何变更

**清理**：
```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
```

---

### TC-CSS-18：使用 -y 自动确认重启

**前置条件**：服务已以 daemon 模式运行

**操作步骤**：
1. 先以 daemon 模式启动服务：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 -d --unsafe-ssl
   ```
2. 使用 -y 参数再次启动：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 -d --unsafe-ssl -y
   ```

**预期结果**：
- 不出现交互式提示，自动停止旧进程并启动新进程
- 输出表明旧进程已被停止、新进程已启动
- 执行 `curl -x http://127.0.0.1:8800 http://httpbin.org/get` 新服务正常工作

**清理**：
```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
```

---

### TC-CSS-19：查看版本信息（-v）

**操作步骤**：
1. 执行以下命令：
   ```bash
   cargo run --bin bifrost -- -v
   ```

**预期结果**：
- 输出 Bifrost 版本号（格式如 `bifrost x.y.z`）
- 命令执行后立即退出，不启动服务

---

### TC-CSS-20：同时指定多个 --rules 参数

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl \
     --rules "httpbin.org reqHeaders://(X-First: 1)" \
     --rules "httpbin.org reqHeaders://(X-Second: 2)"
   ```
2. 通过代理验证两条规则均生效：
   ```bash
   curl -x http://127.0.0.1:8800 http://httpbin.org/headers
   ```

**预期结果**：
- 服务正常启动
- curl 响应的 headers 中同时包含 `"X-First": "1"` 和 `"X-Second": "2"`
- 多次指定 --rules 参数可叠加生效

---

### TC-CSS-21：同时指定多个 --proxy-user 参数

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl \
     --proxy-user "user1:pass1" \
     --proxy-user "user2:pass2"
   ```
2. 分别用两个用户认证访问：
   ```bash
   curl -x http://user1:pass1@127.0.0.1:8800 http://httpbin.org/get
   curl -x http://user2:pass2@127.0.0.1:8800 http://httpbin.org/get
   ```

**预期结果**：
- 服务正常启动
- 两个用户均可通过认证，正常返回 httpbin.org 的 JSON 响应
- 使用未注册的用户名密码仍返回 407

---

### TC-CSS-22：--intercept 和 --no-intercept 互斥检查

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl --intercept --no-intercept
   ```

**预期结果**：
- CLI 报错，提示 `--intercept` 和 `--no-intercept` 不能同时使用（clap 的 conflicts_with 机制）
- 服务不会启动
- 进程以非零退出码退出

---

### TC-CSS-23：version-check 子命令检查新版本

**前置条件**：服务已以 daemon 模式运行

**操作步骤**：
1. 以 daemon 模式启动服务：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 -d --unsafe-ssl
   ```
2. 执行版本检查命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- -p 8800 version-check
   ```

**预期结果**：
- 输出当前版本和最新可用版本的对比信息
- 如果是最新版本，提示已是最新
- 如果有新版本，显示新版本号及升级提示
- 命令执行后立即退出

**清理**：
```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- stop
```

---

### TC-CSS-24：status 命令别名 st

**前置条件**：服务已以 daemon 模式运行

**操作步骤**：
1. 执行以下命令：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- st
   ```

**预期结果**：
- 输出与 `status` 命令完全一致的服务状态信息
- `st` 作为 `status` 的别名正常工作

---

## 清理

测试完成后清理临时数据和规则文件：
```bash
rm -rf .bifrost-test
rm -f /tmp/bifrost-test-rules.txt
```
