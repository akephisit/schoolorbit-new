use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::supervision::models::{
    CreateSupervisionCycleRequest, CreateSupervisionCycleTargetRequest, SupervisionCycle,
    SupervisionCycleStatus, SupervisionCycleTarget, SupervisionTargetType,
    UpdateSupervisionCycleRequest,
};

use super::shared::{parse_cycle_status, parse_target_type};

#[derive(Debug, sqlx::FromRow)]
struct SupervisionCycleRow {
    id: Uuid,
    academic_year: i32,
    semester: String,
    academic_semester_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    template_id: Uuid,
    booking_opens_at: Option<DateTime<Utc>>,
    booking_closes_at: Option<DateTime<Utc>>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    status: String,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct SupervisionCycleTargetRow {
    pub(super) id: Uuid,
    pub(super) cycle_id: Uuid,
    pub(super) target_type: String,
    pub(super) target_id: Option<Uuid>,
    pub(super) required_observations: i32,
    pub(super) priority: i32,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

pub async fn list_cycles(pool: &PgPool) -> Result<Vec<SupervisionCycle>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionCycleRow>(
        r#"
        SELECT id, academic_year, semester, academic_semester_id, title, description,
               template_id, booking_opens_at, booking_closes_at, starts_at, ends_at,
               status, created_by, created_at, updated_at
        FROM supervision_cycles
        ORDER BY academic_year DESC, semester DESC, created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to list supervision cycles: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงรอบนิเทศได้".to_string())
    })?;

    let targets_by_cycle = load_cycle_targets_by_cycle(pool).await?;
    rows.into_iter()
        .map(|row| cycle_from_row(row, &targets_by_cycle))
        .collect()
}

pub async fn get_cycle(pool: &PgPool, id: Uuid) -> Result<SupervisionCycle, AppError> {
    let row = sqlx::query_as::<_, SupervisionCycleRow>(
        r#"
        SELECT id, academic_year, semester, academic_semester_id, title, description,
               template_id, booking_opens_at, booking_closes_at, starts_at, ends_at,
               status, created_by, created_at, updated_at
        FROM supervision_cycles
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision cycle: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงรอบนิเทศได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบนิเทศ".to_string()))?;

    let targets = load_cycle_targets(pool, id).await?;
    cycle_from_row_with_targets(row, targets)
}

pub async fn create_cycle(
    pool: &PgPool,
    input: CreateSupervisionCycleRequest,
    created_by: Uuid,
) -> Result<SupervisionCycle, AppError> {
    validate_cycle_schedule(
        input.booking_opens_at,
        input.booking_closes_at,
        input.starts_at,
        input.ends_at,
    )?;
    validate_cycle_targets(&input.targets)?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin create supervision cycle transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มสร้างรอบนิเทศได้".to_string())
    })?;

    let status = input.status.unwrap_or(SupervisionCycleStatus::Draft);
    let cycle_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO supervision_cycles (
            academic_year, semester, academic_semester_id, title, description, template_id,
            booking_opens_at, booking_closes_at, starts_at, ends_at, status, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(input.academic_year)
    .bind(&input.semester)
    .bind(input.academic_semester_id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(input.template_id)
    .bind(input.booking_opens_at)
    .bind(input.booking_closes_at)
    .bind(input.starts_at)
    .bind(input.ends_at)
    .bind(status.as_str())
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to create supervision cycle: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างรอบนิเทศได้".to_string())
    })?;

    insert_cycle_targets(&mut tx, cycle_id, &input.targets).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit create supervision cycle transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกรอบนิเทศได้".to_string())
    })?;

    get_cycle(pool, cycle_id).await
}

