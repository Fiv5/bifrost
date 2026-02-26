use bytes::{Buf, Bytes, BytesMut};
use h3::quic::BidiStream;
use h3::server::RequestStream;

use crate::protocol::{SseEvent, WebSocketFrame};

pub struct H3StreamAdapter<S> {
    stream: RequestStream<S, Bytes>,
}

impl<S: BidiStream<Bytes> + Send + 'static> H3StreamAdapter<S> {
    pub fn new(stream: RequestStream<S, Bytes>) -> Self {
        Self { stream }
    }

    pub fn into_inner(self) -> RequestStream<S, Bytes> {
        self.stream
    }

    pub fn stream_mut(&mut self) -> &mut RequestStream<S, Bytes> {
        &mut self.stream
    }

    pub async fn recv_all_data(&mut self) -> std::io::Result<Bytes> {
        let mut data = BytesMut::new();
        loop {
            match self.stream.recv_data().await {
                Ok(Some(mut chunk)) => {
                    while chunk.has_remaining() {
                        let bytes = chunk.chunk();
                        data.extend_from_slice(bytes);
                        chunk.advance(bytes.len());
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return Err(std::io::Error::other(format!("H3 recv error: {}", e)));
                }
            }
        }
        Ok(data.freeze())
    }

    pub async fn recv_data(&mut self) -> std::io::Result<Option<Bytes>> {
        match self.stream.recv_data().await {
            Ok(Some(mut chunk)) => {
                let mut data = BytesMut::new();
                while chunk.has_remaining() {
                    let bytes = chunk.chunk();
                    data.extend_from_slice(bytes);
                    chunk.advance(bytes.len());
                }
                Ok(Some(data.freeze()))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(std::io::Error::other(format!("H3 recv error: {}", e))),
        }
    }

    pub async fn send_data(&mut self, data: Bytes) -> std::io::Result<()> {
        self.stream
            .send_data(data)
            .await
            .map_err(|e| std::io::Error::other(format!("H3 send error: {}", e)))
    }

    pub async fn finish(&mut self) -> std::io::Result<()> {
        self.stream
            .finish()
            .await
            .map_err(|e| std::io::Error::other(format!("H3 finish error: {}", e)))
    }
}

pub struct H3WebSocketStream<S> {
    inner: H3StreamAdapter<S>,
    recv_buffer: BytesMut,
}

impl<S: BidiStream<Bytes> + Send + 'static> H3WebSocketStream<S> {
    pub fn new(stream: RequestStream<S, Bytes>) -> Self {
        Self {
            inner: H3StreamAdapter::new(stream),
            recv_buffer: BytesMut::with_capacity(8192),
        }
    }

    pub async fn recv_frame(&mut self) -> std::io::Result<Option<WebSocketFrame>> {
        loop {
            if let Some((frame, consumed)) = WebSocketFrame::parse(&self.recv_buffer) {
                let _ = self.recv_buffer.split_to(consumed);
                return Ok(Some(frame));
            }

            match self.inner.recv_data().await? {
                Some(data) => {
                    self.recv_buffer.extend_from_slice(&data);
                }
                None => {
                    return Ok(None);
                }
            }
        }
    }

    pub async fn send_frame(&mut self, frame: WebSocketFrame) -> std::io::Result<()> {
        let encoded = frame.encode();
        self.inner.send_data(encoded).await
    }

    pub async fn send_text(&mut self, text: &str) -> std::io::Result<()> {
        self.send_frame(WebSocketFrame::text(text)).await
    }

    pub async fn send_binary(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.send_frame(WebSocketFrame::binary(data)).await
    }

    pub async fn send_ping(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.send_frame(WebSocketFrame::ping(data)).await
    }

    pub async fn send_pong(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.send_frame(WebSocketFrame::pong(data)).await
    }

    pub async fn close(&mut self, code: Option<u16>, reason: &str) -> std::io::Result<()> {
        self.send_frame(WebSocketFrame::close(code, reason)).await?;
        self.inner.finish().await
    }
}

pub struct H3SseStream<S> {
    inner: H3StreamAdapter<S>,
    buffer: BytesMut,
    last_event_id: Option<String>,
}

impl<S: BidiStream<Bytes> + Send + 'static> H3SseStream<S> {
    pub fn new(stream: RequestStream<S, Bytes>) -> Self {
        Self {
            inner: H3StreamAdapter::new(stream),
            buffer: BytesMut::with_capacity(8192),
            last_event_id: None,
        }
    }

    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    pub async fn recv_event(&mut self) -> std::io::Result<Option<SseEvent>> {
        loop {
            if let Some(event) = self.try_parse_event() {
                return Ok(Some(event));
            }

            match self.inner.recv_data().await? {
                Some(data) => {
                    self.buffer.extend_from_slice(&data);
                }
                None => {
                    return Ok(None);
                }
            }
        }
    }

    fn try_parse_event(&mut self) -> Option<SseEvent> {
        let boundary = (0..self.buffer.len().saturating_sub(1))
            .find(|&i| self.buffer[i] == b'\n' && self.buffer[i + 1] == b'\n')?;

        let event_data = self.buffer.split_to(boundary + 2);
        let event_str = String::from_utf8_lossy(&event_data);

        if let Some(event) = SseEvent::parse(&event_str) {
            if let Some(ref id) = event.id {
                self.last_event_id = Some(id.clone());
            }
            Some(event)
        } else {
            None
        }
    }

    pub async fn send_event(&mut self, event: &SseEvent) -> std::io::Result<()> {
        let encoded = event.encode();
        self.inner.send_data(encoded).await
    }

    pub async fn send_comment(&mut self, comment: &str) -> std::io::Result<()> {
        let mut buf = BytesMut::new();
        for line in comment.lines() {
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(line.as_bytes());
            buf.extend_from_slice(b"\n");
        }
        self.inner.send_data(buf.freeze()).await
    }

    pub async fn send_keepalive(&mut self) -> std::io::Result<()> {
        self.inner
            .send_data(Bytes::from_static(b": keepalive\n\n"))
            .await
    }
}

pub struct H3StreamingResponse<S> {
    inner: H3StreamAdapter<S>,
    is_chunked: bool,
}

impl<S: BidiStream<Bytes> + Send + 'static> H3StreamingResponse<S> {
    pub fn new(stream: RequestStream<S, Bytes>, is_chunked: bool) -> Self {
        Self {
            inner: H3StreamAdapter::new(stream),
            is_chunked,
        }
    }

    pub async fn send_chunk(&mut self, data: &[u8]) -> std::io::Result<()> {
        let chunk = if self.is_chunked {
            crate::protocol::encode_chunk(data)
        } else {
            Bytes::copy_from_slice(data)
        };

        self.inner.send_data(chunk).await
    }

    pub async fn finish(&mut self) -> std::io::Result<()> {
        if self.is_chunked {
            self.inner
                .send_data(crate::protocol::encode_final_chunk())
                .await?;
        }

        self.inner.finish().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_parsing() {
        let raw = "id: 123\nevent: message\ndata: hello world";
        let event = SseEvent::parse(raw).unwrap();

        assert_eq!(event.id, Some("123".to_string()));
        assert_eq!(event.event, Some("message".to_string()));
        assert_eq!(event.data, "hello world");
    }
}
