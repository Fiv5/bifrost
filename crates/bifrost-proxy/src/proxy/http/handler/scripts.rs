use std::collections::HashMap;
use std::sync::Arc;

use bifrost_admin::AdminState;
use bifrost_script::{RequestData, ResponseData, ScriptContext, ScriptType};

use crate::server::ResolvedRules;
use crate::utils::logging::RequestContext;

use super::decode::{build_matched_rules_info, parse_url_parts};

pub(super) fn headers_to_hashmap(headers: &[(String, String)]) -> HashMap<String, String> {
    headers.iter().cloned().collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_request_scripts(
    admin_state: &Option<Arc<AdminState>>,
    script_names: &[String],
    ctx: &RequestContext,
    resolved_rules: &ResolvedRules,
    url: &str,
    method: &mut String,
    headers: &mut HashMap<String, String>,
    body: &mut Option<String>,
    values: &HashMap<String, String>,
) -> Vec<bifrost_script::ScriptExecutionResult> {
    if script_names.is_empty() {
        return vec![];
    }

    let state = match admin_state {
        Some(s) => s,
        None => return vec![],
    };

    let manager = match &state.script_manager {
        Some(m) => m,
        None => return vec![],
    };

    let cfg = if let Some(cm) = state.config_manager.as_ref() {
        Some(cm.config().await)
    } else {
        None
    };

    let matched_rules = build_matched_rules_info(resolved_rules);
    let (host, path, protocol) = parse_url_parts(url);

    let mut request_data = RequestData {
        url: url.to_string(),
        method: method.clone(),
        host,
        path,
        protocol,
        client_ip: ctx.client_ip.clone(),
        client_app: ctx.client_app.clone(),
        headers: headers.clone(),
        body: body.clone(),
    };

    let script_ctx = ScriptContext {
        request_id: ctx.id_str().to_string(),
        script_name: script_names.first().cloned().unwrap_or_default(),
        script_type: ScriptType::Request,
        values: values.clone(),
        matched_rules,
    };

    let mgr = manager.read().await;
    let results = if let Some(ref cfg) = cfg {
        mgr.execute_request_scripts_with_config(script_names, &mut request_data, &script_ctx, cfg)
            .await
    } else {
        mgr.execute_request_scripts(script_names, &mut request_data, &script_ctx)
            .await
    };

    if results.iter().any(|r| r.success) {
        *method = request_data.method;
        *headers = request_data.headers;
        *body = request_data.body;
    }

    results
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_response_scripts(
    admin_state: &Option<Arc<AdminState>>,
    script_names: &[String],
    ctx: &RequestContext,
    resolved_rules: &ResolvedRules,
    request_url: &str,
    request_method: &str,
    request_headers: &HashMap<String, String>,
    status: &mut u16,
    status_text: &mut String,
    headers: &mut HashMap<String, String>,
    body: &mut Option<String>,
    values: &HashMap<String, String>,
) -> Vec<bifrost_script::ScriptExecutionResult> {
    if script_names.is_empty() {
        return vec![];
    }

    let state = match admin_state {
        Some(s) => s,
        None => return vec![],
    };

    let manager = match &state.script_manager {
        Some(m) => m,
        None => return vec![],
    };

    let cfg = if let Some(cm) = state.config_manager.as_ref() {
        Some(cm.config().await)
    } else {
        None
    };

    let matched_rules = build_matched_rules_info(resolved_rules);
    let (host, path, protocol) = parse_url_parts(request_url);

    let mut response_data = ResponseData {
        status: *status,
        status_text: status_text.clone(),
        headers: headers.clone(),
        body: body.clone(),
        request: RequestData {
            url: request_url.to_string(),
            method: request_method.to_string(),
            host,
            path,
            protocol,
            client_ip: ctx.client_ip.clone(),
            client_app: ctx.client_app.clone(),
            headers: request_headers.clone(),
            body: None,
        },
    };

    let script_ctx = ScriptContext {
        request_id: ctx.id_str().to_string(),
        script_name: script_names.first().cloned().unwrap_or_default(),
        script_type: ScriptType::Response,
        values: values.clone(),
        matched_rules,
    };

    let mgr = manager.read().await;
    let results = if let Some(ref cfg) = cfg {
        mgr.execute_response_scripts_with_config(script_names, &mut response_data, &script_ctx, cfg)
            .await
    } else {
        mgr.execute_response_scripts(script_names, &mut response_data, &script_ctx)
            .await
    };

    if results.iter().any(|r| r.success) {
        *status = response_data.status;
        *status_text = response_data.status_text;
        *headers = response_data.headers;
        *body = response_data.body;
    }

    results
}
