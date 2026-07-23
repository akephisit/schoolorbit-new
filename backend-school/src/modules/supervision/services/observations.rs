use chrono::{DateTime, Datelike, FixedOffset, Utc, Weekday};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable::TimetableEntry;
use crate::modules::academic::services::timetable_service::{self, TimetableFilter};
use crate::modules::supervision::models::{
    ApproveObservationRequest, CancelObservationRequest, LessonSnapshot, ManualLesson,
    ManualLessonInput, RequestSupervisionObservationRequest, ReturnObservationRequest,
    SupervisionAction, SupervisionCycleStatus, SupervisionEvaluator,
    SupervisionEvaluatorAvailability, SupervisionObservation, SupervisionObservationFilter,
    SupervisionObservationStatus, UpdateRequestedObservationRequest,
    UpdateSupervisionObservationRequest,
};

use super::cycles::SupervisionCycleTargetRow;
use super::evaluations::{
    evaluator_availability_from_row, insert_supervision_evaluators,
    validate_evaluator_availability_for_observation, EvaluatorAvailabilityRow,
};
use super::reviews_and_reports::fetch_observation_average_rating;
use super::shared::{
    can_transition_observation_status, evaluator_conflict_status_codes, has_required_evaluator,
    manager_can_edit_observation, parse_cycle_status, parse_evaluator_status,
    parse_observation_status, parse_optional_observation_status, parse_target_type,
    resolve_supervision_target_rule, teacher_can_edit_requested_observation,
    SupervisionObservationListAccess, SupervisionTargetMatch, SupervisionTargetRule,
};