pub async fn update_cycle(
    pool: &PgPool,
    id: Uuid,
    input: UpdateSupervisionCycleRequest,
) -> Result<SupervisionCycle, AppError> {
    let current = get_cycle(pool, id).await?;
    let academic_year = input.academic_year.unwrap_or(current.academic_year);
    let semester = input.semester.unwrap_or(current.semester);
    let academic_semester_id = input.academic_semester_id.or(current.academic_semester_id);
    let title = input.title.unwrap_or(current.title);
    let description = input.description.or(current.description);
    let template_id = input.template_id.unwrap_or(current.template_id);
    let booking_opens_at = input.booking_opens_at.or(current.booking_opens_at);
    let booking_closes_at = input.booking_closes_at.or(current.booking_closes_at);
    let starts_at = input.starts_at.unwrap_or(current.starts_at);
    let ends_at = input.ends_at.unwrap_or(current.ends_at);
    let status = input.status.unwrap_or(current.status);

    validate_cycle_schedule(booking_opens_at, booking_closes_at, starts_at, ends_at)?;
    if let Some(targets) = &input.targets {
        validate_cycle_targets(targets)?;
    }

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin update supervision cycle transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขรอบนิเทศได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE supervision_cycles
        SET academic_year = $2, semester = $3, academic_semester_id = $4,
            title = $5, description = $6, template_id = $7,
            booking_opens_at = $8, booking_closes_at = $9,
            starts_at = $10, ends_at = $11, status = $12
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(academic_year)
    .bind(&semester)
    .bind(academic_semester_id)
    .bind(&title)
    .bind(&description)
    .bind(template_id)
    .bind(booking_opens_at)
    .bind(booking_closes_at)
    .bind(starts_at)
    .bind(ends_at)
    .bind(status.as_str())
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update supervision cycle: {}", error);
        AppError::InternalServerError("ไม่สามารถแก้ไขรอบนิเทศได้".to_string())
    })?;

    if let Some(targets) = input.targets {
        sqlx::query("DELETE FROM supervision_cycle_targets WHERE cycle_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::error!("Failed to clear supervision cycle targets: {}", error);
                AppError::InternalServerError("ไม่สามารถแก้ไขเป้าหมายรอบนิเทศได้".to_string())
            })?;
        insert_cycle_targets(&mut tx, id, &targets).await?;
    }

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit update supervision cycle transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกรอบนิเทศได้".to_string())
    })?;

    get_cycle(pool, id).await
}

