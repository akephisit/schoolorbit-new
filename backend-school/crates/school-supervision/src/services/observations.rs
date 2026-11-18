use school_database::DbSession;
use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::models::{
    ApproveObservationRequest, CancelObservationRequest, RequestSupervisionObservationRequest,
    ReturnObservationRequest, SupervisionAction, SupervisionBookingOptions, SupervisionCycleStatus,
    SupervisionEvaluator, SupervisionEvaluatorAvailability, SupervisionObservation,
    SupervisionObservationFilter, SupervisionObservationStatus, UpdateRequestedObservationRequest,
    UpdateSupervisionObservationRequest,
};
use school_errors::AppError;

use super::cycles::SupervisionCycleTargetRow;
use super::evaluations::{
    evaluator_availability_from_row, insert_supervision_evaluators,
    load_evaluator_submission_states, validate_evaluator_availability_for_observation,
    EvaluatorAvailabilityRow,
};
use super::lifecycle::{require_cycle_write, require_observation_write};
use super::shared::{
    all_required_evaluators_submitted, can_transition_observation_status,
    evaluator_conflict_status_codes, has_approval_evidence, has_required_evaluator,
    manager_can_edit_observation, parse_cycle_status, parse_evaluator_status,
    parse_observation_status, parse_optional_observation_status, parse_target_type,
    resolve_supervision_target_rule, teacher_can_edit_requested_observation,
    SupervisionObservationListAccess, SupervisionTargetMatch, SupervisionTargetRule,
};

use super::{booking, periods};

