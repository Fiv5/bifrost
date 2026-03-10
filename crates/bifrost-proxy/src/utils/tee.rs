use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bifrost_admin::{AdminState, BodyRef, BodyStreamWriter, FrameDirection, TrafficType};
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, Incoming};
use memchr::memchr;
use tokio::time::Sleep;

use crate::server::BoxBody;
use crate::transform::decompress::decompress_body_with_limit;

fn persist_socket_summary(state: &AdminState, record_id: &str, total_bytes: usize) {
    let status = state.sse_hub.get_socket_status(record_id).map(|mut s| {
        s.is_open = false;
        s
    });
    let frame_count = status.as_ref().map(|s| s.frame_count).unwrap_or(0);
    let last_frame_id = frame_count as u64;
    let mut response_size = status.as_ref().map(|s| s.receive_bytes).unwrap_or(0) as usize;
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
                    let max_decompress_output_bytes = state
                        .config_manager
                        .as_ref()
                        .and_then(|cm| cm.try_config())
                        .map(|cfg| cfg.sandbox.limits.max_decompress_output_bytes)
                        .unwrap_or(10 * 1024 * 1024);
                    let decompressed = decompress_body_with_limit(
                        &self.buffer,
                        self.content_encoding.as_deref(),
                        max_decompress_output_bytes,
                    );
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
                state.ws_payload_store.as_ref(),
            );
        }
    }
}

struct TeeBody<B> {
    inner: Pin<Box<B>>,
    guard: TeeBodyDropGuard,
}

const DEFAULT_MAX_BODY_BUFFER_SIZE: usize = 10 * 1024 * 1024;

impl<B> TeeBody<B> {
    pub fn new(
        inner: B,
        admin_state: Option<Arc<AdminState>>,
        record_id: String,
        max_body_size: Option<usize>,
        content_encoding: Option<String>,
        traffic_type: Option<TrafficType>,
        response_headers_size: usize,
    ) -> Self {
        let max_size = max_body_size.unwrap_or(DEFAULT_MAX_BODY_BUFFER_SIZE);
        Self {
            inner: Box::pin(inner),
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

    pub fn boxed(self) -> BoxBody
    where
        B: Body<Data = Bytes, Error = hyper::Error> + Send + Sync + 'static,
    {
        BodyExt::boxed(self)
    }
}

impl<B> Body for TeeBody<B>
where
    B: Body<Data = Bytes, Error = hyper::Error>,
{
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.guard.finished {
            return Poll::Ready(None);
        }

        match self.inner.as_mut().poll_frame(cx) {
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
    body: impl Body<Data = Bytes, Error = hyper::Error> + Send + Sync + 'static,
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    max_body_size: Option<usize>,
    content_encoding: Option<String>,
    traffic_type: Option<TrafficType>,
    response_headers_size: usize,
) -> BoxBody {
    TeeBody::new(
        body,
        admin_state,
        record_id,
        max_body_size,
        content_encoding,
        traffic_type,
        response_headers_size,
    )
    .boxed()
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
    inner: Pin<Box<dyn Body<Data = Bytes, Error = hyper::Error> + Send + Sync>>,
    guard: RequestTeeBodyDropGuard,
}

impl Body for RequestTeeBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.inner.as_mut().poll_frame(cx) {
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
    body: impl Body<Data = Bytes, Error = hyper::Error> + Send + Sync + 'static,
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
) -> (BoxBody, BodyCaptureHandle) {
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
    let body = RequestTeeBody {
        inner: Box::pin(body),
        guard,
    };
    (BodyExt::boxed(body), capture)
}

struct SseTeeBodyDropGuard {
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    total_bytes: usize,
    finished: bool,
    traffic_type: Option<TrafficType>,
    file_writer: Option<BodyStreamWriter>,
}

impl Drop for SseTeeBodyDropGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.store_body_and_update_record();
        }
    }
}

impl SseTeeBodyDropGuard {
    fn store_body_and_update_record(&mut self) {
        if let Some(ref state) = self.admin_state {
            let response_body_ref = self.file_writer.take().map(|w| w.finish());
            state.sse_hub.set_closed(&self.record_id);
            state.update_traffic_by_id(&self.record_id, move |record| {
                record.response_body_ref = response_body_ref.clone();
            });
            persist_socket_summary(state, &self.record_id, self.total_bytes);
            state.sse_hub.unregister(&self.record_id);
        }
        self.finished = true;
    }
}

const DEFAULT_MAX_SSE_EVENT_BUFFER_BYTES: usize = 256 * 1024;

pub struct SseTeeBody {
    inner: Incoming,
    guard: SseTeeBodyDropGuard,
    prev_byte: Option<u8>,
    event_size: usize,
    max_buffer_size: usize,
    overflowed: bool,
    flush_interval: Option<std::time::Duration>,
    flush_sleep: Option<Pin<Box<Sleep>>>,
}

