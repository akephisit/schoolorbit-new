use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::{json, Value};

pub fn liveness_response() -> (StatusCode, Value) {
    (
        StatusCode::OK,
        json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }),
    )
}

pub fn readiness_response_from_db_result(db_result: Result<(), String>) -> (StatusCode, Value) {
    match db_result {
        Ok(()) => (
            StatusCode::OK,
            json!({
                "status": "ready",
                "database": "connected",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "status": "not_ready",
                "database": "unavailable",
                "error": error,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ),
    }
}

pub async fn health_check() -> impl IntoResponse {
    let (status, body) = liveness_response();
    (status, Json(body))
}

pub async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let db_result = sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map(|_| ())
        .map_err(|_| "database ping failed".to_string());

    let (status, body) = readiness_response_from_db_result(db_result);
    (status, Json(body))
}

#[cfg(test)]
mod tests {
    use super::{liveness_response, readiness_response_from_db_result};
    use axum::http::StatusCode;

    #[test]
    fn liveness_response_is_healthy_without_database_status() {
        let (status, body) = liveness_response();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "healthy");
        assert!(body.get("database").is_none());
    }

    #[test]
    fn readiness_db_ping_success_returns_healthy() {
        let (status, body) = readiness_response_from_db_result(Ok(()));

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ready");
        assert_eq!(body["database"], "connected");
    }

    #[test]
    fn readiness_db_ping_failure_returns_service_unavailable() {
        let (status, body) =
            readiness_response_from_db_result(Err("database ping failed".to_string()));

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "not_ready");
        assert_eq!(body["database"], "unavailable");
    }
}
