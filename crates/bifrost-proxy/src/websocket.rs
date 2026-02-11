use std::sync::Arc;
use std::time::Instant;

use bifrost_admin::{AdminState, MatchedRule, RequestTiming, TrafficRecord};
use bifrost_core::{BifrostError, Result};
use bytes::Bytes;
use hyper::body::Incoming;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error};

use crate::logging::RequestContext;
use crate::server::{empty_body, BoxBody, RulesResolver};

pub async fn handle_websocket_upgrade(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    admin_state: Option<Arc<AdminState>>,
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

    let total_ms = start_time.elapsed().as_millis() as u64;
    let record_id = ctx.id_str();

    if let Some(ref state) = admin_state {
        state.metrics_collector.increment_requests();

        let ws_url = format!(
            "ws://{}{}",
            host,
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
        );

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
        record.matched_rules = if resolved_rules.rules.is_empty() {
            None
        } else {
            Some(
                resolved_rules
                    .rules
                    .iter()
                    .map(|r| MatchedRule {
                        pattern: r.pattern.clone(),
                        protocol: format!("{:?}", r.protocol),
                        value: r.value.clone(),
                    })
                    .collect(),
            )
        };

        state.traffic_recorder.record(record);
    }

    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = websocket_bidirectional(upgraded, target_stream, admin_state).await
                {
                    error!("WebSocket tunnel error: {}", e);
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

async fn websocket_bidirectional(
    upgraded: Upgraded,
    target: TcpStream,
    admin_state: Option<Arc<AdminState>>,
) -> Result<()> {
    let client = TokioIo::new(upgraded);
    let (mut target_read, mut target_write) = target.into_split();

    let (client_read, client_write) = tokio::io::split(client);
    let mut client_read = client_read;
    let mut client_write = client_write;

    let admin_state_clone = admin_state.clone();

    let client_to_target = async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone {
                state.metrics_collector.add_bytes_sent(n as u64);
            }
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(())
    };

    let target_to_client = async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state {
                state.metrics_collector.add_bytes_received(n as u64);
            }
        }
        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_target, target_to_client);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketOpcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl WebSocketOpcode {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte & 0x0F {
            0x0 => Some(WebSocketOpcode::Continuation),
            0x1 => Some(WebSocketOpcode::Text),
            0x2 => Some(WebSocketOpcode::Binary),
            0x8 => Some(WebSocketOpcode::Close),
            0x9 => Some(WebSocketOpcode::Ping),
            0xA => Some(WebSocketOpcode::Pong),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct WebSocketFrame {
    pub fin: bool,
    pub opcode: WebSocketOpcode,
    pub masked: bool,
    pub payload: Bytes,
}

impl WebSocketFrame {
    pub fn text(data: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            opcode: WebSocketOpcode::Text,
            masked: false,
            payload: data.into(),
        }
    }

    pub fn binary(data: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            opcode: WebSocketOpcode::Binary,
            masked: false,
            payload: data.into(),
        }
    }

    pub fn close() -> Self {
        Self {
            fin: true,
            opcode: WebSocketOpcode::Close,
            masked: false,
            payload: Bytes::new(),
        }
    }

    pub fn ping(data: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            opcode: WebSocketOpcode::Ping,
            masked: false,
            payload: data.into(),
        }
    }

    pub fn pong(data: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            opcode: WebSocketOpcode::Pong,
            masked: false,
            payload: data.into(),
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
    fn test_websocket_opcode_from_byte() {
        assert_eq!(
            WebSocketOpcode::from_byte(0x0),
            Some(WebSocketOpcode::Continuation)
        );
        assert_eq!(WebSocketOpcode::from_byte(0x1), Some(WebSocketOpcode::Text));
        assert_eq!(
            WebSocketOpcode::from_byte(0x2),
            Some(WebSocketOpcode::Binary)
        );
        assert_eq!(
            WebSocketOpcode::from_byte(0x8),
            Some(WebSocketOpcode::Close)
        );
        assert_eq!(WebSocketOpcode::from_byte(0x9), Some(WebSocketOpcode::Ping));
        assert_eq!(WebSocketOpcode::from_byte(0xA), Some(WebSocketOpcode::Pong));
        assert_eq!(WebSocketOpcode::from_byte(0xF), None);
    }

    #[test]
    fn test_websocket_frame_text() {
        let frame = WebSocketFrame::text("hello");
        assert!(frame.fin);
        assert_eq!(frame.opcode, WebSocketOpcode::Text);
        assert!(!frame.masked);
        assert_eq!(frame.payload, Bytes::from("hello"));
    }

    #[test]
    fn test_websocket_frame_binary() {
        let data = vec![0x01, 0x02, 0x03];
        let frame = WebSocketFrame::binary(data.clone());
        assert!(frame.fin);
        assert_eq!(frame.opcode, WebSocketOpcode::Binary);
        assert_eq!(frame.payload, Bytes::from(data));
    }

    #[test]
    fn test_websocket_frame_close() {
        let frame = WebSocketFrame::close();
        assert!(frame.fin);
        assert_eq!(frame.opcode, WebSocketOpcode::Close);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn test_websocket_frame_ping_pong() {
        let ping = WebSocketFrame::ping("ping");
        assert_eq!(ping.opcode, WebSocketOpcode::Ping);

        let pong = WebSocketFrame::pong("pong");
        assert_eq!(pong.opcode, WebSocketOpcode::Pong);
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
