use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::{Body, Frame};

pub enum BoundedBody<B> {
    Complete(Bytes),
    Exceeded(PrefixReplayBody<B>),
}

pub struct PrefixReplayBody<B> {
    prefix: VecDeque<Frame<Bytes>>,
    inner: Pin<Box<B>>,
}

impl<B> PrefixReplayBody<B> {
    pub fn new(prefix: VecDeque<Frame<Bytes>>, inner: B) -> Self {
        Self {
            prefix,
            inner: Box::pin(inner),
        }
    }

    pub fn boxed(self) -> crate::server::BoxBody
    where
        B: Body<Data = Bytes, Error = hyper::Error> + Send + Sync + 'static,
    {
        http_body_util::BodyExt::boxed(self)
    }
}

impl<B> Body for PrefixReplayBody<B>
where
    B: Body<Data = Bytes, Error = hyper::Error>,
{
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if let Some(frame) = self.prefix.pop_front() {
            return Poll::Ready(Some(Ok(frame)));
        }
        self.inner.as_mut().poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.prefix.is_empty() && self.inner.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}

pub async fn read_body_bounded<B>(
    mut body: B,
    max_bytes: usize,
) -> Result<BoundedBody<B>, hyper::Error>
where
    B: Body<Data = Bytes, Error = hyper::Error> + Unpin + Send + Sync + 'static,
{
    let mut frames: VecDeque<Frame<Bytes>> = VecDeque::new();
    let mut seen: usize = 0;

    while let Some(frame) = body.frame().await {
        let frame = frame?;
        if let Some(data) = frame.data_ref() {
            seen = seen.saturating_add(data.len());
        }
        frames.push_back(frame);
        if seen > max_bytes {
            return Ok(BoundedBody::Exceeded(PrefixReplayBody::new(frames, body)));
        }
    }

    let mut buf = BytesMut::with_capacity(seen);
    for frame in &frames {
        if let Some(data) = frame.data_ref() {
            buf.extend_from_slice(data);
        }
    }
    Ok(BoundedBody::Complete(buf.freeze()))
}
