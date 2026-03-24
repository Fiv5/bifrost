use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::traffic::SocketStatus;

pub const MAX_OPENAI_LIKE_SSE_ASSEMBLY_INPUT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub seq: u64,
    pub ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<u64>,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default)]
    pub parse_error: bool,
}

#[derive(Debug, Clone)]
struct SseConnectionState {
    is_open: bool,
    receive_bytes: u64,
    receive_count: u64,
    // 当用户在管理端主动查看 SSE messages 时，proxy 侧需要更激进地把 sse_raw 写盘，
    // 否则可能出现：count 在涨（基于解析/计数），但文件未 flush 导致详情流读不到数据。
    force_flush_until_ms: u64,
}

impl SseConnectionState {
    fn new() -> Self {
        Self {
            is_open: true,
            receive_bytes: 0,
            receive_count: 0,
            force_flush_until_ms: 0,
        }
    }
}

#[derive(Debug)]
pub struct SseHub {
    connections: RwLock<HashMap<String, SseConnectionState>>,
}

impl SseHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn register(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        connections
            .entry(connection_id.to_string())
            .or_insert_with(SseConnectionState::new);
    }

    pub fn set_closed(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        if let Some(state) = connections.get_mut(connection_id) {
            state.is_open = false;
        }
    }

    pub fn unregister(&self, connection_id: &str) {
        self.connections.write().remove(connection_id);
    }

    pub fn add_receive_bytes(&self, connection_id: &str, bytes: usize) {
        let mut connections = self.connections.write();
        if let Some(state) = connections.get_mut(connection_id) {
            state.receive_bytes = state.receive_bytes.saturating_add(bytes as u64);
        }
    }

    pub fn add_receive_event(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        if let Some(state) = connections.get_mut(connection_id) {
            state.receive_count = state.receive_count.saturating_add(1);
        }
    }

    pub fn request_force_flush(&self, connection_id: &str, duration_ms: u64) {
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let until = now.saturating_add(duration_ms);
        let mut connections = self.connections.write();
        let state = connections
            .entry(connection_id.to_string())
            .or_insert_with(SseConnectionState::new);
        state.force_flush_until_ms = state.force_flush_until_ms.max(until);
    }

    pub fn should_force_flush(&self, connection_id: &str) -> bool {
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.force_flush_until_ms > now)
            .unwrap_or(false)
    }

    pub fn is_open(&self, connection_id: &str) -> Option<bool> {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.is_open)
    }

    pub fn get_socket_status(&self, connection_id: &str) -> Option<SocketStatus> {
        let connections = self.connections.read();
        let state = connections.get(connection_id)?;
        Some(SocketStatus {
            is_open: state.is_open,
            send_count: 0,
            receive_count: state.receive_count,
            send_bytes: 0,
            receive_bytes: state.receive_bytes,
            frame_count: state.receive_count as usize,
            close_code: None,
            close_reason: None,
        })
    }

    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }

    pub fn open_connection_count(&self) -> usize {
        self.connections
            .read()
            .values()
            .filter(|s| s.is_open)
            .count()
    }
}

impl Default for SseHub {
    fn default() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }
}

pub fn parse_sse_event(raw: &str) -> SseEvent {
    parse_sse_event_with_error(raw, false)
}

