use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct LivenessResponse {
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadinessResponse {
    status: &'static str,
    control_plane: &'static str,
    timestamp: String,
}

fn liveness_response(timestamp: String) -> (StatusCode, LivenessResponse) {
    (
        StatusCode::OK,
        LivenessResponse {
            status: "healthy",
            timestamp,
        },
    )
}

fn readiness_response(
    timestamp: String,
    control_plane_result: Result<(), String>,
) -> (StatusCode, ReadinessResponse) {
    match control_plane_result {
        Ok(()) => (
            StatusCode::OK,
            ReadinessResponse {
                status: "ready",
                control_plane: "connected",
                timestamp,
            },
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            ReadinessResponse {
                status: "not_ready",
                control_plane: "unavailable",
                timestamp,
            },
        ),
    }
}

pub async fn health_check() -> impl IntoResponse {
    let (status, response) = liveness_response(chrono::Utc::now().to_rfc3339());
    (status, Json(response))
}

pub async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let result = state.admin_client.check_readiness().await;
    if let Err(error) = &result {
        tracing::warn!(error = %error, "Backend-school readiness check failed");
    }
    let (status, response) = readiness_response(chrono::Utc::now().to_rfc3339(), result);
    (status, Json(response))
}

#[cfg(test)]
mod tests {
    use super::{liveness_response, readiness_response};
    use axum::http::StatusCode;

    #[test]
    fn liveness_is_dependency_free() {
        let (status, response) = liveness_response("2026-07-23T00:00:00Z".to_string());

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, "healthy");
    }

    #[test]
    fn available_control_plane_is_ready() {
        let (status, response) = readiness_response("2026-07-23T00:00:00Z".to_string(), Ok(()));

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, "ready");
        assert_eq!(response.control_plane, "connected");
    }

    #[test]
    fn unavailable_control_plane_fails_closed_without_internal_error() {
        let (status, response) = readiness_response(
            "2026-07-23T00:00:00Z".to_string(),
            Err("secret internal detail".to_string()),
        );

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.status, "not_ready");
        assert_eq!(response.control_plane, "unavailable");
        let json = serde_json::to_value(response).expect("readiness response must serialize");
        assert!(json.get("error").is_none());
        assert!(json.get("controlPlane").is_some());
    }
}