#[derive(Debug, sqlx::FromRow)]
struct SupervisionObservationRow {
    id: Uuid,
    cycle_id: Uuid,
    observed_user_id: Uuid,
    observed_display_name: Option<String>,
    requested_by: Option<Uuid>,
    approved_by: Option<Uuid>,
    template_id: Uuid,
    timetable_entry_id: Option<Uuid>,
    observed_at: DateTime<Utc>,
    manual_subject_name: Option<String>,
    manual_classroom_label: Option<String>,
    manual_room_label: Option<String>,
    manual_period_label: Option<String>,
    manual_reason: Option<String>,
    lesson_snapshot: Json<LessonSnapshot>,
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
struct CycleForRequestRow {
    id: Uuid,
    template_id: Uuid,
    academic_semester_id: Option<Uuid>,
    status: String,
    booking_opens_at: Option<DateTime<Utc>>,
    booking_closes_at: Option<DateTime<Utc>>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

pub async fn list_observations(
    pool: &PgPool,
    access: SupervisionObservationListAccess,
    filter: SupervisionObservationFilter,
) -> Result<Vec<SupervisionObservation>, AppError> {
    let rows = list_observation_rows(pool, access, filter).await?;
    let mut observations = Vec::with_capacity(rows.len());
    for row in rows {
        observations.push(observation_from_row(pool, row).await?);
    }
    Ok(observations)
}

pub async fn get_observation(pool: &PgPool, id: Uuid) -> Result<SupervisionObservation, AppError> {
    let row = load_observation_row(pool, id).await?;
    observation_from_row(pool, row).await
}

pub async fn evaluator_availability(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<SupervisionEvaluatorAvailability>, AppError> {
    let observation = get_observation(pool, observation_id).await?;
    let conflict_statuses = evaluator_conflict_status_codes()
        .iter()
        .map(|status| (*status).to_string())
        .collect::<Vec<_>>();

    let rows = sqlx::query_as::<_, EvaluatorAvailabilityRow>(
        r#"
        SELECT u.id, u.title, u.first_name, u.last_name,
               conflict.observation_id AS conflict_observation_id,
               conflict.observed_display_name AS conflict_observed_display_name,
               conflict.observed_at AS conflict_observed_at,
               conflict.subject_name AS conflict_subject_name,
               conflict.period_label AS conflict_period_label
        FROM users u
        LEFT JOIN LATERAL (
            SELECT o.id AS observation_id,
                   NULLIF(TRIM(CONCAT(COALESCE(observed.title, ''), observed.first_name, ' ', observed.last_name)), '')
                       AS observed_display_name,
                   o.observed_at,
                   COALESCE(NULLIF(o.manual_subject_name, ''), NULLIF(o.lesson_snapshot->>'subjectName', ''))
                       AS subject_name,
                   COALESCE(NULLIF(o.manual_period_label, ''), NULLIF(o.lesson_snapshot->>'periodLabel', ''))
                       AS period_label
            FROM supervision_evaluators e
            JOIN supervision_observations o ON o.id = e.observation_id
            JOIN users observed ON observed.id = o.observed_user_id
            WHERE e.evaluator_user_id = u.id
              AND o.id <> $1
              AND o.observed_at = $2
              AND o.status = ANY($3::text[])
            ORDER BY o.approved_at DESC NULLS LAST, o.created_at DESC
            LIMIT 1
        ) conflict ON true
        WHERE u.user_type = 'staff'
          AND u.status = 'active'
          AND u.id <> $4
        ORDER BY (conflict.observation_id IS NOT NULL), u.first_name, u.last_name
        "#,
    )
    .bind(observation_id)
    .bind(observation.observed_at)
    .bind(&conflict_statuses)
    .bind(observation.observed_user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision evaluator availability: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(evaluator_availability_from_row)
        .collect())
}

pub async fn observation_timetable_options(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<TimetableEntry>, AppError> {
    let observation = get_observation(pool, observation_id).await?;
    let cycle = load_cycle_for_request(pool, observation.cycle_id).await?;

    timetable_service::list_entries(
        pool,
        TimetableFilter {
            instructor_id: Some(observation.observed_user_id),
            academic_semester_id: cycle.academic_semester_id,
            include_team_ghosts: true,
            ..TimetableFilter::default()
        },
    )
    .await
}

pub async fn request_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    input: RequestSupervisionObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let cycle = load_cycle_for_request(pool, input.cycle_id).await?;
    validate_cycle_accepts_requests(&cycle)?;
    ensure_cycle_target_allows_teacher(pool, cycle.id, actor_user_id).await?;

    let lesson = resolve_lesson_input(
        pool,
        &cycle,
        actor_user_id,
        input.timetable_entry_id,
        input.observed_at,
        input.manual_lesson,
    )
    .await?;

    let observation_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO supervision_observations (
            cycle_id, observed_user_id, requested_by, template_id, timetable_entry_id,
            manual_subject_name, manual_classroom_label, manual_room_label,
            observed_at, manual_period_label, manual_reason, lesson_snapshot
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(cycle.id)
    .bind(actor_user_id)
    .bind(actor_user_id)
    .bind(cycle.template_id)
    .bind(lesson.timetable_entry_id)
    .bind(&lesson.manual_subject_name)
    .bind(&lesson.manual_classroom_label)
    .bind(&lesson.manual_room_label)
    .bind(lesson.observed_at)
    .bind(&lesson.manual_period_label)
    .bind(&lesson.manual_reason)
    .bind(Json(lesson.snapshot))
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to request supervision observation: {}", error);
        AppError::InternalServerError("ไม่สามารถส่งคำขอนิเทศได้".to_string())
    })?;

    insert_observation_action(
        pool,
        observation_id,
        Some(actor_user_id),
        "requested",
        None,
        Some(SupervisionObservationStatus::Requested),
        None,
    )
    .await?;

    get_observation(pool, observation_id).await
}

pub async fn update_requested_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: UpdateRequestedObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let current = get_observation(pool, observation_id).await?;
    if current.observed_user_id != actor_user_id {
        return Err(AppError::Forbidden("แก้ไขคำขอนิเทศของผู้อื่นไม่ได้".to_string()));
    }
    if !teacher_can_edit_requested_observation(current.status) {
        return Err(AppError::ValidationError(
            "แก้ไขคำขอนิเทศได้เฉพาะสถานะรออนุมัติ".to_string(),
        ));
    }

