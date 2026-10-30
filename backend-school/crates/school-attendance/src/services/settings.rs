use super::{audit, lock_term};
use crate::{
    models::*,
    rules::{counted, invalid, validate_configuration},
};
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool, types::Json};
use uuid::Uuid;
pub async fn get(pool: &PgPool, term: Uuid) -> Result<AttendanceSettings, AppError> {
    get_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.settings.read")
            .await?,
        term,
    )
    .await
}
pub async fn get_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<AttendanceSettings, AppError> {
    Ok(AttendanceReadContext::load(connection, term)
        .await?
        .settings)
}

/// Read once per database stage; callers retain their existing transaction guards.
pub(crate) struct AttendanceReadContext {
    pub settings: AttendanceSettings,
    pub term_writable: Option<bool>,
}
impl AttendanceReadContext {
    pub async fn load(connection: &mut PgConnection, term: Uuid) -> Result<Self, AppError> {
        let (configuration, row_version, archived, auto_finalize_from, term_writable):
            (Option<Json<AttendanceConfiguration>>, Option<i64>, bool, NaiveDate, Option<bool>) =
            DbSession::new(&mut *connection).query_as("attendance.services.settings.load", "SELECT s.configuration,s.row_version,EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=$1),m.auto_finalize_from,t.status NOT IN ('closed','cancelled') AND y.status NOT IN ('closed','archived') FROM attendance_module_state m LEFT JOIN attendance_settings s ON s.academic_term_id=$1 LEFT JOIN academic_terms t ON t.id=$1 LEFT JOIN academic_years y ON y.id=t.academic_year_id WHERE m.singleton")
                .bind(term).fetch_one().await?;
        let row_version = row_version.unwrap_or(0);
        Ok(Self {
            settings: AttendanceSettings {
                academic_term_id: term,
                configuration: if row_version == 0 {
                    AttendanceConfiguration::default()
                } else {
                    configuration.map(|v| v.0).unwrap_or_default()
                },
                row_version,
                archived,
                auto_finalize_from,
            },
            term_writable,
        })
    }
}

