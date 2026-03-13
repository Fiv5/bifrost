use std::sync::Arc;

use bifrost_admin::{AdminState, FrameDirection, TrafficType};
use bifrost_core::{BifrostError, Result};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tracing::debug;

pub async fn tunnel_bidirectional(
    upgraded: Upgraded,
    target: TcpStream,
    verbose_logging: bool,
    req_id: &str,
    admin_state: Option<&Arc<AdminState>>,
) -> Result<()> {
    let client = TokioIo::new(upgraded);
    let (mut target_read, mut target_write) = target.into_split();

    let (client_read, client_write) = tokio::io::split(client);
    let mut client_read = client_read;
    let mut client_write = client_write;

    let admin_state_clone = admin_state.cloned();
    let admin_state_clone2 = admin_state.cloned();

    let client_to_target = async move {
        let mut buf = [0u8; 16384];
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Tunnel, n as u64);
            }
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(())
    };

    let target_to_client = async move {
        let mut buf = [0u8; 16384];
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone2 {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Tunnel, n as u64);
            }
        }
        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_target, target_to_client);

    match result {
        Ok(_) => {
            if verbose_logging {
                debug!("[{}] Tunnel closed normally", req_id);
            } else {
                debug!("Tunnel closed normally");
            }
            Ok(())
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::BrokenPipe
            {
                if verbose_logging {
                    debug!("[{}] Tunnel closed: {}", req_id, e);
                } else {
                    debug!("Tunnel closed: {}", e);
                }
                Ok(())
            } else {
                Err(BifrostError::Network(format!("Tunnel error: {}", e)))
            }
        }
    }
}

pub struct TunnelStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub cancelled: bool,
}

pub async fn tunnel_bidirectional_with_cancel(
    upgraded: Upgraded,
    target: TcpStream,
    verbose_logging: bool,
    req_id: &str,
    admin_state: Option<&Arc<AdminState>>,
    cancel_rx: oneshot::Receiver<()>,
) -> Result<TunnelStats> {
    let client = TokioIo::new(upgraded);
    let (mut target_read, mut target_write) = target.into_split();

    let (client_read, client_write) = tokio::io::split(client);
    let mut client_read = client_read;
    let mut client_write = client_write;

    let admin_state_clone = admin_state.cloned();
    let admin_state_clone2 = admin_state.cloned();
    let req_id_owned = req_id.to_string();
    let req_id_owned2 = req_id.to_string();

    let client_to_target = async move {
        let mut buf = [0u8; 16384];
        let mut total_sent: u64 = 0;
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;
            total_sent += n as u64;

            if let Some(ref state) = admin_state_clone {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Tunnel, n as u64);
                // 对于隧道连接，只更新流量统计，不记录详细帧
                state.connection_monitor.update_traffic(
                    &req_id_owned,
                    FrameDirection::Send,
                    n as u64,
                );
            }
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(total_sent)
    };

    let target_to_client = async move {
        let mut buf = [0u8; 16384];
        let mut total_received: u64 = 0;
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;
            total_received += n as u64;

            if let Some(ref state) = admin_state_clone2 {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Tunnel, n as u64);
                // 对于隧道连接，只更新流量统计，不记录详细帧
                state.connection_monitor.update_traffic(
                    &req_id_owned2,
                    FrameDirection::Receive,
                    n as u64,
                );
            }
        }

        Ok::<_, std::io::Error>(total_received)
    };

    let bidirectional = async { tokio::try_join!(client_to_target, target_to_client) };

    tokio::select! {
        result = bidirectional => {
            match result {
                Ok((bytes_sent, bytes_received)) => {
                    if verbose_logging {
                        debug!("[{}] Tunnel closed normally", req_id);
                    } else {
                        debug!("Tunnel closed normally");
                    }
                    Ok(TunnelStats { bytes_sent, bytes_received, cancelled: false })
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::ConnectionReset
                        || e.kind() == std::io::ErrorKind::BrokenPipe
                    {
                        if verbose_logging {
                            debug!("[{}] Tunnel closed: {}", req_id, e);
                        } else {
                            debug!("Tunnel closed: {}", e);
                        }
                        Ok(TunnelStats { bytes_sent: 0, bytes_received: 0, cancelled: false })
                    } else {
                        Err(BifrostError::Network(format!("Tunnel error: {}", e)))
                    }
                }
            }
        }
        _ = cancel_rx => {
            if verbose_logging {
                debug!("[{}] Tunnel cancelled by config change", req_id);
            }
            Ok(TunnelStats { bytes_sent: 0, bytes_received: 0, cancelled: true })
        }
    }
}