    let cycle = load_cycle_for_request(pool, current.cycle_id).await?;
    let lesson = resolve_lesson_input(
        pool,
        &cycle,
        actor_user_id,
        input.timetable_entry_id,
        input.observed_at,
        input.manual_lesson,
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE supervision_observations
        SET timetable_entry_id = $2,
            manual_subject_name = $3,
            manual_classroom_label = $4,
            manual_room_label = $5,
            observed_at = $6,
            manual_period_label = $7,
            manual_reason = $8,
            lesson_snapshot = $9
        WHERE id = $1
        "#,
    )
    .bind(observation_id)
    .bind(lesson.timetable_entry_id)
    .bind(&lesson.manual_subject_name)
    .bind(&lesson.manual_classroom_label)
    .bind(&lesson.manual_room_label)
    .bind(lesson.observed_at)
    .bind(&lesson.manual_period_label)
    .bind(&lesson.manual_reason)
    .bind(Json(lesson.snapshot))
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update supervision request: {}", error);
        AppError::InternalServerError("ไม่สามารถแก้ไขคำขอนิเทศได้".to_string())
    })?;

    get_observation(pool, observation_id).await
}

pub async fn cancel_requested_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    let current = get_observation(pool, observation_id).await?;
    if current.observed_user_id != actor_user_id {
        return Err(AppError::Forbidden("ยกเลิกคำขอนิเทศของผู้อื่นไม่ได้".to_string()));
    }
    if !teacher_can_edit_requested_observation(current.status) {
        return Err(AppError::ValidationError(
            "ยกเลิกคำขอนิเทศได้เฉพาะสถานะรออนุมัติ".to_string(),
        ));
    }

    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Cancelled,
        "request_cancelled",
        None,
    )
    .await
}

pub async fn update_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: UpdateSupervisionObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let current = get_observation(pool, observation_id).await?;
    if !manager_can_edit_observation(current.status) {
        return Err(AppError::ValidationError(
            "แก้ไขรายการนิเทศได้เฉพาะสถานะรออนุมัติ วางแผน หรือส่งกลับ".to_string(),
        ));
    }

    let cycle = load_cycle_for_request(pool, current.cycle_id).await?;
    let template_id = input.template_id.unwrap_or(current.template_id);
    let manual_lesson = match (input.manual_lesson, current.manual_lesson) {
        (Some(manual), _) => Some(manual),
        (None, Some(manual)) if input.timetable_entry_id.is_none() => Some(ManualLessonInput {
            subject_name: manual.subject_name,
            classroom_label: manual.classroom_label,
            room_label: manual.room_label,
            observed_at: input.observed_at.unwrap_or(manual.observed_at),
            period_label: manual.period_label,
            reason: manual.reason,
        }),
        (None, _) => None,
    };
    let timetable_entry_id = if manual_lesson.is_some() {
        None
    } else {
        input.timetable_entry_id.or(current.timetable_entry_id)
    };
    let observed_at = if manual_lesson.is_some() {
        None
    } else {
        Some(input.observed_at.unwrap_or(current.observed_at))
    };
    let lesson = resolve_lesson_input(
        pool,
        &cycle,
        current.observed_user_id,
        timetable_entry_id,
        observed_at,
        manual_lesson,
    )
    .await?;
    let evaluator_user_ids = current
        .evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect::<Vec<_>>();
    validate_evaluator_availability_for_observation(
        pool,
        observation_id,
        lesson.observed_at,
        &evaluator_user_ids,
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE supervision_observations
        SET template_id = $2,
            timetable_entry_id = $3,
            manual_subject_name = $4,
            manual_classroom_label = $5,
            manual_room_label = $6,
            observed_at = $7,
            manual_period_label = $8,
            manual_reason = $9,
            lesson_snapshot = $10
        WHERE id = $1
        "#,
    )
    .bind(observation_id)
    .bind(template_id)
    .bind(lesson.timetable_entry_id)
    .bind(&lesson.manual_subject_name)
    .bind(&lesson.manual_classroom_label)
    .bind(&lesson.manual_room_label)
    .bind(lesson.observed_at)
    .bind(&lesson.manual_period_label)
    .bind(&lesson.manual_reason)
    .bind(Json(lesson.snapshot))
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update supervision observation: {}", error);
        AppError::InternalServerError("ไม่สามารถแก้ไขรายการนิเทศได้".to_string())
    })?;

    insert_observation_action(
        pool,
        observation_id,
        Some(actor_user_id),
        "updated",
        Some(current.status),
        Some(current.status),
        None,
    )
    .await?;

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

    let observation = get_observation(pool, observation_id).await?;
    if observation.status != SupervisionObservationStatus::Requested
        && observation.status != SupervisionObservationStatus::Returned
    {
        return Err(AppError::ValidationError(
            "อนุมัติคำขอได้เฉพาะรายการที่รออนุมัติหรือถูกส่งกลับ".to_string(),
        ));
    }

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
        pool,
        observation_id,
        observation.observed_at,
        &requested_evaluator_user_ids,
    )
    .await?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin approve supervision request transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มอนุมัติคำขอนิเทศได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE supervision_observations
        SET status = 'planned', approved_by = $2, approved_at = now()
        WHERE id = $1
        "#,
    )
    .bind(observation_id)
    .bind(actor_user_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to approve supervision request: {}", error);
        AppError::InternalServerError("ไม่สามารถอนุมัติคำขอนิเทศได้".to_string())
    })?;

    sqlx::query("DELETE FROM supervision_evaluators WHERE observation_id = $1")
        .bind(observation_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!("Failed to reset supervision evaluators: {}", error);
            AppError::InternalServerError("ไม่สามารถกำหนดผู้ประเมินได้".to_string())
        })?;

    insert_supervision_evaluators(&mut tx, observation_id, &input.evaluators).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit approve supervision request transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกการอนุมัติคำขอนิเทศได้".to_string())
    })?;

    insert_observation_action(
        pool,
        observation_id,
        Some(actor_user_id),
        "planned",
        Some(observation.status),
        Some(SupervisionObservationStatus::Planned),
        None,
    )
    .await?;

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
    )
    .await
}

