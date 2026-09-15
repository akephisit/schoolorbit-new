use std::fmt;

use axum::{
    http::{header::RETRY_AFTER, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use school_errors::AppError;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct EmptyData {}

impl ApiResponse<EmptyData> {
    pub fn empty() -> Self {
        Self::ok(EmptyData::default())
    }

    pub fn empty_with_message(message: impl Into<String>) -> Self {
        Self::with_message(EmptyData::default(), message)
    }
}

#[derive(Debug, Serialize)]
pub struct IdData<T> {
    pub id: T,
}

/// OpenAPI transport schema for the UUID identifier payload emitted by `IdData<Uuid>`.
#[derive(Debug, Serialize, ToSchema)]
pub struct UuidIdData {
    pub id: uuid::Uuid,
}

impl<T> IdData<T> {
    pub fn new(id: T) -> Self {
        Self { id }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponse {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub message: Option<String>,
}

impl ApiErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: error.into(),
            message: None,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponseWithData<T> {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub data: T,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponseWithOptionalData<T> {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiErrorResponseWithData<T> {
    pub fn new(error: impl Into<String>, data: T) -> Self {
        Self {
            success: false,
            error: error.into(),
            message: None,
            data,
        }
    }

    pub fn with_message(error: impl Into<String>, message: impl Into<String>, data: T) -> Self {
        Self {
            success: false,
            error: error.into(),
            message: Some(message.into()),
            data,
        }
    }
}

#[derive(Debug)]
pub struct HttpError(AppError);

/// Transport mapping for domain errors. Import this trait only in HTTP adapters or tests that
/// intentionally verify the wire contract; domain crates must not depend on it at runtime.
pub trait AppErrorHttpExt {
    fn status_code(&self) -> StatusCode;
    fn public_message(&self) -> &str;
    fn retry_after_seconds(&self) -> Option<u64>;
}

impl AppErrorHttpExt for AppError {
    fn status_code(&self) -> StatusCode {
        status_code(self)
    }

    fn public_message(&self) -> &str {
        public_message(self)
    }

    fn retry_after_seconds(&self) -> Option<u64> {
        retry_after_seconds(self)
    }
}

impl HttpError {
    #[allow(non_upper_case_globals)]
    pub const PayloadTooLarge: Self = Self(AppError::PayloadTooLarge);

    #[allow(non_snake_case)]
    pub fn AuthError(message: String) -> Self {
        AppError::AuthError(message).into()
    }

    #[allow(non_snake_case)]
    pub fn Forbidden(message: String) -> Self {
        AppError::Forbidden(message).into()
    }

    #[allow(non_snake_case)]
    pub fn NotFound(message: String) -> Self {
        AppError::NotFound(message).into()
    }

    #[allow(non_snake_case)]
    pub fn ValidationError(message: String) -> Self {
        AppError::ValidationError(message).into()
    }

    #[allow(non_snake_case)]
    pub fn InternalServerError(message: String) -> Self {
        AppError::InternalServerError(message).into()
    }

    #[allow(non_snake_case)]
    pub fn BadRequest(message: String) -> Self {
        AppError::BadRequest(message).into()
    }

    #[allow(non_snake_case)]
    pub fn Conflict(message: String) -> Self {
        AppError::Conflict(message).into()
    }

    #[allow(non_snake_case)]
    pub fn ConfigError(message: String) -> Self {
        AppError::ConfigError(message).into()
    }

    #[allow(non_snake_case)]
    pub fn ServiceUnavailable(message: String) -> Self {
        AppError::ServiceUnavailable(message).into()
    }

    pub fn as_domain(&self) -> &AppError {
        &self.0
    }

    pub fn into_domain(self) -> AppError {
        self.0
    }

    pub fn status_code(&self) -> axum::http::StatusCode {
        self.0.status_code()
    }

    pub fn public_message(&self) -> &str {
        self.0.public_message()
    }

    pub fn retry_after_seconds(&self) -> Option<u64> {
        self.0.retry_after_seconds()
    }
}

impl From<AppError> for HttpError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for HttpError {}

fn status_code(error: &AppError) -> StatusCode {
    match error {
        AppError::DbError(sqlx_error) => match sqlx_error {
            sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
            sqlx::Error::Database(database_error)
                if matches!(database_error.code().as_deref(), Some("23503" | "23001")) =>
            {
                StatusCode::BAD_REQUEST
            }
            sqlx::Error::Database(database_error)
                if database_error.code().as_deref() == Some("23505") =>
            {
                StatusCode::CONFLICT
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        },
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

fn public_message(error: &AppError) -> &str {
    match error {
        AppError::DbError(sqlx::Error::RowNotFound) => "ไม่พบข้อมูล",
        AppError::DbError(sqlx::Error::Database(database_error))
            if matches!(database_error.code().as_deref(), Some("23503" | "23001")) =>
        {
            "ไม่สามารถทำรายการได้ (ข้อมูลอ้างอิงไม่ถูกต้องหรือถูกใช้งานอยู่)"
        }
        AppError::DbError(sqlx::Error::Database(database_error))
            if database_error.code().as_deref() == Some("23505") =>
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

fn retry_after_seconds(error: &AppError) -> Option<u64> {
    match error {
        AppError::RateLimited {
            retry_after_seconds,
        } => Some((*retry_after_seconds).clamp(1, 30)),
        _ => None,
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        match &self.0 {
            AppError::DbError(_) => {
                let database_code = self.0.database_code();
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
        let mut response = (
            status,
            Json(ApiErrorResponse::new(self.public_message().to_string())),
        )
            .into_response();

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
    use axum::body::to_bytes;

    use super::*;

    #[test]
    fn conflict_and_auth_statuses_match_the_public_contract() {
        assert_eq!(
            AppError::Conflict("สถานะทรัพยากรขัดแย้ง".into()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            AppError::AuthError("Authentication required".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn retry_after_is_bounded_at_both_ends() {
        assert_eq!(
            AppError::RateLimited {
                retry_after_seconds: 0
            }
            .retry_after_seconds(),
            Some(1)
        );
        assert_eq!(
            AppError::RateLimited {
                retry_after_seconds: 99
            }
            .retry_after_seconds(),
            Some(30)
        );
    }

    #[tokio::test]
    async fn statuses_messages_and_retry_headers_keep_the_wire_contract() {
        let response = HttpError::from(AppError::PayloadTooLarge).into_response();
        assert_eq!(response.status(), axum::http::StatusCode::PAYLOAD_TOO_LARGE);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"], "Request payload is too large");

        let response = HttpError::from(AppError::RateLimited {
            retry_after_seconds: 99,
        })
        .into_response();
        assert_eq!(response.headers()[RETRY_AFTER], "30");
    }
}
