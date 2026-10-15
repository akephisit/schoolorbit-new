//! Admin-owned customer accounts and service control plane. Never accesses tenant DBs.
pub mod auth;
pub mod http;
pub mod migration;
pub mod models;
pub mod service;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub struct PortalError(pub StatusCode, pub &'static str);

impl PortalError {
    pub fn invalid() -> Self {
        Self(StatusCode::BAD_REQUEST, "กรุณาตรวจสอบข้อมูลที่กรอก")
    }
    pub fn unauthorized() -> Self {
        Self(StatusCode::UNAUTHORIZED, "กรุณาเข้าสู่ระบบอีกครั้ง")
    }
    pub fn forbidden() -> Self {
        Self(StatusCode::FORBIDDEN, "คุณไม่มีสิทธิ์ดำเนินการนี้")
    }
    pub fn unavailable() -> Self {
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "บริการยังไม่พร้อม กรุณาลองอีกครั้ง",
        )
    }
}
impl From<sqlx::Error> for PortalError {
    fn from(error: sqlx::Error) -> Self {
        let code = error
            .as_database_error()
            .and_then(|error| error.code())
            .map(|code| code.into_owned());
        tracing::error!(
            operation = "admin_portal_database",
            sqlstate = code,
            "Admin data operation unavailable"
        );
        Self::unavailable()
    }
}
impl IntoResponse for PortalError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorBody {
            success: bool,
            error: &'static str,
        }
        let mut response = (
            self.0,
            Json(ErrorBody {
                success: false,
                error: self.1,
            }),
        )
            .into_response();
        response.headers_mut().insert(
            "cache-control",
            axum::http::HeaderValue::from_static("no-store"),
        );
        if self.0 == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from_static("900"));
        }
        response
    }
}
