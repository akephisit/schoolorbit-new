use school_database::DbSession;
use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, PgPool, Postgres};
use uuid::Uuid;

use crate::models::{
    CreateSupervisionCycleRequest, CreateSupervisionCycleTargetRequest, SupervisionCycle,
    SupervisionCycleQuery, SupervisionCycleStatus, SupervisionCycleTarget, SupervisionTargetType,
    UpdateSupervisionCycleRequest,
};
use school_errors::AppError;

use super::lifecycle::{require_context_writes, require_cycle_write};
use super::shared::{parse_cycle_status, parse_target_type};

#[derive(Debug, sqlx::FromRow)]
struct SupervisionCycleRow {
    id: Uuid,
    academic_year_id: Uuid,
    academic_term_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    template_id: Uuid,
    status: String,
    observation_start_date: Option<NaiveDate>,
    observation_end_date: Option<NaiveDate>,
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

pub async fn list_cycles(
    pool: &PgPool,
    query: SupervisionCycleQuery,
) -> Result<Vec<SupervisionCycle>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "supervision.cycles.list_cycles")
            .await?;

    let rows = DbSession::new(&mut *connection)
        .query_as::<SupervisionCycleRow>(
            "supervision.services.cycles.list_cycles",
            r#"
        SELECT id, academic_year_id, academic_term_id, title, description,
               template_id,
               status, observation_start_date, observation_end_date, created_by, created_at, updated_at
        FROM supervision_cycles
        WHERE academic_year_id = $1
          AND ($2::uuid IS NULL OR academic_term_id = $2 OR academic_term_id IS NULL)
        ORDER BY academic_term_id NULLS FIRST, created_at DESC
        "#,
        )
        .bind(query.academic_year_id)
        .bind(query.academic_term_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to list supervision cycles: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงรอบนิเทศได้".to_string())
        })?;

    let cycle_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();

    let targets_by_cycle = load_cycle_targets_by_cycle(&mut connection, &cycle_ids).await?;
    rows.into_iter()
        .map(|row| cycle_from_row(row, &targets_by_cycle))
        .collect()
}

pub async fn get_cycle(pool: &PgPool, id: Uuid) -> Result<SupervisionCycle, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(&pool, "supervision.cycles.get_cycle")
            .await?;
    get_cycle_with_connection(&mut connection, id).await
}

async fn get_cycle_with_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<SupervisionCycle, AppError> {
    let row = DbSession::new(&mut *connection)
        .query_as::<SupervisionCycleRow>(
            "supervision.services.cycles.get_cycle_with_connection",
            r#"
        SELECT id, academic_year_id, academic_term_id, title, description,
               template_id,
               status, observation_start_date, observation_end_date, created_by, created_at, updated_at
        FROM supervision_cycles
        WHERE id = $1
        "#,
        )
        .bind(id)
        .fetch_optional()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to get supervision cycle: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงรอบนิเทศได้".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบนิเทศ".to_string()))?;

    let targets = load_cycle_targets(connection, id).await?;
    cycle_from_row_with_targets(row, targets)
}

