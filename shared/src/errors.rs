use actix_web::{HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum ServiceError {
    NotFound,
    InternalError(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    BadRequest(String),
    InvalidClient(String),
    InvalidGrant(String),
    InvalidRequest(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::NotFound => write!(f, "Resource not found"),
            ServiceError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            ServiceError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ServiceError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            ServiceError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            ServiceError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            ServiceError::InvalidClient(msg) => write!(f, "Invalid client: {}", msg),
            ServiceError::InvalidGrant(msg) => write!(f, "Invalid grant: {}", msg),
            ServiceError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
        }
    }
}

impl ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServiceError::NotFound => {
                HttpResponse::NotFound().json(serde_json::json!({"error": "not_found"}))
            }
            ServiceError::InternalError(msg) => HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "server_error", "error_description": msg})),
            ServiceError::Unauthorized(msg) => HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "access_denied", "error_description": msg})),
            ServiceError::Forbidden(msg) => HttpResponse::Forbidden()
                .json(serde_json::json!({"error": "forbidden", "error_description": msg})),
            ServiceError::Conflict(msg) => HttpResponse::Conflict()
                .json(serde_json::json!({"error": "conflict", "error_description": msg})),
            ServiceError::BadRequest(msg) => HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "bad_request", "error_description": msg})),
            ServiceError::InvalidClient(msg) => HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "invalid_client", "error_description": msg})),
            ServiceError::InvalidGrant(msg) => HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "invalid_grant", "error_description": msg})),
            ServiceError::InvalidRequest(msg) => HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "invalid_request", "error_description": msg})),
        }
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ServiceError::NotFound,
            _ => ServiceError::InternalError(err.to_string()),
        }
    }
}
