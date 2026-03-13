use bifrost_tls::{
    generate_root_ca, init_crypto_provider, DynamicCertGenerator, TlsConfig as BifrostTlsConfig,
};
use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio::net::UdpSocket;
use tokio_rustls::TlsAcceptor;

#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub timestamp: Instant,
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Clone)]
struct MockResponse {
    status: u16,
    body: String,
    headers: HashMap<String, String>,
}

impl Default for MockResponse {
    fn default() -> Self {
        Self {
            status: 200,
            body: "ok".to_string(),
            headers: HashMap::new(),
        }
    }
}

pub struct EnhancedMockServer {
    pub port: u16,
    pub addr: SocketAddr,
    requests: Arc<RwLock<Vec<RecordedRequest>>>,
    response: Arc<RwLock<MockResponse>>,
}

impl EnhancedMockServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(RwLock::new(Vec::new()));
        let response = Arc::new(RwLock::new(MockResponse::default()));

        let requests_clone = Arc::clone(&requests);
        let response_clone = Arc::clone(&response);

        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let io = TokioIo::new(stream);
                    let requests = Arc::clone(&requests_clone);
                    let response = Arc::clone(&response_clone);

                    tokio::spawn(async move {
                        let service = service_fn(|req: Request<hyper::body::Incoming>| {
                            let requests = Arc::clone(&requests);
                            let response = Arc::clone(&response);

                            async move {
                                let method = req.method().to_string();
                                let path = req.uri().path().to_string();
                                let query = req.uri().query().map(|s| s.to_string());

                                let headers: HashMap<String, String> = req
                                    .headers()
                                    .iter()
                                    .map(|(k, v)| {
                                        (k.to_string(), v.to_str().unwrap_or("").to_string())
                                    })
                                    .collect();

                                let body_bytes =
                                    match http_body_util::BodyExt::collect(req.into_body()).await {
                                        Ok(collected) => collected.to_bytes(),
                                        Err(_) => Bytes::new(),
                                    };
                                let body = if body_bytes.is_empty() {
                                    None
                                } else {
                                    Some(String::from_utf8_lossy(&body_bytes).to_string())
                                };

                                let recorded = RecordedRequest {
                                    timestamp: Instant::now(),
                                    method: method.clone(),
                                    path: path.clone(),
                                    query,
                                    headers: headers.clone(),
                                    body,
                                };
                                requests.write().push(recorded);

                                let resp = response.read();
                                let status =
                                    StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK);

                                let mut response_headers = resp.headers.clone();
                                response_headers
                                    .entry("Content-Type".to_string())
                                    .or_insert_with(|| "application/json".to_string());

                                let response_body = serde_json::json!({
                                    "status": "ok",
                                    "mock_response": resp.body,
                                    "received": {
                                        "method": method,
                                        "path": path,
                                        "headers": headers
                                    }
                                });

                                let mut builder = Response::builder().status(status);
                                for (k, v) in &response_headers {
                                    builder = builder.header(k.as_str(), v.as_str());
                                }

                                Ok::<_, hyper::Error>(
                                    builder
                                        .body(Full::new(Bytes::from(response_body.to_string())))
                                        .unwrap(),
                                )
                            }
                        });

                        let _ = http1::Builder::new().serve_connection(io, service).await;
                    });
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Self {
            port: addr.port(),
            addr,
            requests,
            response,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }

    pub fn get_requests(&self) -> Vec<RecordedRequest> {
        self.requests.read().clone()
    }

    pub fn last_request(&self) -> Option<RecordedRequest> {
        self.requests.read().last().cloned()
    }

    pub fn clear_requests(&self) {
        self.requests.write().clear();
    }

    pub fn request_count(&self) -> usize {
        self.requests.read().len()
    }

    pub fn set_response(&self, status: u16, body: &str) {
        let mut resp = self.response.write();
        resp.status = status;
        resp.body = body.to_string();
    }

    pub fn set_response_with_headers(
        &self,
        status: u16,
        body: &str,
        headers: HashMap<String, String>,
    ) {
        let mut resp = self.response.write();
        resp.status = status;
        resp.body = body.to_string();
        resp.headers = headers;
    }

    pub fn assert_request_received(&self) -> Result<RecordedRequest, String> {
        self.last_request()
            .ok_or_else(|| "No request received by mock server".to_string())
    }

    pub fn assert_header_received(&self, header: &str, expected: &str) -> Result<(), String> {
        let request = self.assert_request_received()?;
        let header_lower = header.to_lowercase();

        for (k, v) in &request.headers {
            if k.to_lowercase() == header_lower {
                if v == expected {
                    return Ok(());
                } else {
                    return Err(format!(
                        "Header '{}' expected '{}', got '{}'",
                        header, expected, v
                    ));
                }
            }
        }

        Err(format!(
            "Header '{}' not found in request. Available headers: {:?}",
            header,
            request.headers.keys().collect::<Vec<_>>()
        ))
    }

    pub fn assert_header_contains(&self, header: &str, substring: &str) -> Result<(), String> {
        let request = self.assert_request_received()?;
        let header_lower = header.to_lowercase();

        for (k, v) in &request.headers {
            if k.to_lowercase() == header_lower {
                if v.contains(substring) {
                    return Ok(());
                } else {
                    return Err(format!(
                        "Header '{}' does not contain '{}', value: '{}'",
                        header, substring, v
                    ));
                }
            }
        }

        Err(format!("Header '{}' not found in request", header))
    }

    pub fn assert_path(&self, expected: &str) -> Result<(), String> {
        let request = self.assert_request_received()?;
        if request.path == expected {
            Ok(())
        } else {
            Err(format!(
                "Path expected '{}', got '{}'",
                expected, request.path
            ))
        }
    }

    pub fn assert_method(&self, expected: &str) -> Result<(), String> {
        let request = self.assert_request_received()?;
        if request.method == expected {
            Ok(())
        } else {
            Err(format!(
                "Method expected '{}', got '{}'",
                expected, request.method
            ))
        }
    }
}

