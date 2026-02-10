use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bifrost_admin::AdminState;
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::{Body, Frame, Incoming};
use pin_project_lite::pin_project;

use crate::server::BoxBody;

type StoreCallback = Box<dyn FnOnce(Vec<u8>) + Send + 'static>;

pin_project! {
    pub struct TeeBody {
        #[pin]
        inner: Incoming,
        buffer: Vec<u8>,
        store_callback: Arc<Mutex<Option<StoreCallback>>>,
        finished: bool,
    }
}

impl TeeBody {
    pub fn new(inner: Incoming, callback: StoreCallback) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            store_callback: Arc::new(Mutex::new(Some(callback))),
            finished: false,
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
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        if *this.finished {
            return Poll::Ready(None);
        }

        match this.inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.buffer.extend_from_slice(data);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                *this.finished = true;
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                *this.finished = true;
                if let Ok(mut guard) = this.store_callback.lock() {
                    if let Some(callback) = guard.take() {
                        let data = std::mem::take(this.buffer);
                        callback(data);
                    }
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.finished || self.inner.is_end_stream()
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
    let callback: StoreCallback = Box::new(move |data: Vec<u8>| {
        if let Some(state) = admin_state {
            state
                .metrics_collector
                .add_bytes_received(data.len() as u64);

            let body_ref = if let Some(ref body_store) = state.body_store {
                let store = body_store.read();
                store.store(&record_id, "res", &data)
            } else {
                None
            };

            state.traffic_recorder.update_by_id(&record_id, |record| {
                record.response_size = data.len();
                record.response_body_ref = body_ref;
            });
        }
    });

    TeeBody::new(body, callback)
}
