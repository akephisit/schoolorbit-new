use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug)]
pub enum ProfileRelationshipError {
    InvalidFile,
    Database(sqlx::Error),
}

impl std::fmt::Display for ProfileRelationshipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFile => "profile_image_invalid",
            Self::Database(_) => "profile_image_relationship_database",
        })
    }
}

impl std::error::Error for ProfileRelationshipError {}

impl From<sqlx::Error> for ProfileRelationshipError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// Validates and promotes a ready profile image inside the caller-owned
/// relationship transaction.
pub async fn promote_ready_owned_image(
    connection: &mut PgConnection,
    file_id: Uuid,
    owner_user_id: Uuid,
) -> Result<(), ProfileRelationshipError> {
    let valid = sqlx::query_scalar::<_, bool>(
        r#"
SELECT EXISTS(
    SELECT 1
    FROM files
    WHERE id = $1
      AND owner_user_id = $2
      AND purpose_code = 'profile_image'
      AND lifecycle_status = 'ready'
      AND deleted_at IS NULL
)
"#,
    )
    .bind(file_id)
    .bind(owner_user_id)
    .fetch_one(&mut *connection)
    .await?;
    if !valid {
        return Err(ProfileRelationshipError::InvalidFile);
    }

    sqlx::query(
        "UPDATE files
         SET retention_class = 'standard', expires_at = NULL, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(file_id)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use school_test_db::{create_named_test_pool, create_test_user, run_test_migrations};
    use sqlx::PgPool;

    async fn insert_ready_temporary_profile_image(pool: &PgPool, owner_user_id: Uuid) -> Uuid {
        let file_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let mut transaction = pool.begin().await.expect("fixture should begin");
        sqlx::query(
            r#"
INSERT INTO files (
    id, owner_user_id, display_filename, created_by, purpose_code, visibility,
    lifecycle_status, retention_class, expires_at
) VALUES (
    $1, $2, 'profile.jpg', $2, 'profile_image', 'private', 'processing',
    'temporary', now() + INTERVAL '1 hour'
)
"#,
        )
        .bind(file_id)
        .bind(owner_user_id)
        .execute(&mut *transaction)
        .await
        .expect("file fixture should insert");
        sqlx::query(
            r#"
INSERT INTO file_versions (
    id, file_id, version_number, provider_code, storage_class, storage_status,
    object_key, detected_mime_type, canonical_extension, byte_size, checksum,
    scan_status, scanner_result_code, scanned_at, created_by
) VALUES (
    $1, $2, 1, 'test', 'private', 'stored', $3, 'image/jpeg', 'jpg', 1,
    repeat('a', 64), 'clean', 'clean', now(), $4
)
"#,
        )
        .bind(version_id)
        .bind(file_id)
        .bind(format!("profile-test/{}", Uuid::new_v4()))
        .bind(owner_user_id)
        .execute(&mut *transaction)
        .await
        .expect("version fixture should insert");
        sqlx::query(
            "UPDATE files
             SET current_version_id = $1, lifecycle_status = 'ready'
             WHERE id = $2",
        )
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await
        .expect("file fixture should become ready");
        transaction.commit().await.expect("fixture should commit");
        file_id
    }

    #[tokio::test]
    async fn profile_image_promotion_requires_ready_owned_exact_purpose_file() {
        let pool = create_named_test_pool("profile_relationship_promotion").await;
        run_test_migrations(&pool).await;
        let owner = create_test_user(&pool, "profile-owner@example.test", "test-password")
            .await
            .expect("owner fixture should insert");
        let other = create_test_user(&pool, "profile-other@example.test", "test-password")
            .await
            .expect("other fixture should insert");
        let file_id = insert_ready_temporary_profile_image(&pool, owner).await;

        let mut rejected = pool
            .begin()
            .await
            .expect("rejected transaction should begin");
        let error = promote_ready_owned_image(&mut rejected, file_id, other)
            .await
            .expect_err("another user's file must be rejected");
        assert!(matches!(error, ProfileRelationshipError::InvalidFile));
        rejected
            .rollback()
            .await
            .expect("rejected transaction should roll back");

        let mut accepted = pool
            .begin()
            .await
            .expect("accepted transaction should begin");
        promote_ready_owned_image(&mut accepted, file_id, owner)
            .await
            .expect("owned ready profile image should promote");
        accepted
            .commit()
            .await
            .expect("accepted transaction should commit");

        let promoted: (String, Option<chrono::DateTime<chrono::Utc>>) =
            sqlx::query_as("SELECT retention_class, expires_at FROM files WHERE id = $1")
                .bind(file_id)
                .fetch_one(&pool)
                .await
                .expect("promoted file should load");
        assert_eq!(promoted, ("standard".to_string(), None));
    }
}
