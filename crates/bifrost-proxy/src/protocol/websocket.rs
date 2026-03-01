use std::io::Read;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::read::DeflateDecoder;
use futures_util::Stream;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn from_u8(value: u8) -> Option<Self> {
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

    pub fn is_control(&self) -> bool {
        matches!(self, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
}

#[derive(Debug, Clone)]
pub struct WebSocketFrame {
    pub fin: bool,
    pub rsv1: bool,
    pub opcode: Opcode,
    pub mask: Option<[u8; 4]>,
    pub payload: Bytes,
}

impl WebSocketFrame {
    pub fn text(data: impl AsRef<str>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::copy_from_slice(data.as_ref().as_bytes()),
        }
    }

    pub fn binary(data: impl AsRef<[u8]>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            opcode: Opcode::Binary,
            mask: None,
            payload: Bytes::copy_from_slice(data.as_ref()),
        }
    }

    pub fn ping(data: impl AsRef<[u8]>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            opcode: Opcode::Ping,
            mask: None,
            payload: Bytes::copy_from_slice(data.as_ref()),
        }
    }

    pub fn pong(data: impl AsRef<[u8]>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            opcode: Opcode::Pong,
            mask: None,
            payload: Bytes::copy_from_slice(data.as_ref()),
        }
    }

    pub fn close(code: Option<u16>, reason: &str) -> Self {
        let mut payload = BytesMut::new();
        if let Some(code) = code {
            payload.put_u16(code);
            payload.extend_from_slice(reason.as_bytes());
        }
        Self {
            fin: true,
            rsv1: false,
            opcode: Opcode::Close,
            mask: None,
            payload: payload.freeze(),
        }
    }

    pub fn with_mask(mut self, mask: [u8; 4]) -> Self {
        self.mask = Some(mask);
        self
    }

    pub fn encode(&self) -> Bytes {
        let payload_len = self.payload.len();
        let mut buf = BytesMut::with_capacity(14 + payload_len);

        let mut first_byte = self.opcode as u8;
        if self.fin {
            first_byte |= 0x80;
        }
        if self.rsv1 {
            first_byte |= 0x40;
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

    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 {
            return None;
        }

        let first_byte = data[0];
        let second_byte = data[1];

        let fin = (first_byte & 0x80) != 0;
        let rsv1 = (first_byte & 0x40) != 0;
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
            opcode,
            mask,
            payload: Bytes::from(payload),
        };

        Some((frame, offset + payload_len))
    }

    pub fn close_code(&self) -> Option<u16> {
        if self.opcode == Opcode::Close && self.payload.len() >= 2 {
            Some(u16::from_be_bytes([self.payload[0], self.payload[1]]))
        } else {
            None
        }
    }

    pub fn close_reason(&self) -> Option<&str> {
        if self.opcode == Opcode::Close && self.payload.len() > 2 {
            std::str::from_utf8(&self.payload[2..]).ok()
        } else {
            None
        }
    }

    pub fn decompress_payload(&self) -> Bytes {
        if !self.rsv1 || self.payload.is_empty() {
            return self.payload.clone();
        }

        let mut data = self.payload.to_vec();
        data.extend_from_slice(&[0x00, 0x00, 0xff, 0xff]);

        let mut decoder = DeflateDecoder::new(&data[..]);
        let mut decompressed = Vec::new();
        match decoder.read_to_end(&mut decompressed) {
            Ok(_) => Bytes::from(decompressed),
            Err(e) => {
                tracing::debug!("[WS] Failed to decompress frame payload: {}", e);
                self.payload.clone()
            }
        }
    }

    pub fn is_compressed(&self) -> bool {
        self.rsv1
    }
}

pub fn parse_permessage_deflate(extensions: &str) -> bool {
    extensions
        .split(',')
        .any(|ext| ext.trim().starts_with("permessage-deflate"))
}

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

const DEFAULT_MAX_FRAGMENT_BUFFER_SIZE: usize = 16 * 1024 * 1024;

pin_project! {
    pub struct WebSocketReader<R> {
        #[pin]
        inner: R,
        buffer: BytesMut,
        fragment_buffer: Vec<u8>,
        fragment_opcode: Option<Opcode>,
        max_fragment_size: usize,
    }
}

impl<R> WebSocketReader<R> {
    pub fn new(inner: R) -> Self {
        Self::with_max_fragment_size(inner, DEFAULT_MAX_FRAGMENT_BUFFER_SIZE)
    }

