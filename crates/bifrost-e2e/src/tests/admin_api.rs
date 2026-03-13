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
    ]
}

fn pick_unused_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind ephemeral port: {}", e))?
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| format!("Failed to read ephemeral port: {}", e))
}
