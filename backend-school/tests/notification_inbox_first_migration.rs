//! Forward-only inbox preservation and interruption/refusal fixtures; no real tenant data.
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use sqlx::PgPool;
use uuid::Uuid;
const USER: Uuid = Uuid::from_u128(0x50000000000000000000000000000002);
async fn fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 138).await.unwrap();
    pool
}
async fn legacy(
    pool: &PgPool,
    stored: bool,
    terminal: bool,
    attempts: i32,
    inbox: bool,
) -> (Uuid, Uuid) {
    let id = Uuid::new_v4();
    let notification = Uuid::new_v4();
    sqlx::query("INSERT INTO notification_dispatch_jobs(id,notification_id,recipient_id,source_kind,dedup_key,title,message,type,link,created_at,stored_at,published_at,attempts) VALUES($1,$2,$3,'direct',$4,'Synthetic','Preserved body','info','/staff/calendar','2026-01-01 00:00:00+00',CASE WHEN $5 THEN '2026-01-01 00:01:00+00'::timestamptz END,CASE WHEN $6 THEN '2026-01-01 00:02:00+00'::timestamptz END,$7)")
 .bind(id).bind(notification).bind(USER).bind(format!("synthetic:{id}")).bind(stored).bind(terminal).bind(attempts).execute(pool).await.unwrap();
    if inbox {
        sqlx::query("INSERT INTO notifications(id,user_id,title,message,type,link,created_at,read_at) VALUES($1,$2,'Synthetic','Preserved body','info','/staff/calendar','2026-01-01 00:01:00+00','2026-01-01 00:03:00+00')").bind(notification).bind(USER).execute(pool).await.unwrap();
    }
    (id, notification)
}
async fn canonical_notification_state(pool: &PgPool) -> serde_json::Value {
    sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
            'receipts', (SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]'::jsonb) FROM notification_receipts r),
            'inbox', (SELECT coalesce(jsonb_agg(to_jsonb(n) ORDER BY n.id),'[]'::jsonb) FROM notifications n),
            'attendance_sources', (SELECT coalesce(jsonb_agg(to_jsonb(a) ORDER BY a.id),'[]'::jsonb) FROM attendance_notifications a),
            'process_sources', (SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY s.id),'[]'::jsonb) FROM process_notification_sources s),
            'presentations', (SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY p.receipt_id),'[]'::jsonb) FROM notification_presentations p)
        )"#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn assert_current_runner_replay_preserves_notification_state(pool: &PgPool) {
    // First advance through the real current runner. Future migrations may add canonical columns;
    // historical 139/140 preservation is checked separately before capturing this latest baseline.
    school_migrations::run_tenant_migrations(pool)
        .await
        .unwrap();
    assert_eq!(
        school_migrations::verify_tenant_schema(pool).await.unwrap(),
        school_migrations::latest_migration_version()
    );
    let accepted = canonical_notification_state(pool).await;
    school_migrations::run_tenant_migrations(pool)
        .await
        .unwrap();
    assert_eq!(
        school_migrations::verify_tenant_schema(pool).await.unwrap(),
        school_migrations::latest_migration_version()
    );
    assert_eq!(canonical_notification_state(pool).await, accepted);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_presentations")
            .fetch_one(pool)
            .await
            .unwrap(),
        0,
        "replaying the current migration runner must not schedule legacy alerts"
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT bool_and(presentation_outcome='legacy_skipped' AND presentation_consumed_at IS NOT NULL) FROM notification_receipts")
            .fetch_one(pool).await.unwrap(),
        "all legacy receipts must retain their explicit consumed suppression outcome"
    );
}