#[derive(Debug, sqlx::FromRow)]
struct SupervisionObservationRow {
    id: Uuid,
    cycle_id: Uuid,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    observed_user_id: Uuid,
    observed_display_name: Option<String>,
    requested_by: Option<Uuid>,
    approved_by: Option<Uuid>,
    template_id: Uuid,
    status: String,
    requested_at: DateTime<Utc>,
    approved_at: Option<DateTime<Utc>>,
    cancelled_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionEvaluatorRow {
    id: Uuid,
    observation_id: Uuid,
    evaluator_user_id: Uuid,
    evaluator_display_name: Option<String>,
    role_label: Option<String>,
    is_required: bool,
    status: String,
    submitted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn evaluator_from_row(row: SupervisionEvaluatorRow) -> Result<SupervisionEvaluator, AppError> {
    Ok(SupervisionEvaluator {
        id: row.id,
        observation_id: row.observation_id,
        evaluator_user_id: row.evaluator_user_id,
        evaluator_display_name: row.evaluator_display_name,
        role_label: row.role_label,
        is_required: row.is_required,
        status: parse_evaluator_status(&row.status)?,
        submitted_at: row.submitted_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionActionRow {
    id: Uuid,
    observation_id: Uuid,
    actor_user_id: Option<Uuid>,
    actor_display_name: Option<String>,
    action_kind: String,
    from_status: Option<String>,
    to_status: Option<String>,
    comment: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct CycleForRequestRow {
    pub(super) id: Uuid,
    pub(super) academic_year_id: Uuid,
    pub(super) academic_term_id: Option<Uuid>,
    pub(super) template_id: Uuid,
    pub(super) status: String,
    pub(super) observation_start_date: Option<NaiveDate>,
    pub(super) observation_end_date: Option<NaiveDate>,
}

impl CycleForRequestRow {
    pub(super) fn contains_date(&self, date: NaiveDate) -> bool {
        self.observation_start_date
            .zip(self.observation_end_date)
            .is_some_and(|(start, end)| start <= date && date <= end)
    }
    pub(super) fn require_date(&self, date: NaiveDate) -> Result<(), AppError> {
        if !self.contains_date(date) {
            return Err(AppError::ValidationError(
                "วันที่นิเทศต้องอยู่ในช่วงวันที่ที่กำหนดของรอบนิเทศ".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn contains_periods(
        &self,
        periods: &[crate::models::SupervisionObservationPeriod],
    ) -> bool {
        !periods.is_empty()
            && periods.iter().all(|p| {
                self.contains_date(
                    p.observed_at
                        .with_timezone(&booking::SCHOOL_TIMEZONE)
                        .date_naive(),
                ) && self.contains_date(
                    p.ends_at
                        .map(|end| end - chrono::Duration::microseconds(1))
                        .unwrap_or(p.observed_at)
                        .with_timezone(&booking::SCHOOL_TIMEZONE)
                        .date_naive(),
                )
            })
    }
    pub(super) fn require_periods(
        &self,
        periods: &[crate::models::SupervisionObservationPeriod],
    ) -> Result<(), AppError> {
        if !self.contains_periods(periods) {
            return Err(AppError::ValidationError(
                "รายการนิเทศต้องอยู่ในช่วงวันที่ที่กำหนดของรอบนิเทศ".into(),
            ));
        }
        Ok(())
    }
}

pub async fn list_observations(
    pool: &PgPool,
    access: SupervisionObservationListAccess,
    filter: SupervisionObservationFilter,
) -> Result<Vec<SupervisionObservation>, AppError> {
    if !access.school && access.is_empty() {
        return Ok(Vec::new());
    }
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "supervision.observations.list_observations",
    )
    .await?;
    let rows = list_observation_rows(&mut connection, access, filter).await?;

    hydrate_observations(&mut connection, rows).await
}

pub async fn get_observation(pool: &PgPool, id: Uuid) -> Result<SupervisionObservation, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        &pool,
        "supervision.observations.get_observation",
    )
    .await?;
    get_observation_with_connection(&mut connection, id).await
}

pub(super) async fn get_observation_with_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    let row = load_observation_row(&mut *connection, id).await?;
    hydrate_observations(connection, vec![row])
        .await?
        .pop()
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการนิเทศ".to_string()))
}

pub async fn evaluator_availability(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<SupervisionEvaluatorAvailability>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "supervision.observations.evaluator_availability",
    )
    .await?;
    let observation =
        super::observations::get_observation_with_connection(&mut connection, observation_id)
            .await?;
    let conflict_statuses = evaluator_conflict_status_codes()
        .iter()
        .map(|status| (*status).to_string())
        .collect::<Vec<_>>();

    let rows=DbSession::new(&mut connection).query_as::<EvaluatorAvailabilityRow>("supervision.observations.evaluator_availability",r#"
        SELECT u.id,u.title,u.display_name,conflict.observation_id AS conflict_observation_id,
            conflict.observed_display_name AS conflict_observed_display_name,
            conflict.observed_at AS conflict_observed_at,conflict.subject_name AS conflict_subject_name,
            conflict.period_label AS conflict_period_label
        FROM user_display_names u LEFT JOIN LATERAL (
            SELECT o.id AS observation_id,NULLIF(observed.display_name,'') AS observed_display_name,
                p.observed_at,
                COALESCE(NULLIF(p.manual_subject_name,''),NULLIF(p.lesson_snapshot->>'subjectName','')) AS subject_name,
                COALESCE(NULLIF(p.manual_period_label,''),NULLIF(p.lesson_snapshot->>'periodLabel','')) AS period_label
            FROM supervision_evaluators e JOIN supervision_observations o ON o.id=e.observation_id
            JOIN supervision_observation_periods p ON p.observation_id=o.id
            JOIN user_display_names observed ON observed.id=o.observed_user_id
            WHERE e.evaluator_user_id=u.id AND o.id<>$1 AND o.status=ANY($2)
              AND EXISTS(SELECT 1 FROM supervision_observation_periods chosen WHERE chosen.observation_id=$1 AND (
                (p.ends_at IS NULL AND chosen.ends_at IS NULL AND p.observed_at=chosen.observed_at)
                OR (p.ends_at IS NULL AND chosen.ends_at IS NOT NULL AND chosen.observed_at<=p.observed_at AND p.observed_at<chosen.ends_at)
                OR (p.ends_at IS NOT NULL AND chosen.ends_at IS NULL AND p.observed_at<=chosen.observed_at AND chosen.observed_at<p.ends_at)
                OR (p.ends_at IS NOT NULL AND chosen.ends_at IS NOT NULL AND p.observed_at<chosen.ends_at AND chosen.observed_at<p.ends_at)))
            ORDER BY o.approved_at DESC NULLS LAST,o.created_at DESC,o.id,p.position LIMIT 1
        ) conflict ON true WHERE u.user_type='staff' AND u.status='active' AND u.id<>$3
        ORDER BY (conflict.observation_id IS NOT NULL),u.first_name,u.last_name,u.id
    "#).bind(observation_id).bind(conflict_statuses).bind(observation.observed_user_id).fetch_all().await?;

    Ok(rows
        .into_iter()
        .map(evaluator_availability_from_row)
        .collect())
}

pub async fn observation_timetable_options(
    pool: &PgPool,
    observation_id: Uuid,
    week_start_date: NaiveDate,
) -> Result<SupervisionBookingOptions, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "supervision.observations.timetable_options",
    )
    .await?;
    let observation = get_observation_with_connection(&mut connection, observation_id).await?;
    let cycle = load_cycle_for_request(&mut connection, observation.cycle_id).await?;
    validate_observation_context(&mut connection, &cycle, observation.academic_term_id).await?;
    ensure_cycle_target_allows_teacher(&mut connection, cycle.id, observation.observed_user_id)
        .await?;
    let mut options = booking::options_on_connection(
        &mut connection,
        observation.observed_user_id,
        &cycle,
        observation.academic_term_id,
        week_start_date,
    )
    .await?;
    if observation.periods_locked {
        options.items.clear();
    }
    Ok(options)
}

pub async fn request_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    input: RequestSupervisionObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.observations.request_observation",
    )
    .await?;
    let before = load_cycle_for_request(&mut *tx, input.cycle_id).await?;
    ensure_cycle_target_allows_teacher(&mut tx, input.cycle_id, actor_user_id).await?;
    let context = require_cycle_write(
        &mut tx,
        input.cycle_id,
        Some((before.academic_year_id, Some(input.academic_term_id))),
    )
    .await?;
    if context != (before.academic_year_id, before.academic_term_id) {
        return Err(AppError::Conflict("รอบนิเทศเปลี่ยนแล้ว กรุณาโหลดข้อมูลใหม่".into()));
    }
    let cycle = load_cycle_for_request(&mut *tx, input.cycle_id).await?;
    validate_cycle_accepts_requests(&cycle)?;
    validate_observation_context(&mut *tx, &cycle, input.academic_term_id).await?;
    ensure_cycle_target_allows_teacher(&mut tx, cycle.id, actor_user_id).await?;

    let selected = booking::resolve(
        &mut tx,
        actor_user_id,
        &cycle,
        input.academic_term_id,
        input.observed_date,
        &input.timetable_block_group_ids,
        &input.expected_source_digest,
    )
    .await?;
    periods::ensure_no_own_duplicate(
        &mut tx,
        actor_user_id,
        cycle.id,
        None,
        input.observed_date,
        &selected,
    )
    .await?;
    let observation_id:Uuid=DbSession::new(&mut tx).scalar("supervision.observations.request_observation",
        "INSERT INTO supervision_observations(cycle_id,academic_year_id,academic_term_id,observed_user_id,requested_by,template_id)
         VALUES($1,$2,$3,$4,$4,$5) RETURNING id")
        .bind(cycle.id).bind(cycle.academic_year_id).bind(input.academic_term_id).bind(actor_user_id).bind(cycle.template_id)
        .fetch_one().await?;
    periods::replace(
        &mut tx,
        observation_id,
        cycle.academic_year_id,
        input.academic_term_id,
        &selected,
    )
    .await?;

    insert_observation_action(
        &mut tx,
        observation_id,
        Some(actor_user_id),
        "requested",
        None,
        Some(SupervisionObservationStatus::Requested),
        None,
    )
    .await?;

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub async fn update_requested_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: UpdateRequestedObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.observations.update_requested_observation",
    )
    .await?;
    let current = get_observation_with_connection(&mut tx, observation_id).await?;
    if current.observed_user_id != actor_user_id {
        return Err(AppError::Forbidden("แก้ไขคำขอนิเทศของผู้อื่นไม่ได้".to_string()));
    }
    require_observation_write(&mut tx, observation_id).await?;
    let current = get_observation_with_connection(&mut tx, observation_id).await?;
    if !teacher_can_edit_requested_observation(current.status) {
        return Err(AppError::ValidationError(
            "แก้ไขคำขอนิเทศได้เฉพาะสถานะรออนุมัติหรือส่งกลับ".to_string(),
        ));
    }

    if current.periods_locked {
        return Err(AppError::Conflict(
            "คาบนิเทศที่เคยอนุมัติแล้วเปลี่ยนไม่ได้ กรุณาส่งคำขอเดิมอีกครั้ง".into(),
        ));
    }
    let cycle = load_cycle_for_request(&mut *tx, current.cycle_id).await?;
    if current.status == SupervisionObservationStatus::Returned {
        validate_cycle_accepts_requests(&cycle)?;
        ensure_no_submitted_evaluators(&current)?;
        ensure_cycle_target_allows_teacher(&mut tx, cycle.id, actor_user_id).await?;
    }
    ensure_no_submitted_evaluators(&current)?;
    ensure_cycle_target_allows_teacher(&mut tx, cycle.id, actor_user_id).await?;
    let selected = booking::resolve(
        &mut tx,
        actor_user_id,
        &cycle,
        current.academic_term_id,
        input.observed_date,
        &input.timetable_block_group_ids,
        &input.expected_source_digest,
    )
    .await?;
    periods::ensure_no_own_duplicate(
        &mut tx,
        actor_user_id,
        current.cycle_id,
        Some(observation_id),
        input.observed_date,
        &selected,
    )
    .await?;
    periods::replace(
        &mut tx,
        observation_id,
        current.academic_year_id,
        current.academic_term_id,
        &selected,
    )
    .await?;
    let evaluator_ids = current
        .evaluators
        .iter()
        .map(|e| e.evaluator_user_id)
        .collect::<Vec<_>>();
    validate_evaluator_availability_for_observation(&mut tx, observation_id, &evaluator_ids)
        .await?;
    DbSession::new(&mut tx).query("supervision.observations.update_requested_observation",
        "UPDATE supervision_observations SET status='requested',requested_at=CASE WHEN status='returned' THEN now() ELSE requested_at END WHERE id=$1")
        .bind(observation_id).execute().await?;

    if current.status == SupervisionObservationStatus::Returned {
        insert_observation_action(
            &mut tx,
            observation_id,
            Some(actor_user_id),
            "requested",
            Some(SupervisionObservationStatus::Returned),
            Some(SupervisionObservationStatus::Requested),
            None,
        )
        .await?;
    }

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub async fn resubmit_observation_request(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "supervision.observations.resubmit_saved_request",
    )
    .await?;
    let before = get_observation_with_connection(&mut tx, observation_id).await?;
    if before.observed_user_id != actor_user_id {
        return Err(AppError::Forbidden("ส่งคำขอนิเทศของผู้อื่นไม่ได้".into()));
    }
    require_observation_write(&mut tx, observation_id).await?;
    let current = get_observation_with_connection(&mut tx, observation_id).await?;
    if current.status != SupervisionObservationStatus::Returned || !current.periods_locked {
        return Err(AppError::ValidationError(
            "ส่งคำขอเดิมได้เฉพาะรายการที่เคยอนุมัติและถูกส่งกลับ".into(),
        ));
    }
    ensure_no_submitted_evaluators(&current)?;
    let cycle = load_cycle_for_request(&mut tx, current.cycle_id).await?;
    validate_cycle_accepts_requests(&cycle)?;
    cycle.require_periods(&current.periods)?;
    ensure_cycle_target_allows_teacher(&mut tx, cycle.id, actor_user_id).await?;
    let evaluators = current
        .evaluators
        .iter()
        .map(|e| e.evaluator_user_id)
        .collect::<Vec<_>>();
    validate_evaluator_availability_for_observation(&mut tx, observation_id, &evaluators).await?;
    DbSession::new(&mut tx)
        .query(
            "supervision.observations.resubmit_saved_request",
            "UPDATE supervision_observations SET status='requested',requested_at=now() WHERE id=$1",
        )
        .bind(observation_id)
        .execute()
        .await?;
    insert_observation_action(
        &mut tx,
        observation_id,
        Some(actor_user_id),
        "requested",
        Some(current.status),
        Some(SupervisionObservationStatus::Requested),
        None,
    )
    .await?;
    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub async fn cancel_requested_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Cancelled,
        "request_cancelled",
        None,
        ObservationTransitionPolicy::RequestedTeacher,
    )
    .await
}

pub async fn update_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: UpdateSupervisionObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.observations.update_observation",
    )
    .await?;
    require_observation_write(&mut tx, observation_id).await?;
    let current = get_observation_with_connection(&mut tx, observation_id).await?;
    if !manager_can_edit_observation(current.status) {
        return Err(AppError::ValidationError(
            "แก้ไขรายการนิเทศได้เฉพาะสถานะรออนุมัติ วางแผน หรือส่งกลับ".to_string(),
        ));
    }

    if let Some((date, ids, digest)) = manager_selection(&input)? {
        if current.periods_locked {
            return Err(AppError::Conflict("คาบนิเทศที่เคยอนุมัติแล้วเปลี่ยนไม่ได้".into()));
        }
        let cycle = load_cycle_for_request(&mut tx, current.cycle_id).await?;
        ensure_cycle_target_allows_teacher(&mut tx, cycle.id, current.observed_user_id).await?;
        ensure_no_submitted_evaluators(&current)?;
        let selected = booking::resolve(
            &mut tx,
            current.observed_user_id,
            &cycle,
            current.academic_term_id,
            date,
            ids,
            digest,
        )
        .await?;
        periods::ensure_no_own_duplicate(
            &mut tx,
            current.observed_user_id,
            current.cycle_id,
            Some(observation_id),
            date,
            &selected,
        )
        .await?;
        periods::replace(
            &mut tx,
            observation_id,
            current.academic_year_id,
            current.academic_term_id,
            &selected,
        )
        .await?;
        let evaluators = current
            .evaluators
            .iter()
            .map(|e| e.evaluator_user_id)
            .collect::<Vec<_>>();
        validate_evaluator_availability_for_observation(&mut tx, observation_id, &evaluators)
            .await?;
    }
    if let Some(template_id) = input.template_id {
        ensure_no_submitted_evaluators(&current)?;
        DbSession::new(&mut tx)
            .query(
                "supervision.observations.update_template",
                "UPDATE supervision_observations SET template_id=$2 WHERE id=$1",
            )
            .bind(observation_id)
            .bind(template_id)
            .execute()
            .await?;
    }

    insert_observation_action(
        &mut tx,
        observation_id,
        Some(actor_user_id),
        "updated",
        Some(current.status),
        Some(current.status),
        None,
    )
    .await?;

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub async fn cancel_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: CancelObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Cancelled,
        "cancelled",
        input.reason,
        ObservationTransitionPolicy::Manager,
    )
    .await
}

