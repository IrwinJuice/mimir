use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tracing::error;

/// A structured JSON error body returned to the client.
#[derive(Serialize)]
pub struct ErrorBody {
    pub status: u16,
    pub error: String,
    pub message: String,
}

/// Application-level error type that can be returned from any Axum handler.
#[derive(Debug)]
pub enum AppError {
    /// 400 Bad Request
    BadRequest(String),
    /// 404 Not Found
    NotFound(String),
    /// 500 Internal Server Error (wraps any `std::error::Error`)
    Internal(String),
    /// 422 Unprocessable Entity
    Unprocessable(String),
}

impl AppError {
    pub fn internal<E: std::error::Error>(err: E) -> Self {
        error!("Internal error: {}", err);
        AppError::Internal("An internal server error occurred".to_string())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound(msg.into())
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_label, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "Bad Request", msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "Not Found", msg),
            AppError::Unprocessable(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Unprocessable Entity",
                msg,
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                msg,
            ),
        };

        let body = ErrorBody {
            status: status.as_u16(),
            error: error_label.to_string(),
            message,
        };

        (status, Json(body)).into_response()
    }
}

/// Convenience conversion: a plain `String` message → `AppError::BadRequest`.
/// This lets you use `?` on any `Result<_, String>` inside a handler.
impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError::BadRequest(msg)
    }
}

/// Convenience conversion: any `sqlx::Error` → `AppError`.
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found".to_string()),
            other => {
                error!("Database error: {}", other);
                AppError::Internal("A database error occurred".to_string())
            }
        }
    }
}

/// Convenience conversion: any `reqwest::Error` → `AppError`.
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        error!("HTTP client error: {}", err);
        AppError::Internal("An upstream service error occurred".to_string())
    }
}



