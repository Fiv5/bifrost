use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bifrost_admin::AdminState;
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, Incoming};

use crate::server::BoxBody;

struct TeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
}

impl Drop for TeeBodyDropGuard {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(ref state) = self.admin_state {
                state
                    .traffic_recorder
                    .update_by_id(&self.record_id, |record| {
                        record.response_size = self.total_bytes;
                    });
            }
        }
    }
}

pub struct TeeBody {
    inner: Incoming,
    guard: TeeBodyDropGuard,
}

impl TeeBody {
    pub fn new(inner: Incoming, admin_state: Option<Arc<AdminState>>, record_id: String) -> Self {
        Self {
            inner,
            guard: TeeBodyDropGuard {
                admin_state,
                record_id,
                total_bytes: 0,
                finished: false,
            },
        }
    }

    pub fn boxed(self) -> BoxBody {
        BodyExt::boxed(self)
    }
}

impl Body for TeeBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.guard.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let len = data.len();
                    self.guard.total_bytes += len;

                    if let Some(ref state) = self.guard.admin_state {
                        state.metrics_collector.add_bytes_received(len as u64);
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.guard.finished = true;
                if let Some(ref state) = self.guard.admin_state {
                    state
                        .traffic_recorder
                        .update_by_id(&self.guard.record_id, |record| {
                            record.response_size = self.guard.total_bytes;
                        });
                }
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                self.guard.finished = true;
                if let Some(ref state) = self.guard.admin_state {
                    state
                        .traffic_recorder
                        .update_by_id(&self.guard.record_id, |record| {
                            record.response_size = self.guard.total_bytes;
                        });
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.guard.finished || self.inner.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}

pub fn create_tee_body_with_store(
    body: Incoming,
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
) -> TeeBody {
    TeeBody::new(body, admin_state, record_id)
}

struct SseTeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
}

impl Drop for SseTeeBodyDropGuard {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(ref state) = self.admin_state {
                state
                    .traffic_recorder
                    .update_by_id(&self.record_id, |record| {
                        record.response_size = self.total_bytes;
                    });
            }
        }
    }
}

pub struct SseTeeBody {
    inner: Incoming,
    guard: SseTeeBodyDropGuard,
    buffer: BytesMut,
}

impl SseTeeBody {
    pub fn new(inner: Incoming, admin_state: Option<Arc<AdminState>>, record_id: String) -> Self {
        if let Some(ref state) = admin_state {
            state.websocket_monitor.register_connection(&record_id);
        }

        Self {
            inner,
            guard: SseTeeBodyDropGuard {
                admin_state,
                record_id,
                total_bytes: 0,
                finished: false,
            },
            buffer: BytesMut::with_capacity(4096),
        }
    }

    pub fn boxed(self) -> BoxBody {
        BodyExt::boxed(self)
    }

    fn process_sse_events(&mut self) {
        while let Some(pos) = self.find_event_boundary() {
            let event_data = self.buffer.split_to(pos + 2);
            let event_bytes = &event_data[..event_data.len() - 2];

            if !event_bytes.is_empty() {
                if let Some(ref state) = self.guard.admin_state {
                    tracing::debug!(
                        "[SSE] Recording event for {}, bytes: {}",
                        self.guard.record_id,
                        event_bytes.len()
                    );
                    state.websocket_monitor.record_sse_event(
                        &self.guard.record_id,
                        event_bytes,
                        state.body_store.as_ref(),
                    );
                } else {
                    tracing::warn!("[SSE] No admin state for recording event");
                }
            }
        }
    }

    fn find_event_boundary(&self) -> Option<usize> {
        let bytes = &self.buffer[..];
        (0..bytes.len().saturating_sub(1)).find(|&i| bytes[i] == b'\n' && bytes[i + 1] == b'\n')
    }
}

impl Body for SseTeeBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.guard.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let len = data.len();
                    self.guard.total_bytes += len;

                    if let Some(ref state) = self.guard.admin_state {
                        state.metrics_collector.add_bytes_received(len as u64);
                    }

                    self.buffer.extend_from_slice(data);
                    self.process_sse_events();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.guard.finished = true;
                if let Some(ref state) = self.guard.admin_state {
                    state
                        .traffic_recorder
                        .update_by_id(&self.guard.record_id, |record| {
                            record.response_size = self.guard.total_bytes;
                        });
                }
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                self.guard.finished = true;
                if !self.buffer.is_empty() {
                    if let Some(ref state) = self.guard.admin_state {
                        state.websocket_monitor.record_sse_event(
                            &self.guard.record_id,
                            &self.buffer,
                            state.body_store.as_ref(),
                        );
                    }
                    self.buffer.clear();
                }
                if let Some(ref state) = self.guard.admin_state {
                    state
                        .traffic_recorder
                        .update_by_id(&self.guard.record_id, |record| {
                            record.response_size = self.guard.total_bytes;
                        });
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.guard.finished || self.inner.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}

pub fn create_sse_tee_body(
    body: Incoming,
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
) -> SseTeeBody {
    SseTeeBody::new(body, admin_state, record_id)
}