pub async fn approve_observation_request(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: ApproveObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    if !has_required_evaluator(&input.evaluators) {
        return Err(AppError::ValidationError(
            "ต้องมีผู้ประเมินหลักอย่างน้อย 1 คน".to_string(),
        ));
    }

    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.observations.approve_observation_request",
    )
    .await?;
    require_observation_write(&mut tx, observation_id).await?;
    let observation = get_observation_with_connection(&mut tx, observation_id).await?;
    if observation.status != SupervisionObservationStatus::Requested
        && observation.status != SupervisionObservationStatus::Returned
    {
        return Err(AppError::ValidationError(
            "อนุมัติคำขอได้เฉพาะรายการที่รออนุมัติหรือถูกส่งกลับ".to_string(),
        ));
    }

    let cycle = load_cycle_for_request(&mut tx, observation.cycle_id).await?;
    cycle.require_periods(&observation.periods)?;
    ensure_no_submitted_evaluators(&observation)?;

    if input
        .evaluators
        .iter()
        .any(|evaluator| evaluator.evaluator_user_id == observation.observed_user_id)
    {
        return Err(AppError::ValidationError(
            "ครูผู้ถูกนิเทศเป็นผู้ประเมินรายการของตนเองไม่ได้".to_string(),
        ));
    }
    let requested_evaluator_user_ids = input
        .evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect::<Vec<_>>();
    validate_evaluator_availability_for_observation(
        &mut *tx,
        observation_id,
        &requested_evaluator_user_ids,
    )
    .await?;

    DbSession::new(&mut *tx)
        .query(
            "supervision.services.observations.approve_observation_request",
            r#"
        UPDATE supervision_observations
        SET status = 'planned', approved_by = $2, approved_at = now()
        WHERE id = $1
        "#,
        )
        .bind(observation_id)
        .bind(actor_user_id)
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to approve supervision request: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถอนุมัติคำขอนิเทศได้".to_string())
        })?;

    DbSession::new(&mut *tx)
        .query(
            "supervision.services.observations.approve_observation_request",
            "DELETE FROM supervision_evaluators WHERE observation_id = $1",
        )
        .bind(observation_id)
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to reset supervision evaluators: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถกำหนดผู้ประเมินได้".to_string())
        })?;

    insert_supervision_evaluators(&mut tx, observation_id, &input.evaluators).await?;

    insert_observation_action(
        &mut tx,
        observation_id,
        Some(actor_user_id),
        "planned",
        Some(observation.status),
        Some(SupervisionObservationStatus::Planned),
        None,
    )
    .await?;

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub async fn return_observation_request(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: ReturnObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Returned,
        "request_returned",
        input.comment,
        ObservationTransitionPolicy::Manager,
    )
    .await
}

