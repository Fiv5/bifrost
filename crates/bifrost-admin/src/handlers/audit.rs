use hyper::{Method, Request, Response, StatusCode};
use serde::Serialize;

use super::{cors_preflight, error_response, json_response, method_not_allowed, BoxBody};
use crate::admin_audit::{count_logins, list_logins, AdminLoginAuditEntry};

#[derive(Debug, Serialize)]
pub struct AuditListResponse {
    pub total: i64,
    pub items: Vec<AdminLoginAuditEntry>,
    pub limit: usize,
    pub offset: usize,
}

fn parse_usize_query(uri: &hyper::Uri, key: &str) -> Option<usize> {
    let q = uri.query()?;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?.trim();
        if k != key {
            continue;
        }
        let v = it.next().unwrap_or("");
        return v.parse::<usize>().ok();
    }
    None
}

pub async fn handle_audit<B>(req: Request<B>, _path: &str) -> Response<BoxBody>
where
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: std::error::Error + Send + Sync,
{
    if req.method() == Method::OPTIONS {
        return cors_preflight();
    }

    if *req.method() != Method::GET {
        return method_not_allowed();
    }

    let mut limit = parse_usize_query(req.uri(), "limit").unwrap_or(50);
    let offset = parse_usize_query(req.uri(), "offset").unwrap_or(0);
    if limit == 0 {
        limit = 50;
    }
    limit = limit.min(500);

    let total = match count_logins() {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to count audit logs: {e}"),
            )
        }
    };
    let items = match list_logins(limit, offset) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to query audit logs: {e}"),
            )
        }
    };

    json_response(&AuditListResponse {
        total,
        items,
        limit,
        offset,
    })
}

