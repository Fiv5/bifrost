use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bifrost_admin::AdminState;
use bifrost_core::{BifrostError, Result};
use hyper::body::Incoming;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info};

use crate::logging::{format_rules_summary, RequestContext};
use crate::server::{empty_body, BoxBody, ProxyConfig, RulesResolver, TlsConfig};

pub async fn handle_connect(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    proxy_config: &ProxyConfig,
    verbose_logging: bool,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
) -> Result<Response<BoxBody>> {
    let uri = req.uri().clone();
    let authority = uri
        .authority()
        .ok_or_else(|| BifrostError::Network("CONNECT request missing authority".to_string()))?;

    let host = authority.host().to_string();
    let port = authority.port_u16().unwrap_or(443);

    if verbose_logging {
        debug!(
            "[{}] CONNECT tunnel request to {}:{}",
            ctx.id_str(),
            host,
            port
        );
    } else {
        debug!("CONNECT tunnel to {}:{}", host, port);
    }

    let url = format!("https://{}:{}", host, port);
    let resolved_rules = rules.resolve(&url, "CONNECT");

    let has_rules = resolved_rules.host.is_some() || !resolved_rules.rules.is_empty();
    if verbose_logging && has_rules {
        info!(
            "[{}] CONNECT rules matched: {}",
            ctx.id_str(),
            format_rules_summary(&resolved_rules)
        );
    }

    let (target_host, target_port) = if let Some(ref host_rule) = resolved_rules.host {
        let parts: Vec<&str> = host_rule.split(':').collect();
        let h = parts[0].to_string();
        let p = if parts.len() > 1 {
            parts[1].parse().unwrap_or(port)
        } else {
            port
        };
        if verbose_logging {
            info!(
                "[{}] CONNECT target redirected: {}:{} -> {}:{}",
                ctx.id_str(),
                host,
                port,
                h,
                p
            );
        }
        (h, p)
    } else {
        (host.clone(), port)
    };

    let should_intercept = proxy_config.enable_tls_interception
        && tls_config.ca_cert.is_some()
        && !is_domain_excluded(&host, &proxy_config.intercept_exclude);

    if should_intercept {
        if verbose_logging {
            debug!("[{}] TLS interception enabled", ctx.id_str());
        }
        return handle_tls_interception(
            req,
            &target_host,
            target_port,
            rules,
            tls_config,
            verbose_logging,
            ctx,
        )
        .await;
    } else if proxy_config.enable_tls_interception
        && tls_config.ca_cert.is_some()
        && verbose_logging
    {
        debug!(
            "[{}] TLS interception skipped (domain excluded): {}",
            ctx.id_str(),
            host
        );
    }

    let target_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                target_host, target_port, e
            ))
        })?;

    if verbose_logging {
        info!(
            "[{}] CONNECT tunnel established to {}:{}",
            ctx.id_str(),
            target_host,
            target_port
        );
    }

    let req_id = ctx.id_str();
    let verbose = verbose_logging;
    if let Some(ref state) = admin_state {
        state.metrics_collector.increment_connections();
    }
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                let result = tunnel_bidirectional(
                    upgraded,
                    target_stream,
                    verbose,
                    &req_id,
                    admin_state.as_ref(),
                )
                .await;
                if let Some(ref state) = admin_state {
                    state.metrics_collector.decrement_connections();
                }
                if let Err(e) = result {
                    error!("[{}] Tunnel error: {}", req_id, e);
                }
            }
            Err(e) => {
                if let Some(ref state) = admin_state {
                    state.metrics_collector.decrement_connections();
                }
                error!("[{}] Upgrade error: {}", req_id, e);
            }
        }
    });

    Ok(Response::builder().status(200).body(empty_body()).unwrap())
}

