use bifrost_core::{
    validate_rules_with_context, ParseError, ParseErrorSeverity, ScriptReference, VariableInfo,
};
use bifrost_storage::{ConfigChangeEvent, RuleFile};
use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::push::SharedPushManager;
use crate::state::SharedAdminState;

#[derive(Debug, Serialize)]
struct RuleFileInfo {
    name: String,
    enabled: bool,
    rule_count: usize,
    error_count: usize,
    warning_count: usize,
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

#[derive(Debug, Deserialize)]
struct RenameRuleRequest {
    new_name: String,
}

#[derive(Debug, Deserialize)]
struct ValidateRuleRequest {
    content: String,
    #[serde(default)]
    global_values: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ValidateRuleResponse {
    valid: bool,
    rule_count: usize,
    errors: Vec<ParseError>,
    warnings: Vec<ParseError>,
    defined_variables: Vec<VariableInfo>,
    script_references: Vec<ScriptReference>,
}

pub async fn handle_rules(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/rules" || path == "/api/rules/" {
        match method {
            Method::GET => list_rules(state).await,
            Method::POST => create_rule(req, state, push_manager).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/rules/validate" {
        match method {
            Method::POST => validate_rule(req, state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(name) = path.strip_prefix("/api/rules/") {
        let name = urlencoding::decode(name).unwrap_or_default();
        let name = name.as_ref();

        if let Some(name) = name.strip_suffix("/enable") {
            match method {
                Method::PUT => enable_rule(state, name, true, push_manager).await,
                _ => method_not_allowed(),
            }
        } else if let Some(name) = name.strip_suffix("/disable") {
            match method {
                Method::PUT => enable_rule(state, name, false, push_manager).await,
                _ => method_not_allowed(),
            }
        } else if let Some(name) = name.strip_suffix("/rename") {
            match method {
                Method::PUT => rename_rule(req, state, name, push_manager).await,
                _ => method_not_allowed(),
            }
        } else {
            match method {
                Method::GET => get_rule(state, name).await,
                Method::PUT => update_rule(req, state, name, push_manager).await,
                Method::DELETE => delete_rule(state, name, push_manager).await,
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
                .map(|r| {
                    let result = validate_rules_with_context(&r.content, &HashMap::new());
                    RuleFileInfo {
                        name: r.name.clone(),
                        enabled: r.enabled,
                        rule_count: result.rule_count,
                        error_count: result.errors.len(),
                        warning_count: result.warnings.len(),
                    }
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

async fn validate_rule(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            )
        }
    };

    let request: ValidateRuleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    let content = request.content.clone();
    let global_values = request.global_values.clone();

    let validation_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        validate_rules_with_context(&content, &global_values)
    }));

    let mut result = match validation_result {
        Ok(r) => r,
        Err(e) => {
            let panic_msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic during validation".to_string()
            };

            tracing::error!(
                target: "bifrost_admin::rules",
                error = %panic_msg,
                "Validation panic caught - returning safe error response"
            );

            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Validation failed unexpectedly: {}", panic_msg),
            );
        }
    };

    if let Some(ref script_manager) = state.script_manager {
        let manager = script_manager.read().await;
        let engine = manager.engine();

        let req_scripts: std::collections::HashSet<String> = engine
            .list_scripts(bifrost_script::ScriptType::Request)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.name)
            .collect();

        let res_scripts: std::collections::HashSet<String> = engine
            .list_scripts(bifrost_script::ScriptType::Response)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.name)
            .collect();

        for script_ref in &result.script_references {
            let scripts = if script_ref.script_type == "request" {
                &req_scripts
            } else {
                &res_scripts
            };

            if !scripts.contains(&script_ref.name) {
                let warning = ParseError::with_range(
                    script_ref.line,
                    1,
                    script_ref.name.len() + 15,
                    format!(
                        "Script '{}' not found. Available {} scripts: {}",
                        script_ref.name,
                        script_ref.script_type,
                        if scripts.is_empty() {
                            "(none)".to_string()
                        } else {
                            scripts.iter().cloned().collect::<Vec<_>>().join(", ")
                        }
                    ),
                )
                .with_severity(ParseErrorSeverity::Warning)
                .with_code("W003")
                .with_suggestion(format!(
                    "Create a {} script named '{}' in the scripts directory, or use an existing script.",
                    script_ref.script_type,
                    script_ref.name
                ));
                result.warnings.push(warning);
            }
        }
    }

