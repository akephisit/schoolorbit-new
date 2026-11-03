use school_database::DbSession;
use std::collections::HashSet;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use school_errors::AppError;

use super::super::models::{
    AcademicYearStatus, BellSchedule, BellScheduleEditBlockReason, BellSchedulePeriod,
    BellSchedulePeriodInput, BellSchedulePeriodsSnapshot, CloneBellScheduleRequest,
    CreateBellScheduleRequest, ReplaceBellSchedulePeriodsRequest, UpdateBellScheduleRequest,
};
use super::years_terms::append_audit;
use super::{lifecycle_guard, parse_row_version};

const SCHEDULE_COLUMNS: &str = r#"
    id, academic_year_id, code, name, is_default, status,
    owning_organization_unit_id, row_version, created_at, updated_at
"#;
const SCHOOL_DAYS: &[&str] = &super::school_days::ALL;

pub async fn list(pool: &PgPool, academic_year_id: Uuid) -> Result<Vec<BellSchedule>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.bell_schedules.list",
    )
    .await?;
    list_on_connection(&mut connection, academic_year_id).await
}

pub async fn list_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
) -> Result<Vec<BellSchedule>, AppError> {
    let sql = format!(
        "SELECT {SCHEDULE_COLUMNS} FROM bell_schedules WHERE academic_year_id = $1 \
         ORDER BY is_default DESC, code, id"
    );
    let mut schedules = DbSession::new(connection)
        .query_as(
            "academic_core.services.bell_schedules.list_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(academic_year_id)
        .fetch_all()
        .await?;
    attach_editability(connection, &mut schedules).await?;
    Ok(schedules)
}

pub(super) async fn list_all(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<BellSchedule>, AppError> {
    let sql = format!(
        "SELECT schedule.* FROM (SELECT {SCHEDULE_COLUMNS} FROM bell_schedules) schedule \
         JOIN academic_years year ON year.id = schedule.academic_year_id \
         ORDER BY year.year DESC, year.id, schedule.is_default DESC, schedule.code, schedule.id"
    );
    let mut schedules = DbSession::new(connection)
        .query_as(
            "academic_core.services.bell_schedules.list_all",
            sqlx::AssertSqlSafe(sql),
        )
        .fetch_all()
        .await?;
    attach_editability(connection, &mut schedules).await?;
    Ok(schedules)
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<BellSchedule, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.bell_schedules.get",
    )
    .await?;
    get_on_connection(&mut connection, id).await
}

pub async fn get_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<BellSchedule, AppError> {
    let sql = format!("SELECT {SCHEDULE_COLUMNS} FROM bell_schedules WHERE id = $1");
    let mut schedule: BellSchedule = DbSession::new(connection)
        .query_as(
            "academic_core.services.bell_schedules.get_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))?;
    attach_editability(connection, std::slice::from_mut(&mut schedule)).await?;
    Ok(schedule)
}

pub async fn create(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateBellScheduleRequest,
) -> Result<BellSchedule, AppError> {
    validate_schedule_fields(&request.name)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.bell_schedules.create",
    )
    .await?;
    require_setup_year(&mut transaction, request.academic_year_id).await?;
    let schedule_count: i64 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.bell_schedules.create",
            "SELECT count(*) FROM bell_schedules WHERE academic_year_id = $1",
        )
        .bind(request.academic_year_id)
        .fetch_one()
        .await?;
    let is_default = schedule_count == 0;
    let code = if is_default {
        "DEFAULT".to_string()
    } else {
        format!("SCHEDULE-{}", schedule_count + 1)
    };
    let id = Uuid::now_v7();
    let sql = format!(
        "INSERT INTO bell_schedules (id, academic_year_id, code, name, is_default, status, \
         owning_organization_unit_id) VALUES ($1, $2, $3, $4, $5, 'draft', $6) \
         RETURNING {SCHEDULE_COLUMNS}"
    );
    let mut schedule: BellSchedule = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.bell_schedules.create",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .bind(request.academic_year_id)
        .bind(code)
        .bind(request.name.trim())
        .bind(is_default)
        .bind(request.owning_organization_unit_id)
        .fetch_one()
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
    attach_editability(&mut transaction, std::slice::from_mut(&mut schedule)).await?;
    transaction.commit().await?;
    Ok(schedule)
}

