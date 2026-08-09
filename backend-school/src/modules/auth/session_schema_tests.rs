use crate::test_helpers::{create_named_test_pool, create_test_user, run_test_migrations};

#[tokio::test]
async fn migration_034_enforces_hash_and_expiry_constraints() {
    let pool = create_named_test_pool("auth_session_schema").await;
    run_test_migrations(&pool).await;

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'auth_sessions' \
         ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(columns.contains(&"current_token_hash".to_string()));
    assert!(!columns
        .iter()
        .any(|column| column.contains("token") && column.ends_with("value")));

    let user_id = create_test_user(&pool, "auth-schema@example.test", "test-password")
        .await
        .unwrap();
    let invalid_expiry = sqlx::query(
        "INSERT INTO auth_sessions \
         (user_id, current_token_hash, remember_me, device_label, created_at, last_seen_at, \
          idle_expires_at, absolute_expires_at, rotated_at) \
         VALUES ($1, decode(repeat('00', 32), 'hex'), false, 'Test device', now(), now(), \
                 now() - interval '1 second', now() + interval '1 hour', now())",
    )
    .bind(user_id)
    .execute(&pool)
    .await;
    assert!(
        invalid_expiry.is_err(),
        "idle expiry before creation must violate the check"
    );

    let invalid_hash = sqlx::query(
        "INSERT INTO auth_login_throttles \
         (bucket_kind, bucket_hash, failure_count, window_started_at, updated_at) \
         VALUES ('identifier', decode('00', 'hex'), 1, now(), now())",
    )
    .execute(&pool)
    .await;
    assert!(
        invalid_hash.is_err(),
        "one-byte throttle hashes must violate the check"
    );
}
