use hyper::{body::Incoming, Method, Request, Response, StatusCode};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::state::SharedAdminState;

pub async fn handle_metrics(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/api/metrics" | "/api/metrics/" => match method {
            Method::GET => get_current_metrics(state).await,
            _ => method_not_allowed(),
        },
        "/api/metrics/history" => match method {
            Method::GET => get_metrics_history(req, state).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_current_metrics(state: SharedAdminState) -> Response<BoxBody> {
    let metrics = state.metrics_collector.get_current();
    json_response(&metrics)
}

async fn get_metrics_history(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let limit = parse_limit(query);

    let history = state.metrics_collector.get_history(limit);
    json_response(&history)
}

fn parse_limit(query: &str) -> Option<usize> {
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == "limit" {
                return value.parse().ok();
            }
        }
    }
    None
}
