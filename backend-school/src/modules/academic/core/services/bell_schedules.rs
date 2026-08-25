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
    validate_schedule_fields(&request.code, &request.name)?;
    let mut transaction = pool.begin().await?;
    require_planning_year(&mut transaction, request.academic_year_id).await?;
    if request.is_default {
        clear_default(&mut transaction, request.academic_year_id).await?;
    }
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO bell_schedules (id, academic_year_id, code, name, is_default, status, \
         owning_organization_unit_id) VALUES ($1, $2, $3, $4, $5, 'draft', $6) \
         RETURNING {SCHEDULE_COLUMNS}"
    );
    let schedule: BellSchedule = sqlx::query_as(&sql)
        .bind(id)
        .bind(request.academic_year_id)
        .bind(request.code.trim().to_uppercase())
        .bind(request.name.trim())
        .bind(request.is_default)
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
        serde_json::json!({"isDefault": request.is_default}),
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
    validate_schedule_fields(&request.code, &request.name)?;
    parse_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let academic_year_id: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM bell_schedules WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))?;
    require_planning_year(&mut transaction, academic_year_id).await?;
    if request.is_default {
        clear_default_except(&mut transaction, academic_year_id, id).await?;
    }
    let sql = format!(
        "UPDATE bell_schedules SET code = $1, name = $2, is_default = $3, \
         owning_organization_unit_id = $4, row_version = row_version + 1, updated_at = now() \
         WHERE id = $5 AND row_version = $6 RETURNING {SCHEDULE_COLUMNS}"
    );
    let schedule: BellSchedule = sqlx::query_as(&sql)
        .bind(request.code.trim().to_uppercase())
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
        .bind(period.applicable_days.join(","))
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

fn validate_schedule_fields(code: &str, name: &str) -> Result<(), AppError> {
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::ValidationError(
            "รหัสและชื่อตารางคาบห้ามว่าง".to_string(),
        ));
    }
    Ok(())
}

fn validate_periods(periods: &[BellSchedulePeriodInput]) -> Result<(), AppError> {
    let mut order_indexes = std::collections::HashSet::new();
    let mut previous_end = None;
    let mut sorted = periods.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|period| period.order_index);
    for period in sorted {
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
        if previous_end.is_some_and(|end| period.start_time < end) {
            return Err(AppError::ValidationError(
                "ช่วงเวลาคาบเรียนห้ามซ้อนกัน".to_string(),
            ));
        }
        previous_end = Some(period.end_time);
    }
    Ok(())
}

async fn require_planning_year(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM academic_years WHERE id = $1 FOR SHARE")
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

async fn clear_default(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("UPDATE bell_schedules SET is_default = false WHERE academic_year_id = $1")
        .bind(academic_year_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
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
