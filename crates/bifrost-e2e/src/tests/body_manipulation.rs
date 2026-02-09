use crate::curl::CurlCommand;
use crate::mock::EnhancedMockServer;
use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use std::collections::HashMap;
use std::time::Duration;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "body_file_inline_json",
            "file 规则返回内联 JSON",
            "body",
            test_file_inline_json,
        ),
        TestCase::standalone(
            "body_rawfile_inline",
            "rawfile 规则返回内联内容（无自动响应头）",
            "body",
            test_rawfile_inline,
        ),
        TestCase::standalone(
            "body_tpl_timestamp",
            "tpl 规则返回包含时间戳的模板",
            "body",
            test_tpl_timestamp,
        ),
        TestCase::standalone(
            "body_tpl_uuid",
            "tpl 规则返回包含 UUID 的模板",
            "body",
            test_tpl_uuid,
        ),
        TestCase::standalone(
            "body_tpl_request_info",
            "tpl 规则返回请求信息（method/path）",
            "body",
            test_tpl_request_info,
        ),
        TestCase::standalone(
            "body_reqBody_set_json",
            "reqBody 规则设置请求 Body",
            "body",
            test_reqbody_set_json,
        ),
        TestCase::standalone(
            "body_resBody_set_json",
            "resBody 规则设置响应 Body",
            "body",
            test_resbody_set_json,
        ),
        TestCase::standalone(
            "body_reqReplace_simple",
            "reqReplace 规则简单替换请求 Body",
            "body",
            test_reqreplace_simple,
        ),
        TestCase::standalone(
            "body_resReplace_simple",
            "resReplace 规则简单替换响应 Body",
            "body",
            test_resreplace_simple,
        ),
        TestCase::standalone(
            "body_resReplace_regex_global",
            "resReplace 正则全局替换",
            "body",
            test_resreplace_regex_global,
        ),
        TestCase::standalone(
            "body_reqMerge_add_field",
            "reqMerge 规则添加 JSON 字段",
            "body",
            test_reqmerge_add_field,
        ),
        TestCase::standalone(
            "body_resMerge_add_field",
            "resMerge 规则添加响应 JSON 字段",
            "body",
            test_resmerge_add_field,
        ),
        TestCase::standalone(
            "body_resAppend_content",
            "resAppend 规则在响应末尾追加内容",
            "body",
            test_resappend_content,
        ),
        TestCase::standalone(
            "body_resPrepend_content",
            "resPrepend 规则在响应开头插入内容",
            "body",
            test_resprepend_content,
        ),
        TestCase::standalone(
            "body_htmlAppend_script",
            "htmlAppend 规则在 HTML 末尾注入脚本",
            "body",
            test_htmlappend_script,
        ),
        TestCase::standalone(
            "body_jsAppend_code",
            "jsAppend 规则在 JS 末尾追加代码",
            "body",
            test_jsappend_code,
        ),
    ]
}

async fn test_file_inline_json() -> Result<(), String> {
    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(port, vec!["test.local/api file://({\"ok\":true})"])
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
    result.assert_body_contains("{\"ok\":true}")?;
    Ok(())
}

async fn test_rawfile_inline() -> Result<(), String> {
    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(port, vec!["test.local/raw rawfile://(raw-content-here)"])
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/raw",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("raw-content-here")?;
    Ok(())
}

async fn test_tpl_timestamp() -> Result<(), String> {
    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(port, vec!["test.local/tpl tpl://`({\"time\":${now}})`"])
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/tpl",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("\"time\":")?;
    let body = &result.body;
    if !body.contains(|c: char| c.is_ascii_digit()) {
        return Err("Expected timestamp in response".to_string());
    }
    Ok(())
}

async fn test_tpl_uuid() -> Result<(), String> {
    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec!["test.local/uuid tpl://`({\"id\":\"${randomUUID}\"})`"],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/uuid",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("\"id\":\"")?;
    if !result.body.contains("-") {
        return Err("Expected UUID format with dashes".to_string());
    }
    Ok(())
}

