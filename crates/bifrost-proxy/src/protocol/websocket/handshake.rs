const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub fn extract_sec_websocket_extensions(response: &str) -> Option<String> {
    for line in response.lines() {
        if line.to_lowercase().starts_with("sec-websocket-extensions:") {
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

pub fn compute_accept_key(key: &str) -> String {
    use base64::Engine;
    use sha1::{Digest, Sha1};

    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    let result = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(result)
}

pub fn generate_sec_websocket_key() -> String {
    use base64::Engine;
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let bytes = seed.to_le_bytes();
    base64::engine::general_purpose::STANDARD.encode(&bytes[..16])
}

pub fn build_websocket_request_headers(
    host: &str,
    path: &str,
    key: &str,
    protocols: Option<&[&str]>,
) -> String {
    let mut headers = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n",
        path, host, key
    );

    if let Some(protocols) = protocols {
        headers.push_str(&format!(
            "Sec-WebSocket-Protocol: {}\r\n",
            protocols.join(", ")
        ));
    }

    headers.push_str("\r\n");
    headers
}

pub fn build_websocket_response_headers(key: &str, protocol: Option<&str>) -> String {
    let accept = compute_accept_key(key);

    let mut headers = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n",
        accept
    );

    if let Some(protocol) = protocol {
        headers.push_str(&format!("Sec-WebSocket-Protocol: {}\r\n", protocol));
    }

    headers.push_str("\r\n");
    headers
}