pub async fn create_cycle(
    pool: &PgPool,
    input: CreateSupervisionCycleRequest,
    created_by: Uuid,
) -> Result<SupervisionCycle, AppError> {
    validate_cycle_targets(&input.targets)?;
    validate_observation_dates(input.observation_start_date, input.observation_end_date)?;

    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "supervision.cycles.create_cycle")
            .await
            .map_err(|error| {
                tracing::error!(
                    "Failed to begin create supervision cycle transaction: {}",
                    school_database::error_class(&error)
                );
                AppError::InternalServerError("ไม่สามารถเริ่มสร้างรอบนิเทศได้".to_string())
            })?;

    require_context_writes(&mut tx, &[(input.academic_year_id, input.academic_term_id)]).await?;
    let status = input.status.unwrap_or(SupervisionCycleStatus::Draft);
    let cycle_id: Uuid = DbSession::new(&mut *tx)
        .scalar(
            "supervision.services.cycles.create_cycle",
            r#"
        INSERT INTO supervision_cycles (
            academic_year_id, academic_term_id, title, description, template_id,
            status, created_by, observation_start_date, observation_end_date
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
        )
        .bind(input.academic_year_id)
        .bind(input.academic_term_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(input.template_id)
        .bind(status.as_str())
        .bind(created_by)
        .bind(input.observation_start_date)
        .bind(input.observation_end_date)
        .fetch_one()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to create supervision cycle: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถสร้างรอบนิเทศได้".to_string())
        })?;

    insert_cycle_targets(&mut tx, cycle_id, &input.targets).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit create supervision cycle transaction: {}",
            school_database::error_class(&error)
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "supervision.cycles.update_cycle")
            .await?;
    let before = get_cycle_with_connection(&mut tx, id).await?;
    let target = (
        input.academic_year_id.unwrap_or(before.academic_year_id),
        input.academic_term_id.unwrap_or(before.academic_term_id),
    );
    let locked = require_cycle_write(&mut tx, id, Some(target)).await?;
    if locked != (before.academic_year_id, before.academic_term_id) {
        return Err(AppError::Conflict(
            "รอบนิเทศถูกย้ายปีหรือภาคเรียนแล้ว กรุณาโหลดข้อมูลใหม่".into(),
        ));
    }
    let current = get_cycle_with_connection(&mut tx, id).await?;
    let academic_year_id = input.academic_year_id.unwrap_or(current.academic_year_id);
    let academic_term_id = input.academic_term_id.unwrap_or(current.academic_term_id);
    let requested_dates = match (input.observation_start_date, input.observation_end_date) {
        (None, None) => None,
        (Some(start), Some(end)) => {
            validate_observation_dates(start, end)?;
            Some((start, end))
        }
        _ => {
            return Err(AppError::ValidationError(
                "ต้องระบุวันเริ่มและวันสิ้นสุดช่วงนิเทศพร้อมกัน".into(),
            ))
        }
    };
    if let Some((start, end)) = requested_dates {
        let confirmed_states = super::shared::evaluator_conflict_status_codes();
        let historical_confirmed_states: Vec<_> = confirmed_states
            .iter()
            .copied()
            .chain(super::shared::LEGACY_RESULT_STATUS_CODES)
            .collect();
        let outside:bool=DbSession::new(&mut tx).scalar("supervision.cycles.protect_approved_dates",r#"
            SELECT EXISTS(SELECT 1 FROM supervision_observations o
                JOIN supervision_observation_periods p ON p.observation_id=o.id
                WHERE o.cycle_id=$1 AND o.status<>'cancelled'
                  AND (o.approved_at IS NOT NULL OR o.status=ANY($4) OR EXISTS(
                    SELECT 1 FROM supervision_actions a WHERE a.observation_id=o.id
                      AND (a.from_status=ANY($5) OR a.to_status=ANY($5))))
                  AND ((p.observed_at AT TIME ZONE 'Asia/Bangkok')::date<$2
                    OR (p.observed_at AT TIME ZONE 'Asia/Bangkok')::date>$3
                    OR ((COALESCE(p.ends_at-interval '1 microsecond',p.observed_at)) AT TIME ZONE 'Asia/Bangkok')::date>$3))
        "#).bind(id).bind(start).bind(end).bind(confirmed_states).bind(historical_confirmed_states).fetch_one().await?;
        if outside {
            return Err(AppError::Conflict(
                "ช่วงวันที่ใหม่ต้องครอบคลุมรายการนิเทศที่เคยอนุมัติแล้วและยังไม่ยกเลิก".into(),
            ));
        }
    }
    let observation_start_date = requested_dates
        .map(|d| d.0)
        .or(current.observation_start_date);
    let observation_end_date = requested_dates
        .map(|d| d.1)
        .or(current.observation_end_date);
    let title = input.title.unwrap_or(current.title);
    let description = input.description.or(current.description);
    let template_id = input.template_id.unwrap_or(current.template_id);
    let status = input.status.unwrap_or(current.status);

    if let Some(targets) = &input.targets {
        validate_cycle_targets(targets)?;
    }
    if target != locked {
        let incompatible: bool = DbSession::new(&mut *tx).scalar("supervision.services.cycles.update_cycle",
            "SELECT EXISTS (SELECT 1 FROM supervision_observations WHERE cycle_id=$1 AND (academic_year_id<>$2 OR ($3::uuid IS NOT NULL AND academic_term_id<>$3)))",
        ).bind(id).bind(academic_year_id).bind(academic_term_id).fetch_one().await?;
        if incompatible {
            return Err(AppError::Conflict(
                "รอบนิเทศมีรายการนิเทศที่ไม่ตรงกับปีหรือภาคเรียนปลายทาง".into(),
            ));
        }
    }
    DbSession::new(&mut *tx)
        .query(
            "supervision.services.cycles.update_cycle",
            r#"
        UPDATE supervision_cycles
        SET academic_year_id = $2, academic_term_id = $3,
            title = $4, description = $5, template_id = $6,
            status = $7, observation_start_date=$8, observation_end_date=$9
        WHERE id = $1
        "#,
        )
        .bind(id)
        .bind(academic_year_id)
        .bind(academic_term_id)
        .bind(&title)
        .bind(&description)
        .bind(template_id)
        .bind(status.as_str())
        .bind(observation_start_date)
        .bind(observation_end_date)
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to update supervision cycle: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถแก้ไขรอบนิเทศได้".to_string())
        })?;

    if let Some(targets) = input.targets {
        DbSession::new(&mut *tx)
            .query(
                "supervision.services.cycles.update_cycle",
                "DELETE FROM supervision_cycle_targets WHERE cycle_id = $1",
            )
            .bind(id)
            .execute()
            .await
            .map_err(|error| {
                tracing::error!(
                    "Failed to clear supervision cycle targets: {}",
                    school_database::error_class(&error)
                );
                AppError::InternalServerError("ไม่สามารถแก้ไขเป้าหมายรอบนิเทศได้".to_string())
            })?;
        insert_cycle_targets(&mut tx, id, &targets).await?;
    }

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit update supervision cycle transaction: {}",
            school_database::error_class(&error)
        );
        AppError::InternalServerError("ไม่สามารถบันทึกรอบนิเทศได้".to_string())
    })?;

    get_cycle(pool, id).await
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

    DbSession::new(&mut **tx)
        .query(
            "supervision.services.cycles.insert_cycle_targets",
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
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to insert supervision cycle targets: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถบันทึกเป้าหมายรอบนิเทศได้".to_string())
        })?;

    Ok(())
}

