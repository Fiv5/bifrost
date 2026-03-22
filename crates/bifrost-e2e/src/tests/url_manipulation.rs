use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "url_urlParams_add_single",
            "urlParams 添加单个参数",
            "url",
            test_urlparams_add_single,
        ),
        TestCase::standalone(
            "url_urlParams_add_to_existing",
            "urlParams 添加到已有参数",
            "url",
            test_urlparams_add_to_existing,
        ),
        TestCase::standalone(
            "url_urlParams_override_existing",
            "urlParams 覆盖已有参数",
            "url",
            test_urlparams_override_existing,
        ),
        TestCase::standalone(
            "url_urlParams_parentheses_format",
            "urlParams 小括号格式",
            "url",
            test_urlparams_parentheses_format,
        ),
        TestCase::standalone(
            "url_pathReplace_simple",
            "pathReplace 简单替换",
            "url",
            test_pathreplace_simple,
        ),
        TestCase::standalone(
            "url_pathReplace_version",
            "pathReplace 版本替换 v1->v2",
            "url",
            test_pathreplace_version,
        ),
        TestCase::standalone(
            "url_pathReplace_regex",
            "pathReplace 正则替换",
            "url",
            test_pathreplace_regex,
        ),
        TestCase::standalone(
            "url_combined_path_and_params",
            "pathReplace + urlParams 组合",
            "url",
            test_combined_path_and_params,
        ),
        TestCase::standalone(
            "url_full_url_pattern_with_regex_path_replace",
            "Full URL pattern + regex pathReplace",
            "url",
            test_full_url_pattern_with_regex_path_replace,
        ),
        TestCase::standalone(
            "url_bare_target_rewrite_chain_with_regex_path_replace",
            "Bare target + urlParams + regex pathReplace chain",
            "url",
            test_bare_target_rewrite_chain_with_regex_path_replace,
        ),
        TestCase::standalone(
            "url_full_url_bare_target_with_regex_path_replace",
            "Full URL pattern + bare target + regex pathReplace",
            "url",
            test_full_url_bare_target_with_regex_path_replace,
        ),
    ]
}

async fn test_urlparams_add_single() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} urlParams://debug=true",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if let Some(query) = &req.query {
        if !query.contains("debug=true") {
            return Err(format!("Expected debug=true in query, got: {}", query));
        }
    } else {
        return Err("Query string missing".to_string());
    }

    Ok(())
}

async fn test_urlparams_add_to_existing() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} urlParams://b=2",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api?a=1",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if let Some(query) = &req.query {
        if !query.contains("a=1") || !query.contains("b=2") {
            return Err(format!("Expected a=1 and b=2 in query, got: {}", query));
        }
    } else {
        return Err("Query string missing".to_string());
    }

    Ok(())
}

async fn test_urlparams_override_existing() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} urlParams://a=new",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api?a=old",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if let Some(query) = &req.query {
        if !query.contains("a=new") {
            return Err(format!("Expected a=new in query, got: {}", query));
        }
        if query.contains("a=old") {
            return Err("Old parameter value should be overridden".to_string());
        }
    } else {
        return Err("Query string missing".to_string());
    }

    Ok(())
}

async fn test_urlparams_parentheses_format() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} urlParams://(x:1)",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result =
        CurlCommand::with_proxy(&format!("http://127.0.0.1:{}", port), "http://test.local/")
            .execute()
            .await
            .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if let Some(query) = &req.query {
        if !query.contains("x=1") {
            return Err(format!("Expected x=1 in query, got: {}", query));
        }
    } else {
        return Err("Query string missing".to_string());
    }

    Ok(())
}

async fn test_pathreplace_simple() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} pathReplace://old=new",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/old/path",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if !req.path.contains("/new/path") {
        return Err(format!("Expected /new/path in path, got: {}", req.path));
    }

    Ok(())
}

async fn test_pathreplace_version() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} pathReplace://v1=v2",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api/v1/users",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if !req.path.contains("/api/v2/users") {
        return Err(format!("Expected /api/v2/users in path, got: {}", req.path));
    }

    Ok(())
}

async fn test_pathreplace_regex() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} pathReplace://(/v\\d+/=v99)",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/api/v1/users",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if !req.path.contains("v99") {
        return Err(format!("Expected v99 in path, got: {}", req.path));
    }

    Ok(())
}

async fn test_combined_path_and_params() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} pathReplace://old=new urlParams://migrated=true",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/old/api",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if !req.path.contains("/new/api") {
        return Err(format!("Expected /new/api in path, got: {}", req.path));
    }
    if let Some(query) = &req.query {
        if !query.contains("migrated=true") {
            return Err(format!("Expected migrated=true in query, got: {}", query));
        }
    } else {
        return Err("Query string missing".to_string());
    }

    Ok(())
}

async fn test_full_url_pattern_with_regex_path_replace() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "http://regex-op.test/api host://127.0.0.1:{} pathReplace://(/api/=/rewritten/)",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://regex-op.test/api/users",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if req.path != "/rewritten/users" {
        return Err(format!(
            "Expected rewritten path /rewritten/users, got: {}",
            req.path
        ));
    }

    Ok(())
}

async fn test_bare_target_rewrite_chain_with_regex_path_replace() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start_with_rules_text(
        port,
        &format!(
            "rewrite-chain.rule.test 127.0.0.1:{} urlParams://keep=rewritten urlParams://remove_me= pathReplace://(/legacy/v\\d+/=/api/v99)",
            mock.port
        ),
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://rewrite-chain.rule.test/legacy/v1/users?keep=original&remove_me=yes",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if req.path != "/api/v99/users" {
        return Err(format!(
            "Expected rewritten path /api/v99/users, got: {}",
            req.path
        ));
    }

    let query = req.query.ok_or("Query string missing".to_string())?;
    if !query.contains("keep=rewritten") {
        return Err(format!(
            "Expected keep=rewritten in query after override, got: {}",
            query
        ));
    }
    if query.contains("remove_me=") || query.contains("remove_me=yes") {
        return Err(format!(
            "Expected remove_me to be deleted from query, got: {}",
            query
        ));
    }

    Ok(())
}

async fn test_full_url_bare_target_with_regex_path_replace() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start_with_rules_text(
        port,
        &format!(
            "http://full-url-bare.rule.test/api 127.0.0.1:{} pathReplace://(/api/=/edge/)",
            mock.port
        ),
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://full-url-bare.rule.test/api/users",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    if req.path != "/edge/users" {
        return Err(format!(
            "Expected rewritten path /edge/users, got: {}",
            req.path
        ));
    }

    Ok(())
}
