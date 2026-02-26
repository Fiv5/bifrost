pub fn calculate_request_size(
    method: &str,
    uri: &str,
    headers: &[(String, String)],
    body_len: usize,
) -> usize {
    let mut size = 0;
    size += method.len();
    size += 1;
    size += uri.len();
    size += " HTTP/1.1\r\n".len();

    for (name, value) in headers {
        size += name.len();
        size += ": ".len();
        size += value.len();
        size += "\r\n".len();
    }

    size += "\r\n".len();
    size += body_len;

    size
}

pub fn calculate_response_size(
    status_code: u16,
    headers: &[(String, String)],
    body_len: usize,
) -> usize {
    calculate_response_headers_size(status_code, headers) + body_len
}

pub fn calculate_response_headers_size(status_code: u16, headers: &[(String, String)]) -> usize {
    let mut size = 0;
    size += "HTTP/1.1 ".len();
    size += format!("{}", status_code).len();
    size += " ".len();
    size += status_reason(status_code).len();
    size += "\r\n".len();

    for (name, value) in headers {
        size += name.len();
        size += ": ".len();
        size += value.len();
        size += "\r\n".len();
    }

    size += "\r\n".len();

    size
}

fn status_reason(code: u16) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}
