use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bifrost_admin::{AdminState, BodyRef, BodyStreamWriter, FrameDirection, TrafficType};
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, Incoming};

use crate::server::BoxBody;
use crate::transform::decompress::decompress_body;

fn persist_socket_summary(state: &AdminState, record_id: &str, total_bytes: usize) {
    let status = state.connection_monitor.get_connection_status(record_id);
    let last_frame_id = state
        .connection_monitor
        .get_last_frame_id(record_id)
        .unwrap_or(0);
    let frame_count = status.as_ref().map(|s| s.frame_count).unwrap_or(0);
    let status = status.map(|mut s| {
        s.is_open = false;
        s
    });
    let mut response_size = status
        .as_ref()
        .map(|s| s.send_bytes + s.receive_bytes)
        .unwrap_or(0) as usize;
    if response_size == 0 {
        response_size = total_bytes;
    }
    state.update_traffic_by_id(record_id, move |record| {
        record.response_size = response_size;
        record.frame_count = frame_count;
        record.last_frame_id = last_frame_id;
        if let Some(ref s) = status {
            record.socket_status = Some(s.clone());
        }
    });
}

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
                persist_socket_summary(state, &self.record_id, total_bytes);
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
    preview_limit: usize,
    stream_writer: Option<BodyStreamWriter>,
    stream_path: Option<String>,
    stream_bytes: usize,
    event_start: usize,
    prev_byte: Option<u8>,
    preview_buf: Vec<u8>,
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
        let preview_limit = admin_state
            .as_ref()
            .map(|state| state.connection_monitor.sse_preview_limit())
            .unwrap_or(0)
            .min(max_buffer_size);

        Self {
            inner,
            guard: SseTeeBodyDropGuard {
                admin_state,
                record_id,
                total_bytes: 0,
                finished: false,
                traffic_type,
            },
            preview_limit,
            stream_writer: None,
            stream_path: None,
            stream_bytes: 0,
            event_start: 0,
            prev_byte: None,
            preview_buf: Vec::with_capacity(preview_limit.min(4096)),
        }
    }

    pub fn boxed(self) -> BoxBody {
        BodyExt::boxed(self)
    }

    fn init_stream_writer(&mut self) {
        if self.stream_writer.is_some() {
            return;
        }
        let Some(ref state) = self.guard.admin_state else {
            return;
        };
        let Some(ref body_store) = state.body_store else {
            return;
        };
        let store = body_store.read();
        if let Ok(writer) = store.start_stream(&self.guard.record_id, "sse_stream") {
            self.stream_path = Some(writer.path().to_string_lossy().to_string());
            self.stream_writer = Some(writer);
        }
    }

    fn write_stream_chunk(&mut self, data: &[u8]) {
        if let Some(ref mut writer) = self.stream_writer {
            if writer.write_chunk(data).is_err() {
                self.stream_writer = None;
                self.stream_path = None;
            }
        }
    }

    fn record_event(&mut self, event_len: usize) {
        if event_len == 0 {
            self.preview_buf.clear();
            return;
        }
        let preview_bytes = self.preview_buf.clone();
        if let Some(ref state) = self.guard.admin_state {
            let payload_ref = self.stream_path.as_ref().map(|path| BodyRef::FileRange {
                path: path.clone(),
                offset: self.event_start as u64,
                size: event_len,
            });
            state.connection_monitor.record_sse_event_streamed(
                &self.guard.record_id,
                event_len,
                &preview_bytes,
                payload_ref,
                state.frame_store.as_ref(),
            );
        }
        self.preview_buf.clear();
    }

    fn process_sse_chunk(&mut self, data: &[u8]) {
        let chunk_start = self.stream_bytes;
        for (i, &byte) in data.iter().enumerate() {
            if self.prev_byte == Some(b'\n') && byte == b'\n' {
                if self.preview_limit > 0 {
                    if let Some(&last) = self.preview_buf.last() {
                        if last == b'\n' {
                            self.preview_buf.pop();
                        }
                    }
                }
                let boundary_offset = chunk_start + i;
                let event_len = boundary_offset
                    .saturating_sub(1)
                    .saturating_sub(self.event_start);
                self.record_event(event_len);
                self.event_start = boundary_offset + 1;
                self.prev_byte = Some(byte);
                continue;
            }
            if self.preview_limit > 0 && self.preview_buf.len() < self.preview_limit {
                self.preview_buf.push(byte);
            }
            self.prev_byte = Some(byte);
        }
        self.stream_bytes += data.len();
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

                    self.init_stream_writer();
                    self.write_stream_chunk(data);
                    self.process_sse_chunk(data);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.guard.finished = true;
                if let Some(ref state) = self.guard.admin_state {
                    state.connection_monitor.set_connection_closed(
                        &self.guard.record_id,
                        None,
                        None,
                        state.frame_store.as_ref(),
                    );
                    persist_socket_summary(state, &self.guard.record_id, self.guard.total_bytes);
                }
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                self.guard.finished = true;
                if self.event_start < self.stream_bytes {
                    let event_len = self.stream_bytes - self.event_start;
                    self.record_event(event_len);
                }
                if let Some(ref state) = self.guard.admin_state {
                    state.connection_monitor.set_connection_closed(
                        &self.guard.record_id,
                        None,
                        None,
                        state.frame_store.as_ref(),
                    );
                    persist_socket_summary(state, &self.guard.record_id, self.guard.total_bytes);
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
