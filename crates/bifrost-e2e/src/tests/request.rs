use crate::assertions::*;
use crate::client::ProxyClient;
use crate::runner::TestCase;

pub fn tests() -> Vec<TestCase> {
    vec![
        TestCase::new(
            "request_add_header",
            "request",
            vec!["httpbin.org reqHeaders://{X-Custom-Header: test-value}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_header(&json, "X-Custom-Header", "test-value")?;
                Ok(())
            },
        ),
        TestCase::new(
            "request_modify_ua",
            "request",
            vec!["httpbin.org ua://Bifrost-Test-Agent/1.0"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/user-agent")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field_contains(&json, "user-agent", "Bifrost-Test-Agent")?;
                Ok(())
            },
        ),
        TestCase::new(
            "request_set_referer",
            "request",
            vec!["httpbin.org referer://https://bifrost.test/"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_header(&json, "Referer", "https://bifrost.test/")?;
                Ok(())
            },
        ),
        TestCase::new(
            "request_add_cookie",
            "request",
            vec!["httpbin.org reqCookies://{session: abc123}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/cookies")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field(&json, "cookies.session", "abc123")?;
                Ok(())
            },
        ),
        TestCase::new(
            "request_multiple_headers",
            "request",
            vec![
                "httpbin.org reqHeaders://{X-Header-1: value1}",
                "httpbin.org reqHeaders://{X-Header-2: value2}",
            ],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_header(&json, "X-Header-1", "value1")?;
                assert_json_header(&json, "X-Header-2", "value2")?;
                Ok(())
            },
        ),
        TestCase::new(
            "request_auth_header",
            "request",
            vec!["httpbin.org reqHeaders://{Authorization: Bearer token123}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_header(&json, "Authorization", "Bearer token123")?;
                Ok(())
            },
        ),
        TestCase::new(
            "request_accept_header",
            "request",
            vec!["httpbin.org reqHeaders://{Accept: application/xml}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_header_contains(&json, "Accept", "application/xml")?;
                Ok(())
            },
        ),
    ]
}