async fn handle_tls_interception(
    _req: Request<Incoming>,
    _host: &str,
    _port: u16,
    _rules: Arc<dyn RulesResolver>,
    _tls_config: Arc<TlsConfig>,
    _verbose_logging: bool,
    _ctx: &RequestContext,
) -> Result<Response<BoxBody>> {
    Ok(Response::builder().status(200).body(empty_body()).unwrap())
}

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

    let bytes_sent = Arc::new(AtomicU64::new(0));
    let bytes_received = Arc::new(AtomicU64::new(0));
    let bytes_sent_clone = bytes_sent.clone();
    let bytes_received_clone = bytes_received.clone();

    let client_to_target = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;
            bytes_sent_clone.fetch_add(n as u64, Ordering::Relaxed);
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(())
    };

    let target_to_client = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;
            bytes_received_clone.fetch_add(n as u64, Ordering::Relaxed);
        }
        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_target, target_to_client);

    if let Some(state) = admin_state {
        state
            .metrics_collector
            .add_bytes_sent(bytes_sent.load(Ordering::Relaxed));
        state
            .metrics_collector
            .add_bytes_received(bytes_received.load(Ordering::Relaxed));
    }

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

fn is_domain_excluded(host: &str, exclude_list: &[String]) -> bool {
    if exclude_list.is_empty() {
        return false;
    }

    let host_lower = host.to_lowercase();
    for pattern in exclude_list {
        let pattern_lower = pattern.to_lowercase();

        if let Some(base_domain) = pattern_lower.strip_prefix("*.") {
            let suffix = format!(".{}", base_domain);
            if host_lower.ends_with(&suffix) || host_lower == base_domain {
                return true;
            }
        } else if host_lower == pattern_lower
            || host_lower.ends_with(&format!(".{}", pattern_lower))
        {
            return true;
        }
    }

    false
}

pub fn parse_connect_authority(authority: &str) -> Result<(String, u16)> {
    let parts: Vec<&str> = authority.split(':').collect();
    match parts.len() {
        1 => Ok((parts[0].to_string(), 443)),
        2 => {
            let port = parts[1]
                .parse()
                .map_err(|_| BifrostError::Parse(format!("Invalid port: {}", parts[1])))?;
            Ok((parts[0].to_string(), port))
        }
        _ => Err(BifrostError::Parse(format!(
            "Invalid authority: {}",
            authority
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect_authority_with_port() {
        let (host, port) = parse_connect_authority("example.com:443").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_connect_authority_custom_port() {
        let (host, port) = parse_connect_authority("example.com:8443").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8443);
    }

    #[test]
    fn test_parse_connect_authority_default_port() {
        let (host, port) = parse_connect_authority("example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_connect_authority_invalid_port() {
        let result = parse_connect_authority("example.com:invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_connect_authority_multiple_colons() {
        let result = parse_connect_authority("example.com:443:extra");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_domain_excluded_exact_match() {
        let exclude = vec!["example.com".to_string()];
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(!is_domain_excluded("other.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_subdomain_match() {
        let exclude = vec!["example.com".to_string()];
        assert!(is_domain_excluded("sub.example.com", &exclude));
        assert!(is_domain_excluded("deep.sub.example.com", &exclude));
        assert!(!is_domain_excluded("notexample.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_wildcard() {
        let exclude = vec!["*.example.com".to_string()];
        assert!(is_domain_excluded("sub.example.com", &exclude));
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(!is_domain_excluded("other.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_case_insensitive() {
        let exclude = vec!["Example.COM".to_string()];
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(is_domain_excluded("EXAMPLE.COM", &exclude));
        assert!(is_domain_excluded("Sub.Example.Com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_empty_list() {
        let exclude: Vec<String> = vec![];
        assert!(!is_domain_excluded("example.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_multiple_patterns() {
        let exclude = vec![
            "example.com".to_string(),
            "*.google.com".to_string(),
            "internal.corp".to_string(),
        ];
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(is_domain_excluded("maps.google.com", &exclude));
        assert!(is_domain_excluded("api.internal.corp", &exclude));
        assert!(!is_domain_excluded("other.com", &exclude));
    }
}