pub struct HttpsMockServer {
    pub port: u16,
    pub addr: SocketAddr,
    requests: Arc<RwLock<Vec<RecordedRequest>>>,
    response: Arc<RwLock<MockResponse>>,
}

impl HttpsMockServer {
    pub async fn start(domain: &str) -> Self {
        init_crypto_provider();
        let ca = Arc::new(generate_root_ca().expect("failed to generate test CA"));
        let cert_generator = DynamicCertGenerator::new(ca);
        let cert = cert_generator
            .generate_for_domain(domain)
            .expect("failed to generate test server certificate");
        let server_config = BifrostTlsConfig::build_server_config(&cert)
            .expect("failed to build HTTPS mock server config");

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(RwLock::new(Vec::new()));
        let response = Arc::new(RwLock::new(MockResponse::default()));
        let acceptor = TlsAcceptor::from(server_config);

        let requests_clone = Arc::clone(&requests);
        let response_clone = Arc::clone(&response);

        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let acceptor = acceptor.clone();
                    let requests = Arc::clone(&requests_clone);
                    let response = Arc::clone(&response_clone);

                    tokio::spawn(async move {
                        let Ok(stream) = acceptor.accept(stream).await else {
                            return;
                        };

                        let io = TokioIo::new(stream);
                        let service = service_fn(|req: Request<hyper::body::Incoming>| {
                            let requests = Arc::clone(&requests);
                            let response = Arc::clone(&response);

                            async move {
                                let method = req.method().to_string();
                                let path = req.uri().path().to_string();
                                let query = req.uri().query().map(|s| s.to_string());

                                let headers: HashMap<String, String> = req
                                    .headers()
                                    .iter()
                                    .map(|(k, v)| {
                                        (k.to_string(), v.to_str().unwrap_or("").to_string())
                                    })
                                    .collect();

                                let body_bytes =
                                    match http_body_util::BodyExt::collect(req.into_body()).await {
                                        Ok(collected) => collected.to_bytes(),
                                        Err(_) => Bytes::new(),
                                    };
                                let body = if body_bytes.is_empty() {
                                    None
                                } else {
                                    Some(String::from_utf8_lossy(&body_bytes).to_string())
                                };

                                requests.write().push(RecordedRequest {
                                    timestamp: Instant::now(),
                                    method: method.clone(),
                                    path: path.clone(),
                                    query,
                                    headers: headers.clone(),
                                    body,
                                });

                                let resp = response.read();
                                let status =
                                    StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK);

                                let mut response_headers = resp.headers.clone();
                                response_headers
                                    .entry("Content-Type".to_string())
                                    .or_insert_with(|| "application/json".to_string());

                                let response_body = serde_json::json!({
                                    "status": "ok",
                                    "mock_response": resp.body,
                                    "received": {
                                        "method": method,
                                        "path": path,
                                        "headers": headers
                                    }
                                });

                                let mut builder = Response::builder().status(status);
                                for (k, v) in &response_headers {
                                    builder = builder.header(k.as_str(), v.as_str());
                                }

                                Ok::<_, hyper::Error>(
                                    builder
                                        .body(Full::new(Bytes::from(response_body.to_string())))
                                        .unwrap(),
                                )
                            }
                        });

                        let _ = http1::Builder::new().serve_connection(io, service).await;
                    });
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Self {
            port: addr.port(),
            addr,
            requests,
            response,
        }
    }

    pub fn set_response(&self, status: u16, body: &str) {
        let mut resp = self.response.write();
        resp.status = status;
        resp.body = body.to_string();
    }

    pub fn request_count(&self) -> usize {
        self.requests.read().len()
    }

    pub fn assert_request_received(&self) -> Result<RecordedRequest, String> {
        self.requests
            .read()
            .last()
            .cloned()
            .ok_or_else(|| "No request received by HTTPS mock server".to_string())
    }
}

pub struct MockDnsServer {
    pub addr: SocketAddr,
    records: Arc<HashMap<String, IpAddr>>,
    queries: Arc<RwLock<Vec<String>>>,
}

