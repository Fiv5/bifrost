use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::{Body, Frame};
use tokio::time::Sleep;

use crate::server::BoxBody;

pub struct ThrottledBoxBody {
    inner: BoxBody,
    bytes_per_second: u64,
    bytes_sent_this_window: u64,
    window_start: Instant,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl ThrottledBoxBody {
    pub fn new(inner: BoxBody, bytes_per_second: u64) -> Self {
        Self {
            inner,
            bytes_per_second,
            bytes_sent_this_window: 0,
            window_start: Instant::now(),
            sleep: None,
        }
    }
}

impl Body for ThrottledBoxBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if let Some(ref mut sleep) = self.sleep {
            match sleep.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    self.sleep = None;
                    self.bytes_sent_this_window = 0;
                    self.window_start = Instant::now();
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let len = data.len() as u64;
                    self.bytes_sent_this_window += len;

                    if self.bytes_sent_this_window >= self.bytes_per_second {
                        let elapsed = self.window_start.elapsed();
                        if elapsed < Duration::from_secs(1) {
                            let sleep_duration = Duration::from_secs(1) - elapsed;
                            self.sleep = Some(Box::pin(tokio::time::sleep(sleep_duration)));
                        }
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}

pub fn wrap_throttled_body(body: BoxBody, bytes_per_second: Option<u64>) -> BoxBody {
    match bytes_per_second {
        Some(speed) if speed > 0 => BodyExt::boxed(ThrottledBoxBody::new(body, speed)),
        _ => body,
    }
}
