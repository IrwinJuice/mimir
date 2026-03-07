use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::SinkExt;
use futures_util::stream::StreamExt;
use tokio::sync::broadcast;
use tracing::{error, info};


pub type WsTx = broadcast::Sender<String>;

pub async fn handle_socket(
    ws: WebSocketUpgrade,
    State(tx): State<WsTx>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let rx = tx.subscribe();
        run_socket(socket, rx).await;
    })
}

async fn run_socket(socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    let (mut sender, mut receiver) = socket.split();

    // Forward broadcast messages to the WebSocket client
    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if sender.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    error!("WS receiver lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Drain incoming frames (ping/close handling)
    while let Some(Ok(_msg)) = receiver.next().await {}

    send_task.abort();
    info!("WebSocket connection closed");
}