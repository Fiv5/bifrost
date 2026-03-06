use std::sync::Arc;
use std::time::Instant;

use bifrost_admin::{
    AdminState, FrameDirection, FrameType, RequestTiming, TrafficRecord, TrafficType,
};
use bifrost_core::{BifrostError, Result};

use futures_util::StreamExt;
use hyper::body::Incoming;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, trace};

use crate::protocol::{
    extract_sec_websocket_extensions, parse_permessage_deflate, Opcode, WebSocketReader,
    WebSocketWriter,
};
use crate::server::{empty_body, BoxBody, RulesResolver};
use crate::utils::logging::RequestContext;
use crate::utils::process_info::resolve_client_process;

fn persist_socket_summary(state: &AdminState, record_id: &str) {
    let status = state.connection_monitor.get_connection_status(record_id);
    let last_frame_id = state
        .connection_monitor
        .get_last_frame_id(record_id)
        .unwrap_or(0);
    let frame_count = status.as_ref().map(|s| s.frame_count).unwrap_or(0);
    let status = status.map(|mut s| {
        s.is_open = false;
        s
    });
    let response_size = status
        .as_ref()
        .map(|s| s.send_bytes + s.receive_bytes)
        .unwrap_or(0) as usize;
    state.update_traffic_by_id(record_id, move |record| {
        record.response_size = response_size;
        record.frame_count = frame_count;
        record.last_frame_id = last_frame_id;
        if let Some(ref s) = status {
            record.socket_status = Some(s.clone());
        }
    });
}

pub async fn handle_websocket_upgrade(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    admin_state: Option<Arc<AdminState>>,
    peer_addr: std::net::SocketAddr,
) -> Result<Response<BoxBody>> {
    let ctx = RequestContext::new();
    let start_time = Instant::now();
    let uri = req.uri().clone();
    let url = uri.to_string();
    let method = req.method().to_string();

    let resolved_rules = rules.resolve(&url, "GET");

    let has_rules = !resolved_rules.rules.is_empty() || resolved_rules.host.is_some();

    let req_headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let host = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| BifrostError::Network("Missing Host header".to_string()))?;

    let (target_host, target_port) = if let Some(ref host_rule) = resolved_rules.host {
        parse_host_port(host_rule)?
    } else {
        parse_host_port(host)?
    };

    debug!("WebSocket upgrade to {}:{}", target_host, target_port);

    let connect_start = Instant::now();
    let mut target_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                target_host, target_port, e
            ))
        })?;
    let tcp_connect_ms = connect_start.elapsed().as_millis() as u64;

    if let Err(e) = target_stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY on WebSocket connection: {}", e);
    }

    let upgrade_request = build_websocket_handshake(&req)?;
    target_stream
        .write_all(upgrade_request.as_bytes())
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send handshake: {}", e)))?;

    let mut response_buf = vec![0u8; 4096];
    let n = target_stream
        .read(&mut response_buf)
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to read handshake response: {}", e)))?;

    let response_str = String::from_utf8_lossy(&response_buf[..n]);
    if !response_str.contains("101") {
        return Err(BifrostError::Network(format!(
            "WebSocket handshake failed: {}",
            response_str
        )));
    }

    let sec_accept = extract_sec_websocket_accept(&response_str);

    let compression_enabled = extract_sec_websocket_extensions(&response_str)
        .map(|ext| parse_permessage_deflate(&ext))
        .unwrap_or(false);

    let total_ms = start_time.elapsed().as_millis() as u64;
    let record_id = ctx.id_str();

    if compression_enabled {
        debug!(
            "[WS] permessage-deflate compression enabled for {}",
            record_id
        );
    }

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_requests_by_type(TrafficType::Ws);

        let ws_url = format!(
            "ws://{}{}",
            host,
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
        );

        let client_process = resolve_client_process(&peer_addr);
        let (client_app, client_pid, client_path) = client_process
            .as_ref()
            .map(|p| (Some(p.name.clone()), Some(p.pid), p.path.clone()))
            .unwrap_or((None, None, None));

        let mut record = TrafficRecord::new(record_id.clone(), method, ws_url);
        record.status = 101;
        record.protocol = "ws".to_string();
        record.duration_ms = total_ms;
        record.timing = Some(RequestTiming {
            dns_ms: None,
            connect_ms: Some(tcp_connect_ms),
            tls_ms: None,
            send_ms: None,
            wait_ms: Some(total_ms.saturating_sub(tcp_connect_ms)),
            receive_ms: None,
            total_ms,
        });
        record.request_headers = Some(req_headers);
        record.has_rule_hit = has_rules;
        record.matched_rules = crate::utils::build_matched_rules(&resolved_rules);
        record.client_ip = peer_addr.ip().to_string();
        record.client_app = client_app;
        record.client_pid = client_pid;
        record.client_path = client_path;
        record.set_websocket();

        state.connection_monitor.register_connection(&record_id);
        state.record_traffic(record);
    }

    let record_id_clone = record_id.clone();
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = websocket_bidirectional_with_capture(
                    upgraded,
                    target_stream,
                    &record_id_clone,
                    admin_state.clone(),
                    compression_enabled,
                )
                .await
                {
                    error!("WebSocket tunnel error: {}", e);
                }

                if let Some(ref state) = admin_state {
                    state.connection_monitor.set_connection_closed(
                        &record_id_clone,
                        None,
                        None,
                        state.frame_store.as_ref(),
                        state.ws_payload_store.as_ref(),
                    );
                }
            }
            Err(e) => {
                error!("WebSocket upgrade error: {}", e);
            }
        }
    });

    let mut response = Response::builder()
        .status(101)
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::CONNECTION, "Upgrade");

    if let Some(accept) = sec_accept {
        response = response.header("Sec-WebSocket-Accept", accept);
    }

    Ok(response.body(empty_body()).unwrap())
}

