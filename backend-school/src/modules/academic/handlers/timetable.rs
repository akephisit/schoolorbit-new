use axum::{
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    CreateBatchTimetableEntriesRequest, CreateTimetableEntryRequest, DeleteTimetableEntryQuery,
    SwapTimetableEntriesRequest, TimetableOccupancyQuery, TimetableQuery,
    UpdateTimetableEntryRequest, ValidateMovesRequest,
};
use crate::modules::academic::services::{daily_teaching_service, timetable_service};
use crate::modules::academic::websockets::TimetableEvent;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::policies::learning_offering_access_policy::{
    require_learning_group_access, require_learning_offering_list_access, OfferingAction,
};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/academic/timetable",
    operation_id = "listTimetableEntries",
    params(TimetableQuery),
    responses(
        (status = 200, description = "Timetable entries in the selected term", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable read permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn list_timetable_entries(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<TimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    let entries = timetable_service::list_entries(&context.tenant.pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(entries)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/me/timetable",
    operation_id = "getMyTimetable",
    params(TimetableQuery),
    responses(
        (status = 200, description = "Current staff or student timetable in the selected term", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Staff timetable access denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn get_my_timetable(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(mut query): Query<TimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    if session.user_type == "student" {
        let entries = timetable_service::list_student_entries(
            &session.tenant.pool,
            query.academic_term_id,
            session.user_id,
        )
        .await?;
        return Ok(Json(ApiResponse::ok(entries)).into_response());
    }
    if session.user_type != "staff" {
        return Err(AppError::Forbidden(
            "บัญชีนี้ไม่มีตารางเรียนหรือตารางสอนส่วนบุคคล".to_string(),
        ));
    }
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    query.instructor_id = Some(context.actor.user_id);
    let entries = timetable_service::list_entries(&context.tenant.pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(entries)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable",
    operation_id = "createTimetableEntry",
    request_body = CreateTimetableEntryRequest,
    responses(
        (status = 200, description = "Created timetable entry", body = ApiResponse<crate::modules::academic::models::timetable::TimetableEntry>),
        (status = 400, description = "Invalid timetable entry", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 409, description = "Timetable conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn create_timetable_entry(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<CreateTimetableEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_create_access(&context, payload.learning_group_id).await?;
    let entry =
        timetable_service::create_entry(&context.tenant.pool, context.actor.user_id, payload)
            .await?;
    broadcast_entry_changed(&state, &headers, context.actor.user_id, &entry);
    Ok(Json(ApiResponse::ok(entry)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable/batch",
    operation_id = "createBatchTimetableEntries",
    request_body = CreateBatchTimetableEntriesRequest,
    responses(
        (status = 200, description = "Created timetable entries", body = ApiResponse<crate::modules::academic::models::timetable::BatchTimetableResult>),
        (status = 400, description = "Invalid timetable batch", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 409, description = "Timetable conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn create_batch_timetable_entries(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<CreateBatchTimetableEntriesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    if payload.learning_group_ids.is_empty() {
        context
            .actor
            .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    } else {
        for group_id in &payload.learning_group_ids {
            require_learning_group_access(
                &context.tenant.pool,
                &context.actor,
                *group_id,
                OfferingAction::Manage,
            )
            .await?;
        }
    }
    let result =
        timetable_service::create_batch(&context.tenant.pool, context.actor.user_id, payload)
            .await?;
    for entry in &result.entries {
        broadcast_entry_changed(&state, &headers, context.actor.user_id, entry);
    }
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/timetable/{entry_id}",
    operation_id = "updateTimetableEntry",
    params(("entry_id" = Uuid, Path, description = "Timetable entry ID")),
    request_body = UpdateTimetableEntryRequest,
    responses(
        (status = 200, description = "Updated timetable entry", body = ApiResponse<crate::modules::academic::models::timetable::TimetableEntry>),
        (status = 400, description = "Invalid timetable update", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Timetable entry not found", body = ApiErrorResponse),
        (status = 409, description = "Stale version or timetable conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn update_timetable_entry(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(entry_id): Path<Uuid>,
    Json(payload): Json<UpdateTimetableEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_entry_manage_access(&context, entry_id).await?;
    let entry = timetable_service::update_entry(
        &context.tenant.pool,
        entry_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    broadcast_entry_changed(&state, &headers, context.actor.user_id, &entry);
    Ok(Json(ApiResponse::ok(entry)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable/{entry_id}",
    operation_id = "deleteTimetableEntry",
    params(
        ("entry_id" = Uuid, Path, description = "Timetable entry ID"),
        DeleteTimetableEntryQuery
    ),
    responses(
        (status = 200, description = "Deactivated timetable entry", body = ApiResponse<crate::modules::academic::models::timetable::TimetableEntry>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Timetable entry not found", body = ApiErrorResponse),
        (status = 409, description = "Stale timetable entry version", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn delete_timetable_entry(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(entry_id): Path<Uuid>,
    Query(query): Query<DeleteTimetableEntryQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_entry_manage_access(&context, entry_id).await?;
    let entry = timetable_service::deactivate_entry(
        &context.tenant.pool,
        entry_id,
        query.row_version,
        context.actor.user_id,
    )
    .await?;
    broadcast_entry_changed(&state, &headers, context.actor.user_id, &entry);
    Ok(Json(ApiResponse::ok(entry)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable/batch-group/{batch_id}",
    operation_id = "deleteTimetableBatch",
    params(("batch_id" = Uuid, Path, description = "Timetable batch ID")),
    responses(
        (status = 200, description = "Deactivated timetable batch", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn delete_batch_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(batch_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let entries =
        timetable_service::deactivate_batch(&context.tenant.pool, batch_id, context.actor.user_id)
            .await?;
    for entry in &entries {
        broadcast_entry_changed(&state, &headers, context.actor.user_id, entry);
    }
    Ok(Json(ApiResponse::ok(entries)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable/swap",
    operation_id = "swapTimetableEntries",
    request_body = SwapTimetableEntriesRequest,
    responses(
        (status = 200, description = "Swapped timetable entries", body = ApiResponse<crate::modules::academic::models::timetable::SwapTimetableEntriesResponse>),
        (status = 400, description = "Invalid timetable swap", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse),
        (status = 409, description = "Stale version or timetable conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn swap_timetable_entries(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<SwapTimetableEntriesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_entry_manage_access(&context, payload.entry_a_id).await?;
    require_existing_entry_manage_access(&context, payload.entry_b_id).await?;
    let result =
        timetable_service::swap_entries(&context.tenant.pool, context.actor.user_id, payload)
            .await?;
    broadcast_entry_changed(&state, &headers, context.actor.user_id, &result.entry_a);
    broadcast_entry_changed(&state, &headers, context.actor.user_id, &result.entry_b);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable/validate-moves",
    operation_id = "validateTimetableMoves",
    request_body = ValidateMovesRequest,
    responses(
        (status = 200, description = "Valid timetable destinations", body = ApiResponse<Vec<crate::modules::academic::models::timetable::MoveValidityCell>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn validate_timetable_moves(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<ValidateMovesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_entry_manage_access(&context, payload.entry_id).await?;
    let cells = timetable_service::validate_moves(
        &context.tenant.pool,
        payload.academic_term_id,
        payload.entry_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(cells)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/timetable/occupancy",
    operation_id = "getTimetableOccupancy",
    params(TimetableOccupancyQuery),
    responses(
        (status = 200, description = "Timetable occupancy for the selected term", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableOccupancyCell>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable read permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn get_timetable_occupancy(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<TimetableOccupancyQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_READ_SCHOOL)?;
    let occupancy =
        timetable_service::occupancy(&context.tenant.pool, query.academic_term_id).await?;
    Ok(Json(ApiResponse::ok(occupancy)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/timetable/daily-teaching",
    operation_id = "getDailyTeachingOverview",
    params(daily_teaching_service::DailyTeachingQuery),
    responses((status = 200, body = ApiResponse<daily_teaching_service::DailyTeachingOverview>)),
    tag = "academic"
)]
pub async fn daily_teaching_overview(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<daily_teaching_service::DailyTeachingQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_READ_SCHOOL)?;
    let overview =
        daily_teaching_service::get_daily_teaching_overview(&context.tenant.pool, query).await?;
    Ok(Json(ApiResponse::ok(overview)).into_response())
}

async fn require_create_access(
    context: &crate::utils::request_context::ActorTenantContext,
    learning_group_id: Option<Uuid>,
) -> Result<(), AppError> {
    if let Some(group_id) = learning_group_id {
        require_learning_group_access(
            &context.tenant.pool,
            &context.actor,
            group_id,
            OfferingAction::Manage,
        )
        .await?;
    } else {
        context
            .actor
            .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    }
    Ok(())
}

async fn require_existing_entry_manage_access(
    context: &crate::utils::request_context::ActorTenantContext,
    entry_id: Uuid,
) -> Result<(), AppError> {
    let entry = timetable_service::get_entry(&context.tenant.pool, entry_id).await?;
    require_create_access(context, entry.learning_group_id).await
}

fn broadcast_entry_changed(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    entry: &crate::modules::academic::models::timetable::TimetableEntry,
) {
    let subdomain =
        extract_subdomain_from_request(headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        entry.academic_term_id,
        TimetableEvent::TimetableChanged {
            user_id,
            academic_term_id: entry.academic_term_id,
            learning_group_id: entry.learning_group_id,
            revision: entry.row_version,
        },
    );
}
