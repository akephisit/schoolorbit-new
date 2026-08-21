use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{header::RETRY_AFTER, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    api_response::{ApiErrorResponse, ApiErrorResponseWithOptionalData, ApiResponse},
    error::AppError,
    modules::{
        auth::session_service::AuthenticatedSession,
        certificates::{
            models::{
                AttachCertificateAssetRequest, AttachCertificateBackgroundRequest,
                AttachCertificateFontBatchRequest, CertificateAccountSearchQuery,
                CertificateCampaignDetail, CertificateCampaignListQuery,
                CertificateCampaignPurgeImpact, CertificateCampaignPurgeStatus,
                CertificateCampaignSummary, CertificateCandidateAccount,
                CertificateCandidateBulkRequest, CertificateCandidateBulkResult,
                CertificateCandidateDetail, CertificateCandidateImportResult,
                CertificateCandidateListQuery, CertificateCandidateListResponse,
                CertificateFontUploadInspection, CertificateImportRequest,
                CertificateIssueRequestDetail, CertificateIssueRequestListQuery,
                CertificateIssueRequestSummary, CertificatePreviewManifestRequest,
                CertificateRenderManifest, CertificateRenderManifestBatchRequest,
                CertificateResourceLocked, CertificateTemplateDeleteResult,
                CertificateTemplateDetail, CertificateTemplateVariableCatalog,
                ChangeCertificateCampaignStatusRequest, CreateAccountCertificateCandidateRequest,
                CreateCertificateCampaignRequest, CreateCertificateTemplateRequest,
                CreateManualExternalCandidateRequest, InspectCertificateFontUploadsRequest,
                IssueCertificateOutcome, IssueCertificateRequest, IssuedCertificateDetail,
                IssuedCertificateListQuery, IssuedCertificateSummary,
                ManualCertificateVerificationRequest, PublicCertificateRenderRequest,
                PublicCertificateVerificationData, QrCertificateVerificationRequest,
                ReturnCertificateIssueRequest, RevokeCertificateRequest, RevokeCertificateResult,
                StartCertificateCampaignPurgeRequest, SubmitCertificateIssueRequest,
                UpdateCertificateCampaignRequest, UpdateCertificateCandidateRequest,
                UpdateCertificateTemplateRequest,
            },
            services::{
                campaign_service, candidate_service, issuance_service, purge_service,
                render_service, request_service, template_service, verification_service,
            },
        },
        files::consumer_service::request_deletions,
        lookup::models::OrganizationUnitLookupItem,
    },
    permissions::registry::codes,
    utils::{
        client_address::client_address, request_context::actor_tenant_context_from_session,
        tenant::tenant_context,
    },
    AppState,
};

pub(crate) struct PublicCertificateError(AppError);

impl From<AppError> for PublicCertificateError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}

