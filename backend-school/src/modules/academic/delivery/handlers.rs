use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::policies::learning_offering_access_policy::{self, OfferingAction};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

use super::models::*;
use super::services::{groups, offerings};

fn ok<T: serde::Serialize>(data: T) -> Response {
    Json(ApiResponse::ok(data)).into_response()
}

fn created<T: serde::Serialize>(data: T) -> Response {
    (StatusCode::CREATED, Json(ApiResponse::ok(data))).into_response()
}

fn signal_delivery_changed(
    state: &AppState,
    session: &AuthenticatedSession,
    actor: &ActorContext,
    academic_term_id: Uuid,
    learning_offering_id: Uuid,
    learning_group_id: Option<Uuid>,
    revision: i64,
) {
    state.websocket_manager.broadcast_learning_delivery_changed(
        session.tenant.subdomain.clone(),
        actor.user_id,
        academic_term_id,
        learning_offering_id,
        learning_group_id,
        revision,
    );
}

pub async fn list_offerings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<LearningOfferingQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(
        offerings::list(&context.tenant.pool, query, &filter).await?
    ))
}

pub async fn create_offering(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateLearningOfferingRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    if !learning_offering_access_policy::learning_offering_owner_allowed(
        &filter,
        request.owning_organization_unit_id(),
    ) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์สร้างรายการเปิดสอนให้หน่วยงานนี้".to_string(),
        ));
    }
    let offering = offerings::create(&context.tenant.pool, context.actor.user_id, request).await?;
    signal_delivery_changed(
        &state,
        &session,
        &context.actor,
        offering.academic_term_id,
        offering.id,
        None,
        offering.row_version,
    );
    Ok(created(offering))
}

pub async fn preview_offerings_from_curriculum(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<PreviewCurriculumOfferingsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    Ok(ok(offerings::preview_from_curriculum(
        &context.tenant.pool,
        request,
    )
    .await?))
}

pub async fn apply_offerings_from_curriculum(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<ApplyCurriculumOfferingsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    if !learning_offering_access_policy::learning_offering_owner_allowed(
        &filter,
        request.owning_organization_unit_id,
    ) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์สร้างรายการเปิดสอนให้หน่วยงานนี้".to_string(),
        ));
    }
    let result =
        offerings::apply_from_curriculum(&context.tenant.pool, context.actor.user_id, request)
            .await?;
    for offering_id in &result.offering_ids {
        let offering = offerings::get(&context.tenant.pool, *offering_id).await?;
        signal_delivery_changed(
            &state,
            &session,
            &context.actor,
            offering.academic_term_id,
            offering.id,
            None,
            offering.row_version,
        );
    }
    Ok(ok(result))
}

pub async fn get_offering(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(offerings::get(&context.tenant.pool, id).await?))
}

pub async fn update_offering(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateLearningOfferingRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    if !learning_offering_access_policy::learning_offering_owner_allowed(
        &filter,
        request.owning_organization_unit_id,
    ) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ย้ายรายการเปิดสอนไปยังหน่วยงานนี้".to_string(),
        ));
    }
    let offering =
        offerings::update(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_delivery_changed(
        &state,
        &session,
        &context.actor,
        offering.academic_term_id,
        offering.id,
        None,
        offering.row_version,
    );
    Ok(ok(offering))
}

pub async fn publish_offering(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishLearningOfferingRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let offering =
        offerings::publish(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_delivery_changed(
        &state,
        &session,
        &context.actor,
        offering.academic_term_id,
        offering.id,
        None,
        offering.row_version,
    );
    Ok(ok(offering))
}

pub async fn list_groups(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(offering_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        offering_id,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(groups::list(&context.tenant.pool, offering_id).await?))
}

pub async fn create_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(offering_id): Path<Uuid>,
    Json(request): Json<CreateLearningGroupRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        offering_id,
        OfferingAction::Manage,
    )
    .await?;
    let group = groups::create(
        &context.tenant.pool,
        context.actor.user_id,
        offering_id,
        request,
    )
    .await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(created(group))
}

pub async fn get_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(groups::get(&context.tenant.pool, id).await?))
}

pub async fn update_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateLearningGroupRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let group = groups::update(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(ok(group))
}

pub async fn list_group_homerooms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(groups::get(&context.tenant.pool, id)
        .await?
        .homeroom_ids))
}

pub async fn replace_group_homerooms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceLearningGroupHomeroomsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let group =
        groups::replace_homerooms(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(ok(group))
}

pub async fn list_group_teachers(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(groups::get(&context.tenant.pool, id)
        .await?
        .teacher_assignments))
}

pub async fn replace_group_teachers(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceLearningGroupTeachersRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let group =
        groups::replace_teachers(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(ok(group))
}

pub async fn apply_group_roster(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ApplyRosterRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let group =
        groups::apply_roster(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(ok(group))
}

pub async fn preview_group_roster(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    Ok(ok(groups::preview_roster(&context.tenant.pool, id).await?))
}

pub async fn publish_group_roster(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishRosterRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let group =
        groups::publish_roster(&context.tenant.pool, context.actor.user_id, id, request).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(ok(group))
}

fn signal_group_changed(
    state: &AppState,
    session: &AuthenticatedSession,
    actor: &ActorContext,
    group: &LearningGroup,
) {
    signal_delivery_changed(
        state,
        session,
        actor,
        group.academic_term_id,
        group.learning_offering_id,
        Some(group.id),
        group.row_version,
    );
}
