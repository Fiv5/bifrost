use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone)]
pub struct PortRebindResponse {
    pub expected_port: u16,
    pub actual_port: u16,
}

pub struct PortRebindRequest {
    pub expected_port: u16,
    pub response_tx: oneshot::Sender<Result<PortRebindResponse, String>>,
}

#[derive(Clone)]
pub struct PortRebindManager {
    tx: mpsc::Sender<PortRebindRequest>,
}

pub type SharedPortRebindManager = Arc<PortRebindManager>;

impl PortRebindManager {
    pub fn new(tx: mpsc::Sender<PortRebindRequest>) -> Self {
        Self { tx }
    }

    pub fn channel(buffer: usize) -> (SharedPortRebindManager, mpsc::Receiver<PortRebindRequest>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Arc::new(Self::new(tx)), rx)
    }

    pub async fn rebind_port(&self, expected_port: u16) -> Result<PortRebindResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(PortRebindRequest {
                expected_port,
                response_tx,
            })
            .await
            .map_err(|_| "port rebind manager is unavailable".to_string())?;

        response_rx
            .await
            .map_err(|_| "port rebind response channel closed".to_string())?
    }
}
