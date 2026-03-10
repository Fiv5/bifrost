use std::collections::HashSet;
use std::io;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::read::DeflateDecoder;
use sha1::{Digest, Sha1};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x0 => Some(Opcode::Continuation),
            0x1 => Some(Opcode::Text),
            0x2 => Some(Opcode::Binary),
            0x8 => Some(Opcode::Close),
            0x9 => Some(Opcode::Ping),
            0xA => Some(Opcode::Pong),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct WebSocketFrame {
    pub(super) fin: bool,
    pub(super) rsv1: bool,
    pub(super) rsv2: bool,
    pub(super) rsv3: bool,
    pub(super) opcode: Opcode,
    pub(super) mask: Option<[u8; 4]>,
    pub(super) payload: Bytes,
}

impl WebSocketFrame {
    pub(super) fn encode(&self) -> Bytes {
        let payload_len = self.payload.len();
        let mut buf = BytesMut::with_capacity(14 + payload_len);

        let mut first_byte = self.opcode as u8;
        if self.fin {
            first_byte |= 0x80;
        }
        if self.rsv1 {
            first_byte |= 0x40;
        }
        if self.rsv2 {
            first_byte |= 0x20;
        }
        if self.rsv3 {
            first_byte |= 0x10;
        }
        buf.put_u8(first_byte);

        let mask_bit = if self.mask.is_some() { 0x80 } else { 0 };
        if payload_len < 126 {
            buf.put_u8(mask_bit | payload_len as u8);
        } else if payload_len < 65536 {
            buf.put_u8(mask_bit | 126);
            buf.put_u16(payload_len as u16);
        } else {
            buf.put_u8(mask_bit | 127);
            buf.put_u64(payload_len as u64);
        }

        if let Some(mask) = self.mask {
            buf.put_slice(&mask);
            let mut masked_payload = self.payload.to_vec();
            for (i, byte) in masked_payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
            buf.extend_from_slice(&masked_payload);
        } else {
            buf.extend_from_slice(&self.payload);
        }

        buf.freeze()
    }

    pub(super) fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 {
            return None;
        }

        let first_byte = data[0];
        let second_byte = data[1];

        let fin = (first_byte & 0x80) != 0;
        let rsv1 = (first_byte & 0x40) != 0;
        let rsv2 = (first_byte & 0x20) != 0;
        let rsv3 = (first_byte & 0x10) != 0;
        let opcode = Opcode::from_u8(first_byte & 0x0F)?;
        let masked = (second_byte & 0x80) != 0;
        let payload_len_indicator = second_byte & 0x7F;

        let mut offset = 2;
        let payload_len: usize;

        match payload_len_indicator.cmp(&126) {
            std::cmp::Ordering::Less => {
                payload_len = payload_len_indicator as usize;
            }
            std::cmp::Ordering::Equal => {
                if data.len() < offset + 2 {
                    return None;
                }
                payload_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2;
            }
            std::cmp::Ordering::Greater => {
                if data.len() < offset + 8 {
                    return None;
                }
                let mut len_bytes = [0u8; 8];
                len_bytes.copy_from_slice(&data[offset..offset + 8]);
                payload_len = u64::from_be_bytes(len_bytes) as usize;
                offset += 8;
            }
        }

        let mask = if masked {
            if data.len() < offset + 4 {
                return None;
            }
            let mut m = [0u8; 4];
            m.copy_from_slice(&data[offset..offset + 4]);
            offset += 4;
            Some(m)
        } else {
            None
        };

        if data.len() < offset + payload_len {
            return None;
        }

        let mut payload = data[offset..offset + payload_len].to_vec();
        if let Some(m) = mask {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= m[i % 4];
            }
        }

        let frame = WebSocketFrame {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask,
            payload: Bytes::from(payload),
        };

        Some((frame, offset + payload_len))
    }

    pub(super) fn close_code(&self) -> Option<u16> {
        if self.opcode == Opcode::Close && self.payload.len() >= 2 {
            Some(u16::from_be_bytes([self.payload[0], self.payload[1]]))
        } else {
            None
        }
    }

    pub(super) fn close_reason(&self) -> Option<&str> {
        if self.opcode == Opcode::Close && self.payload.len() > 2 {
            std::str::from_utf8(&self.payload[2..]).ok()
        } else {
            None
        }
    }

    pub(super) fn decompress_payload(&self) -> Bytes {
        if !self.rsv1 || self.payload.is_empty() {
            return self.payload.clone();
        }

        let mut data = self.payload.to_vec();
        data.extend_from_slice(&[0x00, 0x00, 0xff, 0xff]);

        let mut decoder = DeflateDecoder::new(&data[..]);
        let mut decompressed = Vec::new();
        match decoder.read_to_end(&mut decompressed) {
            Ok(_) => Bytes::from(decompressed),
            Err(_) => self.payload.clone(),
        }
    }
}