fn build_websocket_handshake(req: &Request<Incoming>) -> Result<String> {
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let host = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let ws_key = req
        .headers()
        .get("Sec-WebSocket-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let ws_version = req
        .headers()
        .get("Sec-WebSocket-Version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("13");

    let mut handshake = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: {}\r\n",
        path, host, ws_key, ws_version
    );

    if let Some(protocol) = req
        .headers()
        .get("Sec-WebSocket-Protocol")
        .and_then(|v| v.to_str().ok())
    {
        handshake.push_str(&format!("Sec-WebSocket-Protocol: {}\r\n", protocol));
    }

    if let Some(extensions) = req
        .headers()
        .get("Sec-WebSocket-Extensions")
        .and_then(|v| v.to_str().ok())
    {
        handshake.push_str(&format!("Sec-WebSocket-Extensions: {}\r\n", extensions));
    }

    handshake.push_str("\r\n");

    Ok(handshake)
}

fn extract_sec_websocket_accept(response: &str) -> Option<String> {
    for line in response.lines() {
        if line.to_lowercase().starts_with("sec-websocket-accept:") {
            return Some(
                line.split(':')
                    .skip(1)
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
        }
    }
    None
}

fn parse_host_port(host: &str) -> Result<(String, u16)> {
    let host_without_path = host.split('/').next().unwrap_or(host);

    let parts: Vec<&str> = host_without_path.split(':').collect();
    match parts.len() {
        1 => Ok((parts[0].to_string(), 80)),
        2 => {
            let port = parts[1]
                .parse()
                .map_err(|_| BifrostError::Parse(format!("Invalid port: {}", parts[1])))?;
            Ok((parts[0].to_string(), port))
        }
        _ => Err(BifrostError::Parse(format!("Invalid host: {}", host))),
    }
}

fn opcode_to_frame_type(opcode: Opcode) -> FrameType {
    match opcode {
        Opcode::Continuation => FrameType::Continuation,
        Opcode::Text => FrameType::Text,
        Opcode::Binary => FrameType::Binary,
        Opcode::Close => FrameType::Close,
        Opcode::Ping => FrameType::Ping,
        Opcode::Pong => FrameType::Pong,
    }
}

async fn websocket_bidirectional_with_capture(
    upgraded: Upgraded,
    target: TcpStream,
    record_id: &str,
    admin_state: Option<Arc<AdminState>>,
    compression_enabled: bool,
) -> Result<()> {
    let client = TokioIo::new(upgraded);
    let (target_read, target_write) = target.into_split();
    let (client_read, client_write) = tokio::io::split(client);

    let record_id_owned = record_id.to_string();
    let admin_state_c2s = admin_state.clone();
    let admin_state_s2c = admin_state.clone();

    let client_to_server = async move {
        let mut reader = WebSocketReader::new(client_read);
        let mut writer = WebSocketWriter::new(target_write, true);

        while let Some(result) = reader.next().await {
            let frame = match result {
                Ok(f) => f,
                Err(e) => {
                    trace!("Client read error: {}", e);
                    break;
                }
            };

            if let Some(ref state) = admin_state_c2s {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Ws, frame.payload.len() as u64);

                let payload_for_record = if compression_enabled && frame.is_compressed() {
                    frame.decompress_payload()
                } else {
                    frame.payload.clone()
                };

                state.connection_monitor.record_frame(
                    &record_id_owned,
                    FrameDirection::Send,
                    opcode_to_frame_type(frame.opcode),
                    &payload_for_record,
                    frame.mask.is_some(),
                    frame.fin,
                    state.body_store.as_ref(),
                    state.ws_payload_store.as_ref(),
                    state.frame_store.as_ref(),
                );

                if frame.opcode == Opcode::Close {
                    let close_code = frame.close_code();
                    let close_reason = frame.close_reason().map(str::to_string);
                    state.connection_monitor.set_connection_closed(
                        &record_id_owned,
                        close_code,
                        close_reason,
                        state.frame_store.as_ref(),
                        state.ws_payload_store.as_ref(),
                    );
                    persist_socket_summary(state, &record_id_owned);
                }
            }

            if let Err(e) = writer.write_frame(frame).await {
                trace!("Server write error: {}", e);
                break;
            }
        }

        Ok::<_, std::io::Error>(())
    };

    let record_id_owned2 = record_id.to_string();
    let server_to_client = async move {
        let mut reader = WebSocketReader::new(target_read);
        let mut writer = WebSocketWriter::new(client_write, false);

        while let Some(result) = reader.next().await {
            let frame = match result {
                Ok(f) => f,
                Err(e) => {
                    trace!("Server read error: {}", e);
                    break;
                }
            };

            if let Some(ref state) = admin_state_s2c {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Ws, frame.payload.len() as u64);

                let payload_for_record = if compression_enabled && frame.is_compressed() {
                    frame.decompress_payload()
                } else {
                    frame.payload.clone()
                };

                state.connection_monitor.record_frame(
                    &record_id_owned2,
                    FrameDirection::Receive,
                    opcode_to_frame_type(frame.opcode),
                    &payload_for_record,
                    frame.mask.is_some(),
                    frame.fin,
                    state.body_store.as_ref(),
                    state.ws_payload_store.as_ref(),
                    state.frame_store.as_ref(),
                );

                if frame.opcode == Opcode::Close {
                    let close_code = frame.close_code();
                    let close_reason = frame.close_reason().map(str::to_string);
                    state.connection_monitor.set_connection_closed(
                        &record_id_owned2,
                        close_code,
                        close_reason,
                        state.frame_store.as_ref(),
                        state.ws_payload_store.as_ref(),
                    );
                    persist_socket_summary(state, &record_id_owned2);
                }
            }

            if let Err(e) = writer.write_frame(frame).await {
                trace!("Client write error: {}", e);
                break;
            }
        }

        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_server, server_to_client);

    if let Some(ref state) = admin_state {
        let should_close = state
            .connection_monitor
            .get_connection_status(record_id)
            .map(|s| s.is_open)
            .unwrap_or(false);
        if should_close {
            state.connection_monitor.set_connection_closed(
                record_id,
                None,
                None,
                state.frame_store.as_ref(),
                state.ws_payload_store.as_ref(),
            );
        }
        persist_socket_summary(state, record_id);
    }

    match result {
        Ok(_) => {
            debug!("WebSocket connection closed normally");
            Ok(())
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::BrokenPipe
            {
                debug!("WebSocket connection closed: {}", e);
                Ok(())
            } else {
                Err(BifrostError::Network(format!("WebSocket error: {}", e)))
            }
        }
    }
}

