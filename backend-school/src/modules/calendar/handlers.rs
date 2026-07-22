use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarEventQuery, UpsertCalendarCategoryRequest, UpsertCalendarEventRequest,
    UpsertCalendarTagRequest,
};
use crate::modules::calendar::services as calendar_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{
    actor_tenant_context, current_user_tenant_context_from_headers, tenant_context,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

pub async fn list_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_READ_SCHOOL)?;
    let events = calendar_service::list_management_events(&context.tenant.pool, query).await?;
    Ok(Json(ApiResponse::ok(events)))
}

pub async fn create_calendar_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertCalendarEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    let outcome =
        calendar_service::create_event(&context.tenant.pool, context.actor.user_id, payload)
            .await?;
    let event = outcome.event;

    if outcome.notify_audience {
        if let Err(error) = calendar_service::send_event_notification(
            &context.tenant.pool,
            &state.notification_channel,
            &context.tenant.subdomain,
            &event,
            outcome.notification_kind,
        )
        .await
        {
            tracing::error!(
                event_id = %event.id,
                error = %error,
                "Calendar event notification failed after create"
            );
        }
    }

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(event))))
}

pub async fn update_calendar_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpsertCalendarEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    let outcome =
        calendar_service::update_event(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let event = outcome.event;

    if outcome.notify_audience {
        if let Err(error) = calendar_service::send_event_notification(
            &context.tenant.pool,
            &state.notification_channel,
            &context.tenant.subdomain,
            &event,
            outcome.notification_kind,
        )
        .await
        {
            tracing::error!(
                event_id = %event.id,
                error = %error,
                "Calendar event notification failed after update"
            );
        }
    }

    Ok(Json(ApiResponse::ok(event)))
}

pub async fn delete_calendar_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    calendar_service::soft_delete_event(&context.tenant.pool, id, context.actor.user_id).await?;
    Ok(Json(ApiResponse::empty()))
}

pub async fn list_calendar_categories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_READ_SCHOOL)?;
    let categories = calendar_service::list_categories(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(categories)))
}

pub async fn create_calendar_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertCalendarCategoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    let category = calendar_service::create_category(&context.tenant.pool, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(category))))
}

pub async fn update_calendar_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpsertCalendarCategoryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    let category = calendar_service::update_category(&context.tenant.pool, id, payload).await?;
    Ok(Json(ApiResponse::ok(category)))
}

pub async fn delete_calendar_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    calendar_service::hard_delete_category(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::empty()))
}

pub async fn list_calendar_tags(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_READ_SCHOOL)?;
    let tags = calendar_service::list_tags(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(tags)))
}

pub async fn create_calendar_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertCalendarTagRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    let tag = calendar_service::create_tag(&context.tenant.pool, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(tag))))
}

pub async fn update_calendar_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpsertCalendarTagRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    let tag = calendar_service::update_tag(&context.tenant.pool, id, payload).await?;
    Ok(Json(ApiResponse::ok(tag)))
}

pub async fn delete_calendar_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::CALENDAR_MANAGE_SCHOOL)?;
    calendar_service::hard_delete_tag(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::empty()))
}

#[utoipa::path(
    get,
    path = "/api/me/calendar/events",
    operation_id = "listMyCalendarEvents",
    tag = "calendar",
    params(
        ("from" = Option<chrono::NaiveDate>, Query, description = "Inclusive range start"),
        ("to" = Option<chrono::NaiveDate>, Query, description = "Inclusive range end"),
        ("category_id" = Option<Uuid>, Query, description = "Calendar category ID"),
        ("tag_id" = Option<Uuid>, Query, description = "Calendar tag ID"),
        ("audience" = Option<String>, Query, description = "Audience: all, staff, student, or parent"),
        ("visibility" = Option<String>, Query, description = "Visibility: public or private"),
        ("q" = Option<String>, Query, description = "Title or description search")
    ),
    responses(
        (status = 200, description = "Calendar events visible to the current student or staff member", body = ApiResponse<Vec<crate::modules::calendar::models::CalendarViewerEvent>>),
        (status = 400, description = "Invalid date range", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student or staff account required", body = ApiErrorResponse)
    )
)]
pub async fn list_my_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let events =
        calendar_service::list_my_events(&context.tenant.pool, context.user_id, query).await?;
    Ok(Json(ApiResponse::ok(events)))
}

pub async fn list_public_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = tenant_context(&state, &headers).await?;
    let events = calendar_service::list_public_events(&context.pool, query).await?;
    Ok(Json(ApiResponse::ok(events)))
}
