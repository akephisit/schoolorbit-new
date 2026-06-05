use super::models::{SchoolSettingsResponse, SchoolSettingsRow, UpdateSchoolSettingsRequest};
use crate::error::AppError;
use crate::services::r2_client::R2Client;
use crate::utils::file_url::get_file_url_from_string;
use sqlx::PgPool;

fn empty_settings_row() -> SchoolSettingsRow {
    SchoolSettingsRow {
        logo_path: None,
        logo_file_id: None,
    }
}

pub async fn get_settings_row(pool: &PgPool) -> Result<SchoolSettingsRow, AppError> {
    sqlx::query_as::<_, SchoolSettingsRow>(
        "SELECT logo_path, logo_file_id FROM school_settings LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch school settings: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })
    .map(|row| row.unwrap_or_else(empty_settings_row))
}

pub async fn get_settings_response(pool: &PgPool) -> Result<SchoolSettingsResponse, AppError> {
    let row = get_settings_row(pool).await?;

    Ok(SchoolSettingsResponse {
        logo_url: get_file_url_from_string(&row.logo_path),
        logo_file_id: row.logo_file_id,
    })
}

pub async fn update_settings(
    pool: &PgPool,
    payload: UpdateSchoolSettingsRequest,
) -> Result<(), AppError> {
    sqlx::query("UPDATE school_settings SET logo_path = $1, logo_file_id = $2, updated_at = NOW()")
        .bind(&payload.logo_path)
        .bind(payload.logo_file_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update school settings: {}", e);
            AppError::InternalServerError("Database error".to_string())
        })?;

    Ok(())
}

pub async fn delete_logo(pool: &PgPool) -> Result<(), AppError> {
    let row = get_settings_row(pool).await?;

    if let Some(path) = &row.logo_path {
        match R2Client::new().await {
            Ok(r2) => {
                if let Err(error) = r2.delete_file(path).await {
                    tracing::warn!("Failed to delete logo from R2: {}", error);
                }
            }
            Err(error) => tracing::warn!(
                "Failed to initialize R2 client for logo deletion: {}",
                error
            ),
        }
    }

    if let Some(file_id) = row.logo_file_id {
        if let Err(error) = sqlx::query("DELETE FROM files WHERE id = $1")
            .bind(file_id)
            .execute(pool)
            .await
        {
            tracing::warn!("Failed to delete logo file record: {}", error);
        }
    }

    sqlx::query(
        "UPDATE school_settings SET logo_path = NULL, logo_file_id = NULL, updated_at = NOW()",
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to clear school logo settings: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    Ok(())
}
