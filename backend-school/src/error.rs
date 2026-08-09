use axum::{
    http::{header::RETRY_AFTER, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::api_response::ApiErrorResponse;

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

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Rate limited")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Payload too large")]
    PayloadTooLarge,
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::DbError(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND,
            AppError::DbError(sqlx::Error::Database(error))
                if matches!(error.code().as_deref(), Some("23503" | "23001")) =>
            {
                StatusCode::BAD_REQUEST
            }
            AppError::DbError(sqlx::Error::Database(error))
                if error.code().as_deref() == Some("23505") =>
            {
                StatusCode::CONFLICT
            }
            AppError::DbError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::AuthError(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::InternalServerError(_) | AppError::ConfigError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        }
    }

    pub fn public_message(&self) -> &str {
        match self {
            AppError::DbError(sqlx::Error::RowNotFound) => "ไม่พบข้อมูล",
            AppError::DbError(sqlx::Error::Database(error))
                if matches!(error.code().as_deref(), Some("23503" | "23001")) =>
            {
                "ไม่สามารถทำรายการได้ (ข้อมูลอ้างอิงไม่ถูกต้องหรือถูกใช้งานอยู่)"
            }
            AppError::DbError(sqlx::Error::Database(error))
                if error.code().as_deref() == Some("23505") =>
            {
                "ข้อมูลซ้ำกับที่มีอยู่ในระบบแล้ว"
            }
            AppError::DbError(_) => "เกิดข้อผิดพลาดในการเชื่อมต่อฐานข้อมูล",
            AppError::AuthError(message)
            | AppError::Forbidden(message)
            | AppError::NotFound(message)
            | AppError::ValidationError(message)
            | AppError::BadRequest(message)
            | AppError::Conflict(message) => message,
            AppError::InternalServerError(_) => "Internal server error",
            AppError::ConfigError(_) => "System configuration error",
            AppError::ServiceUnavailable(_) => "Service temporarily unavailable",
            AppError::RateLimited { .. } => "Too many attempts; try again later",
            AppError::PayloadTooLarge => "Request payload is too large",
        }
    }

    pub fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            AppError::RateLimited {
                retry_after_seconds,
            } => Some((*retry_after_seconds).clamp(1, 30)),
            _ => None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            AppError::DbError(error) => {
                let database_code = match error {
                    sqlx::Error::Database(error) => error.code(),
                    _ => None,
                };
                tracing::error!(reason = "database_error", database_code = ?database_code);
            }
            AppError::ConfigError(reason) => {
                tracing::error!(reason = %reason, "configuration error");
            }
            AppError::ServiceUnavailable(reason) => {
                tracing::warn!(reason = %reason, "service unavailable");
            }
            AppError::InternalServerError(reason) => {
                tracing::error!(reason = %reason, "internal server error");
            }
            _ => {}
        }

        let status = self.status_code();
        let retry_after = self.retry_after_seconds();
        let body = Json(ApiErrorResponse::new(self.public_message().to_string()));
        let mut response = (status, body).into_response();

        if let Some(seconds) = retry_after {
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from(seconds));
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_error_returns_standard_409_envelope() {
        let response = AppError::Conflict("สถานะทรัพยากรขัดแย้ง".to_string()).into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn rate_limited_error_caps_retry_after_and_uses_standard_status() {
        let error = AppError::RateLimited {
            retry_after_seconds: 99,
        };

        assert_eq!(error.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error.retry_after_seconds(), Some(30));

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("retry-after").unwrap(), "30");
    }

    #[test]
    fn payload_too_large_uses_413_without_reflecting_input() {
        let error = AppError::PayloadTooLarge;

        assert_eq!(error.status_code(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(error.public_message(), "Request payload is too large");
    }
}