impl SseTeeBody {
    pub fn new(
        inner: Incoming,
        admin_state: Option<Arc<AdminState>>,
        record_id: String,
        traffic_type: Option<TrafficType>,
        file_writer: Option<BodyStreamWriter>,
        max_buffer_size: usize,
    ) -> Self {
        let flush_interval = file_writer
            .as_ref()
            .map(|w| w.flush_interval())
            .filter(|d| !d.is_zero());
        let flush_sleep = flush_interval.map(|d| Box::pin(tokio::time::sleep(d)));

        if let Some(ref state) = admin_state {
            state.sse_hub.register(&record_id);
        }

        Self {
            inner,
            guard: SseTeeBodyDropGuard {
                admin_state,
                record_id,
                total_bytes: 0,
                finished: false,
                traffic_type,
                file_writer,
            },
            prev_byte: None,
            event_size: 0,
            max_buffer_size,
            overflowed: false,
            flush_interval,
            flush_sleep,
        }
    }

    pub fn boxed(self) -> BoxBody {
        BodyExt::boxed(self)
    }

    fn process_sse_chunk(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        let mut i = 0;
        while i < data.len() {
            let Some(rel) = memchr(b'\n', &data[i..]) else {
                if !self.overflowed {
                    self.event_size = self.event_size.saturating_add(data.len() - i);
                    if self.max_buffer_size > 0 && self.event_size > self.max_buffer_size {
                        self.overflowed = true;
                    }
                }
                self.prev_byte = Some(*data.last().unwrap());
                return;
            };

            let pos = i + rel;
            if pos > i {
                if !self.overflowed {
                    self.event_size = self.event_size.saturating_add(pos - i);
                    if self.max_buffer_size > 0 && self.event_size > self.max_buffer_size {
                        self.overflowed = true;
                    }
                }
                self.prev_byte = Some(data[pos - 1]);
            }

            if self.prev_byte == Some(b'\n') {
                if self.event_size > 0 {
                    if let Some(ref state) = self.guard.admin_state {
                        state.sse_hub.add_receive_event(&self.guard.record_id);
                    }
                }
                self.event_size = 0;
                self.overflowed = false;
                self.prev_byte = Some(b'\n');
            } else {
                if !self.overflowed {
                    self.event_size = self.event_size.saturating_add(1);
                    if self.max_buffer_size > 0 && self.event_size > self.max_buffer_size {
                        self.overflowed = true;
                    }
                }
                self.prev_byte = Some(b'\n');
            }

            i = pos + 1;
        }
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

        if let (Some(interval), Some(mut sleep_fut)) =
            (self.flush_interval, self.flush_sleep.take())
        {
            if sleep_fut.as_mut().poll(cx).is_ready() {
                if let Some(ref mut writer) = self.guard.file_writer {
                    let _ = writer.flush_buffered();
                }
                self.flush_sleep = Some(Box::pin(tokio::time::sleep(interval)));
            } else {
                self.flush_sleep = Some(sleep_fut);
            }
        }

        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let len = data.len();
                    self.guard.total_bytes += len;

                    if let Some(ref state) = self.guard.admin_state {
                        state.sse_hub.add_receive_bytes(&self.guard.record_id, len);
                        if let Some(traffic_type) = self.guard.traffic_type {
                            state
                                .metrics_collector
                                .add_bytes_received_by_type(traffic_type, len as u64);
                        } else {
                            state.metrics_collector.add_bytes_received(len as u64);
                        }
                    }

                    let should_force_flush = self
                        .guard
                        .admin_state
                        .as_ref()
                        .map(|state| state.sse_hub.should_force_flush(&self.guard.record_id))
                        .unwrap_or(false);

                    if let Some(ref mut writer) = self.guard.file_writer {
                        let _ = writer.write_chunk(data);
                        if should_force_flush {
                            let _ = writer.flush_buffered();
                        }
                    }

                    self.process_sse_chunk(data);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.guard.store_body_and_update_record();
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
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

pub fn create_sse_tee_body(
    body: Incoming,
    admin_state: Option<Arc<AdminState>>,
    record_id: String,
    traffic_type: Option<TrafficType>,
    file_writer: Option<BodyStreamWriter>,
    max_buffer_size: usize,
) -> SseTeeBody {
    let max_buffer_size = max_buffer_size.min(DEFAULT_MAX_SSE_EVENT_BUFFER_BYTES);
    SseTeeBody::new(
        body,
        admin_state,
        record_id,
        traffic_type,
        file_writer,
        max_buffer_size,
    )
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
            let max_decompress_output_bytes = state
                .config_manager
                .as_ref()
                .and_then(|cm| cm.try_config())
                .map(|cfg| cfg.sandbox.limits.max_decompress_output_bytes)
                .unwrap_or(10 * 1024 * 1024);
            let decompressed = decompress_body_with_limit(
                body_data,
                content_encoding,
                max_decompress_output_bytes,
            );
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
