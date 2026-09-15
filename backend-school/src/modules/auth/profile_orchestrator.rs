use school_auth::{models::UpdateProfileRequest, models::User, services};
use school_errors::AppError;
use school_file_platform::{
    platform_service::FilePlatform,
    profile_relationships::{promote_ready_owned_image, ProfileRelationshipError},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::files::consumer_service::request_deletions;

/// Composes the auth-owned profile mutation with the File Platform-owned
/// relationship update in one transaction, then requests lifecycle cleanup.
pub async fn update_profile(
    file_platform: &FilePlatform,
    pool: &PgPool,
    user_id: Uuid,
    payload: UpdateProfileRequest,
) -> Result<User, AppError> {
    let mut transaction = pool.begin().await?;
    if let Some(Some(file_id)) = payload.profile_image_file_id {
        promote_ready_owned_image(&mut transaction, file_id, user_id)
            .await
            .map_err(map_profile_relationship_error)?;
    }
    let result = services::update_profile(&mut transaction, user_id, payload).await?;
    transaction.commit().await?;

    let user = services::find_user_by_id(pool, user_id).await?;
    if let Some(file_id) = result.replaced_file_id {
        request_deletions(file_platform, pool, [file_id]).await?;
    }
    Ok(user)
}

fn map_profile_relationship_error(error: ProfileRelationshipError) -> AppError {
    match error {
        ProfileRelationshipError::InvalidFile => {
            AppError::ValidationError("ไฟล์รูปโปรไฟล์ไม่พร้อมใช้งาน".to_string())
        }
        ProfileRelationshipError::Database(error) => AppError::DbError(error),
    }
}