async fn list_observation_rows(
    pool: &PgPool,
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
            builder.push(") AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE))");
        }

        builder.push(")");
    }

    if let Some(cycle_id) = filter.cycle_id {
        builder.push(" AND o.cycle_id = ");
        builder.push_bind(cycle_id);
    }

    if let Some(status) = filter.status {
        builder.push(" AND o.status = ");
        builder.push_bind(status.as_str());
    }

    builder.push(" ORDER BY o.created_at DESC");

    builder
        .build_query_as::<SupervisionObservationRow>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to list supervision observations: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงรายการนิเทศได้".to_string())
        })
}

async fn load_observation_row(
    pool: &PgPool,
    id: Uuid,
) -> Result<SupervisionObservationRow, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(observation_select_sql());
    builder.push(" AND o.id = ");
    builder.push_bind(id);

    builder
        .build_query_as::<SupervisionObservationRow>()
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to load supervision observation: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงรายการนิเทศได้".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการนิเทศ".to_string()))
}

fn observation_select_sql() -> &'static str {
    r#"
    SELECT o.id, o.cycle_id, o.observed_user_id,
           NULLIF(TRIM(CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)), '')
               AS observed_display_name,
           o.requested_by, o.approved_by, o.template_id, o.timetable_entry_id,
           o.observed_at,
           o.manual_subject_name, o.manual_classroom_label, o.manual_room_label,
           o.manual_period_label, o.manual_reason,
           o.lesson_snapshot, o.status, o.requested_at, o.approved_at,
           o.cancelled_at, o.created_at, o.updated_at
    FROM supervision_observations o
    JOIN users u ON u.id = o.observed_user_id
    WHERE 1 = 1
    "#
}

async fn observation_from_row(
    pool: &PgPool,
    row: SupervisionObservationRow,
) -> Result<SupervisionObservation, AppError> {
    let evaluators = load_observation_evaluators(pool, row.id).await?;
    let actions = load_observation_actions(pool, row.id).await?;
    let average_rating = fetch_observation_average_rating(pool, row.id).await?;
    let manual_lesson = manual_lesson_from_row(&row);

    Ok(SupervisionObservation {
        id: row.id,
        cycle_id: row.cycle_id,
        observed_user_id: row.observed_user_id,
        observed_display_name: row.observed_display_name,
        requested_by: row.requested_by,
        approved_by: row.approved_by,
        template_id: row.template_id,
        timetable_entry_id: row.timetable_entry_id,
        observed_at: row.observed_at,
        manual_lesson,
        lesson_snapshot: row.lesson_snapshot.0,
        status: parse_observation_status(&row.status)?,
        requested_at: row.requested_at,
        approved_at: row.approved_at,
        cancelled_at: row.cancelled_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        evaluators,
        actions,
        average_rating,
    })
}

