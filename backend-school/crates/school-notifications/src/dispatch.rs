//! Transactional notification storage and fenced durable delivery work.
use crate::{models::Notification, publisher::NotificationType};
use chrono::{DateTime, Utc};
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const DELIVERY_BATCH_SIZE: i64 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationSource {
    Direct,
    Attendance { source_id: Uuid },
    Process { source_id: Uuid },
}
#[derive(Clone, Copy)]
pub enum NotificationSourceKind {
    Direct,
    Attendance,
    Process,
}
impl NotificationSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Attendance => "attendance",
            Self::Process => "process",
        }
    }
}
impl NotificationSource {
    fn parts(self) -> (&'static str, Option<Uuid>) {
        match self {
            Self::Direct => ("direct", None),
            Self::Attendance { source_id } => ("attendance", Some(source_id)),
            Self::Process { source_id } => ("process", Some(source_id)),
        }
    }
}
#[derive(Clone, Debug)]
pub struct QueuedNotification {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub recipient_id: Uuid,
    pub source: NotificationSource,
    pub dedup_key: Option<String>,
    pub title: String,
    pub message: String,
    pub type_: NotificationType,
    pub link: Option<String>,
}
pub struct EnqueueOutcome {
    pub input_id: Uuid,
    pub queue_id: Uuid,
    pub notification_id: Uuid,
    pub source: NotificationSource,
    pub inserted: bool,
}
#[derive(Clone)]
pub struct ClaimedNotification {
    pub notification: QueuedNotification,
    pub lease_token: Uuid,
}
#[derive(sqlx::FromRow)]
pub struct NotificationProgress {
    pub notification_id: Uuid,
    pub stored_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
}
#[async_trait::async_trait]
pub trait DeliveryEligibilityPort: Send + Sync {
    /// Validate structural source identity under its guards BEFORE the job lock.
    /// Recipient activity/relationships were resolved by the producer and are never rechecked here.
    async fn authorize_on_connection(
        &self,
        connection: &mut PgConnection,
        source: &NotificationSource,
        recipient_id: Uuid,
    ) -> Result<bool, AppError>;
}
#[derive(sqlx::FromRow)]
struct Job {
    id: Uuid,
    notification_id: Uuid,
    recipient_id: Uuid,
    source_kind: String,
    source_id: Option<Uuid>,
    dedup_key: Option<String>,
    title: String,
    message: String,
    type_: String,
    link: Option<String>,
    stored_at: Option<DateTime<Utc>>,
    lease_token: Option<Uuid>,
}
#[derive(serde::Serialize)]
struct InputJob<'a> {
    id: Uuid,
    notification_id: Uuid,
    recipient_id: Uuid,
    source_kind: &'static str,
    source_id: Option<Uuid>,
    dedup_key: &'a Option<String>,
    title: &'a str,
    message: &'a str,
    #[serde(rename = "type")]
    type_: &'static str,
    link: &'a Option<String>,
}
fn integrity(code: &'static str) -> AppError {
    AppError::Conflict(code.into())
}
impl Job {
    fn notification(&self) -> Result<QueuedNotification, AppError> {
        let source = match (self.source_kind.as_str(), self.source_id) {
            ("direct", None) => NotificationSource::Direct,
            ("attendance", Some(source_id)) => NotificationSource::Attendance { source_id },
            ("process", Some(source_id)) => NotificationSource::Process { source_id },
            _ => return Err(integrity("notification_source_unsupported")),
        };
        let type_ = match self.type_.as_str() {
            "info" => NotificationType::Info,
            "success" => NotificationType::Success,
            "warning" => NotificationType::Warning,
            "error" => NotificationType::Error,
            _ => return Err(integrity("notification_type_unsupported")),
        };
        Ok(QueuedNotification {
            id: self.id,
            notification_id: self.notification_id,
            recipient_id: self.recipient_id,
            source,
            dedup_key: self.dedup_key.clone(),
            title: self.title.clone(),
            message: self.message.clone(),
            type_,
            link: self.link.clone(),
        })
    }
}
fn same_identity(a: &QueuedNotification, b: &QueuedNotification) -> bool {
    a.id == b.id
        && a.notification_id == b.notification_id
        && a.recipient_id == b.recipient_id
        && a.source == b.source
        && a.dedup_key == b.dedup_key
        && a.title == b.title
        && a.message == b.message
        && a.type_ == b.type_
        && a.link == b.link
}
struct JobIndex<'a> {
    by_id: HashMap<Uuid, &'a Job>,
    by_notification: HashMap<Uuid, &'a Job>,
    by_business: HashMap<(String, Uuid, String), &'a Job>,
}
impl<'a> JobIndex<'a> {
    fn new(rows: &'a [Job]) -> Self {
        let mut index = Self {
            by_id: HashMap::with_capacity(rows.len()),
            by_notification: HashMap::with_capacity(rows.len()),
            by_business: HashMap::with_capacity(rows.len()),
        };
        for row in rows {
            index.by_id.insert(row.id, row);
            index.by_notification.insert(row.notification_id, row);
            if let Some(key) = &row.dedup_key {
                index.by_business.insert(
                    (row.source_kind.clone(), row.recipient_id, key.clone()),
                    row,
                );
            }
        }
        index
    }
    fn resolve(&self, item: &QueuedNotification) -> Result<&'a Job, AppError> {
        if let Some(row) = self
            .by_id
            .get(&item.id)
            .or_else(|| self.by_notification.get(&item.notification_id))
        {
            // An exact ID collision must never fall back to a business dedup key,
            // including when queue ID and notification ID refer to different rows.
            if !same_identity(&row.notification()?, item) {
                return Err(integrity("notification_identity_conflict"));
            }
            return Ok(row);
        }
        item.dedup_key
            .as_ref()
            .and_then(|key| {
                self.by_business.get(&(
                    item.source.parts().0.into(),
                    item.recipient_id,
                    key.clone(),
                ))
            })
            .copied()
            .ok_or_else(|| integrity("notification_identity_conflict"))
    }
}
pub async fn enqueue_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    items: &[QueuedNotification],
) -> Result<Vec<EnqueueOutcome>, AppError> {
    if items.is_empty() {
        return Ok(vec![]);
    }
    let input: Vec<_> = items
        .iter()
        .map(|n| {
            let (source_kind, source_id) = n.source.parts();
            InputJob {
                id: n.id,
                notification_id: n.notification_id,
                recipient_id: n.recipient_id,
                source_kind,
                source_id,
                dedup_key: &n.dedup_key,
                title: &n.title,
                message: &n.message,
                type_: n.type_.as_str(),
                link: &n.link,
            }
        })
        .collect();
    let inserted: Vec<Uuid> = sqlx::query_scalar(r#"INSERT INTO notification_dispatch_jobs(id,notification_id,recipient_id,source_kind,source_id,dedup_key,title,message,type,link)
        SELECT id,notification_id,recipient_id,source_kind,source_id,dedup_key,title,message,type,link FROM jsonb_to_recordset($1) n(id uuid,notification_id uuid,recipient_id uuid,source_kind text,source_id uuid,dedup_key text,title text,message text,type text,link text)
        ON CONFLICT DO NOTHING RETURNING id"#).bind(Json(&input)).fetch_all(&mut **tx).await?;
    let existing: Vec<Job> = sqlx::query_as(r#"SELECT DISTINCT j.id,j.notification_id,j.recipient_id,j.source_kind,j.source_id,j.dedup_key,j.title,j.message,j.type type_,j.link,j.stored_at,j.lease_token
        FROM notification_dispatch_jobs j JOIN jsonb_to_recordset($1) n(id uuid,notification_id uuid,recipient_id uuid,source_kind text,dedup_key text)
        ON j.id=n.id OR j.notification_id=n.notification_id OR (j.source_kind=n.source_kind AND j.recipient_id=n.recipient_id AND j.dedup_key=n.dedup_key)"#).bind(Json(&input)).fetch_all(&mut **tx).await?;
    let index = JobIndex::new(&existing);
    let inserted: HashSet<_> = inserted.into_iter().collect();
    let mut outcomes = Vec::with_capacity(items.len());
    for item in items {
        let job = index.resolve(item)?;
        outcomes.push(EnqueueOutcome {
            input_id: item.id,
            queue_id: job.id,
            notification_id: job.notification_id,
            source: job.notification()?.source,
            inserted: inserted.contains(&job.id),
        });
    }
    Ok(outcomes)
}
pub async fn store_and_enqueue_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    item: &QueuedNotification,
) -> Result<Uuid, AppError> {
    let outcome = enqueue_in_transaction(tx, std::slice::from_ref(item))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| integrity("notification_enqueue_missing"))?;
    if outcome.queue_id != item.id {
        return Err(integrity("notification_identity_conflict"));
    }
    // Queue before inbox is also the cancellation lock order.
    sqlx::query("SELECT id FROM notification_dispatch_jobs WHERE id=$1 FOR UPDATE")
        .bind(item.id)
        .execute(&mut **tx)
        .await?;
    insert_inbox(&mut **tx, item).await?;
    sqlx::query(
        "UPDATE notification_dispatch_jobs SET stored_at=coalesce(stored_at,now()) WHERE id=$1",
    )
    .bind(item.id)
    .execute(&mut **tx)
    .await?;
    Ok(item.notification_id)
}
async fn insert_inbox(
    connection: &mut PgConnection,
    item: &QueuedNotification,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO notifications(id,user_id,title,message,type,link) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(id) DO NOTHING")
        .bind(item.notification_id).bind(item.recipient_id).bind(&item.title).bind(&item.message).bind(item.type_.as_str()).bind(&item.link).execute(&mut *connection).await?;
    validate_inbox(connection, item).await
}
async fn validate_inbox(
    connection: &mut PgConnection,
    item: &QueuedNotification,
) -> Result<(), AppError> {
    let matches:bool=sqlx::query_scalar("SELECT user_id IS NOT DISTINCT FROM $2 AND title=$3 AND message=$4 AND type=$5 AND link IS NOT DISTINCT FROM $6 FROM notifications WHERE id=$1")
        .bind(item.notification_id).bind(item.recipient_id).bind(&item.title).bind(&item.message).bind(item.type_.as_str()).bind(&item.link).fetch_one(connection).await?;
    if !matches {
        return Err(integrity("notification_inbox_identity_conflict"));
    }
    Ok(())
}
pub async fn progress_on_connection(
    connection: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<NotificationProgress>, AppError> {
    let rows:Vec<NotificationProgress>=sqlx::query_as("SELECT notification_id,stored_at,published_at FROM notification_dispatch_jobs WHERE notification_id=ANY($1)").bind(ids).fetch_all(connection).await?;
    require_complete_progress(ids, &rows)?;
    Ok(rows)
}
fn require_complete_progress(ids: &[Uuid], rows: &[NotificationProgress]) -> Result<(), AppError> {
    let present: HashSet<_> = rows.iter().map(|row| row.notification_id).collect();
    if ids.iter().any(|id| !present.contains(id)) {
        return Err(integrity("notification_progress_missing"));
    }
    Ok(())
}
pub async fn cancel_identified_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source: NotificationSourceKind,
    ids: &[Uuid],
    remove_inbox: bool,
) -> Result<(), AppError> {
    let owned:Vec<Uuid>=sqlx::query_scalar("SELECT notification_id FROM notification_dispatch_jobs WHERE source_kind=$1 AND notification_id=ANY($2) ORDER BY id FOR UPDATE")
        .bind(source.as_str()).bind(ids).fetch_all(&mut **tx).await?;
    if remove_inbox {
        sqlx::query("DELETE FROM notifications WHERE id=ANY($1)")
            .bind(&owned)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query(
        "DELETE FROM notification_dispatch_jobs WHERE source_kind=$1 AND notification_id=ANY($2)",
    )
    .bind(source.as_str())
    .bind(&owned)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
pub async fn claim(pool: &PgPool) -> Result<Vec<ClaimedNotification>, AppError> {
    let rows:Vec<Job>=sqlx::query_as(r#"WITH due AS (
        SELECT id FROM notification_dispatch_jobs j WHERE published_at IS NULL AND next_attempt_at<=now()
        ORDER BY next_attempt_at,id LIMIT $1 FOR UPDATE OF j SKIP LOCKED
    ) UPDATE notification_dispatch_jobs j SET lease_token=gen_random_uuid(),
        next_attempt_at=now()+interval '120 seconds',attempts=j.attempts+1 FROM due WHERE j.id=due.id
        RETURNING j.id,j.notification_id,j.recipient_id,j.source_kind,j.source_id,j.dedup_key,j.title,j.message,j.type type_,j.link,j.stored_at,j.lease_token"#)
        .bind(DELIVERY_BATCH_SIZE).fetch_all(pool).await?;
    rows.into_iter()
        .filter_map(|j| {
            j.lease_token.map(|lease_token| {
                j.notification().map(|notification| ClaimedNotification {
                    notification,
                    lease_token,
                })
            })
        })
        .collect()
}
pub async fn materialize(
    pool: &PgPool,
    claimed: &ClaimedNotification,
    eligibility: &dyn DeliveryEligibilityPort,
) -> Result<Option<Notification>, AppError> {
    let mut tx = pool.begin().await?;
    let allowed = eligibility
        .authorize_on_connection(
            &mut tx,
            &claimed.notification.source,
            claimed.notification.recipient_id,
        )
        .await?;
    let job:Option<Job>=sqlx::query_as("SELECT id,notification_id,recipient_id,source_kind,source_id,dedup_key,title,message,type type_,link,stored_at,lease_token FROM notification_dispatch_jobs WHERE id=$1 AND lease_token=$2 AND published_at IS NULL AND next_attempt_at>now() FOR UPDATE")
        .bind(claimed.notification.id).bind(claimed.lease_token).fetch_optional(&mut *tx).await?;
    let Some(job) = job else {
        tx.commit().await?;
        return Ok(None);
    };
    if !same_identity(&job.notification()?, &claimed.notification) {
        return Err(integrity("notification_identity_conflict"));
    }
    if !allowed {
        sqlx::query("UPDATE notification_dispatch_jobs SET published_at=now(),lease_token=NULL WHERE id=$1 AND lease_token=$2").bind(job.id).bind(claimed.lease_token).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(None);
    }
    if job.stored_at.is_none() {
        insert_inbox(&mut tx, &claimed.notification).await?;
        sqlx::query(
            "UPDATE notification_dispatch_jobs SET stored_at=now() WHERE id=$1 AND lease_token=$2",
        )
        .bind(job.id)
        .bind(claimed.lease_token)
        .execute(&mut *tx)
        .await?;
    }
    let notification = sqlx::query_as(
        "SELECT id,title,message,type type_,link,read_at,created_at FROM notifications WHERE id=$1",
    )
    .bind(job.notification_id)
    .fetch_optional(&mut *tx)
    .await?;
    if notification.is_none() {
        sqlx::query("UPDATE notification_dispatch_jobs SET published_at=now(),lease_token=NULL WHERE id=$1 AND lease_token=$2").bind(job.id).bind(claimed.lease_token).execute(&mut *tx).await?;
        tracing::warn!(error_code="notification_inbox_missing",notification_id=%job.notification_id,"Previously stored notification is absent");
    } else {
        validate_inbox(&mut tx, &claimed.notification).await?;
    }
    tx.commit().await?;
    Ok(notification)
}
pub async fn acknowledge(pool: &PgPool, claimed: &ClaimedNotification) -> Result<(), AppError> {
    sqlx::query("UPDATE notification_dispatch_jobs SET published_at=now(),lease_token=NULL WHERE id=$1 AND lease_token=$2 AND published_at IS NULL AND next_attempt_at>now()")
        .bind(claimed.notification.id).bind(claimed.lease_token).execute(pool).await?;
    Ok(())
}
/// Fence endpoint cleanup and acknowledgment together against a stale claim.
pub(crate) async fn finish_delivery(
    pool: &PgPool,
    claimed: &ClaimedNotification,
    receipt: crate::publisher::DeliveryReceipt,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let current: Option<Job> = sqlx::query_as("SELECT id,notification_id,recipient_id,source_kind,source_id,dedup_key,title,message,type type_,link,stored_at,lease_token FROM notification_dispatch_jobs WHERE id=$1 AND lease_token=$2 AND published_at IS NULL AND next_attempt_at>now() FOR UPDATE")
        .bind(claimed.notification.id).bind(claimed.lease_token).fetch_optional(&mut *tx).await?;
    let Some(current) = current else {
        tx.commit().await?;
        return Ok(());
    };
    if !same_identity(&current.notification()?, &claimed.notification) {
        return Err(integrity("notification_identity_conflict"));
    }
    let delivery = crate::publisher::NotificationService::complete_delivery_on_connection(
        &mut tx,
        claimed.notification.recipient_id,
        receipt,
    )
    .await;
    if delivery.is_ok() {
        sqlx::query("UPDATE notification_dispatch_jobs SET published_at=now(),lease_token=NULL WHERE id=$1 AND lease_token=$2 AND next_attempt_at>now()")
            .bind(claimed.notification.id).bind(claimed.lease_token).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    delivery.map_err(|_| AppError::ServiceUnavailable("notification_transport_unavailable".into()))
}

#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct QueueStatus {
    pub pending: bool,
    pub due: bool,
}
pub async fn status(pool: &PgPool) -> Result<QueueStatus, AppError> {
    Ok(sqlx::query_as("SELECT EXISTS(SELECT 1 FROM notification_dispatch_jobs WHERE published_at IS NULL) pending,EXISTS(SELECT 1 FROM notification_dispatch_jobs WHERE published_at IS NULL AND next_attempt_at<=now()) due").fetch_one(pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job() -> Job {
        Job {
            id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            recipient_id: Uuid::new_v4(),
            source_kind: "direct".into(),
            source_id: None,
            dedup_key: None,
            title: "Synthetic".into(),
            message: "Body".into(),
            type_: "info".into(),
            link: None,
            stored_at: None,
            lease_token: None,
        }
    }
    #[test]
    fn source_and_type_decoding_reject_unknown_or_inconsistent_persisted_policy() {
        let mut row = job();
        assert_eq!(
            row.notification().unwrap().source,
            NotificationSource::Direct
        );
        row.source_kind = "unknown".into();
        assert!(row.notification().is_err());
        row.source_kind = "attendance".into();
        assert!(row.notification().is_err());
        row.source_id = Some(row.id);
        assert_eq!(
            row.notification().unwrap().source,
            NotificationSource::Attendance { source_id: row.id }
        );
        row.source_kind = "direct".into();
        assert!(row.notification().is_err());
        row.source_id = None;
        row.type_ = "private-unregistered-type".into();
        assert!(row.notification().is_err());
    }
    #[test]
    fn stable_identity_validation_rejects_recipient_payload_and_source_drift() {
        let original = job().notification().unwrap();
        let mut changed = original.clone();
        assert!(same_identity(&original, &changed));
        changed.recipient_id = Uuid::new_v4();
        assert!(!same_identity(&original, &changed));
        changed = original.clone();
        changed.message = "Different body".into();
        assert!(!same_identity(&original, &changed));
        changed = original.clone();
        changed.source = NotificationSource::Attendance {
            source_id: changed.id,
        };
        assert!(!same_identity(&original, &changed));
    }
    #[test]
    fn progress_requires_every_identity_but_accepts_duplicate_requests() {
        let id = Uuid::new_v4();
        let rows = [NotificationProgress {
            notification_id: id,
            stored_at: None,
            published_at: None,
        }];
        assert!(require_complete_progress(&[id, id], &rows).is_ok());
        assert!(
            matches!(require_complete_progress(&[id,Uuid::new_v4()],&rows),Err(AppError::Conflict(code)) if code=="notification_progress_missing")
        );
        assert!(require_complete_progress(&[], &[]).is_ok());
    }
    #[test]
    fn enqueue_lookup_preserves_identity_precedence_first_wins_and_null_key_separation() {
        let mut first = job();
        first.dedup_key = Some("arrival-day".into());
        let second = job();
        let exact = first.notification().unwrap();
        let rows = [first, second];
        let index = JobIndex::new(&rows);
        assert_eq!(index.resolve(&exact).unwrap().id, rows[0].id);
        let mut repeated = exact.clone();
        repeated.id = Uuid::new_v4();
        repeated.notification_id = Uuid::new_v4();
        repeated.message = "Later arrival time".into();
        assert_eq!(
            index.resolve(&repeated).unwrap().notification_id,
            rows[0].notification_id
        );
        repeated.id = rows[1].id;
        assert!(index.resolve(&repeated).is_err());
        let mut split = exact.clone();
        split.notification_id = rows[1].notification_id;
        assert!(index.resolve(&split).is_err());
        let mut no_key = rows[1].notification().unwrap();
        no_key.id = Uuid::new_v4();
        no_key.notification_id = Uuid::new_v4();
        assert!(index.resolve(&no_key).is_err());
        let mut wrong_recipient = exact.clone();
        wrong_recipient.recipient_id = Uuid::new_v4();
        assert!(index.resolve(&wrong_recipient).is_err());
    }
}
