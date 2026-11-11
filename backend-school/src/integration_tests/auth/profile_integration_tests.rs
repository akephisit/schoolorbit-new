use school_auth::{models::UpdateProfileRequest, services};
use school_auth_http::http::state::ProfileUpdatePort;
use school_errors::AppError;
use school_test_db::{create_named_test_pool, create_test_user, run_test_migrations};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::adapters::auth_profile::ApplicationAuthProfile;

pub(crate) async fn insert_ready_temporary_profile_image(
    pool: &PgPool,
    owner_user_id: Uuid,
) -> Uuid {
    let file_id = Uuid::new_v4();
    insert_profile_image(
        pool,
        owner_user_id,
        file_id,
        format!("profile-test/{}", Uuid::new_v4()),
    )
    .await
}

pub(crate) async fn insert_cleanup_profile_image(pool: &PgPool, owner_user_id: Uuid) -> Uuid {
use school_file_contracts::platform_types::{DetectedContent, FilePurpose};
    use school_file_platform::purpose_registry::original_object_key;

    let file_id = Uuid::new_v4();
    let key = original_object_key(
        Uuid::new_v4(),
        FilePurpose::ProfileImage,
        file_id,
        1,
        DetectedContent::Jpeg,
    )
    .expect("cleanup fixture uses the File Platform's canonical key");
    insert_profile_image(pool, owner_user_id, file_id, key.as_str().to_string()).await
}

async fn insert_profile_image(
    pool: &PgPool,
    owner_user_id: Uuid,
    file_id: Uuid,
    object_key: String,
) -> Uuid {
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
    .bind(object_key)
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

fn profile_updates() -> Arc<dyn ProfileUpdatePort> {
    Arc::new(ApplicationAuthProfile::new(Arc::new(
        crate::integration_tests::staff::career_integration_tests::file_platform(),
    )))
}

fn profile_update(image: Option<Option<Uuid>>, nickname: &str) -> UpdateProfileRequest {
    UpdateProfileRequest {
        title: None,
        nickname: Some(nickname.to_string()),
        email: None,
        phone: None,
        emergency_contact: None,
        line_id: None,
        date_of_birth: None,
        gender: None,
        address: None,
        profile_image_file_id: image,
    }
}

async fn file_snapshot(pool: &PgPool, file_id: Uuid) -> Value {
    sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'file', to_jsonb(f),
            'versions', (SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY v.id), '[]'::jsonb)
                         FROM file_versions v WHERE v.file_id = f.id),
            'operations', (SELECT COALESCE(jsonb_agg(to_jsonb(o) ORDER BY o.id), '[]'::jsonb)
                           FROM file_operations o WHERE o.file_id = f.id),
            'watches', (SELECT COALESCE(jsonb_agg(to_jsonb(w) ORDER BY w.id), '[]'::jsonb)
                        FROM file_storage_cleanup_watches w WHERE w.file_id = f.id))
         FROM files f WHERE f.id = $1",
    )
    .bind(file_id)
    .fetch_one(pool)
    .await
    .expect("file facts should load")
}

async fn user_snapshot(pool: &PgPool, user_id: Uuid) -> Value {
    sqlx::query_scalar("SELECT to_jsonb(u) FROM users u WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("user facts should load")
}

async fn assert_retained_image(pool: &PgPool, file_id: Uuid) {
    let facts = file_snapshot(pool, file_id).await;
    assert_eq!(facts["file"]["lifecycle_status"], "ready");
    assert_eq!(facts["file"]["retention_class"], "standard");
    assert!(facts["file"]["expires_at"].is_null());
    assert!(facts["file"]["delete_requested_at"].is_null());
    assert_eq!(facts["versions"][0]["storage_status"], "stored");
    assert!(facts["operations"].as_array().unwrap().is_empty());
    assert!(facts["watches"][0]["revoked_at"].is_null());
}

async fn assert_deletion_requested(pool: &PgPool, file_id: Uuid) {
    let facts = file_snapshot(pool, file_id).await;
    assert_eq!(facts["file"]["lifecycle_status"], "delete_requested");
    assert_eq!(facts["file"]["retention_class"], "standard");
    assert!(facts["file"]["expires_at"].is_null());
    assert!(facts["file"]["delete_requested_at"].is_string());
    assert!(facts["file"]["deleted_at"].is_null());
    assert_eq!(facts["versions"][0]["storage_status"], "delete_requested");
    assert_eq!(facts["operations"].as_array().unwrap().len(), 1);
    assert_eq!(facts["operations"][0]["operation_type"], "delete_object");
    // The shared fixture has no attested physical namespace. Cleanup is durable retry work,
    // rather than a provider call or a falsely reported physical deletion.
    assert_eq!(facts["operations"][0]["status"], "retryable_failure");
    assert_eq!(
        facts["operations"][0]["last_error_code"],
        "storage_namespace_unverified"
    );
    assert!(facts["watches"][0]["revoked_at"].is_string());
}

#[tokio::test]
async fn profile_update_composes_file_promotion_and_auth_mutation_atomically() {
    let pool = create_named_test_pool("auth_profile_file_composition").await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(&pool, "auth-profile@example.test", "test-password")
        .await
        .expect("user fixture should insert");
    let file_id = insert_ready_temporary_profile_image(&pool, user_id).await;
    let before = file_snapshot(&pool, file_id).await;
    assert_eq!(before["file"]["retention_class"], "temporary");
    assert!(before["file"]["expires_at"].is_string());

    let user = profile_updates()
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(file_id)), "Profile composition"),
        )
        .await
        .expect("application profile command should commit");
    assert_eq!(user.profile_image_file_id, Some(file_id));
    assert_eq!(user.nickname.as_deref(), Some("Profile composition"));
    assert_eq!(
        services::find_user_by_id(&pool, user_id)
            .await
            .unwrap()
            .profile_image_file_id,
        Some(file_id)
    );
    assert_retained_image(&pool, file_id).await;
}

