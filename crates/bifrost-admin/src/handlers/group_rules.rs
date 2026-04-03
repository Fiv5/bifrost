use bifrost_storage::RulesStorage;
use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::state::SharedAdminState;
use bifrost_storage::ConfigChangeEvent;

#[derive(Debug, Deserialize)]
struct RemoteResponse<T> {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    message: String,
    data: T,
}

#[derive(Debug, Deserialize)]
struct RemoteListPayload<T> {
    list: Option<Vec<T>>,
    #[allow(dead_code)]
    total: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
struct RemotePeer {
    user_id: String,
    channel: i32,
    group_id: Option<String>,
    editable: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct RemoteRoom {
    user_id: String,
    level: i32,
    group_id: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RemoteEnv {
    id: String,
    user_id: String,
    name: String,
    rule: String,
    create_time: String,
    update_time: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RemoteGroup {
    #[allow(dead_code)]
    id: String,
    name: String,
    #[allow(dead_code)]
    visibility: Option<i32>,
}

#[derive(Debug, Serialize)]
struct GroupRuleInfo {
    name: String,
    enabled: bool,
    sort_order: i32,
    rule_count: usize,
    created_at: String,
    updated_at: String,
    remote_env_id: Option<String>,
    remote_user_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct GroupRuleDetail {
    name: String,
    content: String,
    enabled: bool,
    sort_order: i32,
    created_at: String,
    updated_at: String,
    sync: GroupRuleSyncInfo,
}

#[derive(Debug, Serialize)]
struct GroupRuleSyncInfo {
    status: String,
    remote_id: Option<String>,
    remote_updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct GroupRulesResponse {
    group_id: String,
    group_name: String,
    writable: bool,
    rules: Vec<GroupRuleInfo>,
}

#[derive(Debug, Deserialize)]
struct CreateGroupRuleRequest {
    name: String,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateGroupRuleRequest {
    content: String,
}

fn sanitize_group_dir_name(name: &str) -> String {
    name.replace(['/', '\\', '\0', ':'], "_")
}

async fn proxy_get_json<T: serde::de::DeserializeOwned>(
    sync_manager: &bifrost_sync::SyncManager,
    path: &str,
    query: Option<&str>,
) -> Result<T, String> {
    let (status, _content_type, body) = sync_manager
        .proxy_forward(reqwest::Method::GET, path, query, None)
        .await
        .map_err(|e| format!("proxy_forward failed: {e}"))?;
    if status != 200 {
        let body_str = String::from_utf8_lossy(&body);
        return Err(format!("Remote API returned status {status}: {body_str}"));
    }
    serde_json::from_slice(&body).map_err(|e| format!("JSON parse error: {e}"))
}

async fn proxy_post_json<T: serde::de::DeserializeOwned>(
    sync_manager: &bifrost_sync::SyncManager,
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    let body_bytes = serde_json::to_vec(body).map_err(|e| format!("JSON encode: {e}"))?;
    let (status, _ct, resp_body) = sync_manager
        .proxy_forward(reqwest::Method::POST, path, None, Some(body_bytes))
        .await
        .map_err(|e| format!("proxy_forward failed: {e}"))?;
    if status != 200 {
        let s = String::from_utf8_lossy(&resp_body);
        return Err(format!("Remote API returned status {status}: {s}"));
    }
    serde_json::from_slice(&resp_body).map_err(|e| format!("JSON parse error: {e}"))
}

async fn proxy_patch_json<T: serde::de::DeserializeOwned>(
    sync_manager: &bifrost_sync::SyncManager,
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    let body_bytes = serde_json::to_vec(body).map_err(|e| format!("JSON encode: {e}"))?;
    let (status, _ct, resp_body) = sync_manager
        .proxy_forward(reqwest::Method::PATCH, path, None, Some(body_bytes))
        .await
        .map_err(|e| format!("proxy_forward failed: {e}"))?;
    if status != 200 {
        let s = String::from_utf8_lossy(&resp_body);
        return Err(format!("Remote API returned status {status}: {s}"));
    }
    serde_json::from_slice(&resp_body).map_err(|e| format!("JSON parse error: {e}"))
}

async fn proxy_delete(sync_manager: &bifrost_sync::SyncManager, path: &str) -> Result<(), String> {
    let (status, _ct, resp_body) = sync_manager
        .proxy_forward(reqwest::Method::DELETE, path, None, None)
        .await
        .map_err(|e| format!("proxy_forward failed: {e}"))?;
    if status != 200 {
        let s = String::from_utf8_lossy(&resp_body);
        return Err(format!("Remote API returned status {status}: {s}"));
    }
    Ok(())
}

async fn fetch_group_info(
    sync_manager: &bifrost_sync::SyncManager,
    group_id: &str,
) -> Result<RemoteGroup, String> {
    let resp: RemoteResponse<RemoteGroup> =
        proxy_get_json(sync_manager, &format!("/v4/group/{group_id}"), None).await?;
    Ok(resp.data)
}

#[allow(dead_code)]
async fn fetch_room_members(
    sync_manager: &bifrost_sync::SyncManager,
    group_id: &str,
) -> Result<Vec<RemoteRoom>, String> {
    let query = format!("group_id={group_id}&offset=0&limit=500");
    let resp: RemoteResponse<RemoteListPayload<RemoteRoom>> =
        proxy_get_json(sync_manager, "/v4/room", Some(&query)).await?;
    Ok(resp.data.list.unwrap_or_default())
}

async fn fetch_user_envs(
    sync_manager: &bifrost_sync::SyncManager,
    user_id: &str,
) -> Result<Vec<RemoteEnv>, String> {
    let query = format!("user_id={user_id}&offset=0&limit=500");
    let resp: RemoteResponse<RemoteListPayload<RemoteEnv>> =
        proxy_get_json(sync_manager, "/v4/env", Some(&query)).await?;
    Ok(resp.data.list.unwrap_or_default())
}

async fn fetch_peers(sync_manager: &bifrost_sync::SyncManager) -> Result<Vec<RemotePeer>, String> {
    let resp: RemoteResponse<RemoteListPayload<RemotePeer>> =
        proxy_get_json(sync_manager, "/v4/user/peer", Some("offset=0&limit=500")).await?;
    Ok(resp.data.list.unwrap_or_default())
}

fn find_virtual_user_for_group<'a>(
    peers: &'a [RemotePeer],
    group_id: &str,
) -> Option<&'a RemotePeer> {
    peers.iter().find(|p| {
        p.channel == 3
            && p.group_id
                .as_deref()
                .map(|gid| gid == group_id)
                .unwrap_or(false)
    })
}

fn sync_envs_to_local(
    rules_storage: &RulesStorage,
    envs: &[RemoteEnv],
    group_name: &str,
) -> Result<(), String> {
    let existing_names: std::collections::HashSet<String> = rules_storage
        .list()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut remote_names = std::collections::HashSet::new();
    for env in envs {
        let rule_name = env.name.clone();
        remote_names.insert(rule_name.clone());

        let existing_enabled = rules_storage
            .load(&rule_name)
            .ok()
            .map(|r| r.enabled)
            .unwrap_or(false);

        let mut rule_file = bifrost_storage::RuleFile::new(&rule_name, &env.rule);
        rule_file.enabled = existing_enabled;
        rule_file.group = Some(group_name.to_string());
        rule_file.created_at = env.create_time.clone();
        rule_file.updated_at = env.update_time.clone();
        rule_file.mark_synced(&env.id, &env.user_id, &env.create_time, &env.update_time);

        rules_storage
            .save(&rule_file)
            .map_err(|e| format!("Failed to save rule: {e}"))?;
    }

    for name in &existing_names {
        if !remote_names.contains(name) {
            let _ = rules_storage.delete(name);
        }
    }

    Ok(())
}

fn build_rule_info_from_storage(rules_storage: &RulesStorage) -> Vec<GroupRuleInfo> {
    let names = rules_storage.list().unwrap_or_default();
    let mut rules = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if let Ok(rule) = rules_storage.load(name) {
            let rule_count = rule
                .content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && !t.starts_with('#')
                })
                .count();
            rules.push(GroupRuleInfo {
                name: rule.name.clone(),
                enabled: rule.enabled,
                sort_order: i as i32,
                rule_count,
                created_at: rule.created_at,
                updated_at: rule.updated_at,
                remote_env_id: rule.sync.remote_id,
                remote_user_id: rule.sync.remote_user_id,
            });
        }
    }
    rules
}

pub async fn handle_group_rules(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let Some(sync_manager) = state.sync_manager.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Sync manager not available",
        );
    };

