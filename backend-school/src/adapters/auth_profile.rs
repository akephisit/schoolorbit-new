use async_trait::async_trait;
use school_auth::{
    models::{UpdateProfileRequest, User},
    services,
};
use school_auth_http::http::state::ProfileUpdatePort;
use school_errors::AppError;
use school_file_platform::{
    consumer_service::request_deletions,
    platform_service::FilePlatform,
    profile_relationships::{promote_ready_owned_image, ProfileRelationshipError},
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub(crate) struct ApplicationAuthProfile {
    file_platform: Arc<FilePlatform>,
}

impl ApplicationAuthProfile {
    pub(crate) fn new(file_platform: Arc<FilePlatform>) -> Self {
        Self { file_platform }
    }
}

/// Composes the auth-owned profile mutation with the File Platform-owned
/// relationship update in one transaction, then requests lifecycle cleanup.
#[async_trait]
impl ProfileUpdatePort for ApplicationAuthProfile {
    async fn update_profile(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        payload: UpdateProfileRequest,
    ) -> Result<User, AppError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &pool,
            "auth-http.profile_orchestrator.update_profile",
        )
        .await?;
        if let Some(Some(file_id)) = payload.profile_image_file_id {
            promote_ready_owned_image(&mut transaction, file_id, user_id)
                .await
                .map_err(map_profile_relationship_error)?;
        }
        let result = services::update_profile(&mut transaction, user_id, payload).await?;
        transaction.commit().await?;

        let user = services::find_user_by_id(pool, user_id).await?;
        if let Some(file_id) = result.replaced_file_id {
            request_deletions(self.file_platform.as_ref(), pool, [file_id]).await?;
        }
        Ok(user)
    }
}

fn map_profile_relationship_error(error: ProfileRelationshipError) -> AppError {
    match error {
        ProfileRelationshipError::InvalidFile => {
            AppError::ValidationError("ไฟล์รูปโปรไฟล์ไม่พร้อมใช้งาน".to_string())
        }
        ProfileRelationshipError::Database(error) => AppError::DbError(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_profile_image_preserves_the_public_validation_error() {
        assert!(matches!(
            map_profile_relationship_error(ProfileRelationshipError::InvalidFile),
            AppError::ValidationError(message) if message == "ไฟล์รูปโปรไฟล์ไม่พร้อมใช้งาน"
        ));
    }

    #[test]
    fn profile_relationship_database_error_preserves_the_original_error() {
        assert!(matches!(
            map_profile_relationship_error(ProfileRelationshipError::Database(
                sqlx::Error::RowNotFound
            )),
            AppError::DbError(sqlx::Error::RowNotFound)
        ));
    }
}
