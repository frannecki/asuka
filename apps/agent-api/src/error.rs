use agent_core::CoreError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    error: String,
}

pub struct ApiError(pub CoreError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            Json(ErrorEnvelope {
                error: self.0.message,
            }),
        )
            .into_response()
    }
}

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        Self(value)
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;
