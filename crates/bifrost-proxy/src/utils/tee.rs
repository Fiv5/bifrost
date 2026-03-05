use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bifrost_admin::{AdminState, BodyRef, BodyStreamWriter, FrameDirection, TrafficType};
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, Incoming};

use crate::server::BoxBody;
use crate::transform::decompress::decompress_body;

struct TeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
    buffer: BytesMut,
    max_body_size: usize,
    content_encoding: Option<String>,
    traffic_type: Option<TrafficType>,
    response_headers_size: usize,
    file_writer: Option<BodyStreamWriter>,
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
            let response_body_ref = if let Some(writer) = self.file_writer.take() {
                Some(writer.finish())
            } else if !self.buffer.is_empty() {
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

            let total_bytes = self.total_bytes + self.response_headers_size;
            state.update_traffic_by_id(&self.record_id, move |record| {
                record.response_size = total_bytes;
                if response_body_ref.is_some() {
                    record.response_body_ref = response_body_ref.clone();
                }
            });

            state.connection_monitor.set_connection_closed(
                &self.record_id,
                None,
                None,
                state.frame_store.as_ref(),
            );
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
        traffic_type: Option<TrafficType>,
        response_headers_size: usize,
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
                traffic_type,
                response_headers_size,
                file_writer: None,
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

                    let mut new_writer: Option<BodyStreamWriter> = None;
                    if self.guard.file_writer.is_none()
                        && self.guard.buffer.len() + len > self.guard.max_body_size
                    {
                        if let Some(ref state) = self.guard.admin_state {
                            if let Some(ref body_store) = state.body_store {
                                let store = body_store.read();
                                if let Ok(writer) = store.start_stream(&self.guard.record_id, "res")
                                {
                                    new_writer = Some(writer);
                                }
                            }
                        }
                    }

                    if let Some(mut writer) = new_writer {
                        if !self.guard.buffer.is_empty() {
                            let _ = writer.write_chunk(&self.guard.buffer);
                            self.guard.buffer.clear();
                        }
                        let _ = writer.write_chunk(data);
                        self.guard.file_writer = Some(writer);
                    } else if self.guard.file_writer.is_some() {
                        if let Some(writer) = self.guard.file_writer.as_mut() {
                            let _ = writer.write_chunk(data);
                        }
                    } else if self.guard.buffer.len() + len <= self.guard.max_body_size {
                        self.guard.buffer.extend_from_slice(data);
                    } else {
                        self.guard.buffer.clear();
                    }

                    if let Some(ref state) = self.guard.admin_state {
                        if let Some(traffic_type) = self.guard.traffic_type {
                            state
                                .metrics_collector
                                .add_bytes_received_by_type(traffic_type, len as u64);
                        } else {
                            state.metrics_collector.add_bytes_received(len as u64);
                        }
                        state.connection_monitor.update_traffic(
                            &self.guard.record_id,
                            FrameDirection::Receive,
                            len as u64,
                        );
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
    traffic_type: Option<TrafficType>,
    response_headers_size: usize,
) -> TeeBody {
    TeeBody::new(
        body,
        admin_state,
        record_id,
        max_body_size,
        content_encoding,
        traffic_type,
        response_headers_size,
    )
}

#[derive(Clone)]
pub struct BodyCaptureHandle {
    body_ref: Arc<Mutex<Option<BodyRef>>>,
}

impl BodyCaptureHandle {
    pub fn take(&self) -> Option<BodyRef> {
        self.body_ref.lock().ok()?.take()
    }
}

struct RequestTeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    file_writer: Option<BodyStreamWriter>,
    capture: BodyCaptureHandle,
}

impl Drop for RequestTeeBodyDropGuard {
    fn drop(&mut self) {
        if let Some(writer) = self.file_writer.take() {
            let body_ref = writer.finish();
            if let Ok(mut slot) = self.capture.body_ref.lock() {
                *slot = Some(body_ref);
            }
            if let Some(ref state) = self.admin_state {
                let capture = self.capture.clone();
                state.update_traffic_by_id(&self.record_id, move |record| {
                    if let Some(body_ref) = capture.take() {
                        record.request_body_ref = Some(body_ref);
                    }
                });
            }
        }
    }
}

pub struct RequestTeeBody {
    inner: Incoming,
    guard: RequestTeeBodyDropGuard,
}

impl Body for RequestTeeBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    if self.guard.file_writer.is_none() {
                        let mut new_writer: Option<BodyStreamWriter> = None;
                        if let Some(ref state) = self.guard.admin_state {
                            if let Some(ref body_store) = state.body_store {
                                let store = body_store.read();
                                if let Ok(writer) = store.start_stream(&self.guard.record_id, "req")
                                {
                                    new_writer = Some(writer);
                                }
                            }
                        }
                        if let Some(writer) = new_writer {
                            self.guard.file_writer = Some(writer);
                        }
                    }
                    if let Some(writer) = self.guard.file_writer.as_mut() {
                        let _ = writer.write_chunk(data);
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}

pub fn create_request_tee_body(
    body: Incoming,
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
) -> (RequestTeeBody, BodyCaptureHandle) {
    let capture = BodyCaptureHandle {
        body_ref: Arc::new(Mutex::new(None)),
    };
    let guard = RequestTeeBodyDropGuard {
        admin_state,
        record_id,
        file_writer: None,
        capture: BodyCaptureHandle {
            body_ref: capture.body_ref.clone(),
        },
    };
    (RequestTeeBody { inner: body, guard }, capture)
}

struct SseTeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
    traffic_type: Option<TrafficType>,
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
    max_buffer_size: usize,
}

impl SseTeeBody {
    pub fn new(
        inner: Incoming,
        admin_state: Option<Arc<AdminState>>,
        record_id: String,
        traffic_type: Option<TrafficType>,
        max_buffer_size: usize,
    ) -> Self {
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
                traffic_type,
            },
            buffer: BytesMut::with_capacity(4096),
            max_buffer_size,
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
                        if let Some(traffic_type) = self.guard.traffic_type {
                            state
                                .metrics_collector
                                .add_bytes_received_by_type(traffic_type, len as u64);
                        } else {
                            state.metrics_collector.add_bytes_received(len as u64);
                        }
                    }

                    if self.buffer.len() + len > self.max_buffer_size {
                        tracing::warn!(
                            "[SSE] buffer exceeded limit ({} bytes), dropping buffered data for {}",
                            self.max_buffer_size,
                            self.guard.record_id
                        );
                        self.buffer.clear();
                    }
                    if len <= self.max_buffer_size {
                        self.buffer.extend_from_slice(data);
                    }
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
    traffic_type: Option<TrafficType>,
    max_buffer_size: usize,
) -> SseTeeBody {
    SseTeeBody::new(body, admin_state, record_id, traffic_type, max_buffer_size)
}

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

pub fn store_response_body(
    admin_state: &Option<Arc<AdminState>>,
    record_id: &str,
    body_data: &[u8],
) -> Option<BodyRef> {
    if body_data.is_empty() {
        return None;
    }

    if let Some(ref state) = admin_state {
        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();
            return store.store(record_id, "res", body_data);
        }
    }
    None
}