impl IntoResponse for PublicCertificateError {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            AppError::RateLimited {
                retry_after_seconds,
            } => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(ApiErrorResponse::new("ไม่พบข้อมูลที่ตรงกัน")),
                )
                    .into_response();
                response.headers_mut().insert(
                    RETRY_AFTER,
                    HeaderValue::from(retry_after_seconds.clamp(1, 30)),
                );
                response
            }
            error => error.into_response(),
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/public/certificates/verify/manual",
    operation_id = "verifyCertificateManually",
    tag = "public-certificate",
    request_body = ManualCertificateVerificationRequest,
    responses(
        (status = 200, description = "Allowlisted public certificate verification result", body = ApiResponse<PublicCertificateVerificationData>),
        (status = 404, description = "Certificate number and recipient name did not match", body = ApiErrorResponse),
        (status = 429, description = "Public verification rate limited", body = ApiErrorResponse),
        (status = 422, description = "Malformed verification request", body = ApiErrorResponse)
    )
)]
pub async fn verify_certificate_manually(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<ManualCertificateVerificationRequest>,
) -> Result<impl IntoResponse, PublicCertificateError> {
    let tenant = tenant_context(&state, &headers).await?;
    let source = client_address(
        peer,
        &headers,
        &state.auth_runtime.config.trusted_proxy_cidrs,
    );
    let result = verification_service::verify_rate_limited(
        &tenant.pool,
        tenant.tenant_id,
        source,
        state.certificate_verification_limiter.as_ref(),
        verification_service::CertificateVerificationAttempt::Manual(payload),
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(result))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/public/certificates/verify/qr",
    operation_id = "verifyCertificateByQr",
    tag = "public-certificate",
    request_body = QrCertificateVerificationRequest,
    responses(
        (status = 200, description = "Allowlisted public certificate verification result", body = ApiResponse<PublicCertificateVerificationData>),
        (status = 404, description = "Certificate number and QR proof did not match", body = ApiErrorResponse),
        (status = 429, description = "Public verification rate limited", body = ApiErrorResponse),
        (status = 422, description = "Malformed verification request", body = ApiErrorResponse)
    )
)]
pub async fn verify_certificate_by_qr(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<QrCertificateVerificationRequest>,
) -> Result<impl IntoResponse, PublicCertificateError> {
    let tenant = tenant_context(&state, &headers).await?;
    let source = client_address(
        peer,
        &headers,
        &state.auth_runtime.config.trusted_proxy_cidrs,
    );
    let result = verification_service::verify_rate_limited(
        &tenant.pool,
        tenant.tenant_id,
        source,
        state.certificate_verification_limiter.as_ref(),
        verification_service::CertificateVerificationAttempt::Qr(payload),
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(result))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/public/certificates/render-manifest",
    operation_id = "createPublicCertificateRenderManifest",
    tag = "public-certificate",
    request_body = PublicCertificateRenderRequest,
    responses(
        (status = 200, description = "Fresh public certificate render manifest", body = ApiResponse<CertificateRenderManifest>),
        (status = 404, description = "Receipt is invalid, expired, for another tenant, or no longer renderable", body = ApiErrorResponse),
        (status = 429, description = "Public rendering rate limited", body = ApiErrorResponse),
        (status = 422, description = "Malformed render request", body = ApiErrorResponse),
        (status = 503, description = "Private asset grant unavailable", body = ApiErrorResponse)
    )
)]
pub async fn create_public_certificate_render_manifest(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<PublicCertificateRenderRequest>,
) -> Result<impl IntoResponse, PublicCertificateError> {
    let tenant = tenant_context(&state, &headers).await?;
    let source = client_address(
        peer,
        &headers,
        &state.auth_runtime.config.trusted_proxy_cidrs,
    );
    let manifest = render_service::public_manifest_rate_limited(
        &tenant.pool,
        state.file_platform.as_ref(),
        &tenant.subdomain,
        &state.auth_runtime.config.base_domain,
        tenant.tenant_id,
        source,
        state.certificate_verification_limiter.as_ref(),
        &payload.receipt,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(manifest))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/me/certificates",
    operation_id = "listOwnCertificates",
    tag = "certificate",
    responses(
        (status = 200, description = "Issued and revoked certificates linked to the current account", body = ApiResponse<Vec<IssuedCertificateSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Own certificate permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_own_certificates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::CERTIFICATE_READ_OWN)?;
    let certificates =
        issuance_service::list_own_certificates(&context.tenant.pool, session.user_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(certificates))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/me/certificates/{certificate_id}",
    operation_id = "getOwnCertificate",
    tag = "certificate",
    params(("certificate_id" = Uuid, Path, description = "Certificate ID")),
    responses(
        (status = 200, description = "Certificate linked to the current account", body = ApiResponse<IssuedCertificateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Own certificate permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate is not linked to the current account", body = ApiErrorResponse)
    )
)]
pub async fn get_own_certificate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(certificate_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::CERTIFICATE_READ_OWN)?;
    let certificate = issuance_service::get_own_certificate(
        &context.tenant.pool,
        session.user_id,
        certificate_id,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(certificate))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/me/certificates/{certificate_id}/render-manifest",
    operation_id = "createOwnCertificateRenderManifest",
    tag = "certificate",
    params(("certificate_id" = Uuid, Path, description = "Certificate ID")),
    responses(
        (status = 200, description = "Fresh render manifest for an issued certificate linked to the current account", body = ApiResponse<CertificateRenderManifest>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Own certificate permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate is not linked to the current account", body = ApiErrorResponse),
        (status = 409, description = "The linked certificate is revoked or no longer renderable", body = ApiErrorResponse),
        (status = 503, description = "Private asset grant unavailable", body = ApiErrorResponse)
    )
)]
pub async fn create_own_certificate_render_manifest(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(certificate_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::CERTIFICATE_READ_OWN)?;
    let manifest = render_service::own_manifest(
        &context.tenant.pool,
        session.user_id,
        state.file_platform.as_ref(),
        &context.tenant.subdomain,
        &state.auth_runtime.config.base_domain,
        certificate_id,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(manifest))).into_response())
}

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
        (status = 409, description = "Campaign changed or locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
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
        (status = 409, description = "Campaign changed or locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
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
    get,
    path = "/api/certificates/campaigns/{campaign_id}/purge-impact",
    operation_id = "getCertificateCampaignPurgeImpact",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 200, description = "Permanent purge impact snapshot", body = ApiResponse<CertificateCampaignPurgeImpact>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found or outside the actor's exact delete scope", body = ApiErrorResponse),
        (status = 409, description = "Campaign purge already started", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_campaign_purge_impact(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let impact = purge_service::impact(&context.tenant.pool, &context.actor, campaign_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(impact))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/purge",
    operation_id = "startCertificateCampaignPurge",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = StartCertificateCampaignPurgeRequest,
    responses(
        (status = 202, description = "Permanent campaign purge accepted", body = ApiResponse<CertificateCampaignPurgeStatus>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found or outside the actor's exact delete scope", body = ApiErrorResponse),
        (status = 409, description = "Campaign or purge impact changed", body = ApiErrorResponse),
        (status = 422, description = "Confirmation name is invalid", body = ApiErrorResponse)
    )
)]
pub async fn start_certificate_campaign_purge(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<StartCertificateCampaignPurgeRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let status = purge_service::start(
        &context.tenant.pool,
        &context.actor,
        state.file_platform.as_ref(),
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(ApiResponse::ok(status))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}/purge-status",
    operation_id = "getCertificateCampaignPurgeStatus",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 200, description = "Current permanent purge status", body = ApiResponse<CertificateCampaignPurgeStatus>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "Campaign purge not found, already completed, or outside the actor's exact delete scope", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_campaign_purge_status(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let status = purge_service::status(&context.tenant.pool, &context.actor, campaign_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(status))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/purge/retry",
    operation_id = "retryCertificateCampaignPurge",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 202, description = "Permanent campaign purge retry accepted", body = ApiResponse<CertificateCampaignPurgeStatus>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "Campaign purge not found, already completed, or outside the actor's exact delete scope", body = ApiErrorResponse),
        (status = 409, description = "Campaign purge state is inconsistent", body = ApiErrorResponse)
    )
)]
pub async fn retry_certificate_campaign_purge(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let status = purge_service::retry(
        &context.tenant.pool,
        &context.actor,
        state.file_platform.as_ref(),
        campaign_id,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(ApiResponse::ok(status))).into_response())
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

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}/templates",
    operation_id = "listCertificateTemplates",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 200, description = "Scoped certificate template list", body = ApiResponse<Vec<CertificateTemplateDetail>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse)
    )
)]
pub async fn list_certificate_templates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let templates =
        template_service::list_templates(&context.tenant.pool, &context.actor, campaign_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(templates))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/templates",
    operation_id = "createCertificateTemplate",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = CreateCertificateTemplateRequest,
    responses(
        (status = 201, description = "Certificate template shell created", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission denied", body = ApiErrorResponse),
        (status = 409, description = "Duplicate template name", body = ApiErrorResponse),
        (status = 422, description = "Invalid template", body = ApiErrorResponse)
    )
)]
pub async fn create_certificate_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<CreateCertificateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let template = template_service::create_template(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(template))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/templates/{template_id}",
    operation_id = "getCertificateTemplate",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    responses(
        (status = 200, description = "Certificate template detail", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate template not found", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let template =
        template_service::get_template(&context.tenant.pool, &context.actor, template_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(template))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/certificates/templates/{template_id}",
    operation_id = "updateCertificateTemplate",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    request_body = UpdateCertificateTemplateRequest,
    responses(
        (status = 200, description = "Certificate template updated", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate template not found", body = ApiErrorResponse),
        (status = 409, description = "Template changed, locked, or needs confirmation", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
        (status = 422, description = "Invalid template", body = ApiErrorResponse)
    )
)]
pub async fn update_certificate_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<UpdateCertificateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome = template_service::update_template(
        &context.tenant.pool,
        &context.actor,
        template_id,
        payload,
    )
    .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        outcome.detached_file_ids,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(outcome.template))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/certificates/templates/{template_id}",
    operation_id = "deleteCertificateTemplate",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    responses(
        (status = 200, description = "Unused template deleted or used template deactivated", body = ApiResponse<CertificateTemplateDeleteResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate template not found", body = ApiErrorResponse),
        (status = 409, description = "Certificate template is locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>)
    )
)]
pub async fn delete_certificate_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        template_service::delete_template(&context.tenant.pool, &context.actor, template_id)
            .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        outcome.detached_file_ids,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(outcome.result))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/certificates/templates/{template_id}/background",
    operation_id = "attachCertificateTemplateBackground",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    request_body = AttachCertificateBackgroundRequest,
    responses(
        (status = 200, description = "Inspected PDF background attached", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission denied", body = ApiErrorResponse),
        (status = 409, description = "File not ready, template locked, or preview required", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
        (status = 422, description = "Invalid PDF geometry", body = ApiErrorResponse)
    )
)]
pub async fn attach_certificate_template_background(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<AttachCertificateBackgroundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome = template_service::attach_background(
        &context.tenant.pool,
        &context.actor,
        template_id,
        payload,
    )
    .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        outcome.detached_file_ids,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(outcome.template))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/templates/{template_id}/assets",
    operation_id = "attachCertificateTemplateAsset",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    request_body = AttachCertificateAssetRequest,
    responses(
        (status = 201, description = "Inspected private template asset attached", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission denied", body = ApiErrorResponse),
        (status = 409, description = "File not ready, duplicate, or template locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
        (status = 422, description = "Invalid asset or font rights not confirmed", body = ApiErrorResponse)
    )
)]
pub async fn attach_certificate_template_asset(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<AttachCertificateAssetRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let template =
        template_service::attach_asset(&context.tenant.pool, &context.actor, template_id, payload)
            .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(template))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/templates/{template_id}/assets/fonts/inspect",
    operation_id = "inspectCertificateFontUploads",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    request_body = InspectCertificateFontUploadsRequest,
    responses(
        (status = 200, description = "Private font uploads inspected for exact variants", body = ApiResponse<CertificateFontUploadInspection>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission or file relationship denied", body = ApiErrorResponse),
        (status = 422, description = "Invalid font file selection", body = ApiErrorResponse)
    )
)]
pub async fn inspect_certificate_font_uploads(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<InspectCertificateFontUploadsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let inspection = template_service::inspect_font_uploads(
        &context.tenant.pool,
        &context.actor,
        template_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(inspection))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/templates/{template_id}/assets/fonts/batch",
    operation_id = "attachCertificateFontBatch",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    request_body = AttachCertificateFontBatchRequest,
    responses(
        (status = 201, description = "Reviewed static font variants attached atomically", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission or file relationship denied", body = ApiErrorResponse),
        (status = 409, description = "Duplicate variant or template locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
        (status = 422, description = "Invalid batch, unsupported font, or rights not confirmed", body = ApiErrorResponse)
    )
)]
pub async fn attach_certificate_font_batch(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<AttachCertificateFontBatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let template = template_service::attach_font_batch(
        &context.tenant.pool,
        &context.actor,
        template_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(template))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/certificates/templates/{template_id}/assets/{asset_id}",
    operation_id = "deleteCertificateTemplateAsset",
    tag = "certificate",
    params(
        ("template_id" = Uuid, Path, description = "Certificate template ID"),
        ("asset_id" = Uuid, Path, description = "Certificate template asset ID")
    ),
    responses(
        (status = 200, description = "Unused template asset detached", body = ApiResponse<CertificateTemplateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate template asset not found", body = ApiErrorResponse),
        (status = 409, description = "Asset is referenced by the layout or template is locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>)
    )
)]
pub async fn delete_certificate_template_asset(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((template_id, asset_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        template_service::delete_asset(&context.tenant.pool, &context.actor, template_id, asset_id)
            .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        outcome.detached_file_ids,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(outcome.template))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/templates/{template_id}/variables",
    operation_id = "getCertificateTemplateVariableCatalog",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    responses(
        (status = 200, description = "Renderable standard and campaign custom variables", body = ApiResponse<CertificateTemplateVariableCatalog>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate template not found", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_template_variable_catalog(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let catalog =
        template_service::variable_catalog(&context.tenant.pool, &context.actor, template_id)
            .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(catalog))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/templates/{template_id}/preview-manifest",
    operation_id = "createCertificateTemplatePreviewManifest",
    tag = "certificate",
    params(("template_id" = Uuid, Path, description = "Certificate template ID")),
    request_body = CertificatePreviewManifestRequest,
    responses(
        (status = 200, description = "Short-lived private render manifest marked as sample", body = ApiResponse<CertificateRenderManifest>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate template read permission denied", body = ApiErrorResponse),
        (status = 409, description = "Template or asset is not ready", body = ApiErrorResponse),
        (status = 422, description = "Invalid sample values", body = ApiErrorResponse),
        (status = 503, description = "Private asset grant unavailable", body = ApiErrorResponse)
    )
)]
pub async fn create_certificate_template_preview_manifest(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<CertificatePreviewManifestRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let school_name = state
        .admin_client
        .get_school_name(&context.tenant.subdomain)
        .await
        .map_err(|_| AppError::ServiceUnavailable("school_name_lookup_failed".to_string()))?;
    let manifest = render_service::preview_manifest(
        &context.tenant.pool,
        &context.actor,
        state.file_platform.as_ref(),
        school_name,
        template_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(manifest))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}/candidates",
    operation_id = "listCertificateCandidates",
    tag = "certificate",
    params(
        ("campaign_id" = Uuid, Path, description = "Certificate campaign ID"),
        CertificateCandidateListQuery
    ),
    responses(
        (status = 200, description = "Filtered candidates and campaign status counts", body = ApiResponse<CertificateCandidateListResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 422, description = "Invalid candidate query", body = ApiErrorResponse)
    )
)]
pub async fn list_certificate_candidates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Query(query): Query<CertificateCandidateListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidates = candidate_service::list_candidates(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        query,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(candidates))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/candidates/import",
    operation_id = "importCertificateCandidates",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = CertificateImportRequest,
    responses(
        (status = 201, description = "Typed rows validated, matched, and stored", body = ApiResponse<CertificateCandidateImportResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 409, description = "Certificate campaign cannot be changed", body = ApiErrorResponse),
        (status = 422, description = "Import headers or request are invalid", body = ApiErrorResponse)
    )
)]
pub async fn import_certificate_candidates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<CertificateImportRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let imported = candidate_service::import_candidates(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(imported))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/candidates/manual",
    operation_id = "createManualCertificateCandidate",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = CreateManualExternalCandidateRequest,
    responses(
        (status = 201, description = "Manual external candidate created", body = ApiResponse<CertificateCandidateImportResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 409, description = "Certificate campaign cannot be changed", body = ApiErrorResponse),
        (status = 422, description = "Candidate values are invalid", body = ApiErrorResponse)
    )
)]
pub async fn create_manual_certificate_candidate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<CreateManualExternalCandidateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidate = candidate_service::create_manual_external(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(candidate))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}/candidates/account-search",
    operation_id = "searchCertificateCandidateAccounts",
    tag = "certificate",
    params(
        ("campaign_id" = Uuid, Path, description = "Certificate campaign ID"),
        CertificateAccountSearchQuery
    ),
    responses(
        (status = 200, description = "Minimal active student or staff account matches", body = ApiResponse<Vec<CertificateCandidateAccount>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 422, description = "Invalid account search", body = ApiErrorResponse)
    )
)]
pub async fn search_certificate_candidate_accounts(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Query(query): Query<CertificateAccountSearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let accounts = candidate_service::search_accounts(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        query,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(accounts))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/candidates/account-search",
    operation_id = "createAccountCertificateCandidate",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = CreateAccountCertificateCandidateRequest,
    responses(
        (status = 201, description = "Candidate created from an active account", body = ApiResponse<CertificateCandidateImportResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Campaign or account not found", body = ApiErrorResponse),
        (status = 409, description = "Account or campaign cannot be used", body = ApiErrorResponse),
        (status = 422, description = "Candidate values are invalid", body = ApiErrorResponse)
    )
)]
pub async fn create_account_certificate_candidate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<CreateAccountCertificateCandidateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidate = candidate_service::create_account_candidate(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(candidate))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/candidates/bulk",
    operation_id = "bulkUpdateCertificateCandidates",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = CertificateCandidateBulkRequest,
    responses(
        (status = 200, description = "Atomic bulk candidate resolution", body = ApiResponse<CertificateCandidateBulkResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Candidate selection not found in campaign", body = ApiErrorResponse),
        (status = 409, description = "Mixed or locked candidate selection", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
        (status = 422, description = "Invalid bulk request", body = ApiErrorResponse)
    )
)]
pub async fn bulk_update_certificate_candidates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<CertificateCandidateBulkRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidates = candidate_service::bulk_update_for_campaign(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(candidates))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/candidates/{candidate_id}",
    operation_id = "getCertificateCandidate",
    tag = "certificate",
    params(("candidate_id" = Uuid, Path, description = "Certificate candidate ID")),
    responses(
        (status = 200, description = "Candidate detail", body = ApiResponse<CertificateCandidateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Candidate not found", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_candidate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidate =
        candidate_service::get_candidate(&context.tenant.pool, &context.actor, candidate_id)
            .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(candidate))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/certificates/candidates/{candidate_id}",
    operation_id = "updateCertificateCandidate",
    tag = "certificate",
    params(("candidate_id" = Uuid, Path, description = "Certificate candidate ID")),
    request_body = UpdateCertificateCandidateRequest,
    responses(
        (status = 200, description = "Candidate revalidated and updated", body = ApiResponse<CertificateCandidateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Candidate not found", body = ApiErrorResponse),
        (status = 409, description = "Candidate changed or is locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>),
        (status = 422, description = "Candidate values are invalid", body = ApiErrorResponse)
    )
)]
pub async fn update_certificate_candidate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(candidate_id): Path<Uuid>,
    Json(payload): Json<UpdateCertificateCandidateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidate = candidate_service::update_candidate(
        &context.tenant.pool,
        &context.actor,
        candidate_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(candidate))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/certificates/candidates/{candidate_id}",
    operation_id = "deleteCertificateCandidate",
    tag = "certificate",
    params(("candidate_id" = Uuid, Path, description = "Certificate candidate ID")),
    responses(
        (status = 200, description = "Candidate soft-deleted", body = ApiResponse<CertificateCandidateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate candidate delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Candidate not found", body = ApiErrorResponse),
        (status = 409, description = "Candidate is issued or locked", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>)
    )
)]
pub async fn delete_certificate_candidate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let candidate =
        candidate_service::delete_candidate(&context.tenant.pool, &context.actor, candidate_id)
            .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(candidate))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}/issue-requests",
    operation_id = "listCertificateCampaignIssueRequests",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    responses(
        (status = 200, description = "Scoped issue request history for one campaign", body = ApiResponse<Vec<CertificateIssueRequestSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate campaign read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse)
    )
)]
pub async fn list_certificate_campaign_issue_requests(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let requests =
        request_service::list_campaign_requests(&context.tenant.pool, &context.actor, campaign_id)
            .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(requests))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/issue-requests",
    operation_id = "submitCertificateIssueRequest",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = SubmitCertificateIssueRequest,
    responses(
        (status = 201, description = "Issue request submitted and selected candidates locked", body = ApiResponse<CertificateIssueRequestDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Exact-scope certificate submit permission denied", body = ApiErrorResponse),
        (status = 404, description = "Campaign or selected candidate not found", body = ApiErrorResponse),
        (status = 409, description = "Selected candidate is already locked", body = crate::api_response::ApiErrorResponseWithData<CertificateResourceLocked>),
        (status = 422, description = "Campaign or selected candidate is not ready", body = ApiErrorResponse)
    )
)]
pub async fn submit_certificate_issue_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<SubmitCertificateIssueRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let request = request_service::submit_issue_request(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        payload.candidate_ids,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(request))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/issue-requests",
    operation_id = "listCertificateIssueRequests",
    tag = "certificate",
    params(CertificateIssueRequestListQuery),
    responses(
        (status = 200, description = "School issue request queue", body = ApiResponse<Vec<CertificateIssueRequestSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School certificate issue permission denied", body = ApiErrorResponse),
        (status = 422, description = "Invalid issue request query", body = ApiErrorResponse)
    )
)]
pub async fn list_certificate_issue_requests(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<CertificateIssueRequestListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let requests =
        request_service::list_issue_queue(&context.tenant.pool, &context.actor, query).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(requests))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/issue-requests/{request_id}",
    operation_id = "getCertificateIssueRequest",
    tag = "certificate",
    params(("request_id" = Uuid, Path, description = "Certificate issue request ID")),
    responses(
        (status = 200, description = "Scoped certificate issue request detail", body = ApiResponse<CertificateIssueRequestDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate issue request read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate issue request not found", body = ApiErrorResponse)
    )
)]
pub async fn get_certificate_issue_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(request_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let request =
        request_service::get_issue_request(&context.tenant.pool, &context.actor, request_id)
            .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(request))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/issue-requests/{request_id}/withdraw",
    operation_id = "withdrawCertificateIssueRequest",
    tag = "certificate",
    params(("request_id" = Uuid, Path, description = "Certificate issue request ID")),
    responses(
        (status = 200, description = "Pending issue request withdrawn", body = ApiResponse<CertificateIssueRequestDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Only the scoped submitter may withdraw", body = ApiErrorResponse),
        (status = 404, description = "Certificate issue request not found", body = ApiErrorResponse),
        (status = 409, description = "Issue request cannot be withdrawn from its current state", body = ApiErrorResponse)
    )
)]
pub async fn withdraw_certificate_issue_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(request_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let request =
        request_service::withdraw(&context.tenant.pool, &context.actor, request_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(request))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/issue-requests/{request_id}/review",
    operation_id = "startCertificateIssueRequestReview",
    tag = "certificate",
    params(("request_id" = Uuid, Path, description = "Certificate issue request ID")),
    responses(
        (status = 200, description = "Issue request moved to reviewing", body = ApiResponse<CertificateIssueRequestDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School certificate issue permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate issue request not found", body = ApiErrorResponse),
        (status = 409, description = "Issue request cannot enter review from its current state", body = ApiErrorResponse)
    )
)]
pub async fn start_certificate_issue_request_review(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(request_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let request =
        request_service::start_review(&context.tenant.pool, &context.actor, request_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(request))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/issue-requests/{request_id}/return",
    operation_id = "returnCertificateIssueRequest",
    tag = "certificate",
    params(("request_id" = Uuid, Path, description = "Certificate issue request ID")),
    request_body = ReturnCertificateIssueRequest,
    responses(
        (status = 200, description = "Reviewing issue request returned for a new corrected request", body = ApiResponse<CertificateIssueRequestDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School certificate issue permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate issue request not found", body = ApiErrorResponse),
        (status = 409, description = "Issue request cannot be returned from its current state", body = ApiErrorResponse),
        (status = 422, description = "Return reasons or note are invalid", body = ApiErrorResponse)
    )
)]
pub async fn return_certificate_issue_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(request_id): Path<Uuid>,
    Json(payload): Json<ReturnCertificateIssueRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let request = request_service::return_request(
        &context.tenant.pool,
        &context.actor,
        request_id,
        payload.issue_codes,
        payload.return_note,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(request))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/issue-requests/{request_id}/issue",
    operation_id = "issueCertificates",
    tag = "certificate",
    params(("request_id" = Uuid, Path, description = "Certificate issue request ID")),
    request_body = IssueCertificateRequest,
    responses(
        (status = 200, description = "Idempotent issued or returned outcome", body = ApiResponse<IssueCertificateOutcome>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School certificate issue permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate issue request not found", body = ApiErrorResponse),
        (status = 409, description = "Request state, idempotency key, or number range conflict", body = ApiErrorResponse),
        (status = 503, description = "Authoritative school name unavailable", body = ApiErrorResponse)
    )
)]
pub async fn issue_certificates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(request_id): Path<Uuid>,
    Json(payload): Json<IssueCertificateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    if let Some(outcome) = issuance_service::replay_issue_request(
        &context.tenant.pool,
        &context.actor,
        request_id,
        payload.idempotency_key,
    )
    .await?
    {
        return Ok((StatusCode::OK, Json(ApiResponse::ok(outcome))).into_response());
    }
    let school_name = state
        .admin_client
        .get_school_name(&context.tenant.subdomain)
        .await
        .map_err(|_| AppError::ServiceUnavailable("school_name_lookup_failed".to_string()))?;
    let outcome = issuance_service::issue_request(
        &context.tenant.pool,
        &context.actor,
        school_name,
        request_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(outcome))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/campaigns/{campaign_id}/issued",
    operation_id = "listIssuedCertificates",
    tag = "certificate",
    params(
        ("campaign_id" = Uuid, Path, description = "Certificate campaign ID"),
        IssuedCertificateListQuery
    ),
    responses(
        (status = 200, description = "Scoped issued and revoked certificate list", body = ApiResponse<Vec<IssuedCertificateSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Certificate campaign not found", body = ApiErrorResponse),
        (status = 422, description = "Invalid issued certificate query", body = ApiErrorResponse)
    )
)]
pub async fn list_issued_certificates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Query(query): Query<IssuedCertificateListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let certificates = issuance_service::list_campaign_certificates(
        &context.tenant.pool,
        &context.actor,
        campaign_id,
        query,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(certificates))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/certificates/{certificate_id}",
    operation_id = "getIssuedCertificate",
    tag = "certificate",
    params(("certificate_id" = Uuid, Path, description = "Issued certificate ID")),
    responses(
        (status = 200, description = "Scoped issued certificate detail without proof or lookup identifiers", body = ApiResponse<IssuedCertificateDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Issued certificate not found", body = ApiErrorResponse)
    )
)]
pub async fn get_issued_certificate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(certificate_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let certificate =
        issuance_service::get_certificate(&context.tenant.pool, &context.actor, certificate_id)
            .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(certificate))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/{certificate_id}/revoke",
    operation_id = "revokeIssuedCertificate",
    tag = "certificate",
    params(("certificate_id" = Uuid, Path, description = "Issued certificate ID")),
    request_body = RevokeCertificateRequest,
    responses(
        (status = 200, description = "Certificate revoked with optional replacement draft", body = ApiResponse<RevokeCertificateResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School certificate revoke permission denied", body = ApiErrorResponse),
        (status = 404, description = "Issued certificate not found", body = ApiErrorResponse),
        (status = 409, description = "Certificate already revoked", body = ApiErrorResponse),
        (status = 422, description = "Invalid revocation reason", body = ApiErrorResponse)
    )
)]
pub async fn revoke_issued_certificate(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(certificate_id): Path<Uuid>,
    Json(payload): Json<RevokeCertificateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = issuance_service::revoke_certificate(
        &context.tenant.pool,
        &context.actor,
        certificate_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(result))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/{certificate_id}/render-manifest",
    operation_id = "createIssuedCertificateRenderManifest",
    tag = "certificate",
    params(("certificate_id" = Uuid, Path, description = "Issued certificate ID")),
    responses(
        (status = 200, description = "Authorized short-lived render manifest", body = ApiResponse<CertificateRenderManifest>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate download permission denied", body = ApiErrorResponse),
        (status = 404, description = "Issued certificate not found", body = ApiErrorResponse),
        (status = 409, description = "Certificate revoked or current template unavailable", body = ApiErrorResponse),
        (status = 503, description = "Private asset grant unavailable", body = ApiErrorResponse)
    )
)]
pub async fn create_issued_certificate_render_manifest(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(certificate_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let manifest = render_service::issued_manifest(
        &context.tenant.pool,
        &context.actor,
        state.file_platform.as_ref(),
        &context.tenant.subdomain,
        &state.auth_runtime.config.base_domain,
        certificate_id,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(manifest))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/certificates/campaigns/{campaign_id}/render-manifests",
    operation_id = "createIssuedCertificateRenderManifests",
    tag = "certificate",
    params(("campaign_id" = Uuid, Path, description = "Certificate campaign ID")),
    request_body = CertificateRenderManifestBatchRequest,
    responses(
        (status = 200, description = "Ordered authorized render manifests", body = ApiResponse<Vec<CertificateRenderManifest>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Certificate download permission denied", body = ApiErrorResponse),
        (status = 404, description = "Selected certificate not found in campaign", body = ApiErrorResponse),
        (status = 409, description = "A certificate is revoked or its current template unavailable", body = ApiErrorResponse),
        (status = 422, description = "Select between 1 and 200 unique certificates", body = ApiErrorResponse),
        (status = 503, description = "Private asset grant unavailable", body = ApiErrorResponse)
    )
)]
pub async fn create_issued_certificate_render_manifests(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
    Json(payload): Json<CertificateRenderManifestBatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let manifests = render_service::issued_manifests(
        &context.tenant.pool,
        &context.actor,
        state.file_platform.as_ref(),
        &context.tenant.subdomain,
        &state.auth_runtime.config.base_domain,
        campaign_id,
        payload,
    )
    .await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(manifests))).into_response())
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::*;

    #[derive(serde::Deserialize)]
    struct TestErrorResponse {
        success: bool,
        error: String,
    }

    #[tokio::test]
    async fn public_rate_limit_keeps_retry_metadata_but_uses_the_generic_failure_body() {
        let response = PublicCertificateError::from(AppError::RateLimited {
            retry_after_seconds: 17,
        })
        .into_response();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("retry-after").unwrap(), "17");
        let body = to_bytes(response.into_body(), 4_096).await.unwrap();
        let payload: TestErrorResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.success);
        assert_eq!(payload.error, "ไม่พบข้อมูลที่ตรงกัน");
    }
}