fn parse_sse_event_with_error(raw: &str, parse_error: bool) -> SseEvent {
    let mut id: Option<String> = None;
    let mut event: Option<String> = None;
    let mut retry: Option<u64> = None;
    let mut data_lines: Vec<String> = Vec::new();

    for raw_line in raw.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            let v = rest.trim_start();
            if !v.is_empty() {
                event = Some(v.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("id:") {
            let v = rest.trim_start();
            if !v.is_empty() {
                id = Some(v.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("retry:") {
            let v = rest.trim_start();
            if let Ok(n) = v.parse::<u64>() {
                retry = Some(n);
            }
            continue;
        }
    }

    let data = if !data_lines.is_empty() {
        data_lines.join("\n")
    } else {
        raw.to_string()
    };

    let event = if event.is_none() && data.trim() == "[DONE]" {
        Some("finish".to_string())
    } else {
        event
    };

    SseEvent {
        seq: 0,
        ts: 0,
        id,
        event,
        retry,
        data,
        raw: if parse_error {
            Some(raw.to_string())
        } else {
            None
        },
        parse_error,
    }
}

pub fn parse_sse_events_from_text(input: &str) -> (Vec<SseEvent>, String) {
    let mut events: Vec<SseEvent> = Vec::new();
    let mut buffer = String::new();
    let mut prev_nl = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' && matches!(chars.peek(), Some('\n')) {
            continue;
        }
        if ch == '\n' {
            if prev_nl {
                let chunk = buffer.trim_end_matches('\n').to_string();
                if !chunk.is_empty() {
                    events.push(parse_sse_event(&chunk));
                }
                buffer.clear();
                prev_nl = false;
                continue;
            }
            buffer.push('\n');
            prev_nl = true;
            continue;
        }
        prev_nl = false;
        buffer.push(ch);
    }
    (events, buffer)
}

#[derive(Debug, Default, Clone)]
struct ChatFunctionCallState {
    name: Option<String>,
    arguments: String,
    extra: Map<String, Value>,
}

#[derive(Debug, Default, Clone)]
struct ChatToolCallState {
    index: usize,
    id: Option<String>,
    call_type: Option<String>,
    extra: Map<String, Value>,
    function: Option<ChatFunctionCallState>,
}

#[derive(Debug, Default, Clone)]
struct ChatChoiceState {
    index: usize,
    role: Option<String>,
    content: String,
    reasoning_content: String,
    refusal: String,
    finish_reason: Option<Value>,
    extra: Map<String, Value>,
    message_extra: Map<String, Value>,
    function_call: Option<ChatFunctionCallState>,
    tool_calls: Vec<ChatToolCallState>,
}

fn merge_json_object(
    target: &mut Map<String, Value>,
    source: &Map<String, Value>,
    excluded_keys: &[&str],
) {
    for (key, value) in source {
        if excluded_keys.iter().any(|excluded| excluded == key) {
            continue;
        }

        match (target.get_mut(key), value) {
            (Some(Value::Object(existing)), Value::Object(incoming)) => {
                merge_json_object(existing, incoming, &[]);
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn strip_duplicate_keys(
    record: &Map<String, Value>,
    parent: &Map<String, Value>,
) -> Map<String, Value> {
    let mut next = Map::new();
    for (key, value) in record {
        if parent
            .get(key)
            .is_some_and(|parent_value| parent_value == value)
        {
            continue;
        }
        next.insert(key.clone(), value.clone());
    }
    next
}

fn append_value_string(buffer: &mut String, value: Option<&Value>) {
    if let Some(Value::String(text)) = value {
        buffer.push_str(text);
    }
}

fn extract_delta_text_parts(delta: &Map<String, Value>) -> (String, String, String) {
    let mut content = String::new();
    let mut reasoning_content = String::new();
    let mut refusal = String::new();

    append_value_string(&mut content, delta.get("content"));
    append_value_string(&mut reasoning_content, delta.get("reasoning_content"));
    append_value_string(&mut refusal, delta.get("refusal"));

    if let Some(Value::Array(parts)) = delta.get("content") {
        for part in parts {
            match part {
                Value::String(text) => content.push_str(text),
                Value::Object(part_obj) => {
                    let part_type = part_obj
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let text = part_obj
                        .get("text")
                        .and_then(Value::as_str)
                        .or_else(|| part_obj.get("content").and_then(Value::as_str))
                        .unwrap_or_default();
                    if part_type.contains("reasoning") {
                        reasoning_content.push_str(text);
                    } else if part_type.contains("refusal") {
                        refusal.push_str(text);
                    } else {
                        content.push_str(text);
                    }
                }
                _ => {}
            }
        }
    }

    (content, reasoning_content, refusal)
}

fn ensure_choice_state(choices: &mut Vec<ChatChoiceState>, index: usize) -> &mut ChatChoiceState {
    if let Some(position) = choices.iter().position(|choice| choice.index == index) {
        return &mut choices[position];
    }
    choices.push(ChatChoiceState {
        index,
        ..Default::default()
    });
    choices.last_mut().expect("just pushed")
}

fn ensure_tool_call_state(
    tool_calls: &mut Vec<ChatToolCallState>,
    index: usize,
) -> &mut ChatToolCallState {
    if let Some(position) = tool_calls.iter().position(|call| call.index == index) {
        return &mut tool_calls[position];
    }
    tool_calls.push(ChatToolCallState {
        index,
        ..Default::default()
    });
    tool_calls.last_mut().expect("just pushed")
}

fn merge_function_call_state(state: &mut ChatFunctionCallState, payload: &Map<String, Value>) {
    merge_json_object(&mut state.extra, payload, &["name", "arguments"]);
    if let Some(name) = payload.get("name").and_then(Value::as_str) {
        state.name = Some(name.to_string());
    }
    if let Some(arguments) = payload.get("arguments").and_then(Value::as_str) {
        state.arguments.push_str(arguments);
    }
}

fn assemble_openai_like_chat_completion(events: &[SseEvent]) -> Option<String> {
    let mut top_level = Map::new();
    let mut choices: Vec<ChatChoiceState> = Vec::new();
    let mut recognized = false;

    for event in events {
        let data = event.data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(data) else {
            if recognized {
                return None;
            }
            continue;
        };
        let Some(Value::Array(payload_choices)) = payload.get("choices") else {
            if recognized {
                return None;
            }
            continue;
        };

        recognized = true;
        merge_json_object(&mut top_level, &payload, &["choices"]);

        for raw_choice in payload_choices {
            let Value::Object(choice_obj) = raw_choice else {
                continue;
            };
            let index = choice_obj
                .get("index")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .unwrap_or(0);
            let choice = ensure_choice_state(&mut choices, index);
            merge_json_object(
                &mut choice.extra,
                choice_obj,
                &["index", "delta", "message"],
            );

            if let Some(finish_reason) = choice_obj.get("finish_reason") {
                choice.finish_reason = Some(finish_reason.clone());
            }

            if let Some(Value::Object(delta)) = choice_obj.get("delta") {
                merge_json_object(
                    &mut choice.message_extra,
                    delta,
                    &[
                        "role",
                        "content",
                        "reasoning_content",
                        "refusal",
                        "function_call",
                        "tool_calls",
                    ],
                );
                if let Some(role) = delta.get("role").and_then(Value::as_str) {
                    choice.role = Some(role.to_string());
                }
                let (content, reasoning_content, refusal) = extract_delta_text_parts(delta);
                choice.content.push_str(&content);
                choice.reasoning_content.push_str(&reasoning_content);
                choice.refusal.push_str(&refusal);

                if let Some(Value::Object(function_call)) = delta.get("function_call") {
                    let state = choice
                        .function_call
                        .get_or_insert_with(ChatFunctionCallState::default);
                    merge_function_call_state(state, function_call);
                }

                if let Some(Value::Array(tool_calls)) = delta.get("tool_calls") {
                    for (position, raw_tool_call) in tool_calls.iter().enumerate() {
                        let Value::Object(tool_call_obj) = raw_tool_call else {
                            continue;
                        };
                        let tool_index = tool_call_obj
                            .get("index")
                            .and_then(Value::as_u64)
                            .map(|value| value as usize)
                            .unwrap_or(position);
                        let tool_call = ensure_tool_call_state(&mut choice.tool_calls, tool_index);
                        merge_json_object(
                            &mut tool_call.extra,
                            tool_call_obj,
                            &["index", "id", "type", "function"],
                        );
                        if let Some(id) = tool_call_obj.get("id").and_then(Value::as_str) {
                            tool_call.id = Some(id.to_string());
                        }
                        if let Some(call_type) = tool_call_obj.get("type").and_then(Value::as_str) {
                            tool_call.call_type = Some(call_type.to_string());
                        }
                        if let Some(Value::Object(function)) = tool_call_obj.get("function") {
                            let state = tool_call
                                .function
                                .get_or_insert_with(ChatFunctionCallState::default);
                            merge_function_call_state(state, function);
                        }
                    }
                }
            }

            if let Some(Value::Object(message)) = choice_obj.get("message") {
                merge_json_object(
                    &mut choice.message_extra,
                    message,
                    &[
                        "role",
                        "content",
                        "reasoning_content",
                        "refusal",
                        "function_call",
                        "tool_calls",
                    ],
                );
                if let Some(role) = message.get("role").and_then(Value::as_str) {
                    choice.role = Some(role.to_string());
                }
                append_value_string(&mut choice.content, message.get("content"));
                append_value_string(
                    &mut choice.reasoning_content,
                    message.get("reasoning_content"),
                );
                append_value_string(&mut choice.refusal, message.get("refusal"));
            }
        }
    }

    if !recognized || choices.is_empty() {
        return None;
    }

    let assembled_choices = choices
        .into_iter()
        .map(|choice| {
            let mut message = choice.message_extra;
            message.insert(
                "role".to_string(),
                Value::String(choice.role.unwrap_or_else(|| "assistant".to_string())),
            );
            if !choice.content.is_empty() {
                message.insert("content".to_string(), Value::String(choice.content));
            }
            if !choice.reasoning_content.is_empty() {
                message.insert(
                    "reasoning_content".to_string(),
                    Value::String(choice.reasoning_content),
                );
            }
            if !choice.refusal.is_empty() {
                message.insert("refusal".to_string(), Value::String(choice.refusal));
            }
            if let Some(function_call) = choice.function_call {
                let mut function_value = function_call.extra;
                if let Some(name) = function_call.name {
                    function_value.insert("name".to_string(), Value::String(name));
                }
                function_value.insert(
                    "arguments".to_string(),
                    Value::String(function_call.arguments),
                );
                message.insert("function_call".to_string(), Value::Object(function_value));
            }
            if !choice.tool_calls.is_empty() {
                let mut tool_calls = choice.tool_calls;
                tool_calls.sort_by_key(|call| call.index);
                message.insert(
                    "tool_calls".to_string(),
                    Value::Array(
                        tool_calls
                            .into_iter()
                            .map(|tool_call| {
                                let mut value = tool_call.extra;
                                if let Some(id) = tool_call.id {
                                    value.insert("id".to_string(), Value::String(id));
                                }
                                if let Some(call_type) = tool_call.call_type {
                                    value.insert("type".to_string(), Value::String(call_type));
                                }
                                if let Some(function) = tool_call.function {
                                    let mut function_value = function.extra;
                                    if let Some(name) = function.name {
                                        function_value
                                            .insert("name".to_string(), Value::String(name));
                                    }
                                    function_value.insert(
                                        "arguments".to_string(),
                                        Value::String(function.arguments),
                                    );
                                    value.insert(
                                        "function".to_string(),
                                        Value::Object(function_value),
                                    );
                                }
                                Value::Object(value)
                            })
                            .collect(),
                    ),
                );
            }

            let choice_extra = strip_duplicate_keys(&choice.extra, &top_level);
            let mut assembled = choice_extra;
            assembled.insert("index".to_string(), Value::Number(choice.index.into()));
            assembled.insert("message".to_string(), Value::Object(message));
            assembled.insert(
                "finish_reason".to_string(),
                choice.finish_reason.unwrap_or(Value::Null),
            );
            Value::Object(assembled)
        })
        .collect();

    top_level.insert(
        "object".to_string(),
        Value::String("chat.completion".to_string()),
    );
    top_level.insert("choices".to_string(), Value::Array(assembled_choices));

    serde_json::to_string_pretty(&Value::Object(top_level)).ok()
}

pub fn assemble_openai_like_response_body_from_text(input: &str) -> Option<String> {
    if input.len() > MAX_OPENAI_LIKE_SSE_ASSEMBLY_INPUT_BYTES {
        return None;
    }
    let (events, _) = parse_sse_events_from_text(input);
    assemble_openai_like_chat_completion(&events)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        assemble_openai_like_response_body_from_text, parse_sse_event,
        MAX_OPENAI_LIKE_SSE_ASSEMBLY_INPUT_BYTES,
    };

    #[test]
    fn done_message_is_normalized_to_finish_event() {
        let event = parse_sse_event("data: [DONE]");
        assert_eq!(event.event.as_deref(), Some("finish"));
        assert_eq!(event.data, "[DONE]");
    }

    #[test]
    fn assemble_openai_like_chat_completion_keeps_reasoning_and_usage() {
        let raw = concat!(
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"model\":\"kimi-k2.5\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"think-1 \",\"content\":\"hello \"},\"finish_reason\":null}],\"usage\":{\"total_tokens\":10}}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"model\":\"kimi-k2.5\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"think-2 \",\"content\":\"world\",\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"a\\\":\"}}]},\"finish_reason\":null}],\"usage\":{\"total_tokens\":10}}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"model\":\"kimi-k2.5\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]},\"finish_reason\":\"tool_calls\",\"usage\":{\"total_tokens\":10}}]}\n\n",
            "data: [DONE]\n\n"
        );

        let assembled = assemble_openai_like_response_body_from_text(raw).expect("assembled");
        let value: Value = serde_json::from_str(&assembled).expect("json");
        assert_eq!(value["object"], "chat.completion");
        assert_eq!(value["usage"]["total_tokens"], 10);
        assert_eq!(value["choices"][0]["message"]["content"], "hello world");
        assert_eq!(
            value["choices"][0]["message"]["reasoning_content"],
            "think-1 think-2 "
        );
        assert_eq!(
            value["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
            "{\"a\":1}"
        );
        assert!(value["choices"][0].get("usage").is_none());
    }

    #[test]
    fn assemble_openai_like_chat_completion_returns_none_for_large_input() {
        let oversized = "x".repeat(MAX_OPENAI_LIKE_SSE_ASSEMBLY_INPUT_BYTES + 1);
        assert!(assemble_openai_like_response_body_from_text(&oversized).is_none());
    }

    #[test]
    fn assemble_openai_like_chat_completion_returns_none_for_invalid_chunks() {
        let raw = concat!(
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hel\"}}]}\n\n",
            "data: {not-json}\n\n",
            "data: [DONE]\n\n"
        );
        assert!(assemble_openai_like_response_body_from_text(raw).is_none());
    }
}
