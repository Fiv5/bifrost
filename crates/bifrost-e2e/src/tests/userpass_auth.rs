use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use bifrost_core::{UserPassAccountConfig, UserPassAuthConfig};
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "userpass_http_correct_credentials",
            "HTTP proxy: correct user:password passes through",
            "userpass_auth",
            test_http_correct_credentials,
        ),
        TestCase::standalone(
            "userpass_http_wrong_credentials",
            "HTTP proxy: wrong password returns 407",
            "userpass_auth",
            test_http_wrong_credentials,
        ),
        TestCase::standalone(
            "userpass_http_no_credentials",
            "HTTP proxy: missing credentials returns 407",
            "userpass_auth",
            test_http_no_credentials,
        ),
        TestCase::standalone(
            "userpass_http_multi_accounts",
            "HTTP proxy: multiple accounts, any enabled account passes",
            "userpass_auth",
            test_http_multi_accounts,
        ),
        TestCase::standalone(
            "userpass_http_disabled_account",
            "HTTP proxy: disabled account is rejected",
            "userpass_auth",
            test_http_disabled_account,
        ),
        TestCase::standalone(
            "userpass_socks5_correct_credentials",
            "SOCKS5 proxy: correct user:password passes through",
            "userpass_auth",
            test_socks5_correct_credentials,
        ),
        TestCase::standalone(
            "userpass_socks5_wrong_credentials",
            "SOCKS5 proxy: wrong password is rejected",
            "userpass_auth",
            test_socks5_wrong_credentials,
        ),
    ]
}

fn make_auth_config(accounts: Vec<(&str, &str, bool)>) -> UserPassAuthConfig {
    UserPassAuthConfig {
        enabled: true,
        accounts: accounts
            .into_iter()
            .map(|(username, password, enabled)| UserPassAccountConfig {
                username: username.to_string(),
                password: if password.is_empty() {
                    None
                } else {
                    Some(password.to_string())
                },
                enabled,
            })
            .collect(),
    }
}

async fn test_http_correct_credentials() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "auth_ok");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "secret123", true)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("alice:secret123")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("auth_ok")?;
    Ok(())
}

async fn test_http_wrong_credentials() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "should_not_reach");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "secret123", true)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("alice:wrongpass")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_status(407)?;
    Ok(())
}

async fn test_http_no_credentials() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "should_not_reach");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "secret123", true)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_status(407)?;
    Ok(())
}

async fn test_http_multi_accounts() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "multi_ok");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "pass_a", true), ("bob", "pass_b", true)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result_alice = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("alice:pass_a")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;
    result_alice.assert_success()?;
    result_alice.assert_body_contains("multi_ok")?;

    let result_bob = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("bob:pass_b")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;
    result_bob.assert_success()?;
    result_bob.assert_body_contains("multi_ok")?;

    let result_wrong = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("charlie:pass_c")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;
    result_wrong.assert_status(407)?;

    Ok(())
}

async fn test_http_disabled_account() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "should_not_reach");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "secret123", false)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("alice:secret123")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_status(407)?;
    Ok(())
}

async fn test_socks5_correct_credentials() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "socks5_auth_ok");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "secret123", true)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = CurlCommand::with_proxy(
        &format!("socks5h://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("alice:secret123")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("socks5_auth_ok")?;
    Ok(())
}

async fn test_socks5_wrong_credentials() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "should_not_reach");

    let port = portpicker::pick_unused_port().unwrap();
    let auth_config = make_auth_config(vec![("alice", "secret123", true)]);
    let _proxy = ProxyInstance::start_with_userpass(
        port,
        vec![&format!("test.local host://127.0.0.1:{}", mock.port)],
        auth_config,
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = CurlCommand::with_proxy(
        &format!("socks5h://127.0.0.1:{}", port),
        "http://test.local/hello",
    )
    .proxy_user("alice:wrongpass")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    if result.exit_code == 0 {
        return Err("Expected SOCKS5 auth failure but curl succeeded".to_string());
    }

    Ok(())
}
