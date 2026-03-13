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
