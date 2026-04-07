use crate::assertions::{assert_json_field, assert_status};
use crate::{ProxyInstance, TestCase};
use std::net::TcpListener;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "validate_basic_line_block",
            "Validate basic line` block parses correctly without errors",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "line`\n\
                    https://example.com http://localhost:3000\n\
                    `";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "valid", "true")?;
                assert_json_field(&json, "rule_count", "1")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_line_block_with_filters",
            "Validate line` block with includeFilter and excludeFilter",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "line`\n\
                    https://m.bifrost.local http://localhost:5173\n\
                    includeFilter://m.bifrost.local\n\
                    excludeFilter://m.bifrost.local/api\n\
                    excludeFilter://m.bifrost.local/upload\n\
                    `";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "rule_count", "1")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors for line block with filters, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_complex_line_blocks_with_markdown_values",
            "Validate complex scenario: 2 line blocks + markdown values + reqHeaders",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "\
line`
https://m.bifrost.local http://localhost:5173
includeFilter://m.bifrost.local
excludeFilter://m.bifrost.local/api
excludeFilter://m.bifrost.local/global_config
excludeFilter://m.bifrost.local/devops
excludeFilter://m.bifrost.local/upload
excludeFilter://m.bifrost.local/proxy
excludeFilter://m.bifrost.local/mira/api
excludeFilter://m.bifrost.local/mira/scheduler
`

line`
https://m.bifrost.local2 http://localhost:5173
includeFilter://m.bifrost.local2
excludeFilter://m.bifrost.local2/api
excludeFilter://m.bifrost.local2/global_config
excludeFilter://m.bifrost.local2/devops
excludeFilter://m.bifrost.local2/upload
excludeFilter://m.bifrost.local2/proxy
excludeFilter://m.bifrost.local2/mira/api
excludeFilter://m.bifrost.local2/mira/scheduler
`

```mcp-ppe
x-use-ppe: 1
x-tt-env: ppe_mira_mcp_app
```

```task-ppe
x-use-ppe: 1
x-tt-env: ppe_yqq_test
```

# m.bifrost.local2 reqHeaders://{mcp-ppe}
# m.bifrost.local reqHeaders://{mcp-ppe}";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "rule_count", "2")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors for complex line blocks, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                let defined_vars = json
                    .get("defined_variables")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'defined_variables' field")?;
                let var_names: Vec<&str> = defined_vars
                    .iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .collect();
                if !var_names.contains(&"mcp-ppe") {
                    return Err(format!(
                        "Expected 'mcp-ppe' in defined_variables, got: {:?}",
                        var_names
                    ));
                }
                if !var_names.contains(&"task-ppe") {
                    return Err(format!(
                        "Expected 'task-ppe' in defined_variables, got: {:?}",
                        var_names
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_unclosed_line_block_error",
            "Validate unclosed line` block produces E006 error with correct message",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "line`\n\
                    https://example.com http://localhost:3000\n\
                    includeFilter://example.com";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "valid", "false")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;

                let has_e006 = errors
                    .iter()
                    .any(|e| e.get("code").and_then(|c| c.as_str()) == Some("E006"));
                if !has_e006 {
                    return Err(format!(
                        "Expected E006 error for unclosed line block, got: {:?}",
                        errors
                    ));
                }

                let e006_error = errors
                    .iter()
                    .find(|e| e.get("code").and_then(|c| c.as_str()) == Some("E006"))
                    .unwrap();

                let message = e006_error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .ok_or("E006 error missing message")?;
                if !message.contains("Unclosed line block") {
                    return Err(format!(
                        "E006 message should mention 'Unclosed line block', got: {}",
                        message
                    ));
                }

                let suggestion = e006_error
                    .get("suggestion")
                    .and_then(|s| s.as_str())
                    .ok_or("E006 error missing suggestion")?;
                if !suggestion.contains("`") {
                    return Err(format!(
                        "E006 suggestion should mention closing '`', got: {}",
                        suggestion
                    ));
                }

                let line = e006_error
                    .get("line")
                    .and_then(|l| l.as_u64())
                    .ok_or("E006 error missing line number")?;
                if line != 1 {
                    return Err(format!(
                        "E006 error should point to line 1 (start of line block), got line {}",
                        line
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_line_block_mixed_with_normal_rules",
            "Validate line` block mixed with normal rules parses correctly",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "\
example.com redirect://https://example.org

line`
https://m.bifrost.local http://localhost:5173
includeFilter://m.bifrost.local
`

test.example.com file:///tmp/mock.json";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "rule_count", "3")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors for mixed rules, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_backslash_continuation_lines",
            "Validate backslash continuation lines parse correctly",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "example.com \\\nredirect://https://example.org";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "valid", "true")?;
                assert_json_field(&json, "rule_count", "1")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors for continuation lines, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_line_block_with_reqheaders_and_variables",
            "Validate line` block + markdown values + active reqHeaders with variable expansion",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "\
```mcp-ppe
x-use-ppe: 1
x-tt-env: ppe_mira_mcp_app
```

line`
https://m.bifrost.local http://localhost:5173
includeFilter://m.bifrost.local
`

m.bifrost.local reqHeaders://{mcp-ppe}";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors for reqHeaders with variables, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                let defined_vars = json
                    .get("defined_variables")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'defined_variables' field")?;
                let var_names: Vec<&str> = defined_vars
                    .iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .collect();
                if !var_names.contains(&"mcp-ppe") {
                    return Err(format!(
                        "Expected 'mcp-ppe' in defined_variables, got: {:?}",
                        var_names
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "validate_empty_line_block_no_crash",
            "Validate empty line` block does not crash and produces no rules",
            "rule_validation",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy: {}", e))?;

                let client = build_client()?;

                let content = "line`\n`";

                let body = serde_json::json!({
                    "content": content,
                    "global_values": {}
                });

                let resp = client
                    .post(format!(
                        "http://127.0.0.1:{}/_bifrost/api/rules/validate",
                        port
                    ))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                assert_status(&resp, 200)?;

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

                assert_json_field(&json, "rule_count", "0")?;

                let errors = json
                    .get("errors")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing 'errors' field")?;
                if !errors.is_empty() {
                    return Err(format!(
                        "Expected no errors for empty line block, got {}: {:?}",
                        errors.len(),
                        errors
                    ));
                }

                Ok(())
            },
        ),
    ]
}

fn pick_unused_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind ephemeral port: {}", e))?
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| format!("Failed to read ephemeral port: {}", e))
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))
}