async fn load_cycle_targets(
    connection: &mut PgConnection,
    cycle_id: Uuid,
) -> Result<Vec<SupervisionCycleTarget>, AppError> {
    let rows = DbSession::new(connection)
        .query_as::<SupervisionCycleTargetRow>(
            "supervision.services.cycles.load_cycle_targets",
            r#"
        SELECT id, cycle_id, target_type, target_id, required_observations,
               priority, created_at, updated_at
        FROM supervision_cycle_targets
        WHERE cycle_id = $1
        ORDER BY priority, created_at
        "#,
        )
        .bind(cycle_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision cycle targets: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงเป้าหมายรอบนิเทศได้".to_string())
        })?;

    rows.into_iter().map(cycle_target_from_row).collect()
}

async fn load_cycle_targets_by_cycle(
    connection: &mut sqlx::PgConnection,
    cycle_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<SupervisionCycleTarget>>, AppError> {
    if cycle_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = DbSession::new(&mut *connection)
        .query_as::<SupervisionCycleTargetRow>(
            "supervision.services.cycles.load_cycle_targets_by_cycle",
            r#"
        SELECT id, cycle_id, target_type, target_id, required_observations,
               priority, created_at, updated_at
        FROM supervision_cycle_targets
        WHERE cycle_id = ANY($1)
        ORDER BY priority, created_at
        "#,
        )
        .bind(cycle_ids)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision cycle targets: {}",
                school_database::error_class(&error)
            );
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
        academic_year_id: row.academic_year_id,
        academic_term_id: row.academic_term_id,
        title: row.title,
        description: row.description,
        template_id: row.template_id,
        status: parse_cycle_status(&row.status)?,
        observation_start_date: row.observation_start_date,
        observation_end_date: row.observation_end_date,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
        targets,
    })
}

fn validate_observation_dates(start: NaiveDate, end: NaiveDate) -> Result<(), AppError> {
    if start > end {
        return Err(AppError::ValidationError(
            "วันเริ่มช่วงนิเทศต้องไม่อยู่หลังวันสิ้นสุด".into(),
        ));
    }
    Ok(())
}
