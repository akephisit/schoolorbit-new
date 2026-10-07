use crate::models::{CalendarEvent, UpsertCalendarEventRequest};
use crate::services::events::create_event_in_transaction;
use crate::services::shared::{validate_event_date_time, validate_event_text};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum CalendarRequestStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCalendarRequest {
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventRequest {
    pub id: Uuid,
    pub requested_by: Uuid,
    pub requester_name: String,
    pub title: String,
    pub description: String,
    #[schema(required = true)]
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    #[schema(required = true)]
    pub start_time: Option<NaiveTime>,
    #[schema(required = true)]
    pub end_time: Option<NaiveTime>,
    pub status: CalendarRequestStatus,
    #[schema(required = true)]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub rejection_reason: Option<String>,
    #[schema(required = true)]
    pub event_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct CalendarRequestQuery {
    pub status: Option<CalendarRequestStatus>,
    #[serde(default)]
    pub review: bool,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRequestPage {
    pub records: Vec<CalendarEventRequest>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct PendingCalendarQuery {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingCalendarRequest {
    pub id: Uuid,
    pub title: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    #[schema(required = true)]
    pub start_time: Option<NaiveTime>,
    #[schema(required = true)]
    pub end_time: Option<NaiveTime>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingCalendarPage {
    pub records: Vec<PendingCalendarRequest>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RejectCalendarRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRequestApproval {
    pub request: CalendarEventRequest,
    pub event: CalendarEvent,
}

const SELECT_REQUEST: &str = "SELECT r.id,r.requested_by,concat_ws(' ',u.first_name,u.last_name) AS requester_name,
    r.title,r.description,r.location,r.start_date,r.end_date,r.all_day,r.start_time,r.end_time,r.status,
    r.reviewed_at,r.rejection_reason,r.event_id,r.created_at
    FROM calendar_event_requests r JOIN users u ON u.id=r.requested_by";

pub fn require_request_access(actor: &ActorContext, review: bool) -> Result<(), AppError> {
    actor.require_permission(codes::CALENDAR_READ_SCHOOL)?;
    if review {
        actor.require_permission(codes::CALENDAR_MANAGE_SCHOOL)
    } else {
        actor.require_any_permission(&[codes::CALENDAR_REQUEST_OWN, codes::CALENDAR_MANAGE_SCHOOL])
    }
}

pub fn validate_request(payload: &CreateCalendarRequest) -> Result<(), AppError> {
    validate_event_text(
        &payload.title,
        Some(&payload.description),
        payload.location.as_deref(),
    )?;
    if payload.description.trim().is_empty() {
        return Err(AppError::BadRequest("กรุณาระบุรายละเอียดกิจกรรม".into()));
    }
    validate_event_date_time(
        payload.start_date,
        payload.end_date,
        payload.all_day,
        payload.start_time,
        payload.end_time,
    )
}

pub async fn list_requests(
    pool: &PgPool,
    actor: &ActorContext,
    query: CalendarRequestQuery,
) -> Result<CalendarRequestPage, AppError> {
    require_request_access(actor, query.review)?;
    let offset = query.offset.unwrap_or(0);
    if !(0..=100000).contains(&offset) {
        return Err(AppError::BadRequest("หน้าคำร้องไม่ถูกต้อง".into()));
    }
    let status = query.status.as_ref().map(|status| match status {
        CalendarRequestStatus::Pending => "pending",
        CalendarRequestStatus::Approved => "approved",
        CalendarRequestStatus::Rejected => "rejected",
    });
    let order = if query.review { "ASC" } else { "DESC" };
    let sql=format!("{SELECT_REQUEST} WHERE ($1::boolean OR r.requested_by=$2) AND (NOT $1::boolean OR r.status<>'approved') AND ($3::text IS NULL OR r.status=$3) ORDER BY r.created_at {order},r.id {order} LIMIT 26 OFFSET $4");
    let mut records = sqlx::query_as::<_, CalendarEventRequest>(sqlx::AssertSqlSafe(sql))
        .bind(query.review)
        .bind(actor.user_id)
        .bind(status)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let has_more = records.len() > 25;
    records.truncate(25);
    Ok(CalendarRequestPage { records, has_more })
}

fn validate_pending_range(query: &PendingCalendarQuery) -> Result<(), AppError> {
    let days = (query.to - query.from).num_days();
    if !(0..=61).contains(&days) {
        return Err(AppError::BadRequest("ช่วงวันที่คำร้องต้องไม่เกิน 62 วัน".into()));
    }
    Ok(())
}

pub async fn list_pending_calendar(
    pool: &PgPool,
    actor: &ActorContext,
    query: PendingCalendarQuery,
) -> Result<PendingCalendarPage, AppError> {
    require_request_access(actor, false)?;
    validate_pending_range(&query)?;
    let mut records = sqlx::query_as::<_, PendingCalendarRequest>(
        "SELECT id,title,start_date,end_date,all_day,start_time,end_time
         FROM calendar_event_requests
         WHERE status='pending' AND start_date<=$1 AND end_date>=$2
           AND ($3::boolean OR requested_by=$4)
         ORDER BY created_at ASC,id ASC LIMIT 501",
    )
    .bind(query.to)
    .bind(query.from)
    .bind(actor.has_permission(codes::CALENDAR_MANAGE_SCHOOL))
    .bind(actor.user_id)
    .fetch_all(pool)
    .await?;
    let has_more = records.len() > 500;
    records.truncate(500);
    Ok(PendingCalendarPage { records, has_more })
}

async fn get_request(pool: &PgPool, id: Uuid) -> Result<CalendarEventRequest, AppError> {
    let sql = format!("{SELECT_REQUEST} WHERE r.id=$1");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบคำร้อง".into()))
}

pub async fn get_request_for_review(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<CalendarEventRequest, AppError> {
    require_request_access(actor, true)?;
    get_request(pool, id).await
}

pub async fn create_request(
    pool: &PgPool,
    actor: &ActorContext,
    payload: CreateCalendarRequest,
) -> Result<CalendarEventRequest, AppError> {
    actor.require_all_permissions(&[codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_REQUEST_OWN])?;
    validate_request(&payload)?;
    let id=sqlx::query_scalar::<_,Uuid>("INSERT INTO calendar_event_requests(requested_by,title,description,location,start_date,end_date,all_day,start_time,end_time)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id")
        .bind(actor.user_id).bind(payload.title.trim()).bind(payload.description.trim())
        .bind(payload.location.as_deref().map(str::trim).filter(|value| !value.is_empty()))
        .bind(payload.start_date).bind(payload.end_date).bind(payload.all_day)
        .bind(if payload.all_day {None} else {payload.start_time})
        .bind(if payload.all_day {None} else {payload.end_time}).fetch_one(pool).await?;
    get_request(pool, id).await
}

pub async fn approve_request(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarRequestApproval, AppError> {
    require_request_access(actor, true)?;
    let mut tx = pool.begin().await?;
    let status: Option<CalendarRequestStatus> =
        sqlx::query_scalar("SELECT status FROM calendar_event_requests WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    require_pending(status)?;
    let event_id = create_event_in_transaction(&mut tx, actor.user_id, &payload).await?;
    sqlx::query("UPDATE calendar_event_requests SET status='approved',reviewed_by=$2,reviewed_at=now(),event_id=$3 WHERE id=$1")
        .bind(id).bind(actor.user_id).bind(event_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(CalendarRequestApproval {
        request: get_request(pool, id).await?,
        event: crate::services::get_event_for_response(pool, event_id).await?,
    })
}

pub async fn reject_request(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    reason: &str,
) -> Result<CalendarEventRequest, AppError> {
    require_request_access(actor, true)?;
    let reason = reason.trim();
    if reason.is_empty() || reason.chars().count() > 2000 {
        return Err(AppError::BadRequest("กรุณาระบุเหตุผลไม่เกิน 2000 ตัวอักษร".into()));
    }
    let mut tx = pool.begin().await?;
    let status =
        sqlx::query_scalar("SELECT status FROM calendar_event_requests WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    require_pending(status)?;
    sqlx::query("UPDATE calendar_event_requests SET status='rejected',reviewed_by=$2,reviewed_at=now(),rejection_reason=$3 WHERE id=$1")
        .bind(id).bind(actor.user_id).bind(reason).execute(&mut *tx).await?;
    tx.commit().await?;
    get_request(pool, id).await
}

fn require_pending(status: Option<CalendarRequestStatus>) -> Result<(), AppError> {
    match status {
        Some(CalendarRequestStatus::Pending) => Ok(()),
        None => Err(AppError::NotFound("ไม่พบคำร้อง".into())),
        Some(_) => Err(AppError::Conflict("คำร้องนี้พิจารณาแล้ว กรุณาโหลดข้อมูลใหม่".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
        }
    }
    #[test]
    fn pending_calendar_requires_a_bounded_ordered_date_range() {
        let from = NaiveDate::from_ymd_opt(2027, 6, 1).unwrap();
        for days in [0, 41, 61] {
            assert!(validate_pending_range(&PendingCalendarQuery {
                from,
                to: from + chrono::Duration::days(days),
            })
            .is_ok());
        }
        for days in [-1, 62, 365] {
            assert!(validate_pending_range(&PendingCalendarQuery {
                from,
                to: from + chrono::Duration::days(days),
            })
            .is_err());
        }
    }
    #[test]
    fn own_access_never_grants_review() {
        let own = actor(&[codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_REQUEST_OWN]);
        assert!(require_request_access(&own, false).is_ok());
        assert!(require_request_access(&own, true).is_err());
        assert!(require_request_access(&actor(&[codes::CALENDAR_READ_SCHOOL]), false).is_err());
        assert!(require_request_access(
            &actor(&[codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_MANAGE_SCHOOL]),
            true
        )
        .is_ok());
    }
    #[test]
    fn only_pending_requests_can_be_decided() {
        assert!(require_pending(Some(CalendarRequestStatus::Pending)).is_ok());
        assert!(matches!(require_pending(None), Err(AppError::NotFound(_))));
        for status in [
            CalendarRequestStatus::Approved,
            CalendarRequestStatus::Rejected,
        ] {
            assert!(matches!(
                require_pending(Some(status)),
                Err(AppError::Conflict(_))
            ));
        }
    }
    #[test]
    fn request_validation_accepts_dates_without_academic_context() {
        let mut payload = CreateCalendarRequest {
            title: "กิจกรรม".into(),
            description: "รายละเอียด".into(),
            location: None,
            start_date: NaiveDate::from_ymd_opt(2030, 1, 5).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2030, 1, 5).unwrap(),
            all_day: true,
            start_time: None,
            end_time: None,
        };
        assert!(validate_request(&payload).is_ok());
        payload.description = " ".into();
        assert!(validate_request(&payload).is_err());
        payload.description = "รายละเอียด".into();
        payload.all_day = false;
        assert!(validate_request(&payload).is_err());
        payload.all_day = true;
        payload.end_date = NaiveDate::from_ymd_opt(2030, 1, 4).unwrap();
        assert!(validate_request(&payload).is_err());
    }
}