pub async fn clone_schedule(
    pool: &PgPool,
    actor_user_id: Uuid,
    source_id: Uuid,
    request: CloneBellScheduleRequest,
) -> Result<BellSchedule, AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.bell_schedules.clone_schedule",
    )
    .await?;
    validate_schedule_fields(&request.name)?;
    parse_row_version(request.row_version)?;
    lifecycle_guard::lock_transition_shared(&mut transaction).await?;
    let source_year: Uuid = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.bell_schedules.clone_schedule",
            "SELECT academic_year_id FROM bell_schedules WHERE id=$1",
        )
        .bind(source_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบต้นฉบับ".into()))?;
    // Source may be historical. Lock both parents in a stable order without
    // applying the destination's writable-state rule to the historical source.
    let _: Vec<Uuid> = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.bell_schedules.clone_schedule",
            "SELECT id FROM academic_years WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(vec![source_year, request.academic_year_id])
        .fetch_all()
        .await?;
    require_setup_year(&mut transaction, request.academic_year_id).await?;
    let source = capture_copy_source(
        &mut transaction,
        source_id,
        source_year,
        request.row_version,
    )
    .await?;
    let mut schedule =
        clone_captured_schedule(&mut transaction, actor_user_id, &source, request).await?;
    attach_editability(&mut transaction, std::slice::from_mut(&mut schedule)).await?;
    transaction.commit().await?;
    Ok(schedule)
}

/// Constructed only after the caller locks the source and destination years.
/// Keep this evidence on the same transaction until the copy commits.
pub(super) struct CapturedScheduleCopySource {
    id: Uuid,
    academic_year_id: Uuid,
    row_version: i64,
    periods: Vec<BellSchedulePeriod>,
}

impl CapturedScheduleCopySource {
    pub(super) fn periods(&self) -> &[BellSchedulePeriod] {
        &self.periods
    }
}

