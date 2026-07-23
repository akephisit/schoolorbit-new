use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarCategory, CalendarTag, UpsertCalendarCategoryRequest, UpsertCalendarTagRequest,
};

const DUPLICATE_CATEGORY_MESSAGE: &str = "มีหมวดหมู่นี้อยู่แล้ว";
const DUPLICATE_TAG_MESSAGE: &str = "มีแท็กนี้อยู่แล้ว";
const CATEGORY_NOT_FOUND_MESSAGE: &str = "ไม่พบหมวดหมู่";
const TAG_NOT_FOUND_MESSAGE: &str = "ไม่พบแท็ก";

pub async fn list_categories(pool: &PgPool) -> Result<Vec<CalendarCategory>, AppError> {
    sqlx::query_as::<_, CalendarCategory>(
        r#"
        SELECT id, name, color, order_index, is_active, created_at, updated_at
        FROM calendar_categories
        WHERE is_active = true
        ORDER BY order_index, name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create_category(
    pool: &PgPool,
    payload: UpsertCalendarCategoryRequest,
) -> Result<CalendarCategory, AppError> {
    sqlx::query_as::<_, CalendarCategory>(
        r#"
        INSERT INTO calendar_categories (name, color, order_index, is_active)
        VALUES (
            $1,
            $2,
            COALESCE($3, (SELECT COALESCE(MAX(order_index), 0) + 1 FROM calendar_categories)),
            COALESCE($4, true)
        )
        RETURNING id, name, color, order_index, is_active, created_at, updated_at
        "#,
    )
    .bind(payload.name)
    .bind(payload.color)
    .bind(payload.order_index)
    .bind(payload.is_active)
    .fetch_one(pool)
    .await
    .map_err(map_category_write_error)
}

pub async fn update_category(
    pool: &PgPool,
    id: Uuid,
    payload: UpsertCalendarCategoryRequest,
) -> Result<CalendarCategory, AppError> {
    let category = sqlx::query_as::<_, CalendarCategory>(
        r#"
        UPDATE calendar_categories
        SET
            name = $1,
            color = $2,
            order_index = COALESCE($3, order_index),
            is_active = COALESCE($4, is_active)
        WHERE id = $5 AND is_active = true
        RETURNING id, name, color, order_index, is_active, created_at, updated_at
        "#,
    )
    .bind(payload.name)
    .bind(payload.color)
    .bind(payload.order_index)
    .bind(payload.is_active)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(map_category_write_error)?;

    category.ok_or_else(|| AppError::NotFound(CATEGORY_NOT_FOUND_MESSAGE.to_string()))
}

pub async fn hard_delete_category(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM calendar_categories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(CATEGORY_NOT_FOUND_MESSAGE.to_string()));
    }

    Ok(())
}

pub async fn list_tags(pool: &PgPool) -> Result<Vec<CalendarTag>, AppError> {
    sqlx::query_as::<_, CalendarTag>(
        r#"
        SELECT id, name, created_at, updated_at
        FROM calendar_tags
        ORDER BY LOWER(name), id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create_tag(
    pool: &PgPool,
    payload: UpsertCalendarTagRequest,
) -> Result<CalendarTag, AppError> {
    let name = normalized_tag_name(payload.name)?;
    sqlx::query_as::<_, CalendarTag>(
        r#"
        INSERT INTO calendar_tags (name)
        VALUES ($1)
        RETURNING id, name, created_at, updated_at
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(map_tag_write_error)
}

pub async fn update_tag(
    pool: &PgPool,
    id: Uuid,
    payload: UpsertCalendarTagRequest,
) -> Result<CalendarTag, AppError> {
    let name = normalized_tag_name(payload.name)?;
    let tag = sqlx::query_as::<_, CalendarTag>(
        r#"
        UPDATE calendar_tags
        SET name = $1
        WHERE id = $2
        RETURNING id, name, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(map_tag_write_error)?;

    tag.ok_or_else(|| AppError::NotFound(TAG_NOT_FOUND_MESSAGE.to_string()))
}

pub async fn hard_delete_tag(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM calendar_tags WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(TAG_NOT_FOUND_MESSAGE.to_string()));
    }

    Ok(())
}

fn map_category_write_error(error: sqlx::Error) -> AppError {
    if is_duplicate_active_category_name(&error) {
        AppError::BadRequest(DUPLICATE_CATEGORY_MESSAGE.to_string())
    } else {
        AppError::from(error)
    }
}

pub(super) fn normalized_tag_name(name: String) -> Result<String, AppError> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::BadRequest("กรุณาระบุชื่อแท็ก".to_string()));
    }
    if normalized.chars().count() > 80 {
        return Err(AppError::BadRequest("ชื่อแท็กต้องไม่เกิน 80 ตัวอักษร".to_string()));
    }

    Ok(normalized.to_string())
}

fn map_tag_write_error(error: sqlx::Error) -> AppError {
    if is_unique_violation_for_constraint(&error, "idx_calendar_tags_name_unique") {
        AppError::BadRequest(DUPLICATE_TAG_MESSAGE.to_string())
    } else {
        AppError::from(error)
    }
}

fn is_duplicate_active_category_name(error: &sqlx::Error) -> bool {
    is_unique_violation_for_constraint(error, "idx_calendar_categories_active_name_unique")
}

fn is_unique_violation_for_constraint(error: &sqlx::Error, constraint_name: &str) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };

    if database_error.code().as_deref() != Some("23505") {
        return false;
    }

    let constraint_matches = database_error
        .constraint()
        .map(|constraint| constraint == constraint_name)
        .unwrap_or(false);
    let message_matches = database_error.message().contains(constraint_name);

    constraint_matches || message_matches
}
