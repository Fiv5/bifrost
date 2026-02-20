use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "body_cache_request_body_small",
            "请求体缓存 - 小请求体内联存储",
            "body_cache",
            test_request_body_small,
        ),
        TestCase::standalone(
            "body_cache_request_body_with_rule",
            "请求体缓存 - 带规则的请求体",
            "body_cache",
            test_request_body_with_rule,
        ),
        TestCase::standalone(
            "body_cache_request_body_post",
            "请求体缓存 - POST 请求体正确存储",
            "body_cache",
            test_request_body_post,
        ),
    ]
}

async fn test_request_body_small() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "response-ok");

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, admin_state) = ProxyInstance::start_with_admin(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        false,
        false,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .method("POST")
    .data("small-request-body")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("response-ok")?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let records = admin_state.traffic_recorder.get_all();
    if records.is_empty() {
        return Err("No traffic records found".to_string());
    }

    let record_id = &records[0].id;
    let record = admin_state
        .traffic_recorder
        .get_by_id(record_id)
        .ok_or("Failed to get traffic record detail")?;

    let body_ref = record
        .request_body_ref
        .ok_or("request_body_ref is None - body was not stored")?;

    if let bifrost_admin::BodyRef::Inline { data } = body_ref {
        if !data.contains("small-request-body") {
            return Err(format!(
                "Expected 'small-request-body' in body, got: {}",
                data
            ));
        }
    } else {
        return Err("Expected Inline body ref for small request".to_string());
    }

    Ok(())
}

async fn test_request_body_with_rule() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "ok");

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, admin_state) = ProxyInstance::start_with_admin(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} reqReplace://original=modified",
            mock.port
        )],
        false,
        false,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .method("POST")
    .data("original-content")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received by mock")?;
    let body = req.body.ok_or("No body in request")?;
    if !body.contains("modified-content") {
        return Err(format!(
            "Expected 'modified-content' in forwarded body, got: {}",
            body
        ));
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let records = admin_state.traffic_recorder.get_all();
    if records.is_empty() {
        return Err("No traffic records found".to_string());
    }

    let record_id = &records[0].id;
    let record = admin_state
        .traffic_recorder
        .get_by_id(record_id)
        .ok_or("Failed to get traffic record detail")?;

    let body_ref = record
        .request_body_ref
        .ok_or("request_body_ref is None - body was not stored")?;

    if let bifrost_admin::BodyRef::Inline { data } = body_ref {
        if !data.contains("original-content") {
            return Err(format!("Expected original body in store, got: {}", data));
        }
    } else {
        return Err("Expected Inline body ref".to_string());
    }

    Ok(())
}

async fn test_request_body_post() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "ok");

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, admin_state) = ProxyInstance::start_with_admin(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        false,
        false,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let json_body = r#"{"name":"test","value":123}"#;
    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .method("POST")
    .header("Content-Type", "application/json")
    .data(json_body)
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received by mock")?;
    let received_body = req.body.ok_or("No body in request")?;
    if received_body != json_body {
        return Err(format!(
            "Expected body '{}', got: '{}'",
            json_body, received_body
        ));
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let records = admin_state.traffic_recorder.get_all();
    if records.is_empty() {
        return Err("No traffic records found".to_string());
    }

    let record_id = &records[0].id;
    let record = admin_state
        .traffic_recorder
        .get_by_id(record_id)
        .ok_or("Failed to get traffic record detail")?;

    let body_ref = record
        .request_body_ref
        .ok_or("request_body_ref is None - body was not stored")?;

    if let bifrost_admin::BodyRef::Inline { data } = body_ref {
        if data != json_body {
            return Err(format!(
                "Expected '{}' in stored body, got: '{}'",
                json_body, data
            ));
        }
    } else {
        return Err("Expected Inline body ref for small request".to_string());
    }

    Ok(())
}