pub async fn websocket_bidirectional_generic_with_capture<S>(
    upgraded: Upgraded,
    target: S,
    record_id: &str,
    admin_state: Option<Arc<AdminState>>,
    compression_enabled: bool,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let client = TokioIo::new(upgraded);
    let (target_read, target_write) = tokio::io::split(target);
    let (client_read, client_write) = tokio::io::split(client);

    let record_id_owned = record_id.to_string();
    let admin_state_c2s = admin_state.clone();
    let admin_state_s2c = admin_state.clone();

    let client_to_server = async move {
        let mut reader = WebSocketReader::new(client_read);
        let mut writer = WebSocketWriter::new(target_write, true);

        while let Some(result) = reader.next().await {
            let frame = match result {
                Ok(f) => f,
                Err(e) => {
                    trace!("Client read error: {}", e);
                    break;
                }
            };

            if let Some(ref state) = admin_state_c2s {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Ws, frame.payload.len() as u64);

                let payload_for_record = if compression_enabled && frame.is_compressed() {
                    frame.decompress_payload()
                } else {
                    frame.payload.clone()
                };

                state.connection_monitor.record_frame(
                    &record_id_owned,
                    FrameDirection::Send,
                    opcode_to_frame_type(frame.opcode),
                    &payload_for_record,
                    frame.mask.is_some(),
                    frame.fin,
                    state.body_store.as_ref(),
                    state.ws_payload_store.as_ref(),
                    state.frame_store.as_ref(),
                );

                if frame.opcode == Opcode::Close {
                    let close_code = frame.close_code();
                    let close_reason = frame.close_reason().map(str::to_string);
                    state.connection_monitor.set_connection_closed(
                        &record_id_owned,
                        close_code,
                        close_reason,
                        state.frame_store.as_ref(),
                        state.ws_payload_store.as_ref(),
                    );
                    persist_socket_summary(state, &record_id_owned);
                }
            }

            if let Err(e) = writer.write_frame(frame).await {
                trace!("Server write error: {}", e);
                break;
            }
        }

        Ok::<_, std::io::Error>(())
    };

    let record_id_owned2 = record_id.to_string();
    let server_to_client = async move {
        let mut reader = WebSocketReader::new(target_read);
        let mut writer = WebSocketWriter::new(client_write, false);

        while let Some(result) = reader.next().await {
            let frame = match result {
                Ok(f) => f,
                Err(e) => {
                    trace!("Server read error: {}", e);
                    break;
                }
            };

            if let Some(ref state) = admin_state_s2c {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Ws, frame.payload.len() as u64);

                let payload_for_record = if compression_enabled && frame.is_compressed() {
                    frame.decompress_payload()
                } else {
                    frame.payload.clone()
                };

                state.connection_monitor.record_frame(
                    &record_id_owned2,
                    FrameDirection::Receive,
                    opcode_to_frame_type(frame.opcode),
                    &payload_for_record,
                    frame.mask.is_some(),
                    frame.fin,
                    state.body_store.as_ref(),
                    state.ws_payload_store.as_ref(),
                    state.frame_store.as_ref(),
                );

                if frame.opcode == Opcode::Close {
                    let close_code = frame.close_code();
                    let close_reason = frame.close_reason().map(str::to_string);
                    state.connection_monitor.set_connection_closed(
                        &record_id_owned2,
                        close_code,
                        close_reason,
                        state.frame_store.as_ref(),
                        state.ws_payload_store.as_ref(),
                    );
                    persist_socket_summary(state, &record_id_owned2);
                }
            }

            if let Err(e) = writer.write_frame(frame).await {
                trace!("Client write error: {}", e);
                break;
            }
        }

        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_server, server_to_client);

    if let Some(ref state) = admin_state {
        let should_close = state
            .connection_monitor
            .get_connection_status(record_id)
            .map(|s| s.is_open)
            .unwrap_or(false);
        if should_close {
            state.connection_monitor.set_connection_closed(
                record_id,
                None,
                None,
                state.frame_store.as_ref(),
                state.ws_payload_store.as_ref(),
            );
        }
        persist_socket_summary(state, record_id);
    }

    match result {
        Ok(_) => {
            debug!("WebSocket connection closed normally");
            Ok(())
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::BrokenPipe
            {
                debug!("WebSocket connection closed: {}", e);
                Ok(())
            } else {
                Err(BifrostError::Network(format!("WebSocket error: {}", e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_port_with_port() {
        let (host, port) = parse_host_port("example.com:8080").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_host_port_default() {
        let (host, port) = parse_host_port("example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
    }

    #[test]
    fn test_parse_host_port_invalid() {
        let result = parse_host_port("example.com:invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_host_port_with_path() {
        let (host, port) = parse_host_port("127.0.0.1:3020/ws").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 3020);
    }

    #[test]
    fn test_parse_host_port_with_path_no_port() {
        let (host, port) = parse_host_port("example.com/ws/path").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
    }

    #[test]
    fn test_websocket_opcode_from_u8() {
        use crate::protocol::{Opcode, WebSocketFrame};
        assert_eq!(Opcode::from_u8(0x0), Some(Opcode::Continuation));
        assert_eq!(Opcode::from_u8(0x1), Some(Opcode::Text));
        assert_eq!(Opcode::from_u8(0x2), Some(Opcode::Binary));
        assert_eq!(Opcode::from_u8(0x8), Some(Opcode::Close));
        assert_eq!(Opcode::from_u8(0x9), Some(Opcode::Ping));
        assert_eq!(Opcode::from_u8(0xA), Some(Opcode::Pong));
        assert_eq!(Opcode::from_u8(0xF), None);
        let _ = WebSocketFrame::text("");
    }

    #[test]
    fn test_websocket_frame_text() {
        use crate::protocol::{Opcode, WebSocketFrame};
        let frame = WebSocketFrame::text("hello");
        assert!(frame.fin);
        assert_eq!(frame.opcode, Opcode::Text);
        assert!(frame.mask.is_none());
        assert_eq!(frame.payload, bytes::Bytes::from("hello"));
    }

    #[test]
    fn test_websocket_frame_binary() {
        use crate::protocol::{Opcode, WebSocketFrame};
        let data = vec![0x01, 0x02, 0x03];
        let frame = WebSocketFrame::binary(data.clone());
        assert!(frame.fin);
        assert_eq!(frame.opcode, Opcode::Binary);
        assert_eq!(frame.payload, bytes::Bytes::from(data));
    }

    #[test]
    fn test_websocket_frame_close() {
        use crate::protocol::{Opcode, WebSocketFrame};
        let frame = WebSocketFrame::close(None, "");
        assert!(frame.fin);
        assert_eq!(frame.opcode, Opcode::Close);
    }

    #[test]
    fn test_websocket_frame_ping_pong() {
        use crate::protocol::{Opcode, WebSocketFrame};
        let ping = WebSocketFrame::ping("ping");
        assert_eq!(ping.opcode, Opcode::Ping);

        let pong = WebSocketFrame::pong("pong");
        assert_eq!(pong.opcode, Opcode::Pong);
    }

    #[test]
    fn test_extract_sec_websocket_accept() {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\
                        Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
        let accept = extract_sec_websocket_accept(response);
        assert_eq!(accept, Some("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=".to_string()));
    }

    #[test]
    fn test_extract_sec_websocket_accept_missing() {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\r\n";
        let accept = extract_sec_websocket_accept(response);
        assert!(accept.is_none());
    }
}
