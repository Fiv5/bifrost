use crate::handlers::{error_response, json_response, success_response, BoxBody};
use bifrost_script::{ScriptDetail, ScriptEngine, ScriptEngineConfig, ScriptInfo, ScriptType};
use http_body_util::BodyExt;
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

pub struct ScriptManager {
    engine: ScriptEngine,
}

impl ScriptManager {
    pub fn new(scripts_dir: PathBuf) -> Self {
        Self {
            engine: ScriptEngine::new(ScriptEngineConfig {
                scripts_dir,
                timeout_ms: 10000,
                max_memory: 16 * 1024 * 1024,
            }),
        }
    }

    pub async fn init(&self) -> Result<(), bifrost_script::ScriptError> {
        self.engine.init().await
    }

    pub fn engine(&self) -> &ScriptEngine {
        &self.engine
    }

    pub async fn execute_request_scripts(
        &self,
        script_names: &[String],
        request: &mut bifrost_script::RequestData,
        ctx: &bifrost_script::ScriptContext,
    ) -> Vec<bifrost_script::ScriptExecutionResult> {
        let mut results = Vec::new();
        for script_name in script_names {
            let result = self
                .engine
                .execute_request_script(script_name, request, ctx)
                .await;
            results.push(result);
        }
        results
    }

    pub async fn execute_response_scripts(
        &self,
        script_names: &[String],
        response: &mut bifrost_script::ResponseData,
        ctx: &bifrost_script::ScriptContext,
    ) -> Vec<bifrost_script::ScriptExecutionResult> {
        let mut results = Vec::new();
        for script_name in script_names {
            let result = self
                .engine
                .execute_response_script(script_name, response, ctx)
                .await;
            results.push(result);
        }
        results
    }
}

#[derive(Serialize)]
struct ScriptsListResponse {
    request: Vec<ScriptInfo>,
    response: Vec<ScriptInfo>,
}

#[derive(Deserialize)]
pub struct SaveScriptRequest {
    pub content: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct TestScriptRequest {
    #[serde(rename = "type")]
    pub script_type: ScriptType,
    pub content: String,
    #[serde(default)]
    pub mock_request: Option<MockRequestData>,
    #[serde(default)]
    pub mock_response: Option<MockResponseData>,
}

#[derive(Deserialize, Default)]
pub struct MockRequestData {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

fn default_url() -> String {
    "https://example.com/api".to_string()
}

fn default_method() -> String {
    "GET".to_string()
}

#[derive(Deserialize, Default)]
pub struct MockResponseData {
    #[serde(default = "default_status")]
    pub status: u16,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

fn default_status() -> u16 {
    200
}

pub async fn handle_scripts_request(
    req: Request<hyper::body::Incoming>,
    script_manager: Arc<RwLock<ScriptManager>>,
    admin_path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();
    let path = admin_path.to_string();

    match method {
        Method::GET => handle_get(path, script_manager).await,
        Method::PUT => handle_put(req, path, script_manager).await,
        Method::DELETE => handle_delete(path, script_manager).await,
        Method::POST => handle_post(req, path, script_manager).await,
        _ => error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    }
}

async fn handle_get(path: String, script_manager: Arc<RwLock<ScriptManager>>) -> Response<BoxBody> {
    let manager = script_manager.read().await;

    if path == "/api/scripts" || path == "/api/scripts/" {
        let request_scripts = manager
            .engine()
            .list_scripts(ScriptType::Request)
            .await
            .unwrap_or_default();
        let response_scripts = manager
            .engine()
            .list_scripts(ScriptType::Response)
            .await
            .unwrap_or_default();

        return json_response(&ScriptsListResponse {
            request: request_scripts,
            response: response_scripts,
        });
    }

    let remaining = path.trim_start_matches("/api/scripts/");
    let first_slash = remaining.find('/');
    if first_slash.is_none() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid path: missing script name");
    }

    let (type_str, name) = remaining.split_at(first_slash.unwrap());
    let name = name.trim_start_matches('/');

    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid path: empty script name");
    }

    let script_type = match type_str {
        "request" => ScriptType::Request,
        "response" => ScriptType::Response,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid script type"),
    };