    let sub_path = path
        .strip_prefix("/api/group-rules")
        .unwrap_or("")
        .trim_start_matches('/');

    let parts: Vec<&str> = sub_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "group_id is required");
    }

    let group_id = parts[0];
    let rule_name_segment = if parts.len() > 1 {
        Some(parts[1..].join("/"))
    } else {
        None
    };

    let method = req.method().clone();

    if let Some(ref seg) = rule_name_segment {
        let decoded = urlencoding::decode(seg)
            .map(|v| v.into_owned())
            .unwrap_or_else(|_| seg.clone());

        if let Some(rule_name) = decoded.strip_suffix("/enable") {
            return match method {
                Method::PUT => handle_enable_rule(state, group_id, rule_name, true).await,
                _ => method_not_allowed(),
            };
        }
        if let Some(rule_name) = decoded.strip_suffix("/disable") {
            return match method {
                Method::PUT => handle_enable_rule(state, group_id, rule_name, false).await,
                _ => method_not_allowed(),
            };
        }
    }

    match method {
        Method::GET if rule_name_segment.is_none() => {
            handle_list_and_sync(sync_manager, state, group_id).await
        }
        Method::GET if rule_name_segment.is_some() => {
            let rule_name = urlencoding::decode(rule_name_segment.as_ref().unwrap())
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| rule_name_segment.unwrap());
            handle_get_rule(state, group_id, &rule_name).await
        }
        Method::POST => handle_create_rule(req, sync_manager, state, group_id).await,
        Method::PUT if rule_name_segment.is_some() => {
            let rule_name = urlencoding::decode(rule_name_segment.as_ref().unwrap())
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| rule_name_segment.unwrap());
            handle_update_rule(req, sync_manager, state, group_id, &rule_name).await
        }
        Method::DELETE if rule_name_segment.is_some() => {
            let rule_name = urlencoding::decode(rule_name_segment.as_ref().unwrap())
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| rule_name_segment.unwrap());
            handle_delete_rule(sync_manager, state, group_id, &rule_name).await
        }
        _ => method_not_allowed(),
    }
}