pub(super) async fn capture_copy_source(
    transaction: &mut Transaction<'_, Postgres>,
    source_id: Uuid,
    expected_year: Uuid,
    expected_version: i64,
) -> Result<CapturedScheduleCopySource, AppError> {
    parse_row_version(expected_version)?;
    let (academic_year_id, row_version): (Uuid, i64) = DbSession::new(&mut **transaction)
        .query_as(
            "academic_core.services.bell_schedules.capture_copy_source",
            "SELECT academic_year_id,row_version FROM bell_schedules WHERE id=$1 AND academic_year_id=$2 FOR SHARE",
        )
        .bind(source_id)
        .bind(expected_year)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::Conflict(
            "ตารางคาบต้นฉบับไม่อยู่ในปีการศึกษาที่ตรวจไว้ กรุณาโหลดใหม่ก่อนทำสำเนา".into(),
        ))?;
    if row_version != expected_version {
        return Err(AppError::Conflict(
            "ตารางคาบต้นฉบับเปลี่ยนแล้ว กรุณาโหลดใหม่ก่อนทำสำเนา".into(),
        ));
    }
    let source_periods = list_periods_for_copy(transaction, source_id).await?;
    let inputs = source_periods
        .iter()
        .map(|period| {
            Ok(BellSchedulePeriodInput {
                name: period.name.clone(),
                start_time: period.start_time,
                end_time: period.end_time,
                order_index: period.order_index,
                applicable_days: period
                    .applicable_days
                    .as_deref()
                    .map(super::school_days::parse_stored)
                    .transpose()?,
                is_active: period.is_active,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    validate_periods(&inputs)?;
    Ok(CapturedScheduleCopySource {
        id: source_id,
        academic_year_id,
        row_version,
        periods: source_periods,
    })
}

/// The owning entry point has checked its destination policy under the parent
/// locks. Source identity, revision and periods come from the same transaction.
pub(super) async fn clone_captured_schedule(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    source: &CapturedScheduleCopySource,
    request: CloneBellScheduleRequest,
) -> Result<BellSchedule, AppError> {
    validate_schedule_fields(&request.name)?;
    if source.row_version != request.row_version {
        return Err(AppError::Conflict(
            "ตารางคาบต้นฉบับเปลี่ยนแล้ว กรุณาโหลดใหม่ก่อนทำสำเนา".into(),
        ));
    }
    let count: i64 = DbSession::new(&mut **transaction)
        .scalar(
            "academic_core.services.bell_schedules.clone_schedule",
            "SELECT count(*) FROM bell_schedules WHERE academic_year_id=$1",
        )
        .bind(request.academic_year_id)
        .fetch_one()
        .await?;
    let id = Uuid::now_v7();
    let sql = format!("INSERT INTO bell_schedules (id,academic_year_id,code,name,is_default,status,owning_organization_unit_id) \
        VALUES($1,$2,$3,$4,$5,'draft',$6) RETURNING {SCHEDULE_COLUMNS}");
    let schedule: BellSchedule = DbSession::new(&mut **transaction)
        .query_as(
            "academic_core.services.bell_schedules.clone_schedule",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .bind(request.academic_year_id)
        .bind(if count == 0 {
            "DEFAULT".to_string()
        } else {
            format!("SCHEDULE-{}", count + 1)
        })
        .bind(request.name.trim())
        .bind(count == 0)
        .bind(request.owning_organization_unit_id)
        .fetch_one()
        .await
        .map_err(map_schedule_write_error)?;
    DbSession::new(&mut **transaction).query("academic_core.services.bell_schedules.clone_schedule",
        "INSERT INTO bell_schedule_periods (id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days,is_active) \
         SELECT uuidv7(),$1,name,start_time,end_time,order_index,applicable_days,is_active FROM bell_schedule_periods WHERE bell_schedule_id=$2")
        .bind(id).bind(source.id).execute().await?;
    append_audit(transaction, "bell_schedule.cloned", "bell_schedule", id,
        Some(request.academic_year_id), None, actor_user_id,
        serde_json::json!({"sourceBellScheduleId": source.id, "sourceAcademicYearId": source.academic_year_id, "sourceRowVersion": source.row_version, "periodCount": source.periods.len()})).await?;
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
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.bell_schedules.update",
    )
    .await?;
    require_schedule_editable(&mut transaction, id).await?;
    let (academic_year_id, current_is_default): (Uuid, bool) = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.bell_schedules.update",
            "SELECT academic_year_id, is_default FROM bell_schedules WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))?;
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
    let mut schedule: BellSchedule = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.bell_schedules.update",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(request.name.trim())
        .bind(request.is_default)
        .bind(request.owning_organization_unit_id)
        .bind(id)
        .bind(request.row_version)
        .fetch_optional()
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
    attach_editability(&mut transaction, std::slice::from_mut(&mut schedule)).await?;
    transaction.commit().await?;
    Ok(schedule)
}

pub async fn list_periods(
    pool: &PgPool,
    schedule_id: Uuid,
) -> Result<BellSchedulePeriodsSnapshot, AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.bell_schedules.list_periods",
    )
    .await?;
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.bell_schedules.list_periods",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let row_version: i64 = DbSession::new(&mut *transaction)
        .scalar(
            "academic_core.services.bell_schedules.list_periods",
            "SELECT row_version FROM bell_schedules WHERE id=$1",
        )
        .bind(schedule_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".into()))?;
    let periods = list_periods_on_connection(&mut transaction, schedule_id).await?;
    transaction.commit().await?;
    Ok(BellSchedulePeriodsSnapshot {
        bell_schedule_id: schedule_id,
        row_version,
        periods,
    })
}

pub(crate) async fn list_periods_on_connection(
    connection: &mut sqlx::PgConnection,
    schedule_id: Uuid,
) -> Result<Vec<BellSchedulePeriod>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_core.services.bell_schedules.list_periods_on_connection",
            r#"
        SELECT id, bell_schedule_id, name, start_time, end_time, order_index,
               applicable_days, is_active
        FROM bell_schedule_periods
        WHERE bell_schedule_id = $1
        ORDER BY order_index, start_time, id
        "#,
        )
        .bind(schedule_id)
        .fetch_all()
        .await?)
}

pub(super) async fn list_periods_for_copy(
    connection: &mut sqlx::PgConnection,
    schedule_id: Uuid,
) -> Result<Vec<BellSchedulePeriod>, AppError> {
    let periods: Vec<BellSchedulePeriod> = DbSession::new(connection)
        .query_as("academic_core.services.bell_schedules.list_periods_for_copy",
            "SELECT id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days,is_active FROM bell_schedule_periods WHERE bell_schedule_id=$1 ORDER BY order_index,start_time,id LIMIT 1001")
        .bind(schedule_id).fetch_all().await?;
    if periods.len() > 1_000 {
        return Err(AppError::ValidationError(
            "ตารางคาบต้นฉบับมีคาบเกิน 1,000 คาบ".into(),
        ));
    }
    Ok(periods)
}

