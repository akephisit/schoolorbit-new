use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::policies::learning_offering_access_policy::{self, OfferingAction};
use crate::utils::request_context::{actor_tenant_context_from_session, ActorTenantContext};
use crate::AppState;

use super::models::*;
use super::services::{
    activities, change_sets, groups, offerings, roster_memberships, teacher_handoff, workspaces,
};

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

fn require_student_session(session: &AuthenticatedSession) -> Result<(), AppError> {
    if session.user_type == "student" {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "เฉพาะนักเรียนเท่านั้นที่จัดการการลงทะเบียนกิจกรรมของตนเองได้".to_string(),
        ))
    }
}

async fn require_term_change_set_access(
    context: &ActorTenantContext,
    change_set_id: Uuid,
    action: OfferingAction,
) -> Result<Vec<Uuid>, AppError> {
    let offering_ids =
        offerings::operational_change_offering_ids(&context.tenant.pool, change_set_id).await?;
    learning_offering_access_policy::require_learning_offering_batch_access(
        &context.tenant.pool,
        &context.actor,
        &offering_ids,
        action,
    )
    .await?;
    Ok(offering_ids)
}

async fn signal_term_change_set_changed(
    state: &AppState,
    session: &AuthenticatedSession,
    context: &ActorTenantContext,
    change_set: &AcademicTermChangeSet,
    prior_descriptors: &[offerings::LearningOfferingSignalDescriptor],
) -> Result<(), AppError> {
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        context.actor.user_id,
        "academic_term_change_set",
        Some(change_set.id),
        Some(change_set.academic_year_id),
        Some(change_set.academic_term_id),
    );
    let offering_ids =
        offerings::operational_change_offering_ids(&context.tenant.pool, change_set.id).await?;
    let current_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &offering_ids).await?;
    let mut descriptors = std::collections::BTreeMap::new();
    for descriptor in prior_descriptors.iter().chain(current_descriptors.iter()) {
        descriptors.insert(descriptor.learning_offering_id, descriptor.clone());
    }
    for descriptor in descriptors.into_values() {
        signal_delivery_changed(
            state,
            session,
            &context.actor,
            descriptor.academic_term_id,
            descriptor.learning_offering_id,
            None,
            descriptor.row_version,
        );
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/me/activity-registrations",
    operation_id = "listMyActivityRegistrations",
    tag = "academic",
    params(StudentActivityRegistrationQuery),
    responses(
        (status = 200, description = "Self-registration activity options for the selected learner term", body = ApiResponse<Vec<StudentActivityOfferingOption>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student activity access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_my_activity_registrations(
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<StudentActivityRegistrationQuery>,
) -> Result<Response, AppError> {
    require_student_session(&session)?;
    Ok(ok(activities::list_registration_options(
        &session.tenant.pool,
        session.user_id,
        query,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/me/activity-registrations/{group_id}",
    operation_id = "enrollMyActivityRegistration",
    tag = "academic",
    params(
        ("group_id" = Uuid, Path, description = "Learning group ID"),
        StudentActivityRegistrationQuery
    ),
    responses(
        (status = 200, description = "Student enrolled in the selected activity group", body = ApiResponse<StudentActivityRegistrationResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student is not eligible for the selected activity", body = ApiErrorResponse),
        (status = 404, description = "Activity group not found", body = ApiErrorResponse),
        (status = 409, description = "Activity registration conflict", body = ApiErrorResponse),
        (status = 422, description = "Academic term context mismatch", body = ApiErrorResponse)
    )
)]
pub async fn enroll_my_activity_registration(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<StudentActivityRegistrationQuery>,
) -> Result<Response, AppError> {
    require_student_session(&session)?;
    let academic_term_id = query.academic_term_id;
    let result = activities::enroll(&session.tenant.pool, session.user_id, group_id, query).await?;
    state.websocket_manager.broadcast_learning_delivery_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        academic_term_id,
        result.learning_offering_id,
        Some(result.learning_group_id),
        result.revision,
    );
    Ok(ok(result))
}

#[utoipa::path(
    delete,
    path = "/api/me/activity-registrations/{group_id}",
    operation_id = "unenrollMyActivityRegistration",
    tag = "academic",
    params(
        ("group_id" = Uuid, Path, description = "Learning group ID"),
        StudentActivityRegistrationQuery
    ),
    responses(
        (status = 200, description = "Student removed from the selected activity group", body = ApiResponse<StudentActivityRegistrationResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student activity access denied", body = ApiErrorResponse),
        (status = 404, description = "Active activity registration not found", body = ApiErrorResponse),
        (status = 409, description = "Activity registration window is closed", body = ApiErrorResponse),
        (status = 422, description = "Academic term context mismatch", body = ApiErrorResponse)
    )
)]
pub async fn unenroll_my_activity_registration(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<StudentActivityRegistrationQuery>,
) -> Result<Response, AppError> {
    require_student_session(&session)?;
    let academic_term_id = query.academic_term_id;
    let result =
        activities::unenroll(&session.tenant.pool, session.user_id, group_id, query).await?;
    state.websocket_manager.broadcast_learning_delivery_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        academic_term_id,
        result.learning_offering_id,
        Some(result.learning_group_id),
        result.revision,
    );
    Ok(ok(result))
}

#[utoipa::path(
    get,
    path = "/api/academic/offerings",
    operation_id = "listLearningOfferings",
    tag = "academic",
    params(LearningOfferingQuery),
    responses(
        (status = 200, description = "Learning offerings in the selected term", body = ApiResponse<Vec<LearningOffering>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/delivery/workspace",
    operation_id = "getLearningDeliveryOverview",
    tag = "academic",
    params(LearningOfferingQuery),
    responses(
        (status = 200, description = "Term-scoped learning delivery overview", body = ApiResponse<LearningDeliveryOverview>),
        (status = 400, description = "Invalid academic term query or oversized workspace", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn get_delivery_overview(
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
    Ok(ok(workspaces::delivery_overview(
        &context.tenant.pool,
        query.academic_term_id,
        &filter,
    )
    .await?))
}

#[utoipa::path(
    get,
    path = "/api/academic/delivery/homerooms",
    operation_id = "getHomeroomDeliveryWorkspace",
    tag = "academic",
    params(HomeroomDeliveryQuery),
    responses(
        (status = 200, description = "Homeroom-first curriculum delivery workspace", body = ApiResponse<HomeroomDeliveryWorkspace>),
        (status = 400, description = "Invalid academic year or term query", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found in the selected year", body = ApiErrorResponse)
    )
)]
pub async fn get_homeroom_delivery_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<HomeroomDeliveryQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(workspaces::homeroom_delivery_workspace_for_version(
        &context.tenant.pool,
        query.academic_year_id,
        query.academic_term_id,
        query.timetable_version_id,
        &filter,
    )
    .await?))
}

#[utoipa::path(
    get,
    path = "/api/academic/delivery/management-options",
    operation_id = "getLearningDeliveryManagementOptions",
    tag = "academic",
    params(LearningOfferingQuery),
    responses(
        (status = 200, description = "Term-scoped options for managing learning delivery", body = ApiResponse<DeliveryManagementOptions>),
        (status = 400, description = "Invalid academic term query or oversized option set", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found", body = ApiErrorResponse)
    )
)]
pub async fn get_delivery_management_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<LearningOfferingQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    Ok(ok(workspaces::delivery_management_options(
        &context.tenant.pool,
        query.academic_term_id,
        context.actor.user_id,
        &filter,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/offerings",
    operation_id = "createLearningOffering",
    tag = "academic",
    request_body = CreateLearningOfferingRequest,
    responses(
        (status = 201, description = "Learning offering created", body = ApiResponse<LearningOffering>),
        (status = 400, description = "Invalid learning offering", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Learning offering conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/offerings/preview-from-curriculum",
    operation_id = "previewLearningOfferingsFromCurriculum",
    tag = "academic",
    request_body = PreviewCurriculumOfferingsRequest,
    responses(
        (status = 200, description = "Curriculum offering preview", body = ApiResponse<CurriculumOfferingPreview>),
        (status = 400, description = "Invalid curriculum offering preview", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Curriculum source conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/offerings/apply-from-curriculum",
    operation_id = "applyLearningOfferingsFromCurriculum",
    tag = "academic",
    request_body = ApplyCurriculumOfferingsRequest,
    responses(
        (status = 200, description = "Curriculum offerings applied", body = ApiResponse<ApplyCurriculumOfferingsResult>),
        (status = 400, description = "Invalid curriculum offering request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Curriculum source hash conflict", body = ApiErrorResponse)
    )
)]
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
    let signal_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &result.offering_ids).await?;
    for descriptor in signal_descriptors {
        signal_delivery_changed(
            &state,
            &session,
            &context.actor,
            descriptor.academic_term_id,
            descriptor.learning_offering_id,
            None,
            descriptor.row_version,
        );
    }
    Ok(ok(result))
}

#[utoipa::path(
    get,
    path = "/api/academic/offerings/{id}",
    operation_id = "getLearningOffering",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning offering ID")),
    responses(
        (status = 200, description = "Learning offering", body = ApiResponse<LearningOffering>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning offering not found", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    patch,
    path = "/api/academic/offerings/{id}",
    operation_id = "updateLearningOffering",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning offering ID")),
    request_body = UpdateLearningOfferingRequest,
    responses(
        (status = 200, description = "Learning offering updated", body = ApiResponse<LearningOffering>),
        (status = 400, description = "Invalid or immutable learning offering", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning offering not found", body = ApiErrorResponse),
        (status = 409, description = "Learning offering row version conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/offerings/{id}/publish",
    operation_id = "publishLearningOffering",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning offering ID")),
    request_body = PublishLearningOfferingRequest,
    responses(
        (status = 200, description = "Learning offering published", body = ApiResponse<LearningOffering>),
        (status = 400, description = "Learning offering cannot be published", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning offering not found", body = ApiErrorResponse),
        (status = 409, description = "Learning offering publish conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/learning-groups",
    operation_id = "listLearningGroupsForTerm",
    tag = "academic",
    params(LearningGroupTermQuery),
    responses(
        (status = 200, description = "Learning groups in the selected term", body = ApiResponse<Vec<LearningGroup>>),
        (status = 400, description = "Invalid academic term query or oversized workspace", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_groups_for_term(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<LearningGroupTermQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(groups::list_for_term(
        &context.tenant.pool,
        query.academic_term_id,
        &filter,
    )
    .await?))
}

#[utoipa::path(
    get,
    path = "/api/academic/offerings/{id}/groups",
    operation_id = "listLearningGroups",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning offering ID")),
    responses(
        (status = 200, description = "Learning groups", body = ApiResponse<Vec<LearningGroup>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning offering not found", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/offerings/{id}/groups",
    operation_id = "createLearningGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning offering ID")),
    request_body = CreateLearningGroupRequest,
    responses(
        (status = 201, description = "Learning group created", body = ApiResponse<LearningGroup>),
        (status = 400, description = "Invalid learning group", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning offering not found", body = ApiErrorResponse),
        (status = 409, description = "Learning group conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/learning-groups/{id}",
    operation_id = "getLearningGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    responses(
        (status = 200, description = "Learning group", body = ApiResponse<LearningGroup>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    patch,
    path = "/api/academic/learning-groups/{id}",
    operation_id = "updateLearningGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    request_body = UpdateLearningGroupRequest,
    responses(
        (status = 200, description = "Learning group updated", body = ApiResponse<LearningGroup>),
        (status = 400, description = "Invalid learning group", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse),
        (status = 409, description = "Learning group row version conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/learning-groups/{id}/homerooms",
    operation_id = "listLearningGroupHomerooms",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    responses(
        (status = 200, description = "Learning group homeroom IDs", body = ApiResponse<LearningGroupHomeroomIds>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse)
    )
)]
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
    Ok(ok(LearningGroupHomeroomIds(
        groups::get(&context.tenant.pool, id).await?.homeroom_ids,
    )))
}

#[utoipa::path(
    put,
    path = "/api/academic/learning-groups/{id}/homerooms",
    operation_id = "replaceLearningGroupHomerooms",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    request_body = ReplaceLearningGroupHomeroomsRequest,
    responses(
        (status = 200, description = "Learning group homerooms replaced", body = ApiResponse<LearningGroup>),
        (status = 400, description = "Invalid learning group homerooms", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group or homeroom not found", body = ApiErrorResponse),
        (status = 409, description = "Learning group row version conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/learning-groups/{id}/teachers",
    operation_id = "listLearningGroupTeachers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    responses(
        (status = 200, description = "Learning group teachers", body = ApiResponse<Vec<LearningGroupTeacherAssignment>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    put,
    path = "/api/academic/learning-groups/{id}/teachers",
    operation_id = "replaceLearningGroupTeachers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    request_body = ReplaceLearningGroupTeachersRequest,
    responses(
        (status = 200, description = "Learning group teachers replaced", body = ApiResponse<LearningGroup>),
        (status = 400, description = "Invalid learning group teachers", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group or teacher not found", body = ApiErrorResponse),
        (status = 409, description = "Learning group row version conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    put,
    path = "/api/academic/learning-groups/{id}/roster",
    operation_id = "applyLearningGroupRoster",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    request_body = ApplyRosterRequest,
    responses(
        (status = 200, description = "Learning group roster applied", body = ApiResponse<LearningGroup>),
        (status = 400, description = "Invalid learning group roster", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse),
        (status = 409, description = "Learning group roster source conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/learning-groups/{id}/roster",
    operation_id = "previewLearningGroupRoster",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    responses(
        (status = 200, description = "Learning group roster preview", body = ApiResponse<RosterPreview>),
        (status = 400, description = "Learning group roster cannot be previewed", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/learning-groups/{id}/roster/publish",
    operation_id = "publishLearningGroupRoster",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    request_body = PublishRosterRequest,
    responses(
        (status = 200, description = "Learning group roster published", body = ApiResponse<LearningGroup>),
        (status = 400, description = "Learning group roster cannot be published", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse),
        (status = 409, description = "Learning group roster publish conflict", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/term-change-sets",
    operation_id = "listAcademicTermChangeSets",
    tag = "academic",
    params(AcademicTermChangeSetQuery),
    responses(
        (status = 200, description = "Operational academic changes for the selected term", body = ApiResponse<Vec<AcademicTermChangeSet>>),
        (status = 400, description = "Invalid academic term query", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found", body = ApiErrorResponse),
        (status = 409, description = "Academic term state conflict", body = ApiErrorResponse)
    )
)]
pub async fn list_term_change_sets(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicTermChangeSetQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    Ok(ok(change_sets::list_change_sets(
        &context.tenant.pool,
        query.academic_term_id,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/term-change-sets",
    operation_id = "createAcademicTermChangeSet",
    tag = "academic",
    request_body = CreateAcademicTermChangeSetRequest,
    responses(
        (status = 201, description = "Operational academic change draft created", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Invalid operational change", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term or timetable version not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change creation conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_term_change_set(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateAcademicTermChangeSetRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    let change_set =
        change_sets::create_change_set(&context.tenant.pool, context.actor.user_id, request)
            .await?;
    signal_term_change_set_changed(&state, &session, &context, &change_set, &[]).await?;
    Ok(created(change_set))
}

#[utoipa::path(
    get,
    path = "/api/academic/term-change-sets/{id}",
    operation_id = "getAcademicTermChangeSet",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    responses(
        (status = 200, description = "Operational academic change", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Invalid operational change ID", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change state conflict", body = ApiErrorResponse)
    )
)]
pub async fn get_term_change_set(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_term_change_set_access(&context, id, OfferingAction::Read).await?;
    Ok(ok(
        change_sets::get_change_set(&context.tenant.pool, id).await?
    ))
}

#[utoipa::path(
    patch,
    path = "/api/academic/term-change-sets/{id}",
    operation_id = "updateAcademicTermChangeSet",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    request_body = UpdateAcademicTermChangeSetRequest,
    responses(
        (status = 200, description = "Operational academic change updated", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Invalid operational change update", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_term_change_set(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateAcademicTermChangeSetRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let offering_ids = require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    let prior_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &offering_ids).await?;
    let change_set =
        change_sets::update_change_set(&context.tenant.pool, context.actor.user_id, id, request)
            .await?;
    signal_term_change_set_changed(&state, &session, &context, &change_set, &prior_descriptors)
        .await?;
    Ok(ok(change_set))
}

#[utoipa::path(
    post,
    path = "/api/academic/term-change-sets/{id}/cancel",
    operation_id = "cancelAcademicTermChangeSet",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    request_body = CancelAcademicTermChangeSetRequest,
    responses(
        (status = 200, description = "Operational academic change cancelled", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Invalid cancellation request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change cancellation conflict", body = ApiErrorResponse)
    )
)]
pub async fn cancel_term_change_set(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<CancelAcademicTermChangeSetRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let offering_ids = require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    let prior_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &offering_ids).await?;
    let change_set =
        change_sets::cancel_change_set(&context.tenant.pool, context.actor.user_id, id, request)
            .await?;
    signal_term_change_set_changed(&state, &session, &context, &change_set, &prior_descriptors)
        .await?;
    Ok(ok(change_set))
}

#[utoipa::path(
    put,
    path = "/api/academic/term-change-sets/{id}/items",
    operation_id = "upsertAcademicTermChangeItem",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    request_body = UpsertAcademicTermChangeItemRequest,
    responses(
        (status = 200, description = "Operational change item upserted", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Invalid operational change item", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change or offering not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change item conflict", body = ApiErrorResponse)
    )
)]
pub async fn upsert_term_change_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpsertAcademicTermChangeItemRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let offering_ids = require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    let prior_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &offering_ids).await?;
    let manage_filter = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Manage,
    )
    .await?;
    if let Some(owner_id) = request.owning_organization_unit_id() {
        if !learning_offering_access_policy::learning_offering_owner_allowed(
            &manage_filter,
            owner_id,
        ) {
            return Err(AppError::Forbidden(
                "ไม่มีสิทธิ์เพิ่มรายการเปิดสอนให้หน่วยงานนี้".to_string(),
            ));
        }
    }
    if let Some(offering_id) = request.existing_learning_offering_id() {
        learning_offering_access_policy::require_learning_offering_batch_access(
            &context.tenant.pool,
            &context.actor,
            &[offering_id],
            OfferingAction::Manage,
        )
        .await?;
    }
    if let Some(group_id) = request.existing_learning_group_id() {
        learning_offering_access_policy::require_learning_group_access(
            &context.tenant.pool,
            &context.actor,
            group_id,
            OfferingAction::Manage,
        )
        .await?;
    }
    let change_set =
        change_sets::upsert_change_item(&context.tenant.pool, context.actor.user_id, id, request)
            .await?;
    signal_term_change_set_changed(&state, &session, &context, &change_set, &prior_descriptors)
        .await?;
    Ok(ok(change_set))
}

#[utoipa::path(
    delete,
    path = "/api/academic/term-change-sets/{id}/items/{itemId}",
    operation_id = "deleteAcademicTermChangeItem",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Operational change set ID"),
        ("itemId" = Uuid, Path, description = "Operational change item ID")
    ),
    request_body = DeleteAcademicTermChangeItemRequest,
    responses(
        (status = 200, description = "Operational change item deleted", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Invalid delete request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change item not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change item row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn delete_term_change_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<DeleteAcademicTermChangeItemRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let offering_ids = require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    let prior_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &offering_ids).await?;
    let change_set = change_sets::delete_change_item(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        item_id,
        request,
    )
    .await?;
    signal_term_change_set_changed(&state, &session, &context, &change_set, &prior_descriptors)
        .await?;
    Ok(ok(change_set))
}

#[utoipa::path(
    post,
    path = "/api/academic/term-change-sets/{id}/teacher-handoff/preview",
    operation_id = "previewTeacherHandoff",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    request_body = PreviewTeacherHandoffRequest,
    responses(
        (status = 200, description = "Teacher timetable handoff preview", body = ApiResponse<TeacherHandoffPreview>),
        (status = 400, description = "Invalid teacher handoff", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Teacher change item not found", body = ApiErrorResponse),
        (status = 409, description = "Teacher handoff conflict", body = ApiErrorResponse)
    )
)]
pub async fn preview_teacher_handoff(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PreviewTeacherHandoffRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    Ok(ok(teacher_handoff::preview(
        &context.tenant.pool,
        id,
        request,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/term-change-sets/{id}/teacher-handoff/apply",
    operation_id = "applyTeacherHandoff",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    request_body = ApplyTeacherHandoffRequest,
    responses(
        (status = 200, description = "Teacher timetable handoff applied", body = ApiResponse<ApplyTeacherHandoffResponse>),
        (status = 400, description = "Invalid teacher handoff", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Teacher change item not found", body = ApiErrorResponse),
        (status = 409, description = "Teacher handoff conflict or stale preview", body = ApiErrorResponse)
    )
)]
pub async fn apply_teacher_handoff(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ApplyTeacherHandoffRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    let outcome =
        teacher_handoff::apply(&context.tenant.pool, context.actor.user_id, id, request).await?;
    for entry in &outcome.response.handoff.proposed_entries {
        state.websocket_manager.broadcast_mutation(
            session.tenant.subdomain.clone(),
            outcome.academic_term_id,
            crate::modules::academic::websockets::TimetableEvent::TimetableChanged {
                user_id: context.actor.user_id,
                academic_term_id: outcome.academic_term_id,
                timetable_version_id: outcome.response.handoff.target_timetable_version_id,
                block_id: None,
                revision: entry.row_version + 1,
            },
        );
    }
    Ok(ok(outcome.response))
}

#[utoipa::path(
    get,
    path = "/api/academic/term-change-sets/{id}/preview",
    operation_id = "previewAcademicTermChangeSet",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    responses(
        (status = 200, description = "Operational change readiness preview", body = ApiResponse<AcademicTermChangeSetPreview>),
        (status = 400, description = "Invalid preview request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change not found", body = ApiErrorResponse),
        (status = 409, description = "Operational change preview conflict", body = ApiErrorResponse)
    )
)]
pub async fn preview_term_change_set(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_term_change_set_access(&context, id, OfferingAction::Read).await?;
    Ok(ok(change_sets::preview_change_set(
        &context.tenant.pool,
        id,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/term-change-sets/{id}/publish",
    operation_id = "publishAcademicTermChangeSet",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Operational change set ID")),
    request_body = PublishAcademicTermChangeSetRequest,
    responses(
        (status = 200, description = "Operational academic change published", body = ApiResponse<AcademicTermChangeSet>),
        (status = 400, description = "Readiness blockers remain", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning offering management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Operational change not found", body = ApiErrorResponse),
        (status = 409, description = "Preview or publication conflict", body = ApiErrorResponse)
    )
)]
pub async fn publish_term_change_set(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishAcademicTermChangeSetRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let offering_ids = require_term_change_set_access(&context, id, OfferingAction::Manage).await?;
    let prior_descriptors =
        offerings::signal_descriptors(&context.tenant.pool, &offering_ids).await?;
    let change_set =
        change_sets::publish_change_set(&context.tenant.pool, context.actor.user_id, id, request)
            .await?;
    signal_term_change_set_changed(&state, &session, &context, &change_set, &prior_descriptors)
        .await?;
    Ok(ok(change_set))
}

#[utoipa::path(
    get,
    path = "/api/academic/learning-groups/{id}/memberships",
    operation_id = "listDatedRosterMemberships",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    responses(
        (status = 200, description = "Dated roster membership history", body = ApiResponse<Vec<DatedRosterMembership>>),
        (status = 400, description = "Invalid learning group ID", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group not found", body = ApiErrorResponse),
        (status = 409, description = "Roster state conflict", body = ApiErrorResponse)
    )
)]
pub async fn list_group_memberships(
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
    Ok(ok(roster_memberships::list_memberships(
        &context.tenant.pool,
        id,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/learning-groups/{id}/memberships",
    operation_id = "addDatedRosterMembership",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Learning group ID")),
    request_body = AddDatedRosterMembershipRequest,
    responses(
        (status = 201, description = "Dated roster membership added", body = ApiResponse<DatedRosterMembership>),
        (status = 400, description = "Invalid membership interval", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group or student year not found", body = ApiErrorResponse),
        (status = 409, description = "Membership overlap or row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn add_group_membership(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<AddDatedRosterMembershipRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let membership = roster_memberships::add_membership(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        request,
    )
    .await?;
    let group = groups::get(&context.tenant.pool, id).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(created(membership))
}

#[utoipa::path(
    post,
    path = "/api/academic/learning-groups/{id}/memberships/{membershipId}/end",
    operation_id = "endDatedRosterMembership",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Learning group ID"),
        ("membershipId" = Uuid, Path, description = "Dated roster membership ID")
    ),
    request_body = RemoveDatedRosterMembershipRequest,
    responses(
        (status = 200, description = "Dated roster membership ended", body = ApiResponse<DatedRosterMembership>),
        (status = 400, description = "Invalid inclusive end date", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Learning group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Learning group or membership not found", body = ApiErrorResponse),
        (status = 409, description = "Membership row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn end_group_membership(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((id, membership_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<RemoveDatedRosterMembershipRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    learning_offering_access_policy::require_learning_group_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let membership = roster_memberships::remove_membership(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        membership_id,
        request,
    )
    .await?;
    let group = groups::get(&context.tenant.pool, id).await?;
    signal_group_changed(&state, &session, &context.actor, &group);
    Ok(ok(membership))
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
