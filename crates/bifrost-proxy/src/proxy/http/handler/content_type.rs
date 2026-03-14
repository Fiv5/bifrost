use hyper::http::response::Parts as ResponseParts;

const STREAMING_CONTENT_TYPES: &[&str] = &[
    "video/x-flv",
    "video/mp4",
    "video/webm",
    "video/ogg",
    "video/mpeg",
    "video/mp2t",
    "application/x-mpegurl",
    "application/vnd.apple.mpegurl",
    "application/dash+xml",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "audio/aac",
    "text/event-stream",
    "application/octet-stream",
];

pub(super) fn is_likely_text_content_type(content_type: &str) -> bool {
    let ct = content_type.trim();
    if ct.is_empty() {
        return false;
    }
    if ct.starts_with("text/") {
        return true;
    }
    if ct.starts_with("application/json") {
        return true;
    }
    if ct.contains("+json") {
        return true;
    }
    if ct.starts_with("application/xml") || ct.contains("+xml") {
        return true;
    }
    if ct.starts_with("application/javascript")
        || ct.starts_with("application/x-javascript")
        || ct.starts_with("application/ecmascript")
    {
        return true;
    }
    if ct.starts_with("application/x-www-form-urlencoded") {
        return true;
    }
    false
}

pub(super) fn is_likely_binary_content_type(content_type: &str) -> bool {
    let ct = content_type.trim();
    if ct.is_empty() || is_likely_text_content_type(ct) {
        return false;
    }

    ct.starts_with("application/octet-stream")
        || ct.starts_with("application/pdf")
        || ct.starts_with("application/zip")
        || ct.starts_with("application/gzip")
        || ct.starts_with("application/x-gzip")
        || ct.starts_with("application/x-tar")
        || ct.starts_with("application/x-rar")
        || ct.starts_with("application/x-7z")
        || ct.starts_with("application/vnd.rar")
        || ct.starts_with("application/vnd.ms-cab-compressed")
        || ct.starts_with("application/x-bittorrent")
        || ct.starts_with("application/wasm")
        || ct.starts_with("application/font-")
        || ct.starts_with("application/vnd.ms-fontobject")
        || ct.starts_with("audio/")
        || ct.starts_with("video/")
        || ct.starts_with("font/")
        || ct.contains("protobuf")
        || ct.contains("grpc")
}

pub(super) fn should_use_binary_performance_mode(
    res_parts: &ResponseParts,
    binary_traffic_performance_mode: bool,
) -> bool {
    if !binary_traffic_performance_mode || is_sse_response(res_parts) {
        return false;
    }

    let content_type_lower = get_content_type(res_parts);
    if content_type_lower.starts_with("image/") {
        return false;
    }
    let has_attachment = res_parts
        .headers
        .get(hyper::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("attachment"))
        .unwrap_or(false);

    if !has_attachment && !is_likely_binary_content_type(&content_type_lower) {
        return false;
    }

    has_attachment || is_likely_binary_content_type(&content_type_lower)
}

pub(super) fn get_content_type(res_parts: &ResponseParts) -> String {
    res_parts
        .headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase()
}

pub(super) fn is_sse_response(res_parts: &ResponseParts) -> bool {
    get_content_type(res_parts).starts_with("text/event-stream")
}

pub(super) fn is_streaming_response(
    res_parts: &ResponseParts,
    res_content_length: Option<usize>,
    max_body_buffer_size: usize,
) -> bool {
    if is_sse_response(res_parts) {
        return true;
    }

    let content_type_lower = get_content_type(res_parts);
    if STREAMING_CONTENT_TYPES
        .iter()
        .any(|streaming_type| content_type_lower.starts_with(streaming_type))
    {
        return true;
    }

    let is_chunked = res_parts
        .headers
        .get(hyper::header::TRANSFER_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("chunked"))
        .unwrap_or(false);
    if is_chunked {
        return true;
    }

    match res_content_length {
        Some(len) => len > max_body_buffer_size,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::should_use_binary_performance_mode;
    use hyper::Response;

    #[test]
    fn image_responses_stay_recordable() {
        let response = Response::builder()
            .header(hyper::header::CONTENT_TYPE, "image/png")
            .header(hyper::header::CONTENT_LENGTH, "10485760")
            .body(())
            .unwrap();
        let (parts, _) = response.into_parts();

        assert!(!should_use_binary_performance_mode(&parts, true,));
    }

    #[test]
    fn octet_stream_uses_performance_mode_even_when_small() {
        let response = Response::builder()
            .header(hyper::header::CONTENT_TYPE, "application/octet-stream")
            .header(hyper::header::CONTENT_LENGTH, "128")
            .body(())
            .unwrap();
        let (parts, _) = response.into_parts();

        assert!(should_use_binary_performance_mode(&parts, true));
    }

    #[test]
    fn chunked_video_stream_uses_performance_mode() {
        let response = Response::builder()
            .header(hyper::header::CONTENT_TYPE, "video/mp2t")
            .header(hyper::header::TRANSFER_ENCODING, "chunked")
            .body(())
            .unwrap();
        let (parts, _) = response.into_parts();

        assert!(should_use_binary_performance_mode(&parts, true));
    }
}
