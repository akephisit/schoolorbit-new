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
    let valid = school_database::DbSession::new(&mut *connection)
        .scalar::<bool>(
            "file-platform.src.profile_relationships.promote_ready_owned_image",
            r#"
SELECT true
    FROM files
    WHERE id = $1
      AND owner_user_id = $2
      AND purpose_code = 'profile_image'
      AND lifecycle_status = 'ready'
      AND deleted_at IS NULL
    FOR UPDATE
"#,
        )
        .bind(file_id)
        .bind(owner_user_id)
        .fetch_optional()
        .await
        .map(|value| value.unwrap_or(false))?;
    if !valid {
        return Err(ProfileRelationshipError::InvalidFile);
    }

    school_database::DbSession::new(&mut *connection)
        .query(
            "file-platform.src.profile_relationships.promote_ready_owned_image",
            "UPDATE files
         SET retention_class = 'standard', expires_at = NULL, updated_at = NOW()
         WHERE id = $1",
        )
        .bind(file_id)
        .execute()
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
    #[tokio::test]
    async fn profile_promotion_waits_for_revocation_and_never_retains_revoked_file() {
        use std::time::Duration;
        let pool = school_test_db::create_named_test_pool_with_max_connections(
            "profile_revocation_promotion_race",
            3,
        )
        .await;
        run_test_migrations(&pool).await;
        let owner = create_test_user(&pool, "profile-race@example.test", "test-password")
            .await
            .unwrap();
        let file = insert_ready_temporary_profile_image(&pool, owner).await;
        let mut revocation = pool.begin().await.unwrap();
        // Delete acceptance owns this row lock; the trigger persists irreversible revocation.
        sqlx::query("UPDATE files SET lifecycle_status='delete_requested',delete_requested_at=statement_timestamp() WHERE id=$1").bind(file).execute(&mut *revocation).await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *revocation)
            .await
            .unwrap();
        let worker_pool = pool.clone();
        let worker = tokio::spawn(async move {
            let mut transaction = worker_pool.begin().await.unwrap();
            let result = promote_ready_owned_image(&mut transaction, file, owner).await;
            transaction.rollback().await.unwrap();
            result
        });
        tokio::time::timeout(Duration::from_secs(3),async {loop {
            sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *revocation).await.unwrap();
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))").bind(pid).fetch_one(&mut *revocation).await.unwrap();
            if waiting {break;}tokio::task::yield_now().await;
        }}).await.unwrap();
        let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('file',to_jsonb(f),'watch',to_jsonb(w)) FROM files f JOIN file_storage_cleanup_watches w ON w.file_id=f.id WHERE f.id=$1").bind(file).fetch_one(&mut *revocation).await.unwrap();
        revocation.commit().await.unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), worker)
                .await
                .unwrap()
                .unwrap(),
            Err(ProfileRelationshipError::InvalidFile)
        ));
        let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('file',to_jsonb(f),'watch',to_jsonb(w)) FROM files f JOIN file_storage_cleanup_watches w ON w.file_id=f.id WHERE f.id=$1").bind(file).fetch_one(&pool).await.unwrap();
        assert_eq!(before, after);
        assert_eq!(after["file"]["retention_class"], "temporary");
        assert!(after["watch"]["revoked_at"].is_string());
    }
}
