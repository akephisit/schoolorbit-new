use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

const STUDENT: Uuid = Uuid::from_u128(0x50000000000000000000000000000001);
const TEACHER: Uuid = Uuid::from_u128(0x50000000000000000000000000000002);
const TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000251);

async fn fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 118).await.unwrap();
    pool
}

async fn rows(pool: &PgPool, query: &'static str) -> Value {
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}

async fn legacy_rows(pool: &PgPool) -> Value {
    rows(
        pool,
        "SELECT coalesce(jsonb_agg(to_jsonb(a) ORDER BY id),'[]') FROM attendance_notifications a",
    )
    .await
}

async fn inbox_rows(pool: &PgPool) -> Value {
    rows(
        pool,
        "SELECT coalesce(jsonb_agg(to_jsonb(n) ORDER BY id),'[]') FROM notifications n",
    )
    .await
}

async fn central_rows(pool: &PgPool) -> Value {
    rows(
        pool,
        "SELECT coalesce(jsonb_agg(to_jsonb(j) ORDER BY id),'[]') FROM notification_dispatch_jobs j",
    )
    .await
}

async fn seed_legacy(pool: &PgPool) -> Vec<(Uuid, Uuid)> {
    let guardian = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,'dispatch-guardian@example.invalid','dispatch-guardian','fixture-not-a-login','Guardian','Fixture','parent','active')")
        .bind(guardian).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO student_parents(student_user_id,parent_user_id,relationship) VALUES($1,$2,'parent')")
        .bind(STUDENT).bind(guardian).execute(pool).await.unwrap();
    let mut identities = Vec::new();
    for index in 0..8_i32 {
        // Both UUID generations occur in real legacy data; neither is regenerated.
        let id = if index % 2 == 0 {
            Uuid::new_v4()
        } else {
            Uuid::now_v7()
        };
        let notification_id = Uuid::new_v4();
        let recipient = if index == 7 { guardian } else { STUDENT };
        sqlx::query(
            r#"INSERT INTO attendance_notifications(
                id,academic_term_id,recipient_id,student_id,dedup_key,title,message,link,
                notification_id,created_at,stored_at,published_at,next_attempt_at,attempts)
            VALUES($1,$2,$3,CASE WHEN $5=6 THEN NULL ELSE $4 END,
                CASE WHEN $5=7 THEN 'legacy:0' ELSE 'legacy:'||$5::text END,
                'Legacy title','Legacy message','/student/attendance', $6,
                '2026-10-01 01:00:00.123456+00'::timestamptz + $5 * interval '1 second',
                CASE WHEN $5 IN (2,3,5,6) THEN '2026-10-02 02:00:00.654321+00'::timestamptz END,
                CASE WHEN $5 IN (3,4,6) THEN '2026-10-03 03:00:00.987654+00'::timestamptz END,
                CASE WHEN $5=1 THEN now()+interval '2 hours' ELSE now()-interval '1 day' END,
                $5)"#,
        )
        .bind(id)
        .bind(TERM)
        .bind(recipient)
        .bind(STUDENT)
        .bind(index)
        .bind(notification_id)
        .execute(pool)
        .await
        .unwrap();
        if matches!(index, 2 | 3) {
            sqlx::query(
                r#"INSERT INTO notifications(id,user_id,title,message,type,link,read_at,created_at)
                SELECT notification_id,recipient_id,title,message,'info',link,
                    CASE WHEN $2=2 THEN '2026-10-04 04:00:00.111111+00'::timestamptz END,
                    '2026-10-02 02:00:01.222222+00'::timestamptz
                FROM attendance_notifications WHERE id=$1"#,
            )
            .bind(id)
            .bind(index)
            .execute(pool)
            .await
            .unwrap();
        }
        identities.push((id, notification_id));
    }
    sqlx::query("INSERT INTO notifications(user_id,title,message,type,link,read_at) VALUES($1,'Unrelated','Preserve this inbox','warning',NULL,now())")
        .bind(TEACHER).execute(pool).await.unwrap();
    identities
}

async fn assert_refusal(pool: &PgPool, code: &str) {
    let source_before = legacy_rows(pool).await;
    let inbox_before = inbox_rows(pool).await;
    let jobs_before = central_rows(pool).await;
    let error = apply_migrations_through(pool, 120).await.unwrap_err();
    assert!(
        error.to_string().contains(code),
        "expected bounded code {code}"
    );
    assert_eq!(legacy_rows(pool).await, source_before);
    assert_eq!(inbox_rows(pool).await, inbox_before);
    assert_eq!(central_rows(pool).await, jobs_before);
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(latest, 119);
    let legacy_payload_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='attendance_notifications' AND column_name='message')",
    ).fetch_one(pool).await.unwrap();
    assert!(
        legacy_payload_exists,
        "failed cutover must not contract source"
    );
    assert!(school_migrations::verify_tenant_schema(pool).await.is_err());
}

