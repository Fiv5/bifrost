use crate::proxy::ProxyInstance;
use crate::runner::TestCase;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, client_async};

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "ws_payload_persistence_binary",
            "WS 帧持久化 - Binary payload 通过 FileRange 读取并 Base64 返回",
            "ws_payload",
            test_ws_payload_persistence_binary,
        ),
        TestCase::standalone(
            "ws_payload_shared_file_per_connection",
            "同一个 WS 连接的多个 frame 共享同一个 payload 文件",
            "ws_payload",
            test_ws_payload_shared_file_per_connection,
        ),
        TestCase::standalone(
            "ws_payload_clear_closed_connection",
            "清理已关闭连接后 WS payload 文件被移除",
            "ws_payload",
            test_ws_payload_clear_closed_connection,
        ),
        TestCase::standalone(
            "ws_payload_clear_active_connection",
            "清理 traffic 时活跃 WS 连接不会联动清理 payload",
            "ws_payload",
            test_ws_payload_clear_active_connection,
        ),
    ]
}

async fn start_ws_echo_server() -> Result<(u16, tokio::task::JoinHandle<()>), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed to bind ws server: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get ws server addr: {}", e))?
        .port();

    let handle = tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            tokio::spawn(async move {
                if let Ok(mut ws) = accept_async(stream).await {
                    while let Some(msg) = ws.next().await {
                        if let Ok(msg) = msg {
                            if msg.is_close() {
                                let _ = ws.close(None).await;
                                break;
                            }
                            let _ = ws.send(msg).await;
                        } else {
                            break;
                        }
                    }
                }
            });
        }
    });

    Ok((port, handle))
}

