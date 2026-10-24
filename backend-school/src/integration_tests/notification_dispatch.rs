//! Actual central provider/worker regressions; all identities and databases are synthetic.
use crate::{
    adapters::notification_delivery::APPLICATION_NOTIFICATION_ELIGIBILITY as ELIGIBILITY,
    integration_tests::academic::core::services_tests::prepare_core_fixture_through,
};
use school_errors::AppError;
use school_notifications::{
    dispatch::{self, NotificationSource, NotificationSourceKind, QueuedNotification},
    publisher::{NotificationService, NotificationType, TenantNotificationPublisher},
};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, Uuid) {
    let latest = sqlx::migrate!("./migrations")
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap();
    let pool = prepare_core_fixture_through(name, latest).await;
    let recipient = sqlx::query_scalar(
        "SELECT id FROM users WHERE status='active' AND user_type='staff' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    (pool, recipient)
}
fn item(recipient_id: Uuid) -> QueuedNotification {
    QueuedNotification {
        id: Uuid::new_v4(),
        notification_id: Uuid::new_v4(),
        recipient_id,
        source: NotificationSource::Direct,
        dedup_key: None,
        title: "Synthetic notice".into(),
        message: "Synthetic body".into(),
        type_: NotificationType::Info,
        link: Some("/staff/calendar".into()),
    }
}
async fn enqueue(pool: &PgPool, items: &[QueuedNotification]) {
    let mut tx = pool.begin().await.unwrap();
    dispatch::enqueue_in_transaction(&mut tx, items)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}
pub(super) async fn drain(
    pool: &PgPool,
    tenant: &str,
    channel: &tokio::sync::broadcast::Sender<school_notifications::events::TenantNotificationEvent>,
) {
    for _ in 0..20 {
        let status = school_notifications::runtime::deliver_tenant_batch(
            pool,
            tenant,
            channel,
            &ELIGIBILITY,
        )
        .await
        .unwrap();
        if !status.due {
            assert!(
                !status.pending,
                "fixture must have no failed or future leased dispatch"
            );
            return;
        }
    }
    panic!("fixture dispatch exceeded its bounded expected backlog");
}

#[tokio::test]
async fn notification_dispatch_atomic_enqueue_and_immediate_send_preserve_inbox_visibility() {
    let (pool, recipient) = fixture("dispatch_atomic_enqueue").await;
    let queued = item(recipient);
    let mut tx = pool.begin().await.unwrap();
    dispatch::enqueue_in_transaction(&mut tx, std::slice::from_ref(&queued))
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert!(!dispatch::status(&pool).await.unwrap().pending);
    enqueue(&pool, std::slice::from_ref(&queued)).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let claimed = dispatch::claim(&pool).await.unwrap().remove(0);
    let first = dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE notifications SET read_at=now() WHERE id=$1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let replay = dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.id, replay.id);
    assert_eq!(first.created_at, replay.created_at);
    assert!(replay.read_at.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    dispatch::acknowledge(&pool, &claimed).await.unwrap();
    let (channel, mut events) = tokio::sync::broadcast::channel(8);
    let publisher = TenantNotificationPublisher::new("dispatch-fixture", &channel);
    let id = NotificationService::send(
        &pool,
        &publisher,
        recipient,
        "Immediate",
        "Body",
        NotificationType::Success,
        None,
    )
    .await
    .unwrap();
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert!(
        events.try_recv().is_err(),
        "transport must wait for the central worker"
    );
    drain(&pool, "dispatch-fixture", &channel).await;
    let event = events.try_recv().unwrap();
    assert_eq!(event.notification.id, id);
    assert_eq!(event.tenant, "dispatch-fixture");
    assert_eq!(event.user_id, recipient);
    assert_eq!(id.get_version_num(), 7);
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn notification_dispatch_direct_selects_active_once_and_retries_original_recipient() {
    let (pool, recipient) = fixture("dispatch_direct_snapshot").await;
    let (channel, mut events) = tokio::sync::broadcast::channel(8);
    let publisher = TenantNotificationPublisher::new("direct-snapshot", &channel);
    let id = NotificationService::send(
        &pool,
        &publisher,
        recipient,
        "Snapshot",
        "Body",
        NotificationType::Info,
        None,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE notifications SET read_at=now() WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let before: (
        Option<chrono::DateTime<chrono::Utc>>,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as("SELECT read_at,created_at FROM notifications WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(recipient)
        .execute(&pool)
        .await
        .unwrap();
    for target in [recipient, Uuid::new_v4()] {
        assert!(matches!(
            NotificationService::send(
                &pool,
                &publisher,
                target,
                "Rejected",
                "Body",
                NotificationType::Info,
                None
            )
            .await,
            Err(sqlx::Error::RowNotFound)
        ));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_dispatch_jobs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let first = dispatch::claim(&pool).await.unwrap().remove(0);
    let notice = dispatch::materialize(&pool, &first, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    NotificationService::deliver_stored(&pool, &publisher, recipient, notice)
        .await
        .unwrap();
    // Simulate interruption after external delivery but before acknowledgement.
    sqlx::query("UPDATE notification_dispatch_jobs SET next_attempt_at=now()-interval '1 second' WHERE id=$1").bind(first.notification.id).execute(&pool).await.unwrap();
    drain(&pool, "direct-snapshot", &channel).await;
    assert_eq!(events.try_recv().unwrap().notification.id, id);
    assert_eq!(events.try_recv().unwrap().notification.id, id);
    assert!(events.try_recv().is_err());
    let after: (
        Option<chrono::DateTime<chrono::Utc>>,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as("SELECT read_at,created_at FROM notifications WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT attempts FROM notification_dispatch_jobs WHERE notification_id=$1"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
}

#[tokio::test]
async fn notification_dispatch_claims_are_disjoint_and_expired_tokens_cannot_store_or_ack() {
    let (pool, recipient) = fixture("dispatch_fenced_claims").await;
    let items: Vec<_> = (0..35).map(|_| item(recipient)).collect();
    enqueue(&pool, &items).await;
    let (a, b) = tokio::join!(dispatch::claim(&pool), dispatch::claim(&pool));
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(!a.is_empty() && !b.is_empty());
    assert!(a.len() <= 32 && b.len() <= 32);
    let ids: std::collections::HashSet<_> = a.iter().chain(&b).map(|n| n.notification.id).collect();
    assert_eq!(ids.len(), 35);
    let status = dispatch::status(&pool).await.unwrap();
    assert!(status.pending && !status.due);
    assert!(dispatch::claim(&pool).await.unwrap().is_empty());
    let old = &a[0];
    sqlx::query("UPDATE notification_dispatch_jobs SET next_attempt_at=now()-interval '1 second' WHERE id=$1").bind(old.notification.id).execute(&pool).await.unwrap();
    let fresh = dispatch::claim(&pool).await.unwrap().remove(0);
    assert_ne!(old.lease_token, fresh.lease_token);
    assert_eq!(
        old.notification.notification_id,
        fresh.notification.notification_id
    );
    assert!(dispatch::materialize(&pool, old, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    dispatch::acknowledge(&pool, old).await.unwrap();
    let status = dispatch::progress_on_connection(
        &mut pool.acquire().await.unwrap(),
        &[old.notification.notification_id],
    )
    .await
    .unwrap();
    assert!(status[0].stored_at.is_none() && status[0].published_at.is_none());
    assert!(dispatch::materialize(&pool, &fresh, &ELIGIBILITY)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT attempts FROM notification_dispatch_jobs WHERE id=$1")
            .bind(old.notification.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn notification_dispatch_snapshot_recipients_remain_claimable_in_bounded_batches_after_deactivation(
) {
    let (pool, recipient) = fixture("dispatch_inactive_snapshot").await;
    enqueue(&pool, &(0..35).map(|_| item(recipient)).collect::<Vec<_>>()).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(recipient)
        .execute(&pool)
        .await
        .unwrap();
    for expected in [32, 3] {
        let batch = dispatch::claim(&pool).await.unwrap();
        assert_eq!(batch.len(), expected);
        for claimed in batch {
            assert!(dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
                .await
                .unwrap()
                .is_some());
            dispatch::acknowledge(&pool, &claimed).await.unwrap();
        }
    }
    assert!(!dispatch::status(&pool).await.unwrap().pending);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        35
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notification_dispatch_jobs WHERE published_at IS NOT NULL"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        35
    );
}

#[tokio::test]
async fn notification_dispatch_identity_conflicts_and_missing_stored_inbox_fail_closed() {
    let (pool, recipient) = fixture("dispatch_identity_privacy").await;
    let original = item(recipient);
    enqueue(&pool, std::slice::from_ref(&original)).await;
    let mut conflict = original.clone();
    conflict.message = "Different identity payload".into();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        dispatch::enqueue_in_transaction(&mut tx, &[conflict]).await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let claimed = dispatch::claim(&pool).await.unwrap().remove(0);
    dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE notifications SET message='unexpected drift' WHERE id=$1")
        .bind(original.notification_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        dispatch::materialize(&pool, &claimed, &ELIGIBILITY).await,
        Err(AppError::Conflict(_))
    ));
    sqlx::query("DELETE FROM notifications WHERE id=$1")
        .bind(original.notification_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    assert!(!dispatch::status(&pool).await.unwrap().pending);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn notification_dispatch_row_failure_keeps_retry_while_other_claimed_siblings_finish() {
    let (pool, recipient) = fixture("dispatch_independent_failures").await;
    let items: Vec<_> = (0..35).map(|_| item(recipient)).collect();
    enqueue(&pool, &items).await;
    let failed = items[0].notification_id;
    // Only a generated UUID is formatted; fixture SQL contains no caller input.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION reject_one_notice() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='{failed}'::uuid THEN RAISE EXCEPTION 'synthetic_notice_failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_one_notice BEFORE INSERT ON notifications FOR EACH ROW EXECUTE FUNCTION reject_one_notice();")))
        .execute(&pool).await.unwrap();
    let (channel, _) = tokio::sync::broadcast::channel(100);
    for _ in 0..2 {
        school_notifications::runtime::deliver_tenant_batch(
            &pool,
            "dispatch-failure",
            &channel,
            &ELIGIBILITY,
        )
        .await
        .unwrap();
    }
    let status = dispatch::status(&pool).await.unwrap();
    assert!(status.pending && !status.due);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notification_dispatch_jobs WHERE published_at IS NOT NULL"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        34
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        34
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT stored_at IS NULL AND published_at IS NULL AND attempts=1 FROM notification_dispatch_jobs WHERE notification_id=$1").bind(failed).fetch_one(&pool).await.unwrap());
    sqlx::raw_sql(
        "DROP TRIGGER reject_one_notice ON notifications; DROP FUNCTION reject_one_notice();",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE notification_dispatch_jobs SET next_attempt_at=now()-interval '1 second' WHERE notification_id=$1").bind(failed).execute(&pool).await.unwrap();
    drain(&pool, "dispatch-failure", &channel).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        35
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT attempts FROM notification_dispatch_jobs WHERE notification_id=$1"
        )
        .bind(failed)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
}

#[tokio::test]
async fn notification_dispatch_owned_source_cancel_prevents_prefetched_recreation() {
    let (pool, recipient) = fixture("dispatch_source_cancellation").await;
    let student: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='student' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let term: Uuid = sqlx::query_scalar("SELECT id FROM academic_terms ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let parent = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'not-a-login','Synthetic','Parent','parent','active')").bind(parent).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_parents(student_user_id,parent_user_id,relationship) VALUES($1,$2,'parent')").bind(student).bind(parent).execute(&pool).await.unwrap();
    let mut owned = item(parent);
    owned.source = NotificationSource::Attendance {
        source_id: owned.id,
    };
    owned.dedup_key = Some("source-cancel-fixture".into());
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO attendance_notifications(id,academic_term_id,recipient_id,student_id,dedup_key,notification_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(owned.id).bind(term).bind(parent).bind(student).bind(&owned.dedup_key).bind(owned.notification_id).execute(&mut *tx).await.unwrap();
    dispatch::enqueue_in_transaction(&mut tx, std::slice::from_ref(&owned))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let claimed = dispatch::claim(&pool).await.unwrap().remove(0);
    assert!(dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
        .await
        .unwrap()
        .is_some());
    let direct = item(recipient);
    enqueue(&pool, std::slice::from_ref(&direct)).await;
    let direct_claim = dispatch::claim(&pool).await.unwrap().remove(0);
    dispatch::materialize(&pool, &direct_claim, &ELIGIBILITY)
        .await
        .unwrap();
    // A fresh eligible item prefetched before reset must wait for the domain guard,
    // then observe committed cancellation rather than recreate its inbox.
    let mut prefetched = item(student);
    prefetched.source = NotificationSource::Attendance {
        source_id: prefetched.id,
    };
    prefetched.dedup_key = Some("prefetched-reset-fixture".into());
    let mut setup = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO attendance_notifications(id,academic_term_id,recipient_id,student_id,dedup_key,notification_id) VALUES($1,$2,$3,$3,$4,$5)")
        .bind(prefetched.id).bind(term).bind(student).bind(&prefetched.dedup_key).bind(prefetched.notification_id).execute(&mut *setup).await.unwrap();
    dispatch::enqueue_in_transaction(&mut setup, std::slice::from_ref(&prefetched))
        .await
        .unwrap();
    setup.commit().await.unwrap();
    let prefetched_claim = dispatch::claim(&pool).await.unwrap().remove(0);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(17010000)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let waiting = tokio::spawn(async move {
        dispatch::materialize(&worker_pool, &prefetched_claim, &ELIGIBILITY).await
    });
    wait_for_blocker(&pool, blocker).await;
    dispatch::cancel_identified_in_transaction(
        &mut tx,
        NotificationSourceKind::Attendance,
        &[
            owned.notification_id,
            prefetched.notification_id,
            direct.notification_id,
        ],
        true,
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM attendance_notifications WHERE id=ANY($1)")
        .bind(vec![owned.id, prefetched.id])
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(waiting.await.unwrap().unwrap().is_none());
    assert!(dispatch::materialize(&pool, &claimed, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(direct.notification_id)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(owned.notification_id)
    .fetch_one(&pool)
    .await
    .unwrap());
}

async fn wait_for_blocker(pool: &PgPool, blocker: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker)
            .fetch_one(pool)
            .await
            .unwrap()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("third bounded pass must self-wake promptly");
}
#[tokio::test]
async fn notification_dispatch_due_backlog_revisits_other_tenants_before_next_batch() {
    use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
    use std::{collections::HashMap, sync::Arc};
    struct AbortOnDrop(tokio::task::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let (first, first_recipient) = fixture("dispatch_fair_queue_first").await;
    let (second, second_recipient) = fixture("dispatch_fair_queue_second").await;
    let (recovering, recovering_recipient) = fixture("dispatch_retry_admission").await;
    enqueue(&recovering, &[item(recovering_recipient)]).await;
    let lock_key = i64::from_le_bytes(*Uuid::new_v4().as_bytes().first_chunk::<8>().unwrap());
    let mut blocker = first.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut directory = HashMap::new();
    let mut active = Vec::new();
    let failing_tenant = format!("a-failing-{}", Uuid::new_v4().simple());
    active.push(serde_json::json!({"subdomain":failing_tenant}));
    let failed_admissions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let admission_observer = failed_admissions.clone();
    let recovery_ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let recovery_control = recovery_ready.clone();
    let recovery_schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&recovering)
        .await
        .unwrap();
    let mut recovery_url =
        reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    recovery_url.query_pairs_mut().append_pair(
        "options",
        &format!("-csearch_path={recovery_schema},public"),
    );
    directory.insert(failing_tenant.clone(),serde_json::json!({"id":Uuid::new_v4(),"name":"fixture","db_connection_string":recovery_url.as_str()}));
    for (f, recipient, name) in [
        (&first, first_recipient, "queue-first"),
        (&second, second_recipient, "queue-second"),
    ] {
        enqueue(f, &(0..97).map(|_| item(recipient)).collect::<Vec<_>>()).await;
        // Two complete passes are permitted; the third must pause. A worker that
        // drains one tenant before visiting the other leaves that other at zero.
        // The formatted advisory key is a generated i64, never caller SQL.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION block_later_notice_batch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.published_at IS NOT NULL AND OLD.published_at IS NULL AND (SELECT count(*) FROM notification_dispatch_jobs WHERE published_at IS NOT NULL)>=64 THEN PERFORM pg_advisory_xact_lock({lock_key}); END IF; RETURN NEW; END $$; CREATE TRIGGER block_later_notice_batch BEFORE UPDATE ON notification_dispatch_jobs FOR EACH ROW EXECUTE FUNCTION block_later_notice_batch();")))
            .execute(&*f).await.unwrap();
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&*f)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let subdomain = format!("{name}-{}", Uuid::new_v4().simple());
        directory.insert(subdomain.clone(), serde_json::json!({"id":Uuid::new_v4(),"name":"fixture","db_connection_string":url.as_str()}));
        active.push(serde_json::json!({"subdomain":subdomain,"db_connection_string":url.as_str()}));
    }
    let directory = Arc::new(directory);
    let app = Router::new()
        .route(
            "/internal/schools",
            get(move || {
                let schools = active.clone();
                async move { Json(serde_json::json!({"schools":schools})) }
            }),
        )
        .route(
            "/internal/schools/{tenant}",
            get(move |Path(tenant): Path<String>| {
                let directory = directory.clone();
                let failing_tenant = failing_tenant.clone();
                let failed_admissions = failed_admissions.clone();
                let recovery_ready = recovery_ready.clone();
                async move {
                    if tenant == failing_tenant {
                        failed_admissions.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if !recovery_ready.load(std::sync::atomic::Ordering::SeqCst) {
                            return Err(StatusCode::NOT_FOUND);
                        }
                    }
                    directory
                        .get(&tenant)
                        .cloned()
                        .map(Json)
                        .ok_or(StatusCode::NOT_FOUND)
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut _server = AbortOnDrop(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let (channel, _) = tokio::sync::broadcast::channel(100);
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: Arc::new(school_tenancy::AdminClient::new(
            format!("http://{address}"),
            "fixture-internal".into(),
            school_tenancy::AdminClientConfig::from_env().unwrap(),
        )),
        pool_manager: Arc::new(school_tenancy::PoolManager::new()),
        notification_channel: channel,
        eligibility: &ELIGIBILITY,
    };
    let mut _worker = AbortOnDrop(tokio::spawn(school_notifications::runtime::start(state)));
    // Startup contributes at most one Notify permit and the initial interval tick.
    // Reaching the third batch promptly also proves due-work self-waking.
    wait_for_blocker(&first, blocker_pid).await;
    assert_eq!(
        admission_observer.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "failed admission must defer while other tenants have immediate backlog"
    );
    for f in [&first, &second] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM notification_dispatch_jobs WHERE published_at IS NOT NULL"
            )
            .fetch_one(&*f)
            .await
            .unwrap(),
            64,
            "each tenant must receive one bounded batch per pass"
        );
    }
    blocker.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if !dispatch::status(&first).await.unwrap().pending
                && !dispatch::status(&second).await.unwrap().pending
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("due backlog must drain without waiting for the 15-second timer");
    recovery_control.store(true, std::sync::atomic::Ordering::SeqCst);
    // No new wake hint: a failed startup candidate must remain scheduled for a
    // regular retry, independently of the300-second directory refresh.
    tokio::time::timeout(std::time::Duration::from_secs(35), async {
        while dispatch::status(&recovering).await.unwrap().pending {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("failed tenant admission must recover on regular retry");
    _worker.0.abort();
    let _ = (&mut _worker.0).await;
    _server.0.abort();
    let _ = (&mut _server.0).await;
    first.close().await;
    second.close().await;
    recovering.close().await;
}

struct ControlledBatchTransport {
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
    endpoint: String,
}
#[async_trait::async_trait]
impl school_notifications::runtime::test_support::ControlledTransport for ControlledBatchTransport {
    async fn deliver(
        &self,
        _notification_id: Uuid,
    ) -> Result<school_notifications::runtime::test_support::DeliveryResult, &'static str> {
        self.started.notify_one();
        self.release.notified().await;
        Ok(
            school_notifications::runtime::test_support::DeliveryResult {
                invalid_endpoints: vec![self.endpoint.clone()],
                retryable: false,
            },
        )
    }
}

async fn managed_batch_phase_case(name: &str, remap: bool) {
    use axum::{routing::get, Json, Router};
    use school_tenancy::{
        background::TenantJobKind, AdminClient, AdminClientConfig, PoolManager, PoolManagerConfig,
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    struct Abort(tokio::task::JoinHandle<()>);
    impl Drop for Abort {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let (fixture, recipient) = fixture(name).await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let endpoint = "https://synthetic.invalid/device";
    sqlx::query("INSERT INTO push_subscriptions(user_id,endpoint,p256dh_key,auth_key) VALUES($1,$2,'synthetic','synthetic')")
        .bind(recipient).bind(endpoint).execute(&fixture).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&fixture)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema},public"));
    let directory_id = Arc::new(Mutex::new(Uuid::new_v4()));
    let response_id = directory_id.clone();
    let response_url = url.to_string();
    let app = Router::new().route("/internal/schools/{tenant}", get(move || {
        let id = *response_id.lock().unwrap();
        let database_url = response_url.clone();
        async move { Json(serde_json::json!({"id":id,"name":"synthetic","db_connection_string":database_url})) }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _server = Abort(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    let observed = manager.get_pool(url.as_str(), name).await.unwrap();
    let observed_raw = observed.as_ref().clone();
    drop(observed);
    let (channel, _) = tokio::sync::broadcast::channel(8);
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: Arc::new(AdminClient::new(
            format!("http://{address}"),
            "synthetic".into(),
            AdminClientConfig::for_tests(Duration::from_secs(1), 1, Duration::from_millis(1)),
        )),
        pool_manager: manager.clone(),
        notification_channel: channel,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        endpoint: endpoint.into(),
    });
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let tenant = name.to_string();
    let worker = tokio::spawn(async move {
        school_notifications::runtime::test_support::deliver_managed_batch(
            &worker_state,
            &tenant,
            worker_transport.as_ref(),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), transport.started.notified())
        .await
        .unwrap();
    let turns = manager.background_turns();
    for kind in [
        TenantJobKind::CalendarReminders,
        TenantJobKind::AuthRetention,
        TenantJobKind::AttendanceMaintenance,
        TenantJobKind::FileReconciliation,
    ] {
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(200),
                turns.run("healthy", kind, || async { 1 })
            )
            .await
            .unwrap(),
            Some(1)
        );
    }
    assert!(turns
        .begin(name, TenantJobKind::NotificationDispatch)
        .is_none());
    let mut second_url = url.clone();
    second_url
        .query_pairs_mut()
        .append_pair("application_name", "phase-second");
    let second = manager
        .get_pool(second_url.as_str(), "phase-second")
        .await
        .unwrap();
    let mut third_url = url.clone();
    third_url
        .query_pairs_mut()
        .append_pair("application_name", "phase-third");
    let third = tokio::time::timeout(
        Duration::from_secs(1),
        manager.get_pool(third_url.as_str(), "phase-third"),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        observed_raw.is_closed(),
        "actual notification DB lease must end before blocked transport"
    );
    assert!(!second.is_closed() && !third.is_closed());
    drop(second);
    drop(third);
    if remap {
        let token = Uuid::new_v4();
        state
            .admin_client
            .begin_directory_mutation(name, token)
            .unwrap();
        *directory_id.lock().unwrap() = Uuid::new_v4();
        state
            .admin_client
            .finish_directory_mutation(name, token)
            .unwrap();
    }
    transport.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
    if remap {
        assert!(result.is_err());
    } else {
        assert!(!result.unwrap().pending);
    }
    let present: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM push_subscriptions WHERE endpoint=$1)")
            .bind(endpoint)
            .fetch_one(&fixture)
            .await
            .unwrap();
    let published: bool = sqlx::query_scalar(
        "SELECT published_at IS NOT NULL FROM notification_dispatch_jobs WHERE id=$1",
    )
    .bind(queued.id)
    .fetch_one(&fixture)
    .await
    .unwrap();
    assert_eq!(
        present, remap,
        "remapped finish must not delete an endpoint"
    );
    assert_eq!(
        published, !remap,
        "remapped finish must not acknowledge the original claim"
    );
}

#[tokio::test]
async fn notification_managed_batch_releases_real_pool_lease_and_shared_permit_during_transport() {
    managed_batch_phase_case("dispatch_managed_phase_release", false).await;
}

#[tokio::test]
async fn notification_managed_batch_uuid_remap_refuses_endpoint_cleanup_and_acknowledgment() {
    managed_batch_phase_case("dispatch_managed_uuid_remap", true).await;
}

#[tokio::test]
async fn notification_managed_slow_admin_http_allows_actual_other_kind_sql_and_cancels_without_claim(
) {
    use crate::integration_tests::tenant_directory::{database_url, DirectoryServer};
    use school_tenancy::{background::TenantJobKind, PoolManager, PoolManagerConfig};
    use std::{sync::Arc, time::Duration};
    let (fixture, recipient) = fixture("dispatch_slow_admin").await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let url = database_url(&fixture).await;
    let directory = DirectoryServer::start(Uuid::new_v4(), url.to_string(), true).await;
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    assert_eq!(manager.background_turns().limit(), 1);
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: directory.client(2, Duration::from_secs(30)),
        pool_manager: manager.clone(),
        notification_channel: tokio::sync::broadcast::channel(8).0,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        endpoint: "synthetic".into(),
    });
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let worker = tokio::spawn(async move {
        school_notifications::runtime::test_support::deliver_managed_batch(
            &worker_state,
            "dispatch_slow_admin",
            worker_transport.as_ref(),
        )
        .await
    });
    directory.wait_for_request().await;
    let turns = manager.background_turns();
    let progress = tokio::time::timeout(
        Duration::from_secs(1),
        turns.run("healthy", TenantJobKind::AttendanceMaintenance, || async {
            let pool = manager
                .get_pool_without_migrations(url.as_str(), "healthy")
                .await
                .unwrap();
            let mut connection = pool
                .checkout("fixture.slow_admin_other_kind")
                .await
                .unwrap();
            school_database::DbSession::new(&mut connection)
                .scalar::<i32>("fixture.slow_admin_other_kind", "SELECT 1")
                .fetch_one()
                .await
                .unwrap()
        }),
    )
    .await
    .unwrap();
    assert_eq!(progress, Some(1));
    assert!(!worker.is_finished());
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    directory.unblock();
    assert!(turns
        .begin("dispatch_slow_admin", TenantJobKind::NotificationDispatch)
        .is_some());
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM notification_dispatch_jobs WHERE id=$1")
            .bind(queued.id)
            .fetch_one(&fixture)
            .await
            .unwrap();
    assert_eq!(attempts, 0);
    assert_eq!(directory.calls(), 1);
}

#[derive(Clone, Copy)]
enum QueuedDirectoryChange {
    Fence,
    Rotation,
    Expiry,
    Eviction,
}

async fn notification_managed_queued_finish_case(name: &str, change: QueuedDirectoryChange) {
    use crate::integration_tests::tenant_directory::{database_url, DirectoryServer, PhaseSignals};
    use school_tenancy::{background::TenantJobKind, PoolManager, PoolManagerConfig};
    use std::{sync::Arc, time::Duration};
    use tracing::instrument::WithSubscriber;
    let (fixture, recipient) = fixture(name).await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let endpoint = "https://synthetic.invalid/queued-directory";
    sqlx::query("INSERT INTO push_subscriptions(user_id,endpoint,p256dh_key,auth_key) VALUES($1,$2,'synthetic','synthetic')").bind(recipient).bind(endpoint).execute(&fixture).await.unwrap();
    let url = database_url(&fixture).await;
    let tenant_id = Uuid::new_v4();
    let directory = DirectoryServer::start(tenant_id, url.to_string(), false).await;
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    let ttl = if matches!(change, QueuedDirectoryChange::Expiry) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(30)
    };
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: directory.client(1, ttl),
        pool_manager: manager.clone(),
        notification_channel: tokio::sync::broadcast::channel(8).0,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        endpoint: endpoint.into(),
    });
    let (signals, dispatcher) = PhaseSignals::for_phase("finish");
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let tenant = name.to_string();
    let worker = tokio::spawn(
        async move {
            school_notifications::runtime::test_support::deliver_managed_batch(
                &worker_state,
                &tenant,
                worker_transport.as_ref(),
            )
            .await
        }
        .with_subscriber(dispatcher),
    );
    tokio::time::timeout(Duration::from_secs(3), transport.started.notified())
        .await
        .unwrap();
    let turns = manager.background_turns();
    assert_eq!(turns.limit(), 1);
    let blocker = turns
        .begin("hold-for-finish", TenantJobKind::AuthRetention)
        .unwrap();
    let mut held = Box::pin(blocker.run(|| std::future::pending::<()>()));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(held.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    transport.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), signals.resolved.notified())
        .await
        .unwrap();
    let calls = directory.calls();
    let extra_requests = match change {
        QueuedDirectoryChange::Fence => {
            state
                .admin_client
                .begin_directory_mutation(name, Uuid::new_v4())
                .unwrap();
            0
        }
        QueuedDirectoryChange::Rotation => {
            let token = Uuid::new_v4();
            state
                .admin_client
                .begin_directory_mutation(name, token)
                .unwrap();
            let mut rotated = url.clone();
            rotated
                .set_password(Some("synthetic-rotated-credential"))
                .unwrap();
            directory.set_mapping(tenant_id, rotated.to_string());
            state
                .admin_client
                .finish_directory_mutation(name, token)
                .unwrap();
            state
                .admin_client
                .resolve_database_work(name)
                .await
                .unwrap();
            1
        }
        QueuedDirectoryChange::Expiry => {
            tokio::time::sleep(Duration::from_millis(1100)).await;
            1
        }
        QueuedDirectoryChange::Eviction => {
            state
                .admin_client
                .resolve_database_work("evicting-tenant")
                .await
                .unwrap();
            2
        }
    };
    assert!(!worker.is_finished());
    drop(held);
    let result = tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
    if let Ok(status) = result {
        assert!(status.pending);
    }
    let (endpoint_present, published): (bool, bool) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM push_subscriptions WHERE endpoint=$1),(SELECT published_at IS NOT NULL FROM notification_dispatch_jobs WHERE id=$2)").bind(endpoint).bind(queued.id).fetch_one(&fixture).await.unwrap();
    assert!(
        endpoint_present,
        "invalidated proof must not delete an endpoint"
    );
    assert!(!published, "invalidated proof must not acknowledge a claim");
    assert_eq!(directory.calls(), calls + extra_requests, "finish cannot refetch inside admission; only explicit rotation/eviction and later status resolution fetch");
}

#[tokio::test]
async fn notification_managed_queued_finish_fence_refuses_cleanup_and_ack_without_http_retry() {
    notification_managed_queued_finish_case("dispatch_queued_fence", QueuedDirectoryChange::Fence)
        .await;
}
#[tokio::test]
async fn notification_managed_queued_finish_same_uuid_credential_rotation_refuses_cleanup_and_ack()
{
    notification_managed_queued_finish_case(
        "dispatch_queued_rotation",
        QueuedDirectoryChange::Rotation,
    )
    .await;
}
#[tokio::test]
async fn notification_managed_queued_finish_expired_snapshot_refuses_cleanup_and_ack() {
    notification_managed_queued_finish_case(
        "dispatch_queued_expiry",
        QueuedDirectoryChange::Expiry,
    )
    .await;
}
#[tokio::test]
async fn notification_managed_queued_finish_evicted_snapshot_refuses_cleanup_and_ack() {
    notification_managed_queued_finish_case(
        "dispatch_queued_eviction",
        QueuedDirectoryChange::Eviction,
    )
    .await;
}

#[tokio::test]
async fn notification_managed_physical_pool_wait_rechecks_same_uuid_rotated_credentials_before_claim(
) {
    use crate::integration_tests::tenant_directory::{database_url, DirectoryServer, PhaseSignals};
    use school_tenancy::{PoolManager, PoolManagerConfig};
    use std::{sync::Arc, time::Duration};
    use tracing::instrument::WithSubscriber;
    let (fixture, recipient) = fixture("dispatch_physical_rotation").await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let url = database_url(&fixture).await;
    let tenant_id = Uuid::new_v4();
    let directory = DirectoryServer::start(tenant_id, url.to_string(), false).await;
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    let mut first_url = url.clone();
    first_url
        .query_pairs_mut()
        .append_pair("application_name", "occupied-one");
    let first = manager
        .get_pool_without_migrations(first_url.as_str(), "occupied-one")
        .await
        .unwrap();
    let mut second_url = url.clone();
    second_url
        .query_pairs_mut()
        .append_pair("application_name", "occupied-two");
    let second = manager
        .get_pool_without_migrations(second_url.as_str(), "occupied-two")
        .await
        .unwrap();
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: directory.client(2, Duration::from_secs(30)),
        pool_manager: manager,
        notification_channel: tokio::sync::broadcast::channel(8).0,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        endpoint: "synthetic".into(),
    });
    let (signals, dispatcher) = PhaseSignals::for_phase("claim");
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let worker = tokio::spawn(
        async move {
            school_notifications::runtime::test_support::deliver_managed_batch(
                &worker_state,
                "dispatch_physical_rotation",
                worker_transport.as_ref(),
            )
            .await
        }
        .with_subscriber(dispatcher),
    );
    tokio::time::timeout(Duration::from_secs(1), signals.physical_wait.notified())
        .await
        .unwrap();
    let token = Uuid::new_v4();
    state
        .admin_client
        .begin_directory_mutation("dispatch_physical_rotation", token)
        .unwrap();
    let mut rotated = url.clone();
    rotated
        .set_password(Some("synthetic-new-password"))
        .unwrap();
    directory.set_mapping(tenant_id, rotated.to_string());
    state
        .admin_client
        .finish_directory_mutation("dispatch_physical_rotation", token)
        .unwrap();
    state
        .admin_client
        .resolve_database_work("dispatch_physical_rotation")
        .await
        .unwrap();
    drop(first);
    assert!(tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    let (attempts, published): (i32, bool) = sqlx::query_as(
        "SELECT attempts,published_at IS NOT NULL FROM notification_dispatch_jobs WHERE id=$1",
    )
    .bind(queued.id)
    .fetch_one(&fixture)
    .await
    .unwrap();
    assert_eq!(
        attempts, 0,
        "second local check must reject before business claim SQL"
    );
    assert!(!published);
    assert_eq!(directory.calls(), 2);
    drop(second);
}
