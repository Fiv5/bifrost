use crate::assertions::*;
use crate::client::ProxyClient;
use crate::runner::TestCase;

pub fn tests() -> Vec<TestCase> {
    vec![
        TestCase::new(
            "template_var_now",
            "template",
            vec!["httpbin.org reqHeaders://X-Timestamp=${now}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let timestamp = headers
                    .get("X-Timestamp")
                    .or_else(|| headers.get("x-timestamp"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Timestamp header not found")?;

                assert_is_number(timestamp)?;
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_random",
            "template",
            vec!["httpbin.org reqHeaders://X-Random=${random}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let random = headers
                    .get("X-Random")
                    .or_else(|| headers.get("x-random"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Random header not found")?;

                assert_is_number(random)?;
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_uuid",
            "template",
            vec!["httpbin.org reqHeaders://X-UUID=${randomUUID}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let uuid = headers
                    .get("X-Uuid")
                    .or_else(|| headers.get("x-uuid"))
                    .or_else(|| headers.get("X-UUID"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-UUID header not found")?;

                assert_is_uuid(uuid)?;
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_url",
            "template",
            vec!["httpbin.org reqHeaders://X-Url=${url}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let url = headers
                    .get("X-Url")
                    .or_else(|| headers.get("x-url"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Url header not found")?;

                if !url.contains("httpbin.org") {
                    return Err(format!("URL does not contain httpbin.org: {}", url));
                }
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_host",
            "template",
            vec!["httpbin.org reqHeaders://X-Host=${host}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let host = headers
                    .get("X-Host")
                    .or_else(|| headers.get("x-host"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Host header not found")?;

                if !host.contains("httpbin.org") {
                    return Err(format!("Host does not contain httpbin.org: {}", host));
                }
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_path",
            "template",
            vec!["httpbin.org reqHeaders://X-Path=${path}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let path = headers
                    .get("X-Path")
                    .or_else(|| headers.get("x-path"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Path header not found")?;

                if !path.contains("/headers") {
                    return Err(format!("Path does not contain /headers: {}", path));
                }
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_method",
            "template",
            vec!["httpbin.org reqHeaders://X-Method=${method}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let method = headers
                    .get("X-Method")
                    .or_else(|| headers.get("x-method"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Method header not found")?;

                if method != "GET" {
                    return Err(format!("Method is not GET: {}", method));
                }
                Ok(())
            },
        ),
        TestCase::new(
            "template_var_version",
            "template",
            vec!["httpbin.org reqHeaders://X-Version=${version}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let version = headers
                    .get("X-Version")
                    .or_else(|| headers.get("x-version"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Version header not found")?;

                if version.is_empty() {
                    return Err("Version is empty".to_string());
                }
                Ok(())
            },
        ),
        TestCase::new(
            "template_combined_vars",
            "template",
            vec!["httpbin.org reqHeaders://X-Info=${host|${method}|${now}}"],
            |client: ProxyClient| async move {
                let json = client
                    .get_json("http://httpbin.org/headers")
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let headers = json.get("headers").ok_or("No headers in response")?;
                let info = headers
                    .get("X-Info")
                    .or_else(|| headers.get("x-info"))
                    .and_then(|v| v.as_str())
                    .ok_or("X-Info header not found")?;

                let parts: Vec<&str> = info.split('|').collect();
                if parts.len() != 3 {
                    return Err(format!("Expected 3 parts separated by |, got: {}", info));
                }

                if !parts[0].contains("httpbin") {
                    return Err(format!("First part should contain httpbin: {}", parts[0]));
                }
                if parts[1] != "GET" {
                    return Err(format!("Second part should be GET: {}", parts[1]));
                }
                assert_is_number(parts[2])?;

                Ok(())
            },
        ),
    ]
}
