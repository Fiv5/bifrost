use crate::assertions::*;
use crate::client::ProxyClient;
use crate::runner::TestCase;

pub fn tests() -> Vec<TestCase> {
    vec![
        TestCase::new(
            "routing_httpbin_basic",
            "routing",
            vec![],
            |client: ProxyClient| async move {
                let json = client.get_json("http://httpbin.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field_exists(&json, "url")?;
                assert_json_field_contains(&json, "url", "httpbin.org")?;
                Ok(())
            },
        ),
        TestCase::new(
            "routing_host_redirect",
            "routing",
            vec!["www.example.com host://httpbin.org"],
            |client: ProxyClient| async move {
                let json = client.get_json("http://www.example.com/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field_exists(&json, "url")?;
                Ok(())
            },
        ),
        TestCase::new(
            "routing_wildcard_pattern",
            "routing",
            vec!["*.example.org host://httpbin.org"],
            |client: ProxyClient| async move {
                let json = client.get_json("http://api.example.org/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field_exists(&json, "url")?;
                Ok(())
            },
        ),
        TestCase::new(
            "routing_path_with_host_redirect",
            "routing",
            vec!["test-path.local host://httpbin.org"],
            |client: ProxyClient| async move {
                let json = client.get_json("http://test-path.local/anything/api/users").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field_exists(&json, "url")?;
                assert_json_field_contains(&json, "url", "/anything/api/users")?;
                Ok(())
            },
        ),
        TestCase::new(
            "routing_subdomain_pattern",
            "routing",
            vec!["test-sub.domain.local host://httpbin.org"],
            |client: ProxyClient| async move {
                let json = client.get_json("http://test-sub.domain.local/get").await
                    .map_err(|e| format!("Request failed: {}", e))?;
                assert_json_field_exists(&json, "url")?;
                Ok(())
            },
        ),
        TestCase::new(
            "routing_multi_domain",
            "routing",
            vec![
                "domain1.test host://httpbin.org",
                "domain2.test host://httpbin.org",
            ],
            |client: ProxyClient| async move {
                let json1 = client.get_json("http://domain1.test/get").await
                    .map_err(|e| format!("Request to domain1 failed: {}", e))?;
                assert_json_field_exists(&json1, "url")?;

                let json2 = client.get_json("http://domain2.test/get").await
                    .map_err(|e| format!("Request to domain2 failed: {}", e))?;
                assert_json_field_exists(&json2, "url")?;

                Ok(())
            },
        ),
    ]
}