async fn list_observation_rows(
    connection: &mut sqlx::PgConnection,
    access: SupervisionObservationListAccess,
    filter: SupervisionObservationFilter,
) -> Result<Vec<SupervisionObservationRow>, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(observation_select_sql());

    if !access.school {
        if access.is_empty() {
            return Ok(Vec::new());
        }

        let mut has_scope = false;
        builder.push(" AND (");

        if let Some(user_id) = access.own_user_id {
            builder.push("o.observed_user_id = ");
            builder.push_bind(user_id);
            has_scope = true;
        }

        if let Some(user_id) = access.assigned_user_id {
            if has_scope {
                builder.push(" OR ");
            }
            builder.push(
                "EXISTS (
                    SELECT 1 FROM supervision_evaluators e
                    WHERE e.observation_id = o.id AND e.evaluator_user_id = ",
            );
            builder.push_bind(user_id);
            builder.push(")");
            has_scope = true;
        }

        if !access.organization_unit_ids.is_empty() {
            if has_scope {
                builder.push(" OR ");
            }
            builder.push(
                "EXISTS (
                    SELECT 1 FROM organization_members om
                    WHERE om.user_id = o.observed_user_id
                      AND om.organization_unit_id = ANY(",
            );
            builder.push_bind(access.organization_unit_ids);
            builder.push(") AND om.started_at <= CURRENT_DATE AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE))");
        }

        builder.push(")");
    }

    if let Some(cycle_id) = filter.cycle_id {
        builder.push(" AND o.cycle_id = ");
        builder.push_bind(cycle_id);
    }

    builder.push(" AND o.academic_year_id = ");
    builder.push_bind(filter.academic_year_id);

    if let Some(academic_term_id) = filter.academic_term_id {
        builder.push(" AND o.academic_term_id = ");
        builder.push_bind(academic_term_id);
    }

    if let Some(status) = filter.status {
        builder.push(" AND o.status = ");
        builder.push_bind(status.as_str());
    }

    builder.push(" ORDER BY o.created_at DESC");

    DbSession::new(&mut *connection)
        .prepared_query_as(
            "supervision.services.observations.list_observation_rows",
            builder.build_query_as::<SupervisionObservationRow>(),
        )
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to list supervision observations: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงรายการนิเทศได้".to_string())
        })
}

