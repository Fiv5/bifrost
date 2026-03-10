use std::collections::HashMap;
use std::sync::Arc;

use bifrost_admin::{AdminState, FrameDirection, FrameType};
use bifrost_script::{MatchedRuleInfo, RequestData, ResponseData, ScriptContext, ScriptType};
use bytes::Bytes;
use tracing::warn;

use crate::protocol::parse_permessage_deflate_config;
use crate::server::ResolvedRules;
use crate::utils::logging::RequestContext;

#[derive(Debug, Clone, Default)]
pub struct WsHandshakeMeta {
    pub negotiated_protocol: Option<String>,
    pub negotiated_extensions: Option<String>,
}

fn build_matched_rules_info(resolved_rules: &ResolvedRules) -> Vec<MatchedRuleInfo> {
    resolved_rules
        .rules
        .iter()
        .map(|r| MatchedRuleInfo {
            pattern: r.pattern.clone(),
            protocol: r.protocol.to_string(),
            value: r.value.clone(),
        })
        .collect()
}

fn parse_url_parts(url: &str) -> (String, String, String) {
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("").to_string();
        let path = parsed.path().to_string();
        let protocol = parsed.scheme().to_string();
        (host, path, protocol)
    } else {
        ("".to_string(), url.to_string(), "http".to_string())
    }
}

async fn get_values_from_state(admin_state: &Option<Arc<AdminState>>) -> HashMap<String, String> {
    use bifrost_core::ValueStore;
    if let Some(state) = admin_state {
        if let Some(values_storage) = &state.values_storage {
            let storage = values_storage.read();
            return storage.as_hashmap();
        }
    }
    HashMap::new()
}

fn is_builtin_decoder(name: &str) -> bool {
    matches!(name, "utf8" | "default")
}

fn builtin_decode_utf8(input: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(input).to_string().into_bytes()
}