pub async fn save(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    p: SaveAttendanceSettings,
) -> Result<AttendanceSettings, AppError> {
    validate_configuration(&p.configuration)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.settings.transaction").await?;
    lock_term(&mut tx, term, true, true).await?;
    let old: Option<i64> = DbSession::new(&mut *tx)
        .scalar(
            "attendance.services.settings.save",
            "SELECT row_version FROM attendance_settings WHERE academic_term_id=$1 FOR UPDATE",
        )
        .bind(term)
        .fetch_optional()
        .await?;
    if old.unwrap_or(0) != p.row_version {
        return Err(AppError::Conflict("การตั้งค่าเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    if let Some(date) = p.auto_finalize_from {
        let current: NaiveDate = DbSession::new(&mut *tx)
            .scalar(
                "attendance.services.settings.save",
                "SELECT auto_finalize_from FROM attendance_module_state WHERE singleton FOR UPDATE",
            )
            .fetch_one()
            .await?;
        if date != current
            && (date < current
                || date
                    < chrono::Utc::now()
                        .with_timezone(&chrono_tz::Asia::Bangkok)
                        .date_naive())
        {
            return Err(invalid("วันเริ่มปิดยอดอัตโนมัติต้องไม่ย้อนก่อนวันเดิมหรือวันนี้"));
        }
        DbSession::new(&mut *tx)
            .query(
                "attendance.services.settings.save",
                "UPDATE attendance_module_state SET auto_finalize_from=$1 WHERE singleton",
            )
            .bind(date)
            .execute()
            .await?;
    }
    DbSession::new(&mut *tx).query("attendance.services.settings.save", "INSERT INTO attendance_settings(academic_term_id,configuration,updated_by) VALUES($1,$2,$3) ON CONFLICT(academic_term_id) DO UPDATE SET configuration=excluded.configuration,row_version=attendance_settings.row_version+1,updated_by=excluded.updated_by,updated_at=now()").bind(term).bind(Json(p.configuration)).bind(actor).execute().await?;
    audit(&mut tx, actor, term, "update", "attendance settings").await?;
    super::score_changes::enqueue_term_days(&mut tx, term).await?;
    DbSession::new(&mut *tx).query("attendance.services.settings.save", "DELETE FROM attendance_finalized_dates WHERE generation=(SELECT generation FROM attendance_module_state WHERE singleton) AND academic_term_id=$1 AND date>=(SELECT auto_finalize_from FROM attendance_module_state WHERE singleton)")
        .bind(term).execute().await?;
    tx.commit().await?;
    get(pool, term).await
}
fn validate_day_range(start: NaiveDate, end: NaiveDate) -> Result<(), AppError> {
    if end < start || (end - start).num_days() > 370 {
        return Err(invalid("ช่วงปฏิทินต้องไม่เกินหนึ่งปี"));
    }
    Ok(())
}
pub async fn days(
    pool: &PgPool,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendanceDay>, AppError> {
    validate_day_range(start, end)?;
    days_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.settings.read")
            .await?,
        term,
        start,
        end,
    )
    .await
}
pub async fn days_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendanceDay>, AppError> {
    validate_day_range(start, end)?;
    Ok(DbSession::new(&mut *connection).query_as("attendance.services.settings.days_on_connection", "SELECT date,counted,note FROM attendance_days WHERE academic_term_id=$1 AND date BETWEEN $2 AND $3 ORDER BY date").bind(term).bind(start).bind(end).fetch_all().await?)
}
pub async fn save_days(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    p: SaveAttendanceDays,
) -> Result<AttendanceSettings, AppError> {
    if p.days.is_empty() || p.days.len() > 366 || p.days.iter().any(|d| d.note.len() > 500) {
        return Err(invalid("รายการวันที่ไม่ถูกต้อง"));
    }
    let mut seen = std::collections::HashSet::new();
    for d in &p.days {
        if !seen.insert(d.date) {
            return Err(invalid("วันที่ซ้ำ"));
        }
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.settings.transaction").await?;
    lock_term(&mut tx, term, true, true).await?;
    let version: Option<i64> = DbSession::new(&mut *tx)
        .scalar(
            "attendance.services.settings.save_days",
            "SELECT row_version FROM attendance_settings WHERE academic_term_id=$1 FOR UPDATE",
        )
        .bind(term)
        .fetch_optional()
        .await?;
    if version.unwrap_or(0) != p.row_version {
        return Err(AppError::Conflict("การตั้งค่าเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    DbSession::new(&mut *tx).query("attendance.services.settings.save_days", "INSERT INTO attendance_days(academic_term_id,date,counted,note) SELECT $1,date,counted,note FROM jsonb_to_recordset($2) AS d(date date,counted boolean,note text) ON CONFLICT(academic_term_id,date) DO UPDATE SET counted=excluded.counted,note=excluded.note").bind(term).bind(Json(&p.days)).execute().await?;
    DbSession::new(&mut *tx).query("attendance.services.settings.save_days", "INSERT INTO attendance_settings(academic_term_id,configuration,updated_by) VALUES($1,$2,$3) ON CONFLICT(academic_term_id) DO UPDATE SET row_version=attendance_settings.row_version+1,updated_by=excluded.updated_by,updated_at=now()")
        .bind(term)
        .bind(Json(AttendanceConfiguration::default()))
        .bind(actor)
        .execute()
        .await?;
    audit(
        &mut tx,
        actor,
        term,
        "update",
        "attendance processing dates",
    )
    .await?;
    let dates = p.days.iter().map(|d| d.date).collect::<Vec<_>>();
    super::score_changes::enqueue_dates(&mut tx, term, &dates).await?;
    DbSession::new(&mut *tx).query("attendance.services.settings.save_days", "DELETE FROM attendance_finalized_dates WHERE generation=(SELECT generation FROM attendance_module_state WHERE singleton) AND academic_term_id=$1 AND date=ANY($2)")
        .bind(term).bind(&dates).execute().await?;
    tx.commit().await?;
    get(pool, term).await
}
pub async fn is_counted(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
    c: &AttendanceConfiguration,
) -> Result<bool, AppError> {
    is_counted_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.settings.read")
            .await?,
        term,
        date,
        c,
    )
    .await
}
pub(crate) async fn is_counted_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
    c: &AttendanceConfiguration,
) -> Result<bool, AppError> {
    let day: Option<bool> = DbSession::new(&mut *connection)
        .scalar(
            "attendance.services.settings.is_counted_on_connection",
            "SELECT counted FROM attendance_days WHERE academic_term_id=$1 AND date=$2",
        )
        .bind(term)
        .bind(date)
        .fetch_optional()
        .await?;
    Ok(counted(c, date, day))
}
