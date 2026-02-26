use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use bifrost_core::{BifrostError, Result};
use bytes::{Buf, Bytes};
use h3::client::SendRequest;
use hyper::{Request, Response};
use quinn::{ClientConfig, Endpoint};
use tracing::{debug, info, warn};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_SIZE: usize = 100 * 1024 * 1024;

pub struct Http3Client {
    endpoint: Endpoint,
}

impl Http3Client {
    pub fn new() -> Result<Self> {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let mut crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        crypto.alpn_protocols = vec![b"h3".to_vec()];

        let quic_config =
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto).map_err(|e| {
                BifrostError::Tls(format!("Failed to create QUIC client config: {}", e))
            })?;

        let mut client_config = ClientConfig::new(Arc::new(quic_config));

        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
        transport_config.keep_alive_interval(Some(Duration::from_secs(10)));
        client_config.transport_config(Arc::new(transport_config));

        let mut endpoint = Endpoint::client("[::]:0".parse().unwrap())
            .map_err(|e| BifrostError::Network(format!("Failed to create QUIC endpoint: {}", e)))?;

        endpoint.set_default_client_config(client_config);

        info!("[HTTP/3 Client] Initialized with QUIC endpoint");
        Ok(Self { endpoint })
    }

    pub async fn request(
        &self,
        host: &str,
        port: u16,
        req: Request<Bytes>,
    ) -> Result<Response<Bytes>> {
        let addr = format!("{}:{}", host, port)
            .to_socket_addrs()
            .map_err(|e| {
                BifrostError::Network(format!("DNS resolution failed for {}: {}", host, e))
            })?
            .next()
            .ok_or_else(|| BifrostError::Network(format!("No addresses found for {}", host)))?;

        info!("[HTTP/3 Client] Connecting to {} ({}) via QUIC", host, addr);

        let connection = tokio::time::timeout(CONNECT_TIMEOUT, async {
            self.endpoint
                .connect(addr, host)
                .map_err(|e| BifrostError::Network(format!("QUIC connect error: {}", e)))?
                .await
                .map_err(|e| BifrostError::Network(format!("QUIC connection failed: {}", e)))
        })
        .await
        .map_err(|_| BifrostError::Network(format!("Connection timeout to {}", host)))??;

        info!(
            "[HTTP/3 Client] QUIC connection established to {} (RTT: {:?})",
            host,
            connection.rtt()
        );

        let quinn_conn = h3_quinn::Connection::new(connection);
        let (mut driver, send_request) = h3::client::new(quinn_conn)
            .await
            .map_err(|e| BifrostError::Network(format!("H3 handshake failed: {}", e)))?;

        info!("[HTTP/3 Client] HTTP/3 connection ready to {}", host);

        let response: Response<Bytes> = tokio::select! {
            result = Self::send_request(send_request, req.clone()) => result?,
            result = async { driver.wait_idle().await; Err::<Response<Bytes>, BifrostError>(BifrostError::Network("Connection closed".to_string())) } => result?,
        };

        info!(
            "[HTTP/3 Client] Response from {}: {} {}",
            host,
            response.status(),
            req.uri()
        );

        Ok(response)
    }

    async fn send_request(
        mut send_request: SendRequest<h3_quinn::OpenStreams, Bytes>,
        req: Request<Bytes>,
    ) -> Result<Response<Bytes>> {
        let uri = req.uri().clone();
        let method = req.method().clone();
        let body = req.body().clone();

        debug!(
            "[HTTP/3 Client] Sending request: {} {} (body: {} bytes)",
            method,
            uri,
            body.len()
        );

        let (parts, _) = req.into_parts();
        let h3_req = Request::from_parts(parts, ());

        let mut stream = send_request
            .send_request(h3_req)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to send H3 request: {}", e)))?;

        if !body.is_empty() {
            stream
                .send_data(body)
                .await
                .map_err(|e| BifrostError::Network(format!("Failed to send H3 body: {}", e)))?;
        }

        stream
            .finish()
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to finish H3 request: {}", e)))?;

        debug!("[HTTP/3 Client] Request sent, waiting for response...");

        let response = stream
            .recv_response()
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to receive H3 response: {}", e)))?;

        let status = response.status();
        let headers = response.headers().clone();

        debug!(
            "[HTTP/3 Client] Received response headers: {} (headers: {})",
            status,
            headers.len()
        );

        let mut body_data = Vec::new();
        while let Some(mut data) = stream
            .recv_data()
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to receive H3 body: {}", e)))?
        {
            while data.has_remaining() {
                let chunk = data.chunk();
                body_data.extend_from_slice(chunk);
                data.advance(chunk.len());

                if body_data.len() > MAX_RESPONSE_SIZE {
                    warn!(
                        "[HTTP/3 Client] Response body too large, truncating at {} bytes",
                        MAX_RESPONSE_SIZE
                    );
                    break;
                }
            }
        }

        debug!(
            "[HTTP/3 Client] Received response body: {} bytes",
            body_data.len()
        );

        let mut builder = Response::builder().status(status);
        for (key, value) in headers.iter() {
            builder = builder.header(key, value);
        }

        let response = builder
            .body(Bytes::from(body_data))
            .map_err(|e| BifrostError::Parse(format!("Failed to build response: {}", e)))?;

        Ok(response)
    }

    pub async fn check_h3_support(host: &str, port: u16) -> Result<bool> {
        let client = Self::new()?;

        let req = Request::builder()
            .method("HEAD")
            .uri(format!("https://{}:{}/", host, port))
            .header("Host", host)
            .body(Bytes::new())
            .map_err(|e| BifrostError::Parse(format!("Failed to build request: {}", e)))?;

        match client.request(host, port, req).await {
            Ok(_) => {
                info!("[HTTP/3 Client] {} supports HTTP/3", host);
                Ok(true)
            }
            Err(e) => {
                debug!("[HTTP/3 Client] {} does not support HTTP/3: {}", host, e);
                Ok(false)
            }
        }
    }
}

impl Default for Http3Client {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP/3 client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_http3_client_xiaohongshu() {
        let client = Http3Client::new().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri("https://edith.xiaohongshu.com/api/sns/web/global/config")
            .header("Host", "edith.xiaohongshu.com")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            )
            .body(Bytes::new())
            .unwrap();

        let response = client.request("edith.xiaohongshu.com", 443, req).await;
        println!("Response: {:?}", response);
        assert!(response.is_ok());
    }
}