#[tokio::test]
async fn inbox_first_preserves_original_rows_materializes_claimed_pending_and_never_resurrects_suppressed(
) {
    let pool = fixture("inbox_first_migration").await;
    let pending = legacy(&pool, false, false, 3, false).await;
    let stored = legacy(&pool, true, false, 1, true).await;
    let missing = legacy(&pool, true, false, 1, false).await;
    let suppressed = legacy(&pool, false, true, 4, false).await;
    let stored_before: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(n) FROM notifications n WHERE id=$1")
            .bind(stored.1)
            .fetch_one(&pool)
            .await
            .unwrap();
    apply_migrations_through(&pool, 139).await.unwrap();
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1 AND created_at='2026-01-01 00:00:00+00' AND read_at IS NULL)").bind(pending.1).fetch_one(&pool).await.unwrap());
    apply_migrations_through(&pool, 140).await.unwrap();
    let stored_after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(n) FROM notifications n WHERE id=$1")
            .bind(stored.1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_before, stored_after);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications WHERE id=ANY($1)")
            .bind(vec![missing.1, suppressed.1])
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM notification_receipts WHERE presentation_outcome='legacy_skipped' AND presentation_consumed_at IS NOT NULL").fetch_one(&pool).await.unwrap(),4);
    assert!(sqlx::query_scalar::<_,bool>("SELECT inbox_accepted_at IS NULL AND legacy_published_at IS NOT NULL FROM notification_receipts WHERE id=$1").bind(suppressed.0).fetch_one(&pool).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_presentations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='notification_receipts' AND column_name IN ('title','message','type','link','attempts','lease_token','next_attempt_at')").fetch_one(&pool).await.unwrap(),0);
    apply_migrations_through(&pool, 140).await.unwrap();
    assert_current_runner_replay_preserves_notification_state(&pool).await;
}
#[tokio::test]
async fn inbox_first_refuses_conflicting_payload_and_suppressed_materialization_then_repairs() {
    let pool = fixture("inbox_first_payload_gate").await;
    let row = legacy(&pool, true, false, 0, true).await;
    sqlx::query("UPDATE notifications SET message='drift' WHERE id=$1")
        .bind(row.1)
        .execute(&pool)
        .await
        .unwrap();
    let error = apply_migrations_through(&pool, 139).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("NOTIFICATION_INBOX_FIRST_IDENTITY_CONFLICT"));
    sqlx::query("UPDATE notifications SET message='Preserved body' WHERE id=$1")
        .bind(row.1)
        .execute(&pool)
        .await
        .unwrap();
    let pending = legacy(&pool, false, false, 3, false).await;
    sqlx::raw_sql("CREATE FUNCTION suppress_synthetic_materialization() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$; CREATE TRIGGER suppress_synthetic_materialization BEFORE INSERT ON notifications FOR EACH ROW EXECUTE FUNCTION suppress_synthetic_materialization();").execute(&pool).await.unwrap();
    assert!(apply_migrations_through(&pool, 139)
        .await
        .unwrap_err()
        .to_string()
        .contains("NOTIFICATION_INBOX_FIRST_PRESERVATION_FAILED"));
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(pending.1)
    .fetch_one(&pool)
    .await
    .unwrap());
    sqlx::raw_sql("DROP TRIGGER suppress_synthetic_materialization ON notifications; DROP FUNCTION suppress_synthetic_materialization();").execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 140).await.unwrap();
}
#[tokio::test]
async fn inbox_first_contraction_rejects_drift_after_expansion_and_retries_only_after_exact_repair()
{
    let pool = fixture("inbox_first_stale_evidence").await;
    let row = legacy(&pool, true, false, 1, true).await;
    apply_migrations_through(&pool, 139).await.unwrap();
    for statement in [
        "UPDATE notifications SET read_at=read_at+interval '1 second' WHERE id=$1",
        "UPDATE notifications SET created_at=created_at+interval '1 second' WHERE id=$1",
    ] {
        sqlx::query(statement)
            .bind(row.1)
            .execute(&pool)
            .await
            .unwrap();
        assert!(apply_migrations_through(&pool, 140)
            .await
            .unwrap_err()
            .to_string()
            .contains("NOTIFICATION_RECEIPT_CONTRACTION_STALE_EVIDENCE"));
        sqlx::query("UPDATE notifications SET created_at='2026-01-01 00:01:00+00',read_at='2026-01-01 00:03:00+00' WHERE id=$1").bind(row.1).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE notification_dispatch_jobs SET dedup_key='drift' WHERE id=$1")
        .bind(row.0)
        .execute(&pool)
        .await
        .unwrap();
    assert!(apply_migrations_through(&pool, 140)
        .await
        .unwrap_err()
        .to_string()
        .contains("NOTIFICATION_RECEIPT_CONTRACTION_STALE_EVIDENCE"));
    sqlx::query("UPDATE notification_dispatch_jobs SET dedup_key=$2 WHERE id=$1")
        .bind(row.0)
        .bind(format!("synthetic:{}", row.0))
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 140).await.unwrap();
    assert!(!sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema=current_schema() AND table_name='notification_inbox_first_evidence')").fetch_one(&pool).await.unwrap());
}

