use std::time::Duration;

use crate::client::DirectClient;
use crate::{ProxyClient, ProxyInstance, TestCase};

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "mock_file_traffic_recorded",
            "file:// 规则的请求应被录制到 network traffic 中",
            "traffic",
            test_mock_file_traffic_recorded,
        ),
        TestCase::standalone(
            "mock_redirect_traffic_recorded",
            "redirect 规则的请求应被录制到 network traffic 中",
            "traffic",
            test_mock_redirect_traffic_recorded,
        ),
    ]
}

async fn test_mock_file_traffic_recorded() -> Result<(), String> {
    let port = portpicker::pick_unused_port().ok_or("Failed to pick unused port")?;

    let rules = vec![r#"test.local file://({"mock":"file_response"})"#];
    let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, rules, false, true)
        .await
        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

    let proxy_url = format!("http://127.0.0.1:{}", port);
    let client = ProxyClient::new(&proxy_url).map_err(|e| e.to_string())?;

    let resp = client.get("http://test.local/api/data").await;
    if let Err(e) = &resp {
        return Err(format!("Request failed: {}", e));
    }

    tokio::time::sleep(Duration::from_millis(300)).await;

    let direct = DirectClient::new().map_err(|e| e.to_string())?;
    let list_url = format!("http://127.0.0.1:{}/_bifrost/api/traffic?limit=20", port);
    let list_json = direct
        .get_json(&list_url)
        .await
        .map_err(|e| e.to_string())?;

    let records = list_json
        .get("records")
        .and_then(|v| v.as_array())
        .ok_or("Expected records array in traffic list")?;

    if records.is_empty() {
        return Err(
            "file:// rule response should be recorded in traffic, but no records found".to_string(),
        );
    }

    let found = records.iter().any(|r| {
        r.get("h")
            .and_then(|v| v.as_str())
            .map(|h| h.contains("test.local"))
            .unwrap_or(false)
    });
    if !found {
        return Err(format!(
            "Expected traffic record for test.local, but not found in {:?}",
            records
        ));
    }

    let record = records
        .iter()
        .find(|r| {
            r.get("h")
                .and_then(|v| v.as_str())
                .map(|h| h.contains("test.local"))
                .unwrap_or(false)
        })
        .unwrap();

    let status = record.get("s").and_then(|v| v.as_u64()).unwrap_or(0);
    if status != 200 {
        return Err(format!("Expected status 200, got {}", status));
    }

    let rule_count = record.get("rc").and_then(|v| v.as_u64()).unwrap_or(0);
    if rule_count == 0 {
        return Err("Expected rule count > 0 for file:// rule hit".to_string());
    }

    Ok(())
}

async fn test_mock_redirect_traffic_recorded() -> Result<(), String> {
    let port = portpicker::pick_unused_port().ok_or("Failed to pick unused port")?;

    let rules = vec!["test.local/old redirect://https://example.com/new"];
    let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, rules, false, true)
        .await
        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

    let proxy_url = format!("http://127.0.0.1:{}", port);
    let client = ProxyClient::new(&proxy_url).map_err(|e| e.to_string())?;

    let _ = client.get("http://test.local/old").await;

    tokio::time::sleep(Duration::from_millis(300)).await;

    let direct = DirectClient::new().map_err(|e| e.to_string())?;
    let list_url = format!("http://127.0.0.1:{}/_bifrost/api/traffic?limit=20", port);
    let list_json = direct
        .get_json(&list_url)
        .await
        .map_err(|e| e.to_string())?;

    let records = list_json
        .get("records")
        .and_then(|v| v.as_array())
        .ok_or("Expected records array in traffic list")?;

    if records.is_empty() {
        return Err(
            "redirect rule response should be recorded in traffic, but no records found"
                .to_string(),
        );
    }

    let found = records.iter().any(|r| {
        r.get("h")
            .and_then(|v| v.as_str())
            .map(|h| h.contains("test.local"))
            .unwrap_or(false)
    });
    if !found {
        return Err(format!(
            "Expected traffic record for test.local redirect, but not found in {:?}",
            records
        ));
    }

    Ok(())
}
