use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),

    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("Authorization error: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::DbError(err) => {
                // Log the actual db error
                tracing::error!("Database error: {:?}", err);
                match err {
                    sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "ไม่พบข้อมูล".to_string()),
                    sqlx::Error::Database(db_err) => {
                        let code = db_err.code().unwrap_or_default();
                        if code == "23503" || code == "23001" {
                            (StatusCode::BAD_REQUEST, "ไม่สามารถลบหรือแก้ไขข้อมูลได้ เนื่องจากข้อมูลนี้ถูกใช้งานอยู่ในส่วนอื่นของระบบ".to_string())
                        } else if code == "23505" {
                            (StatusCode::CONFLICT, "ข้อมูลซ้ำกับที่มีอยู่ในระบบแล้ว".to_string())
                        } else {
                            (StatusCode::INTERNAL_SERVER_ERROR, "เกิดข้อผิดพลาดในการเชื่อมต่อฐานข้อมูล".to_string())
                        }
                    },
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, "เกิดข้อผิดพลาดในการเชื่อมต่อฐานข้อมูล".to_string()),
                }
            },
            AppError::AuthError(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::ValidationError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::ConfigError(msg) => {
                tracing::error!("Configuration error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "System configuration error".to_string())
            },
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            },
        };

        let body = Json(json!({
            "success": false,
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
