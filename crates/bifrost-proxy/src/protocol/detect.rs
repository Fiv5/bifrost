use super::{DetectionResult, Priority, TransportProtocol};

const HTTP2_PREFACE_PREFIX: &[u8] = b"PRI *";

const TLS_HANDSHAKE: u8 = 0x16;
const TLS_VERSION_MAJOR: u8 = 0x03;

const SOCKS5_VERSION: u8 = 0x05;
const SOCKS4_VERSION: u8 = 0x04;

const HTTP_METHODS: &[&[u8]] = &[
    b"GET ",
    b"POST ",
    b"PUT ",
    b"DELETE ",
    b"HEAD ",
    b"OPTIONS ",
    b"PATCH ",
    b"CONNECT ",
    b"TRACE ",
];

pub struct ProtocolDetector;

impl ProtocolDetector {
    pub fn detect_transport(data: &[u8]) -> DetectionResult {
        if data.is_empty() {
            return DetectionResult::NeedMoreData(1);
        }

        if let result @ DetectionResult::Match(_) = Self::detect_http2(data) {
            return result;
        }

        if let result @ DetectionResult::Match(_) = Self::detect_tls(data) {
            return result;
        }

        if let result @ DetectionResult::Match(_) = Self::detect_socks(data) {
            return result;
        }

        if let result @ DetectionResult::Match(_) = Self::detect_http1(data) {
            return result;
        }

        DetectionResult::NotMatch
    }

    pub fn detect_protocol_type(data: &[u8]) -> Option<TransportProtocol> {
        if data.is_empty() {
            return None;
        }

        if Self::is_http2_preface(data) {
            return Some(TransportProtocol::Http2);
        }

        if Self::is_tls_handshake(data) {
            return Some(TransportProtocol::Tls);
        }

        if data[0] == SOCKS5_VERSION && data.len() >= 2 {
            return Some(TransportProtocol::Socks5);
        }

        if data[0] == SOCKS4_VERSION && data.len() >= 2 {
            return Some(TransportProtocol::Socks4);
        }

        if Self::is_http_method(data) {
            return Some(TransportProtocol::Http1);
        }

        None
    }

    fn detect_http2(data: &[u8]) -> DetectionResult {
        if data.len() < HTTP2_PREFACE_PREFIX.len() {
            if data.starts_with(&HTTP2_PREFACE_PREFIX[..data.len()]) {
                return DetectionResult::NeedMoreData(HTTP2_PREFACE_PREFIX.len() - data.len());
            }
            return DetectionResult::NotMatch;
        }

        if Self::is_http2_preface(data) {
            return DetectionResult::Match(Priority::HIGHEST);
        }

        DetectionResult::NotMatch
    }

    fn detect_tls(data: &[u8]) -> DetectionResult {
        if data.len() < 3 {
            return DetectionResult::NeedMoreData(3 - data.len());
        }

        if Self::is_tls_handshake(data) {
            return DetectionResult::Match(Priority::HIGH);
        }

        DetectionResult::NotMatch
    }

    fn detect_socks(data: &[u8]) -> DetectionResult {
        if data.is_empty() {
            return DetectionResult::NeedMoreData(1);
        }

        if data[0] == SOCKS5_VERSION {
            if data.len() < 2 {
                return DetectionResult::NeedMoreData(1);
            }
            return DetectionResult::Match(Priority::HIGH);
        }

        if data[0] == SOCKS4_VERSION {
            if data.len() < 2 {
                return DetectionResult::NeedMoreData(1);
            }
            return DetectionResult::Match(Priority::HIGH);
        }

        DetectionResult::NotMatch
    }

    fn detect_http1(data: &[u8]) -> DetectionResult {
        if data.len() < 4 {
            return DetectionResult::NeedMoreData(4 - data.len());
        }

        if Self::is_http_method(data) {
            return DetectionResult::Match(Priority::NORMAL);
        }

        DetectionResult::NotMatch
    }

    fn is_http2_preface(data: &[u8]) -> bool {
        data.starts_with(HTTP2_PREFACE_PREFIX)
    }

