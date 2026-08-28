use std::collections::HashSet;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{
    BellSchedule, BellSchedulePeriod, BellSchedulePeriodInput, CreateBellScheduleRequest,
    ReplaceBellSchedulePeriodsRequest, UpdateBellScheduleRequest,
};
use super::parse_row_version;
use super::years_terms::append_audit;

const SCHEDULE_COLUMNS: &str = r#"
    id, academic_year_id, code, name, is_default, status,
    owning_organization_unit_id, row_version, created_at, updated_at
"#;
const SCHOOL_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

pub async fn list(pool: &PgPool, academic_year_id: Uuid) -> Result<Vec<BellSchedule>, AppError> {
    let sql = format!(
        "SELECT {SCHEDULE_COLUMNS} FROM bell_schedules WHERE academic_year_id = $1 \
         ORDER BY is_default DESC, code, id"
    );
    Ok(sqlx::query_as(&sql)
        .bind(academic_year_id)
        .fetch_all(pool)
        .await?)
}

pub(super) async fn list_all(pool: &PgPool) -> Result<Vec<BellSchedule>, AppError> {
    let sql = format!(
        "SELECT schedule.* FROM (SELECT {SCHEDULE_COLUMNS} FROM bell_schedules) schedule \
         JOIN academic_years year ON year.id = schedule.academic_year_id \
         ORDER BY year.year DESC, year.id, schedule.is_default DESC, schedule.code, schedule.id"
    );
    Ok(sqlx::query_as(&sql).fetch_all(pool).await?)
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<BellSchedule, AppError> {
    let sql = format!("SELECT {SCHEDULE_COLUMNS} FROM bell_schedules WHERE id = $1");
    sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))
}

pub async fn create(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateBellScheduleRequest,
) -> Result<BellSchedule, AppError> {
    validate_schedule_fields(&request.name)?;
    let mut transaction = pool.begin().await?;
    require_planning_year(&mut transaction, request.academic_year_id).await?;
    let schedule_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM bell_schedules WHERE academic_year_id = $1")
            .bind(request.academic_year_id)
            .fetch_one(&mut *transaction)
            .await?;
    let is_default = schedule_count == 0;
    let code = if is_default {
        "DEFAULT".to_string()
    } else {
        format!("SCHEDULE-{}", schedule_count + 1)
    };
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO bell_schedules (id, academic_year_id, code, name, is_default, status, \
         owning_organization_unit_id) VALUES ($1, $2, $3, $4, $5, 'draft', $6) \
         RETURNING {SCHEDULE_COLUMNS}"
    );
    let schedule: BellSchedule = sqlx::query_as(&sql)
        .bind(id)
        .bind(request.academic_year_id)
        .bind(code)
        .bind(request.name.trim())
        .bind(is_default)
        .bind(request.owning_organization_unit_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_schedule_write_error)?;
    append_audit(
        &mut transaction,
        "bell_schedule.created",
        "bell_schedule",
        id,
        Some(request.academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({"isDefault": is_default}),
    )
    .await?;
    transaction.commit().await?;
    Ok(schedule)
}