#[derive(Debug)]
pub struct BellSchedulePeriodsWriteOutcome {
    pub academic_year_id: Uuid,
    pub snapshot: BellSchedulePeriodsSnapshot,
}

pub async fn replace_periods(
    pool: &PgPool,
    actor_user_id: Uuid,
    schedule_id: Uuid,
    request: ReplaceBellSchedulePeriodsRequest,
) -> Result<BellSchedulePeriodsWriteOutcome, AppError> {
    parse_row_version(request.row_version)?;
    validate_periods(&request.periods)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.bell_schedules.replace_periods",
    )
    .await?;
    require_schedule_editable(&mut transaction, schedule_id).await?;
    let (academic_year_id, actual_version): (Uuid, i64) = DbSession::new(&mut *transaction)
        .query_as(
            "academic_core.services.bell_schedules.replace_periods",
            "SELECT academic_year_id, row_version FROM bell_schedules WHERE id = $1 FOR UPDATE",
        )
        .bind(schedule_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".to_string()))?;
    if actual_version != request.row_version {
        return Err(AppError::Conflict("ตารางคาบถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()));
    }
    DbSession::new(&mut *transaction)
        .query(
            "academic_core.services.bell_schedules.replace_periods",
            "DELETE FROM bell_schedule_periods WHERE bell_schedule_id = $1",
        )
        .bind(schedule_id)
        .execute()
        .await?;
    for period in request.periods {
        DbSession::new(&mut *transaction)
            .query(
                "academic_core.services.bell_schedules.replace_periods",
                r#"
            INSERT INTO bell_schedule_periods (
                id, bell_schedule_id, name, start_time, end_time, order_index,
                applicable_days, is_active
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            )
            .bind(Uuid::now_v7())
            .bind(schedule_id)
            .bind(period.name)
            .bind(period.start_time)
            .bind(period.end_time)
            .bind(period.order_index)
            .bind(
                period
                    .applicable_days
                    .as_deref()
                    .map(normalize_period_days)
                    .transpose()?
                    .map(|days| days.join(",")),
            )
            .bind(period.is_active)
            .execute()
            .await?;
    }
    let row_version: i64 = DbSession::new(&mut *transaction).scalar("academic_core.services.bell_schedules.replace_periods",
        "UPDATE bell_schedules SET row_version = row_version + 1, updated_at = now() WHERE id = $1 RETURNING row_version",
    )
    .bind(schedule_id)
    .fetch_one()
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
    let periods = list_periods_on_connection(&mut transaction, schedule_id).await?;
    let outcome = BellSchedulePeriodsWriteOutcome {
        academic_year_id,
        snapshot: BellSchedulePeriodsSnapshot {
            bell_schedule_id: schedule_id,
            row_version,
            periods,
        },
    };
    transaction.commit().await?;
    Ok(outcome)
}

fn validate_schedule_fields(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::ValidationError("ชื่อตารางเวลาห้ามว่าง".to_string()));
    }
    Ok(())
}

pub fn validate_periods(periods: &[BellSchedulePeriodInput]) -> Result<(), AppError> {
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
        if let Some(days) = &period.applicable_days {
            normalize_period_days(days)?;
        }
    }
    for (index, period) in periods.iter().enumerate() {
        if !period.is_active {
            continue;
        }
        let all_days = SCHOOL_DAYS
            .iter()
            .map(|day| (*day).to_owned())
            .collect::<Vec<_>>();
        let days = period
            .applicable_days
            .as_ref()
            .unwrap_or(&all_days)
            .iter()
            .map(|day| day.trim().to_ascii_uppercase())
            .collect::<HashSet<_>>();
        for other in periods.iter().skip(index + 1).filter(|item| item.is_active) {
            let shares_day = other
                .applicable_days
                .as_ref()
                .unwrap_or(&all_days)
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

const ASSIGNABLE_TERMS_SQL: &str = r#"SELECT term.id FROM academic_terms term
    JOIN academic_years year ON year.id=term.academic_year_id
    WHERE ($1::uuid IS NULL OR term.id=$1)
      AND year.status IN ('planning','ready','active') AND term.status IN ('planning','ready')
      AND NOT EXISTS(SELECT 1 FROM academic_term_transition_receipts receipt
          WHERE receipt.academic_term_id=term.id AND receipt.action='activate')
      AND NOT EXISTS(SELECT 1 FROM academic_timetable_versions version WHERE version.academic_term_id=term.id)
      AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.academic_term_id=term.id)
    ORDER BY year.year,term.sequence_no,term.id"#;

pub(crate) async fn setup_term_ids(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<Uuid>, AppError> {
    Ok(DbSession::new(connection)
        .scalar(
            "academic_core.services.bell_schedules.setup_term_ids",
            ASSIGNABLE_TERMS_SQL,
        )
        .bind(None::<Uuid>)
        .fetch_all()
        .await?)
}

pub(crate) async fn term_schedule_is_assignable(
    connection: &mut sqlx::PgConnection,
    term: Uuid,
) -> Result<bool, AppError> {
    Ok(DbSession::new(connection)
        .scalar::<Uuid>(
            "academic_core.services.bell_schedules.term_schedule_is_assignable",
            ASSIGNABLE_TERMS_SQL,
        )
        .bind(Some(term))
        .fetch_optional()
        .await?
        .is_some())
}

pub(crate) async fn setup_year_ids(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<Uuid>, AppError> {
    Ok(DbSession::new(connection)
        .scalar(
            "academic_core.services.bell_schedules.setup_year_ids",
            r#"SELECT year.id FROM academic_years year
               WHERE year.status IN ('planning','ready') OR (year.status='active' AND EXISTS (
                   SELECT 1 FROM academic_terms term WHERE term.academic_year_id=year.id
                     AND term.status IN ('planning','ready') AND NOT EXISTS (
                         SELECT 1 FROM academic_term_transition_receipts receipt
                         WHERE receipt.academic_term_id=term.id AND receipt.action='activate')))
               ORDER BY year.year,year.id"#,
        )
        .fetch_all()
        .await?)
}

pub(super) async fn require_setup_year(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let status =
        lifecycle_guard::require_year_write_exclusive(transaction, academic_year_id).await?;
    if matches!(
        status,
        AcademicYearStatus::Planning | AcademicYearStatus::Ready
    ) {
        return Ok(());
    }
    if status == AcademicYearStatus::Active
        && setup_year_ids(transaction)
            .await?
            .contains(&academic_year_id)
    {
        return Ok(());
    }
    Err(AppError::Conflict(
        "จัดตารางคาบได้ในปีที่กำลังเตรียม หรือปีที่ใช้งานซึ่งมีภาคเรียนที่ยังไม่เคยเปิด".into(),
    ))
}

#[derive(sqlx::FromRow)]
struct ScheduleEditState {
    id: Uuid,
    year_status: AcademicYearStatus,
    has_unopened_term: bool,
    already_used: bool,
    pinned_timetable: bool,
    timetable_references: bool,
}

impl ScheduleEditState {
    fn blocked_reason(&self) -> Option<BellScheduleEditBlockReason> {
        use BellScheduleEditBlockReason as Reason;
        if !matches!(
            self.year_status,
            AcademicYearStatus::Planning | AcademicYearStatus::Ready | AcademicYearStatus::Active
        ) {
            Some(Reason::YearNotEditable)
        } else if self.already_used {
            Some(Reason::AlreadyUsed)
        } else if self.pinned_timetable {
            Some(Reason::PinnedTimetable)
        } else if self.timetable_references {
            Some(Reason::TimetableReferences)
        } else if self.year_status == AcademicYearStatus::Active && !self.has_unopened_term {
            Some(Reason::NoUnopenedTerm)
        } else {
            None
        }
    }
}

pub(super) async fn attach_editability(
    connection: &mut sqlx::PgConnection,
    schedules: &mut [BellSchedule],
) -> Result<(), AppError> {
    let ids: Vec<_> = schedules.iter().map(|row| row.id).collect();
    let states: Vec<ScheduleEditState> = DbSession::new(connection)
        .query_as(
            "academic_core.services.bell_schedules.attach_editability",
            r#"SELECT schedule.id,year.status AS year_status,
                EXISTS(SELECT 1 FROM academic_terms term WHERE term.academic_year_id=year.id
                    AND term.status IN ('planning','ready') AND NOT EXISTS(
                        SELECT 1 FROM academic_term_transition_receipts receipt
                        WHERE receipt.academic_term_id=term.id AND receipt.action='activate')) AS has_unopened_term,
                (EXISTS(SELECT 1 FROM academic_terms term WHERE term.bell_schedule_id=schedule.id
                    AND term.status IN ('active','closing','closed')) OR EXISTS(
                        SELECT 1 FROM academic_term_transition_receipts receipt WHERE receipt.action='activate'
                          AND receipt.outcome->'context'->>'bellScheduleId'=schedule.id::text)) AS already_used,
                EXISTS(SELECT 1 FROM academic_timetable_versions version
                    WHERE version.bell_schedule_id=schedule.id AND (version.status='published' OR version.published_at IS NOT NULL)) AS pinned_timetable,
                (EXISTS(SELECT 1 FROM academic_timetable_versions version WHERE version.bell_schedule_id=schedule.id)
                 OR EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.bell_schedule_id=schedule.id)) AS timetable_references
               FROM bell_schedules schedule JOIN academic_years year ON year.id=schedule.academic_year_id
               WHERE schedule.id=ANY($1)"#,
        )
        .bind(ids)
        .fetch_all()
        .await?;
    let states: std::collections::HashMap<_, _> =
        states.into_iter().map(|state| (state.id, state)).collect();
    for schedule in schedules {
        let state = states
            .get(&schedule.id)
            .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".into()))?;
        schedule.edit_blocked_reason = state.blocked_reason();
        schedule.can_clone = true;
    }
    Ok(())
}

async fn require_schedule_editable(
    transaction: &mut Transaction<'_, Postgres>,
    schedule_id: Uuid,
) -> Result<(), AppError> {
    let year = DbSession::new(&mut **transaction)
        .scalar(
            "academic_core.services.bell_schedules.require_schedule_editable",
            "SELECT academic_year_id FROM bell_schedules WHERE id=$1",
        )
        .bind(schedule_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบตารางคาบ".into()))?;
    require_setup_year(transaction, year).await?;
    let schedule = get_on_connection(transaction, schedule_id).await?;
    if schedule.edit_blocked_reason == Some(BellScheduleEditBlockReason::TimetableReferences) {
        return Err(AppError::Conflict(
            "มีร่างตารางสอนอ้างอิงตารางคาบนี้ กรุณาจัดการร่างตารางสอนก่อนเปลี่ยนคาบ การทำสำเนาไม่ย้ายรายการในร่างเดิม".into(),
        ));
    }
    if schedule.edit_blocked_reason.is_some() {
        return Err(AppError::Conflict(
            "ตารางคาบนี้ถูกใช้หรือเผยแพร่แล้ว กรุณาทำสำเนาตารางคาบสำหรับภาคเรียนที่ยังไม่เปิด".into(),
        ));
    }
    Ok(())
}

async fn clear_default_except(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    schedule_id: Uuid,
) -> Result<(), AppError> {
    let defaults: Vec<Uuid> = DbSession::new(&mut **transaction)
        .scalar(
            "academic_core.services.bell_schedules.clear_default_except",
            "SELECT id FROM bell_schedules WHERE academic_year_id=$1 AND id<>$2 AND is_default ORDER BY id",
        )
        .bind(academic_year_id)
        .bind(schedule_id)
        .fetch_all()
        .await?;
    for id in defaults {
        require_schedule_editable(transaction, id).await?;
    }
    DbSession::new(&mut **transaction)
        .query(
            "academic_core.services.bell_schedules.clear_default_except",
            "UPDATE bell_schedules SET is_default = false,row_version=row_version+1,updated_at=now() WHERE academic_year_id = $1 AND id <> $2 AND is_default",
        )
        .bind(academic_year_id)
        .bind(schedule_id)
        .execute()
        .await?;
    Ok(())
}

fn map_schedule_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("bell_schedules_academic_year_id_code_key") {
            return AppError::Conflict("รหัสตารางคาบซ้ำในปีการศึกษา".to_string());
        }
    }
    AppError::DbError(error)
}