    fn is_tls_handshake(data: &[u8]) -> bool {
        data.len() >= 3 && data[0] == TLS_HANDSHAKE && data[1] == TLS_VERSION_MAJOR
    }

    fn is_http_method(data: &[u8]) -> bool {
        HTTP_METHODS.iter().any(|method| data.starts_with(method))
    }

    pub fn is_websocket_upgrade(headers: &[(String, String)]) -> bool {
        let has_upgrade_connection = headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("connection") && v.to_lowercase().contains("upgrade")
        });

        let has_websocket_upgrade = headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("upgrade") && v.eq_ignore_ascii_case("websocket"));

        has_upgrade_connection && has_websocket_upgrade
    }

    pub fn is_sse_request(headers: &[(String, String)]) -> bool {
        headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("accept") && v.contains("text/event-stream"))
    }

    pub fn is_sse_response(headers: &[(String, String)]) -> bool {
        headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("content-type") && v.contains("text/event-stream"))
    }

    pub fn is_grpc_request(headers: &[(String, String)]) -> bool {
        headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("content-type") && v.starts_with("application/grpc")
        })
    }

    pub fn is_chunked_transfer(headers: &[(String, String)]) -> bool {
        headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("transfer-encoding") && v.to_lowercase().contains("chunked")
        })
    }

    pub fn get_content_length(headers: &[(String, String)]) -> Option<usize> {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse().ok())
    }
}

pub fn parse_http_headers(data: &[u8]) -> Option<(Vec<(String, String)>, usize)> {
    let header_end = find_header_end(data)?;
    let header_str = std::str::from_utf8(&data[..header_end]).ok()?;

    let mut headers = Vec::new();
    let lines: Vec<&str> = header_str.lines().skip(1).collect();

    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            headers.push((key, value));
        }
    }

    Some((headers, header_end + 4))
}

pub fn find_header_end(data: &[u8]) -> Option<usize> {
    (0..data.len().saturating_sub(3)).find(|&i| &data[i..i + 4] == b"\r\n\r\n")
}