pub async fn update(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateBellScheduleRequest,
) -> Result<BellSchedule, AppError> {
    validate_schedule_fields(&request.name)?;
    parse_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let (academic_year_id, current_is_default): (Uuid, bool) = sqlx::query_as(
        "SELECT academic_year_id, is_default FROM bell_schedules WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))?;
    require_planning_year(&mut transaction, academic_year_id).await?;
    if request.is_default {
        clear_default_except(&mut transaction, academic_year_id, id).await?;
    } else if current_is_default {
        return Err(AppError::ValidationError(
            "กรุณาตั้งตารางเวลาอื่นเป็นตารางหลักก่อน".to_string(),
        ));
    }
    let sql = format!(
        "UPDATE bell_schedules SET name = $1, is_default = $2, owning_organization_unit_id = $3, \
         row_version = row_version + 1, updated_at = now() \
         WHERE id = $4 AND row_version = $5 RETURNING {SCHEDULE_COLUMNS}"
    );
    let schedule: BellSchedule = sqlx::query_as(&sql)
        .bind(request.name.trim())
        .bind(request.is_default)
        .bind(request.owning_organization_unit_id)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_schedule_write_error)?
        .ok_or_else(|| AppError::Conflict("ตารางคาบถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))?;
    append_audit(
        &mut transaction,
        "bell_schedule.updated",
        "bell_schedule",
        id,
        Some(academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({"rowVersion": schedule.row_version}),
    )
    .await?;
    transaction.commit().await?;
    Ok(schedule)
}

pub async fn list_periods(
    pool: &PgPool,
    schedule_id: Uuid,
) -> Result<Vec<BellSchedulePeriod>, AppError> {
    ensure_schedule_exists(pool, schedule_id).await?;
    Ok(sqlx::query_as(
        r#"
        SELECT id, bell_schedule_id, name, start_time, end_time, order_index,
               applicable_days, is_active
        FROM bell_schedule_periods
        WHERE bell_schedule_id = $1
        ORDER BY order_index, start_time, id
        "#,
    )
    .bind(schedule_id)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_periods(
    pool: &PgPool,
    actor_user_id: Uuid,
    schedule_id: Uuid,
    request: ReplaceBellSchedulePeriodsRequest,
) -> Result<Vec<BellSchedulePeriod>, AppError> {
    parse_row_version(request.row_version)?;
    validate_periods(&request.periods)?;
    let mut transaction = pool.begin().await?;
    let (academic_year_id, actual_version): (Uuid, i64) = sqlx::query_as(
        "SELECT academic_year_id, row_version FROM bell_schedules WHERE id = $1 FOR UPDATE",
    )
    .bind(schedule_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))?;
    require_planning_year(&mut transaction, academic_year_id).await?;
    let configured_school_days: String =
        sqlx::query_scalar("SELECT school_days FROM academic_years WHERE id = $1")
            .bind(academic_year_id)
            .fetch_one(&mut *transaction)
            .await?;
    validate_period_days_for_year(&request.periods, &configured_school_days)?;
    if actual_version != request.row_version {
        return Err(AppError::Conflict("ตารางคาบถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    sqlx::query("DELETE FROM bell_schedule_periods WHERE bell_schedule_id = $1")
        .bind(schedule_id)
        .execute(&mut *transaction)
        .await?;
    for period in request.periods {
        sqlx::query(
            r#"
            INSERT INTO bell_schedule_periods (
                id, bell_schedule_id, name, start_time, end_time, order_index,
                applicable_days, is_active
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(schedule_id)
        .bind(period.name)
        .bind(period.start_time)
        .bind(period.end_time)
        .bind(period.order_index)
        .bind(normalize_period_days(&period.applicable_days)?.join(","))
        .bind(period.is_active)
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query(
        "UPDATE bell_schedules SET row_version = row_version + 1, updated_at = now() WHERE id = $1",
    )
    .bind(schedule_id)
    .execute(&mut *transaction)
    .await?;
    append_audit(
        &mut transaction,
        "bell_schedule.periods_replaced",
        "bell_schedule",
        schedule_id,
        Some(academic_year_id),
        None,
        actor_user_id,
        serde_json::json!({}),
    )
    .await?;
    transaction.commit().await?;
    list_periods(pool, schedule_id).await
}

fn validate_schedule_fields(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::ValidationError("ชื่อตารางเวลาห้ามว่าง".to_string()));
    }
    Ok(())
}

pub(crate) fn validate_periods(periods: &[BellSchedulePeriodInput]) -> Result<(), AppError> {
    let mut order_indexes = std::collections::HashSet::new();
    for period in periods {
        if period.order_index <= 0 || !order_indexes.insert(period.order_index) {
            return Err(AppError::ValidationError(
                "ลำดับคาบต้องเป็นจำนวนบวกและห้ามซ้ำ".to_string(),
            ));
        }
        if period.start_time >= period.end_time {
            return Err(AppError::ValidationError(
                "เวลาเริ่มคาบต้องอยู่ก่อนเวลาสิ้นสุด".to_string(),
            ));
        }
        normalize_period_days(&period.applicable_days)?;
    }
    for (index, period) in periods.iter().enumerate() {
        if !period.is_active {
            continue;
        }
        let days = period
            .applicable_days
            .iter()
            .map(|day| day.trim().to_ascii_uppercase())
            .collect::<HashSet<_>>();
        for other in periods.iter().skip(index + 1).filter(|item| item.is_active) {
            let shares_day = other
                .applicable_days
                .iter()
                .map(|day| day.trim().to_ascii_uppercase())
                .any(|day| days.contains(&day));
            let overlaps = period.start_time < other.end_time && other.start_time < period.end_time;
            if shares_day && overlaps {
                return Err(AppError::ValidationError(
                    "ช่วงเวลาคาบเรียนห้ามซ้อนกันในวันเดียวกัน".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn normalize_period_days(days: &[String]) -> Result<Vec<String>, AppError> {
    if days.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องเลือกวันที่ใช้คาบเรียนอย่างน้อยหนึ่งวัน".to_string(),
        ));
    }
    let mut normalized = Vec::new();
    for supported in SCHOOL_DAYS {
        let count = days
            .iter()
            .filter(|day| day.trim().eq_ignore_ascii_case(supported))
            .count();
        if count > 1 {
            return Err(AppError::ValidationError("วันที่ใช้คาบเรียนห้ามซ้ำกัน".to_string()));
        }
        if count == 1 {
            normalized.push((*supported).to_string());
        }
    }
    if normalized.len() != days.len() {
        return Err(AppError::ValidationError("วันที่ใช้คาบเรียนไม่ถูกต้อง".to_string()));
    }
    Ok(normalized)
}

fn validate_period_days_for_year(
    periods: &[BellSchedulePeriodInput],
    configured_school_days: &str,
) -> Result<(), AppError> {
    let allowed = configured_school_days.split(',').collect::<HashSet<_>>();
    if periods.iter().any(|period| {
        period
            .applicable_days
            .iter()
            .any(|day| !allowed.contains(day.trim().to_ascii_uppercase().as_str()))
    }) {
        return Err(AppError::ValidationError(
            "วันที่ใช้คาบเรียนต้องเป็นวันเรียนของปีการศึกษา".to_string(),
        ));
    }
    Ok(())
}

async fn require_planning_year(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM academic_years WHERE id = $1 FOR UPDATE")
            .bind(academic_year_id)
            .fetch_optional(&mut **transaction)
            .await?;
    match status.as_deref() {
        None => Err(AppError::NotFound("ไม่พบปีการศึกษา".to_string())),
        Some("planning") => Ok(()),
        Some(_) => Err(AppError::Conflict(
            "แก้ไขตารางคาบได้เฉพาะปีการศึกษาสถานะ planning".to_string(),
        )),
    }
}

async fn clear_default_except(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    schedule_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE bell_schedules SET is_default = false WHERE academic_year_id = $1 AND id <> $2",
    )
    .bind(academic_year_id)
    .bind(schedule_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn ensure_schedule_exists(pool: &PgPool, schedule_id: Uuid) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM bell_schedules WHERE id = $1)")
            .bind(schedule_id)
            .fetch_one(pool)
            .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("ไม่พบตารางคาบ".to_string()))
    }
}

fn map_schedule_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("bell_schedules_academic_year_id_code_key") {
            return AppError::Conflict("รหัสตารางคาบซ้ำในปีการศึกษา".to_string());
        }
    }
    AppError::DbError(error)
}