fn manual_lesson_from_row(row: &SupervisionObservationRow) -> Option<ManualLesson> {
    Some(ManualLesson {
        subject_name: row.manual_subject_name.clone()?,
        classroom_label: row.manual_classroom_label.clone()?,
        room_label: row.manual_room_label.clone(),
        observed_at: row.observed_at,
        period_label: row.manual_period_label.clone()?,
        reason: row.manual_reason.clone()?,
    })
}

async fn load_observation_evaluators(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<SupervisionEvaluator>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionEvaluatorRow>(
        r#"
        SELECT e.id, e.observation_id, e.evaluator_user_id,
               NULLIF(TRIM(CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)), '')
                   AS evaluator_display_name,
               e.role_label, e.is_required, e.status, e.submitted_at,
               e.created_at, e.updated_at
        FROM supervision_evaluators e
        JOIN users u ON u.id = e.evaluator_user_id
        WHERE e.observation_id = $1
        ORDER BY e.is_required DESC, e.created_at
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision evaluators: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงผู้ประเมินนิเทศได้".to_string())
    })?;

    rows.into_iter().map(evaluator_from_row).collect()
}

async fn load_observation_actions(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<SupervisionAction>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionActionRow>(
        r#"
        SELECT a.id, a.observation_id, a.actor_user_id,
               NULLIF(TRIM(CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)), '')
                   AS actor_display_name,
               a.action_kind, a.from_status, a.to_status, a.comment, a.created_at
        FROM supervision_actions a
        LEFT JOIN users u ON u.id = a.actor_user_id
        WHERE a.observation_id = $1
        ORDER BY a.created_at DESC
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision actions: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงประวัติรายการนิเทศได้".to_string())
    })?;

    rows.into_iter().map(action_from_row).collect()
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

async fn load_cycle_for_request(
    pool: &PgPool,
    cycle_id: Uuid,
) -> Result<CycleForRequestRow, AppError> {
    sqlx::query_as::<_, CycleForRequestRow>(
        r#"
        SELECT id, template_id, academic_semester_id, status,
               booking_opens_at, booking_closes_at, starts_at, ends_at
        FROM supervision_cycles
        WHERE id = $1
        "#,
    )
    .bind(cycle_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle for request: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบรอบนิเทศได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบนิเทศ".to_string()))
}

async fn ensure_cycle_target_allows_teacher(
    pool: &PgPool,
    cycle_id: Uuid,
    staff_user_id: Uuid,
) -> Result<(), AppError> {
    let rows = sqlx::query_as::<_, SupervisionCycleTargetRow>(
        r#"
        SELECT id, cycle_id, target_type, target_id, required_observations,
               priority, created_at, updated_at
        FROM supervision_cycle_targets
        WHERE cycle_id = $1
        "#,
    )
    .bind(cycle_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision target rules: {}", error);
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

    let staff_match = load_supervision_target_match(pool, staff_user_id).await?;
    if resolve_supervision_target_rule(&rules, &staff_match).is_some() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "ไม่ได้อยู่ในกลุ่มเป้าหมายของรอบนิเทศนี้".to_string(),
        ))
    }
}

async fn load_supervision_target_match(
    pool: &PgPool,
    staff_user_id: Uuid,
) -> Result<SupervisionTargetMatch, AppError> {
    let organization_unit_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT DISTINCT organization_unit_id
        FROM organization_members
        WHERE user_id = $1
          AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        "#,
    )
    .bind(staff_user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to load supervision target organization units: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบหน่วยงานของครูได้".to_string())
    })?;

    let subject_group_ids = sqlx::query_scalar::<_, Uuid>(
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
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to load supervision target subject groups: {}",
            error
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

    let now = Utc::now();
    if cycle
        .booking_opens_at
        .is_some_and(|opens_at| now < opens_at)
    {
        return Err(AppError::ValidationError(
            "ยังไม่ถึงเวลาเปิดจองนิเทศ".to_string(),
        ));
    }
    if cycle
        .booking_closes_at
        .is_some_and(|closes_at| now > closes_at)
    {
        return Err(AppError::ValidationError("หมดเวลาจองนิเทศแล้ว".to_string()));
    }
    if now < cycle.starts_at || now > cycle.ends_at {
        return Err(AppError::ValidationError("อยู่นอกช่วงเวลารอบนิเทศ".to_string()));
    }

    Ok(())
}

