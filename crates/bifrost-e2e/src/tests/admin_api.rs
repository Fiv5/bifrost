use crate::assertions::{assert_header_contains, assert_header_value, assert_status};
use crate::{ProxyInstance, TestCase};
use std::net::TcpListener;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "admin_api_cors_preflight_allows_client_id",
            "Validate admin CORS preflight allows desktop client headers",
            "admin",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

                let client = reqwest::Client::builder()
                    .danger_accept_invalid_certs(true)
                    .no_proxy()
                    .build()
                    .map_err(|e| format!("Failed to create client: {}", e))?;

                let response = client
                    .request(
                        reqwest::Method::OPTIONS,
                        format!("http://127.0.0.1:{}/_bifrost/api/system/info", port),
                    )
                    .header("Origin", "http://127.0.0.1:3000")
                    .header("Access-Control-Request-Method", "GET")
                    .header("Access-Control-Request-Headers", "X-Client-Id")
                    .send()
                    .await
                    .map_err(|e| format!("Preflight request failed: {}", e))?;

                assert_status(&response, 204)?;
                assert_header_value(&response, "access-control-allow-origin", "*")?;
                assert_header_contains(&response, "access-control-allow-methods", "OPTIONS")?;
                assert_header_contains(&response, "access-control-allow-headers", "X-Client-Id")?;
                Ok(())
            },
        ),
        TestCase::standalone(
            "admin_public_proxy_qrcode_allows_cors",
            "Validate public proxy qrcode endpoint supports cross-origin access",
            "admin",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

                let client = reqwest::Client::builder()
                    .danger_accept_invalid_certs(true)
                    .no_proxy()
                    .build()
                    .map_err(|e| format!("Failed to create client: {}", e))?;

                let get_response = client
                    .get(format!(
                        "http://127.0.0.1:{}/_bifrost/public/proxy/qrcode?ip=127.0.0.1",
                        port
                    ))
                    .header("Origin", "http://127.0.0.1:3000")
                    .send()
                    .await
                    .map_err(|e| format!("GET qrcode request failed: {}", e))?;

                assert_status(&get_response, 200)?;
                assert_header_value(&get_response, "access-control-allow-origin", "*")?;
                assert_header_contains(&get_response, "access-control-allow-methods", "GET")?;
                assert_header_contains(&get_response, "access-control-allow-methods", "OPTIONS")?;
                assert_header_contains(
                    &get_response,
                    "access-control-allow-headers",
                    "X-Client-Id",
                )?;
                assert_header_contains(&get_response, "content-type", "image/svg+xml")?;

                let preflight_response = client
                    .request(
                        reqwest::Method::OPTIONS,
                        format!(
                            "http://127.0.0.1:{}/_bifrost/public/proxy/qrcode?ip=127.0.0.1",
                            port
                        ),
                    )
                    .header("Origin", "http://127.0.0.1:3000")
                    .header("Access-Control-Request-Method", "GET")
                    .header("Access-Control-Request-Headers", "X-Client-Id")
                    .send()
                    .await
                    .map_err(|e| format!("OPTIONS qrcode request failed: {}", e))?;

                assert_status(&preflight_response, 204)?;
                assert_header_value(&preflight_response, "access-control-allow-origin", "*")?;
                assert_header_contains(
                    &preflight_response,
                    "access-control-allow-methods",
                    "OPTIONS",
                )?;
                assert_header_contains(
                    &preflight_response,
                    "access-control-allow-headers",
                    "X-Client-Id",
                )?;
                Ok(())
            },
        ),
        TestCase::standalone(
            "admin_api_config_connections",
            "Validate GET /api/config/connections returns valid JSON with connections array",
            "admin",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

                let client = reqwest::Client::builder()
                    .danger_accept_invalid_certs(true)
                    .no_proxy()
                    .build()
                    .map_err(|e| format!("Failed to create client: {}", e))?;

                let response = client
                    .get(format!(
                        "http://127.0.0.1:{}/_bifrost/api/config/connections",
                        port
                    ))
                    .send()
                    .await
                    .map_err(|e| format!("GET connections failed: {}", e))?;

                assert_status(&response, 200)?;

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse connections JSON: {}", e))?;

                if json.get("total").is_none() && json.get("connections").is_none() {
                    return Err(format!(
                        "Expected 'total' or 'connections' field in response, got: {}",
                        serde_json::to_string_pretty(&json).unwrap_or_default()
                    ));
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "admin_api_system_memory",
            "Validate GET /api/system/memory returns memory diagnostics with process info",
            "admin",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

                let client = reqwest::Client::builder()
                    .danger_accept_invalid_certs(true)
                    .no_proxy()
                    .build()
                    .map_err(|e| format!("Failed to create client: {}", e))?;

                let response = client
                    .get(format!(
                        "http://127.0.0.1:{}/_bifrost/api/system/memory",
                        port
                    ))
                    .send()
                    .await
                    .map_err(|e| format!("GET system/memory failed: {}", e))?;

                assert_status(&response, 200)?;

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse memory JSON: {}", e))?;

                if json.get("process").is_none() {
                    return Err(format!(
                        "Expected 'process' field in memory response, got: {}",
                        serde_json::to_string_pretty(&json).unwrap_or_default()
                    ));
                }

                let process = &json["process"];
                if process.get("pid").is_none() {
                    return Err("Expected 'pid' in process info".to_string());
                }
                if process.get("rss_kib").is_none() {
                    return Err("Expected 'rss_kib' in process info".to_string());
                }

                if json.get("stores").is_none() {
                    return Err("Expected 'stores' field in memory response".to_string());
                }

                Ok(())
            },
        ),
        TestCase::standalone(
            "admin_api_config_show_server_section",
            "Validate GET /api/config returns server configuration section",
            "admin",
            || async move {
                let port = pick_unused_port()?;
                let (_proxy, _admin_state) =
                    ProxyInstance::start_with_admin(port, vec![], false, true)
                        .await
                        .map_err(|e| format!("Failed to start proxy with admin: {}", e))?;

                let client = reqwest::Client::builder()
                    .danger_accept_invalid_certs(true)
                    .no_proxy()
                    .build()
                    .map_err(|e| format!("Failed to create client: {}", e))?;

                let response = client
                    .get(format!("http://127.0.0.1:{}/_bifrost/api/config", port))
                    .send()
                    .await
                    .map_err(|e| format!("GET config failed: {}", e))?;

                assert_status(&response, 200)?;

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse config JSON: {}", e))?;

                if json.get("port").is_none() && json.get("server").is_none() {
                    return Err(format!(
                        "Expected server config fields (port or server), got: {}",
                        serde_json::to_string_pretty(&json).unwrap_or_default()
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
