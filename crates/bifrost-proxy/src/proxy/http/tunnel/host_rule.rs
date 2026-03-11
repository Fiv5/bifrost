use hyper::Uri;

pub(super) fn parse_host_rule(host_rule: &str) -> Option<(String, Option<u16>, Option<String>)> {
    let mut s = host_rule.trim();
    if s.is_empty() {
        return None;
    }

    // 兼容规则里携带 scheme（http/https/ws/wss/host/xhost/proxy/pac）以及可选路径、query。
    // 目标：稳定提取 host / port / path_and_query。
    for prefix in [
        "http://", "https://", "ws://", "wss://", "host://", "xhost://", "proxy://", "pac://",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest;
            break;
        }
    }

    // `Uri` 解析需要 scheme；这里统一补一个 http scheme，仅用于解析 authority / path。
    let uri: Uri = format!("http://{}", s).parse().ok()?;
    let authority = uri.authority()?;
    let host = authority.host().to_string();
    let port = authority.port_u16();
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .filter(|pq| pq != "/");

    Some((host, port, path_and_query))
}
