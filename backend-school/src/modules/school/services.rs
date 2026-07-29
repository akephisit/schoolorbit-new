use sqlx::PgPool;
use uuid::Uuid;

use super::models::{SchoolSettingsResponse, SchoolSettingsRow};
use crate::error::AppError;

fn empty_settings_row() -> SchoolSettingsRow {
    SchoolSettingsRow { logo_file_id: None }
}

fn settings_response_from_row(row: SchoolSettingsRow) -> SchoolSettingsResponse {
    SchoolSettingsResponse {
        logo_file_id: row.logo_file_id,
    }
}

pub async fn get_settings_row(pool: &PgPool) -> Result<SchoolSettingsRow, AppError> {
    sqlx::query_as::<_, SchoolSettingsRow>("SELECT logo_file_id FROM school_settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to fetch school settings: {}", error);
            AppError::InternalServerError("Database error".to_string())
        })
        .map(|row| row.unwrap_or_else(empty_settings_row))
}

pub async fn get_settings_response(pool: &PgPool) -> Result<SchoolSettingsResponse, AppError> {
    Ok(settings_response_from_row(get_settings_row(pool).await?))
}

/// Atomically validates and attaches a ready school-logo file, returning the
/// previously attached file only when it should be deleted after commit.
pub async fn replace_logo(
    pool: &PgPool,
    new_file_id: Option<Uuid>,
) -> Result<Option<Uuid>, AppError> {
    let mut transaction = pool.begin().await.map_err(database_error)?;

    if let Some(file_id) = new_file_id {
        let valid = sqlx::query_scalar::<_, bool>(
            r#"
SELECT EXISTS(
    SELECT 1
    FROM files
    WHERE id = $1
      AND purpose_code = 'school_logo'
      AND visibility = 'public'
      AND lifecycle_status = 'ready'
      AND deleted_at IS NULL
)
"#,
        )
        .bind(file_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error)?;
        if !valid {
            return Err(AppError::ValidationError(
                "ไฟล์โลโก้ไม่พร้อมใช้งานหรือไม่ใช่โลโก้โรงเรียน".to_string(),
            ));
        }
        sqlx::query(
            "UPDATE files SET retention_class = 'standard', expires_at = NULL, updated_at = NOW() WHERE id = $1",
        )
        .bind(file_id)
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
    }

    let old_file_id = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT logo_file_id FROM school_settings LIMIT 1 FOR UPDATE",
    )
    .fetch_optional(&mut *transaction)
    .await
    .map_err(database_error)?
    .flatten();

    let updated = sqlx::query(
        "UPDATE school_settings SET logo_path = NULL, logo_file_id = $1, updated_at = NOW()",
    )
    .bind(new_file_id)
    .execute(&mut *transaction)
    .await
    .map_err(database_error)?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบการตั้งค่าโรงเรียน".to_string()));
    }

    transaction.commit().await.map_err(database_error)?;
    Ok((old_file_id != new_file_id)
        .then_some(old_file_id)
        .flatten())
}

pub async fn detach_logo(pool: &PgPool) -> Result<Option<Uuid>, AppError> {
    replace_logo(pool, None).await
}

fn database_error(error: sqlx::Error) -> AppError {
    tracing::error!("Failed to update school logo relationship: {}", error);
    AppError::InternalServerError("Database error".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_response_exposes_file_identity_only() {
        let logo_file_id = Uuid::new_v4();
        let response = settings_response_from_row(SchoolSettingsRow {
            logo_file_id: Some(logo_file_id),
        });

        assert_eq!(response.logo_file_id, Some(logo_file_id));
    }

    #[test]
    fn empty_settings_row_has_no_logo_file() {
        assert!(empty_settings_row().logo_file_id.is_none());
    }
}
