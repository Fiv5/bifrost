use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "tls_rule_intercept_override",
            "tlsIntercept:// rule forces TLS interception",
            "tls_intercept_mode",
            test_tls_rule_intercept_override,
        ),
        TestCase::standalone(
            "tls_rule_passthrough_override",
            "tlsPassthrough:// rule forces TLS passthrough",
            "tls_intercept_mode",
            test_tls_rule_passthrough_override,
        ),
        TestCase::standalone(
            "tls_rule_intercept_with_modification",
            "tlsIntercept:// combined with request modification",
            "tls_intercept_mode",
            test_tls_rule_intercept_with_modification,
        ),
    ]
}

async fn test_tls_rule_intercept_override() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "intercepted_ok");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            "*.force-intercept.test tlsIntercept://",
            &format!("force-intercept.test host://127.0.0.1:{}", mock.port),
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://force-intercept.test/api/test",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("intercepted_ok")?;
    mock.assert_request_received()?;

    Ok(())
}

async fn test_tls_rule_passthrough_override() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "passthrough_ok");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            "*.passthrough.test tlsPassthrough://",
            &format!("passthrough.test host://127.0.0.1:{}", mock.port),
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://passthrough.test/api/test",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("passthrough_ok")?;
    mock.assert_request_received()?;

    Ok(())
}

async fn test_tls_rule_intercept_with_modification() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "modified_ok");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![
            "*.api.test tlsIntercept:// reqHeaders://(X-Intercepted: true)",
            &format!("api.test host://127.0.0.1:{}", mock.port),
        ],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://api.test/api/test",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("modified_ok")?;
    mock.assert_request_received()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intercept_rule() {
        let result = test_tls_rule_intercept_override().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_passthrough_rule() {
        let result = test_tls_rule_passthrough_override().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_intercept_with_modification() {
        let result = test_tls_rule_intercept_with_modification().await;
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }
}
