use crate::AppState;
use axum::{
    extract::{Path, State},
    Json,
};
use school_http::{ApiResponse, EmptyData, HttpError};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct MutationToken {
    pub token: Uuid,
}

pub async fn begin(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    Json(body): Json<MutationToken>,
) -> Result<Json<ApiResponse<EmptyData>>, HttpError> {
    state
        .admin_client
        .begin_directory_mutation(&subdomain, body.token)
        .map_err(|_| HttpError::ServiceUnavailable("tenant_directory_mutation".into()))?;
    Ok(Json(ApiResponse::empty()))
}

pub async fn finish(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    Json(body): Json<MutationToken>,
) -> Result<Json<ApiResponse<EmptyData>>, HttpError> {
    state
        .admin_client
        .finish_directory_mutation(&subdomain, body.token)
        .map_err(|_| HttpError::ServiceUnavailable("tenant_directory_mutation".into()))?;
    Ok(Json(ApiResponse::empty()))
}
