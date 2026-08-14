use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
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
                CertificateAccountSearchQuery, CertificateCampaignDetail,
                CertificateCampaignListQuery, CertificateCampaignSummary,
                CertificateCandidateAccount, CertificateCandidateBulkRequest,
                CertificateCandidateBulkResult, CertificateCandidateDetail,
                CertificateCandidateImportResult, CertificateCandidateListQuery,
                CertificateCandidateListResponse, CertificateImportRequest,
                CertificateIssueRequestDetail, CertificateIssueRequestListQuery,
                CertificateIssueRequestSummary, CertificatePreviewManifestRequest,
                CertificateRenderManifest, CertificateResourceLocked,
                CertificateTemplateDeleteResult, CertificateTemplateDetail,
                CertificateTemplateVariableCatalog, ChangeCertificateCampaignStatusRequest,
                CreateAccountCertificateCandidateRequest, CreateCertificateCampaignRequest,
                CreateCertificateTemplateRequest, CreateManualExternalCandidateRequest,
                ReturnCertificateIssueRequest, SubmitCertificateIssueRequest,
                UpdateCertificateCampaignRequest, UpdateCertificateCandidateRequest,
                UpdateCertificateTemplateRequest,
            },
            services::{
                campaign_service, candidate_service, render_service, request_service,
                template_service,
            },
        },
        files::consumer_service::request_deletions,
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
        (status = 409, description = "Campaign cannot be deleted", body = ApiErrorResponseWithOptionalData<CertificateResourceLocked>)
    )
)]
pub async fn delete_certificate_campaign(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(campaign_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let detached_file_ids =
        campaign_service::delete_campaign(&context.tenant.pool, &context.actor, campaign_id)
            .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        detached_file_ids,
    )
    .await?;
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