    match manager.engine().load_script(script_type, name).await {
        Ok(content) => {
            let detail = ScriptDetail {
                info: ScriptInfo {
                    name: name.to_string(),
                    script_type,
                    description: None,
                    created_at: 0,
                    updated_at: 0,
                },
                content,
            };
            json_response(&detail)
        }
        Err(e) => {
            error!("Failed to load script {}/{}: {}", script_type, name, e);
            error_response(StatusCode::NOT_FOUND, &format!("Script not found: {}", e))
        }
    }
}

async fn handle_put(
    req: Request<hyper::body::Incoming>,
    path: String,
    script_manager: Arc<RwLock<ScriptManager>>,
) -> Response<BoxBody> {
    let remaining = path.trim_start_matches("/api/scripts/");
    let first_slash = remaining.find('/');
    if first_slash.is_none() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid path: missing script name");
    }

    let (type_str, name) = remaining.split_at(first_slash.unwrap());
    let name = name.trim_start_matches('/');

    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid path: empty script name");
    }

    let script_type = match type_str {
        "request" => ScriptType::Request,
        "response" => ScriptType::Response,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid script type"),
    };

    let body = match req.collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            );
        }
    };

    let save_req: SaveScriptRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e));
        }
    };

    let manager = script_manager.read().await;
    match manager
        .engine()
        .save_script(script_type, name, &save_req.content)
        .await
    {
        Ok(()) => {
            info!("Saved {} script: {}", script_type, name);
            let detail = ScriptDetail {
                info: ScriptInfo {
                    name: name.to_string(),
                    script_type,
                    description: save_req.description,
                    created_at: chrono::Utc::now().timestamp_millis() as u64,
                    updated_at: chrono::Utc::now().timestamp_millis() as u64,
                },
                content: save_req.content,
            };
            json_response(&detail)
        }
        Err(e) => {
            error!("Failed to save script {}/{}: {}", script_type, name, e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to save script: {}", e),
            )
        }
    }
}

async fn handle_delete(
    path: String,
    script_manager: Arc<RwLock<ScriptManager>>,
) -> Response<BoxBody> {
    let remaining = path.trim_start_matches("/api/scripts/");
    let first_slash = remaining.find('/');
    if first_slash.is_none() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid path: missing script name");
    }

    let (type_str, name) = remaining.split_at(first_slash.unwrap());
    let name = name.trim_start_matches('/');

    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid path: empty script name");
    }

    let script_type = match type_str {
        "request" => ScriptType::Request,
        "response" => ScriptType::Response,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid script type"),
    };

    let manager = script_manager.read().await;
    match manager.engine().delete_script(script_type, name).await {
        Ok(()) => {
            info!("Deleted {} script: {}", script_type, name);
            success_response(&format!("Script {} deleted", name))
        }
        Err(e) => {
            error!("Failed to delete script {}/{}: {}", script_type, name, e);
            error_response(StatusCode::NOT_FOUND, &format!("Failed to delete: {}", e))
        }
    }
}

async fn handle_post(
    req: Request<hyper::body::Incoming>,
    path: String,
    script_manager: Arc<RwLock<ScriptManager>>,
) -> Response<BoxBody> {
    if path != "/api/scripts/test" && path != "/api/scripts/test/" {
        return error_response(StatusCode::NOT_FOUND, "Not found");
    }

    let body = match req.collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            );
        }
    };

    let test_req: TestScriptRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e));
        }
    };

    let manager = script_manager.read().await;

    let mock_request = test_req.mock_request.unwrap_or_default();
    let mock_response = test_req.mock_response.unwrap_or_default();

    let request_data = bifrost_script::RequestData {
        url: mock_request.url.clone(),
        method: mock_request.method.clone(),
        host: url::Url::parse(&mock_request.url)
            .map(|u| u.host_str().unwrap_or("").to_string())
            .unwrap_or_default(),
        path: url::Url::parse(&mock_request.url)
            .map(|u| u.path().to_string())
            .unwrap_or_default(),
        protocol: url::Url::parse(&mock_request.url)
            .map(|u| u.scheme().to_string())
            .unwrap_or_else(|_| "https".to_string()),
        client_ip: "127.0.0.1".to_string(),
        client_app: Some("test".to_string()),
        headers: mock_request.headers,
        body: mock_request.body,
    };

    let response_data = bifrost_script::ResponseData {
        status: mock_response.status,
        status_text: "OK".to_string(),
        headers: mock_response.headers,
        body: mock_response.body,
        request: request_data.clone(),
    };

    let ctx = bifrost_script::ScriptContext {
        request_id: "test".to_string(),
        script_name: "test".to_string(),
        script_type: test_req.script_type,
        values: std::collections::HashMap::new(),
        matched_rules: vec![],
    };

    let result = manager
        .engine()
        .test_script(
            test_req.script_type,
            &test_req.content,
            Some(&request_data),
            Some(&response_data),
            &ctx,
        )
        .await;

    json_response(&result)
}
