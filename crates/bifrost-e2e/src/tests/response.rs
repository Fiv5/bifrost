use crate::assertions::*;
use crate::client::ProxyClient;
use crate::runner::TestCase;

pub fn tests() -> Vec<TestCase> {
    vec![
        TestCase::new(
            "response_add_header",
            "response",
            vec!["httpbin.org resHeaders://{X-Injected-By: Bifrost}"],
            |client: ProxyClient| async move {
                let resp = client.get("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_header_value(&resp, "X-Injected-By", "Bifrost")?;
                Ok(())
            },
        ),
        TestCase::new(
            "response_cors_headers",
            "response",
            vec!["httpbin.org resCors://*"],
            |client: ProxyClient| async move {
                let resp = client.get("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_header_exists(&resp, "access-control-allow-origin")?;
                Ok(())
            },
        ),
        TestCase::new(
            "response_cache_control",
            "response",
            vec!["httpbin.org resHeaders://{Cache-Control: no-cache}"],
            |client: ProxyClient| async move {
                let resp = client.get("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_header_contains(&resp, "Cache-Control", "no-cache")?;
                Ok(())
            },
        ),
        TestCase::new(
            "response_set_cookie",
            "response",
            vec!["httpbin.org resCookies://{proxy_session: xyz789}"],
            |client: ProxyClient| async move {
                let resp = client.get("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_header_contains(&resp, "set-cookie", "proxy_session")?;
                Ok(())
            },
        ),
        TestCase::new(
            "response_content_type",
            "response",
            vec!["httpbin.org resHeaders://{Content-Type: text/plain}"],
            |client: ProxyClient| async move {
                let resp = client.get("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_header_value(&resp, "Content-Type", "text/plain")?;
                Ok(())
            },
        ),
        TestCase::new(
            "response_multiple_headers",
            "response",
            vec![
                "httpbin.org resHeaders://{X-Frame-Options: DENY}",
                "httpbin.org resHeaders://{X-Content-Type-Options: nosniff}",
            ],
            |client: ProxyClient| async move {
                let resp = client.get("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_header_value(&resp, "X-Frame-Options", "DENY")?;
                assert_header_value(&resp, "X-Content-Type-Options", "nosniff")?;
                Ok(())
            },
        ),
    ]
}
