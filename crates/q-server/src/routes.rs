use crate::handlers::{self, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use q_application::ChatService;
use std::{io, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

pub fn app(chat: Arc<ChatService>) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/v1/models", get(handlers::models))
        .route("/v1/ask", post(handlers::ask))
        .route("/v1/chat/completions", post(handlers::chat_completions))
        .with_state(AppState { chat })
}
pub async fn run(chat: Arc<ChatService>, host: &str, port: u16) -> io::Result<()> {
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let listener = TcpListener::bind(address).await?;
    println!("q-rs server listening at http://{address}");
    axum::serve(listener, app(chat)).await
}