#[allow(clippy::too_many_arguments)]
pub async fn decode_ws_payload_for_storage(
    admin_state: &Option<Arc<AdminState>>,
    script_names: &[String],
    ctx: &RequestContext,
    resolved_rules: &ResolvedRules,
    request_url: &str,
    request_method: &str,
    request_headers: &[(String, String)],
    ws_meta: &WsHandshakeMeta,
    direction: FrameDirection,
    frame_type: FrameType,
    payload_bytes: &[u8],
) -> Option<Vec<u8>> {
    if script_names.is_empty() || payload_bytes.is_empty() {
        return None;
    }

    let state = admin_state.as_ref()?;
    let manager = state.script_manager.as_ref()?;
    let cfg = if let Some(cm) = state.config_manager.as_ref() {
        Some(cm.config().await)
    } else {
        None
    };

    const MAX_DECODE_INPUT_BYTES: usize = 2 * 1024 * 1024;
    if payload_bytes.len() > MAX_DECODE_INPUT_BYTES {
        warn!(
            "[{}] [DECODE][WS] skip decode ({} bytes > {} limit)",
            ctx.id_str(),
            payload_bytes.len(),
            MAX_DECODE_INPUT_BYTES
        );
        return None;
    }

    let mut values = resolved_rules.values.clone();
    for (k, v) in get_values_from_state(admin_state).await {
        values.entry(k).or_insert(v);
    }
    values.insert(
        "ws_direction".to_string(),
        format!("{:?}", direction).to_lowercase(),
    );
    values.insert(
        "ws_frame_type".to_string(),
        format!("{:?}", frame_type).to_lowercase(),
    );
    values.insert(
        "ws_payload_size".to_string(),
        payload_bytes.len().to_string(),
    );

    if let Some(ref proto) = ws_meta.negotiated_protocol {
        values.insert("ws_subprotocol".to_string(), proto.clone());
    }
    if let Some(ref ext) = ws_meta.negotiated_extensions {
        values.insert("ws_extensions".to_string(), ext.clone());
        if let Some(cfg) = parse_permessage_deflate_config(ext) {
            values.insert(
                "ws_permessage_deflate".to_string(),
                cfg.enabled().to_string(),
            );
            values.insert(
                "ws_client_no_context_takeover".to_string(),
                cfg.client_no_context_takeover.to_string(),
            );
            values.insert(
                "ws_server_no_context_takeover".to_string(),
                cfg.server_no_context_takeover.to_string(),
            );
            if let Some(bits) = cfg.client_max_window_bits {
                values.insert("ws_client_max_window_bits".to_string(), bits.to_string());
            }
            if let Some(bits) = cfg.server_max_window_bits {
                values.insert("ws_server_max_window_bits".to_string(), bits.to_string());
            }
        } else {
            values.insert("ws_permessage_deflate".to_string(), "false".to_string());
        }
    }

    if let Ok(parsed) = url::Url::parse(request_url) {
        if let Some(h) = parsed.host_str() {
            values.insert("ws_target_host".to_string(), h.to_string());
        }
        if let Some(p) = parsed.port_or_known_default() {
            values.insert("ws_target_port".to_string(), p.to_string());
        }
        let tls = matches!(parsed.scheme(), "wss" | "https");
        values.insert("ws_is_tls".to_string(), tls.to_string());
    }

    let matched_rules = build_matched_rules_info(resolved_rules);
    let (host, path, protocol) = parse_url_parts(request_url);

    let request_data = RequestData {
        url: request_url.to_string(),
        method: request_method.to_string(),
        host,
        path,
        protocol,
        client_ip: ctx.client_ip.clone(),
        client_app: ctx.client_app.clone(),
        headers: request_headers.iter().cloned().collect(),
        body: None,
    };

    let mut response_headers = HashMap::new();
    if let Some(ref proto) = ws_meta.negotiated_protocol {
        response_headers.insert("Sec-WebSocket-Protocol".to_string(), proto.clone());
    }
    if let Some(ref ext) = ws_meta.negotiated_extensions {
        response_headers.insert("Sec-WebSocket-Extensions".to_string(), ext.clone());
    }

    // 附加基础元信息，方便脚本侧判断握手类型。
    response_headers.insert("Upgrade".to_string(), "websocket".to_string());
    response_headers.insert("Connection".to_string(), "Upgrade".to_string());

    let response_data = ResponseData {
        status: 101,
        status_text: "Switching Protocols".to_string(),
        headers: response_headers,
        body: None,
        request: request_data.clone(),
    };

    let phase = match direction {
        FrameDirection::Send => "websocket_send",
        FrameDirection::Receive => "websocket_recv",
    };

    let mut current = payload_bytes.to_vec();
    let mgr = manager.read().await;

    let mut applied = false;
    for script_name in script_names {
        let script_name = script_name.trim();
        if script_name.is_empty() || is_builtin_decoder(script_name) {
            current = builtin_decode_utf8(&current);
            applied = true;
            continue;
        }

        let script_ctx = ScriptContext {
            request_id: ctx.id_str().to_string(),
            script_name: script_name.to_string(),
            script_type: ScriptType::Decode,
            values: values.clone(),
            matched_rules: matched_rules.clone(),
        };

        let exec = if let Some(ref cfg) = cfg {
            mgr.engine()
                .execute_decode_script_with_config(
                    script_name,
                    phase,
                    &request_data,
                    if matches!(direction, FrameDirection::Send) {
                        &current
                    } else {
                        &[]
                    },
                    &response_data,
                    if matches!(direction, FrameDirection::Receive) {
                        &current
                    } else {
                        &[]
                    },
                    &script_ctx,
                    cfg,
                )
                .await
        } else {
            mgr.engine()
                .execute_decode_script(
                    script_name,
                    phase,
                    &request_data,
                    if matches!(direction, FrameDirection::Send) {
                        &current
                    } else {
                        &[]
                    },
                    &response_data,
                    if matches!(direction, FrameDirection::Receive) {
                        &current
                    } else {
                        &[]
                    },
                    &script_ctx,
                )
                .await
        };

        if let Ok((out, _logs)) = exec {
            if out.code == "0" {
                current = Bytes::from(out.data).to_vec();
                applied = true;
            } else {
                current = Bytes::from(out.msg).to_vec();
                applied = true;
                break;
            }
        }
    }

    if applied {
        Some(current)
    } else {
        None
    }
}
