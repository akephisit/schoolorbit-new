use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData};
use crate::error::AppError;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::modules::files::{
    consumer_service::{map_platform_error, request_deletions},
    platform_types::DownloadGrant,
    repository::SqlFileRepository,
};
use crate::modules::question_bank::models::{
    QuestionBankExportDataRequest, QuestionBankListQuery, QuestionBankOptions, QuestionBankPage,
    QuestionDetail, UpsertQuestionRequest,
};
use crate::modules::question_bank::services as question_bank_service;
use crate::policies::{file_access_policy, question_bank_access_policy};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/academic/question-bank/questions",
    operation_id = "listQuestionBankQuestions",
    tag = "question-bank",
    params(QuestionBankListQuery),
    responses(
        (status = 200, description = "Question bank page", body = ApiResponse<QuestionBankPage>),
        (status = 400, description = "Invalid question bank filters", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_questions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<QuestionBankListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access =
        question_bank_access_policy::resolve_access(&context.tenant.pool, &context.actor).await?;
    let questions =
        question_bank_service::list_questions(&context.tenant.pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(questions)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/question-bank/options",
    operation_id = "listQuestionBankOptions",
    tag = "question-bank",
    responses(
        (status = 200, description = "Question bank options", body = ApiResponse<QuestionBankOptions>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access =
        question_bank_access_policy::resolve_access(&context.tenant.pool, &context.actor).await?;
    let options = question_bank_service::list_options(&context.tenant.pool, &access).await?;
    Ok(Json(ApiResponse::ok(options)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/question-bank/questions/{id}",
    operation_id = "getQuestionBankQuestion",
    tag = "question-bank",
    params(("id" = Uuid, Path, description = "Question ID")),
    responses(
        (status = 200, description = "Question detail", body = ApiResponse<QuestionDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank access denied", body = ApiErrorResponse),
        (status = 404, description = "Question not found", body = ApiErrorResponse)
    )
)]
pub async fn get_question(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let question =
        question_bank_service::get_question(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(question)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/question-bank/questions/{question_id}/files/{file_id}",
    operation_id = "getQuestionBankQuestionFile",
    tag = "question-bank",
    params(
        ("question_id" = Uuid, Path, description = "Question ID"),
        ("file_id" = Uuid, Path, description = "Question image file ID")
    ),
    responses(
        (status = 302, description = "Redirect to the authorized private file"),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question image access denied", body = ApiErrorResponse),
        (status = 404, description = "Question image not found", body = ApiErrorResponse)
    )
)]
pub async fn get_question_file(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((question_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let repository = SqlFileRepository::new(context.tenant.pool);
    let file = state
        .file_platform
        .metadata(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    file_access_policy::authorize_existing(
        repository.pool(),
        &context.actor,
        &file,
        file_access_policy::FilePolicyAction::Read,
        Some(question_id),
    )
    .await?;
    match state
        .file_platform
        .private_download(&repository, file_id)
        .await
        .map_err(map_platform_error)?
    {
        DownloadGrant::Redirect { location, .. } => Ok(Redirect::to(&location).into_response()),
        DownloadGrant::Stream { .. } => Err(AppError::InternalServerError(
            "file_stream_grant_not_supported".to_string(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/api/academic/question-bank/questions",
    operation_id = "createQuestionBankQuestion",
    tag = "question-bank",
    request_body = UpsertQuestionRequest,
    responses(
        (status = 201, description = "Question created", body = ApiResponse<QuestionDetail>),
        (status = 400, description = "Invalid question", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank management denied", body = ApiErrorResponse)
    )
)]
pub async fn create_question(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<UpsertQuestionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let question = question_bank_service::create_question(
        &context.tenant.pool,
        &context.actor,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(question))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/question-bank/questions/{id}",
    operation_id = "updateQuestionBankQuestion",
    tag = "question-bank",
    params(("id" = Uuid, Path, description = "Question ID")),
    request_body = UpsertQuestionRequest,
    responses(
        (status = 200, description = "Question updated", body = ApiResponse<QuestionDetail>),
        (status = 400, description = "Invalid question", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank management denied", body = ApiErrorResponse),
        (status = 404, description = "Question not found", body = ApiErrorResponse)
    )
)]
pub async fn update_question(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpsertQuestionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = question_bank_service::update_question(
        &context.tenant.pool,
        &context.actor,
        id,
        context.actor.user_id,
        payload,
    )
    .await?;
    request_deletions(
        state.file_platform.as_ref(),
        &context.tenant.pool,
        result.detached_file_ids,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result.question)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/question-bank/questions/{id}",
    operation_id = "deleteQuestionBankQuestion",
    tag = "question-bank",
    params(("id" = Uuid, Path, description = "Question ID")),
    responses(
        (status = 200, description = "Question deleted", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank management denied", body = ApiErrorResponse),
        (status = 404, description = "Question not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_question(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let file_ids =
        question_bank_service::delete_question(&context.tenant.pool, &context.actor, id).await?;
    request_deletions(state.file_platform.as_ref(), &context.tenant.pool, file_ids).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/question-bank/questions/export-data",
    operation_id = "exportQuestionBankData",
    tag = "question-bank",
    request_body = QuestionBankExportDataRequest,
    responses(
        (status = 200, description = "Ordered question export data", body = ApiResponse<Vec<QuestionDetail>>),
        (status = 400, description = "Invalid export selection", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Question bank access denied", body = ApiErrorResponse),
        (status = 404, description = "Question selection unavailable", body = ApiErrorResponse)
    )
)]
pub async fn export_question_data(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<QuestionBankExportDataRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let questions = question_bank_service::export_question_data(
        &context.tenant.pool,
        &context.actor,
        &payload.question_ids,
    )
    .await?;
    Ok(Json(ApiResponse::ok(questions)).into_response())
}
