mod common;

use bifrost_admin::{AdminState, BodyRef, TrafficDbStore, TrafficRecord};
use bifrost_tls::{generate_root_ca, init_crypto_provider, DynamicCertGenerator};
use std::sync::Arc;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("bifrost-tests-{}-{}-{}", name, pid, ts));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn test_dynamic_cert_chain_validity() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(Arc::clone(&ca));

    let domains = vec![
        "example.com",
        "api.example.com",
        "*.example.com",
        "localhost",
        "127.0.0.1",
    ];

    for domain in domains {
        let cert_key = generator
            .generate_for_domain(domain)
            .unwrap_or_else(|_| panic!("Failed to generate cert for {}", domain));

        assert_eq!(
            cert_key.cert.len(),
            2,
            "Certificate chain for {} should have 2 certs (leaf + CA)",
            domain
        );

        assert!(
            !cert_key.cert[0].as_ref().is_empty(),
            "Leaf certificate should not be empty"
        );
        assert!(
            !cert_key.cert[1].as_ref().is_empty(),
            "CA certificate should not be empty"
        );
    }
}

#[tokio::test]
async fn test_traffic_record_fields_complete() {
    let mut record = TrafficRecord::new(
        "test-id-001".to_string(),
        "POST".to_string(),
        "https://api.example.com/v1/users".to_string(),
    );

    record.status = 201;
    record.content_type = Some("application/json".to_string());
    record.request_size = 128;
    record.response_size = 256;
    record.duration_ms = 45;

    record.request_headers = Some(vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("authorization".to_string(), "Bearer token123".to_string()),
    ]);
    record.response_headers = Some(vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-request-id".to_string(), "req-abc123".to_string()),
    ]);

    record.request_body_ref = Some(BodyRef::Inline {
        data: r#"{"name":"test","email":"test@example.com"}"#.to_string(),
    });
    record.response_body_ref = Some(BodyRef::Inline {
        data: r#"{"id":1,"name":"test","created":true}"#.to_string(),
    });
    assert_eq!(record.id, "test-id-001");
    assert_eq!(record.method, "POST");
    assert_eq!(record.status, 201);
    assert_eq!(record.host, "api.example.com");
    assert_eq!(record.path, "/v1/users");
    assert_eq!(record.protocol, "https");

    assert!(record.request_headers.is_some());
    let req_headers = record.request_headers.take().unwrap();
    assert_eq!(req_headers.len(), 2);
    assert!(req_headers
        .iter()
        .any(|(k, v)| k == "content-type" && v == "application/json"));
    assert!(req_headers
        .iter()
        .any(|(k, v)| k == "authorization" && v == "Bearer token123"));

    assert!(record.response_headers.is_some());
    let res_headers = record.response_headers.take().unwrap();
    assert_eq!(res_headers.len(), 2);

    assert!(record.request_body_ref.is_some());
    if let Some(BodyRef::Inline { data }) = &record.request_body_ref {
        assert!(data.contains("test@example.com"));
    }

    assert!(record.response_body_ref.is_some());
    if let Some(BodyRef::Inline { data }) = &record.response_body_ref {
        assert!(data.contains("created"));
    }
}

#[tokio::test]
async fn test_https_traffic_protocol_detection() {
    let https_record = TrafficRecord::new(
        "https-001".to_string(),
        "GET".to_string(),
        "https://secure.example.com/api".to_string(),
    );
    assert_eq!(https_record.protocol, "https");
    assert_eq!(https_record.host, "secure.example.com");

    let http_record = TrafficRecord::new(
        "http-001".to_string(),
        "GET".to_string(),
        "http://example.com/api".to_string(),
    );
    assert_eq!(http_record.protocol, "http");
    assert_eq!(http_record.host, "example.com");
}

