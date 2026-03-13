use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes, BytesMut};
use futures_util::Stream;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};

use super::{Opcode, WebSocketFrame};

const DEFAULT_MAX_FRAGMENT_BUFFER_SIZE: usize = 16 * 1024 * 1024;

pin_project! {
    pub struct WebSocketReader<R> {
        #[pin]
        inner: R,
        buffer: BytesMut,
        fragment_buffer: Vec<u8>,
        fragment_opcode: Option<Opcode>,
        fragment_rsv1: bool,
        fragment_rsv2: bool,
        fragment_rsv3: bool,
        max_fragment_size: usize,
    }
}

impl<R> WebSocketReader<R> {
    pub fn new(inner: R) -> Self {
        Self::with_max_fragment_size(inner, DEFAULT_MAX_FRAGMENT_BUFFER_SIZE)
    }

    pub fn with_initial_buffer(inner: R, buffer: BytesMut) -> Self {
        Self {
            inner,
            buffer,
            fragment_buffer: Vec::new(),
            fragment_opcode: None,
            fragment_rsv1: false,
            fragment_rsv2: false,
            fragment_rsv3: false,
            max_fragment_size: DEFAULT_MAX_FRAGMENT_BUFFER_SIZE,
        }
    }

    pub fn with_max_fragment_size(inner: R, max_fragment_size: usize) -> Self {
        Self {
            inner,
            buffer: BytesMut::with_capacity(8192),
            fragment_buffer: Vec::new(),
            fragment_opcode: None,
            fragment_rsv1: false,
            fragment_rsv2: false,
            fragment_rsv3: false,
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
                        *this.fragment_rsv1 = false;
                        *this.fragment_rsv2 = false;
                        *this.fragment_rsv3 = false;
                        continue;
                    }
                    this.fragment_buffer.extend_from_slice(&frame.payload);
                    if frame.fin {
                        let opcode = this.fragment_opcode.take().unwrap_or(Opcode::Text);
                        let complete_frame = WebSocketFrame {
                            fin: true,
                            rsv1: *this.fragment_rsv1,
                            rsv2: *this.fragment_rsv2,
                            rsv3: *this.fragment_rsv3,
                            opcode,
                            mask: None,
                            payload: Bytes::from(std::mem::take(this.fragment_buffer)),
                        };
                        *this.fragment_rsv1 = false;
                        *this.fragment_rsv2 = false;
                        *this.fragment_rsv3 = false;
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
                        this.fragment_buffer.clear();
                        *this.fragment_opcode = None;
                        *this.fragment_rsv1 = false;
                        *this.fragment_rsv2 = false;
                        *this.fragment_rsv3 = false;
                        continue;
                    }
                    *this.fragment_opcode = Some(frame.opcode);
                    *this.fragment_rsv1 = frame.rsv1;
                    *this.fragment_rsv2 = frame.rsv2;
                    *this.fragment_rsv3 = frame.rsv3;
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
