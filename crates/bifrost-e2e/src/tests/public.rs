use crate::mock::HttpbinMockServer;
use crate::proxy::ProxyInstance;
use crate::{assert_body_contains, assert_header_value, ProxyClient, TestCase};
use std::time::Duration;

fn is_network_error(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() {
        return true;
    }

    let msg = err.to_string().to_lowercase();
    msg.contains("timeout")
        || msg.contains("timed out")
        || msg.contains("deadline has elapsed")
        || msg.contains("connect error")
        || msg.contains("connection reset")
        || msg.contains("connection refused")
        || msg.contains("dns")
        || msg.contains("name resolution")
        || msg.contains("failed to lookup")
}

fn skip_if_unreachable(host: &str, err: &reqwest::Error) -> Result<(), String> {
    if is_network_error(err) {
        Err(format!(
            "SKIPPED: {host} unreachable in this environment: {err}"
        ))
    } else {
        Err(err.to_string())
    }
}

fn skip_on_upstream_error(host: &str, status: u16) -> Result<(), String> {
    match status {
        502..=504 => Err(format!(
            "SKIPPED: {host} upstream error ({status}), likely unreachable"
        )),
        _ => Err(format!("Unexpected status {status} from {host}")),
    }
}

async fn get_or_skip(
    client: &ProxyClient,
    url: &str,
    host: &str,
) -> Result<reqwest::Response, Result<(), String>> {
    match client.get(url).await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if (502..=504).contains(&status) {
                Err(skip_on_upstream_error(host, status))
            } else {
                Ok(resp)
            }
        }
        Err(e) => Err(skip_if_unreachable(host, &e)),
    }
}

pub fn tests() -> Vec<TestCase> {
    vec![
        TestCase::new(
            "public_baidu_basic",
            "public",
            vec![],
            |client: ProxyClient| async move {
                let resp =
                    match get_or_skip(&client, "http://www.baidu.com/", "www.baidu.com").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp.status().as_u16() != 200 {
                    return skip_on_upstream_error("www.baidu.com", resp.status().as_u16());
                }
                let body = resp.text().await.unwrap_or_default();
                assert_body_contains(&body, "baidu")?;
                Ok(())
            },
        ),
        TestCase::new(
            "public_baidu_with_header",
            "public",
            vec!["www.baidu.com reqHeaders://X-Test-Header=bifrost-e2e"],
            |client: ProxyClient| async move {
                let resp =
                    match get_or_skip(&client, "http://www.baidu.com/", "www.baidu.com").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp.status().as_u16() != 200 {
                    return skip_on_upstream_error("www.baidu.com", resp.status().as_u16());
                }
                let body = resp.text().await.unwrap_or_default();
                assert_body_contains(&body, "baidu")?;
                Ok(())
            },
        ),
        TestCase::new(
            "public_qq_basic",
            "public",
            vec![],
            |client: ProxyClient| async move {
                let resp = match get_or_skip(&client, "http://www.qq.com/", "www.qq.com").await {
                    Ok(r) => r,
                    Err(result) => return result,
                };
                let status = resp.status().as_u16();
                match status {
                    200 | 301 | 302 => Ok(()),
                    _ => skip_on_upstream_error("www.qq.com", status),
                }
            },
        ),
        TestCase::new(
            "public_sina_basic",
            "public",
            vec![],
            |client: ProxyClient| async move {
                let resp = match get_or_skip(&client, "http://www.sina.com.cn/", "www.sina.com.cn")
                    .await
                {
                    Ok(r) => r,
                    Err(result) => return result,
                };
                let status = resp.status().as_u16();
                match status {
                    200 | 301 | 302 => Ok(()),
                    _ => skip_on_upstream_error("www.sina.com.cn", status),
                }
            },
        ),
        TestCase::standalone(
            "public_httpbin_ip",
            "Public IP endpoint via local httpbin mock",
            "public",
            test_public_httpbin_ip,
        ),
        TestCase::standalone(
            "public_httpbin_host_redirect",
            "Host redirect to local httpbin mock",
            "public",
            test_public_httpbin_host_redirect,
        ),
        TestCase::new(
            "public_cors_injection",
            "public",
            vec!["www.baidu.com resHeaders://Access-Control-Allow-Origin=*"],
            |client: ProxyClient| async move {
                let resp =
                    match get_or_skip(&client, "http://www.baidu.com/", "www.baidu.com").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp.status().as_u16() != 200 {
                    return skip_on_upstream_error("www.baidu.com", resp.status().as_u16());
                }
                assert_header_value(&resp, "access-control-allow-origin", "*")?;
                Ok(())
            },
        ),
        TestCase::new(
            "public_custom_header_injection",
            "public",
            vec!["www.baidu.com resHeaders://X-Proxy-By=bifrost, X-Test-Time: 2024"],
            |client: ProxyClient| async move {
                let resp =
                    match get_or_skip(&client, "http://www.baidu.com/", "www.baidu.com").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp.status().as_u16() != 200 {
                    return skip_on_upstream_error("www.baidu.com", resp.status().as_u16());
                }
                assert_header_value(&resp, "x-proxy-by", "bifrost")?;
                assert_header_value(&resp, "x-test-time", "2024")?;
                Ok(())
            },
        ),
        TestCase::new(
            "public_wildcard_pattern",
            "public",
            vec!["*.baidu.com resHeaders://X-Matched=wildcard"],
            |client: ProxyClient| async move {
                let resp =
                    match get_or_skip(&client, "http://www.baidu.com/", "www.baidu.com").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp.status().as_u16() != 200 {
                    return skip_on_upstream_error("www.baidu.com", resp.status().as_u16());
                }
                assert_header_value(&resp, "x-matched", "wildcard")?;
                Ok(())
            },
        ),
        TestCase::new(
            "public_multi_site_rule",
            "public",
            vec![
                "www.baidu.com resHeaders://X-Site=baidu",
                "httpbin.org resHeaders://X-Site=httpbin",
            ],
            |client: ProxyClient| async move {
                let resp1 =
                    match get_or_skip(&client, "http://www.baidu.com/", "www.baidu.com").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp1.status().as_u16() != 200 {
                    return skip_on_upstream_error("www.baidu.com", resp1.status().as_u16());
                }
                assert_header_value(&resp1, "x-site", "baidu")?;

                let resp2 =
                    match get_or_skip(&client, "http://httpbin.org/get", "httpbin.org").await {
                        Ok(r) => r,
                        Err(result) => return result,
                    };
                if resp2.status().as_u16() != 200 {
                    return skip_on_upstream_error("httpbin.org", resp2.status().as_u16());
                }
                assert_header_value(&resp2, "x-site", "httpbin")?;
                Ok(())
            },
        ),
    ]
}

