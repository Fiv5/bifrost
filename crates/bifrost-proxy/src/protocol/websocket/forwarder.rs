use tokio::io::{AsyncRead, AsyncWrite};

use super::{WebSocketFrame, WebSocketReader, WebSocketWriter};

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