/// This target deliberately stays at the historical schema; current readiness must reject it.
async fn assert_historical_schema_120(pool: &PgPool) {
    let latest_successful: i64 =
        sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(latest_successful, 120);
    assert!(school_migrations::latest_migration_version() > 120);
    assert!(school_migrations::verify_tenant_schema(pool).await.is_err());
}

#[tokio::test]
async fn notification_dispatch_migration_preserves_all_states_and_replays() {
    let pool = fixture("dispatch_migration_preservation").await;
    let identities = seed_legacy(&pool).await;
    let legacy = legacy_rows(&pool).await;
    let inbox = inbox_rows(&pool).await;
    // The current compiled runtime refuses these historical predecessor histories; keep the strict gate.
    assert!(school_migrations::verify_tenant_schema(&pool)
        .await
        .is_err());
    apply_migrations_through(&pool, 119).await.unwrap();
    assert_eq!(legacy_rows(&pool).await, legacy);
    assert_eq!(inbox_rows(&pool).await, inbox);
    assert_eq!(central_rows(&pool).await, serde_json::json!([]));
    assert!(school_migrations::verify_tenant_schema(&pool)
        .await
        .is_err());

    let expected_jobs = rows(
        &pool,
        r#"SELECT jsonb_agg(to_jsonb(j) ORDER BY id) FROM (
        SELECT id,notification_id,recipient_id,'attendance'::text source_kind,id source_id,
            dedup_key,title,message,'info'::text type,link,created_at,stored_at,published_at,
            next_attempt_at,attempts,NULL::uuid lease_token FROM attendance_notifications) j"#,
    )
    .await;
    let expected_ledger = rows(
        &pool,
        r#"SELECT jsonb_agg(to_jsonb(l) ORDER BY id) FROM (
        SELECT id,academic_term_id,recipient_id,student_id,dedup_key,notification_id,created_at
        FROM attendance_notifications) l"#,
    )
    .await;
    apply_migrations_through(&pool, 120).await.unwrap();
    assert_eq!(central_rows(&pool).await, expected_jobs);
    assert_eq!(legacy_rows(&pool).await, expected_ledger);
    assert_eq!(inbox_rows(&pool).await, inbox);
    let columns: Vec<String> = sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='attendance_notifications' ORDER BY ordinal_position")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(
        columns,
        [
            "id",
            "academic_term_id",
            "recipient_id",
            "student_id",
            "dedup_key",
            "notification_id",
            "created_at"
        ]
    );
    let old_index_exists: bool = sqlx::query_scalar(
        "SELECT to_regclass(current_schema()||'.attendance_notification_due_idx') IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!old_index_exists);
    for (index, (_, notification_id)) in identities.iter().enumerate() {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)")
                .bind(notification_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            exists,
            matches!(index, 2 | 3),
            "migration must not recreate absent inboxes"
        );
    }
    // Replay only the historical migration selection, so all original queue evidence remains queryable.
    apply_migrations_through(&pool, 120).await.unwrap();
    apply_migrations_through(&pool, 120).await.unwrap();
    assert_historical_schema_120(&pool).await;
    assert_eq!(central_rows(&pool).await, expected_jobs);
    assert_eq!(legacy_rows(&pool).await, expected_ledger);
    assert_eq!(inbox_rows(&pool).await, inbox);
}