async fn wait_for_websocket_record_id(admin_base: &str) -> Result<String, String> {
    const IS_WEBSOCKET_FLAG: u64 = 1 << 1;

    for _ in 0..20 {
        if let Ok(resp) = reqwest::get(format!("{}/traffic?limit=20", admin_base)).await {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(records) = json["records"].as_array() {
                    if let Some(record) = records.iter().find(|item| {
                        item["is_websocket"].as_bool().unwrap_or(false)
                            || item["flags"]
                                .as_u64()
                                .is_some_and(|flags| flags & IS_WEBSOCKET_FLAG != 0)
                            || matches!(
                                item["protocol"].as_str().or_else(|| item["proto"].as_str()),
                                Some("ws" | "wss")
                            )
                    }) {
                        if let Some(id) = record["id"].as_str() {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("No websocket traffic record found".to_string())
}

async fn wait_for_binary_frame(
    admin_base: &str,
    connection_id: &str,
) -> Result<(u64, Value), String> {
    for _ in 0..20 {
        if let Ok(resp) = reqwest::get(format!(
            "{}/traffic/{}/frames?limit=20",
            admin_base, connection_id
        ))
        .await
        {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(frames) = json["frames"].as_array() {
                    for frame in frames {
                        if frame["frame_type"] == "binary" {
                            let frame_id = frame["frame_id"].as_u64().ok_or("frame_id missing")?;
                            return Ok((frame_id, frame.clone()));
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("No binary frame found".to_string())
}

async fn wait_for_binary_frames(
    admin_base: &str,
    connection_id: &str,
    expected: usize,
) -> Result<Vec<Value>, String> {
    for _ in 0..30 {
        if let Ok(resp) = reqwest::get(format!(
            "{}/traffic/{}/frames?limit=50",
            admin_base, connection_id
        ))
        .await
        {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(frames) = json["frames"].as_array() {
                    let binary_frames: Vec<Value> = frames
                        .iter()
                        .filter(|frame| frame["frame_type"] == "binary")
                        .cloned()
                        .collect();
                    if binary_frames.len() >= expected {
                        return Ok(binary_frames);
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(format!("Expected at least {} binary frames", expected))
}

fn file_range_path(frame: &Value) -> Result<String, String> {
    frame["payload_ref"]["FileRange"]["path"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "payload_ref.FileRange.path missing".to_string())
}

fn temp_data_dir(port: u16) -> PathBuf {
    std::env::temp_dir().join(format!("bifrost_e2e_test_{}", port))
}

fn count_root_frame_cache_files(dir: &Path, connection_id: &str) -> Result<usize, String> {
    let prefix = format!("{}_frame_", connection_id);
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read temp data dir {}: {}", dir.display(), e))?;
    let count = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with(&prefix))
                .unwrap_or(false)
        })
        .count();
    Ok(count)
}

async fn test_ws_payload_persistence_binary() -> Result<(), String> {
    let (ws_port, server_handle) = start_ws_echo_server().await?;

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, vec![], false, false)
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let target_url = format!("ws://127.0.0.1:{}/echo", ws_port);
    let proxy_addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect(proxy_addr)
        .await
        .map_err(|e| format!("Failed to connect to proxy: {}", e))?;
    let request = target_url
        .into_client_request()
        .map_err(|e| format!("Failed to build ws request: {}", e))?;

    let (mut ws_stream, _) = client_async(request, stream)
        .await
        .map_err(|e| format!("Failed to open websocket: {}", e))?;

    let payload = vec![1u8, 2, 3, 4, 5, 6];
    ws_stream
        .send(Message::Binary(payload.clone().into()))
        .await
        .map_err(|e| format!("Failed to send ws payload: {}", e))?;

    if let Some(msg) = ws_stream.next().await {
        let msg = msg.map_err(|e| format!("Failed to receive ws message: {}", e))?;
        if msg.into_data() != payload {
            return Err("Echo payload mismatch".to_string());
        }
    }

    let admin_base = format!("http://127.0.0.1:{}/_bifrost/api", port);
    let connection_id = wait_for_websocket_record_id(&admin_base).await?;
    let (frame_id, frame) = wait_for_binary_frame(&admin_base, &connection_id).await?;

    if frame.get("payload_ref").is_none() {
        return Err("payload_ref missing in frame list".to_string());
    }
    let payload_ref = frame.get("payload_ref").unwrap();
    if payload_ref.get("FileRange").is_none() {
        return Err("payload_ref is not FileRange".to_string());
    }

    let detail_url = format!(
        "{}/traffic/{}/frames/{}",
        admin_base, connection_id, frame_id
    );
    let detail: Value = reqwest::get(detail_url)
        .await
        .map_err(|e| format!("Failed to get frame detail: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse frame detail: {}", e))?;

    let full_payload = detail["full_payload"]
        .as_str()
        .ok_or("full_payload missing")?;
    let expected = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, payload);
    if full_payload != expected {
        return Err(format!(
            "Expected base64 payload {}, got {}",
            expected, full_payload
        ));
    }

    let _ = ws_stream.close(None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    server_handle.abort();
    Ok(())
}

async fn test_ws_payload_shared_file_per_connection() -> Result<(), String> {
    let (ws_port, server_handle) = start_ws_echo_server().await?;

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, vec![], false, false)
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let target_url = format!("ws://127.0.0.1:{}/echo", ws_port);
    let proxy_addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect(proxy_addr)
        .await
        .map_err(|e| format!("Failed to connect to proxy: {}", e))?;
    let request = target_url
        .into_client_request()
        .map_err(|e| format!("Failed to build ws request: {}", e))?;

    let (mut ws_stream, _) = client_async(request, stream)
        .await
        .map_err(|e| format!("Failed to open websocket: {}", e))?;

    let first_payload = vec![1u8, 3, 5, 7];
    let second_payload = vec![2u8, 4, 6, 8, 10];
    for payload in [&first_payload, &second_payload] {
        ws_stream
            .send(Message::Binary(payload.clone().into()))
            .await
            .map_err(|e| format!("Failed to send ws payload: {}", e))?;
        if let Some(msg) = ws_stream.next().await {
            let msg = msg.map_err(|e| format!("Failed to receive ws message: {}", e))?;
            if msg.into_data() != *payload {
                return Err("Echo payload mismatch".to_string());
            }
        }
    }

    let admin_base = format!("http://127.0.0.1:{}/_bifrost/api", port);
    let connection_id = wait_for_websocket_record_id(&admin_base).await?;
    let frames = wait_for_binary_frames(&admin_base, &connection_id, 2).await?;
    let first_path = file_range_path(&frames[0])?;
    let second_path = file_range_path(&frames[1])?;
    if first_path != second_path {
        return Err(format!(
            "Expected both frames to share one payload file, got {} and {}",
            first_path, second_path
        ));
    }

    let payload_dir = temp_data_dir(port).join("ws_payload");
    let payload_file_count = std::fs::read_dir(&payload_dir)
        .map_err(|e| {
            format!(
                "Failed to read ws payload dir {}: {}",
                payload_dir.display(),
                e
            )
        })?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count();
    if payload_file_count != 1 {
        return Err(format!(
            "Expected exactly 1 ws payload file, found {}",
            payload_file_count
        ));
    }

    let frame_cache_count = count_root_frame_cache_files(&temp_data_dir(port), &connection_id)?;
    if frame_cache_count != 0 {
        return Err(format!(
            "Expected no per-frame body cache files, found {}",
            frame_cache_count
        ));
    }

    let _ = ws_stream.close(None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    server_handle.abort();
    Ok(())
}

async fn test_ws_payload_clear_closed_connection() -> Result<(), String> {
    let (ws_port, server_handle) = start_ws_echo_server().await?;

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, vec![], false, false)
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let target_url = format!("ws://127.0.0.1:{}/echo", ws_port);
    let proxy_addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect(proxy_addr)
        .await
        .map_err(|e| format!("Failed to connect to proxy: {}", e))?;
    let request = target_url
        .into_client_request()
        .map_err(|e| format!("Failed to build ws request: {}", e))?;

    let (mut ws_stream, _) = client_async(request, stream)
        .await
        .map_err(|e| format!("Failed to open websocket: {}", e))?;

    let payload = vec![2u8, 4, 6, 8];
    ws_stream
        .send(Message::Binary(payload.clone().into()))
        .await
        .map_err(|e| format!("Failed to send ws payload: {}", e))?;

    if let Some(msg) = ws_stream.next().await {
        let msg = msg.map_err(|e| format!("Failed to receive ws message: {}", e))?;
        if msg.into_data() != payload {
            return Err("Echo payload mismatch".to_string());
        }
    }

    let admin_base = format!("http://127.0.0.1:{}/_bifrost/api", port);
    let connection_id = wait_for_websocket_record_id(&admin_base).await?;
    let (frame_id, _) = wait_for_binary_frame(&admin_base, &connection_id).await?;

    let detail_url = format!(
        "{}/traffic/{}/frames/{}",
        admin_base, connection_id, frame_id
    );
    let detail: Value = reqwest::get(&detail_url)
        .await
        .map_err(|e| format!("Failed to get frame detail: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse frame detail: {}", e))?;
    if detail.get("full_payload").is_none() {
        return Err("Expected full_payload before clear".to_string());
    }

    let _ = ws_stream.close(None).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let clear_resp: Value = client
        .delete(format!("{}/traffic", admin_base))
        .json(&serde_json::json!({ "ids": [connection_id.clone()] }))
        .send()
        .await
        .map_err(|e| format!("Failed to call clear traffic: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse clear traffic response: {}", e))?;
    let clear_message = clear_resp
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Clear traffic response missing message".to_string())?;
    if !clear_message.contains("cleared successfully") {
        return Err(format!(
            "Unexpected clear traffic message: {}",
            clear_message
        ));
    }

    let detail_after: Value = reqwest::get(&detail_url)
        .await
        .map_err(|e| format!("Failed to get frame detail after clear: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse frame detail after clear: {}", e))?;
    if detail_after.get("full_payload").is_some() {
        return Err("full_payload still present after clear".to_string());
    }

    server_handle.abort();
    Ok(())
}

async fn test_ws_payload_clear_active_connection() -> Result<(), String> {
    let (ws_port, server_handle) = start_ws_echo_server().await?;

    let port = portpicker::pick_unused_port().unwrap();
    let (_proxy, _admin_state) = ProxyInstance::start_with_admin(port, vec![], false, false)
        .await
        .map_err(|e| format!("Failed to start proxy: {}", e))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let target_url = format!("ws://127.0.0.1:{}/echo", ws_port);
    let proxy_addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect(proxy_addr)
        .await
        .map_err(|e| format!("Failed to connect to proxy: {}", e))?;
    let request = target_url
        .into_client_request()
        .map_err(|e| format!("Failed to build ws request: {}", e))?;

    let (mut ws_stream, _) = client_async(request, stream)
        .await
        .map_err(|e| format!("Failed to open websocket: {}", e))?;

    let payload = vec![9u8, 8, 7, 6];
    ws_stream
        .send(Message::Binary(payload.clone().into()))
        .await
        .map_err(|e| format!("Failed to send ws payload: {}", e))?;

    if let Some(msg) = ws_stream.next().await {
        let msg = msg.map_err(|e| format!("Failed to receive ws message: {}", e))?;
        if msg.into_data() != payload {
            return Err("Echo payload mismatch".to_string());
        }
    }

    let admin_base = format!("http://127.0.0.1:{}/_bifrost/api", port);
    let connection_id = wait_for_websocket_record_id(&admin_base).await?;
    let (frame_id, _) = wait_for_binary_frame(&admin_base, &connection_id).await?;

    let detail_url = format!(
        "{}/traffic/{}/frames/{}",
        admin_base, connection_id, frame_id
    );
    let detail: Value = reqwest::get(&detail_url)
        .await
        .map_err(|e| format!("Failed to get frame detail: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse frame detail: {}", e))?;
    if detail.get("full_payload").is_none() {
        return Err("Expected full_payload before clear".to_string());
    }

    let client = reqwest::Client::new();
    let clear_resp: Value = client
        .delete(format!("{}/traffic", admin_base))
        .json(&serde_json::json!({ "ids": [connection_id.clone()] }))
        .send()
        .await
        .map_err(|e| format!("Failed to call clear traffic: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse clear traffic response: {}", e))?;
    let clear_message = clear_resp
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Clear traffic response missing message".to_string())?;
    if !clear_message.contains("active connections") {
        return Err(format!(
            "Unexpected clear traffic message: {}",
            clear_message
        ));
    }

    let detail_after: Value = reqwest::get(&detail_url)
        .await
        .map_err(|e| format!("Failed to get frame detail after clear: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse frame detail after clear: {}", e))?;
    if detail_after.get("full_payload").is_none() {
        return Err("full_payload removed for active connection".to_string());
    }

    let _ = ws_stream.close(None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    server_handle.abort();
    Ok(())
}