pub(super) struct WebSocketReader<R> {
    inner: R,
    buffer: BytesMut,
}

impl<R> WebSocketReader<R> {
    pub(super) fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: BytesMut::with_capacity(8192),
        }
    }

    pub(super) fn with_initial_buffer(inner: R, buffer: BytesMut) -> Self {
        Self { inner, buffer }
    }
}

impl<R: AsyncRead + Unpin> WebSocketReader<R> {
    pub(super) async fn next_frame(&mut self) -> io::Result<Option<WebSocketFrame>> {
        loop {
            if let Some((frame, consumed)) = WebSocketFrame::parse(&self.buffer) {
                self.buffer.advance(consumed);
                return Ok(Some(frame));
            }

            let mut chunk = [0u8; 8192];
            let n = self.inner.read(&mut chunk).await?;
            if n == 0 {
                return Ok(None);
            }
            self.buffer.extend_from_slice(&chunk[..n]);
        }
    }
}

pub(super) struct WebSocketWriter<W> {
    inner: W,
    is_client: bool,
}

impl<W> WebSocketWriter<W> {
    pub(super) fn new(inner: W, is_client: bool) -> Self {
        Self { inner, is_client }
    }
}

impl<W: AsyncWrite + Unpin> WebSocketWriter<W> {
    pub(super) async fn write_frame(&mut self, mut frame: WebSocketFrame) -> io::Result<()> {
        if self.is_client && frame.mask.is_none() {
            frame.mask = Some(generate_mask());
        }
        let encoded = frame.encode();
        self.inner.write_all(&encoded).await?;
        self.inner.flush().await?;
        Ok(())
    }
}

fn generate_mask() -> [u8; 4] {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32;
    seed.to_be_bytes()
}

pub(super) fn compute_accept_key(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
}

pub(super) fn generate_sec_websocket_key() -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let bytes = seed.to_le_bytes();
    base64::engine::general_purpose::STANDARD.encode(&bytes[..16])
}

#[derive(Debug, Clone)]
pub(super) struct HttpResponse {
    pub(super) status_code: u16,
    pub(super) status_text: String,
    pub(super) headers: Vec<(String, String)>,
}

impl HttpResponse {
    pub(super) fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub(super) fn parse(data: &[u8]) -> Option<(Self, usize)> {
        let header_end = find_header_end(data)?;
        let header_str = std::str::from_utf8(&data[..header_end]).ok()?;

        let mut lines = header_str.lines();
        let first_line = lines.next()?;
        let parts: Vec<&str> = first_line.splitn(3, ' ').collect();
        if parts.len() < 2 {
            return None;
        }

        let status_code: u16 = parts[1].parse().ok()?;
        let status_text = parts.get(2).unwrap_or(&"").to_string();

        let mut headers = Vec::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.push((key, value));
            }
        }

        let header_total = header_end + 4;
        Some((
            HttpResponse {
                status_code,
                status_text,
                headers,
            },
            header_total,
        ))
    }
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    (0..data.len().saturating_sub(3)).find(|&i| &data[i..i + 4] == b"\r\n\r\n")
}

pub(super) async fn read_http1_response_with_leftover<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<(HttpResponse, BytesMut), String> {
    let mut buf = BytesMut::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    let max = 64 * 1024;

    loop {
        if buf.len() > max {
            return Err("HTTP response headers too large".to_string());
        }

        if let Some((resp, consumed)) = HttpResponse::parse(&buf) {
            let leftover = buf.split_off(consumed);
            return Ok((resp, leftover));
        }

        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|e| format!("Failed to read handshake response: {}", e))?;
        if n == 0 {
            return Err("Upstream closed connection during handshake".to_string());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

pub(super) fn header_values(resp: &HttpResponse, name: &str) -> Vec<String> {
    resp.headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
        .collect()
}

pub(super) fn negotiate_protocol(
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
        .collect::<HashSet<_>>();
    if offered.contains(upstream_selected) {
        Some(upstream_selected.to_string())
    } else {
        None
    }
}

pub(super) fn negotiate_extensions(
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
        .collect::<HashSet<_>>();

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

pub(super) fn parse_permessage_deflate(extensions: &str) -> bool {
    extensions
        .split(',')
        .any(|ext| ext.trim().starts_with("permessage-deflate"))
}
