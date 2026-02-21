use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bifrost_admin::AdminState;
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, Incoming};

use crate::decompress::decompress_body;
use crate::server::BoxBody;

struct TeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
    buffer: BytesMut,
    max_body_size: usize,
    content_encoding: Option<String>,
}

impl Drop for TeeBodyDropGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.store_body_and_update_record();
        }
    }
}

impl TeeBodyDropGuard {
    fn store_body_and_update_record(&mut self) {
        if let Some(ref state) = self.admin_state {
            let response_body_ref = if !self.buffer.is_empty() {
                if let Some(ref body_store) = state.body_store {
                    let store = body_store.read();
                    let decompressed =
                        decompress_body(&self.buffer, self.content_encoding.as_deref());
                    store.store(&self.record_id, "res", decompressed.as_ref())
                } else {
                    None
                }
            } else {
                None
            };

            let total_bytes = self.total_bytes;
            state.update_traffic_by_id(&self.record_id, move |record| {
                record.response_size = total_bytes;
                if response_body_ref.is_some() {
                    record.response_body_ref = response_body_ref.clone();
                }
            });
        }
    }
}

pub struct TeeBody {
    inner: Incoming,
    guard: TeeBodyDropGuard,
}

const DEFAULT_MAX_BODY_BUFFER_SIZE: usize = 10 * 1024 * 1024;

impl TeeBody {
    pub fn new(
        inner: Incoming,
        admin_state: Option<Arc<AdminState>>,
        record_id: String,
        max_body_size: Option<usize>,
        content_encoding: Option<String>,
    ) -> Self {
        let max_size = max_body_size.unwrap_or(DEFAULT_MAX_BODY_BUFFER_SIZE);
        Self {
            inner,
            guard: TeeBodyDropGuard {
                admin_state,
                record_id,
                total_bytes: 0,
                finished: false,
                buffer: BytesMut::with_capacity(8192),
                max_body_size: max_size,
                content_encoding,
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

                    if self.guard.buffer.len() + len <= self.guard.max_body_size {
                        self.guard.buffer.extend_from_slice(data);
                    }

                    if let Some(ref state) = self.guard.admin_state {
                        state.metrics_collector.add_bytes_received(len as u64);
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.guard.finished = true;
                self.guard.store_body_and_update_record();
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                self.guard.finished = true;
                self.guard.store_body_and_update_record();
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
    max_body_size: Option<usize>,
    content_encoding: Option<String>,
) -> TeeBody {
    TeeBody::new(
        body,
        admin_state,
        record_id,
        max_body_size,
        content_encoding,
    )
}

struct SseTeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
}

impl Drop for SseTeeBodyDropGuard {
    fn drop(&mut self) {
        if let Some(ref state) = self.admin_state {
            if !self.finished {
                let total_bytes = self.total_bytes;
                state.update_traffic_by_id(&self.record_id, move |record| {
                    record.response_size = total_bytes;
                });
            }
            state.connection_monitor.set_connection_closed(
                &self.record_id,
                None,
                None,
                state.frame_store.as_ref(),
            );
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
            state.connection_monitor.register_connection(&record_id);
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
                    state.connection_monitor.record_sse_event(
                        &self.guard.record_id,
                        event_bytes,
                        state.body_store.as_ref(),
                        state.frame_store.as_ref(),
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
                        state.connection_monitor.record_sse_event(
                            &self.guard.record_id,
                            &self.buffer,
                            state.body_store.as_ref(),
                            state.frame_store.as_ref(),
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

use bifrost_admin::BodyRef;

pub fn store_request_body(
    admin_state: &Option<Arc<AdminState>>,
    record_id: &str,
    body_data: &[u8],
    content_encoding: Option<&str>,
) -> Option<BodyRef> {
    if body_data.is_empty() {
        return None;
    }

    if let Some(ref state) = admin_state {
        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();
            let decompressed = decompress_body(body_data, content_encoding);
            return store.store(record_id, "req", decompressed.as_ref());
        }
    }
    None
}