#[tokio::test]
async fn test_traffic_record_request_response_body() {
    let request_body = r#"{"username":"admin","password":"secret123"}"#;
    let response_body = r#"{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9","expires_in":3600}"#;

    let mut record = TrafficRecord::new(
        "body-test-001".to_string(),
        "POST".to_string(),
        "https://auth.example.com/login".to_string(),
    );

    record.status = 200;
    record.request_body_ref = Some(BodyRef::Inline {
        data: request_body.to_string(),
    });
    record.response_body_ref = Some(BodyRef::Inline {
        data: response_body.to_string(),
    });
    record.request_size = request_body.len();
    record.response_size = response_body.len();
    assert!(record.request_body_ref.is_some());
    if let Some(BodyRef::Inline { data }) = &record.request_body_ref {
        assert!(data.contains("username"));
        assert!(data.contains("admin"));
    }

    assert!(record.response_body_ref.is_some());
    if let Some(BodyRef::Inline { data }) = &record.response_body_ref {
        assert!(data.contains("token"));
        assert!(data.contains("expires_in"));
    }
}

#[tokio::test]
async fn test_traffic_record_headers_complete() {
    let mut record = TrafficRecord::new(
        "headers-test-001".to_string(),
        "GET".to_string(),
        "https://api.example.com/resource".to_string(),
    );

    record.status = 200;
    record.request_headers = Some(vec![
        ("host".to_string(), "api.example.com".to_string()),
        ("user-agent".to_string(), "Mozilla/5.0".to_string()),
        ("accept".to_string(), "application/json".to_string()),
        ("accept-language".to_string(), "en-US,en;q=0.9".to_string()),
        ("authorization".to_string(), "Bearer xyz123".to_string()),
        ("cookie".to_string(), "session=abc".to_string()),
    ]);
    record.response_headers = Some(vec![
        (
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        ),
        ("content-length".to_string(), "1234".to_string()),
        ("cache-control".to_string(), "no-cache".to_string()),
        (
            "set-cookie".to_string(),
            "session=def; HttpOnly; Secure".to_string(),
        ),
        ("x-request-id".to_string(), "req-12345".to_string()),
        ("x-ratelimit-remaining".to_string(), "99".to_string()),
    ]);
    let req_headers = record.request_headers.take().unwrap();
    assert_eq!(req_headers.len(), 6);
    assert!(req_headers
        .iter()
        .any(|(k, v)| k == "authorization" && v.contains("Bearer")));
    assert!(req_headers
        .iter()
        .any(|(k, v)| k == "user-agent" && v.contains("Mozilla")));

    let res_headers = record.response_headers.take().unwrap();
    assert_eq!(res_headers.len(), 6);
    assert!(res_headers
        .iter()
        .any(|(k, v)| k == "content-type" && v.contains("json")));
    assert!(res_headers
        .iter()
        .any(|(k, v)| k == "set-cookie" && v.contains("HttpOnly")));
}

#[tokio::test]
async fn test_traffic_record_large_body() {
    let large_request = "x".repeat(10000);
    let large_response = "y".repeat(50000);

    let mut record = TrafficRecord::new(
        "large-body-001".to_string(),
        "POST".to_string(),
        "https://upload.example.com/data".to_string(),
    );

    record.status = 200;
    record.request_body_ref = Some(BodyRef::Inline {
        data: large_request.clone(),
    });
    record.response_body_ref = Some(BodyRef::Inline {
        data: large_response.clone(),
    });
    record.request_size = large_request.len();
    record.response_size = large_response.len();
    assert_eq!(record.request_size, 10000);
    assert_eq!(record.response_size, 50000);
    assert!(record.request_body_ref.is_some());
    assert!(record.response_body_ref.is_some());
    if let Some(BodyRef::Inline { data }) = &record.request_body_ref {
        assert_eq!(data.len(), 10000);
    }
    if let Some(BodyRef::Inline { data }) = &record.response_body_ref {
        assert_eq!(data.len(), 50000);
    }
}

#[tokio::test]
async fn test_traffic_record_binary_body_as_none() {
    let mut record = TrafficRecord::new(
        "binary-body-001".to_string(),
        "GET".to_string(),
        "https://cdn.example.com/image.png".to_string(),
    );

    record.status = 200;
    record.content_type = Some("image/png".to_string());
    record.request_body_ref = None;
    record.response_body_ref = None;
    record.request_size = 0;
    record.response_size = 1024000;
    assert!(record.request_body_ref.is_none());
    assert!(record.response_body_ref.is_none());
    assert_eq!(record.response_size, 1024000);
}

