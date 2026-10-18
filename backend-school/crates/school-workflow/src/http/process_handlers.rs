//! Authenticated transport only; all policy and transaction logic is domain-owned.
use super::state::WorkflowHttpState;
use crate::{
    correspondence::{self, CorrespondenceListQuery, CorrespondenceOptionsQuery},
    directed, process,
    process_models::*,
};
use axum::{
    extract::{Extension, Path, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, HttpError};
use uuid::Uuid;

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence",tag="correspondence",params(CorrespondenceListQuery),responses((status=200,body=ApiResponse<CorrespondenceList>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<CorrespondenceListQuery>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::list(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence",tag="correspondence",responses((status=200,body=ApiResponse<CorrespondenceDetail>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=CreateCorrespondenceRequest))]
pub async fn create_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateCorrespondenceRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::create(
        &context.tenant.pool,
        &context.actor,
        &crate::document_export::PlatformDocumentValidator {
            pool: &context.tenant.pool,
            platform: &state.file_platform,
        },
        payload,
    )
    .await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence/{id}",tag="correspondence",responses((status=200,body=ApiResponse<CorrespondenceDetail>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),params(("id"=Uuid,Path,description="Resource identifier"))))]
pub async fn get_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::detail(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence/{id}/read",tag="correspondence",request_body=CorrespondenceReadRequest,params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CorrespondenceReadReceipt>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse))))]
pub async fn read_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CorrespondenceReadRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        correspondence::read_with_changes(&context.tenant.pool, &context.actor, id, payload)
            .await?;
    if result.work_changed {
        state.notify_work_items_changed(&context.tenant.subdomain);
    }
    Ok(Json(ApiResponse::ok(result.value)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence/{id}/revisions",tag="correspondence",responses((status=200,body=ApiResponse<CorrespondenceDetail>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=ReviseCorrespondenceRequest,params(("id"=Uuid,Path,description="Resource identifier"))))]
pub async fn revise_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReviseCorrespondenceRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::revise(
        &context.tenant.pool,
        &context.actor,
        id,
        &crate::document_export::PlatformDocumentValidator {
            pool: &context.tenant.pool,
            platform: &state.file_platform,
        },
        payload,
    )
    .await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence/{id}/send",tag="correspondence",responses((status=200,body=ApiResponse<CorrespondenceDetail>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=SendCorrespondenceRequest,params(("id"=Uuid,Path,description="Resource identifier"))))]
pub async fn send_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SendCorrespondenceRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::send(&context.tenant.pool, &context.actor, id, payload).await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence/{id}/directed-send",tag="correspondence",request_body=DirectedSendCorrespondenceRequest,params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CorrespondenceDetail>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn directed_send_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<DirectedSendCorrespondenceRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        correspondence::directed_send(&context.tenant.pool, &context.actor, id, payload).await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence/routing-settings",tag="correspondence",responses((status=200,body=ApiResponse<Vec<CorrespondenceRoutingSettings>>),(status=403,body=ApiErrorResponse))))]
pub async fn correspondence_routing_settings(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        directed::settings(&context.tenant.pool, &context.actor).await?,
    ))
    .into_response())
}
#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/correspondence/routing-settings",tag="correspondence",request_body=SaveCorrespondenceRoutingSettingsRequest,responses((status=200,body=ApiResponse<CorrespondenceRoutingSettings>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_correspondence_routing_settings(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SaveCorrespondenceRoutingSettingsRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        directed::save_settings(&context.tenant.pool, &context.actor, payload).await?,
    ))
    .into_response())
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence/{id}/actions",tag="correspondence",responses((status=200,body=ApiResponse<CorrespondenceDetail>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=ProcessActionRequest,params(("id"=Uuid,Path,description="Resource identifier"))))]
pub async fn act_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProcessActionRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::act_with_changes(
        &context.tenant.pool,
        &context.actor,
        id,
        &crate::document_export::PlatformDocumentValidator {
            pool: &context.tenant.pool,
            platform: &state.file_platform,
        },
        payload,
    )
    .await?;
    if result.work_changed {
        state.notify_work_items_changed(&context.tenant.subdomain);
        school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    }
    Ok(Json(ApiResponse::ok(result.value)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence/options",tag="correspondence",params(CorrespondenceOptionsQuery),responses((status=200,body=ApiResponse<CorrespondenceOptions>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn correspondence_options(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<CorrespondenceOptionsQuery>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let mut result = correspondence::options(&context.tenant.pool, &context.actor, query).await?;
    result.stamp_context.school_name = match state
        .auth_runtime
        .admin_client
        .get_school_name(&context.tenant.subdomain)
        .await
    {
        Ok(name) => Some(name),
        Err(_) => {
            tracing::warn!("correspondence_school_name_lookup_failed");
            None
        }
    };
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence/signatures",tag="correspondence",responses((status=200,body=ApiResponse<Vec<SignatureAsset>>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_signature_assets(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::signatures(&context.tenant.pool, &context.actor).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/correspondence/signatures",tag="correspondence",responses((status=200,body=ApiResponse<SignatureAsset>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=CreateSignatureAssetRequest))]
pub async fn create_signature_asset(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateSignatureAssetRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        correspondence::create_signature(&context.tenant.pool, &context.actor, payload).await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/process/templates",tag="correspondence",responses((status=200,body=ApiResponse<Vec<ProcessTemplate>>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_process_templates(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = {
        if !context.actor.has_any_permission(&[
            school_permissions::registry::codes::CORRESPONDENCE_CREATE_OWN,
            school_permissions::registry::codes::PROCESS_MANAGE_SCHOOL,
        ]) {
            return Err(process::denied().into());
        }
        process::list_templates(&context.tenant.pool).await?
    };
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/process/templates",tag="correspondence",responses((status=200,body=ApiResponse<ProcessTemplate>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=SaveProcessTemplateRequest))]
pub async fn create_process_template(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SaveProcessTemplateRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        process::save_template(&context.tenant.pool, &context.actor, None, payload).await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/process/templates/{id}",tag="correspondence",responses((status=200,body=ApiResponse<ProcessTemplate>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),request_body=SaveProcessTemplateRequest,params(("id"=Uuid,Path,description="Resource identifier"))))]
pub async fn update_process_template(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveProcessTemplateRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        process::save_template(&context.tenant.pool, &context.actor, Some(id), payload).await?;
    state.notify_work_items_changed(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/process/instances/{id}",tag="correspondence",responses((status=200,body=ApiResponse<ProcessInstance>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),params(("id"=Uuid,Path,description="Resource identifier"))))]
pub async fn get_process_instance(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::instance_detail(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence/{id}/export",tag="correspondence",params(("id"=Uuid,Path)),responses((status=200,description="Server composed PDF",content_type="application/pdf"))))]
pub async fn export_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let bytes = crate::document_export::export(
        &context.tenant.pool,
        &context.actor,
        &state.file_platform,
        id,
    )
    .await?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/pdf"),
            (axum::http::header::CACHE_CONTROL, "private, no-store"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=correspondence.pdf",
            ),
        ],
        bytes,
    )
        .into_response())
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/process/instances/{id}/actions",tag="correspondence",params(("id"=Uuid,Path)),request_body=ProcessActionRequest,responses((status=200,body=ApiResponse<ProcessInstance>),(status=400,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn act_process_instance(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProcessActionRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = correspondence::act_instance_with_changes(
        &context.tenant.pool,
        &context.actor,
        id,
        &crate::document_export::PlatformDocumentValidator {
            pool: &context.tenant.pool,
            platform: &state.file_platform,
        },
        payload,
    )
    .await?;
    if result.work_changed {
        state.notify_work_items_changed(&context.tenant.subdomain);
        school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    }
    Ok(Json(ApiResponse::ok(result.value)).into_response())
}
pub fn routes() -> Router<WorkflowHttpState> {
    Router::new()
        .route(
            "/correspondence",
            get(list_correspondence).post(create_correspondence),
        )
        .route("/correspondence/options", get(correspondence_options))
        .route(
            "/correspondence/routing-settings",
            get(correspondence_routing_settings).put(save_correspondence_routing_settings),
        )
        .route(
            "/correspondence/signatures",
            get(list_signature_assets).post(create_signature_asset),
        )
        .route("/correspondence/{id}", get(get_correspondence))
        .route("/correspondence/{id}/read", post(read_correspondence))
        .route(
            "/correspondence/{id}/revisions",
            post(revise_correspondence),
        )
        .route("/correspondence/{id}/send", post(send_correspondence))
        .route(
            "/correspondence/{id}/directed-send",
            post(directed_send_correspondence),
        )
        .route("/correspondence/{id}/actions", post(act_correspondence))
        .route("/correspondence/{id}/export", get(export_correspondence))
        .route("/correspondence/{id}/preview", get(preview_correspondence))
        .route(
            "/process/templates",
            get(list_process_templates).post(create_process_template),
        )
        .route("/process/templates/{id}", put(update_process_template))
        .route("/process/instances/{id}", get(get_process_instance))
        .route(
            "/process/instances/{id}/actions",
            post(act_process_instance),
        )
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrespondencePreviewQuery {
    pub document_revision: i64,
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/correspondence/{id}/preview",tag="correspondence",params(("id"=Uuid,Path),("documentRevision"=i64,Query)),responses((status=200,description="Canonical source-only PDF",content_type="application/pdf"))))]
pub async fn preview_correspondence(
    State(state): State<WorkflowHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(query): Query<CorrespondencePreviewQuery>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let bytes = crate::document_export::preview(
        &context.tenant.pool,
        &context.actor,
        &state.file_platform,
        id,
        query.document_revision,
    )
    .await?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/pdf"),
            (axum::http::header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes,
    )
        .into_response())
}
