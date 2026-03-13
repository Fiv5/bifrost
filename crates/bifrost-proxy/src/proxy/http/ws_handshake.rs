use bifrost_core::{BifrostError, Result};
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::protocol::HttpResponse;

pub async fn read_http1_response_with_leftover<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_header_size: usize,
) -> Result<(HttpResponse, BytesMut)> {
    let mut buf = BytesMut::with_capacity(8192);
    let mut chunk = [0u8; 4096];

    loop {
        if buf.len() > max_header_size {
            return Err(BifrostError::Network(format!(
                "HTTP response headers too large ({} > {} bytes)",
                buf.len(),
                max_header_size
            )));
        }

        if let Some((resp, consumed)) = HttpResponse::parse(&buf) {
            let leftover = buf.split_off(consumed);
            return Ok((resp, leftover));
        }

        let n = reader.read(&mut chunk).await.map_err(|e| {
            BifrostError::Network(format!("Failed to read handshake response: {}", e))
        })?;
        if n == 0 {
            return Err(BifrostError::Network(
                "Upstream closed connection during handshake".to_string(),
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

pub fn header_values(resp: &HttpResponse, name: &str) -> Vec<String> {
    resp.headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
        .collect()
}

pub fn negotiate_protocol(
    client_offer: Option<&str>,
    upstream_selected: Option<&str>,
) -> Option<String> {
    let upstream_selected = upstream_selected?.trim();
    if upstream_selected.is_empty() {
        return None;
    }
    let offered = client_offer?
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<std::collections::HashSet<_>>();
    if offered.contains(upstream_selected) {
        Some(upstream_selected.to_string())
    } else {
        None
    }
}

pub fn negotiate_extensions(
    client_offer: Option<&str>,
    upstream_values: &[String],
) -> Option<String> {
    let client_offer = client_offer?;
    let offered = client_offer
        .split(',')
        .map(|ext| ext.trim())
        .filter(|ext| !ext.is_empty())
        .map(|ext| {
            ext.split(';')
                .next()
                .unwrap_or(ext)
                .trim()
                .to_ascii_lowercase()
        })
        .collect::<std::collections::HashSet<_>>();

    if offered.is_empty() {
        return None;
    }

    let mut accepted_segments = Vec::new();
    for v in upstream_values {
        for seg in v.split(',') {
            let seg = seg.trim();
            if seg.is_empty() {
                continue;
            }
            let name = seg
                .split(';')
                .next()
                .unwrap_or(seg)
                .trim()
                .to_ascii_lowercase();
            if offered.contains(&name) {
                accepted_segments.push(seg.to_string());
            }
        }
    }

    if accepted_segments.is_empty() {
        None
    } else {
        Some(accepted_segments.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_extensions_filters_by_client_offer() {
        let client = Some("permessage-deflate; client_max_window_bits, x-foo");
        let upstream = vec![
            "permessage-deflate; server_no_context_takeover".to_string(),
            "x-bar; a=b".to_string(),
        ];
        assert_eq!(
            negotiate_extensions(client, &upstream),
            Some("permessage-deflate; server_no_context_takeover".to_string())
        );
    }

    #[test]
    fn test_negotiate_extensions_none_when_client_missing() {
        let upstream = vec!["permessage-deflate".to_string()];
        assert_eq!(negotiate_extensions(None, &upstream), None);
    }

    #[test]
    fn test_negotiate_protocol_matches_client_offer() {
        assert_eq!(
            negotiate_protocol(Some("chat, superchat"), Some("superchat")),
            Some("superchat".to_string())
        );
        assert_eq!(
            negotiate_protocol(Some("chat, superchat"), Some("unknown")),
            None
        );
    }
}