async fn load_observation_row(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<SupervisionObservationRow, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(observation_select_sql());
    builder.push(" AND o.id = ");
    builder.push_bind(id);

    DbSession::new(&mut *connection)
        .prepared_query_as(
            "supervision.services.observations.load_observation_row",
            builder.build_query_as::<SupervisionObservationRow>(),
        )
        .fetch_optional()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision observation: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงรายการนิเทศได้".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการนิเทศ".to_string()))
}

fn observation_select_sql() -> &'static str {
    r#"
    SELECT o.id, o.cycle_id, o.academic_year_id, o.academic_term_id,
           o.observed_user_id,
           NULLIF(u.display_name, '')
               AS observed_display_name,
           o.requested_by, o.approved_by, o.template_id,
           o.status, o.requested_at, o.approved_at,
           o.cancelled_at, o.created_at, o.updated_at
    FROM supervision_observations o
    JOIN user_display_names u ON u.id = o.observed_user_id
    WHERE 1 = 1
    "#
}

async fn hydrate_observations(
    connection: &mut PgConnection,
    rows: Vec<SupervisionObservationRow>,
) -> Result<Vec<SupervisionObservation>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let observation_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let mut evaluators_by_observation =
        load_observation_evaluators(&mut *connection, &observation_ids).await?;
    let mut actions_by_observation =
        load_observation_actions(&mut *connection, &observation_ids).await?;
    let mut periods_by_observation = periods::load(connection, &observation_ids).await?;
    let average_ratings =
        load_observation_average_ratings(&mut *connection, &observation_ids).await?;

    rows.into_iter()
        .map(|row| {
            let observation_id = row.id;
            let periods = periods_by_observation
                .remove(&observation_id)
                .unwrap_or_default();
            let observed_at = periods
                .iter()
                .map(|p| p.observed_at)
                .min()
                .ok_or_else(|| AppError::Conflict("รายการนิเทศไม่มีหลักฐานคาบที่บันทึกไว้".into()))?;
            let actions = actions_by_observation
                .remove(&observation_id)
                .unwrap_or_default();
            let status = parse_observation_status(&row.status)?;
            let periods_locked = has_approval_evidence(status, row.approved_at, &actions);
            Ok(SupervisionObservation {
                periods_locked,
                can_manage: false,
                can_review: false,
                id: observation_id,
                cycle_id: row.cycle_id,
                academic_year_id: row.academic_year_id,
                academic_term_id: row.academic_term_id,
                observed_user_id: row.observed_user_id,
                observed_display_name: row.observed_display_name,
                requested_by: row.requested_by,
                approved_by: row.approved_by,
                template_id: row.template_id,
                observed_at,
                periods,
                status,
                requested_at: row.requested_at,
                approved_at: row.approved_at,
                cancelled_at: row.cancelled_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
                evaluators: evaluators_by_observation
                    .remove(&observation_id)
                    .unwrap_or_default(),
                actions,
                average_rating: average_ratings.get(&observation_id).copied(),
            })
        })
        .collect()
}

