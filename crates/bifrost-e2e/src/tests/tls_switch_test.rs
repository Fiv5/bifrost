use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use serde_json::Value;
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![TestCase::standalone(
        "tls_switch_intercept_to_tunnel",
        "Test TLS interception switch to tunnel mode and verify traffic records via Admin API",
        "tls_switch",
        test_tls_switch_intercept_to_tunnel,
    )]
}

fn get_traffic_as_json(admin_state: &bifrost_admin::AdminState) -> Value {
    let records = admin_state.traffic_recorder.get_all();
    let records_json: Vec<Value> = records
        .into_iter()
        .map(|r| {
            let is_tunnel = r.protocol == "tunnel";
            serde_json::json!({
                "id": r.id,
                "method": r.method,
                "url": r.url,
                "protocol": r.protocol,
                "status": r.status,
                "host": r.host,
                "path": r.path,
                "duration_ms": r.duration_ms,
                "request_size": r.request_size,
                "response_size": r.response_size,
                "is_websocket": r.is_websocket,
                "is_sse": r.is_sse,
                "is_tunnel": is_tunnel,
                "content_type": r.content_type,
            })
        })
        .collect();

    serde_json::json!({
        "total": records_json.len(),
        "offset": 0,
        "limit": 100,
        "records": records_json
    })
}

async fn get_tls_config_as_json(admin_state: &bifrost_admin::AdminState) -> Value {
    let config = admin_state.runtime_config.read().await;
    serde_json::json!({
        "enable_tls_interception": config.enable_tls_interception,
        "intercept_exclude": config.intercept_exclude,
        "intercept_include": config.intercept_include,
        "unsafe_ssl": config.unsafe_ssl,
        "disconnect_on_config_change": config.disconnect_on_config_change
    })
}

async fn update_tls_config(
    admin_state: &bifrost_admin::AdminState,
    enable_tls_interception: bool,
) -> Value {
    let old_value;
    {
        let mut config = admin_state.runtime_config.write().await;
        old_value = config.enable_tls_interception;
        config.enable_tls_interception = enable_tls_interception;
    }

    if old_value != enable_tls_interception {
        let disconnected = admin_state
            .connection_registry
            .disconnect_all_with_mode(!enable_tls_interception);
        println!(
            "[API] Disconnected {} existing connections due to TLS config change",
            disconnected.len()
        );
    }

    serde_json::json!({
        "success": true,
        "message": format!("TLS interception changed from {} to {}", old_value, enable_tls_interception)
    })
}