async fn test_public_httpbin_ip() -> Result<(), String> {
    let mock = HttpbinMockServer::start().await;
    let port = portpicker::pick_unused_port().unwrap();
    let rules = mock.http_rules();
    let rule_refs: Vec<&str> = rules.iter().map(String::as_str).collect();
    let _proxy = ProxyInstance::start(port, rule_refs)
        .await
        .map_err(|e| format!("Failed to start proxy: {e}"))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client =
        ProxyClient::new(&format!("http://127.0.0.1:{port}")).map_err(|e| e.to_string())?;
    let json = client
        .get_json("http://httpbin.org/ip")
        .await
        .map_err(|e| e.to_string())?;

    if json.get("origin").is_some() {
        Ok(())
    } else {
        Err("Expected 'origin' field in response".to_string())
    }
}

async fn test_public_httpbin_host_redirect() -> Result<(), String> {
    let mock = HttpbinMockServer::start().await;
    let port = portpicker::pick_unused_port().unwrap();
    let mut rules = mock.http_rules();
    // Avoid outbound dependency and avoid `.local` (may be bypassed and trigger DNS failures in CI).
    rules.push(format!(
        "my-test-domain.invalid host://127.0.0.1:{}",
        mock.http_port
    ));
    let rule_refs: Vec<&str> = rules.iter().map(String::as_str).collect();
    let _proxy = ProxyInstance::start(port, rule_refs)
        .await
        .map_err(|e| format!("Failed to start proxy: {e}"))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client =
        ProxyClient::new(&format!("http://127.0.0.1:{port}")).map_err(|e| e.to_string())?;
    let json = client
        .get_json("http://my-test-domain.invalid/get")
        .await
        .map_err(|e| e.to_string())?;

    if json.get("url").is_some() {
        Ok(())
    } else {
        Err("Expected httpbin response with 'url' field".to_string())
    }
}
