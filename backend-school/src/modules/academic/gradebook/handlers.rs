use super::{models::*, services};
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::{
    api_response::{ApiErrorResponse, ApiResponse},
    error::AppError,
    AppState,
};
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use uuid::Uuid;

fn signal(state: &AppState, session: &AuthenticatedSession, context: &GradebookContext, id: Uuid) {
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "gradebook",
        Some(id),
        Some(context.academic_year_id),
        Some(context.academic_term_id),
    );
}

#[utoipa::path(get,path="/api/academic/gradebook/subjects",operation_id="listGradebookSubjects",tag="academic",params(GradebookContext),responses((status=200,body=ApiResponse<Vec<GradebookSubject>>),(status=403,body=ApiErrorResponse)))]
pub async fn list_subjects(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<GradebookContext>,
) -> Result<Json<ApiResponse<Vec<GradebookSubject>>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_subjects(&context.tenant.pool, &context.actor, &query).await?,
    )))
}
#[utoipa::path(get,path="/api/academic/gradebook/controls",operation_id="listGradebookControls",tag="academic",params(GradebookContext),responses((status=200,body=ApiResponse<Vec<GradebookControl>>),(status=403,body=ApiErrorResponse)))]
pub async fn list_controls(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<GradebookContext>,
) -> Result<Json<ApiResponse<Vec<GradebookControl>>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_controls(&context.tenant.pool, &context.actor, &query).await?,
    )))
}
#[utoipa::path(put,path="/api/academic/gradebook/controls/{control_id}",operation_id="updateGradebookControl",tag="academic",params(GradebookContext,("control_id"=Uuid,Path)),request_body=UpdateControlInput,responses((status=200,body=ApiResponse<GradebookControl>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn update_control(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(query): Query<GradebookContext>,
    Json(input): Json<UpdateControlInput>,
) -> Result<Json<ApiResponse<GradebookControl>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result =
        services::update_control(&context.tenant.pool, &context.actor, id, &query, input).await?;
    signal(&state, &session, &query, id);
    Ok(Json(ApiResponse::ok(result)))
}
#[utoipa::path(get,path="/api/academic/gradebook/groups/{group_id}/phases/{phase_id}",operation_id="getGradebookGroupPhaseWorkspace",tag="academic",params(GradebookContext,("group_id"=Uuid,Path),("phase_id"=Uuid,Path)),responses((status=200,body=ApiResponse<GroupPhaseWorkspace>),(status=403,body=ApiErrorResponse)))]
pub async fn get_group_phase_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group, phase)): Path<(Uuid, Uuid)>,
    Query(query): Query<GradebookContext>,
) -> Result<Json<ApiResponse<GroupPhaseWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_group_phase_workspace(
            &context.tenant.pool,
            &context.actor,
            group,
            phase,
            &query,
        )
        .await?,
    )))
}
#[utoipa::path(post,path="/api/academic/gradebook/groups/{group_id}/phases/{phase_id}/items",operation_id="createGradebookItem",tag="academic",params(GradebookContext,("group_id"=Uuid,Path),("phase_id"=Uuid,Path)),request_body=ItemInput,responses((status=200,body=ApiResponse<ScoreItem>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn create_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group, phase)): Path<(Uuid, Uuid)>,
    Query(query): Query<GradebookContext>,
    Json(input): Json<ItemInput>,
) -> Result<Json<ApiResponse<ScoreItem>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::create_item(
        &context.tenant.pool,
        &context.actor,
        group,
        phase,
        &query,
        input,
    )
    .await?;
    signal(&state, &session, &query, group);
    Ok(Json(ApiResponse::ok(result)))
}
#[utoipa::path(put,path="/api/academic/gradebook/groups/{group_id}/phases/{phase_id}/items/{item_id}",operation_id="updateGradebookItem",tag="academic",params(GradebookContext,("group_id"=Uuid,Path),("phase_id"=Uuid,Path),("item_id"=Uuid,Path)),request_body=ItemInput,responses((status=200,body=ApiResponse<ScoreItem>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn update_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group, phase, item)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<GradebookContext>,
    Json(input): Json<ItemInput>,
) -> Result<Json<ApiResponse<ScoreItem>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::update_item(
        &context.tenant.pool,
        &context.actor,
        group,
        phase,
        item,
        &query,
        input,
    )
    .await?;
    signal(&state, &session, &query, group);
    Ok(Json(ApiResponse::ok(result)))
}
#[utoipa::path(delete,path="/api/academic/gradebook/groups/{group_id}/phases/{phase_id}/items/{item_id}",operation_id="removeGradebookItem",tag="academic",params(GradebookContext,("group_id"=Uuid,Path),("phase_id"=Uuid,Path),("item_id"=Uuid,Path)),request_body=RemoveItemInput,responses((status=200,body=ApiResponse<ScoreItemRemovalOutcome>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn remove_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group, phase, item)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<GradebookContext>,
    Json(input): Json<RemoveItemInput>,
) -> Result<Json<ApiResponse<ScoreItemRemovalOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::remove_item(
        &context.tenant.pool,
        &context.actor,
        group,
        phase,
        item,
        &query,
        input.row_version,
    )
    .await?;
    signal(&state, &session, &query, group);
    Ok(Json(ApiResponse::ok(result)))
}
#[utoipa::path(put,path="/api/academic/gradebook/groups/{group_id}/phases/{phase_id}/scores",operation_id="saveGradebookScoresBatch",tag="academic",params(GradebookContext,("group_id"=Uuid,Path),("phase_id"=Uuid,Path)),request_body=ScoreBatchInput,responses((status=200,body=ApiResponse<ScoreBatchOutcome>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn save_scores_batch(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group, phase)): Path<(Uuid, Uuid)>,
    Query(query): Query<GradebookContext>,
    Json(input): Json<ScoreBatchInput>,
) -> Result<Json<ApiResponse<ScoreBatchOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_scores_batch(
        &context.tenant.pool,
        &context.actor,
        group,
        phase,
        &query,
        input.cells,
    )
    .await?;
    signal(&state, &session, &query, group);
    Ok(Json(ApiResponse::ok(result)))
}
#[utoipa::path(post,path="/api/academic/gradebook/groups/{group_id}/phases/{phase_id}/confirm",operation_id="confirmGradebookPhase",tag="academic",params(GradebookContext,("group_id"=Uuid,Path),("phase_id"=Uuid,Path)),request_body=ConfirmInput,responses((status=200,body=ApiResponse<PhaseConfirmation>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn confirm_phase(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group, phase)): Path<(Uuid, Uuid)>,
    Query(query): Query<GradebookContext>,
    Json(input): Json<ConfirmInput>,
) -> Result<Json<ApiResponse<PhaseConfirmation>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::confirm_phase(
        &context.tenant.pool,
        &context.actor,
        group,
        phase,
        &query,
        input,
    )
    .await?;
    signal(&state, &session, &query, group);
    Ok(Json(ApiResponse::ok(result)))
}