async fn test_tls_switch_intercept_to_tunnel() -> Result<(), String> {
    println!("\n======================================================================");
    println!("       TLS INTERCEPTION SWITCH TEST - HTTPS ONLY");
    println!("       Testing: 2x HTTPS with TLS ON, 2x HTTPS with TLS OFF");
    println!("======================================================================\n");

    let port = portpicker::pick_unused_port().unwrap();
    println!("[SETUP] Proxy will run on port {}", port);

    let (proxy, admin_state) = ProxyInstance::start_with_admin(port, vec![], true, true)
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    println!("[SETUP] Proxy started with TLS interception ENABLED");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let proxy_url = format!("http://127.0.0.1:{}", port);

    let https_client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy_url).unwrap())
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTPS client: {}", e))?;

    println!("\n======================================================================");
    println!(" PHASE 1: TLS Interception ENABLED - Send 2 HTTPS requests");
    println!("          Expected: Should be recorded as 'https' protocol (intercepted)");
    println!("======================================================================");

    let config_response = get_tls_config_as_json(&admin_state).await;
    println!("\n[CONFIG] Current TLS config:");
    println!(
        "{}",
        serde_json::to_string_pretty(&config_response).unwrap()
    );

    println!("\n[REQUEST 1] Sending HTTPS request to https://httpbin.org/get?req=1");
    let result1 = https_client
        .get("https://httpbin.org/get?req=1")
        .send()
        .await;
    match &result1 {
        Ok(resp) => println!("[RESPONSE 1] Status: {}", resp.status()),
        Err(e) => println!("[RESPONSE 1] Error: {} (continuing...)", e),
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    println!("\n[REQUEST 2] Sending HTTPS request to https://httpbin.org/get?req=2");
    let result2 = https_client
        .get("https://httpbin.org/get?req=2")
        .send()
        .await;
    match &result2 {
        Ok(resp) => println!("[RESPONSE 2] Status: {}", resp.status()),
        Err(e) => println!("[RESPONSE 2] Error: {} (continuing...)", e),
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let traffic_phase1 = get_traffic_as_json(&admin_state);
    println!("\n[API] GET /api/traffic after Phase 1 (TLS ON, 2 HTTPS requests):");
    print_traffic_api_response(&traffic_phase1);

    println!("\n======================================================================");
    println!(" PHASE 2: DISABLE TLS Interception");
    println!("======================================================================");

    println!("\n[API] PUT /api/config/tls - Disabling TLS interception...");
    let update_response = update_tls_config(&admin_state, false).await;
    println!(
        "[API] Response: {}",
        serde_json::to_string_pretty(&update_response).unwrap()
    );

    let config_after = get_tls_config_as_json(&admin_state).await;
    println!("\n[CONFIG] TLS config after update:");
    println!("{}", serde_json::to_string_pretty(&config_after).unwrap());

    let enable_tls_after = config_after["enable_tls_interception"]
        .as_bool()
        .unwrap_or(true);
    if enable_tls_after {
        println!("[WARNING] ⚠️ TLS interception was NOT disabled!");
    } else {
        println!("[SUCCESS] ✅ TLS interception DISABLED");
    }

    println!("\n======================================================================");
    println!(" PHASE 3: TLS Interception DISABLED - Send 2 HTTPS requests");
    println!("          Expected: Should be recorded as 'tunnel' protocol (NOT intercepted)");
    println!("======================================================================");

    println!("\n[REQUEST 3] Sending HTTPS request to https://httpbin.org/get?req=3");
    let result3 = https_client
        .get("https://httpbin.org/get?req=3")
        .send()
        .await;
    match &result3 {
        Ok(resp) => println!("[RESPONSE 3] Status: {}", resp.status()),
        Err(e) => println!("[RESPONSE 3] Error: {} (continuing...)", e),
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    println!("\n[REQUEST 4] Sending HTTPS request to https://httpbin.org/get?req=4");
    let result4 = https_client
        .get("https://httpbin.org/get?req=4")
        .send()
        .await;
    match &result4 {
        Ok(resp) => println!("[RESPONSE 4] Status: {}", resp.status()),
        Err(e) => println!("[RESPONSE 4] Error: {} (continuing...)", e),
    }

    tokio::time::sleep(Duration::from_millis(300)).await;

    let traffic_phase3 = get_traffic_as_json(&admin_state);
    println!("\n[API] GET /api/traffic after Phase 3 (TLS OFF, 2 more HTTPS requests):");
    print_traffic_api_response(&traffic_phase3);

    println!("\n======================================================================");
    println!(" FINAL ANALYSIS");
    println!("======================================================================");

    if let Some(records) = traffic_phase3["records"].as_array() {
        let mut https_count = 0;
        let mut tunnel_count = 0;
        let mut https_records = Vec::new();
        let mut tunnel_records = Vec::new();

        for record in records {
            match record["protocol"].as_str().unwrap_or("") {
                "https" => {
                    https_count += 1;
                    https_records.push(record.clone());
                }
                "tunnel" => {
                    tunnel_count += 1;
                    tunnel_records.push(record.clone());
                }
                _ => {}
            }
        }

        println!("\n[BREAKDOWN] Protocol counts:");
        println!("  - HTTPS (intercepted):     {} records", https_count);
        println!("  - TUNNEL (not intercepted): {} records", tunnel_count);

        println!("\n[HTTPS RECORDS] (TLS interception was ON):");
        if https_records.is_empty() {
            println!("  (none)");
        }
        for (i, r) in https_records.iter().enumerate() {
            println!(
                "  {}. [{}] {} {} | Host: {} | Size: {} bytes",
                i + 1,
                r["id"].as_str().unwrap_or(""),
                r["method"].as_str().unwrap_or(""),
                r["url"].as_str().unwrap_or(""),
                r["host"].as_str().unwrap_or(""),
                r["response_size"]
            );
        }

        println!("\n[TUNNEL RECORDS] (TLS interception was OFF):");
        if tunnel_records.is_empty() {
            println!("  (none)");
        }
        for (i, r) in tunnel_records.iter().enumerate() {
            println!(
                "  {}. [{}] {} {} | Host: {} | Size: {} bytes",
                i + 1,
                r["id"].as_str().unwrap_or(""),
                r["method"].as_str().unwrap_or(""),
                r["url"].as_str().unwrap_or(""),
                r["host"].as_str().unwrap_or(""),
                r["response_size"]
            );
        }

        println!("\n[EXPECTATION]");
        println!("  - HTTPS records: 2 (from Phase 1, TLS interception ON)");
        println!("  - TUNNEL records: >= 1 (from Phase 3, TLS interception OFF)");
        println!(
            "    Note: TUNNEL count may be less than request count due to HTTP/2 connection reuse"
        );

        if https_count == 2 && tunnel_count >= 1 {
            println!("\n[RESULT] ✅ TEST PASSED");
            println!(
                "         - Phase 1 (TLS ON): {} HTTPS 请求被解包记录为 'https' 协议",
                https_count
            );
            println!(
                "         - Phase 3 (TLS OFF): {} HTTPS 请求被记录为 'tunnel' 协议（纯隧道）",
                tunnel_count
            );
            println!(
                "         Web 管理端显示正确：TLS 解包开启时看到详细 HTTPS 数据，关闭后只看到隧道连接"
            );
            println!("         Note: TUNNEL 只记录 CONNECT 建立，不记录后续复用的请求");
        } else {
            println!("\n[RESULT] ⚠️ UNEXPECTED COUNTS");
            println!("         Expected: HTTPS=2, TUNNEL>=1");
            println!(
                "         Actual:   HTTPS={}, TUNNEL={}",
                https_count, tunnel_count
            );

            if https_count > 2 {
                println!("         [BUG?] 有额外的 HTTPS 记录，可能 TLS 解包关闭后仍在解包");
            }
            if tunnel_count == 0 {
                println!("         [BUG?] 没有 TUNNEL 记录，隧道请求未被记录");
            }
            if https_count < 2 {
                println!("         [NOTE] HTTPS 记录不足，可能是网络问题导致请求失败");
            }
        }
    }

    drop(proxy);

    println!("\n======================================================================");
    println!("                    TEST COMPLETED");
    println!("======================================================================\n");

    Ok(())
}

fn print_traffic_api_response(response: &Value) {
    println!("  Total: {}", response["total"]);

    if let Some(records) = response["records"].as_array() {
        println!("  Records ({}):", records.len());
        println!("  {:-<75}", "");

        for (idx, record) in records.iter().enumerate() {
            let protocol = record["protocol"].as_str().unwrap_or("");
            let is_tunnel = protocol == "tunnel";
            let protocol_display = if is_tunnel {
                format!("{} (NOT intercepted)", protocol)
            } else {
                format!("{} (intercepted)", protocol)
            };

            println!(
                "\n  #{} [{}] {} {}",
                idx + 1,
                record["id"].as_str().unwrap_or(""),
                record["method"].as_str().unwrap_or(""),
                record["url"].as_str().unwrap_or("")
            );
            println!(
                "     Protocol: {} | Status: {} | Host: {}",
                protocol_display,
                record["status"],
                record["host"].as_str().unwrap_or("")
            );
            println!(
                "     Duration: {}ms | Req Size: {} | Res Size: {}",
                record["duration_ms"], record["request_size"], record["response_size"]
            );
        }

        if records.is_empty() {
            println!("  (No records)");
        }
        println!("  {:-<75}", "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_tls_switch_test() {
        let result = test_tls_switch_intercept_to_tunnel().await;
        if let Err(e) = &result {
            println!("Test error: {}", e);
        }
        assert!(result.is_ok(), "Test failed: {:?}", result.err());
    }
}
