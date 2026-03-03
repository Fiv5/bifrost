use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bifrost_admin::{
    AdminState, ConnectionInfo, FrameDirection, MatchedRule, TrafficRecord, TrafficType,
};
use bifrost_core::{BifrostError, Protocol, Result};
use bytes::{Buf, Bytes};
use h3::quic::BidiStream;
use h3::server::RequestStream;
use hyper::{Request, Response, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::dns::DnsResolver;
use crate::server::{ProxyConfig, RulesResolver};
use crate::utils::logging::RequestContext;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const BUFFER_SIZE: usize = 16384;

pub async fn handle_h3_connect<S>(
    req: Request<()>,
    stream: RequestStream<S, Bytes>,
    peer_addr: SocketAddr,
    rules: Arc<dyn RulesResolver>,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Arc<DnsResolver>,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let authority = req
        .uri()
        .authority()
        .ok_or_else(|| BifrostError::Parse("CONNECT request missing authority".to_string()))?
        .to_string();

    let (host, port) = parse_authority(&authority)?;
    let verbose = proxy_config.verbose_logging;

    let ctx = RequestContext::new();

    if verbose {
        info!("[{}] HTTP/3 CONNECT to {}:{}", ctx.id_str(), host, port);
    }

    let url = format!("https://{}:{}", host, port);
    let resolved_rules = rules.resolve(&url, "CONNECT");

    let has_rules = resolved_rules.host.is_some() || !resolved_rules.rules.is_empty();
    if verbose && has_rules {
        info!(
            "[{}] HTTP/3 CONNECT rules matched for {}:{}",
            ctx.id_str(),
            host,
            port
        );
    }

    let (target_host, target_port) = if let Some(ref host_rule) = resolved_rules.host {
        let host_rule_clean = host_rule.trim_end_matches('/');
        let parts: Vec<&str> = host_rule_clean.split(':').collect();
        let h = parts[0].to_string();
        let p = if parts.len() > 1 {
            parts[1].parse().unwrap_or(port)
        } else {
            match resolved_rules.host_protocol {
                Some(Protocol::Http) | Some(Protocol::Ws) => 80,
                Some(Protocol::Https) | Some(Protocol::Wss) => 443,
                _ => port,
            }
        };
        if verbose {
            info!(
                "[{}] HTTP/3 CONNECT redirected: {}:{} -> {}:{} (protocol={:?})",
                ctx.id_str(),
                host,
                port,
                h,
                p,
                resolved_rules.host_protocol
            );
        }
        (h, p)
    } else {
        (host.clone(), port)
    };

    let connect_host = if !resolved_rules.dns_servers.is_empty() {
        if verbose {
            info!(
                "[{}] [DNS] resolving {} with custom servers: {:?}",
                ctx.id_str(),
                target_host,
                resolved_rules.dns_servers
            );
        }
        match dns_resolver
            .resolve(&target_host, &resolved_rules.dns_servers)
            .await
        {
            Ok(Some(ip)) => {
                if verbose {
                    info!(
                        "[{}] [DNS] resolved {} -> {}",
                        ctx.id_str(),
                        target_host,
                        ip
                    );
                }
                ip.to_string()
            }
            Ok(None) | Err(_) => target_host.clone(),
        }
    } else {
        dns_resolver
            .resolve(&target_host, &[])
            .await
            .ok()
            .flatten()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| target_host.clone())
    };

    let target_stream = match timeout(
        CONNECT_TIMEOUT,
        TcpStream::connect(format!("{}:{}", connect_host, target_port)),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            warn!(
                "[{}] HTTP/3 CONNECT failed to {}:{}: {}",
                ctx.id_str(),
                target_host,
                target_port,
                e
            );
            let mut stream = stream;
            let response = Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(())
                .unwrap();
            let _ = stream.send_response(response).await;
            let _ = stream.finish().await;
            return Err(BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                target_host, target_port, e
            )));
        }
        Err(_) => {
            warn!(
                "[{}] HTTP/3 CONNECT timeout to {}:{}",
                ctx.id_str(),
                target_host,
                target_port
            );
            let mut stream = stream;
            let response = Response::builder()
                .status(StatusCode::GATEWAY_TIMEOUT)
                .body(())
                .unwrap();
            let _ = stream.send_response(response).await;
            let _ = stream.finish().await;
            return Err(BifrostError::Network(format!(
                "Connection timeout to {}:{}",
                target_host, target_port
            )));
        }
    };

    if let Err(e) = target_stream.set_nodelay(true) {
        warn!(
            "[{}] Failed to set TCP_NODELAY on tunnel connection: {}",
            ctx.id_str(),
            e
        );
    }

    let mut stream = stream;
    let response = Response::builder().status(StatusCode::OK).body(()).unwrap();

    stream
        .send_response(response)
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send CONNECT response: {}", e)))?;

    if verbose {
        info!(
            "[{}] HTTP/3 CONNECT tunnel established to {}:{}",
            ctx.id_str(),
            target_host,
            target_port
        );
    }

    let req_id = ctx.id_str();
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_connections_by_type(TrafficType::Tunnel);
        state
            .metrics_collector
            .increment_requests_by_type(TrafficType::Tunnel);

        let conn_info = ConnectionInfo::new(
            req_id.clone(),
            format!("{}:{}", target_host, target_port),
            target_port,
            false,
            ctx.client_app.clone(),
            cancel_tx,
        );
        state.connection_registry.register(conn_info);

        let mut record = TrafficRecord::new(
            req_id.clone(),
            "CONNECT".to_string(),
            format!("h3://{}:{}", host, port),
        );
        record.status = 200;
        record.protocol = "h3-tunnel".to_string();
        record.host = host.clone();
        record.is_tunnel = true;
        record.client_ip = peer_addr.ip().to_string();
        record.has_rule_hit = has_rules;
        record.matched_rules = if resolved_rules.rules.is_empty() {
            None
        } else {
            Some(
                resolved_rules
                    .rules
                    .iter()
                    .map(|r| MatchedRule {
                        pattern: r.pattern.clone(),
                        protocol: format!("{:?}", r.protocol),
                        value: r.value.clone(),
                        rule_name: r.rule_name.clone(),
                        raw: r.raw.clone(),
                        line: r.line,
                    })
                    .collect(),
            )
        };
        state.record_traffic(record);

        state.connection_monitor.register_connection(&req_id);
    }

    let result = h3_tunnel_bidirectional(
        stream,
        target_stream,
        verbose,
        &req_id,
        admin_state.as_ref(),
        cancel_rx,
    )
    .await;

    if let Some(ref state) = admin_state {
        state.connection_registry.unregister(&req_id);
        state.connection_monitor.unregister_connection(&req_id);
        state
            .metrics_collector
            .decrement_connections_by_type(TrafficType::Tunnel);
    }

    result
}