#[tokio::test]
async fn notification_dispatch_migration_refuses_conflicts_and_retries() {
    let pool = fixture("dispatch_migration_refusals").await;
    let identities = seed_legacy(&pool).await;
    apply_migrations_through(&pool, 119).await.unwrap();
    let first = identities[0];
    let second = identities[1];
    sqlx::query("UPDATE attendance_notifications SET notification_id=$2 WHERE id=$1")
        .bind(second.0)
        .bind(first.1)
        .execute(&pool)
        .await
        .unwrap();
    assert_refusal(&pool, "NOTIFICATION_DISPATCH_IDENTITY_CONFLICT").await;
    sqlx::query("UPDATE attendance_notifications SET notification_id=$2 WHERE id=$1")
        .bind(second.0)
        .bind(second.1)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE attendance_notifications SET attempts=-1 WHERE id=$1")
        .bind(first.0)
        .execute(&pool)
        .await
        .unwrap();
    assert_refusal(&pool, "NOTIFICATION_DISPATCH_PAYLOAD_UNMAPPABLE").await;
    sqlx::query("UPDATE attendance_notifications SET attempts=0,title=repeat('ก',256) WHERE id=$1")
        .bind(first.0)
        .execute(&pool)
        .await
        .unwrap();
    assert_refusal(&pool, "NOTIFICATION_DISPATCH_PAYLOAD_UNMAPPABLE").await;
    sqlx::query("UPDATE attendance_notifications SET title='Legacy title' WHERE id=$1")
        .bind(first.0)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO notifications(id,user_id,title,message,type,link) SELECT notification_id,recipient_id,title,message,'info',link FROM attendance_notifications WHERE id=$1")
        .bind(first.0).execute(&pool).await.unwrap();
    for field in ["recipient", "title", "message", "type", "link"] {
        sqlx::query(
            r#"UPDATE notifications SET
            user_id=CASE WHEN $2='recipient' THEN $3 ELSE $4 END,
            title=CASE WHEN $2='title' THEN 'Different' ELSE 'Legacy title' END,
            message=CASE WHEN $2='message' THEN 'Different' ELSE 'Legacy message' END,
            type=CASE WHEN $2='type' THEN 'warning' ELSE 'info' END,
            link=CASE WHEN $2='link' THEN NULL ELSE '/student/attendance' END
            WHERE id=$1"#,
        )
        .bind(first.1)
        .bind(field)
        .bind(TEACHER)
        .bind(STUDENT)
        .execute(&pool)
        .await
        .unwrap();
        assert_refusal(&pool, "NOTIFICATION_DISPATCH_INBOX_CONFLICT").await;
    }
    sqlx::query("DELETE FROM notifications WHERE id=$1")
        .bind(first.1)
        .execute(&pool)
        .await
        .unwrap();

    let unexpected = Uuid::new_v4();
    sqlx::query("INSERT INTO notification_dispatch_jobs(id,notification_id,recipient_id,source_kind,title,message,type) VALUES($1,$2,$3,'direct','Unexpected','Fixture','info')")
        .bind(unexpected).bind(Uuid::new_v4()).bind(STUDENT).execute(&pool).await.unwrap();
    assert_refusal(&pool, "NOTIFICATION_DISPATCH_IDENTITY_CONFLICT").await;
    sqlx::query("DELETE FROM notification_dispatch_jobs WHERE id=$1")
        .bind(unexpected)
        .execute(&pool)
        .await
        .unwrap();
    // Repair only synthetic unmappable input, then retry the unchanged canonical migration.
    apply_migrations_through(&pool, 120).await.unwrap();
    assert_historical_schema_120(&pool).await;
}

#[tokio::test]
async fn notification_dispatch_migration_refuses_reconciliation_drift_and_retries() {
    let pool = fixture("dispatch_migration_reconciliation").await;
    seed_legacy(&pool).await;
    apply_migrations_through(&pool, 119).await.unwrap();
    sqlx::raw_sql(
        r#"CREATE FUNCTION corrupt_dispatch_copy() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN NEW.message := NEW.message || ' drift'; RETURN NEW; END $$;
        CREATE TRIGGER corrupt_dispatch_copy BEFORE INSERT ON notification_dispatch_jobs
        FOR EACH ROW EXECUTE FUNCTION corrupt_dispatch_copy();"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_refusal(&pool, "NOTIFICATION_DISPATCH_PRESERVATION_FAILED").await;
    sqlx::raw_sql("DROP TRIGGER corrupt_dispatch_copy ON notification_dispatch_jobs; DROP FUNCTION corrupt_dispatch_copy();")
        .execute(&pool).await.unwrap();

    sqlx::raw_sql(
        r#"CREATE FUNCTION corrupt_dispatch_inbox() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN UPDATE notifications SET read_at=NULL; RETURN NEW; END $$;
        CREATE TRIGGER corrupt_dispatch_inbox AFTER INSERT ON notification_dispatch_jobs
        FOR EACH ROW EXECUTE FUNCTION corrupt_dispatch_inbox();"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_refusal(&pool, "NOTIFICATION_DISPATCH_PRESERVATION_FAILED").await;
    sqlx::raw_sql("DROP TRIGGER corrupt_dispatch_inbox ON notification_dispatch_jobs; DROP FUNCTION corrupt_dispatch_inbox();")
        .execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 120).await.unwrap();
    assert_historical_schema_120(&pool).await;
}

#[tokio::test]
async fn notification_dispatch_migration_schema_rejects_unknown_sources() {
    let pool = fixture("dispatch_migration_source_constraints").await;
    apply_migrations_through(&pool, 119).await.unwrap();
    for (source, source_id) in [
        ("unknown", None),
        ("attendance", None),
        ("direct", Some(Uuid::new_v4())),
    ] {
        let error = sqlx::query("INSERT INTO notification_dispatch_jobs(notification_id,recipient_id,source_kind,source_id,title,message,type) VALUES($1,$2,$3,$4,'Fixture','Fixture','info')")
            .bind(Uuid::new_v4()).bind(STUDENT).bind(source).bind(source_id)
            .execute(&pool).await.unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("23514")
        );
    }
    assert_eq!(central_rows(&pool).await, serde_json::json!([]));
    apply_migrations_through(&pool, 120).await.unwrap();
    assert_historical_schema_120(&pool).await;
}