fn validate_cycle_schedule(
    booking_opens_at: Option<DateTime<Utc>>,
    booking_closes_at: Option<DateTime<Utc>>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
) -> Result<(), AppError> {
    if starts_at > ends_at {
        return Err(AppError::ValidationError(
            "วันเริ่มรอบนิเทศต้องอยู่ก่อนวันสิ้นสุด".to_string(),
        ));
    }

    if let (Some(open), Some(close)) = (booking_opens_at, booking_closes_at) {
        if open > close {
            return Err(AppError::ValidationError(
                "เวลาเปิดจองต้องอยู่ก่อนเวลาปิดจอง".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_cycle_targets(targets: &[CreateSupervisionCycleTargetRequest]) -> Result<(), AppError> {
    for target in targets {
        if target.required_observations <= 0 {
            return Err(AppError::ValidationError(
                "จำนวนครั้งที่ต้องนิเทศต้องมากกว่า 0".to_string(),
            ));
        }

        match target.target_type {
            SupervisionTargetType::School if target.target_id.is_some() => {
                return Err(AppError::ValidationError(
                    "เป้าหมายทั้งโรงเรียนต้องไม่มี targetId".to_string(),
                ));
            }
            SupervisionTargetType::OrganizationUnit
            | SupervisionTargetType::SubjectGroup
            | SupervisionTargetType::Staff
                if target.target_id.is_none() =>
            {
                return Err(AppError::ValidationError(
                    "เป้าหมายนี้ต้องระบุ targetId".to_string(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

async fn insert_cycle_targets(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    cycle_id: Uuid,
    targets: &[CreateSupervisionCycleTargetRequest],
) -> Result<(), AppError> {
    if targets.is_empty() {
        return Ok(());
    }

    let target_types: Vec<String> = targets
        .iter()
        .map(|target| target.target_type.as_str().to_string())
        .collect();
    let target_ids: Vec<Option<Uuid>> = targets.iter().map(|target| target.target_id).collect();
    let required_observations: Vec<i32> = targets
        .iter()
        .map(|target| target.required_observations)
        .collect();
    let priorities: Vec<i32> = targets.iter().map(|target| target.priority).collect();

    sqlx::query(
        r#"
        INSERT INTO supervision_cycle_targets (
            cycle_id, target_type, target_id, required_observations, priority
        )
        SELECT $1, target_type, target_id, required_observations, priority
        FROM UNNEST($2::text[], $3::uuid[], $4::int4[], $5::int4[])
             AS rows(target_type, target_id, required_observations, priority)
        "#,
    )
    .bind(cycle_id)
    .bind(&target_types)
    .bind(&target_ids)
    .bind(&required_observations)
    .bind(&priorities)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to insert supervision cycle targets: {}", error);
        AppError::InternalServerError("ไม่สามารถบันทึกเป้าหมายรอบนิเทศได้".to_string())
    })?;

    Ok(())
}

async fn load_cycle_targets(
    pool: &PgPool,
    cycle_id: Uuid,
) -> Result<Vec<SupervisionCycleTarget>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionCycleTargetRow>(
        r#"
        SELECT id, cycle_id, target_type, target_id, required_observations,
               priority, created_at, updated_at
        FROM supervision_cycle_targets
        WHERE cycle_id = $1
        ORDER BY priority, created_at
        "#,
    )
    .bind(cycle_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle targets: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงเป้าหมายรอบนิเทศได้".to_string())
    })?;

    rows.into_iter().map(cycle_target_from_row).collect()
}

async fn load_cycle_targets_by_cycle(
    pool: &PgPool,
) -> Result<HashMap<Uuid, Vec<SupervisionCycleTarget>>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionCycleTargetRow>(
        r#"
        SELECT id, cycle_id, target_type, target_id, required_observations,
               priority, created_at, updated_at
        FROM supervision_cycle_targets
        ORDER BY priority, created_at
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle targets: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงเป้าหมายรอบนิเทศได้".to_string())
    })?;

    let mut targets_by_cycle: HashMap<Uuid, Vec<SupervisionCycleTarget>> = HashMap::new();
    for row in rows {
        let cycle_id = row.cycle_id;
        targets_by_cycle
            .entry(cycle_id)
            .or_default()
            .push(cycle_target_from_row(row)?);
    }

    Ok(targets_by_cycle)
}

fn cycle_target_from_row(
    row: SupervisionCycleTargetRow,
) -> Result<SupervisionCycleTarget, AppError> {
    Ok(SupervisionCycleTarget {
        id: row.id,
        cycle_id: row.cycle_id,
        target_type: parse_target_type(&row.target_type)?,
        target_id: row.target_id,
        required_observations: row.required_observations,
        priority: row.priority,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn cycle_from_row(
    row: SupervisionCycleRow,
    targets_by_cycle: &HashMap<Uuid, Vec<SupervisionCycleTarget>>,
) -> Result<SupervisionCycle, AppError> {
    let cycle_id = row.id;
    cycle_from_row_with_targets(
        row,
        targets_by_cycle.get(&cycle_id).cloned().unwrap_or_default(),
    )
}

fn cycle_from_row_with_targets(
    row: SupervisionCycleRow,
    targets: Vec<SupervisionCycleTarget>,
) -> Result<SupervisionCycle, AppError> {
    Ok(SupervisionCycle {
        id: row.id,
        academic_year: row.academic_year,
        semester: row.semester,
        academic_semester_id: row.academic_semester_id,
        title: row.title,
        description: row.description,
        template_id: row.template_id,
        booking_opens_at: row.booking_opens_at,
        booking_closes_at: row.booking_closes_at,
        starts_at: row.starts_at,
        ends_at: row.ends_at,
        status: parse_cycle_status(&row.status)?,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
        targets,
    })
}