    pub fn with_max_fragment_size(inner: R, max_fragment_size: usize) -> Self {
        Self {
            inner,
            buffer: BytesMut::with_capacity(8192),
            fragment_buffer: Vec::new(),
            fragment_opcode: None,
            max_fragment_size,
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: AsyncRead + Unpin> Stream for WebSocketReader<R> {
    type Item = std::io::Result<WebSocketFrame>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            if let Some((frame, consumed)) = WebSocketFrame::parse(this.buffer) {
                this.buffer.advance(consumed);

                if frame.opcode.is_control() {
                    return Poll::Ready(Some(Ok(frame)));
                }

                if frame.opcode == Opcode::Continuation {
                    let new_size = this.fragment_buffer.len() + frame.payload.len();
                    if new_size > *this.max_fragment_size {
                        tracing::warn!(
                            "[WS] Fragment buffer overflow: {} bytes exceeds limit of {} bytes, dropping fragments",
                            new_size,
                            *this.max_fragment_size
                        );
                        this.fragment_buffer.clear();
                        *this.fragment_opcode = None;
                        continue;
                    }
                    this.fragment_buffer.extend_from_slice(&frame.payload);
                    if frame.fin {
                        let opcode = this.fragment_opcode.take().unwrap_or(Opcode::Text);
                        let complete_frame = WebSocketFrame {
                            fin: true,
                            rsv1: false,
                            opcode,
                            mask: None,
                            payload: Bytes::from(std::mem::take(this.fragment_buffer)),
                        };
                        return Poll::Ready(Some(Ok(complete_frame)));
                    }
                } else if !frame.fin {
                    let new_size = frame.payload.len();
                    if new_size > *this.max_fragment_size {
                        tracing::warn!(
                            "[WS] Initial fragment too large: {} bytes exceeds limit of {} bytes",
                            new_size,
                            *this.max_fragment_size
                        );
                        continue;
                    }
                    *this.fragment_opcode = Some(frame.opcode);
                    this.fragment_buffer.clear();
                    this.fragment_buffer.extend_from_slice(&frame.payload);
                } else {
                    return Poll::Ready(Some(Ok(frame)));
                }
            }

            let mut buf = [0u8; 8192];
            let mut read_buf = ReadBuf::new(&mut buf);

            match this.inner.as_mut().poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    let n = read_buf.filled().len();
                    if n == 0 {
                        return Poll::Ready(None);
                    }
                    this.buffer.extend_from_slice(read_buf.filled());
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub struct WebSocketWriter<W> {
    inner: W,
    is_client: bool,
}

impl<W> WebSocketWriter<W> {
    pub fn new(inner: W, is_client: bool) -> Self {
        Self { inner, is_client }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: AsyncWrite + Unpin> WebSocketWriter<W> {
    pub async fn write_frame(&mut self, mut frame: WebSocketFrame) -> std::io::Result<()> {
        if self.is_client && frame.mask.is_none() {
            frame = frame.with_mask(generate_mask());
        }
        let encoded = frame.encode();
        self.inner.write_all(&encoded).await?;
        self.inner.flush().await?;
        Ok(())
    }

    pub async fn write_text(&mut self, text: &str) -> std::io::Result<()> {
        self.write_frame(WebSocketFrame::text(text)).await
    }

    pub async fn write_binary(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.write_frame(WebSocketFrame::binary(data)).await
    }

    pub async fn write_ping(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.write_frame(WebSocketFrame::ping(data)).await
    }

    pub async fn write_pong(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.write_frame(WebSocketFrame::pong(data)).await
    }

    pub async fn write_close(&mut self, code: Option<u16>, reason: &str) -> std::io::Result<()> {
        self.write_frame(WebSocketFrame::close(code, reason)).await
    }
}

fn generate_mask() -> [u8; 4] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32;
    seed.to_be_bytes()
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

pub struct WebSocketForwarder;

pub type WebSocketFrameCallback =
    Box<dyn Fn(&WebSocketFrame) -> Option<WebSocketFrame> + Send + Sync>;

impl WebSocketForwarder {
    pub async fn bidirectional<R1, W1, R2, W2>(
        mut client_reader: R1,
        mut client_writer: W1,
        mut server_reader: R2,
        mut server_writer: W2,
        on_client_frame: Option<WebSocketFrameCallback>,
        on_server_frame: Option<WebSocketFrameCallback>,
    ) -> std::io::Result<(u64, u64)>
    where
        R1: AsyncRead + Unpin + Send + 'static,
        W1: AsyncWrite + Unpin + Send + 'static,
        R2: AsyncRead + Unpin + Send + 'static,
        W2: AsyncWrite + Unpin + Send + 'static,
    {
        use futures_util::StreamExt;

        let client_to_server = async move {
            let mut reader = WebSocketReader::new(&mut client_reader);
            let mut writer = WebSocketWriter::new(&mut server_writer, true);
            let mut count = 0u64;

            while let Some(result) = reader.next().await {
                let frame = result?;

                let frame_to_write = if let Some(ref transform) = on_client_frame {
                    transform(&frame)
                } else {
                    Some(frame)
                };

                if let Some(f) = frame_to_write {
                    writer.write_frame(f).await?;
                    count += 1;
                }
            }

            Ok::<_, std::io::Error>(count)
        };

        let server_to_client = async move {
            let mut reader = WebSocketReader::new(&mut server_reader);
            let mut writer = WebSocketWriter::new(&mut client_writer, false);
            let mut count = 0u64;

            while let Some(result) = reader.next().await {
                let frame = result?;

                let frame_to_write = if let Some(ref transform) = on_server_frame {
                    transform(&frame)
                } else {
                    Some(frame)
                };

                if let Some(f) = frame_to_write {
                    writer.write_frame(f).await?;
                    count += 1;
                }
            }

            Ok::<_, std::io::Error>(count)
        };

        let (r1, r2) = tokio::try_join!(client_to_server, server_to_client)?;
        Ok((r1, r2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_from_u8() {
        assert_eq!(Opcode::from_u8(0x0), Some(Opcode::Continuation));
        assert_eq!(Opcode::from_u8(0x1), Some(Opcode::Text));
        assert_eq!(Opcode::from_u8(0x2), Some(Opcode::Binary));
        assert_eq!(Opcode::from_u8(0x8), Some(Opcode::Close));
        assert_eq!(Opcode::from_u8(0x9), Some(Opcode::Ping));
        assert_eq!(Opcode::from_u8(0xA), Some(Opcode::Pong));
        assert_eq!(Opcode::from_u8(0x3), None);
    }

    #[test]
    fn test_opcode_is_control() {
        assert!(!Opcode::Text.is_control());
        assert!(!Opcode::Binary.is_control());
        assert!(!Opcode::Continuation.is_control());
        assert!(Opcode::Close.is_control());
        assert!(Opcode::Ping.is_control());
        assert!(Opcode::Pong.is_control());
    }

    #[test]
    fn test_frame_encode_decode() {
        let frame = WebSocketFrame::text("Hello, World!");
        let encoded = frame.encode();
        let (decoded, consumed) = WebSocketFrame::parse(&encoded).unwrap();

        assert!(decoded.fin);
        assert_eq!(decoded.opcode, Opcode::Text);
        assert_eq!(decoded.payload.as_ref(), b"Hello, World!");
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_frame_encode_decode_with_mask() {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let frame = WebSocketFrame::text("Hello").with_mask(mask);
        let encoded = frame.encode();
        let (decoded, consumed) = WebSocketFrame::parse(&encoded).unwrap();

        assert_eq!(decoded.payload.as_ref(), b"Hello");
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_frame_encode_large_payload() {
        let data = vec![0u8; 1000];
        let frame = WebSocketFrame::binary(&data);
        let encoded = frame.encode();
        let (decoded, _) = WebSocketFrame::parse(&encoded).unwrap();

        assert_eq!(decoded.payload.len(), 1000);
    }

    #[test]
    fn test_frame_close() {
        let frame = WebSocketFrame::close(Some(1000), "Normal closure");
        assert_eq!(frame.close_code(), Some(1000));
        assert_eq!(frame.close_reason(), Some("Normal closure"));
    }

    #[test]
    fn test_frame_ping_pong() {
        let ping = WebSocketFrame::ping(b"test");
        assert_eq!(ping.opcode, Opcode::Ping);
        assert_eq!(ping.payload.as_ref(), b"test");

        let pong = WebSocketFrame::pong(b"test");
        assert_eq!(pong.opcode, Opcode::Pong);
        assert_eq!(pong.payload.as_ref(), b"test");
    }

    #[test]
    fn test_compute_accept_key() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_accept_key(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn test_generate_sec_websocket_key() {
        let key = generate_sec_websocket_key();
        assert!(!key.is_empty());
    }

    #[test]
    fn test_build_websocket_request_headers() {
        let headers =
            build_websocket_request_headers("example.com", "/ws", "dGVzdA==", Some(&["chat"]));

        assert!(headers.contains("GET /ws HTTP/1.1"));
        assert!(headers.contains("Host: example.com"));
        assert!(headers.contains("Upgrade: websocket"));
        assert!(headers.contains("Connection: Upgrade"));
        assert!(headers.contains("Sec-WebSocket-Key: dGVzdA=="));
        assert!(headers.contains("Sec-WebSocket-Version: 13"));
        assert!(headers.contains("Sec-WebSocket-Protocol: chat"));
    }

    #[test]
    fn test_build_websocket_response_headers() {
        let headers = build_websocket_response_headers("dGVzdA==", Some("chat"));

        assert!(headers.contains("HTTP/1.1 101 Switching Protocols"));
        assert!(headers.contains("Upgrade: websocket"));
        assert!(headers.contains("Connection: Upgrade"));
        assert!(headers.contains("Sec-WebSocket-Accept:"));
        assert!(headers.contains("Sec-WebSocket-Protocol: chat"));
    }

    #[test]
    fn test_frame_parse_incomplete() {
        let data = [0x81]; // Only first byte
        assert!(WebSocketFrame::parse(&data).is_none());
    }

    #[test]
    fn test_frame_parse_incomplete_payload() {
        let data = [0x81, 0x05, 0x48]; // Text frame with 5 bytes, but only 1 byte payload
        assert!(WebSocketFrame::parse(&data).is_none());
    }

    #[test]
    fn test_frame_rsv1_parsing() {
        let mut buf = BytesMut::new();
        buf.put_u8(0xC1);
        buf.put_u8(0x05);
        buf.extend_from_slice(b"hello");

        let (frame, _) = WebSocketFrame::parse(&buf).unwrap();
        assert!(frame.fin);
        assert!(frame.rsv1);
        assert_eq!(frame.opcode, Opcode::Text);
        assert!(frame.is_compressed());
    }

    #[test]
    fn test_frame_rsv1_encoding() {
        let frame = WebSocketFrame {
            fin: true,
            rsv1: true,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from("hello"),
        };
        let encoded = frame.encode();
        assert_eq!(encoded[0], 0xC1);
    }

    #[test]
    fn test_parse_permessage_deflate() {
        assert!(parse_permessage_deflate("permessage-deflate"));
        assert!(parse_permessage_deflate(
            "permessage-deflate; client_max_window_bits"
        ));
        assert!(parse_permessage_deflate(
            "permessage-deflate; server_no_context_takeover"
        ));
        assert!(!parse_permessage_deflate("x-webkit-deflate-frame"));
        assert!(!parse_permessage_deflate(""));
    }

    #[test]
    fn test_extract_sec_websocket_extensions() {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\
                        Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\
                        Sec-WebSocket-Extensions: permessage-deflate; server_no_context_takeover\r\n\r\n";
        let ext = extract_sec_websocket_extensions(response);
        assert!(ext.is_some());
        assert!(ext.unwrap().contains("permessage-deflate"));
    }

    #[test]
    fn test_extract_sec_websocket_extensions_missing() {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\r\n";
        let ext = extract_sec_websocket_extensions(response);
        assert!(ext.is_none());
    }

    #[test]
    fn test_decompress_payload_uncompressed() {
        let frame = WebSocketFrame {
            fin: true,
            rsv1: false,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from("hello"),
        };
        let decompressed = frame.decompress_payload();
        assert_eq!(decompressed.as_ref(), b"hello");
    }

    #[test]
    fn test_decompress_payload_compressed() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = b"hello world hello world";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let frame = WebSocketFrame {
            fin: true,
            rsv1: true,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from(compressed),
        };

        let decompressed = frame.decompress_payload();
        assert_eq!(decompressed.as_ref(), original);
    }

    #[test]
    fn test_decompress_payload_rfc7692_format() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = b"Hello";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let mut compressed = encoder.finish().unwrap();

        if compressed.ends_with(&[0x00, 0x00, 0xff, 0xff]) {
            compressed.truncate(compressed.len() - 4);
        }

        let frame = WebSocketFrame {
            fin: true,
            rsv1: true,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from(compressed),
        };

        let decompressed = frame.decompress_payload();
        assert_eq!(decompressed.as_ref(), original);
    }
}