async fn test_tpl_request_info() -> Result<(), String> {
    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec!["test.local/info tpl://`({\"method\":\"${method}\"})`"],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/info",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("\"method\":\"GET\"")?;
    Ok(())
}

async fn test_reqbody_set_json() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "ok");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} reqBody://({{\"injected\":true}})",
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
    .method("POST")
    .data("{\"original\":1}")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    let body = req.body.ok_or("No body in request")?;
    if !body.contains("{\"injected\":true}") {
        return Err(format!("Expected injected body, got: {}", body));
    }
    Ok(())
}

async fn test_resbody_set_json() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "original-response");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} resBody://({{\"mocked\":true}})",
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
    result.assert_body_contains("{\"mocked\":true}")?;
    Ok(())
}

async fn test_reqreplace_simple() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "ok");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} reqReplace://old=new",
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
    .method("POST")
    .data("this is old value")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    let body = req.body.ok_or("No body in request")?;
    if !body.contains("this is new value") {
        return Err(format!("Expected replaced body, got: {}", body));
    }
    Ok(())
}

async fn test_resreplace_simple() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "production mode active");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} resReplace://production=development",
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
    result.assert_body_contains("development mode active")?;
    Ok(())
}

async fn test_resreplace_regex_global() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "aaa-bbb-aaa");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} resReplace://(/a/g=x)",
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
    result.assert_body_contains("xxx-bbb-xxx")?;
    Ok(())
}

async fn test_reqmerge_add_field() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "ok");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} reqMerge://(extra:\"added\")",
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
    .method("POST")
    .header("Content-Type", "application/json")
    .data("{\"original\":1}")
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;

    let req = mock.last_request().ok_or("No request received")?;
    let body = req.body.ok_or("No body in request")?;
    if !body.contains("extra") {
        return Err(format!("Expected merged body with 'extra', got: {}", body));
    }
    Ok(())
}

async fn test_resmerge_add_field() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    mock.set_response_with_headers(200, "{\"data\":[]}", headers);

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} resMerge://(proxy:true)",
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
    result.assert_body_contains("proxy")?;
    Ok(())
}

async fn test_resappend_content() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "original");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} resAppend://(--appended)",
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
    result.assert_body_contains("original")?;
    result.assert_body_contains("--appended")?;
    Ok(())
}

async fn test_resprepend_content() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    mock.set_response(200, "original");

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} resPrepend://(prefix--)",
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
    result.assert_body_contains("prefix--")?;
    result.assert_body_contains("original")?;
    Ok(())
}

async fn test_htmlappend_script() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "text/html".to_string());
    mock.set_response_with_headers(200, "<html><body>page</body></html>", headers);

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} htmlAppend://(<script>console.log('injected')</script>)",
            mock.port
        )],
    ).await.map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/page.html",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("<html>")?;
    result.assert_body_contains("<script>console.log('injected')</script>")?;
    Ok(())
}

async fn test_jsappend_code() -> Result<(), String> {
    let mock = EnhancedMockServer::start().await;
    let mut headers = HashMap::new();
    headers.insert(
        "Content-Type".to_string(),
        "application/javascript".to_string(),
    );
    mock.set_response_with_headers(200, "var app = {};", headers);

    let port = portpicker::pick_unused_port().unwrap();
    let _proxy = ProxyInstance::start(
        port,
        vec![&format!(
            "test.local host://127.0.0.1:{} jsAppend://(;console.log('loaded');)",
            mock.port
        )],
    )
    .await
    .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = CurlCommand::with_proxy(
        &format!("http://127.0.0.1:{}", port),
        "http://test.local/app.js",
    )
    .execute()
    .await
    .map_err(|e| format!("curl failed: {}", e))?;

    result.assert_success()?;
    result.assert_body_contains("var app = {};")?;
    result.assert_body_contains(";console.log('loaded');")?;
    Ok(())
}