async fn load_observation_evaluators(
    connection: &mut PgConnection,
    observation_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<SupervisionEvaluator>>, AppError> {
    let rows = DbSession::new(&mut *connection)
        .query_as::<SupervisionEvaluatorRow>(
            "supervision.services.observations.load_observation_evaluators",
            r#"
        SELECT e.id, e.observation_id, e.evaluator_user_id,
               NULLIF(u.display_name, '')
                   AS evaluator_display_name,
               e.role_label, e.is_required, e.status, e.submitted_at,
               e.created_at, e.updated_at
        FROM supervision_evaluators e
        JOIN user_display_names u ON u.id = e.evaluator_user_id
        WHERE e.observation_id = ANY($1)
        ORDER BY e.observation_id, e.is_required DESC, e.created_at
        "#,
        )
        .bind(observation_ids)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision evaluators: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงผู้ประเมินนิเทศได้".to_string())
        })?;

    let mut evaluators_by_observation = HashMap::new();
    for row in rows {
        let observation_id = row.observation_id;
        evaluators_by_observation
            .entry(observation_id)
            .or_insert_with(Vec::new)
            .push(evaluator_from_row(row)?);
    }
    Ok(evaluators_by_observation)
}

async fn load_observation_actions(
    connection: &mut PgConnection,
    observation_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<SupervisionAction>>, AppError> {
    let rows = DbSession::new(&mut *connection)
        .query_as::<SupervisionActionRow>(
            "supervision.services.observations.load_observation_actions",
            r#"
        SELECT a.id, a.observation_id, a.actor_user_id,
               NULLIF(u.display_name, '')
                   AS actor_display_name,
               a.action_kind, a.from_status, a.to_status, a.comment, a.created_at
        FROM supervision_actions a
        LEFT JOIN user_display_names u ON u.id = a.actor_user_id
        WHERE a.observation_id = ANY($1)
        ORDER BY a.observation_id, a.created_at DESC
        "#,
        )
        .bind(observation_ids)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision actions: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถดึงประวัติรายการนิเทศได้".to_string())
        })?;

    let mut actions_by_observation = HashMap::new();
    for row in rows {
        let observation_id = row.observation_id;
        actions_by_observation
            .entry(observation_id)
            .or_insert_with(Vec::new)
            .push(action_from_row(row)?);
    }
    Ok(actions_by_observation)
}

async fn load_observation_average_ratings(
    connection: &mut PgConnection,
    observation_ids: &[Uuid],
) -> Result<HashMap<Uuid, f64>, AppError> {
    let rows = DbSession::new(&mut *connection)
        .query_as::<(Uuid, f64)>(
            "supervision.services.observations.load_observation_average_ratings",
            r#"
        SELECT observation_id, AVG(evaluator_average)::double precision AS average_rating
        FROM (
            SELECT e.observation_id, e.id,
                   AVG(r.rating_score)::double precision AS evaluator_average
            FROM supervision_evaluators e
            JOIN supervision_evaluator_responses r ON r.evaluator_id = e.id
            JOIN supervision_template_items i ON i.id = r.template_item_id
            WHERE e.observation_id = ANY($1)
              AND e.status = 'submitted'
              AND i.item_type = 'rating'
              AND r.rating_score IS NOT NULL
            GROUP BY e.observation_id, e.id
        ) evaluator_averages
        GROUP BY observation_id
        "#,
        )
        .bind(observation_ids)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision observation averages: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
        })?;

    Ok(rows.into_iter().collect())
}

