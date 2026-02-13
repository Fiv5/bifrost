use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bifrost_core::{BifrostError, Result};
use hickory_resolver::config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

#[derive(Debug, Clone)]
pub enum DnsServerType {
    Standard { addr: SocketAddr },
    DoH { url: String },
    DoT { addr: SocketAddr, hostname: String },
}

impl DnsServerType {
    pub fn parse(server: &str) -> Result<Self> {
        if server.starts_with("https://") {
            Ok(DnsServerType::DoH {
                url: server.to_string(),
            })
        } else if let Some(rest) = server.strip_prefix("tls://") {
            let (host, port) = if let Some(colon_pos) = rest.rfind(':') {
                let port_str = &rest[colon_pos + 1..];
                if let Ok(port) = port_str.parse::<u16>() {
                    (&rest[..colon_pos], port)
                } else {
                    (rest, 853)
                }
            } else {
                (rest, 853)
            };

            let addr = if let Ok(ip) = IpAddr::from_str(host) {
                SocketAddr::new(ip, port)
            } else {
                return Err(BifrostError::Config(format!(
                    "DoT requires IP address, got hostname: {}",
                    host
                )));
            };

            Ok(DnsServerType::DoT {
                addr,
                hostname: host.to_string(),
            })
        } else {
            let (host, port) = if let Some(colon_pos) = server.rfind(':') {
                let port_str = &server[colon_pos + 1..];
                if let Ok(port) = port_str.parse::<u16>() {
                    (&server[..colon_pos], port)
                } else {
                    (server, 53)
                }
            } else {
                (server, 53)
            };

            let ip = IpAddr::from_str(host).map_err(|e| {
                BifrostError::Config(format!("Invalid DNS server IP '{}': {}", host, e))
            })?;

            Ok(DnsServerType::Standard {
                addr: SocketAddr::new(ip, port),
            })
        }
    }
}

pub struct DnsResolver {
    system_resolver: TokioAsyncResolver,
    custom_resolvers: RwLock<std::collections::HashMap<String, Arc<TokioAsyncResolver>>>,
    timeout: Duration,
    verbose_logging: bool,
}

