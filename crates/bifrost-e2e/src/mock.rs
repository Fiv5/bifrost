use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_basic() {
        let server = EnhancedMockServer::start().await;
        assert!(server.port > 0);

        let client = reqwest::Client::new();
        let resp = client
            .get(&server.url("/test"))
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
        let resp = client.post(&server.url("/create")).send().await.unwrap();

        assert_eq!(resp.status().as_u16(), 201);
    }

    #[tokio::test]
    async fn test_mock_server_assertions() {
        let server = EnhancedMockServer::start().await;

        let client = reqwest::Client::new();
        client
            .post(&server.url("/api/users"))
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
