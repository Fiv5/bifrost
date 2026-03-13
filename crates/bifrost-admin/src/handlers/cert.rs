use std::net::IpAddr;

use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::Serialize;

use super::{
    cors_preflight, error_response, full_body, json_response, method_not_allowed,
    public_response_builder, BoxBody,
};
use crate::state::SharedAdminState;

#[derive(Serialize)]
struct CertInfo {
    available: bool,
    local_ips: Vec<String>,
    download_urls: Vec<String>,
    qrcode_urls: Vec<String>,
}

pub async fn handle_cert(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/api/cert" | "/api/cert/" | "/api/cert/info" => match method {
            Method::GET => get_cert_info(req, state).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

pub async fn handle_cert_public(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/public/cert" | "/public/cert/" => match method {
            Method::GET => download_ca_cert(state).await,
            Method::OPTIONS => cors_preflight(),
            _ => method_not_allowed(),
        },
        "/public/cert/qrcode" | "/public/cert/qrcode/" => match method {
            Method::GET => get_cert_qrcode(req, state).await,
            Method::OPTIONS => cors_preflight(),
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn download_ca_cert(state: SharedAdminState) -> Response<BoxBody> {
    let cert_path = match &state.ca_cert_path {
        Some(path) => path,
        None => {
            return error_response(StatusCode::NOT_FOUND, "CA certificate not configured");
        }
    };

    if !cert_path.exists() {
        return error_response(StatusCode::NOT_FOUND, "CA certificate not found");
    }

    match std::fs::read(cert_path) {
        Ok(cert_data) => public_response_builder(StatusCode::OK)
            .header("Content-Type", "application/x-pem-file")
            .header(
                "Content-Disposition",
                "attachment; filename=\"bifrost-ca.crt\"",
            )
            .body(full_body(cert_data))
            .unwrap(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to read certificate: {}", e),
        ),
    }
}

async fn get_cert_qrcode(req: Request<Incoming>, _state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let ip_from_query = query.split('&').find_map(|pair| {
        let mut parts = pair.split('=');
        match (parts.next(), parts.next()) {
            (Some("ip"), Some(value)) => Some(urlencoding::decode(value).ok()?.into_owned()),
            _ => None,
        }
    });

    let host = ip_from_query.unwrap_or_else(|| {
        req.headers()
            .get(hyper::header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("127.0.0.1")
            .to_string()
    });

    let download_url = format!("http://{}/_bifrost/public/cert", host);

    let code = match QrCode::new(download_url.as_bytes()) {
        Ok(code) => code,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to generate QR code: {}", e),
            );
        }
    };

    let svg_string = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    public_response_builder(StatusCode::OK)
        .header("Content-Type", "image/svg+xml")
        .body(full_body(svg_string))
        .unwrap()
}

async fn get_cert_info(_req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let available = state
        .ca_cert_path
        .as_ref()
        .map(|p| p.exists())
        .unwrap_or(false);

    let local_ips = get_local_ips();
    let port = state.port();

    let download_urls: Vec<String> = local_ips
        .iter()
        .map(|ip| format!("http://{}:{}/_bifrost/public/cert", ip, port))
        .collect();

    let qrcode_urls: Vec<String> = local_ips
        .iter()
        .map(|ip| format!("http://{}:{}/_bifrost/public/cert/qrcode", ip, port))
        .collect();

    let info = CertInfo {
        available,
        local_ips,
        download_urls,
        qrcode_urls,
    };

    json_response(&info)
}

fn get_local_ips() -> Vec<String> {
    let mut ips = Vec::new();

    if let Ok(interfaces) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if interfaces.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = interfaces.local_addr() {
                ips.push(addr.ip().to_string());
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("hostname").arg("-I").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for ip_str in stdout.split_whitespace() {
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        if is_private_ip(&ip) && !ips.contains(&ip.to_string()) {
                            ips.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    if ips.is_empty() {
        ips.push("127.0.0.1".to_string());
    }

    ips
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private() || ipv4.is_loopback() || ipv4.is_link_local(),
        IpAddr::V6(ipv6) => ipv6.is_loopback(),
    }
}

pub async fn handle_proxy_public(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/public/proxy/qrcode" | "/public/proxy/qrcode/" => match method {
            Method::GET => get_proxy_qrcode(req, state).await,
            Method::OPTIONS => cors_preflight(),
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_proxy_qrcode(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let ip_from_query = query.split('&').find_map(|pair| {
        let mut parts = pair.split('=');
        match (parts.next(), parts.next()) {
            (Some("ip"), Some(value)) => Some(urlencoding::decode(value).ok()?.into_owned()),
            _ => None,
        }
    });

    let host = ip_from_query.unwrap_or_else(|| {
        req.headers()
            .get(hyper::header::HOST)
            .and_then(|h| h.to_str().ok())
            .map(|h| h.split(':').next().unwrap_or(h))
            .unwrap_or("127.0.0.1")
            .to_string()
    });

    let proxy_address = format!("{}:{}", host, state.port());

    let code = match QrCode::new(proxy_address.as_bytes()) {
        Ok(code) => code,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to generate QR code: {}", e),
            );
        }
    };

    let svg_string = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    public_response_builder(StatusCode::OK)
        .header("Content-Type", "image/svg+xml")
        .body(full_body(svg_string))
        .unwrap()
}
