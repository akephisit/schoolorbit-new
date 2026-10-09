use super::{audit, lock_term};
use crate::{
    models::*,
    rules::{counted, invalid, validate_configuration},
};
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;
pub async fn get(pool: &PgPool, term: Uuid) -> Result<AttendanceSettings, AppError> {
    let archived: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance_term_archives WHERE academic_term_id=$1)",
    )
    .bind(term)
    .fetch_one(pool)
    .await?;
    let row: Option<(Json<AttendanceConfiguration>, i64)> = sqlx::query_as(
        "SELECT configuration,row_version FROM attendance_settings WHERE academic_term_id=$1",
    )
    .bind(term)
    .fetch_optional(pool)
    .await?;
    let (configuration, row_version) = row.map(|(c, v)| (c.0, v)).unwrap_or_default();
    Ok(AttendanceSettings {
        academic_term_id: term,
        configuration: if row_version == 0 {
            AttendanceConfiguration::default()
        } else {
            configuration
        },
        row_version,
        archived,
    })
}
pub async fn save(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    p: SaveAttendanceSettings,
) -> Result<AttendanceSettings, AppError> {
    validate_configuration(&p.configuration)?;
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, term, true, true).await?;
    let old: Option<i64> = sqlx::query_scalar(
        "SELECT row_version FROM attendance_settings WHERE academic_term_id=$1 FOR UPDATE",
    )
    .bind(term)
    .fetch_optional(&mut *tx)
    .await?;
    if old.unwrap_or(0) != p.row_version {
        return Err(AppError::Conflict("การตั้งค่าเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration,updated_by) VALUES($1,$2,$3) ON CONFLICT(academic_term_id) DO UPDATE SET configuration=excluded.configuration,row_version=attendance_settings.row_version+1,updated_by=excluded.updated_by,updated_at=now()").bind(term).bind(Json(p.configuration)).bind(actor).execute(&mut *tx).await?;
    audit(&mut tx, actor, term, "update", "attendance settings").await?;
    tx.commit().await?;
    get(pool, term).await
}
pub async fn days(
    pool: &PgPool,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendanceDay>, AppError> {
    if end < start || (end - start).num_days() > 370 {
        return Err(invalid("ช่วงปฏิทินต้องไม่เกินหนึ่งปี"));
    }
    Ok(sqlx::query_as("SELECT date,counted,note FROM attendance_days WHERE academic_term_id=$1 AND date BETWEEN $2 AND $3 ORDER BY date").bind(term).bind(start).bind(end).fetch_all(pool).await?)
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
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, term, true, true).await?;
    let (start,end):(NaiveDate,NaiveDate)=sqlx::query_as("SELECT t.start_date,COALESCE(t.closed_on,t.planned_end_date,y.end_date) FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1").bind(term).fetch_one(&mut *tx).await?;
    if p.days.iter().any(|d| d.date < start || d.date > end) {
        return Err(invalid("วันที่อยู่นอกภาคเรียน"));
    }
    let version: Option<i64> = sqlx::query_scalar(
        "SELECT row_version FROM attendance_settings WHERE academic_term_id=$1 FOR UPDATE",
    )
    .bind(term)
    .fetch_optional(&mut *tx)
    .await?;
    if version != Some(p.row_version) {
        return Err(AppError::Conflict(
            "บันทึกเกณฑ์พื้นฐานก่อน หรือโหลดการตั้งค่าใหม่".into(),
        ));
    }
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted,note) SELECT $1,date,counted,note FROM jsonb_to_recordset($2) AS d(date date,counted boolean,note text) ON CONFLICT(academic_term_id,date) DO UPDATE SET counted=excluded.counted,note=excluded.note").bind(term).bind(Json(p.days)).execute(&mut *tx).await?;
    sqlx::query("UPDATE attendance_settings SET row_version=row_version+1,updated_by=$2,updated_at=now() WHERE academic_term_id=$1").bind(term).bind(actor).execute(&mut *tx).await?;
    audit(
        &mut tx,
        actor,
        term,
        "update",
        "attendance processing dates",
    )
    .await?;
    tx.commit().await?;
    get(pool, term).await
}
pub async fn is_counted(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
    c: &AttendanceConfiguration,
) -> Result<bool, AppError> {
    let day: Option<bool> = sqlx::query_scalar(
        "SELECT counted FROM attendance_days WHERE academic_term_id=$1 AND date=$2",
    )
    .bind(term)
    .bind(date)
    .fetch_optional(pool)
    .await?;
    Ok(counted(c, date, day))
}