fn parse_authority(authority: &str) -> Result<(String, u16)> {
    if let Some(colon_pos) = authority.rfind(':') {
        let host = authority[..colon_pos].to_string();
        let port = authority[colon_pos + 1..].parse::<u16>().map_err(|_| {
            BifrostError::Parse(format!("Invalid port in authority: {}", authority))
        })?;
        Ok((host, port))
    } else {
        Ok((authority.to_string(), 443))
    }
}

async fn h3_tunnel_bidirectional<S>(
    stream: RequestStream<S, Bytes>,
    target: TcpStream,
    verbose_logging: bool,
    req_id: &str,
    admin_state: Option<&Arc<AdminState>>,
    cancel_rx: oneshot::Receiver<()>,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let (mut target_read, mut target_write) = target.into_split();
    let (mut send_stream, mut recv_stream) = stream.split();

    let admin_state_clone = admin_state.cloned();
    let admin_state_clone2 = admin_state.cloned();
    let req_id_owned = req_id.to_string();
    let req_id_owned2 = req_id.to_string();

    let client_to_target = async move {
        let mut total_sent: u64 = 0;
        loop {
            match recv_stream.recv_data().await {
                Ok(Some(mut data)) => {
                    while data.remaining() > 0 {
                        let chunk = data.chunk();
                        let len = chunk.len();
                        if len == 0 {
                            break;
                        }

                        target_write.write_all(chunk).await?;
                        target_write.flush().await?;
                        total_sent += len as u64;
                        data.advance(len);

                        if let Some(ref state) = admin_state_clone {
                            state
                                .metrics_collector
                                .add_bytes_sent_by_type(TrafficType::Tunnel, len as u64);
                            state.connection_monitor.update_traffic(
                                &req_id_owned,
                                FrameDirection::Send,
                                len as u64,
                            );
                        }
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    return Err(std::io::Error::other(format!("H3 recv error: {}", e)));
                }
            }
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(total_sent)
    };

    let target_to_client = async move {
        let mut buf = vec![0u8; BUFFER_SIZE];
        let mut total_received: u64 = 0;
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            send_stream
                .send_data(Bytes::copy_from_slice(&buf[..n]))
                .await
                .map_err(|e| std::io::Error::other(format!("H3 send error: {}", e)))?;

            total_received += n as u64;

            if let Some(ref state) = admin_state_clone2 {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Tunnel, n as u64);
                state.connection_monitor.update_traffic(
                    &req_id_owned2,
                    FrameDirection::Receive,
                    n as u64,
                );
            }
        }
        send_stream
            .finish()
            .await
            .map_err(|e| std::io::Error::other(format!("H3 finish error: {}", e)))?;
        Ok::<_, std::io::Error>(total_received)
    };

    tokio::select! {
        result = async {
            tokio::try_join!(client_to_target, target_to_client)
        } => {
            match result {
                Ok((sent, received)) => {
                    if verbose_logging {
                        debug!(
                            "[{}] HTTP/3 tunnel closed normally (sent: {}, received: {})",
                            req_id, sent, received
                        );
                    }
                    Ok(())
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::ConnectionReset
                        || e.kind() == std::io::ErrorKind::BrokenPipe
                    {
                        if verbose_logging {
                            debug!("[{}] HTTP/3 tunnel closed: {}", req_id, e);
                        }
                        Ok(())
                    } else {
                        Err(BifrostError::Network(format!("HTTP/3 tunnel error: {}", e)))
                    }
                }
            }
        }
        _ = cancel_rx => {
            if verbose_logging {
                debug!("[{}] HTTP/3 tunnel cancelled", req_id);
            }
            Ok(())
        }
    }
}
