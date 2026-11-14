use chrono::{NaiveDate, Utc};
use school_academic_core::services::lifecycle_guard;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::collaboration::TimetableMutationContext;
use crate::models::timetable::{CreateTimetableRequest, Timetable};

pub mod opening;
mod term_preparation;
pub use term_preparation::apply as apply_term_preparation;

const COLUMNS: &str =
    "id,academic_term_id,academic_year_id,bell_schedule_id,row_version,created_at,updated_at";

pub fn bangkok_date_at(now: chrono::DateTime<Utc>) -> NaiveDate {
    (now + chrono::Duration::hours(7)).date_naive()
}
pub fn bangkok_today() -> NaiveDate {
    bangkok_date_at(Utc::now())
}

/// Reading an explicit term never creates its current timetable.
pub async fn get_for_term(pool: &PgPool, term: Uuid) -> Result<Option<Timetable>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.current.get_for_term",
    )
    .await?;
    get_for_term_on_connection(&mut connection, term).await
}
pub async fn get_for_term_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<Option<Timetable>, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM academic_timetables WHERE academic_term_id=$1");
    Ok(DbSession::new(connection)
        .query_as(
            "academic_timetable.current.get_for_term",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(term)
        .fetch_optional()
        .await?)
}
pub async fn get_by_id_on_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Timetable, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM academic_timetables WHERE id=$1");
    DbSession::new(connection)
        .query_as(
            "academic_timetable.current.get_by_id",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางเรียน".into()))
}
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Timetable, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.current.get_by_id",
    )
    .await?;
    get_by_id_on_connection(&mut connection, id).await
}

/// Singleton creation is explicit and serialized by the canonical term boundary.
/// Repeating the same request returns the existing identity without changing it.
pub async fn create(
    pool: &PgPool,
    mutation: &TimetableMutationContext,
    request: CreateTimetableRequest,
) -> Result<Timetable, AppError> {
    mutation.actor.require_permission(
        school_permissions::registry::codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
    )?;
    mutation.execute_creation(pool, request.academic_term_id, move |pool, permit| async move {
        let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "academic_timetable.current.create").await?;
        let (year, schedule): (Uuid, Uuid) = DbSession::new(&mut tx).query_as("academic_timetable.current.create.context", "SELECT academic_year_id,bell_schedule_id FROM academic_terms WHERE id=$1")
            .bind(request.academic_term_id).fetch_optional().await?.ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
        lifecycle_guard::require_term_write_exclusive(&mut tx, year, request.academic_term_id).await?;
        if let Some(current) = get_for_term_on_connection(&mut tx, request.academic_term_id).await? {
            permit.commit(tx).await?;
            return Ok(current);
        }
        let valid: bool = DbSession::new(&mut tx).scalar("academic_timetable.current.create.schedule", "SELECT EXISTS(SELECT 1 FROM bell_schedules WHERE id=$1 AND academic_year_id=$2 AND is_active)").bind(schedule).bind(year).fetch_one().await?;
        if !valid { return Err(AppError::ValidationError("ภาคเรียนต้องมีตารางคาบมาตรฐานที่ใช้งานได้".into())); }
        let id = Uuid::now_v7();
        DbSession::new(&mut tx).query("academic_timetable.current.create.insert", "INSERT INTO academic_timetables(id,academic_term_id,academic_year_id,bell_schedule_id) VALUES($1,$2,$3,$4)").bind(id).bind(request.academic_term_id).bind(year).bind(schedule).execute().await?;
        let current = get_by_id_on_connection(&mut tx, id).await?;
        permit.commit(tx).await?;
        Ok(current)
    }).await
}

/// Every board writer takes the lifecycle lock before locking the current aggregate.
pub async fn require_current_write(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), AppError> {
    let (year, term): (Uuid, Uuid) = DbSession::new(&mut **tx)
        .query_as(
            "academic_timetable.current.write_context",
            "SELECT academic_year_id,academic_term_id FROM academic_timetables WHERE id=$1",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางเรียน".into()))?;
    lifecycle_guard::require_term_write_exclusive(tx, year, term).await?;
    let _: Uuid = DbSession::new(&mut **tx)
        .scalar(
            "academic_timetable.current.lock",
            "SELECT id FROM academic_timetables WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_one()
        .await?;
    Ok(())
}
pub(crate) async fn refresh_content_revision_on_connection(
    connection: &mut PgConnection,
    current: Timetable,
) -> Result<Timetable, AppError> {
    get_by_id_on_connection(connection, current.id).await
}
/// A current board has no unpublished work or publication prerequisite for closure.
pub async fn pending_term_work(
    _tx: &mut Transaction<'_, Postgres>,
    _year: Uuid,
    _term: Uuid,
) -> Result<Vec<school_academic_core::ports::PendingTermWork>, AppError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn school_civil_date_uses_bangkok_boundary() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-14T18:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            bangkok_date_at(now),
            NaiveDate::from_ymd_opt(2026, 11, 15).unwrap()
        );
    }
}