    let response = ValidateRuleResponse {
        valid: result.valid,
        rule_count: result.rule_count,
        errors: result.errors,
        warnings: result.warnings,
        defined_variables: result.defined_variables,
        script_references: result.script_references,
    };

    json_response(&response)
}

async fn create_rule(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
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
        Ok(_) => {
            notify_rules_changed(&state);
            invalidate_overview_cache(&push_manager);
            success_response(&format!("Rule '{}' created successfully", request.name))
        }
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
    push_manager: Option<SharedPushManager>,
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
        Ok(_) => {
            notify_rules_changed(&state);
            invalidate_overview_cache(&push_manager);
            success_response(&format!("Rule '{}' updated successfully", name))
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update rule: {}", e),
        ),
    }
}

async fn delete_rule(
    state: SharedAdminState,
    name: &str,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    match state.rules_storage.delete(name) {
        Ok(_) => {
            notify_rules_changed(&state);
            invalidate_overview_cache(&push_manager);
            success_response(&format!("Rule '{}' deleted successfully", name))
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete rule: {}", e),
        ),
    }
}

async fn enable_rule(
    state: SharedAdminState,
    name: &str,
    enabled: bool,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    match state.rules_storage.set_enabled(name, enabled) {
        Ok(_) => {
            notify_rules_changed(&state);
            invalidate_overview_cache(&push_manager);
            let action = if enabled { "enabled" } else { "disabled" };
            success_response(&format!("Rule '{}' {} successfully", name, action))
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update rule: {}", e),
        ),
    }
}

async fn rename_rule(
    req: Request<Incoming>,
    state: SharedAdminState,
    name: &str,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    if !state.rules_storage.exists(name) {
        return error_response(StatusCode::NOT_FOUND, &format!("Rule '{}' not found", name));
    }

    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            )
        }
    };

    let request: RenameRuleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    if request.new_name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "New rule name is required");
    }

    if request.new_name == name {
        return error_response(
            StatusCode::BAD_REQUEST,
            "New name is the same as the old name",
        );
    }

    match state.rules_storage.rename(name, &request.new_name) {
        Ok(_) => {
            notify_rules_changed(&state);
            invalidate_overview_cache(&push_manager);
            success_response(&format!(
                "Rule '{}' renamed to '{}' successfully",
                name, request.new_name
            ))
        }
        Err(e) => {
            let status = if e.to_string().contains("already exists") {
                StatusCode::CONFLICT
            } else if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error_response(status, &format!("Failed to rename rule: {}", e))
        }
    }
}

fn invalidate_overview_cache(push_manager: &Option<SharedPushManager>) {
    if let Some(pm) = push_manager {
        pm.invalidate_overview_cache();
    }
}

fn notify_rules_changed(state: &SharedAdminState) {
    if let Some(ref config_manager) = state.config_manager {
        match config_manager.notify(ConfigChangeEvent::RulesChanged) {
            Ok(count) => {
                tracing::info!(
                    target: "bifrost_admin::rules",
                    receivers = count,
                    "notified rules changed event"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "bifrost_admin::rules",
                    error = %e,
                    "failed to notify rules changed event (no receivers)"
                );
            }
        }
    } else {
        tracing::warn!(
            target: "bifrost_admin::rules",
            "config_manager is not available, cannot notify rules changed"
        );
    }
}