fn get_group_rules_storage(
    state: &SharedAdminState,
    group_name: &str,
) -> Result<RulesStorage, String> {
    let base = state.rules_storage.base_dir();
    let dir = base.join(sanitize_group_dir_name(group_name));
    RulesStorage::with_dir(dir).map_err(|e| format!("Failed to create group rules dir: {e}"))
}

async fn resolve_group_name(
    sync_manager: &bifrost_sync::SyncManager,
    state: &SharedAdminState,
    group_id: &str,
) -> Result<String, String> {
    {
        let cache = state.group_name_cache();
        if let Some(name) = cache.get(group_id) {
            return Ok(name);
        }
    }
    let group = fetch_group_info(sync_manager, group_id).await?;
    let name = group.name.clone();
    {
        let mut cache = state.group_name_cache();
        cache.insert(group_id.to_string(), name.clone());
    }
    Ok(name)
}

async fn handle_list_and_sync(
    sync_manager: std::sync::Arc<bifrost_sync::SyncManager>,
    state: SharedAdminState,
    group_id: &str,
) -> Response<BoxBody> {
    let group_name = match resolve_group_name(&sync_manager, &state, group_id).await {
        Ok(n) => n,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
    };

    let peers = match fetch_peers(&sync_manager).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch peers");
            return error_response(StatusCode::BAD_GATEWAY, &e);
        }
    };

    let virtual_user = find_virtual_user_for_group(&peers, group_id);

    let writable = virtual_user
        .map(|p| p.editable.unwrap_or(false))
        .unwrap_or(false);

    let virtual_user_id = match virtual_user {
        Some(p) => p.user_id.clone(),
        None => {
            tracing::warn!(
                group_id = %group_id,
                group_name = %group_name,
                "No virtual user found for group, returning empty rules"
            );
            let group_storage = match get_group_rules_storage(&state, &group_name) {
                Ok(s) => s,
                Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
            };
            let rules = build_rule_info_from_storage(&group_storage);
            return json_response(&GroupRulesResponse {
                group_id: group_id.to_string(),
                group_name,
                writable: false,
                rules,
            });
        }
    };

    tracing::debug!(
        group_id = %group_id,
        group_name = %group_name,
        virtual_user_id = %virtual_user_id,
        writable = %writable,
        "Fetching group envs via virtual user"
    );

    let envs = match fetch_user_envs(&sync_manager, &virtual_user_id).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch envs for virtual user");
            return error_response(StatusCode::BAD_GATEWAY, &e);
        }
    };

    let group_storage = match get_group_rules_storage(&state, &group_name) {
        Ok(s) => s,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };

    if let Err(e) = sync_envs_to_local(&group_storage, &envs, &group_name) {
        tracing::warn!(error = %e, "Failed to sync envs to local storage");
    }

    let rules = build_rule_info_from_storage(&group_storage);

    json_response(&GroupRulesResponse {
        group_id: group_id.to_string(),
        group_name,
        writable,
        rules,
    })
}

