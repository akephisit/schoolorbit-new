use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::{
    api_response::{ApiErrorResponse, ApiErrorResponseWithData, ApiResponse, EmptyData},
    error::AppError,
    modules::{
        auth::session_service::AuthenticatedSession, files::consumer_service::request_deletions,
    },
    utils::request_context::actor_tenant_context_from_session,
    AppState,
};

use super::{
    models::{
        AttachSchoolFontBatchRequest, InspectSchoolFontUploadsRequest, SchoolFontDeleteConflict,
        SchoolFontListResponse, SchoolFontUploadInspection,
    },
    services::{self, SchoolFontDeleteOutcome},
};

#[utoipa::path(
    get,
    path = "/api/school-fonts",
    operation_id = "listSchoolFonts",
    tag = "school-font",
    responses(
        (status = 200, description = "Shared school fonts", body = ApiResponse<SchoolFontListResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School font manager permission required", body = ApiErrorResponse)
    )
)]
pub async fn list_school_fonts(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let fonts = services::list_for_manager(&context.tenant.pool, &context.actor).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(fonts))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/school-fonts/inspect",
    operation_id = "inspectSchoolFontUploads",
    tag = "school-font",
    request_body = InspectSchoolFontUploadsRequest,
    responses(
        (status = 200, description = "School font uploads inspected in caller order", body = ApiResponse<SchoolFontUploadInspection>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School font manager permission required", body = ApiErrorResponse),
        (status = 422, description = "Invalid or unavailable school font selection", body = ApiErrorResponse)
    )
)]
pub async fn inspect_school_font_uploads(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<InspectSchoolFontUploadsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let inspection =
        services::inspect_for_manager(&context.tenant.pool, &context.actor, payload).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(inspection))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/school-fonts/batch",
    operation_id = "attachSchoolFontBatch",
    tag = "school-font",
    request_body = AttachSchoolFontBatchRequest,
    responses(
        (status = 201, description = "Reviewed static font variants attached atomically", body = ApiResponse<SchoolFontListResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School font manager permission required", body = ApiErrorResponse),
        (status = 409, description = "School font variant conflict", body = ApiErrorResponse),
        (status = 422, description = "Invalid batch or font rights not confirmed", body = ApiErrorResponse)
    )
)]
pub async fn attach_school_font_batch(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<AttachSchoolFontBatchRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let fonts = services::attach_for_manager(&context.tenant.pool, &context.actor, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(fonts))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/school-fonts/{font_id}",
    operation_id = "deleteSchoolFont",
    tag = "school-font",
    params(("font_id" = Uuid, Path, description = "Shared school font ID")),
    responses(
        (status = 200, description = "Unreferenced school font deleted", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School font manager permission required", body = ApiErrorResponse),
        (status = 404, description = "School font not found", body = ApiErrorResponse),
        (status = 409, description = "School font is still referenced", body = ApiErrorResponseWithData<SchoolFontDeleteConflict>)
    )
)]
pub async fn delete_school_font(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(font_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    match services::delete(&context.tenant.pool, &context.actor, font_id).await? {
        SchoolFontDeleteOutcome::Deleted { file_id } => {
            request_deletions(
                state.file_platform.as_ref(),
                &context.tenant.pool,
                [file_id],
            )
            .await?;
            Ok((StatusCode::OK, Json(ApiResponse::empty())).into_response())
        }
        SchoolFontDeleteOutcome::Conflict(conflict) => Ok((
            StatusCode::CONFLICT,
            Json(ApiErrorResponseWithData::new(
                "school_font_in_use",
                conflict,
            )),
        )
            .into_response()),
    }
}
