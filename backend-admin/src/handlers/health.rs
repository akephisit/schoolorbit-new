use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::OnceLock;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

pub fn init_pool(pool: PgPool) {
    DB_POOL.set(pool).ok();
}

pub fn health_response_from_db_result(db_result: Result<(), String>) -> (StatusCode, Value) {
    match db_result {
        Ok(()) => (
            StatusCode::OK,
            json!({
                "status": "healthy",
                "database": "connected",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "status": "unhealthy",
                "database": "unavailable",
                "error": error,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ),
    }
}

pub async fn health_check() -> impl IntoResponse {
    let db_result = match DB_POOL.get() {
        Some(pool) => sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|_| "database ping failed".to_string()),
        None => Err("database not initialized".to_string()),
    };

    let (status, body) = health_response_from_db_result(db_result);
    (status, Json(body))
}

#[cfg(test)]
mod tests {
    use super::health_response_from_db_result;
    use axum::http::StatusCode;

    #[test]
    fn db_ping_success_returns_healthy() {
        let (status, body) = health_response_from_db_result(Ok(()));

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "healthy");
        assert_eq!(body["database"], "connected");
    }

    #[test]
    fn db_ping_failure_returns_service_unavailable() {
        let (status, body) =
            health_response_from_db_result(Err("database ping failed".to_string()));

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unhealthy");
        assert_eq!(body["database"], "unavailable");
    }
}
