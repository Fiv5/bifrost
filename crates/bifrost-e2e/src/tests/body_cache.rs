use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use bifrost_admin::{AdminState, QueryParams, TrafficRecord};
use std::sync::Arc;
use std::time::Duration;

async fn get_latest_record(admin_state: &Arc<AdminState>) -> Result<TrafficRecord, String> {
    let Some(db_store) = admin_state.traffic_db_store.clone() else {
        return Err("Traffic DB not configured".to_string());
    };

    tokio::task::spawn_blocking(move || {
        let result = db_store.query(&QueryParams {
            limit: Some(1),
            ..Default::default()
        });
        let id = result
            .records
            .first()
            .map(|r| r.id.clone())
            .ok_or_else(|| "No traffic records found".to_string())?;
        db_store
            .get_by_id(&id)
            .ok_or_else(|| "Failed to get traffic record detail".to_string())
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {}", e))?
}

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "body_cache_request_body_small",
            "请求体缓存 - 小请求体存储",
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

    let record = get_latest_record(&admin_state).await?;

    let body_ref = record
        .request_body_ref
        .ok_or("request_body_ref is None - body was not stored")?;

    let Some(body_store) = admin_state.body_store.as_ref() else {
        return Err("Body store not configured".to_string());
    };
    let data = body_store
        .read()
        .load(&body_ref)
        .ok_or("Failed to load request body")?;
    if !data.contains("small-request-body") {
        return Err(format!(
            "Expected 'small-request-body' in body, got: {}",
            data
        ));
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

    let record = get_latest_record(&admin_state).await?;

    let body_ref = record
        .request_body_ref
        .ok_or("request_body_ref is None - body was not stored")?;

    let Some(body_store) = admin_state.body_store.as_ref() else {
        return Err("Body store not configured".to_string());
    };
    let data = body_store
        .read()
        .load(&body_ref)
        .ok_or("Failed to load request body")?;
    if !data.contains("original-content") {
        return Err(format!("Expected original body in store, got: {}", data));
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

    let record = get_latest_record(&admin_state).await?;

    let body_ref = record
        .request_body_ref
        .ok_or("request_body_ref is None - body was not stored")?;

    let Some(body_store) = admin_state.body_store.as_ref() else {
        return Err("Body store not configured".to_string());
    };
    let data = body_store
        .read()
        .load(&body_ref)
        .ok_or("Failed to load request body")?;
    if data != json_body {
        return Err(format!(
            "Expected '{}' in stored body, got: '{}'",
            json_body, data
        ));
    }

    Ok(())
}