#[tokio::test]
async fn inbox_first_extant_attendance_identity_conflict_refuses_instead_of_dropping_payload() {
    let pool = fixture("inbox_first_ledger_gate").await;
    let receipt = legacy(&pool, false, false, 3, false).await;
    let source = Uuid::new_v4();
    let original = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_notifications(id,academic_term_id,recipient_id,student_id,dedup_key,notification_id) VALUES($1,'11000000-0000-0000-0000-000000000251',$2,'50000000-0000-0000-0000-000000000001','synthetic-ledger',$3)").bind(source).bind(USER).bind(original).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE notification_dispatch_jobs SET source_kind='attendance',source_id=$2 WHERE id=$1",
    )
    .bind(receipt.0)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap();
    assert!(apply_migrations_through(&pool, 139)
        .await
        .unwrap_err()
        .to_string()
        .contains("NOTIFICATION_INBOX_FIRST_SOURCE_IDENTITY_CONFLICT"));
    sqlx::query("UPDATE attendance_notifications SET notification_id=$2 WHERE id=$1")
        .bind(source)
        .bind(receipt.1)
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 140).await.unwrap();
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(receipt.1)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert_current_runner_replay_preserves_notification_state(&pool).await;
}
#[tokio::test]
async fn inbox_first_process_work_eligibility_drift_blocks_contraction_and_repairs() {
    let pool = fixture("inbox_first_process_eligibility").await;
    let receipt = legacy(&pool, false, false, 1, false).await;
    let instance = Uuid::new_v4();
    let window = Uuid::new_v4();
    let item = Uuid::new_v4();
    let source = Uuid::new_v4();
    let assignee = Uuid::new_v4();
    sqlx::query("INSERT INTO process_instances(id,source_module,source_type,source_id,revision,definition,created_by) VALUES($1,'correspondence','synthetic',$1,1,'{}',$2)").bind(instance).bind(USER).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workflow_windows(id,module_code,workflow_code,title,managed_by_permission,status) VALUES($1,'correspondence','synthetic','Synthetic','process.manage.school','open')").bind(window).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_items(id,workflow_window_id,module_code,source_resource_type,title,action_path) VALUES($1,$2,'correspondence','synthetic','Synthetic','/staff/calendar')").bind(item).bind(window).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_item_assignees(id,work_item_id,assignee_type,user_id) VALUES($1,$2,'user',$3)").bind(assignee).bind(item).bind(USER).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO process_notification_sources(id,instance_id,recipient_id,work_item_id) VALUES($1,$2,$3,$4)").bind(source).bind(instance).bind(USER).bind(item).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE notification_dispatch_jobs SET source_kind='process',source_id=$2 WHERE id=$1",
    )
    .bind(receipt.0)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 139).await.unwrap();
    sqlx::query("UPDATE work_items SET status='closed' WHERE id=$1")
        .bind(item)
        .execute(&pool)
        .await
        .unwrap();
    assert!(apply_migrations_through(&pool, 140)
        .await
        .unwrap_err()
        .to_string()
        .contains("NOTIFICATION_RECEIPT_CONTRACTION_STALE_EVIDENCE"));
    sqlx::query("UPDATE work_items SET status='active' WHERE id=$1")
        .bind(item)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM work_item_assignees WHERE id=$1")
        .bind(assignee)
        .execute(&pool)
        .await
        .unwrap();
    assert!(apply_migrations_through(&pool, 140)
        .await
        .unwrap_err()
        .to_string()
        .contains("NOTIFICATION_RECEIPT_CONTRACTION_STALE_EVIDENCE"));
    sqlx::query("INSERT INTO work_item_assignees(id,work_item_id,assignee_type,user_id) VALUES($1,$2,'user',$3)").bind(assignee).bind(item).bind(USER).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 140).await.unwrap();
    assert_current_runner_replay_preserves_notification_state(&pool).await;
}

#[tokio::test]
async fn inbox_first_refuses_cross_source_business_key_duplicates_and_preserves_nullable_history() {
    let pool = fixture("inbox_first_key_conflicts").await;
    let first = legacy(&pool, false, true, 0, false).await;
    let second = legacy(&pool, false, true, 0, false).await;
    sqlx::query("UPDATE notification_dispatch_jobs SET source_kind='attendance',source_id=$2,dedup_key=$3 WHERE id=$1").bind(second.0).bind(Uuid::new_v4()).bind(format!("synthetic:{}",first.0)).execute(&pool).await.unwrap();
    assert!(apply_migrations_through(&pool, 139)
        .await
        .unwrap_err()
        .to_string()
        .contains("NOTIFICATION_INBOX_FIRST_IDEMPOTENCY_CONFLICT"));
    // Exact reviewed repair restores the original distinct business key; no row is merged/deleted.
    sqlx::query("UPDATE notification_dispatch_jobs SET dedup_key=$2 WHERE id=$1")
        .bind(second.0)
        .bind(format!("synthetic:{}", second.0))
        .execute(&pool)
        .await
        .unwrap();
    let unkeyed = legacy(&pool, true, true, 2, false).await;
    sqlx::query("UPDATE notification_dispatch_jobs SET dedup_key=NULL WHERE id=$1")
        .bind(unkeyed.0)
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 140).await.unwrap();
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT idempotency_key IS NULL FROM notification_receipts WHERE id=$1"
    )
    .bind(unkeyed.0)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}
