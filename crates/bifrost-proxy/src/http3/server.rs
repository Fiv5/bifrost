use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bifrost_admin::{AdminState, SharedPushManager};
use bifrost_core::{BifrostError, ClientAccessControl, Result};
use bytes::Bytes;
use h3::quic::BidiStream;
use h3::server::RequestStream;
use hyper::{Method, Request};
use quinn::{Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::dns::DnsResolver;
use crate::server::{ProxyConfig, RulesResolver, TlsConfig};

use super::proxy::handle_h3_proxy_request;
use super::tunnel::handle_h3_connect;

pub struct Http3Config {
    pub cert_chain: Vec<CertificateDer<'static>>,
    pub private_key: PrivateKeyDer<'static>,
}

pub struct Http3Server {
    config: ProxyConfig,
    h3_config: Http3Config,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    admin_state: Option<Arc<AdminState>>,
    #[allow(dead_code)]
    push_manager: Option<SharedPushManager>,
    dns_resolver: Arc<DnsResolver>,
    access_control: Arc<RwLock<ClientAccessControl>>,
}

impl Http3Server {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: ProxyConfig,
        h3_config: Http3Config,
        rules: Arc<dyn RulesResolver>,
        tls_config: Arc<TlsConfig>,
        admin_state: Option<Arc<AdminState>>,
        push_manager: Option<SharedPushManager>,
        dns_resolver: Arc<DnsResolver>,
        access_control: Arc<RwLock<ClientAccessControl>>,
    ) -> Self {
        Self {
            config,
            h3_config,
            rules,
            tls_config,
            admin_state,
            push_manager,
            dns_resolver,
            access_control,
        }
    }

    pub async fn bind(&self, addr: SocketAddr) -> Result<Endpoint> {
        let server_config = self.build_server_config()?;

        let endpoint = Endpoint::server(server_config, addr).map_err(|e| {
            BifrostError::Network(format!("Failed to create QUIC endpoint on {}: {}", addr, e))
        })?;

        info!("HTTP/3 server listening on UDP {}", addr);
        Ok(endpoint)
    }

    fn build_server_config(&self) -> Result<ServerConfig> {
        let certs = self.h3_config.cert_chain.clone();
        let key = self.h3_config.private_key.clone_key();

        let mut crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| BifrostError::Tls(format!("Failed to build TLS config: {}", e)))?;

        crypto.max_early_data_size = u32::MAX;
        crypto.alpn_protocols = vec![b"h3".to_vec()];

        let quic_config =
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto).map_err(|e| {
                BifrostError::Tls(format!("Failed to create QUIC server config: {}", e))
            })?;

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_config));

        let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
        transport_config.max_idle_timeout(Some(
            Duration::from_secs(self.config.timeout_secs)
                .try_into()
                .unwrap(),
        ));
        transport_config.keep_alive_interval(Some(Duration::from_secs(15)));
        transport_config.datagram_receive_buffer_size(Some(65536));
        transport_config.datagram_send_buffer_size(65536);

        Ok(server_config)
    }

    pub async fn serve(&self, endpoint: Endpoint) -> Result<()> {
        info!("HTTP/3 server started on UDP");

        loop {
            let incoming = match endpoint.accept().await {
                Some(conn) => conn,
                None => {
                    info!("HTTP/3 endpoint closed");
                    break;
                }
            };

            let peer_addr = incoming.remote_address();
            debug!("HTTP/3 connection from {}", peer_addr);

            let rules = Arc::clone(&self.rules);
            let tls_config = Arc::clone(&self.tls_config);
            let proxy_config = self.config.clone();
            let admin_state = self.admin_state.clone();
            let dns_resolver = Arc::clone(&self.dns_resolver);
            let access_control = Arc::clone(&self.access_control);

            tokio::spawn(async move {
                if let Err(e) = handle_h3_connection(
                    incoming,
                    peer_addr,
                    rules,
                    tls_config,
                    proxy_config,
                    admin_state,
                    dns_resolver,
                    access_control,
                )
                .await
                {
                    debug!("HTTP/3 connection error from {}: {}", peer_addr, e);
                }
            });
        }

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| BifrostError::Config(format!("Invalid address: {}", e)))?;

        let endpoint = self.bind(addr).await?;
        self.serve(endpoint).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_h3_connection(
    incoming: quinn::Incoming,
    peer_addr: SocketAddr,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Arc<DnsResolver>,
    _access_control: Arc<RwLock<ClientAccessControl>>,
) -> Result<()> {
    let connection = incoming
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to accept QUIC connection: {}", e)))?;

    debug!(
        "HTTP/3 connection established from {} (protocol: {:?})",
        peer_addr,
        connection.handshake_data()
    );

    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(connection))
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to create H3 connection: {}", e)))?;

    loop {
        match h3_conn.accept().await {
            Ok(Some(resolver)) => {
                let (req, stream) = resolver.resolve_request().await.map_err(|e| {
                    BifrostError::Network(format!("Failed to resolve H3 request: {}", e))
                })?;

                let rules = Arc::clone(&rules);
                let tls_config = Arc::clone(&tls_config);
                let proxy_config = proxy_config.clone();
                let admin_state = admin_state.clone();
                let dns_resolver = Arc::clone(&dns_resolver);

                tokio::spawn(async move {
                    if let Err(e) = handle_h3_request(
                        req,
                        stream,
                        peer_addr,
                        rules,
                        tls_config,
                        proxy_config,
                        admin_state,
                        dns_resolver,
                    )
                    .await
                    {
                        debug!("HTTP/3 request error: {}", e);
                    }
                });
            }
            Ok(None) => {
                debug!("HTTP/3 connection closed by peer: {}", peer_addr);
                break;
            }
            Err(e) => {
                warn!("HTTP/3 accept error from {}: {}", peer_addr, e);
                break;
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_h3_request<S>(
    req: Request<()>,
    stream: RequestStream<S, Bytes>,
    peer_addr: SocketAddr,
    rules: Arc<dyn RulesResolver>,
    _tls_config: Arc<TlsConfig>,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Arc<DnsResolver>,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let method = req.method().clone();
    let uri = req.uri().clone();
    let verbose = proxy_config.verbose_logging;

    if verbose {
        info!("HTTP/3 request from {}: {} {}", peer_addr, method, uri);
    }

    if method == Method::CONNECT {
        if super::is_connect_udp_request(&req) {
            if verbose {
                info!("HTTP/3 CONNECT-UDP request from {}: {}", peer_addr, uri);
            }
            super::handle_connect_udp(
                req,
                stream,
                peer_addr,
                rules,
                proxy_config,
                admin_state,
                dns_resolver,
            )
            .await
        } else {
            handle_h3_connect(
                req,
                stream,
                peer_addr,
                rules,
                proxy_config,
                admin_state,
                dns_resolver,
            )
            .await
        }
    } else {
        handle_h3_proxy_request(
            req,
            stream,
            peer_addr,
            rules,
            proxy_config,
            admin_state,
            dns_resolver,
        )
        .await
    }
}