fn action_from_row(row: SupervisionActionRow) -> Result<SupervisionAction, AppError> {
    Ok(SupervisionAction {
        id: row.id,
        observation_id: row.observation_id,
        actor_user_id: row.actor_user_id,
        actor_display_name: row.actor_display_name,
        action_kind: row.action_kind,
        from_status: parse_optional_observation_status(row.from_status)?,
        to_status: parse_optional_observation_status(row.to_status)?,
        comment: row.comment,
        created_at: row.created_at,
    })
}

pub(super) async fn load_cycle_for_request(
    connection: &mut PgConnection,
    cycle_id: Uuid,
) -> Result<CycleForRequestRow, AppError> {
    DbSession::new(&mut *connection)
        .query_as::<CycleForRequestRow>(
            "supervision.services.observations.load_cycle_for_request",
            r#"
        SELECT id, academic_year_id, academic_term_id, template_id, status, observation_start_date, observation_end_date
        FROM supervision_cycles
        WHERE id = $1
        "#,
        )
        .bind(cycle_id)
        .fetch_optional()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision cycle for request: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบรอบนิเทศได้".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบนิเทศ".to_string()))
}

pub(super) async fn ensure_cycle_target_allows_teacher(
    connection: &mut PgConnection,
    cycle_id: Uuid,
    staff_user_id: Uuid,
) -> Result<(), AppError> {
    let rows = DbSession::new(&mut *connection)
        .query_as::<SupervisionCycleTargetRow>(
            "supervision.services.observations.ensure_cycle_target_allows_teacher",
            r#"
        SELECT id, cycle_id, target_type, target_id, required_observations,
               priority, created_at, updated_at
        FROM supervision_cycle_targets
        WHERE cycle_id = $1
        "#,
        )
        .bind(cycle_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision target rules: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบเป้าหมายรอบนิเทศได้".to_string())
        })?;

    if rows.is_empty() {
        return Err(AppError::ValidationError(
            "รอบนิเทศยังไม่ได้กำหนดเป้าหมาย".to_string(),
        ));
    }

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(SupervisionTargetRule {
            target_type: parse_target_type(&row.target_type)?,
            target_id: row.target_id,
            required_observations: row.required_observations,
            priority: row.priority,
        });
    }

    let staff_match = load_supervision_target_match(connection, staff_user_id).await?;
    if resolve_supervision_target_rule(&rules, &staff_match).is_some() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "ไม่ได้อยู่ในกลุ่มเป้าหมายของรอบนิเทศนี้".to_string(),
        ))
    }
}

async fn load_supervision_target_match(
    connection: &mut PgConnection,
    staff_user_id: Uuid,
) -> Result<SupervisionTargetMatch, AppError> {
    let organization_unit_ids = DbSession::new(&mut *connection)
        .scalar::<Uuid>(
            "supervision.services.observations.load_supervision_target_match",
            r#"
        SELECT DISTINCT organization_unit_id
        FROM organization_members
        WHERE user_id = $1
          AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        "#,
        )
        .bind(staff_user_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision target organization units: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบหน่วยงานของครูได้".to_string())
        })?;

    let subject_group_ids = DbSession::new(&mut *connection)
        .scalar::<Uuid>(
            "supervision.services.observations.load_supervision_target_match",
            r#"
        SELECT DISTINCT ou.subject_group_id
        FROM organization_members om
        JOIN organization_units ou ON ou.id = om.organization_unit_id
        WHERE om.user_id = $1
          AND ou.subject_group_id IS NOT NULL
          AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
        "#,
        )
        .bind(staff_user_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision target subject groups: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบกลุ่มสาระของครูได้".to_string())
        })?;

    Ok(SupervisionTargetMatch {
        staff_user_id,
        subject_group_ids,
        organization_unit_ids,
    })
}

fn validate_cycle_accepts_requests(cycle: &CycleForRequestRow) -> Result<(), AppError> {
    if parse_cycle_status(&cycle.status)? != SupervisionCycleStatus::Open {
        return Err(AppError::ValidationError("รอบนิเทศยังไม่เปิดให้จอง".to_string()));
    }

    Ok(())
}

pub(super) async fn validate_observation_context(
    connection: &mut PgConnection,
    cycle: &CycleForRequestRow,
    academic_term_id: Uuid,
) -> Result<(), AppError> {
    if cycle
        .academic_term_id
        .is_some_and(|cycle_term_id| cycle_term_id != academic_term_id)
    {
        return Err(AppError::ValidationError(
            "ภาคเรียนของรายการนิเทศไม่ตรงกับรอบนิเทศ".to_string(),
        ));
    }

    let term_belongs_to_year: bool = DbSession::new(&mut *connection)
        .scalar(
            "supervision.services.observations.validate_observation_context",
            "SELECT EXISTS (
            SELECT 1 FROM academic_terms
            WHERE id = $1 AND academic_year_id = $2
        )",
        )
        .bind(academic_term_id)
        .bind(cycle.academic_year_id)
        .fetch_one()
        .await?;

    if !term_belongs_to_year {
        return Err(AppError::ValidationError(
            "ภาคเรียนของรายการนิเทศไม่อยู่ในปีการศึกษาของรอบนิเทศ".to_string(),
        ));
    }

    Ok(())
}

