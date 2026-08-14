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
                AttachCertificateAssetRequest, AttachCertificateBackgroundRequest,
                CertificateCampaignDetail, CertificateCampaignListQuery,
                CertificateCampaignSummary, CertificatePreviewManifestRequest,
                CertificateRenderManifest, CertificateTemplateDeleteResult,
                CertificateTemplateDetail, CertificateTemplateVariableCatalog,
                ChangeCertificateCampaignStatusRequest, CreateCertificateCampaignRequest,
                CreateCertificateTemplateRequest, UpdateCertificateCampaignRequest,
                UpdateCertificateTemplateRequest,
            },
            services::{campaign_service, render_service, template_service},
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
        (status = 409, description = "Template changed, locked, or needs confirmation", body = ApiErrorResponse),
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
        (status = 409, description = "Certificate template is locked", body = ApiErrorResponse)
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
        (status = 409, description = "File not ready, template locked, or preview required", body = ApiErrorResponse),
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
        (status = 409, description = "File not ready, duplicate, or template locked", body = ApiErrorResponse),
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
        (status = 409, description = "Asset is referenced by the layout", body = ApiErrorResponse)
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
