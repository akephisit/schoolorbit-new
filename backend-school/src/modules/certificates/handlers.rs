use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    api_response::{ApiErrorResponse, ApiResponse},
    error::AppError,
    modules::{
        auth::session_service::AuthenticatedSession,
        certificates::{
            models::{
                CertificateCampaignDetail, CertificateCampaignListQuery,
                CertificateCampaignSummary, ChangeCertificateCampaignStatusRequest,
                CreateCertificateCampaignRequest, UpdateCertificateCampaignRequest,
            },
            services::campaign_service,
        },
        lookup::models::OrganizationUnitLookupItem,
    },
    utils::request_context::actor_tenant_context_from_session,
    AppState,
};

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns",
    operation_id = "listCertificateCampaigns",
    tag = "certificate",
    params(CertificateCampaignListQuery),
    responses(
        (status = 200, description = "Scoped certificate campaign list", body = ApiResponse<Vec<CertificateCampaignSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign read permission denied", body = ApiErrorResponse),
        (status = 422, description = "Invalid query", body = ApiErrorResponse)
    )
)]
pub async fn list_certificate_campaigns(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<CertificateCampaignListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let campaigns =
        campaign_service::list_campaigns(&context.tenant.pool, &context.actor, query).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(campaigns))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns",
    operation_id = "createCertificateCampaign",
    tag = "certificate",
    request_body = CreateCertificateCampaignRequest,
    responses(
        (status = 201, description = "Certificate campaign created", body = ApiResponse<CertificateCampaignDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign create permission denied", body = ApiErrorResponse),
        (status = 422, description = "Invalid campaign", body = ApiErrorResponse)
    )
)]
pub async fn create_certificate_campaign(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateCertificateCampaignRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let campaign =
        campaign_service::create_campaign(&context.tenant.pool, &context.actor, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(campaign))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}",
    operation_id = "getCertificateCampaign",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 200, description = "Certificate campaign detail", body = ApiResponse<CertificateCampaignDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_campaign(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let campaign =
        campaign_service::get_campaign(&context.tenant.pool, &context.actor, campaign_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(campaign))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/certificates/campaigns/{campaign_id}",
    operation_id = "updateCertificateCampaign",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = UpdateCertificateCampaignRequest,
    responses(
        (status = 200, description = "Certificate campaign updated", body = ApiResponse<CertificateCampaignDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 409, description = "Campaign changed or locked", body = ApiErrorResponse),
        (status = 422, description = "Invalid campaign", body = ApiErrorResponse)
    )
)]
pub async fn update_certificate_campaign(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<UpdateCertificateCampaignRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let campaign = campaign_service::update_campaign(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(campaign))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/certificates/campaigns/{campaign_id}/status",
    operation_id = "changeCertificateCampaignStatus",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = ChangeCertificateCampaignStatusRequest,
    responses(
        (status = 200, description = "Certificate campaign status changed", body = ApiResponse<CertificateCampaignDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 409, description = "Campaign changed or locked", body = ApiErrorResponse),
        (status = 422, description = "Invalid status transition", body = ApiErrorResponse)
    )
)]
pub async fn change_certificate_campaign_status(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<ChangeCertificateCampaignStatusRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let campaign = campaign_service::change_campaign_status(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(campaign))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/certificates/campaigns/{campaign_id}",
    operation_id = "deleteCertificateCampaign",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 200, description = "Draft certificate campaign deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 409, description = "Campaign cannot be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_certificate_campaign(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    campaign_service::delete_campaign(&context.tenant.pool, &context.actor, campaign_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::empty())).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/owner-options",
    operation_id = "listCertificateOwnerOptions",
    tag = "certificate",
    responses(
        (status = 200, description = "Active exact-scope certificate owner options", body = ApiResponse<Vec<OrganizationUnitLookupItem>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign create permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_certificate_owner_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let options =
        campaign_service::list_owner_options(&context.tenant.pool, &context.actor).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(options))).into_response())
}
