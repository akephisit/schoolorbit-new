//! Authoritative transactional inbox acceptance and irreversible presentation consumption.
use crate::{models::Notification, publisher::NotificationType};
use chrono::{DateTime, Utc};
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

pub const DELIVERY_BATCH_SIZE: i64 = 32;
pub const ALERT_FRESHNESS_SECONDS: i64 = 900;
const MAX_ACCEPTANCE_BATCH: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationSource {
    Direct,
    Attendance { source_id: Uuid },
    Process { source_id: Uuid },
    CalendarReminder { source_id: Uuid },
}
#[derive(Clone, Copy)]
pub enum NotificationSourceKind {
    Direct,
    Attendance,
    Process,
    CalendarReminder,
}
impl NotificationSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Attendance => "attendance",
            Self::Process => "process",
            Self::CalendarReminder => "calendar_reminder",
        }
    }
}
impl NotificationSource {
    fn parts(self) -> (&'static str, Option<Uuid>) {
        match self {
            Self::Direct => ("direct", None),
            Self::Attendance { source_id } => ("attendance", Some(source_id)),
            Self::Process { source_id } => ("process", Some(source_id)),
            Self::CalendarReminder { source_id } => ("calendar_reminder", Some(source_id)),
        }
    }
    fn decode(kind: &str, id: Option<Uuid>) -> Result<Self, AppError> {
        match (kind, id) {
            ("direct", None) => Ok(Self::Direct),
            ("attendance", Some(source_id)) => Ok(Self::Attendance { source_id }),
            ("process", Some(source_id)) => Ok(Self::Process { source_id }),
            ("calendar_reminder", Some(source_id)) => Ok(Self::CalendarReminder { source_id }),
            _ => Err(integrity("notification_source_unsupported")),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Presentation {
    #[default]
    AlertOnce,
    InboxOnly,
}
impl Presentation {
    fn as_str(self) -> &'static str {
        match self {
            Self::AlertOnce => "alert_once",
            Self::InboxOnly => "inbox_only",
        }
    }
}
#[derive(Clone, Debug)]
pub struct NewNotification {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub recipient_id: Uuid,
    pub source: NotificationSource,
    pub idempotency_key: String,
    pub title: String,
    pub message: String,
    pub type_: NotificationType,
    pub link: Option<String>,
    pub presentation: Presentation,
}
/// Message meaning is owned by the producer; receipt and inbox IDs are central defaults.
#[derive(Clone, Debug)]
pub struct NotificationContent {
    pub title: String,
    pub message: String,
    pub type_: NotificationType,
    pub link: Option<String>,
}
impl NewNotification {
    pub fn new(
        recipient_id: Uuid,
        source: NotificationSource,
        idempotency_key: String,
        content: NotificationContent,
        presentation: Presentation,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            notification_id: Uuid::now_v7(),
            recipient_id,
            source,
            idempotency_key,
            title: content.title,
            message: content.message,
            type_: content.type_,
            link: content.link,
            presentation,
        }
    }
}
pub struct AcceptanceOutcome {
    pub input_id: Uuid,
    pub receipt_id: Uuid,
    pub notification_id: Uuid,
    pub source: NotificationSource,
    pub inserted: bool,
}
#[derive(Clone, Copy)]
pub struct PendingPresentation {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub recipient_id: Uuid,
    pub source: NotificationSource,
    pub expires_at: DateTime<Utc>,
}
#[derive(sqlx::FromRow)]
pub struct NotificationProgress {
    pub notification_id: Uuid,
    pub inbox_accepted_at: Option<DateTime<Utc>>,
    /// Terminal presentation (attempted/skipped/expired), never proof of human delivery.
    pub presentation_consumed_at: Option<DateTime<Utc>>,
}
#[async_trait::async_trait]
pub trait DeliveryEligibilityPort: Send + Sync {
    /// Source guards precede the receipt lock, matching source-owned cancellation order.
    async fn authorize_on_connection(
        &self,
        connection: &mut PgConnection,
        source: &NotificationSource,
        recipient_id: Uuid,
    ) -> Result<bool, AppError>;
}
#[derive(serde::Serialize)]
struct Input<'a> {
    id: Uuid,
    notification_id: Uuid,
    recipient_id: Uuid,
    source_kind: &'static str,
    source_id: Option<Uuid>,
    idempotency_key: &'a str,
    title: &'a str,
    message: &'a str,
    #[serde(rename = "type")]
    type_: &'static str,
    link: &'a Option<String>,
    presentation: &'static str,
}
#[derive(sqlx::FromRow)]
struct ResolvedInput {
    input_id: Uuid,
    id: Uuid,
    notification_id: Uuid,
    recipient_id: Uuid,
    source_kind: String,
    source_id: Option<Uuid>,
    idempotency_key: String,
    payload_matches: bool,
    presentation: String,
}
fn integrity(code: &'static str) -> AppError {
    AppError::Conflict(code.into())
}
/// Same-origin absolute app paths only. No protocol-relative URLs, escapes or control characters.
pub fn valid_app_link(value: &str) -> bool {
    value.len() <= 2048
        && value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && !value.to_ascii_lowercase().starts_with("/%2f")
        && !value.to_ascii_lowercase().starts_with("/%5c")
}
fn validate(items: &[NewNotification]) -> Result<(), AppError> {
    if items.len() > MAX_ACCEPTANCE_BATCH {
        return Err(AppError::BadRequest("notification_batch_too_large".into()));
    }
    let mut ids = HashSet::new();
    let mut notification_ids = HashSet::new();
    for n in items {
        if !ids.insert(n.id) || !notification_ids.insert(n.notification_id) {
            return Err(integrity("notification_duplicate_input_identity"));
        }
        if n.idempotency_key.trim().is_empty()
            || n.idempotency_key.len() > 512
            || n.title.chars().count() > 255
            || n.message.len() > 8192
            || n.link.as_deref().is_some_and(|s| !valid_app_link(s))
        {
            return Err(AppError::BadRequest("notification_payload_invalid".into()));
        }
    }
    Ok(())
}
/// Producers have already resolved active users and guardian relationships in this transaction.
/// Replays return the original receipt/inbox identity without recreating a removed inbox or alert.
pub async fn create_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    items: &[NewNotification],
) -> Result<Vec<AcceptanceOutcome>, AppError> {
    validate(items)?;
    let mut outcomes = Vec::with_capacity(items.len());
    for chunk in items.chunks(1000) {
        let input: Vec<_> = chunk
            .iter()
            .map(|n| {
                let (source_kind, source_id) = n.source.parts();
                Input {
                    id: n.id,
                    notification_id: n.notification_id,
                    recipient_id: n.recipient_id,
                    source_kind,
                    source_id,
                    idempotency_key: &n.idempotency_key,
                    title: &n.title,
                    message: &n.message,
                    type_: n.type_.as_str(),
                    link: &n.link,
                    presentation: n.presentation.as_str(),
                }
            })
            .collect();
        let inserted:Vec<Uuid>=school_database::DbSession::new(&mut **tx).scalar("notifications.accept.insert_receipts",r#"INSERT INTO notification_receipts(id,notification_id,recipient_id,source_kind,source_id,idempotency_key,payload_hash,presentation,inbox_accepted_at,presentation_consumed_at,presentation_outcome)
            SELECT id,notification_id,recipient_id,source_kind,source_id,idempotency_key,sha256(convert_to(jsonb_build_array(title,message,type,link)::text,'UTF8')),presentation,now(),CASE WHEN presentation='inbox_only' THEN now() END,CASE WHEN presentation='inbox_only' THEN 'skipped' END
            FROM jsonb_to_recordset($1) n(id uuid,notification_id uuid,recipient_id uuid,source_kind text,source_id uuid,idempotency_key text,title text,message text,type text,link text,presentation text)
            ON CONFLICT DO NOTHING RETURNING id"#).bind(Json(&input)).fetch_all().await?;
        let rows:Vec<ResolvedInput>=school_database::DbSession::new(&mut **tx).query_as("notifications.accept.resolve_receipts",r#"SELECT n.id input_id,j.id,j.notification_id,j.recipient_id,j.source_kind,j.source_id,j.idempotency_key,j.presentation,
            j.payload_hash=sha256(convert_to(jsonb_build_array(n.title,n.message,n.type,n.link)::text,'UTF8')) payload_matches
            FROM jsonb_to_recordset($1) n(id uuid,notification_id uuid,recipient_id uuid,source_kind text,source_id uuid,idempotency_key text,title text,message text,type text,link text,presentation text)
            JOIN notification_receipts j ON j.id=n.id OR j.notification_id=n.notification_id OR (j.source_kind=n.source_kind AND j.recipient_id=n.recipient_id AND j.idempotency_key=n.idempotency_key)
            ORDER BY j.id FOR UPDATE OF j"#).bind(Json(&input)).fetch_all().await?;
        let inserted: HashSet<_> = inserted.into_iter().collect();
        for n in chunk {
            let matches: Vec<_> = rows.iter().filter(|r| r.input_id == n.id).collect();
            let [r] = matches.as_slice() else {
                return Err(integrity("notification_identity_conflict"));
            };
            let source = NotificationSource::decode(&r.source_kind, r.source_id)?;
            if source != n.source
                || r.recipient_id != n.recipient_id
                || r.idempotency_key != n.idempotency_key
                || !r.payload_matches
                || r.presentation != n.presentation.as_str()
                || ((r.id == n.id || r.notification_id == n.notification_id)
                    && (r.id != n.id || r.notification_id != n.notification_id))
            {
                return Err(integrity("notification_identity_conflict"));
            }
            outcomes.push(AcceptanceOutcome {
                input_id: n.id,
                receipt_id: r.id,
                notification_id: r.notification_id,
                source,
                inserted: inserted.contains(&r.id),
            });
        }
        school_database::DbSession::new(&mut **tx).query("notifications.accept.insert_inbox",r#"INSERT INTO notifications(id,user_id,title,message,type,link)
            SELECT notification_id,recipient_id,title,message,type,link FROM jsonb_to_recordset($1) n(id uuid,notification_id uuid,recipient_id uuid,title text,message text,type text,link text)
            WHERE id=ANY($2)"#).bind(Json(&input)).bind(inserted.iter().copied().collect::<Vec<_>>()).execute().await?;
        school_database::DbSession::new(&mut **tx).query("notifications.accept.insert_presentations",r#"INSERT INTO notification_presentations(receipt_id,expires_at)
            SELECT id,created_at+interval '15 minutes' FROM notification_receipts WHERE id=ANY($1) AND presentation='alert_once'"#).bind(inserted.iter().copied().collect::<Vec<_>>()).execute().await?;
    }
    Ok(outcomes)
}
pub async fn create(
    pool: &PgPool,
    items: &[NewNotification],
) -> Result<Vec<AcceptanceOutcome>, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(pool, "notifications.create").await?;
    let recipients: Vec<_> = items.iter().map(|n| n.recipient_id).collect();
    let active: i64 = school_database::DbSession::new(&mut *tx)
        .scalar(
            "notifications.create.active_recipients",
            "SELECT count(*) FROM users WHERE id=ANY($1) AND status='active'",
        )
        .bind(&recipients)
        .fetch_one()
        .await?;
    if usize::try_from(active).ok() != Some(recipients.iter().collect::<HashSet<_>>().len()) {
        return Err(AppError::BadRequest(
            "notification_recipient_inactive".into(),
        ));
    }
    let result = create_in_transaction(&mut tx, items).await?;
    tx.commit().await?;
    Ok(result)
}
/// Bounded candidates only: no claim/lease or transport retry exists. Consumption arbitrates races.
pub async fn pending(pool: &PgPool) -> Result<Vec<PendingPresentation>, AppError> {
    let mut c =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "notifications.pending").await?;
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        notification_id: Uuid,
        recipient_id: Uuid,
        source_kind: String,
        source_id: Option<Uuid>,
        expires_at: DateTime<Utc>,
    }
    let rows:Vec<Row>=school_database::DbSession::new(&mut *c).query_as("notifications.presentation.pending","SELECT r.id,r.notification_id,r.recipient_id,r.source_kind,r.source_id,p.expires_at FROM notification_presentations p JOIN notification_receipts r ON r.id=p.receipt_id ORDER BY r.id LIMIT $1").bind(DELIVERY_BATCH_SIZE).fetch_all().await?;
    rows.into_iter()
        .map(|r| {
            Ok(PendingPresentation {
                id: r.id,
                notification_id: r.notification_id,
                recipient_id: r.recipient_id,
                source: NotificationSource::decode(&r.source_kind, r.source_id)?,
                expires_at: r.expires_at,
            })
        })
        .collect()
}
/// Irreversible commit BEFORE any SSE/network I/O. An uncertain commit is never a delivery retry.
pub async fn consume(
    pool: &PgPool,
    item: &PendingPresentation,
    eligibility: &dyn DeliveryEligibilityPort,
) -> Result<Option<Notification>, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(pool, "notifications.consume").await?;
    let allowed = eligibility
        .authorize_on_connection(&mut tx, &item.source, item.recipient_id)
        .await?;
    let locked:Option<(Uuid,Uuid,String,Option<Uuid>,bool)>=school_database::DbSession::new(&mut *tx).query_as("notifications.presentation.lock","SELECT r.notification_id,r.recipient_id,r.source_kind,r.source_id,p.expires_at>now() FROM notification_receipts r JOIN notification_presentations p ON p.receipt_id=r.id WHERE r.id=$1 AND r.presentation_consumed_at IS NULL FOR UPDATE OF r,p").bind(item.id).fetch_optional().await?;
    let Some((notification_id, recipient, kind, source_id, fresh)) = locked else {
        tx.commit().await?;
        return Ok(None);
    };
    if notification_id != item.notification_id
        || recipient != item.recipient_id
        || NotificationSource::decode(&kind, source_id)? != item.source
    {
        return Err(integrity("notification_identity_conflict"));
    }
    let notification:Option<Notification>=school_database::DbSession::new(&mut *tx).query_as("notifications.presentation.inbox","SELECT n.id,n.title,n.message,n.type type_,n.link,n.read_at,n.created_at FROM notifications n JOIN notification_receipts r ON r.notification_id=n.id WHERE r.id=$1 AND n.user_id=r.recipient_id AND r.payload_hash=sha256(convert_to(jsonb_build_array(n.title,n.message,n.type,n.link)::text,'UTF8')) FOR SHARE OF n").bind(item.id).fetch_optional().await?;
    let outcome = if !fresh {
        "expired"
    } else if !allowed || notification.is_none() {
        "skipped"
    } else {
        "attempted"
    };
    school_database::DbSession::new(&mut *tx).query("notifications.presentation.consume","UPDATE notification_receipts SET presentation_consumed_at=now(),presentation_outcome=$2 WHERE id=$1").bind(item.id).bind(outcome).execute().await?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "notifications.presentation.remove_pending",
            "DELETE FROM notification_presentations WHERE receipt_id=$1",
        )
        .bind(item.id)
        .execute()
        .await?;
    tx.commit().await?;
    Ok(if outcome == "attempted" {
        notification
    } else {
        None
    })
}
pub async fn progress_on_connection(
    connection: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<NotificationProgress>, AppError> {
    let rows:Vec<NotificationProgress>=school_database::DbSession::new(connection).query_as("notifications.progress","SELECT notification_id,inbox_accepted_at,presentation_consumed_at FROM notification_receipts WHERE notification_id=ANY($1)").bind(ids).fetch_all().await?;
    let present: HashSet<_> = rows.iter().map(|r| r.notification_id).collect();
    if ids.iter().any(|id| !present.contains(id)) {
        return Err(integrity("notification_progress_missing"));
    }
    Ok(rows)
}
pub async fn cancel_identified_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source: NotificationSourceKind,
    ids: &[Uuid],
    remove_inbox: bool,
) -> Result<(), AppError> {
    let owned:Vec<Uuid>=school_database::DbSession::new(&mut **tx).scalar("notifications.cancel.lock","SELECT notification_id FROM notification_receipts WHERE source_kind=$1 AND notification_id=ANY($2) ORDER BY id FOR UPDATE").bind(source.as_str()).bind(ids).fetch_all().await?;
    if remove_inbox {
        school_database::DbSession::new(&mut **tx)
            .query(
                "notifications.cancel.inbox",
                "DELETE FROM notifications WHERE id=ANY($1)",
            )
            .bind(&owned)
            .execute()
            .await?;
    }
    // Exact source-owned reset is the sole operation allowed to release business identities.
    school_database::DbSession::new(&mut **tx)
        .query(
            "notifications.cancel.receipts",
            "DELETE FROM notification_receipts WHERE source_kind=$1 AND notification_id=ANY($2)",
        )
        .bind(source.as_str())
        .bind(&owned)
        .execute()
        .await?;
    Ok(())
}
#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct QueueStatus {
    pub pending: bool,
    pub due: bool,
}
pub async fn status(pool: &PgPool) -> Result<QueueStatus, AppError> {
    let mut c =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "notifications.status").await?;
    Ok(school_database::DbSession::new(&mut *c).query_as("notifications.presentation.status","SELECT EXISTS(SELECT 1 FROM notification_presentations) pending,EXISTS(SELECT 1 FROM notification_presentations) due").fetch_one().await?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_links_reject_external_ambiguous_and_control_paths() {
        for s in [
            "https://evil.test",
            "//evil.test",
            "/\\evil",
            "/%2fhost",
            "/%5chost",
            "/x\n",
        ] {
            assert!(!valid_app_link(s), "{s:?}")
        }
        assert!(valid_app_link("/student/attendance?term=1"));
    }
    #[test]
    fn inconsistent_source_identity_fails_closed() {
        assert!(NotificationSource::decode("direct", Some(Uuid::new_v4())).is_err());
        assert!(NotificationSource::decode("future", None).is_err());
    }
}
