use crate::{
    api_response::{ApiErrorResponse, ApiResponse},
    error::AppError,
    modules::{
        academic::{
            core::{
                models::{YearReopeningOutcome, YearReopeningRequest},
                services::year_reopening,
            },
            lifecycle::{models::YearReopeningWorkspace, services},
        },
        auth::session_service::AuthenticatedSession,
    },
    utils::request_context::actor_tenant_context_from_session,
    AppState,
};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use uuid::Uuid;

#[utoipa::path(get,path="/api/academic/lifecycle/years/{year_id}/reopening",operation_id="getYearReopeningWorkspace",tag="academic",
    params(("year_id"=Uuid,Path)),
    responses((status=200,body=ApiResponse<YearReopeningWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn get_year_reopening_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(year): Path<Uuid>,
) -> Result<Json<ApiResponse<YearReopeningWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_year_reopening_workspace(&context.tenant.pool, &context.actor, year).await?,
    )))
}

#[utoipa::path(post,path="/api/academic/lifecycle/years/{year_id}/reopening",operation_id="reopenAcademicYear",tag="academic",
    params(("year_id"=Uuid,Path)),request_body=YearReopeningRequest,
    responses((status=200,body=ApiResponse<YearReopeningOutcome>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn reopen_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(year): Path<Uuid>,
    Json(input): Json<YearReopeningRequest>,
) -> Result<Json<ApiResponse<YearReopeningOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        year_reopening::reopen_year(&context.tenant.pool, &context.actor, year, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_year",
        Some(year),
        Some(year),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}