struct ResolvedLessonInput {
    timetable_entry_id: Option<Uuid>,
    observed_at: DateTime<Utc>,
    manual_subject_name: Option<String>,
    manual_classroom_label: Option<String>,
    manual_room_label: Option<String>,
    manual_period_label: Option<String>,
    manual_reason: Option<String>,
    snapshot: LessonSnapshot,
}

async fn resolve_lesson_input(
    pool: &PgPool,
    cycle: &CycleForRequestRow,
    actor_user_id: Uuid,
    timetable_entry_id: Option<Uuid>,
    observed_at: Option<DateTime<Utc>>,
    manual_lesson: Option<ManualLessonInput>,
) -> Result<ResolvedLessonInput, AppError> {
    match (timetable_entry_id, observed_at, manual_lesson) {
        (Some(entry_id), Some(observed_at), None) => {
            validate_observed_at_in_cycle(cycle, observed_at)?;
            let entry_day =
                load_timetable_entry_day_for_teacher(pool, entry_id, actor_user_id).await?;
            if !day_of_week_matches_observed_at(&entry_day, observed_at) {
                return Err(AppError::ValidationError(
                    "วันที่นิเทศไม่ตรงกับวันของคาบสอน".to_string(),
                ));
            }
            Ok(ResolvedLessonInput {
                timetable_entry_id: Some(entry_id),
                observed_at,
                manual_subject_name: None,
                manual_classroom_label: None,
                manual_room_label: None,
                manual_period_label: None,
                manual_reason: None,
                snapshot: load_timetable_lesson_snapshot(pool, entry_id, observed_at).await?,
            })
        }
        (Some(_), None, None) => Err(AppError::ValidationError(
            "ต้องระบุวันที่นิเทศสำหรับคาบจากตารางสอน".to_string(),
        )),
        (None, _, Some(manual)) => {
            validate_manual_lesson(&manual)?;
            validate_observed_at_in_cycle(cycle, manual.observed_at)?;
            Ok(ResolvedLessonInput {
                timetable_entry_id: None,
                observed_at: manual.observed_at,
                manual_subject_name: Some(manual.subject_name.clone()),
                manual_classroom_label: Some(manual.classroom_label.clone()),
                manual_room_label: manual.room_label.clone(),
                manual_period_label: Some(manual.period_label.clone()),
                manual_reason: Some(manual.reason.clone()),
                snapshot: manual.snapshot(),
            })
        }
        _ => Err(AppError::ValidationError(
            "ต้องเลือกคาบจากตารางสอนหรือระบุคาบแบบกำหนดเองอย่างใดอย่างหนึ่ง".to_string(),
        )),
    }
}

fn validate_observed_at_in_cycle(
    cycle: &CycleForRequestRow,
    observed_at: DateTime<Utc>,
) -> Result<(), AppError> {
    if observed_at < cycle.starts_at || observed_at > cycle.ends_at {
        return Err(AppError::ValidationError(
            "วันที่นิเทศอยู่นอกช่วงรอบนิเทศ".to_string(),
        ));
    }

    Ok(())
}

fn day_of_week_matches_observed_at(day_of_week: &str, observed_at: DateTime<Utc>) -> bool {
    let Some(bangkok_offset) = FixedOffset::east_opt(7 * 60 * 60) else {
        return false;
    };
    let observed_weekday = observed_at.with_timezone(&bangkok_offset).weekday();
    matches!(
        (day_of_week, observed_weekday),
        ("MON", Weekday::Mon)
            | ("TUE", Weekday::Tue)
            | ("WED", Weekday::Wed)
            | ("THU", Weekday::Thu)
            | ("FRI", Weekday::Fri)
            | ("SAT", Weekday::Sat)
            | ("SUN", Weekday::Sun)
    )
}