async fn handle_get_rule(
    state: SharedAdminState,
    group_id: &str,
    rule_name: &str,
) -> Response<BoxBody> {
    let group_name = {
        let cache = state.group_name_cache();
        match cache.get(group_id) {
            Some(n) => n.clone(),
            None => {
                return error_response(StatusCode::BAD_REQUEST, "Group not loaded yet, list first")
            }
        }
    };

    let group_storage = match get_group_rules_storage(&state, &group_name) {
        Ok(s) => s,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };

    match group_storage.load(rule_name) {
        Ok(rule) => json_response(&GroupRuleDetail {
            name: rule.name,
            content: rule.content,
            enabled: rule.enabled,
            sort_order: rule.sort_order,
            created_at: rule.created_at,
            updated_at: rule.updated_at,
            sync: GroupRuleSyncInfo {
                status: "synced".to_string(),
                remote_id: rule.sync.remote_id,
                remote_updated_at: rule.sync.remote_updated_at,
            },
        }),
        Err(_) => error_response(StatusCode::NOT_FOUND, "Rule not found"),
    }
}

async fn handle_create_rule(
    req: Request<Incoming>,
    sync_manager: std::sync::Arc<bifrost_sync::SyncManager>,
    state: SharedAdminState,
    group_id: &str,
) -> Response<BoxBody> {
    let group_name = match resolve_group_name(&sync_manager, &state, group_id).await {
        Ok(n) => n,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
    };

    let peers = match fetch_peers(&sync_manager).await {
        Ok(p) => p,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
    };

    let virtual_user = match find_virtual_user_for_group(&peers, group_id) {
        Some(p) => p.clone(),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "No virtual user found for this group",
            )
        }
    };

    if !virtual_user.editable.unwrap_or(false) {
        return error_response(StatusCode::FORBIDDEN, "No write permission for this group");
    }

    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Read body: {e}")),
    };

    let create_req: CreateGroupRuleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {e}")),
    };

    let remote_body = serde_json::json!({
        "user_id": virtual_user.user_id,
        "name": create_req.name,
        "rule": create_req.content.unwrap_or_default(),
    });

    let env_resp: RemoteResponse<RemoteEnv> =
        match proxy_post_json(&sync_manager, "/v4/env", &remote_body).await {
            Ok(r) => r,
            Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
        };
    let env = env_resp.data;

    let group_storage = match get_group_rules_storage(&state, &group_name) {
        Ok(s) => s,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };

    let rule_name = env.name.clone();
    let mut rule_file = bifrost_storage::RuleFile::new(&rule_name, &env.rule);
    rule_file.enabled = false;
    rule_file.group = Some(group_name.clone());
    rule_file.created_at = env.create_time.clone();
    rule_file.updated_at = env.update_time.clone();
    rule_file.mark_synced(&env.id, &env.user_id, &env.create_time, &env.update_time);

    if let Err(e) = group_storage.save(&rule_file) {
        tracing::warn!(error = %e, "Failed to save created rule locally");
    }

    json_response(&GroupRuleDetail {
        name: rule_file.name,
        content: rule_file.content,
        enabled: rule_file.enabled,
        sort_order: 0,
        created_at: rule_file.created_at,
        updated_at: rule_file.updated_at,
        sync: GroupRuleSyncInfo {
            status: "synced".to_string(),
            remote_id: Some(env.id),
            remote_updated_at: Some(env.update_time),
        },
    })
}

