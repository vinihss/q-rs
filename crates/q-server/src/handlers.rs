use crate::{
    dto::{AskRequest, ChatCompletionRequest},
    error::map_error,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use q_application::ChatService;
use q_core::{ChatRequest, Message, StreamEvent};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub chat: Arc<ChatService>,
}
pub async fn health() -> &'static str {
    "ok"
}
pub async fn models() -> Json<Value> {
    Json(
        json!({"object":"list","data":[{"id":"default","object":"model","owned_by":"q-rs"},{"id":"demo","object":"model","owned_by":"q-rs"}]}),
    )
}

pub async fn ask(State(state): State<AppState>, Json(body): Json<AskRequest>) -> Response {
    if body.q.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":{"message":"A pergunta não pode ser vazia."}})),
        )
            .into_response();
    }
    let request = ChatRequest::new(body.model, vec![Message::user(body.q)]);
    match state.chat.complete(&request).await {
        Ok(answer) => Json(json!({"model":answer.model,"answer":answer.content})).into_response(),
        Err(error) => map_error(error),
    }
}

pub async fn chat_completions(
    State(state): State<AppState>,
    Json(body): Json<ChatCompletionRequest>,
) -> Response {
    let request: ChatRequest = body.into();
    if request.stream {
        return match state.chat.stream(&request).await {
            Ok(stream) => {
                let sse = stream
                    .map(|item| match item {
                        Ok(event) => match serde_json::to_string(&event) {
                            Ok(json) => {
                                Ok::<String, std::convert::Infallible>(format!("data: {json}\n\n"))
                            }
                            Err(_) => Ok("data: [DONE]\n\n".to_owned()),
                        },
                        Err(error) => Ok(format!(
                            "data: {}\n\n",
                            json!({"error":{"message":error.to_string()}})
                        )),
                    })
                    .chain(futures_util::stream::once(async {
                        Ok::<String, std::convert::Infallible>("data: [DONE]\n\n".into())
                    }));
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .body(Body::from_stream(sse))
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
            }
            Err(error) => map_error(error),
        };
    }
    match state.chat.complete(&request).await {
        Ok(answer) => Json(json!({"id":answer.id,"object":"chat.completion","model":answer.model,"choices":[{"index":0,"message":{"role":"assistant","content":answer.content},"finish_reason":answer.finish_reason}],"usage":answer.usage})).into_response(),
        Err(error) => map_error(error),
    }
}