impl DnsResolver {
    pub fn new(verbose_logging: bool) -> Self {
        let system_resolver =
            TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

        Self {
            system_resolver,
            custom_resolvers: RwLock::new(std::collections::HashMap::new()),
            timeout: Duration::from_secs(5),
            verbose_logging,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[instrument(skip(self), fields(host = %host, servers = ?dns_servers))]
    pub async fn resolve(&self, host: &str, dns_servers: &[String]) -> Result<Option<IpAddr>> {
        if let Ok(ip) = IpAddr::from_str(host) {
            debug!(
                target: "bifrost_proxy::dns",
                host = %host,
                "Host is already an IP address, skipping DNS resolution"
            );
            return Ok(Some(ip));
        }

        if dns_servers.is_empty() {
            debug!(
                target: "bifrost_proxy::dns",
                host = %host,
                "No custom DNS servers configured, using system resolver"
            );
            return Ok(None);
        }

        if self.verbose_logging {
            info!(
                target: "bifrost_proxy::dns",
                host = %host,
                servers = ?dns_servers,
                "Starting DNS resolution with custom servers"
            );
        }

        for (index, server_str) in dns_servers.iter().enumerate() {
            for server in server_str.split(',') {
                let server = server.trim();
                if server.is_empty() {
                    continue;
                }

                debug!(
                    target: "bifrost_proxy::dns",
                    server = %server,
                    attempt = index + 1,
                    total = dns_servers.len(),
                    "Trying DNS server"
                );

                match self.resolve_with_server(host, server).await {
                    Ok(ip) => {
                        info!(
                            target: "bifrost_proxy::dns",
                            host = %host,
                            server = %server,
                            resolved_ip = %ip,
                            "DNS resolution successful"
                        );
                        return Ok(Some(ip));
                    }
                    Err(e) => {
                        warn!(
                            target: "bifrost_proxy::dns",
                            host = %host,
                            server = %server,
                            error = %e,
                            "DNS server failed, trying next"
                        );
                    }
                }
            }
        }

        warn!(
            target: "bifrost_proxy::dns",
            host = %host,
            servers = ?dns_servers,
            "All custom DNS servers failed, falling back to system resolver"
        );

        Ok(None)
    }

    async fn resolve_with_server(&self, host: &str, server: &str) -> Result<IpAddr> {
        let server_type = DnsServerType::parse(server)?;

        match server_type {
            DnsServerType::Standard { addr } => self.resolve_standard(host, addr).await,
            DnsServerType::DoH { url } => self.resolve_doh(host, &url).await,
            DnsServerType::DoT { addr, hostname } => self.resolve_dot(host, addr, &hostname).await,
        }
    }

    async fn resolve_standard(&self, host: &str, addr: SocketAddr) -> Result<IpAddr> {
        let resolver = self.get_or_create_resolver(&addr.to_string()).await?;

        let lookup = tokio::time::timeout(self.timeout, resolver.lookup_ip(host))
            .await
            .map_err(|_| {
                BifrostError::Network(format!(
                    "DNS lookup timeout for {} using server {}",
                    host, addr
                ))
            })?
            .map_err(|e| {
                BifrostError::Network(format!(
                    "DNS lookup failed for {} using server {}: {}",
                    host, addr, e
                ))
            })?;

        lookup.iter().next().ok_or_else(|| {
            BifrostError::Network(format!(
                "No IP addresses found for {} using server {}",
                host, addr
            ))
        })
    }

    async fn resolve_doh(&self, host: &str, url: &str) -> Result<IpAddr> {
        debug!(
            target: "bifrost_proxy::dns",
            host = %host,
            url = %url,
            "DNS over HTTPS resolution (not yet implemented, falling back)"
        );

        Err(BifrostError::Network(format!(
            "DNS over HTTPS not yet implemented for {}",
            url
        )))
    }

    async fn resolve_dot(&self, host: &str, addr: SocketAddr, hostname: &str) -> Result<IpAddr> {
        debug!(
            target: "bifrost_proxy::dns",
            host = %host,
            addr = %addr,
            hostname = %hostname,
            "DNS over TLS resolution (not yet implemented, falling back)"
        );

        Err(BifrostError::Network(format!(
            "DNS over TLS not yet implemented for {}",
            addr
        )))
    }

    async fn get_or_create_resolver(&self, server_key: &str) -> Result<Arc<TokioAsyncResolver>> {
        {
            let cache = self.custom_resolvers.read().await;
            if let Some(resolver) = cache.get(server_key) {
                return Ok(resolver.clone());
            }
        }

        let addr: SocketAddr = server_key.parse().map_err(|e| {
            BifrostError::Config(format!(
                "Invalid DNS server address '{}': {}",
                server_key, e
            ))
        })?;

        let name_server = NameServerConfig::new(addr, Protocol::Udp);
        let mut config = ResolverConfig::new();
        config.add_name_server(name_server);

        let mut opts = ResolverOpts::default();
        opts.timeout = self.timeout;
        opts.attempts = 2;

        let resolver = Arc::new(TokioAsyncResolver::tokio(config, opts));

        {
            let mut cache = self.custom_resolvers.write().await;
            cache.insert(server_key.to_string(), resolver.clone());
        }

        debug!(
            target: "bifrost_proxy::dns",
            server = %server_key,
            "Created new DNS resolver for server"
        );

        Ok(resolver)
    }

    #[instrument(skip(self), fields(host = %host))]
    pub async fn resolve_with_system(&self, host: &str) -> Result<IpAddr> {
        if let Ok(ip) = IpAddr::from_str(host) {
            return Ok(ip);
        }

        debug!(
            target: "bifrost_proxy::dns",
            host = %host,
            "Resolving with system DNS"
        );

        let lookup = tokio::time::timeout(self.timeout, self.system_resolver.lookup_ip(host))
            .await
            .map_err(|_| BifrostError::Network(format!("System DNS lookup timeout for {}", host)))?
            .map_err(|e| {
                BifrostError::Network(format!("System DNS lookup failed for {}: {}", host, e))
            })?;

        lookup.iter().next().ok_or_else(|| {
            BifrostError::Network(format!(
                "No IP addresses found for {} using system DNS",
                host
            ))
        })
    }
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_dns_server() {
        let server = DnsServerType::parse("8.8.8.8").unwrap();
        match server {
            DnsServerType::Standard { addr } => {
                assert_eq!(addr.ip().to_string(), "8.8.8.8");
                assert_eq!(addr.port(), 53);
            }
            _ => panic!("Expected Standard DNS server"),
        }
    }

    #[test]
    fn test_parse_standard_dns_server_with_port() {
        let server = DnsServerType::parse("8.8.8.8:5353").unwrap();
        match server {
            DnsServerType::Standard { addr } => {
                assert_eq!(addr.ip().to_string(), "8.8.8.8");
                assert_eq!(addr.port(), 5353);
            }
            _ => panic!("Expected Standard DNS server"),
        }
    }

    #[test]
    fn test_parse_doh_server() {
        let server = DnsServerType::parse("https://dns.google/dns-query").unwrap();
        match server {
            DnsServerType::DoH { url } => {
                assert_eq!(url, "https://dns.google/dns-query");
            }
            _ => panic!("Expected DoH DNS server"),
        }
    }

    #[test]
    fn test_parse_dot_server() {
        let server = DnsServerType::parse("tls://8.8.8.8").unwrap();
        match server {
            DnsServerType::DoT { addr, .. } => {
                assert_eq!(addr.ip().to_string(), "8.8.8.8");
                assert_eq!(addr.port(), 853);
            }
            _ => panic!("Expected DoT DNS server"),
        }
    }

    #[test]
    fn test_parse_dot_server_with_port() {
        let server = DnsServerType::parse("tls://8.8.8.8:8853").unwrap();
        match server {
            DnsServerType::DoT { addr, .. } => {
                assert_eq!(addr.ip().to_string(), "8.8.8.8");
                assert_eq!(addr.port(), 8853);
            }
            _ => panic!("Expected DoT DNS server"),
        }
    }

    #[test]
    fn test_parse_invalid_dns_server() {
        let result = DnsServerType::parse("invalid-hostname");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolver_skip_ip_address() {
        let resolver = DnsResolver::new(false);
        let result = resolver.resolve("192.168.1.1", &[]).await.unwrap();
        assert_eq!(result, Some(IpAddr::from_str("192.168.1.1").unwrap()));
    }

    #[tokio::test]
    async fn test_resolver_empty_servers_returns_none() {
        let resolver = DnsResolver::new(false);
        let result = resolver.resolve("example.com", &[]).await.unwrap();
        assert!(result.is_none());
    }
}