pub fn extract_request_line(data: &[u8]) -> Option<(String, String, String)> {
    let header_end = find_header_end(data).unwrap_or(data.len());
    let header_str = std::str::from_utf8(&data[..header_end]).ok()?;
    let first_line = header_str.lines().next()?;

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 3 {
        Some((
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http1_get() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(
            ProtocolDetector::detect_http1(data),
            DetectionResult::Match(Priority::NORMAL)
        );
    }

    #[test]
    fn test_detect_http1_post() {
        let data = b"POST /api HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(
            ProtocolDetector::detect_http1(data),
            DetectionResult::Match(Priority::NORMAL)
        );
    }

    #[test]
    fn test_detect_http1_connect() {
        let data = b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(
            ProtocolDetector::detect_http1(data),
            DetectionResult::Match(Priority::NORMAL)
        );
    }

    #[test]
    fn test_detect_http2_preface() {
        let data = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        assert_eq!(
            ProtocolDetector::detect_http2(data),
            DetectionResult::Match(Priority::HIGHEST)
        );
    }

    #[test]
    fn test_detect_http2_partial() {
        let data = b"PRI ";
        assert_eq!(
            ProtocolDetector::detect_http2(data),
            DetectionResult::NeedMoreData(1)
        );
    }

    #[test]
    fn test_detect_tls() {
        let data = [0x16, 0x03, 0x01, 0x00, 0x05];
        assert_eq!(
            ProtocolDetector::detect_tls(&data),
            DetectionResult::Match(Priority::HIGH)
        );
    }

    #[test]
    fn test_detect_socks5() {
        let data = [0x05, 0x01, 0x00];
        assert_eq!(
            ProtocolDetector::detect_socks(&data),
            DetectionResult::Match(Priority::HIGH)
        );
    }

    #[test]
    fn test_detect_socks4() {
        let data = [0x04, 0x01, 0x00, 0x50];
        assert_eq!(
            ProtocolDetector::detect_socks(&data),
            DetectionResult::Match(Priority::HIGH)
        );
    }

    #[test]
    fn test_detect_protocol_type() {
        assert_eq!(
            ProtocolDetector::detect_protocol_type(b"GET / HTTP/1.1"),
            Some(TransportProtocol::Http1)
        );
        assert_eq!(
            ProtocolDetector::detect_protocol_type(b"PRI * HTTP/2.0"),
            Some(TransportProtocol::Http2)
        );
        assert_eq!(
            ProtocolDetector::detect_protocol_type(&[0x16, 0x03, 0x01]),
            Some(TransportProtocol::Tls)
        );
        assert_eq!(
            ProtocolDetector::detect_protocol_type(&[0x05, 0x01]),
            Some(TransportProtocol::Socks5)
        );
        assert_eq!(
            ProtocolDetector::detect_protocol_type(&[0x04, 0x01]),
            Some(TransportProtocol::Socks4)
        );
    }

    #[test]
    fn test_is_websocket_upgrade() {
        let headers = vec![
            ("Connection".to_string(), "Upgrade".to_string()),
            ("Upgrade".to_string(), "websocket".to_string()),
        ];
        assert!(ProtocolDetector::is_websocket_upgrade(&headers));

        let headers = vec![("Connection".to_string(), "keep-alive".to_string())];
        assert!(!ProtocolDetector::is_websocket_upgrade(&headers));
    }

    #[test]
    fn test_is_sse_request() {
        let headers = vec![("Accept".to_string(), "text/event-stream".to_string())];
        assert!(ProtocolDetector::is_sse_request(&headers));

        let headers = vec![("Accept".to_string(), "application/json".to_string())];
        assert!(!ProtocolDetector::is_sse_request(&headers));
    }

    #[test]
    fn test_is_sse_response() {
        let headers = vec![("Content-Type".to_string(), "text/event-stream".to_string())];
        assert!(ProtocolDetector::is_sse_response(&headers));

        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        assert!(!ProtocolDetector::is_sse_response(&headers));
    }

    #[test]
    fn test_is_grpc_request() {
        let headers = vec![(
            "Content-Type".to_string(),
            "application/grpc+proto".to_string(),
        )];
        assert!(ProtocolDetector::is_grpc_request(&headers));

        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        assert!(!ProtocolDetector::is_grpc_request(&headers));
    }

    #[test]
    fn test_is_chunked_transfer() {
        let headers = vec![("Transfer-Encoding".to_string(), "chunked".to_string())];
        assert!(ProtocolDetector::is_chunked_transfer(&headers));

        let headers = vec![("Content-Length".to_string(), "100".to_string())];
        assert!(!ProtocolDetector::is_chunked_transfer(&headers));
    }

    #[test]
    fn test_get_content_length() {
        let headers = vec![("Content-Length".to_string(), "1024".to_string())];
        assert_eq!(ProtocolDetector::get_content_length(&headers), Some(1024));

        let headers = vec![("Transfer-Encoding".to_string(), "chunked".to_string())];
        assert_eq!(ProtocolDetector::get_content_length(&headers), None);
    }

    #[test]
    fn test_find_header_end() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\nbody";
        assert_eq!(find_header_end(data), Some(33));

        let data = b"GET / HTTP/1.1\r\nHost: example.com";
        assert_eq!(find_header_end(data), None);
    }

    #[test]
    fn test_parse_http_headers() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\nAccept: */*\r\n\r\n";
        let (headers, end) = parse_http_headers(data).unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], ("Host".to_string(), "example.com".to_string()));
        assert_eq!(headers[1], ("Accept".to_string(), "*/*".to_string()));
        assert_eq!(end, 50);
    }

    #[test]
    fn test_extract_request_line() {
        let data = b"GET /path HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let (method, path, version) = extract_request_line(data).unwrap();
        assert_eq!(method, "GET");
        assert_eq!(path, "/path");
        assert_eq!(version, "HTTP/1.1");
    }
}
