use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use futures_util::Stream;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub const DEFAULT_BUFFER_SIZE: usize = 8192;
pub const MAX_BUFFER_SIZE: usize = 65536;

pin_project! {
    pub struct ChunkedReader<R> {
        #[pin]
        inner: R,
        buffer: BytesMut,
        chunk_remaining: usize,
        state: ChunkedState,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkedState {
    ReadingSize,
    ReadingData,
    ReadingTrailer,
    Done,
}

impl<R> ChunkedReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: BytesMut::with_capacity(DEFAULT_BUFFER_SIZE),
            chunk_remaining: 0,
            state: ChunkedState::ReadingSize,
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: AsyncRead + Unpin> Stream for ChunkedReader<R> {
    type Item = std::io::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            match *this.state {
                ChunkedState::Done => return Poll::Ready(None),

                ChunkedState::ReadingSize => {
                    if let Some(pos) = this.buffer.iter().position(|&b| b == b'\n') {
                        let line = this.buffer.split_to(pos + 1);
                        let size_str = std::str::from_utf8(&line)
                            .unwrap_or("")
                            .trim()
                            .split(';')
                            .next()
                            .unwrap_or("");

                        match usize::from_str_radix(size_str, 16) {
                            Ok(0) => {
                                *this.state = ChunkedState::ReadingTrailer;
                            }
                            Ok(size) => {
                                *this.chunk_remaining = size;
                                *this.state = ChunkedState::ReadingData;
                            }
                            Err(_) => {
                                return Poll::Ready(Some(Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "Invalid chunk size",
                                ))));
                            }
                        }
                    } else {
                        let mut buf = [0u8; 256];
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

                ChunkedState::ReadingData => {
                    if *this.chunk_remaining == 0 {
                        if this.buffer.len() >= 2 && &this.buffer[..2] == b"\r\n" {
                            let _ = this.buffer.split_to(2);
                        }
                        *this.state = ChunkedState::ReadingSize;
                        continue;
                    }

                    if !this.buffer.is_empty() {
                        let to_take = (*this.chunk_remaining).min(this.buffer.len());
                        let chunk = this.buffer.split_to(to_take);
                        *this.chunk_remaining -= to_take;
                        return Poll::Ready(Some(Ok(chunk.freeze())));
                    }

                    let mut buf = vec![0u8; (*this.chunk_remaining).min(MAX_BUFFER_SIZE)];
                    let mut read_buf = ReadBuf::new(&mut buf);
                    match this.inner.as_mut().poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            let n = read_buf.filled().len();
                            if n == 0 {
                                return Poll::Ready(Some(Err(std::io::Error::new(
                                    std::io::ErrorKind::UnexpectedEof,
                                    "Unexpected EOF in chunk data",
                                ))));
                            }
                            *this.chunk_remaining -= n;
                            return Poll::Ready(Some(Ok(Bytes::copy_from_slice(
                                read_buf.filled(),
                            ))));
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                        Poll::Pending => return Poll::Pending,
                    }
                }

                ChunkedState::ReadingTrailer => {
                    if let Some(pos) = this.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                        let _ = this.buffer.split_to(pos + 4);
                        *this.state = ChunkedState::Done;
                        return Poll::Ready(None);
                    }
                    if this.buffer.len() >= 2 && &this.buffer[..2] == b"\r\n" {
                        let _ = this.buffer.split_to(2);
                        *this.state = ChunkedState::Done;
                        return Poll::Ready(None);
                    }

                    let mut buf = [0u8; 256];
                    let mut read_buf = ReadBuf::new(&mut buf);
                    match this.inner.as_mut().poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            let n = read_buf.filled().len();
                            if n == 0 {
                                *this.state = ChunkedState::Done;
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
    }
}

pin_project! {
    pub struct ChunkedWriter<W> {
        #[pin]
        inner: W,
    }
}

impl<W> ChunkedWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: AsyncWrite + Unpin> ChunkedWriter<W> {
    pub async fn write_chunk(&mut self, data: &[u8]) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;

        let size_line = format!("{:x}\r\n", data.len());
        self.inner.write_all(size_line.as_bytes()).await?;
        self.inner.write_all(data).await?;
        self.inner.write_all(b"\r\n").await?;
        self.inner.flush().await?;
        Ok(())
    }

    pub async fn finish(&mut self) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;

        self.inner.write_all(b"0\r\n\r\n").await?;
        self.inner.flush().await?;
        Ok(())
    }
}

