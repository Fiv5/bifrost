# CLI `rule list` legacy 容错测试

## 功能模块说明

本文档验证 `bifrost rule list` 在本地规则目录中同时存在正常规则文件和损坏的 legacy `.json` 规则文件时，会跳过坏文件并继续列出可用的本地规则；同时确认列表范围不包含其他 group 的规则。

## 前置条件

1. 在仓库根目录执行以下命令，确保使用独立临时数据目录：
   ```bash
   rm -rf ./.bifrost-test
   ```
2. 后续命令统一使用：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost --
   ```
   为简化描述，后续以 `bifrost` 代指该完整命令前缀。
3. 确认测试期间不使用 9900 端口；本用例不需要启动代理服务。

## 测试用例

### TC-CRL-01：`rule list` 跳过损坏 legacy 文件并继续展示本地规则

**操作步骤**：
1. 执行 `bifrost rule add valid-local -c "example.com host://127.0.0.1:3000"`
2. 执行以下命令写入损坏的 legacy 规则文件：
   ```bash
   mkdir -p ./.bifrost-test/rules
   cat > ./.bifrost-test/rules/broken.json <<'EOF'
   {"content":"broken.example.com host://127.0.0.1:4000","enabled":true}
   EOF
   ```
3. 执行 `bifrost rule list`

**预期结果**：
- 第 1 步输出 `Rule 'valid-local' added successfully.`
- 第 3 步命令执行成功，不输出 `Error:`
- 第 3 步输出包含 `Rules (1):`
- 第 3 步输出包含 `valid-local [enabled]`
- 第 3 步仍然只展示本地规则，不包含其他 group 的规则

## 清理步骤

测试结束后执行：

```bash
rm -rf ./.bifrost-test
```
