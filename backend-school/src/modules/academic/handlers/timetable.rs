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
    path = "/api/staff/me/timetable",
    operation_id = "getStaffTimetable",
    params(TimetableQuery),
    responses(
        (status = 200, description = "Current staff timetable in the selected term", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Staff timetable access denied", body = ApiErrorResponse)
    ),
    tag = "staff"
)]
pub async fn get_my_timetable(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(mut query): Query<TimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
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