#[tokio::test]
async fn test_admin_state_traffic_db_integration() {
    let dir = temp_dir("admin-state-traffic-db");
    let traffic_dir = dir.join("traffic");
    std::fs::create_dir_all(&traffic_dir).unwrap();
    let store =
        TrafficDbStore::new(traffic_dir, 1000, 0, Some(24)).expect("create traffic db store");
    let admin_state = AdminState::new(8080).with_traffic_db_store(store);

    let mut record1 = TrafficRecord::new(
        "admin-test-001".to_string(),
        "GET".to_string(),
        "https://api.example.com/users".to_string(),
    );
    record1.status = 200;
    record1.request_headers = Some(vec![("accept".to_string(), "application/json".to_string())]);
    record1.response_headers = Some(vec![(
        "content-type".to_string(),
        "application/json".to_string(),
    )]);
    record1.response_body_ref = Some(BodyRef::Inline {
        data: r#"[{"id":1,"name":"user1"},{"id":2,"name":"user2"}]"#.to_string(),
    });
    admin_state.record_traffic(record1);

    let mut record2 = TrafficRecord::new(
        "admin-test-002".to_string(),
        "POST".to_string(),
        "https://api.example.com/users".to_string(),
    );
    record2.status = 201;
    record2.request_headers = Some(vec![(
        "content-type".to_string(),
        "application/json".to_string(),
    )]);
    record2.request_body_ref = Some(BodyRef::Inline {
        data: r#"{"name":"newuser"}"#.to_string(),
    });
    record2.response_body_ref = Some(BodyRef::Inline {
        data: r#"{"id":3,"name":"newuser"}"#.to_string(),
    });
    admin_state.record_traffic(record2);

    let db_store = admin_state.traffic_db_store.as_ref().unwrap();
    assert_eq!(db_store.stats().record_count, 2);

    let record1_retrieved = db_store.get_by_id("admin-test-001").unwrap();
    assert_eq!(record1_retrieved.method, "GET");
    if let Some(BodyRef::Inline { data }) = &record1_retrieved.response_body_ref {
        assert!(data.contains("user1"));
    }

    let record2_retrieved = db_store.get_by_id("admin-test-002").unwrap();
    assert_eq!(record2_retrieved.method, "POST");
    if let Some(BodyRef::Inline { data }) = &record2_retrieved.request_body_ref {
        assert!(data.contains("newuser"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn test_cert_generator_caching() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(Arc::clone(&ca));

    let domain = "cache-test.example.com";

    let cert1 = generator
        .generate_for_domain(domain)
        .expect("First generation failed");
    let cert2 = generator
        .generate_for_domain(domain)
        .expect("Second generation failed");

    assert_eq!(cert1.cert.len(), 2);
    assert_eq!(cert2.cert.len(), 2);
}

#[tokio::test]
async fn test_traffic_record_all_url_components() {
    let test_cases = [
        ("https://example.com/path", "https", "example.com", "/path"),
        (
            "http://api.test.com:8080/v1/users",
            "http",
            "api.test.com",
            "/v1/users",
        ),
        (
            "https://secure.site.org/api/data?key=value",
            "https",
            "secure.site.org",
            "/api/data",
        ),
        ("http://localhost:3000/", "http", "localhost", "/"),
    ];

    for (i, (url, expected_protocol, expected_host, expected_path)) in test_cases.iter().enumerate()
    {
        let record = TrafficRecord::new(
            format!("url-test-{}", i),
            "GET".to_string(),
            url.to_string(),
        );
        assert_eq!(
            record.protocol, *expected_protocol,
            "Protocol mismatch for {}",
            url
        );
        assert_eq!(record.host, *expected_host, "Host mismatch for {}", url);
        assert_eq!(record.path, *expected_path, "Path mismatch for {}", url);
    }
}
