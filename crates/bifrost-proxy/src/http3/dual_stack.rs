use std::net::SocketAddr;
use std::sync::Arc;

use bifrost_admin::{AdminState, SharedPushManager};
use bifrost_core::{BifrostError, ClientAccessControl, Result};
use quinn::Endpoint;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::dns::DnsResolver;
use crate::server::{ProxyConfig, ProxyServer, RulesResolver, TlsConfig};

use super::{Http3Config, Http3Server};

pub struct DualStackServer {
    tcp_server: ProxyServer,
    h3_server: Option<Http3Server>,
}

impl DualStackServer {
    pub fn new(tcp_server: ProxyServer, h3_server: Option<Http3Server>) -> Self {
        Self {
            tcp_server,
            h3_server,
        }
    }

    pub async fn bind(&self, addr: SocketAddr) -> Result<(TcpListener, Option<Endpoint>)> {
        let tcp_listener = self.tcp_server.bind(addr).await?;

        let h3_endpoint = if let Some(ref h3_server) = self.h3_server {
            Some(h3_server.bind(addr).await?)
        } else {
            None
        };

        Ok((tcp_listener, h3_endpoint))
    }

    pub async fn run(&self) -> Result<()> {
        let addr: SocketAddr = format!(
            "{}:{}",
            self.tcp_server.config().host,
            self.tcp_server.config().port
        )
        .parse()
        .map_err(|e| BifrostError::Config(format!("Invalid address: {}", e)))?;

        let (tcp_listener, h3_endpoint) = self.bind(addr).await?;

        if let (Some(ref h3_server), Some(h3_endpoint)) = (&self.h3_server, h3_endpoint) {
            info!(
                "Dual-stack proxy server listening on {} (TCP + UDP/QUIC HTTP/3)",
                addr
            );

            tokio::select! {
                result = self.tcp_server.serve(tcp_listener) => {
                    if let Err(e) = result {
                        error!("TCP server error: {}", e);
                    }
                }
                result = h3_server.serve(h3_endpoint) => {
                    if let Err(e) = result {
                        error!("HTTP/3 server error: {}", e);
                    }
                }
            }
        } else {
            info!("Proxy server listening on {} (TCP only)", addr);
            self.tcp_server.serve(tcp_listener).await?;
        }

        Ok(())
    }

    pub fn tcp_server(&self) -> &ProxyServer {
        &self.tcp_server
    }

    pub fn h3_server(&self) -> Option<&Http3Server> {
        self.h3_server.as_ref()
    }

    pub fn is_http3_enabled(&self) -> bool {
        self.h3_server.is_some()
    }
}

pub struct DualStackServerBuilder {
    config: ProxyConfig,
    h3_config: Option<Http3Config>,
    rules: Option<Arc<dyn RulesResolver>>,
    tls_config: Option<Arc<TlsConfig>>,
    admin_state: Option<Arc<AdminState>>,
    push_manager: Option<SharedPushManager>,
    dns_resolver: Option<Arc<DnsResolver>>,
    access_control: Option<Arc<RwLock<ClientAccessControl>>>,
}

impl DualStackServerBuilder {
    pub fn new(config: ProxyConfig) -> Self {
        Self {
            config,
            h3_config: None,
            rules: None,
            tls_config: None,
            admin_state: None,
            push_manager: None,
            dns_resolver: None,
            access_control: None,
        }
    }

    pub fn with_http3_config(mut self, h3_config: Http3Config) -> Self {
        self.h3_config = Some(h3_config);
        self
    }

    pub fn with_rules(mut self, rules: Arc<dyn RulesResolver>) -> Self {
        self.rules = Some(rules);
        self
    }

    pub fn with_tls_config(mut self, tls_config: Arc<TlsConfig>) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

    pub fn with_admin_state(mut self, admin_state: Arc<AdminState>) -> Self {
        self.admin_state = Some(admin_state);
        self
    }

    pub fn with_push_manager(mut self, push_manager: SharedPushManager) -> Self {
        self.push_manager = Some(push_manager);
        self
    }

    pub fn with_dns_resolver(mut self, dns_resolver: Arc<DnsResolver>) -> Self {
        self.dns_resolver = Some(dns_resolver);
        self
    }

    pub fn with_access_control(mut self, access_control: Arc<RwLock<ClientAccessControl>>) -> Self {
        self.access_control = Some(access_control);
        self
    }

    pub fn build(self) -> DualStackServer {
        let tcp_server = self.build_tcp_server();

        let h3_server = self.h3_config.map(|h3_cfg| {
            let rules = self.rules.clone().unwrap_or_else(|| {
                Arc::new(crate::server::NoOpRulesResolver) as Arc<dyn RulesResolver>
            });
            let tls_config = self.tls_config.clone().unwrap_or_default();
            let dns_resolver = self
                .dns_resolver
                .clone()
                .unwrap_or_else(|| Arc::new(DnsResolver::new(self.config.verbose_logging)));
            let access_control = self
                .access_control
                .clone()
                .unwrap_or_else(|| Arc::new(RwLock::new(ClientAccessControl::default())));

            Http3Server::new(
                self.config.clone(),
                h3_cfg,
                rules,
                tls_config,
                self.admin_state.clone(),
                self.push_manager.clone(),
                dns_resolver,
                access_control,
            )
        });

        DualStackServer::new(tcp_server, h3_server)
    }

    fn build_tcp_server(&self) -> ProxyServer {
        let mut tcp_server = ProxyServer::new(self.config.clone());

        if let Some(rules) = self.rules.clone() {
            tcp_server = tcp_server.with_rules(rules);
        }
        if let Some(tls_config) = self.tls_config.clone() {
            tcp_server = tcp_server.with_tls_config(tls_config);
        }
        if let Some(push_manager) = self.push_manager.clone() {
            tcp_server = tcp_server.with_push_manager(push_manager);
        }

        tcp_server
    }
}
