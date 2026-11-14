use super::*;
struct Eligible;
#[async_trait::async_trait]
impl DeliveryEligibilityPort for Eligible {
    async fn authorize_on_connection(
        &self,
        _connection: &mut PgConnection,
        _source: &NotificationSource,
        _recipient: Uuid,
        _notification: Uuid,
    ) -> Result<bool, AppError> {
        Ok(true)
    }
}
async fn fixture(name: &str) -> (PgPool, Uuid) {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 3).await;
    school_test_db::run_test_migrations(&pool).await;
    let user = school_test_db::create_test_user(
        &pool,
        &format!("{name}@example.test"),
        "Fixture-password-123",
    )
    .await
    .unwrap();
    (pool, user)
}
fn notice(recipient: Uuid) -> NewNotification {
    let id = Uuid::new_v4();
    NewNotification {
        id,
        notification_id: Uuid::new_v4(),
        recipient_id: recipient,
        source: NotificationSource::Attendance { source_id: id },
        idempotency_key: format!("attendance-ledger:{id}"),
        title: "Lesson result".into(),
        message: "Saved lesson fact".into(),
        type_: NotificationType::Info,
        link: Some("/student/attendance".into()),
        presentation: Presentation::AlertOnce,
    }
}
fn identity(notice: &NewNotification) -> PendingCancellationIdentity {
    PendingCancellationIdentity {
        receipt_id: notice.id,
        notification_id: notice.notification_id,
        recipient_id: notice.recipient_id,
        source: notice.source,
    }
}
#[tokio::test]
async fn pending_cancellation_retains_terminal_receipts_inboxes_and_prevents_prefetched_replay() {
    let (pool, user) = fixture("notification_pending_cascade").await;
    let items = vec![notice(user), notice(user), notice(user)];
    let mut tx = pool.begin().await.unwrap();
    create_in_transaction(&mut tx, &items).await.unwrap();
    tx.commit().await.unwrap();
    let prefetched = pending(&pool).await.unwrap();
    let first = *prefetched
        .iter()
        .find(|item| item.id == items[0].id)
        .unwrap();
    let terminal = *prefetched
        .iter()
        .find(|item| item.id == items[1].id)
        .unwrap();
    assert!(consume(&pool, &terminal, &Eligible)
        .await
        .unwrap()
        .is_some());
    let terminal_before: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(receipt) FROM notification_receipts receipt WHERE id=$1",
    )
    .bind(terminal.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let inbox_before: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(inbox) ORDER BY id) FROM notifications inbox WHERE id=ANY($1)",
    )
    .bind(
        items
            .iter()
            .map(|item| item.notification_id)
            .collect::<Vec<_>>(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        cancel_pending_in_transaction(&mut tx, &[identity(&items[0]), identity(&items[1])])
            .await
            .unwrap(),
        1
    );
    tx.commit().await.unwrap();
    assert!(consume(&pool, &first, &Eligible).await.unwrap().is_none());
    let terminal_after: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(receipt) FROM notification_receipts receipt WHERE id=$1",
    )
    .bind(terminal.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(terminal_before, terminal_after);
    let inbox_after: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(inbox) ORDER BY id) FROM notifications inbox WHERE id=ANY($1)",
    )
    .bind(
        items
            .iter()
            .map(|item| item.notification_id)
            .collect::<Vec<_>>(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inbox_before, inbox_after);
    let mut tx = pool.begin().await.unwrap();
    let replay = create_in_transaction(&mut tx, &items).await.unwrap();
    assert!(replay.iter().all(|outcome| !outcome.inserted));
    assert_eq!(
        cancel_pending_in_transaction(&mut tx, &[identity(&items[0])])
            .await
            .unwrap(),
        0
    );
    tx.commit().await.unwrap();
    let pending = pending(&pool).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, items[2].id);
}
#[tokio::test]
async fn cancellation_rejects_forged_source_recipient_or_notification_before_any_terminal_change() {
    let (pool, user) = fixture("notification_pending_identity").await;
    let item = notice(user);
    let mut tx = pool.begin().await.unwrap();
    create_in_transaction(&mut tx, std::slice::from_ref(&item))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut forged = vec![identity(&item); 3];
    forged[0].source = NotificationSource::Attendance {
        source_id: Uuid::new_v4(),
    };
    forged[1].recipient_id = Uuid::new_v4();
    forged[2].notification_id = Uuid::new_v4();
    for candidate in forged {
        let mut tx = pool.begin().await.unwrap();
        assert!(cancel_pending_in_transaction(&mut tx, &[candidate])
            .await
            .is_err());
        tx.rollback().await.unwrap();
    }
    assert_eq!(pending(&pool).await.unwrap().len(), 1);
    let consumed: bool = sqlx::query_scalar(
        "SELECT presentation_consumed_at IS NOT NULL FROM notification_receipts WHERE id=$1",
    )
    .bind(item.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!consumed);
}
