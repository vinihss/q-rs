use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use q_core::QError;
use serde_json::json;
pub fn map_error(error: QError) -> Response {
    let (status, message, kind) = match error {
        QError::InvalidRequest(m) => (StatusCode::BAD_REQUEST, m, "invalid_request_error"),
        QError::NoProvider(m) => (
            StatusCode::BAD_REQUEST,
            format!("nenhum provedor para modelo: {m}"),
            "model_not_found",
        ),
        other @ (QError::AllProvidersFailed | QError::Provider { .. }) => {
            (StatusCode::BAD_GATEWAY, other.to_string(), "provider_error")
        }
        QError::UnsupportedOperation(m) => {
            (StatusCode::NOT_IMPLEMENTED, m, "unsupported_operation")
        }
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            other.to_string(),
            "q_rs_error",
        ),
    };
    (
        status,
        Json(json!({"error":{"message":message,"type":kind}})),
    )
        .into_response()
}