async fn handle_update_rule(
    req: Request<Incoming>,
    sync_manager: std::sync::Arc<bifrost_sync::SyncManager>,
    state: SharedAdminState,
    group_id: &str,
    rule_name: &str,
) -> Response<BoxBody> {
    let group_name = match resolve_group_name(&sync_manager, &state, group_id).await {
        Ok(n) => n,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
    };

    let group_storage = match get_group_rules_storage(&state, &group_name) {
        Ok(s) => s,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };

    let existing = match group_storage.load(rule_name) {
        Ok(r) => r,
        Err(_) => return error_response(StatusCode::NOT_FOUND, "Rule not found locally"),
    };

    let remote_id = match &existing.sync.remote_id {
        Some(id) => id.clone(),
        None => return error_response(StatusCode::BAD_REQUEST, "Rule has no remote_id"),
    };

    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Read body: {e}")),
    };

    let update_req: UpdateGroupRuleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {e}")),
    };

    let remote_body = serde_json::json!({
        "id": remote_id,
        "rule": update_req.content,
    });

    let env_resp: RemoteResponse<RemoteEnv> = match proxy_patch_json(
        &sync_manager,
        &format!("/v4/env/{remote_id}"),
        &remote_body,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
    };
    let env = env_resp.data;

    let mut rule_file = bifrost_storage::RuleFile::new(rule_name, &env.rule);
    rule_file.enabled = existing.enabled;
    rule_file.group = Some(group_name.clone());
    rule_file.created_at = env.create_time.clone();
    rule_file.updated_at = env.update_time.clone();
    rule_file.mark_synced(&env.id, &env.user_id, &env.create_time, &env.update_time);

    if let Err(e) = group_storage.save(&rule_file) {
        tracing::warn!(error = %e, "Failed to save updated rule locally");
    }

    if existing.enabled {
        notify_rules_changed(&state);
    }

    json_response(&GroupRuleDetail {
        name: rule_file.name,
        content: rule_file.content,
        enabled: rule_file.enabled,
        sort_order: rule_file.sort_order,
        created_at: rule_file.created_at,
        updated_at: rule_file.updated_at,
        sync: GroupRuleSyncInfo {
            status: "synced".to_string(),
            remote_id: Some(env.id),
            remote_updated_at: Some(env.update_time),
        },
    })
}

async fn handle_delete_rule(
    sync_manager: std::sync::Arc<bifrost_sync::SyncManager>,
    state: SharedAdminState,
    group_id: &str,
    rule_name: &str,
) -> Response<BoxBody> {
    let group_name = match resolve_group_name(&sync_manager, &state, group_id).await {
        Ok(n) => n,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, &e),
    };

    let group_storage = match get_group_rules_storage(&state, &group_name) {
        Ok(s) => s,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };

    let existing = match group_storage.load(rule_name) {
        Ok(r) => r,
        Err(_) => return error_response(StatusCode::NOT_FOUND, "Rule not found locally"),
    };

    if let Some(remote_id) = &existing.sync.remote_id {
        if let Err(e) = proxy_delete(&sync_manager, &format!("/v4/env/{remote_id}")).await {
            return error_response(StatusCode::BAD_GATEWAY, &e);
        }
    }

    if let Err(e) = group_storage.delete(rule_name) {
        tracing::warn!(error = %e, "Failed to delete rule locally");
    }

    success_response("Rule deleted")
}

async fn handle_enable_rule(
    state: SharedAdminState,
    group_id: &str,
    rule_name: &str,
    enabled: bool,
) -> Response<BoxBody> {
    let group_name = {
        let cache = state.group_name_cache();
        match cache.get(group_id) {
            Some(n) => n.clone(),
            None => {
                return error_response(StatusCode::BAD_REQUEST, "Group not loaded yet, list first")
            }
        }
    };

    let group_storage = match get_group_rules_storage(&state, &group_name) {
        Ok(s) => s,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };

    match group_storage.set_enabled(rule_name, enabled) {
        Ok(_) => {
            notify_rules_changed(&state);
            let action = if enabled { "enabled" } else { "disabled" };
            tracing::info!(
                target: "bifrost_admin::group_rules",
                group_id = %group_id,
                group_name = %group_name,
                rule_name = %rule_name,
                enabled = %enabled,
                "group rule {action}"
            );
            success_response(&format!("Rule '{}' {} successfully", rule_name, action))
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update rule: {}", e),
        ),
    }
}

fn notify_rules_changed(state: &SharedAdminState) {
    if let Some(ref config_manager) = state.config_manager {
        match config_manager.notify(ConfigChangeEvent::RulesChanged) {
            Ok(count) => {
                tracing::info!(
                    target: "bifrost_admin::group_rules",
                    receivers = count,
                    "notified rules changed event"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "bifrost_admin::group_rules",
                    error = %e,
                    "failed to notify rules changed event (no receivers)"
                );
            }
        }
    }
}