impl MockDnsServer {
    pub async fn start(records: HashMap<String, IpAddr>) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        let records = Arc::new(
            records
                .into_iter()
                .map(|(host, ip)| (host.to_ascii_lowercase(), ip))
                .collect::<HashMap<_, _>>(),
        );
        let queries = Arc::new(RwLock::new(Vec::new()));

        let records_clone = Arc::clone(&records);
        let queries_clone = Arc::clone(&queries);

        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let Ok((len, peer)) = socket.recv_from(&mut buf).await else {
                    break;
                };
                let packet = &buf[..len];
                let Some((host, qtype, question_end)) = parse_dns_question(packet) else {
                    continue;
                };

                queries_clone.write().push(host.clone());
                let ip = records_clone.get(&host).copied();
                let response = build_dns_response(packet, question_end, qtype, ip);
                let _ = socket.send_to(&response, peer).await;
            }
        });

        Self {
            addr,
            records,
            queries,
        }
    }

    pub fn server(&self) -> String {
        self.addr.to_string()
    }

    pub fn query_count_for(&self, host: &str) -> usize {
        let host = host.to_ascii_lowercase();
        self.queries
            .read()
            .iter()
            .filter(|q| q.as_str() == host)
            .count()
    }

    pub fn assert_query_received(&self, host: &str) -> Result<(), String> {
        let count = self.query_count_for(host);
        if count > 0 {
            Ok(())
        } else {
            Err(format!(
                "DNS query for '{}' not observed. Seen: {:?}",
                host,
                self.queries.read().clone()
            ))
        }
    }

    pub fn records_len(&self) -> usize {
        self.records.len()
    }
}

fn parse_dns_question(packet: &[u8]) -> Option<(String, u16, usize)> {
    if packet.len() < 17 {
        return None;
    }

    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    if qdcount == 0 {
        return None;
    }

    let mut pos = 12usize;
    let mut labels = Vec::new();
    loop {
        let len = *packet.get(pos)? as usize;
        pos += 1;
        if len == 0 {
            break;
        }
        let label = packet.get(pos..pos + len)?;
        labels.push(String::from_utf8_lossy(label).to_ascii_lowercase());
        pos += len;
    }

    let qtype = u16::from_be_bytes([*packet.get(pos)?, *packet.get(pos + 1)?]);
    pos += 4; // qtype + qclass

    Some((labels.join("."), qtype, pos))
}

fn build_dns_response(
    packet: &[u8],
    question_end: usize,
    qtype: u16,
    ip: Option<IpAddr>,
) -> Vec<u8> {
    let question = &packet[12..question_end];
    let ipv4 = match ip {
        Some(IpAddr::V4(ipv4)) if qtype == 1 => Some(ipv4),
        _ => None,
    };

    let mut response = Vec::with_capacity(question_end + 32);
    response.extend_from_slice(&packet[0..2]); // transaction id
    response.extend_from_slice(&[0x81, 0x80]); // standard response, recursion available
    response.extend_from_slice(&packet[4..6]); // qdcount
    response.extend_from_slice(&(if ipv4.is_some() { 1u16 } else { 0u16 }).to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes()); // nscount
    response.extend_from_slice(&0u16.to_be_bytes()); // arcount
    response.extend_from_slice(question);

    if let Some(ipv4) = ipv4 {
        response.extend_from_slice(&[0xC0, 0x0C]); // pointer to qname
        response.extend_from_slice(&1u16.to_be_bytes()); // A
        response.extend_from_slice(&1u16.to_be_bytes()); // IN
        response.extend_from_slice(&60u32.to_be_bytes()); // TTL
        response.extend_from_slice(&4u16.to_be_bytes()); // RDLENGTH
        response.extend_from_slice(&ipv4.octets());
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_basic() {
        let server = EnhancedMockServer::start().await;
        assert!(server.port > 0);

        let client = reqwest::Client::new();
        let resp = client
            .get(server.url("/test"))
            .header("X-Custom", "value123")
            .send()
            .await
            .unwrap();

        assert!(resp.status().is_success());

        let request = server.last_request().unwrap();
        assert_eq!(request.path, "/test");
        assert_eq!(request.method, "GET");
        assert!(request.headers.contains_key("x-custom"));
    }

    #[tokio::test]
    async fn test_mock_server_custom_response() {
        let server = EnhancedMockServer::start().await;
        server.set_response(201, "created");

        let client = reqwest::Client::new();
        let resp = client.post(server.url("/create")).send().await.unwrap();

        assert_eq!(resp.status().as_u16(), 201);
    }

    #[tokio::test]
    async fn test_mock_server_assertions() {
        let server = EnhancedMockServer::start().await;

        let client = reqwest::Client::new();
        client
            .post(server.url("/api/users"))
            .header("Authorization", "Bearer token123")
            .send()
            .await
            .unwrap();

        server.assert_path("/api/users").unwrap();
        server.assert_method("POST").unwrap();
        server
            .assert_header_received("authorization", "Bearer token123")
            .unwrap();
    }
}
