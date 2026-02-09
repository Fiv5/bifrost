use bifrost_storage::RuleFile;
use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::state::SharedAdminState;

#[derive(Debug, Serialize)]
struct RuleFileInfo {
    name: String,
    enabled: bool,
    rule_count: usize,
}

#[derive(Debug, Serialize)]
struct RuleFileDetail {
    name: String,
    content: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct CreateRuleRequest {
    name: String,
    content: String,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateRuleRequest {
    content: Option<String>,
    enabled: Option<bool>,
}

pub async fn handle_rules(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/rules" || path == "/api/rules/" {
        match method {
            Method::GET => list_rules(state).await,
            Method::POST => create_rule(req, state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(name) = path.strip_prefix("/api/rules/") {
        let name = urlencoding::decode(name).unwrap_or_default();
        let name = name.as_ref();

        if let Some(name) = name.strip_suffix("/enable") {
            match method {
                Method::PUT => enable_rule(state, name, true).await,
                _ => method_not_allowed(),
            }
        } else if let Some(name) = name.strip_suffix("/disable") {
            match method {
                Method::PUT => enable_rule(state, name, false).await,
                _ => method_not_allowed(),
            }
        } else {
            match method {
                Method::GET => get_rule(state, name).await,
                Method::PUT => update_rule(req, state, name).await,
                Method::DELETE => delete_rule(state, name).await,
                _ => method_not_allowed(),
            }
        }
    } else {
        error_response(StatusCode::NOT_FOUND, "Not Found")
    }
}

async fn list_rules(state: SharedAdminState) -> Response<BoxBody> {
    match state.rules_storage.load_all() {
        Ok(rules) => {
            let infos: Vec<RuleFileInfo> = rules
                .iter()
                .map(|r| RuleFileInfo {
                    name: r.name.clone(),
                    enabled: r.enabled,
                    rule_count: r
                        .content
                        .lines()
                        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                        .count(),
                })
                .collect();
            json_response(&infos)
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to list rules: {}", e),
        ),
    }
}

async fn create_rule(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            )
        }
    };

    let request: CreateRuleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    if request.name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Rule name is required");
    }

    if state.rules_storage.exists(&request.name) {
        return error_response(StatusCode::CONFLICT, "Rule with this name already exists");
    }

    let rule = RuleFile::new(&request.name, &request.content)
        .with_enabled(request.enabled.unwrap_or(true));

    match state.rules_storage.save(&rule) {
        Ok(_) => success_response(&format!("Rule '{}' created successfully", request.name)),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to create rule: {}", e),
        ),
    }
}

async fn get_rule(state: SharedAdminState, name: &str) -> Response<BoxBody> {
    match state.rules_storage.load(name) {
        Ok(rule) => {
            let detail = RuleFileDetail {
                name: rule.name,
                content: rule.content,
                enabled: rule.enabled,
            };
            json_response(&detail)
        }
        Err(_) => error_response(StatusCode::NOT_FOUND, &format!("Rule '{}' not found", name)),
    }
}

async fn update_rule(
    req: Request<Incoming>,
    state: SharedAdminState,
    name: &str,
) -> Response<BoxBody> {
    let existing = match state.rules_storage.load(name) {
        Ok(r) => r,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, &format!("Rule '{}' not found", name))
        }
    };

    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            )
        }
    };

    let request: UpdateRuleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    let content = request.content.unwrap_or(existing.content);
    let enabled = request.enabled.unwrap_or(existing.enabled);

    let rule = RuleFile::new(name, content).with_enabled(enabled);

    match state.rules_storage.save(&rule) {
        Ok(_) => success_response(&format!("Rule '{}' updated successfully", name)),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update rule: {}", e),
        ),
    }
}

async fn delete_rule(state: SharedAdminState, name: &str) -> Response<BoxBody> {
    match state.rules_storage.delete(name) {
        Ok(_) => success_response(&format!("Rule '{}' deleted successfully", name)),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete rule: {}", e),
        ),
    }
}

async fn enable_rule(state: SharedAdminState, name: &str, enabled: bool) -> Response<BoxBody> {
    match state.rules_storage.set_enabled(name, enabled) {
        Ok(_) => {
            let action = if enabled { "enabled" } else { "disabled" };
            success_response(&format!("Rule '{}' {} successfully", name, action))
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update rule: {}", e),
        ),
    }
}
