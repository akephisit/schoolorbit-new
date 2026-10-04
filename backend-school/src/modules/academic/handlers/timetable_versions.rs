use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::modules::academic::websockets::TimetableEvent;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use school_academic_timetable::models::timetable_publication::{
    PreviewTimetablePublicationRequest, PublishTimetableVersionRequest,
};
use school_academic_timetable::models::timetable_version::{
    CloneTimetableVersionRequest, CreateTimetableVersionRequest, DeleteTimetableDraftRequest,
    ResolveTimetableVersionQuery, TimetableVersionQuery, UpdateTimetableDeliverySourceRequest,
};
use school_academic_timetable::policy::{
    require_timetable_list_access, require_timetable_resources, TimetableAction,
    TimetableResourceSet,
};
use school_academic_timetable::services::{timetable_lifecycle, timetable_version_service};
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
use school_permissions::registry::codes;

#[utoipa::path(post,path="/api/academic/timetable-versions",operation_id="createTimetableVersion",request_body=CreateTimetableVersionRequest,
    responses((status=200,body=ApiResponse<school_academic_timetable::models::timetable_version::TimetableVersion>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),tag="academic")]
pub async fn create_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateTimetableVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL)?;
    let version = timetable_version_service::create_initial_draft(
        &context.tenant.pool,
        context.actor.user_id,
        request,
    )
    .await?;
    signal_version(
        &state,
        &context,
        version.academic_term_id,
        version.id,
        version.row_version,
    );
    Ok(Json(ApiResponse::ok(version)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/timetable-versions",
    operation_id = "listTimetableVersions",
    params(TimetableVersionQuery),
    responses(
        (status = 200, description = "Timetable versions for the selected term", body = ApiResponse<Vec<school_academic_timetable::models::timetable_version::TimetableVersion>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable read permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn list_versions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<TimetableVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter =
        require_timetable_list_access(&context.tenant.pool, &context.actor, TimetableAction::Read)
            .await?;
    let mut versions =
        timetable_version_service::list_versions(&context.tenant.pool, query.academic_term_id)
            .await?;
    timetable_version_service::restrict_targets(&context.tenant.pool, &mut versions, &filter)
        .await?;
    Ok(Json(ApiResponse::ok(versions)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/timetable-versions/resolve",
    operation_id = "resolveTimetableVersion",
    params(ResolveTimetableVersionQuery),
    responses(
        (status = 200, description = "Published timetable version effective on the selected date", body = ApiResponse<school_academic_timetable::models::timetable_version::TimetableVersion>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable read permission denied", body = ApiErrorResponse),
        (status = 404, description = "No effective timetable version", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn resolve_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResolveTimetableVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter =
        require_timetable_list_access(&context.tenant.pool, &context.actor, TimetableAction::Read)
            .await?;
    let mut version = timetable_version_service::resolve_for_date(
        &context.tenant.pool,
        query.academic_term_id,
        query.date,
    )
    .await?;
    timetable_version_service::restrict_targets(
        &context.tenant.pool,
        std::slice::from_mut(&mut version),
        &filter,
    )
    .await?;
    Ok(Json(ApiResponse::ok(version)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-versions/{source_id}/clone",
    operation_id = "cloneTimetableVersion",
    params(("source_id" = Uuid, Path, description = "Published timetable version ID")),
    request_body = CloneTimetableVersionRequest,
    responses(
        (status = 200, description = "Draft timetable version cloned from the published source", body = ApiResponse<school_academic_timetable::models::timetable_version::TimetableVersion>),
        (status = 400, description = "Invalid effective date", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Source timetable version not found", body = ApiErrorResponse),
        (status = 409, description = "Timetable version conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn clone_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(source_id): Path<Uuid>,
    Json(payload): Json<CloneTimetableVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL)?;
    let version = timetable_version_service::clone_draft(
        &context.tenant.pool,
        context.actor.user_id,
        source_id,
        payload,
    )
    .await?;
    signal_version(
        &state,
        &context,
        version.academic_term_id,
        version.id,
        version.row_version,
    );
    Ok(Json(ApiResponse::ok(version)).into_response())
}

#[utoipa::path(put,path="/api/academic/timetable-versions/{version_id}/delivery-source",operation_id="updateTimetableDeliverySource",
    params(("version_id"=Uuid,Path)),request_body=UpdateTimetableDeliverySourceRequest,
    responses((status=200,body=ApiResponse<school_academic_timetable::models::timetable_version::TimetableVersion>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),tag="academic")]
pub async fn update_delivery_source(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateTimetableDeliverySourceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &TimetableResourceSet {
            timetable_version_ids: vec![id],
            requires_school_scope: true,
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let version = timetable_lifecycle::update_source(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        request,
    )
    .await?;
    signal_version(
        &state,
        &context,
        version.academic_term_id,
        version.id,
        version.row_version,
    );
    Ok(Json(ApiResponse::ok(version)))
}

#[utoipa::path(post,path="/api/academic/timetable-versions/{version_id}/delete-draft",operation_id="deleteTimetableDraft",
    params(("version_id"=Uuid,Path)),request_body=DeleteTimetableDraftRequest,
    responses((status=200,body=ApiResponse<school_academic_timetable::models::timetable_version::DeletedTimetableDraft>),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),tag="academic")]
pub async fn delete_draft(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<DeleteTimetableDraftRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &TimetableResourceSet {
            timetable_version_ids: vec![id],
            requires_school_scope: true,
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let version = timetable_version_service::get_version(
        &context.tenant.pool,
        id,
        chrono::Utc::now().date_naive(),
    )
    .await?;
    let result =
        timetable_lifecycle::delete_draft(&context.tenant.pool, context.actor.user_id, id, request)
            .await?;
    signal_version(
        &state,
        &context,
        version.academic_term_id,
        id,
        version.row_version + 1,
    );
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/timetable-versions/{version_id}/publication-preview",operation_id="previewTimetablePublication",
    params(("version_id"=Uuid,Path)),request_body=PreviewTimetablePublicationRequest,
    responses((status=200,body=ApiResponse<school_academic_timetable::models::timetable_publication::TimetablePublicationPreview>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),tag="academic")]
pub async fn preview_publication(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PreviewTimetablePublicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Publish,
        &TimetableResourceSet {
            timetable_version_ids: vec![id],
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    Ok(Json(ApiResponse::ok(
        timetable_lifecycle::preview(&context.tenant.pool, id, request).await?,
    )))
}

#[utoipa::path(post,path="/api/academic/timetable-versions/{version_id}/publish",operation_id="publishTimetableVersion",
    params(("version_id"=Uuid,Path)),request_body=PublishTimetableVersionRequest,
    responses((status=200,body=ApiResponse<school_academic_timetable::models::timetable_version::TimetableVersion>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)),tag="academic")]
pub async fn publish_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishTimetableVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Publish,
        &TimetableResourceSet {
            timetable_version_ids: vec![id],
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let version =
        timetable_lifecycle::publish(&context.tenant.pool, context.actor.user_id, id, request)
            .await?;
    signal_version(
        &state,
        &context,
        version.academic_term_id,
        version.id,
        version.row_version,
    );
    Ok(Json(ApiResponse::ok(version)))
}

fn signal_version(
    state: &AppState,
    context: &crate::utils::request_context::ActorTenantContext,
    term_id: Uuid,
    version_id: Uuid,
    revision: i64,
) {
    state.websocket_manager.broadcast_mutation(
        context.tenant.subdomain.clone(),
        term_id,
        TimetableEvent::TimetableChanged {
            user_id: context.actor.user_id,
            academic_term_id: term_id,
            timetable_version_id: version_id,
            block_id: None,
            revision,
        },
    );
}
