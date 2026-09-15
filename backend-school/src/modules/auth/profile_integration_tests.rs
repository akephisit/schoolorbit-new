use school_auth::{models::UpdateProfileRequest, services};
use school_file_platform::profile_relationships::promote_ready_owned_image;
use school_test_db::{create_named_test_pool, create_test_user, run_test_migrations};
use sqlx::PgPool;
use uuid::Uuid;

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
async fn profile_update_composes_file_promotion_and_auth_mutation_atomically() {
    let pool = create_named_test_pool("auth_profile_file_composition").await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(&pool, "auth-profile@example.test", "test-password")
        .await
        .expect("user fixture should insert");
    let file_id = insert_ready_temporary_profile_image(&pool, user_id).await;
    let payload = UpdateProfileRequest {
        title: None,
        nickname: Some("Profile composition".to_string()),
        email: None,
        phone: None,
        emergency_contact: None,
        line_id: None,
        date_of_birth: None,
        gender: None,
        address: None,
        profile_image_file_id: Some(Some(file_id)),
    };

    let mut transaction = pool
        .begin()
        .await
        .expect("profile transaction should begin");
    promote_ready_owned_image(&mut transaction, file_id, user_id)
        .await
        .expect("File Platform relationship should promote");
    let outcome = services::update_profile(&mut transaction, user_id, payload)
        .await
        .expect("auth profile should update");
    transaction
        .commit()
        .await
        .expect("composed transaction should commit");

    assert_eq!(outcome.replaced_file_id, None);
    let user = services::find_user_by_id(&pool, user_id)
        .await
        .expect("updated user should load");
    assert_eq!(user.profile_image_file_id, Some(file_id));
    assert_eq!(user.nickname.as_deref(), Some("Profile composition"));
    let file: (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT retention_class, expires_at FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .expect("promoted file should load");
    assert_eq!(file, ("standard".to_string(), None));
}