#[tokio::test]
async fn profile_update_preserves_same_replaces_and_clears_images_after_commit() {
    let pool = create_named_test_pool("auth_profile_image_lifecycle").await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(&pool, "auth-images@example.test", "test-password")
        .await
        .unwrap();
    let old_file = insert_cleanup_profile_image(&pool, user_id).await;
    let new_file = insert_cleanup_profile_image(&pool, user_id).await;
    let updates = profile_updates();
    updates
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(old_file)), "Initial"),
        )
        .await
        .unwrap();

    sqlx::raw_sql(
        "CREATE TABLE profile_cleanup_expectation (user_id uuid, image_id uuid);
         CREATE TABLE profile_cleanup_observations (file_id uuid, image_id uuid, nickname text);
         CREATE FUNCTION observe_profile_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$
         DECLARE image uuid; profile_nickname text;
         BEGIN
             SELECT u.profile_image_file_id, u.nickname INTO image, profile_nickname
             FROM users u JOIN profile_cleanup_expectation e ON e.user_id = u.id
             WHERE u.profile_image_file_id IS NOT DISTINCT FROM e.image_id;
             IF NOT FOUND THEN
                 RAISE EXCEPTION 'cleanup_must_observe_committed_profile';
             END IF;
             IF image IS NOT NULL AND NOT EXISTS (
                 SELECT 1 FROM files WHERE id = image AND lifecycle_status = 'ready'
                 AND retention_class = 'standard' AND expires_at IS NULL
             ) THEN
                 RAISE EXCEPTION 'cleanup_must_observe_committed_promotion';
             END IF;
             INSERT INTO profile_cleanup_observations VALUES (OLD.id, image, profile_nickname);
             RETURN NEW;
         END $$;
         CREATE TRIGGER observe_profile_cleanup BEFORE UPDATE OF lifecycle_status ON files
         FOR EACH ROW WHEN (NEW.lifecycle_status = 'delete_requested')
         EXECUTE FUNCTION observe_profile_cleanup();",
    )
    .execute(&pool)
    .await
    .unwrap();

    let omitted = updates
        .update_profile(&pool, user_id, profile_update(None, "Omitted"))
        .await
        .unwrap();
    assert_eq!(omitted.profile_image_file_id, Some(old_file));
    assert_retained_image(&pool, old_file).await;
    let same = updates
        .update_profile(&pool, user_id, profile_update(Some(Some(old_file)), "Same"))
        .await
        .unwrap();
    assert_eq!(same.profile_image_file_id, Some(old_file));
    assert_retained_image(&pool, old_file).await;
    let observations: i64 = sqlx::query_scalar("SELECT count(*) FROM profile_cleanup_observations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        observations, 0,
        "omitted/same image must not request cleanup"
    );

    sqlx::query("INSERT INTO profile_cleanup_expectation VALUES ($1, $2)")
        .bind(user_id)
        .bind(new_file)
        .execute(&pool)
        .await
        .unwrap();
    let replaced = updates
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(new_file)), "Replaced"),
        )
        .await
        .unwrap();
    assert_eq!(replaced.profile_image_file_id, Some(new_file));
    assert_eq!(replaced.nickname.as_deref(), Some("Replaced"));
    assert_retained_image(&pool, new_file).await;
    assert_deletion_requested(&pool, old_file).await;

    sqlx::query("UPDATE profile_cleanup_expectation SET image_id = NULL")
        .execute(&pool)
        .await
        .unwrap();
    let cleared = updates
        .update_profile(&pool, user_id, profile_update(Some(None), "Cleared"))
        .await
        .unwrap();
    assert_eq!(cleared.profile_image_file_id, None);
    assert_eq!(cleared.nickname.as_deref(), Some("Cleared"));
    assert_deletion_requested(&pool, new_file).await;
    let observations: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT file_id, image_id, nickname FROM profile_cleanup_observations ORDER BY nickname DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        observations,
        vec![
            (old_file, Some(new_file), "Replaced".to_string()),
            (new_file, None, "Cleared".to_string())
        ]
    );
}

