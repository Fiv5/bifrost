use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use std::collections::HashMap;
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "req_headers_single",
            "ReqHeaders protocol: add single header",
            "request_modification",
            test_req_headers_single,
        ),
        TestCase::standalone(
            "req_headers_multiple",
            "ReqHeaders protocol: add multiple headers",
            "request_modification",
            test_req_headers_multiple,
        ),
        TestCase::standalone(
            "req_headers_override",
            "ReqHeaders protocol: later rule overrides earlier",
            "request_modification",
            test_req_headers_override,
        ),
        TestCase::standalone(
            "req_headers_value_ref",
            "ReqHeaders protocol: value reference {name} expansion",
            "request_modification",
            test_req_headers_value_ref,
        ),
        TestCase::standalone(
            "req_headers_inline_markdown",
            "ReqHeaders protocol: inline markdown code block values",
            "request_modification",
            test_req_headers_inline_markdown,
        ),
        TestCase::standalone(
            "req_cookies_add",
            "ReqCookies protocol: add request cookies",
            "request_modification",
            test_req_cookies_add,
        ),
        TestCase::standalone(
            "req_cookies_merge",
            "ReqCookies protocol: merge multiple cookies",
            "request_modification",
            test_req_cookies_merge,
        ),
        TestCase::standalone(
            "req_ua_modify",
            "UA protocol: modify User-Agent",
            "request_modification",
            test_req_ua_modify,
        ),
        TestCase::standalone(
            "req_referer_set",
            "Referer protocol: set referer header",
            "request_modification",
            test_req_referer_set,
        ),
        TestCase::standalone(
            "req_auth_basic",
            "Auth protocol: set basic authentication",
            "request_modification",
            test_req_auth_basic,
        ),
        TestCase::standalone(
            "req_method_change",
            "Method protocol: change request method",
            "request_modification",
            test_req_method_change,
        ),
        TestCase::standalone(
            "req_type_json",
            "ReqType protocol: set content-type to json",
            "request_modification",
            test_req_type_json,
        ),
        TestCase::standalone(
            "req_charset_modify",
            "ReqCharset protocol: modify charset",
            "request_modification",
            test_req_charset_modify,
        ),
        TestCase::standalone(
            "req_combined_modifications",
            "Combined: multiple request modification rules",
            "request_modification",
            test_req_combined_modifications,
        ),
    ]
}

async fn test_req_headers_single() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqHeaders://X-Custom-Header=test-value",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("x-custom-header", "test-value")?;

    Ok(())
}

async fn test_req_headers_multiple() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqHeaders://X-Header-A=value-a",
            "test.local reqHeaders://X-Header-B=value-b",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("x-header-a", "value-a")?;
    mock.assert_header_received("x-header-b", "value-b")?;

    Ok(())
}

async fn test_req_headers_override() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqHeaders://X-Override=first",
            "test.local reqHeaders://X-Override=second",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("x-override", "second")?;

    Ok(())
}

async fn test_req_headers_value_ref() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();

    let mut values = HashMap::new();
    values.insert(
        "customHeaders".to_string(),
        "X-Custom-Token=secret-12345".to_string(),
    );

    let _proxy = ProxyInstance::start_with_values(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqHeaders://{customHeaders}",
        ],
        values,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("x-custom-token", "secret-12345")?;

    Ok(())
}

async fn test_req_headers_inline_markdown() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();

    let rules_text = format!(
        r#"
test.local host://127.0.0.1:{}
test.local reqHeaders://{{ppeHeaders}}

```ppeHeaders
X-Use-PPE: 1
X-TT-Env: ppe_test_env
```
"#,
        mock.port
    );

    let _proxy = ProxyInstance::start_with_rules_text(port, &rules_text)
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("x-use-ppe", "1")?;
    mock.assert_header_received("x-tt-env", "ppe_test_env")?;

    Ok(())
}

async fn test_req_cookies_add() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqCookies://session=abc123",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_contains("cookie", "session")?;

    Ok(())
}

async fn test_req_cookies_merge() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqCookies://cookie_a=value1",
            "test.local reqCookies://cookie_b=value2",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_contains("cookie", "cookie_a")?;
    mock.assert_header_contains("cookie", "cookie_b")?;

    Ok(())
}

async fn test_req_ua_modify() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local ua://BifrostTestAgent/2.0",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("user-agent", "BifrostTestAgent/2.0")?;

    Ok(())
}

async fn test_req_referer_set() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local referer://https://referrer.example.com/page",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("referer", "https://referrer.example.com/page")?;

    Ok(())
}

async fn test_req_auth_basic() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local auth://testuser:testpass",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_contains("authorization", "Basic")?;

    Ok(())
}

async fn test_req_method_change() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local method://PUT",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_method("PUT")?;

    Ok(())
}

async fn test_req_type_json() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqType://json",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .method("POST")
    .data(r#"{"test":"data"}"#)
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_contains("content-type", "application/json")?;

    Ok(())
}

async fn test_req_charset_modify() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqCharset://gbk",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .method("POST")
    .header("Content-Type", "text/plain")
    .data("test data")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_contains("content-type", "gbk")?;

    Ok(())
}

async fn test_req_combined_modifications() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            &format!("test.local host://127.0.0.1:{}", mock.port),
            "test.local reqHeaders://X-Custom=combined-test",
            "test.local ua://CombinedAgent/1.0",
            "test.local referer://https://combined.test.com",
            "test.local method://POST",
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    mock.assert_header_received("x-custom", "combined-test")?;
    mock.assert_header_received("user-agent", "CombinedAgent/1.0")?;
    mock.assert_header_received("referer", "https://combined.test.com")?;
    mock.assert_method("POST")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_headers_single() {
        let result = test_req_headers_single().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_headers_override() {
        let result = test_req_headers_override().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_headers_value_ref() {
        let result = test_req_headers_value_ref().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_headers_inline_markdown() {
        let result = test_req_headers_inline_markdown().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_ua() {
        let result = test_req_ua_modify().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_method() {
        let result = test_req_method_change().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_combined() {
        let result = test_req_combined_modifications().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }
}