pub fn encode_chunk(data: &[u8]) -> Bytes {
    let size_line = format!("{:x}\r\n", data.len());
    let mut result = BytesMut::with_capacity(size_line.len() + data.len() + 2);
    result.extend_from_slice(size_line.as_bytes());
    result.extend_from_slice(data);
    result.extend_from_slice(b"\r\n");
    result.freeze()
}

pub fn encode_final_chunk() -> Bytes {
    Bytes::from_static(b"0\r\n\r\n")
}

pin_project! {
    pub struct LengthLimitedReader<R> {
        #[pin]
        inner: R,
        remaining: usize,
    }
}

impl<R> LengthLimitedReader<R> {
    pub fn new(inner: R, length: usize) -> Self {
        Self {
            inner,
            remaining: length,
        }
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: AsyncRead> AsyncRead for LengthLimitedReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();

        if *this.remaining == 0 {
            return Poll::Ready(Ok(()));
        }

        let max_read = (*this.remaining).min(buf.remaining());
        let mut limited_buf = buf.take(max_read);

        match this.inner.poll_read(cx, &mut limited_buf) {
            Poll::Ready(Ok(())) => {
                let n = limited_buf.filled().len();
                *this.remaining -= n;
                unsafe {
                    buf.assume_init(n);
                }
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct StreamForwarder;

impl StreamForwarder {
    pub async fn bidirectional<S1, S2>(
        stream1: S1,
        stream2: S2,
        buffer_size: usize,
    ) -> std::io::Result<(u64, u64)>
    where
        S1: AsyncRead + AsyncWrite + Unpin,
        S2: AsyncRead + AsyncWrite + Unpin,
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut read1, mut write1) = tokio::io::split(stream1);
        let (mut read2, mut write2) = tokio::io::split(stream2);

        let forward1 = async {
            let mut buf = vec![0u8; buffer_size];
            let mut total = 0u64;
            loop {
                let n = read1.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                write2.write_all(&buf[..n]).await?;
                total += n as u64;
            }
            write2.shutdown().await?;
            Ok::<_, std::io::Error>(total)
        };

        let forward2 = async {
            let mut buf = vec![0u8; buffer_size];
            let mut total = 0u64;
            loop {
                let n = read2.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                write1.write_all(&buf[..n]).await?;
                total += n as u64;
            }
            Ok::<_, std::io::Error>(total)
        };

        let (r1, r2) = tokio::try_join!(forward1, forward2)?;
        Ok((r1, r2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_chunk() {
        let data = b"Hello, World!";
        let chunk = encode_chunk(data);
        assert_eq!(chunk.as_ref(), b"d\r\nHello, World!\r\n");
    }

    #[test]
    fn test_encode_chunk_empty() {
        let data = b"";
        let chunk = encode_chunk(data);
        assert_eq!(chunk.as_ref(), b"0\r\n\r\n");
    }

    #[test]
    fn test_encode_final_chunk() {
        let chunk = encode_final_chunk();
        assert_eq!(chunk.as_ref(), b"0\r\n\r\n");
    }

    #[tokio::test]
    async fn test_chunked_reader() {
        use futures_util::StreamExt;

        let chunked_data = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        let cursor = std::io::Cursor::new(chunked_data.to_vec());
        let tokio_cursor = tokio_util::compat::FuturesAsyncReadCompatExt::compat(
            futures_util::io::AllowStdIo::new(cursor),
        );

        let mut reader = ChunkedReader::new(tokio_cursor);
        let mut result = Vec::new();

        while let Some(chunk) = reader.next().await {
            result.extend_from_slice(&chunk.unwrap());
        }

        assert_eq!(result, b"hello world");
    }

    #[test]
    fn test_length_limited_reader() {
        let _reader = LengthLimitedReader::new(std::io::Cursor::new(vec![0u8; 100]), 50);
    }
}
