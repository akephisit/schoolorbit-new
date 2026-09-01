use axum::{
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::timetable::PersonalTimetableQuery;
use crate::modules::academic::models::timetable_block::{
    CreateOrdinaryTimetableBlockRequest, CreateStructuralTimetableBlocksRequest,
    CreateSynchronizedTimetableBlockRequest, DeleteTimetableBlockQuery,
    DeleteTimetableBlockSeriesQuery, RemoveTimetableBlockTargetRequest,
    RestoreTimetableBlockGroupRequest, RetryTimetableBlockSyncRequest, SwapTimetableBlocksRequest,
    TimetableBlock, TimetableBlockKind, TimetableBlockPlacementPreviewRequest,
    TimetableBlockWorkspaceQuery, TimetableTargetKind, UpdateTimetableBlockRequest,
};
use crate::modules::academic::services::{
    daily_teaching_service, timetable_block_service, timetable_version_service,
};
use crate::modules::academic::websockets::TimetableEvent;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::policies::timetable_access_policy::{
    require_timetable_list_access, require_timetable_resources, TimetableAction,
    TimetableResourceSet,
};
use crate::utils::request_context::{actor_tenant_context_from_session, ActorTenantContext};
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/academic/timetable-blocks/workspace",
    operation_id = "getTimetableBlockWorkspace",
    params(TimetableBlockWorkspaceQuery),
    responses(
        (status = 200, body = ApiResponse<crate::modules::academic::models::timetable_block::TimetableBlockWorkspace>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 404, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn get_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<TimetableBlockWorkspaceQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access =
        require_timetable_list_access(&context.tenant.pool, &context.actor, TimetableAction::Read)
            .await?;
    let workspace =
        timetable_block_service::get_workspace(&context.tenant.pool, query, &access).await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/placement-preview",
    operation_id = "previewTimetableBlockPlacement",
    request_body = TimetableBlockPlacementPreviewRequest,
    responses(
        (status = 200, body = ApiResponse<crate::modules::academic::models::timetable_block::TimetableBlockPlacementPreview>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn preview_placement(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<TimetableBlockPlacementPreviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let mut resources = TimetableResourceSet {
        timetable_version_ids: vec![payload.timetable_version_id],
        learning_offering_ids: payload.candidate.learning_offering_id.into_iter().collect(),
        learning_group_ids: payload.candidate.learning_group_id.into_iter().collect(),
        homeroom_ids: payload.candidate.homeroom_ids.clone(),
        teacher_ids: payload
            .candidate
            .teacher_ids
            .iter()
            .chain(payload.candidate.instructor_ids.iter())
            .copied()
            .collect(),
        room_ids: payload.candidate.room_id.into_iter().collect(),
        requires_school_scope: payload.candidate.block_kind == TimetableBlockKind::Structural,
    };
    if let Some(block_id) = payload.expected_target_block_id {
        resources_from_block(&context, block_id, &mut resources).await?;
    }
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &resources,
    )
    .await?;
    let preview = timetable_block_service::preview_placement(&context.tenant.pool, payload).await?;
    Ok(Json(ApiResponse::ok(preview)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/ordinary",
    operation_id = "createOrdinaryTimetableBlock",
    request_body = CreateOrdinaryTimetableBlockRequest,
    responses(
        (status = 200, body = ApiResponse<TimetableBlock>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn create_ordinary(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<CreateOrdinaryTimetableBlockRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &TimetableResourceSet {
            timetable_version_ids: vec![payload.timetable_version_id],
            learning_group_ids: vec![payload.learning_group_id],
            teacher_ids: payload.instructor_ids.clone(),
            room_ids: payload.room_id.into_iter().collect(),
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let block = timetable_block_service::create_ordinary_block(
        &context.tenant.pool,
        context.actor.user_id,
        payload,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/synchronized",
    operation_id = "createSynchronizedTimetableBlock",
    request_body = CreateSynchronizedTimetableBlockRequest,
    responses(
        (status = 200, body = ApiResponse<TimetableBlock>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn create_synchronized(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<CreateSynchronizedTimetableBlockRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &TimetableResourceSet {
            timetable_version_ids: vec![payload.timetable_version_id],
            learning_offering_ids: vec![payload.learning_offering_id],
            homeroom_ids: payload.intended_homeroom_ids.clone(),
            room_ids: payload.room_id.into_iter().collect(),
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let block = timetable_block_service::create_synchronized_block(
        &context.tenant.pool,
        context.actor.user_id,
        payload,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/structural",
    operation_id = "createStructuralTimetableBlocks",
    request_body = CreateStructuralTimetableBlocksRequest,
    responses(
        (status = 200, body = ApiResponse<Vec<TimetableBlock>>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn create_structural(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<CreateStructuralTimetableBlocksRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &TimetableResourceSet {
            timetable_version_ids: vec![payload.timetable_version_id],
            homeroom_ids: payload.homeroom_ids.clone(),
            teacher_ids: payload.teacher_ids.clone(),
            room_ids: payload.room_id.into_iter().collect(),
            requires_school_scope: payload.all_homerooms || payload.all_teachers,
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let blocks = timetable_block_service::create_structural_blocks(
        &context.tenant.pool,
        context.actor.user_id,
        payload,
    )
    .await?;
    for block in &blocks {
        broadcast_changed(&state, &headers, context.actor.user_id, block);
    }
    Ok(Json(ApiResponse::ok(blocks)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/timetable-blocks/{block_id}",
    operation_id = "updateTimetableBlock",
    params(("block_id" = Uuid, Path)),
    request_body = UpdateTimetableBlockRequest,
    responses(
        (status = 200, body = ApiResponse<TimetableBlock>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 404, body = ApiErrorResponse),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn update_block(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(block_id): Path<Uuid>,
    Json(payload): Json<UpdateTimetableBlockRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let mut resources = TimetableResourceSet {
        timetable_version_ids: vec![payload.timetable_version_id],
        teacher_ids: payload.instructor_ids.clone().unwrap_or_default(),
        room_ids: payload.room_id.into_iter().collect(),
        ..TimetableResourceSet::default()
    };
    resources_from_block(&context, block_id, &mut resources).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &resources,
    )
    .await?;
    let block = timetable_block_service::update_block(
        &context.tenant.pool,
        context.actor.user_id,
        block_id,
        payload,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable-blocks/{block_id}/targets",
    operation_id = "removeTimetableBlockTarget",
    params(("block_id" = Uuid, Path)),
    request_body = RemoveTimetableBlockTargetRequest,
    responses(
        (status = 200, body = ApiResponse<TimetableBlock>),
        (status = 400, body = ApiErrorResponse),
        (status = 401, body = ApiErrorResponse),
        (status = 403, body = ApiErrorResponse),
        (status = 404, body = ApiErrorResponse),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn remove_target(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(block_id): Path<Uuid>,
    Json(payload): Json<RemoveTimetableBlockTargetRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let mut resources = TimetableResourceSet {
        timetable_version_ids: vec![payload.timetable_version_id],
        ..TimetableResourceSet::default()
    };
    match payload.target_kind {
        TimetableTargetKind::Group => resources.learning_group_ids.push(payload.target_id),
        TimetableTargetKind::Homeroom => resources.homeroom_ids.push(payload.target_id),
        TimetableTargetKind::Teacher => resources.teacher_ids.push(payload.target_id),
    }
    resources_from_block(&context, block_id, &mut resources).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &resources,
    )
    .await?;
    let block = timetable_block_service::remove_target(
        &context.tenant.pool,
        block_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/{block_id}/sync",
    operation_id = "retryTimetableBlockSync",
    params(("block_id" = Uuid, Path)),
    request_body = RetryTimetableBlockSyncRequest,
    responses((status = 200, body = ApiResponse<TimetableBlock>), (status = 409, body = ApiErrorResponse)),
    tag = "academic"
)]
pub async fn retry_sync(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(block_id): Path<Uuid>,
    Json(payload): Json<RetryTimetableBlockSyncRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_block_manage_access(&context, block_id, payload.timetable_version_id).await?;
    let block = timetable_block_service::retry_sync(
        &context.tenant.pool,
        block_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/{block_id}/restore",
    operation_id = "restoreTimetableBlockGroup",
    params(("block_id" = Uuid, Path)),
    request_body = RestoreTimetableBlockGroupRequest,
    responses((status = 200, body = ApiResponse<TimetableBlock>), (status = 409, body = ApiErrorResponse)),
    tag = "academic"
)]
pub async fn restore_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(block_id): Path<Uuid>,
    Json(payload): Json<RestoreTimetableBlockGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_block_manage_access(&context, block_id, payload.timetable_version_id).await?;
    let block = timetable_block_service::restore_group(
        &context.tenant.pool,
        block_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable-blocks/{block_id}",
    operation_id = "deleteTimetableBlock",
    params(("block_id" = Uuid, Path), DeleteTimetableBlockQuery),
    responses((status = 200, body = ApiResponse<TimetableBlock>), (status = 409, body = ApiErrorResponse)),
    tag = "academic"
)]
pub async fn delete_block(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(block_id): Path<Uuid>,
    Query(query): Query<DeleteTimetableBlockQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_existing_block_manage_access(&context, block_id, query.timetable_version_id).await?;
    let block = timetable_block_service::deactivate_block(
        &context.tenant.pool,
        context.actor.user_id,
        block_id,
        query.timetable_version_id,
        query.row_version,
    )
    .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &block);
    Ok(Json(ApiResponse::ok(block)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable-blocks/series/{series_id}",
    operation_id = "deleteTimetableBlockSeries",
    params(("series_id" = Uuid, Path), DeleteTimetableBlockSeriesQuery),
    responses((status = 200, body = ApiResponse<Vec<TimetableBlock>>), (status = 409, body = ApiErrorResponse)),
    tag = "academic"
)]
pub async fn delete_series(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(series_id): Path<Uuid>,
    Query(query): Query<DeleteTimetableBlockSeriesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &TimetableResourceSet {
            timetable_version_ids: vec![query.timetable_version_id],
            requires_school_scope: true,
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let blocks = timetable_block_service::deactivate_series(
        &context.tenant.pool,
        series_id,
        query.timetable_version_id,
        context.actor.user_id,
    )
    .await?;
    for block in &blocks {
        broadcast_changed(&state, &headers, context.actor.user_id, block);
    }
    Ok(Json(ApiResponse::ok(blocks)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-blocks/swap",
    operation_id = "swapTimetableBlocks",
    request_body = SwapTimetableBlocksRequest,
    responses(
        (status = 200, body = ApiResponse<crate::modules::academic::models::timetable_block::SwapTimetableBlocksResponse>),
        (status = 409, body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn swap_blocks(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<SwapTimetableBlocksRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let mut resources = TimetableResourceSet {
        timetable_version_ids: vec![payload.timetable_version_id],
        ..TimetableResourceSet::default()
    };
    resources_from_block(&context, payload.block_a_id, &mut resources).await?;
    resources_from_block(&context, payload.block_b_id, &mut resources).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &resources,
    )
    .await?;
    let result =
        timetable_block_service::swap_blocks(&context.tenant.pool, context.actor.user_id, payload)
            .await?;
    broadcast_changed(&state, &headers, context.actor.user_id, &result.block_a);
    broadcast_changed(&state, &headers, context.actor.user_id, &result.block_b);
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/me/timetable",
    operation_id = "getMyTimetable",
    params(PersonalTimetableQuery),
    responses((status = 200, body = ApiResponse<Vec<TimetableBlock>>), (status = 403, body = ApiErrorResponse)),
    tag = "academic"
)]
pub async fn get_my_timetable(
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PersonalTimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let version = timetable_version_service::resolve_for_date(
        &session.tenant.pool,
        query.academic_term_id,
        query.date,
    )
    .await?;
    let blocks = match session.user_type.as_str() {
        "student" => {
            timetable_block_service::list_student_blocks(
                &session.tenant.pool,
                version.id,
                query.academic_term_id,
                session.user_id,
                query.date,
            )
            .await?
        }
        "staff" => {
            timetable_block_service::list_instructor_blocks(
                &session.tenant.pool,
                version.id,
                query.academic_term_id,
                session.user_id,
            )
            .await?
        }
        _ => {
            return Err(AppError::Forbidden(
                "บัญชีนี้ไม่มีตารางเรียนหรือตารางสอนส่วนบุคคล".to_string(),
            ))
        }
    };
    Ok(Json(ApiResponse::ok(blocks)).into_response())
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
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Read,
        &TimetableResourceSet {
            requires_school_scope: true,
            ..TimetableResourceSet::default()
        },
    )
    .await?;
    let overview =
        daily_teaching_service::get_daily_teaching_overview(&context.tenant.pool, query).await?;
    Ok(Json(ApiResponse::ok(overview)).into_response())
}

async fn resources_from_block(
    context: &ActorTenantContext,
    block_id: Uuid,
    resources: &mut TimetableResourceSet,
) -> Result<(), AppError> {
    let block = timetable_block_service::get_block(&context.tenant.pool, block_id).await?;
    resources
        .timetable_version_ids
        .push(block.timetable_version_id);
    resources
        .learning_offering_ids
        .extend(block.learning_offering_id);
    resources
        .learning_group_ids
        .extend(block.groups.iter().map(|group| group.learning_group_id));
    resources
        .homeroom_ids
        .extend(block.homerooms.iter().map(|target| target.homeroom_id));
    resources
        .teacher_ids
        .extend(block.teachers.iter().map(|target| target.teacher_id));
    resources.teacher_ids.extend(
        block
            .groups
            .iter()
            .flat_map(|group| group.instructors.iter())
            .map(|instructor| instructor.teacher_id),
    );
    resources.room_ids.extend(
        block
            .groups
            .iter()
            .filter_map(|group| group.room_id)
            .chain(block.homerooms.iter().filter_map(|target| target.room_id)),
    );
    resources.requires_school_scope |= block.block_kind == TimetableBlockKind::Structural;
    Ok(())
}

async fn require_existing_block_manage_access(
    context: &ActorTenantContext,
    block_id: Uuid,
    timetable_version_id: Uuid,
) -> Result<(), AppError> {
    let mut resources = TimetableResourceSet {
        timetable_version_ids: vec![timetable_version_id],
        ..TimetableResourceSet::default()
    };
    resources_from_block(&context, block_id, &mut resources).await?;
    require_timetable_resources(
        &context.tenant.pool,
        &context.actor,
        TimetableAction::Manage,
        &resources,
    )
    .await
}

fn broadcast_changed(state: &AppState, headers: &HeaderMap, user_id: Uuid, block: &TimetableBlock) {
    let subdomain =
        extract_subdomain_from_request(headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        block.academic_term_id,
        TimetableEvent::TimetableChanged {
            user_id,
            academic_term_id: block.academic_term_id,
            timetable_version_id: block.timetable_version_id,
            block_id: Some(block.id),
            revision: block.row_version,
        },
    );
}
