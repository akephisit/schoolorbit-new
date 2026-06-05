use crate::error::AppError;
use crate::modules::system::models::ProvisionRequest;
use crate::modules::system::services::provision_service;
use axum::{http::StatusCode, response::IntoResponse, Json};

/// Handler for provisioning a new school tenant database
///
/// This endpoint:
/// 1. Connects to the provided database URL
/// 2. Runs all migrations
/// 3. Creates initial admin user with provided credentials
/// 4. Returns success/failure
pub async fn provision_tenant(
    Json(payload): Json<ProvisionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let outcome = provision_service::provision_tenant(payload).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "data": {
                "school_id": outcome.school_id,
            },
            "message": format!("Tenant database provisioned successfully. Admin Username: {}", outcome.admin_username),
        })),
    ))
}