#[tokio::test]
async fn profile_update_rejects_invalid_images_without_mutation_or_cleanup() {
    let pool = create_named_test_pool("auth_profile_image_rejections").await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(&pool, "auth-image-owner@example.test", "test-password")
        .await
        .unwrap();
    let other_id = create_test_user(&pool, "auth-image-other@example.test", "test-password")
        .await
        .unwrap();
    let old_file = insert_cleanup_profile_image(&pool, user_id).await;
    let foreign = insert_ready_temporary_profile_image(&pool, other_id).await;
    let wrong_purpose = insert_ready_temporary_profile_image(&pool, user_id).await;
    let unready = insert_ready_temporary_profile_image(&pool, user_id).await;
    sqlx::query("UPDATE files SET purpose_code = 'achievement_image' WHERE id = $1")
        .bind(wrong_purpose)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE files SET lifecycle_status = 'processing' WHERE id = $1")
        .bind(unready)
        .execute(&pool)
        .await
        .unwrap();
    let updates = profile_updates();
    updates
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(old_file)), "Original"),
        )
        .await
        .unwrap();
    let before_user = user_snapshot(&pool, user_id).await;
    let before_old = file_snapshot(&pool, old_file).await;
    for file_id in [foreign, wrong_purpose, unready] {
        let before_file = file_snapshot(&pool, file_id).await;
        let error = updates
            .update_profile(
                &pool,
                user_id,
                profile_update(Some(Some(file_id)), "Must not save"),
            )
            .await
            .expect_err("invalid image must be rejected");
        assert!(matches!(error, AppError::ValidationError(message)
            if message == "ไฟล์รูปโปรไฟล์ไม่พร้อมใช้งาน"));
        assert_eq!(user_snapshot(&pool, user_id).await, before_user);
        assert_eq!(file_snapshot(&pool, old_file).await, before_old);
        assert_eq!(file_snapshot(&pool, file_id).await, before_file);
    }
}

#[tokio::test]
async fn profile_update_rolls_back_promotion_when_auth_rejects_afterward() {
    let pool = create_named_test_pool("auth_profile_promotion_rollback").await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(&pool, "auth-rollback@example.test", "test-password")
        .await
        .unwrap();
    let old_file = insert_cleanup_profile_image(&pool, user_id).await;
    let new_file = insert_cleanup_profile_image(&pool, user_id).await;
    let updates = profile_updates();
    updates
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(old_file)), "Original"),
        )
        .await
        .unwrap();
    let before_user = user_snapshot(&pool, user_id).await;
    let before_old = file_snapshot(&pool, old_file).await;
    let before_new = file_snapshot(&pool, new_file).await;
    assert_eq!(before_new["file"]["retention_class"], "temporary");
    assert!(before_new["file"]["expires_at"].is_string());
    let mut payload = profile_update(Some(Some(new_file)), "Must roll back");
    payload.title = Some("x".repeat(51));
    let error = updates
        .update_profile(&pool, user_id, payload)
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::BadRequest(message)
        if message == "ข้อมูลยาวเกินกำหนดหรือมีอักขระควบคุม"));
    assert_eq!(user_snapshot(&pool, user_id).await, before_user);
    assert_eq!(file_snapshot(&pool, old_file).await, before_old);
    assert_eq!(file_snapshot(&pool, new_file).await, before_new);
}

#[tokio::test]
async fn profile_update_surfaces_cleanup_repository_failure_after_profile_commit() {
    let pool = create_named_test_pool("auth_profile_cleanup_repository_failure").await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(&pool, "auth-cleanup-error@example.test", "test-password")
        .await
        .unwrap();
    let old_file = insert_cleanup_profile_image(&pool, user_id).await;
    let new_file = insert_cleanup_profile_image(&pool, user_id).await;
    let updates = profile_updates();
    updates
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(old_file)), "Original"),
        )
        .await
        .unwrap();
    let before_old = file_snapshot(&pool, old_file).await;
    sqlx::raw_sql(
        "CREATE FUNCTION reject_profile_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'fixture_cleanup_repository_failure'; END $$;
         CREATE TRIGGER reject_profile_cleanup BEFORE UPDATE OF lifecycle_status ON files
         FOR EACH ROW WHEN (NEW.lifecycle_status = 'delete_requested')
         EXECUTE FUNCTION reject_profile_cleanup();",
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = updates
        .update_profile(
            &pool,
            user_id,
            profile_update(Some(Some(new_file)), "Committed"),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::InternalServerError(message)
        if message == "file_metadata_unavailable"));
    let committed = services::find_user_by_id(&pool, user_id).await.unwrap();
    assert_eq!(committed.profile_image_file_id, Some(new_file));
    assert_eq!(committed.nickname.as_deref(), Some("Committed"));
    assert_retained_image(&pool, new_file).await;
    assert_eq!(file_snapshot(&pool, old_file).await, before_old);
}