fn manager_selection(
    input: &UpdateSupervisionObservationRequest,
) -> Result<Option<(NaiveDate, &[Uuid], &str)>, AppError> {
    match (
        input.observed_date,
        input.timetable_block_group_ids.as_deref(),
        input.expected_source_digest.as_deref(),
    ) {
        (None, None, None) => Ok(None),
        (Some(date), Some(ids), Some(digest)) => Ok(Some((date, ids, digest))),
        _ => Err(AppError::ValidationError(
            "ต้องระบุวันที่ คาบสอน และข้อมูลตรวจสอบตารางสอนพร้อมกัน".into(),
        )),
    }
}

fn ensure_no_submitted_evaluators(observation: &SupervisionObservation) -> Result<(), AppError> {
    if observation
        .evaluators
        .iter()
        .any(|evaluator| evaluator.status == crate::models::SupervisionEvaluatorStatus::Submitted)
    {
        return Err(AppError::ValidationError(
            "รายการนี้มีผลประเมินที่ส่งแล้ว ไม่สามารถส่งกลับ ส่งคำขอใหม่ หรือกำหนดผู้ประเมินใหม่ได้".into(),
        ));
    }
    Ok(())
}

pub(super) enum ObservationTransitionPolicy {
    Manager,
    RequestedTeacher,
    Certification,
    ObservedTeacher,
}

pub(super) async fn set_observation_status(
    pool: &PgPool,
    observation_id: Uuid,
    actor_user_id: Uuid,
    to_status: SupervisionObservationStatus,
    action_kind: &str,
    comment: Option<String>,
    policy: ObservationTransitionPolicy,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.observations.set_observation_status",
    )
    .await?;
    if matches!(
        policy,
        ObservationTransitionPolicy::RequestedTeacher
            | ObservationTransitionPolicy::ObservedTeacher
    ) {
        let current = get_observation_with_connection(&mut tx, observation_id).await?;
        if current.observed_user_id != actor_user_id {
            return Err(AppError::Forbidden("แก้ไขรายการนิเทศของผู้อื่นไม่ได้".into()));
        }
    }
    require_observation_write(&mut tx, observation_id).await?;
    let current = get_observation_with_connection(&mut tx, observation_id).await?;
    if to_status == SupervisionObservationStatus::Returned {
        ensure_no_submitted_evaluators(&current)?;
    }
    match policy {
        ObservationTransitionPolicy::RequestedTeacher
            if !teacher_can_edit_requested_observation(current.status) =>
        {
            return Err(AppError::ValidationError(
                "ยกเลิกคำขอนิเทศได้เฉพาะสถานะรออนุมัติหรือส่งกลับ".into(),
            ));
        }
        ObservationTransitionPolicy::Certification => {
            let states = load_evaluator_submission_states(&mut *tx, observation_id).await?;
            if !all_required_evaluators_submitted(&states) {
                return Err(AppError::ValidationError("ผู้ประเมินหลักยังส่งผลไม่ครบ".into()));
            }
        }
        _ => {}
    }
    if !can_transition_observation_status(current.status, to_status) {
        return Err(AppError::ValidationError(
            "ไม่สามารถเปลี่ยนสถานะนิเทศตามลำดับนี้ได้".to_string(),
        ));
    }

    DbSession::new(&mut *tx)
        .query(
            "supervision.services.observations.set_observation_status",
            r#"
        UPDATE supervision_observations
        SET status = $2,
            cancelled_at = CASE WHEN $2 = 'cancelled' THEN now() ELSE cancelled_at END
        WHERE id = $1
        "#,
        )
        .bind(observation_id)
        .bind(to_status.as_str())
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to set supervision observation status: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถอัปเดตสถานะนิเทศได้".to_string())
        })?;

    insert_observation_action(
        &mut tx,
        observation_id,
        Some(actor_user_id),
        action_kind,
        Some(current.status),
        Some(to_status),
        comment,
    )
    .await?;

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub(super) async fn insert_observation_action(
    tx: &mut Transaction<'_, Postgres>,
    observation_id: Uuid,
    actor_user_id: Option<Uuid>,
    action_kind: &str,
    from_status: Option<SupervisionObservationStatus>,
    to_status: Option<SupervisionObservationStatus>,
    comment: Option<String>,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx)
        .query(
            "supervision.services.observations.insert_observation_action",
            r#"
        INSERT INTO supervision_actions (
            observation_id, actor_user_id, action_kind, from_status, to_status, comment
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(observation_id)
        .bind(actor_user_id)
        .bind(action_kind)
        .bind(from_status.map(SupervisionObservationStatus::as_str))
        .bind(to_status.map(SupervisionObservationStatus::as_str))
        .bind(comment)
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to insert supervision action: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถบันทึกประวัตินิเทศได้".to_string())
        })?;

    Ok(())
}

#[cfg(test)]
mod connection_policy_tests {
    use super::*;
    #[tokio::test]
    async fn empty_observation_scope_does_not_acquire_a_connection() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        let rows = list_observations(
            &pool,
            SupervisionObservationListAccess::default(),
            SupervisionObservationFilter {
                academic_year_id: Uuid::new_v4(),
                academic_term_id: None,
                cycle_id: None,
                status: None,
            },
        )
        .await
        .unwrap();
        assert!(rows.is_empty());
    }
}
