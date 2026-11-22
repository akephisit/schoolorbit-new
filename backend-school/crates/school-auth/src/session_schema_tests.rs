use school_test_db::{create_named_test_pool, create_test_user, run_test_migrations};

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

#[tokio::test]
async fn migration_145_enforces_pending_pair_digest_and_lifetime_bounds() {
    let pool = create_named_test_pool("auth_pending_schema").await;
    run_test_migrations(&pool).await;
    let user = create_test_user(&pool, "pending-schema@example.test", "test-password")
        .await
        .unwrap();
    let id: uuid::Uuid = sqlx::query_scalar("INSERT INTO auth_sessions (user_id,current_token_hash,remember_me,device_label,created_at,last_seen_at,idle_expires_at,absolute_expires_at,rotated_at) VALUES ($1,decode(repeat('01',32),'hex'),false,'Synthetic',now(),now(),now()+interval '2 hours',now()+interval '12 hours',now()) RETURNING id")
        .bind(user).fetch_one(&pool).await.unwrap();
    for statement in [
        "UPDATE auth_sessions SET pending_token_hash=decode(repeat('02',32),'hex') WHERE id=$1",
        "UPDATE auth_sessions SET pending_created_at=rotated_at WHERE id=$1",
        "UPDATE auth_sessions SET pending_token_hash=decode('02','hex'),pending_created_at=rotated_at WHERE id=$1",
        "UPDATE auth_sessions SET pending_token_hash=decode(repeat('02',32),'hex'),pending_created_at=rotated_at-interval '1 microsecond' WHERE id=$1",
        "UPDATE auth_sessions SET pending_token_hash=decode(repeat('02',32),'hex'),pending_created_at=absolute_expires_at WHERE id=$1",
        "UPDATE auth_sessions SET pending_token_hash=decode(repeat('02',32),'hex'),pending_created_at=rotated_at,revoked_at=now() WHERE id=$1",
    ] { assert!(sqlx::query(statement).bind(id).execute(&pool).await.is_err()); }
    sqlx::query("UPDATE auth_sessions SET pending_token_hash=decode(repeat('02',32),'hex'),pending_created_at=rotated_at WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    let index: bool = sqlx::query_scalar("SELECT indisunique FROM pg_index WHERE indexrelid='auth_sessions_pending_token_hash_key'::regclass").fetch_one(&pool).await.unwrap();
    assert!(index);
}

#[tokio::test]
async fn migration_168_enforces_device_same_user_links_and_does_not_backfill() {
    let pool = create_named_test_pool("auth_remembered_schema").await;
    run_test_migrations(&pool).await;
    let user = create_test_user(&pool, "remembered-schema@example.test", "test-password")
        .await
        .unwrap();
    let other = create_test_user(&pool, "remembered-other@example.test", "test-password")
        .await
        .unwrap();
    let grant = uuid::Uuid::new_v4();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_remembered_devices")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO auth_remembered_devices(id,user_id,current_token_hash,device_label,created_at,last_used_at) VALUES($1,$2,decode(repeat('a1',32),'hex'),'Synthetic',now(),now())")
        .bind(grant).bind(user).execute(&pool).await.unwrap();
    for sql in [
        "UPDATE auth_remembered_devices SET pending_token_hash=decode(repeat('a2',32),'hex') WHERE id=$1",
        "UPDATE auth_remembered_devices SET current_token_hash=decode('a1','hex') WHERE id=$1",
    ] { assert!(sqlx::query(sql).bind(grant).execute(&pool).await.is_err()); }
    let invalid=sqlx::query("INSERT INTO auth_sessions(user_id,current_token_hash,remember_me,device_label,created_at,last_seen_at,idle_expires_at,absolute_expires_at,rotated_at,remembered_device_id) VALUES($1,decode(repeat('a3',32),'hex'),true,'Synthetic',now(),now(),now()+interval '7 days',now()+interval '30 days',now(),$2)")
        .bind(other).bind(grant).execute(&pool).await;
    assert!(
        invalid.is_err(),
        "a different user's session cannot adopt a remembered device"
    );
    let security_invoker:bool=sqlx::query_scalar("SELECT 'security_invoker=true'=ANY(reloptions) FROM pg_class WHERE oid='notification_eligible_push_subscriptions'::regclass").fetch_one(&pool).await.unwrap();
    assert!(security_invoker);
}

#[tokio::test]
async fn migration_168_preserves_populated_167_sessions_and_push_on_apply_and_replay() {
    let pool = create_named_test_pool("remembered_168_preservation").await;
    school_test_db::cutover_test_support::apply_migrations_through(&pool, 167)
        .await
        .unwrap();
    let user = create_test_user(
        &pool,
        "remembered-preservation@example.test",
        "test-password",
    )
    .await
    .unwrap();
    let session:uuid::Uuid=sqlx::query_scalar("INSERT INTO auth_sessions(user_id,current_token_hash,remember_me,device_label,created_at,last_seen_at,idle_expires_at,absolute_expires_at,rotated_at) VALUES($1,decode(repeat('b1',32),'hex'),true,'Historical remembered browser',now(),now(),now()+interval '7 days',now()+interval '30 days',now()) RETURNING id")
        .bind(user).fetch_one(&pool).await.unwrap();
    let subscription:uuid::Uuid=sqlx::query_scalar("INSERT INTO push_subscriptions(user_id,endpoint,p256dh_key,auth_key,session_id,installation_id,label,key_id,expires_at) VALUES($1,'https://push.example.test/remembered-preservation','synthetic-p256dh','synthetic-auth',$2,$3,'Historical browser','synthetic-key',now()+interval '90 days') RETURNING id")
        .bind(user).bind(session).bind(uuid::Uuid::new_v4()).fetch_one(&pool).await.unwrap();
    let legacy:uuid::Uuid=sqlx::query_scalar("INSERT INTO push_subscriptions(user_id,endpoint,p256dh_key,auth_key) VALUES($1,'https://push.example.test/unproven-preservation','synthetic-legacy-p256dh','synthetic-legacy-auth') RETURNING id")
        .bind(user).fetch_one(&pool).await.unwrap();
    let before_session: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(s) FROM auth_sessions s WHERE id=$1")
            .bind(session)
            .fetch_one(&pool)
            .await
            .unwrap();
    let before_push: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(p) FROM push_subscriptions p WHERE id=ANY($1) ORDER BY id",
    )
    .bind(&[subscription, legacy][..])
    .fetch_all(&pool)
    .await
    .unwrap();
    for _ in 0..2 {
        run_test_migrations(&pool).await;
        let after_session: serde_json::Value = sqlx::query_scalar(
            "SELECT to_jsonb(s)-'remembered_device_id' FROM auth_sessions s WHERE id=$1",
        )
        .bind(session)
        .fetch_one(&pool)
        .await
        .unwrap();
        let after_push:Vec<serde_json::Value>=sqlx::query_scalar("SELECT to_jsonb(p)-'remembered_device_id' FROM push_subscriptions p WHERE id=ANY($1) ORDER BY id").bind(&[subscription,legacy][..]).fetch_all(&pool).await.unwrap();
        assert_eq!(after_session, before_session);
        assert_eq!(after_push, before_push);
        let credentials: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_remembered_devices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            credentials, 0,
            "historical remember_me never mints a browser credential"
        );
        let adopted: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM auth_sessions WHERE remembered_device_id IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(adopted, 0);
    }
    let eligible: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM notification_eligible_push_subscriptions ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        eligible,
        vec![subscription],
        "the unproven legacy registration stays ineligible"
    );
}
