use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use sha1::{Digest, Sha1};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, error, info, warn};

use super::{error_response, BoxBody};
use crate::push::{ClientSubscription, ConnectedData, PushMessage, SharedPushManager};

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub async fn handle_websocket_upgrade(
    req: Request<Incoming>,
    push_manager: SharedPushManager,
) -> Response<BoxBody> {
    let upgrade_header = req
        .headers()
        .get("Upgrade")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !upgrade_header.eq_ignore_ascii_case("websocket") {
        return error_response(StatusCode::BAD_REQUEST, "Invalid upgrade header");
    }

    let ws_key = match req.headers().get("Sec-WebSocket-Key") {
        Some(key) => key.to_str().unwrap_or("").to_string(),
        None => {
            return error_response(StatusCode::BAD_REQUEST, "Missing Sec-WebSocket-Key header");
        }
    };

    let query = req.uri().query().unwrap_or("");
    let subscription = parse_subscription_from_query(query);

    let accept_key = generate_accept_key(&ws_key);

    tokio::spawn(async move {
        let upgraded = match hyper::upgrade::on(req).await {
            Ok(u) => u,
            Err(e) => {
                error!("WebSocket upgrade failed: {}", e);
                return;
            }
        };

        let ws_stream = WebSocketStream::from_raw_socket(
            hyper_util::rt::TokioIo::new(upgraded),
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        )
        .await;

        handle_websocket_connection(ws_stream, push_manager, subscription).await;
    });

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Accept", accept_key)
        .header("Access-Control-Allow-Origin", "*")
        .body(BoxBody::default())
        .unwrap()
}

fn generate_accept_key(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
}

fn parse_subscription_from_query(query: &str) -> ClientSubscription {
    let mut subscription = ClientSubscription::default();

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "last_traffic_id" => {
                    if !value.is_empty() {
                        subscription.last_traffic_id = Some(value.to_string());
                    }
                }
                "pending_ids" => {
                    if !value.is_empty() {
                        subscription.pending_ids =
                            value.split(',').map(|s| s.to_string()).collect();
                    }
                }
                "need_overview" => {
                    subscription.need_overview = value == "true" || value == "1";
                }
                "need_metrics" => {
                    subscription.need_metrics = value == "true" || value == "1";
                }
                "need_history" => {
                    subscription.need_history = value == "true" || value == "1";
                }
                "history_limit" => {
                    if let Ok(limit) = value.parse() {
                        subscription.history_limit = limit;
                    }
                }
                _ => {}
            }
        }
    }

    subscription
}

async fn handle_websocket_connection<S>(
    ws_stream: WebSocketStream<S>,
    push_manager: SharedPushManager,
    subscription: ClientSubscription,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let (client, mut msg_receiver) = push_manager.register_client(subscription);
    let client_id = client.id;

    let connected_msg = PushMessage::Connected(ConnectedData {
        client_id,
        message: "WebSocket connection established".to_string(),
    });

    if let Ok(json) = serde_json::to_string(&connected_msg) {
        if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
            error!(
                client_id = client_id,
                "Failed to send connected message: {}", e
            );
            push_manager.unregister_client(client_id);
            return;
        }
    }

    push_manager.send_initial_data(&client).await;

    let push_manager_clone = push_manager.clone();
    let client_clone = client.clone();

    let sender_task = tokio::spawn(async move {
        while let Some(msg) = msg_receiver.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                        warn!(client_id = client_id, "Failed to send message: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!(client_id = client_id, "Failed to serialize message: {}", e);
                }
            }
        }
    });

    let receiver_task = tokio::spawn(async move {
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(msg) => match msg {
                    Message::Text(text) => {
                        debug!(client_id = client_id, "Received text message: {}", text);
                        if let Ok(subscription) = serde_json::from_str::<ClientSubscription>(&text)
                        {
                            client_clone.update_subscription(subscription);
                            debug!(client_id = client_id, "Updated subscription");
                        }
                    }
                    Message::Ping(_) => {
                        debug!(client_id = client_id, "Received ping");
                    }
                    Message::Pong(_) => {
                        debug!(client_id = client_id, "Received pong");
                    }
                    Message::Close(_) => {
                        info!(client_id = client_id, "Client closed connection");
                        break;
                    }
                    _ => {}
                },
                Err(e) => {
                    warn!(client_id = client_id, "WebSocket error: {}", e);
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = sender_task => {
            debug!(client_id = client_id, "Sender task completed");
        }
        _ = receiver_task => {
            debug!(client_id = client_id, "Receiver task completed");
        }
    }

    push_manager_clone.unregister_client(client_id);
    info!(client_id = client_id, "WebSocket connection closed");
}
