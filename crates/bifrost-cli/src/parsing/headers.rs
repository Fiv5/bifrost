pub fn parse_header_value(value: &str) -> Option<Vec<(String, String)>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (content, use_colon) = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        (&trimmed[1..trimmed.len() - 1], true)
    } else {
        (trimmed, trimmed.contains('\n') || trimmed.contains(':'))
    };

    let mut headers = Vec::new();

    let delimiter = if content.contains('\n') { '\n' } else { ',' };
    for part in content.split(delimiter) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let separator = if use_colon { ':' } else { '=' };
        if let Some(pos) = part.find(separator) {
            let key = part[..pos].trim().to_string();
            let val = part[pos + 1..].trim().to_string();
            if !key.is_empty() {
                headers.push((key, val));
            }
        }
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

pub fn url_decode(s: &str) -> String {
    urlencoding::decode(s)
        .unwrap_or(std::borrow::Cow::Borrowed(s))
        .into_owned()
}

pub fn parse_cors_config(value: &str) -> bifrost_proxy::CorsConfig {
    let value = value.trim();
    if value.is_empty() || value == "*" || value.eq_ignore_ascii_case("enable") {
        return bifrost_proxy::CorsConfig::enable_all();
    }

    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(value) {
        let mut cors = bifrost_proxy::CorsConfig {
            enabled: true,
            ..Default::default()
        };

        if let Some(origin) = json_value.get("origin").and_then(|v| v.as_str()) {
            cors.origin = Some(origin.to_string());
        }
        if let Some(methods) = json_value.get("methods").and_then(|v| v.as_str()) {
            cors.methods = Some(methods.to_string());
        }
        if let Some(headers) = json_value.get("headers").and_then(|v| v.as_str()) {
            cors.headers = Some(headers.to_string());
        }
        if let Some(expose) = json_value
            .get("expose")
            .or_else(|| json_value.get("exposeHeaders"))
            .and_then(|v| v.as_str())
        {
            cors.expose_headers = Some(expose.to_string());
        }
        if let Some(creds) = json_value.get("credentials").and_then(|v| v.as_bool()) {
            cors.credentials = Some(creds);
        }
        if let Some(max_age) = json_value
            .get("maxAge")
            .or_else(|| json_value.get("maxage"))
        {
            if let Some(age) = max_age.as_u64() {
                cors.max_age = Some(age);
            } else if let Some(age_str) = max_age.as_str() {
                if let Ok(age) = age_str.parse::<u64>() {
                    cors.max_age = Some(age);
                }
            }
        }

        return cors;
    }

    bifrost_proxy::CorsConfig::enable_all()
}
