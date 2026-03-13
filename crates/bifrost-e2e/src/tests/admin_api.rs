use crate::assertions::{assert_header_contains, assert_header_value, assert_status};
use crate::{ProxyInstance, TestCase};

pub fn get_all_tests() -> Vec<TestCase> {
    vec![TestCase::standalone(
        "admin_api_cors_preflight_allows_client_id",
        "Validate admin CORS preflight allows desktop client headers",
        "admin",
        || async move {
            let port = portpicker::pick_unused_port().ok_or("Failed to pick unused port")?;
            let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, vec![], false, true)
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
    )]
}