fn validate_manual_lesson(manual: &ManualLessonInput) -> Result<(), AppError> {
    if manual.subject_name.trim().is_empty()
        || manual.classroom_label.trim().is_empty()
        || manual.period_label.trim().is_empty()
        || manual.reason.trim().is_empty()
    {
        return Err(AppError::ValidationError(
            "คาบแบบกำหนดเองต้องระบุวิชา ห้อง คาบ และเหตุผล".to_string(),
        ));
    }
    Ok(())
}

async fn load_timetable_entry_day_for_teacher(
    pool: &PgPool,
    entry_id: Uuid,
    teacher_user_id: Uuid,
) -> Result<String, AppError> {
    let day_of_week = sqlx::query_scalar::<_, String>(
        r#"
        SELECT e.day_of_week
        FROM academic_timetable_entries e
        JOIN timetable_entry_instructors tei ON tei.entry_id = e.id
        WHERE e.id = $1 AND tei.instructor_id = $2
        "#,
    )
    .bind(entry_id)
    .bind(teacher_user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to validate supervision timetable entry: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบคาบสอนได้".to_string())
    })?;

    day_of_week.ok_or_else(|| AppError::Forbidden("เลือกจองได้เฉพาะคาบสอนของตนเอง".to_string()))
}

async fn load_timetable_lesson_snapshot(
    pool: &PgPool,
    entry_id: Uuid,
    observed_at: DateTime<Utc>,
) -> Result<LessonSnapshot, AppError> {
    let row = sqlx::query(
        r#"
        SELECT s.name_th AS subject_name,
               cr.name AS classroom_label,
               r.name_th AS room_label,
               p.name AS period_label
        FROM academic_timetable_entries e
        LEFT JOIN classroom_courses cc ON e.classroom_course_id = cc.id
        LEFT JOIN subjects s ON cc.subject_id = s.id
        LEFT JOIN class_rooms cr ON COALESCE(e.classroom_id, cc.classroom_id) = cr.id
        LEFT JOIN rooms r ON e.room_id = r.id
        LEFT JOIN academic_periods p ON e.period_id = p.id
        WHERE e.id = $1
        "#,
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision lesson snapshot: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลคาบสอนได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบคาบสอน".to_string()))?;

    Ok(LessonSnapshot {
        source: Some("timetable".to_string()),
        timetable_entry_id: Some(entry_id),
        subject_name: row.try_get("subject_name").ok(),
        classroom_label: row.try_get("classroom_label").ok(),
        room_label: row.try_get("room_label").ok(),
        observed_at: Some(observed_at),
        period_label: row.try_get("period_label").ok(),
    })
}

pub(super) async fn set_observation_status(
    pool: &PgPool,
    observation_id: Uuid,
    actor_user_id: Uuid,
    to_status: SupervisionObservationStatus,
    action_kind: &str,
    comment: Option<String>,
) -> Result<SupervisionObservation, AppError> {
    let current = get_observation(pool, observation_id).await?;
    if !can_transition_observation_status(current.status, to_status) {
        return Err(AppError::ValidationError(
            "ไม่สามารถเปลี่ยนสถานะนิเทศตามลำดับนี้ได้".to_string(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE supervision_observations
        SET status = $2,
            cancelled_at = CASE WHEN $2 = 'cancelled' THEN now() ELSE cancelled_at END
        WHERE id = $1
        "#,
    )
    .bind(observation_id)
    .bind(to_status.as_str())
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to set supervision observation status: {}", error);
        AppError::InternalServerError("ไม่สามารถอัปเดตสถานะนิเทศได้".to_string())
    })?;

    insert_observation_action(
        pool,
        observation_id,
        Some(actor_user_id),
        action_kind,
        Some(current.status),
        Some(to_status),
        comment,
    )
    .await?;

    get_observation(pool, observation_id).await
}

pub(super) async fn insert_observation_action(
    pool: &PgPool,
    observation_id: Uuid,
    actor_user_id: Option<Uuid>,
    action_kind: &str,
    from_status: Option<SupervisionObservationStatus>,
    to_status: Option<SupervisionObservationStatus>,
    comment: Option<String>,
) -> Result<(), AppError> {
    sqlx::query(
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
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to insert supervision action: {}", error);
        AppError::InternalServerError("ไม่สามารถบันทึกประวัตินิเทศได้".to_string())
    })?;

    Ok(())
}
