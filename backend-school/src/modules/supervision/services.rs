use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, FixedOffset, Utc, Weekday};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::supervision::models::{
    AcknowledgeObservationRequest, ApproveObservationRequest, CancelObservationRequest,
    CreateSupervisionCycleRequest, CreateSupervisionCycleTargetRequest,
    CreateSupervisionTemplateRequest, CreateSupervisionTemplateSectionRequest,
    CreateSupervisionTemplateStepRequest, EvaluationResponseInput, EvaluatorAssignmentInput,
    LessonSnapshot, ManualLesson, ManualLessonInput, ReplaceObservationEvaluatorsRequest,
    RequestSupervisionObservationRequest, ReturnObservationRequest, SaveEvaluationRequest,
    SupervisionAction, SupervisionCycle, SupervisionCycleProgress, SupervisionCycleStatus,
    SupervisionCycleTarget, SupervisionEvaluator, SupervisionEvaluatorStatus,
    SupervisionObservation, SupervisionObservationFilter, SupervisionObservationStatus,
    SupervisionTargetType, SupervisionTemplate, SupervisionTemplateItem,
    SupervisionTemplateItemType, SupervisionTemplateSection, SupervisionTemplateStatus,
    SupervisionTemplateStep, SupervisionTemplateStepActionKind, SupervisionTemplateStepActorKind,
    UpdateRequestedObservationRequest, UpdateSupervisionCycleRequest,
    UpdateSupervisionObservationRequest, UpdateSupervisionTemplateRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisionTargetRule {
    pub target_type: SupervisionTargetType,
    pub target_id: Option<Uuid>,
    pub required_observations: i32,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionTargetMatch {
    pub staff_user_id: Uuid,
    pub subject_group_ids: Vec<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatorRatingInput {
    pub submitted: bool,
    pub rating_scores: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluatorSubmissionState {
    pub is_required: bool,
    pub submitted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvaluatorReplacementState {
    evaluator_user_id: Uuid,
    submitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TemplateSectionBulkRow {
    id: Uuid,
    title: String,
    description: Option<String>,
    sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TemplateItemBulkRow {
    section_id: Uuid,
    label: String,
    description: Option<String>,
    item_type: SupervisionTemplateItemType,
    required: bool,
    sort_order: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvaluationItemSpec {
    item_type: SupervisionTemplateItemType,
    rating_min: i32,
    rating_max: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct EvaluationResponseBulkRow {
    template_item_id: Uuid,
    rating_score: Option<f64>,
    text_response: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupervisionObservationListAccess {
    pub school: bool,
    pub own_user_id: Option<Uuid>,
    pub assigned_user_id: Option<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
}

impl SupervisionObservationListAccess {
    pub fn school() -> Self {
        Self {
            school: true,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.school
            && self.own_user_id.is_none()
            && self.assigned_user_id.is_none()
            && self.organization_unit_ids.is_empty()
    }
}

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
struct SupervisionCycleTargetRow {
    id: Uuid,
    cycle_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
    required_observations: i32,
    priority: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateRow {
    id: Uuid,
    title: String,
    description: Option<String>,
    status: String,
    rating_min: i32,
    rating_max: i32,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateSectionRow {
    id: Uuid,
    template_id: Uuid,
    title: String,
    description: Option<String>,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateItemRow {
    id: Uuid,
    section_id: Uuid,
    label: String,
    description: Option<String>,
    item_type: String,
    required: bool,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateStepRow {
    id: Uuid,
    template_id: Uuid,
    step_order: i32,
    step_code: String,
    label: String,
    actor_kind: String,
    actor_permission: Option<String>,
    organization_position_code: Option<String>,
    action_kind: String,
    required: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

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
    status: String,
    booking_opens_at: Option<DateTime<Utc>>,
    booking_closes_at: Option<DateTime<Utc>>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

pub fn resolve_supervision_target_rule<'a>(
    rules: &'a [SupervisionTargetRule],
    staff_match: &SupervisionTargetMatch,
) -> Option<&'a SupervisionTargetRule> {
    rules
        .iter()
        .filter(|rule| target_rule_matches(rule, staff_match))
        .min_by_key(|rule| (target_specificity_rank(rule.target_type), rule.priority))
}

pub fn teacher_can_edit_requested_observation(status: SupervisionObservationStatus) -> bool {
    status == SupervisionObservationStatus::Requested
}

pub fn manager_can_edit_observation(status: SupervisionObservationStatus) -> bool {
    matches!(
        status,
        SupervisionObservationStatus::Requested
            | SupervisionObservationStatus::Planned
            | SupervisionObservationStatus::Returned
    )
}

fn normalize_evaluator_replacement(
    existing: &[EvaluatorReplacementState],
    requested: Vec<EvaluatorAssignmentInput>,
) -> Result<Vec<EvaluatorAssignmentInput>, AppError> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();

    for evaluator in existing.iter().filter(|evaluator| evaluator.submitted) {
        if seen.insert(evaluator.evaluator_user_id) {
            normalized.push(EvaluatorAssignmentInput {
                evaluator_user_id: evaluator.evaluator_user_id,
                role_label: None,
                is_required: Some(true),
            });
        }
    }

    for evaluator in requested {
        if seen.insert(evaluator.evaluator_user_id) {
            normalized.push(evaluator);
        } else if !existing
            .iter()
            .any(|state| state.submitted && state.evaluator_user_id == evaluator.evaluator_user_id)
        {
            if let Some(existing_evaluator) = normalized
                .iter_mut()
                .find(|item| item.evaluator_user_id == evaluator.evaluator_user_id)
            {
                *existing_evaluator = evaluator;
            }
        }
    }

    if normalized.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องมีผู้ประเมินอย่างน้อย 1 คน".to_string(),
        ));
    }

    if normalized
        .iter()
        .all(|evaluator| evaluator.is_required.unwrap_or(true) == false)
    {
        return Err(AppError::ValidationError(
            "ต้องมีผู้ประเมินหลักอย่างน้อย 1 คน".to_string(),
        ));
    }

    Ok(normalized)
}

pub fn average_submitted_evaluator_rating(inputs: &[EvaluatorRatingInput]) -> Option<f64> {
    let evaluator_averages = inputs
        .iter()
        .filter(|input| input.submitted)
        .filter_map(|input| {
            let ratings: Vec<f64> = input.rating_scores.iter().flatten().copied().collect();
            if ratings.is_empty() {
                None
            } else {
                Some(ratings.iter().sum::<f64>() / ratings.len() as f64)
            }
        })
        .collect::<Vec<_>>();

    if evaluator_averages.is_empty() {
        return None;
    }

    Some(evaluator_averages.iter().sum::<f64>() / evaluator_averages.len() as f64)
}

pub fn all_required_evaluators_submitted(states: &[EvaluatorSubmissionState]) -> bool {
    states
        .iter()
        .filter(|state| state.is_required)
        .all(|state| state.submitted)
}

pub fn can_transition_observation_status(
    from: SupervisionObservationStatus,
    to: SupervisionObservationStatus,
) -> bool {
    use SupervisionObservationStatus::{
        Acknowledged, Approved, Cancelled, Completed, EvaluatorsSubmitted, InProgress, Planned,
        Published, Requested, Returned, UnderReview,
    };

    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (Requested, Planned)
            | (Requested, Returned)
            | (Requested, Cancelled)
            | (Returned, Planned)
            | (Returned, Cancelled)
            | (Planned, InProgress)
            | (Planned, EvaluatorsSubmitted)
            | (Planned, Returned)
            | (Planned, Cancelled)
            | (InProgress, EvaluatorsSubmitted)
            | (InProgress, Returned)
            | (InProgress, Cancelled)
            | (EvaluatorsSubmitted, UnderReview)
            | (EvaluatorsSubmitted, Returned)
            | (EvaluatorsSubmitted, Cancelled)
            | (UnderReview, Approved)
            | (UnderReview, Returned)
            | (UnderReview, Cancelled)
            | (Approved, Published)
            | (Approved, Returned)
            | (Approved, Cancelled)
            | (Published, Acknowledged)
            | (Acknowledged, Completed)
    )
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

pub async fn list_templates(pool: &PgPool) -> Result<Vec<SupervisionTemplate>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionTemplateRow>(
        r#"
        SELECT id, title, description, status, rating_min, rating_max,
               created_by, created_at, updated_at
        FROM supervision_templates
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to list supervision templates: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงแบบประเมินนิเทศได้".to_string())
    })?;

    let mut templates = Vec::with_capacity(rows.len());
    for row in rows {
        templates.push(get_template(pool, row.id).await?);
    }
    Ok(templates)
}

pub async fn get_template(pool: &PgPool, id: Uuid) -> Result<SupervisionTemplate, AppError> {
    let template_row = sqlx::query_as::<_, SupervisionTemplateRow>(
        r#"
        SELECT id, title, description, status, rating_min, rating_max,
               created_by, created_at, updated_at
        FROM supervision_templates
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงแบบประเมินนิเทศได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบแบบประเมินนิเทศ".to_string()))?;

    let section_rows = sqlx::query_as::<_, SupervisionTemplateSectionRow>(
        r#"
        SELECT id, template_id, title, description, sort_order, created_at, updated_at
        FROM supervision_template_sections
        WHERE template_id = $1
        ORDER BY sort_order, created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template sections: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงหมวดแบบประเมินนิเทศได้".to_string())
    })?;

    let item_rows = sqlx::query_as::<_, SupervisionTemplateItemRow>(
        r#"
        SELECT i.id, i.section_id, i.label, i.description, i.item_type,
               i.required, i.sort_order, i.created_at, i.updated_at
        FROM supervision_template_items i
        JOIN supervision_template_sections s ON i.section_id = s.id
        WHERE s.template_id = $1
        ORDER BY s.sort_order, i.sort_order, i.created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template items: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงหัวข้อแบบประเมินนิเทศได้".to_string())
    })?;

    let step_rows = sqlx::query_as::<_, SupervisionTemplateStepRow>(
        r#"
        SELECT id, template_id, step_order, step_code, label, actor_kind,
               actor_permission, organization_position_code, action_kind,
               required, created_at, updated_at
        FROM supervision_template_steps
        WHERE template_id = $1
        ORDER BY step_order, created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template steps: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงขั้นตอนแบบประเมินนิเทศได้".to_string())
    })?;

    template_from_rows(template_row, section_rows, item_rows, step_rows)
}

pub async fn create_template(
    pool: &PgPool,
    input: CreateSupervisionTemplateRequest,
    created_by: Uuid,
) -> Result<SupervisionTemplate, AppError> {
    validate_template_input(
        input.rating_min,
        input.rating_max,
        &input.sections,
        &input.steps,
    )?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin create supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มสร้างแบบประเมินนิเทศได้".to_string())
    })?;

    let status = input.status.unwrap_or(SupervisionTemplateStatus::Draft);
    let template_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO supervision_templates (
            title, description, status, rating_min, rating_max, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(&input.title)
    .bind(&input.description)
    .bind(status.as_str())
    .bind(input.rating_min)
    .bind(input.rating_max)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to create supervision template: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างแบบประเมินนิเทศได้".to_string())
    })?;

    insert_template_sections(&mut tx, template_id, &input.sections).await?;
    insert_template_steps(&mut tx, template_id, &input.steps).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit create supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกแบบประเมินนิเทศได้".to_string())
    })?;

    get_template(pool, template_id).await
}

pub async fn update_template(
    pool: &PgPool,
    id: Uuid,
    input: UpdateSupervisionTemplateRequest,
) -> Result<SupervisionTemplate, AppError> {
    let current = get_template(pool, id).await?;
    let title = input.title.unwrap_or(current.title);
    let description = input.description.or(current.description);
    let status = input.status.unwrap_or(current.status);
    let rating_min = input.rating_min.unwrap_or(current.rating_min);
    let rating_max = input.rating_max.unwrap_or(current.rating_max);

    if let Some(sections) = &input.sections {
        validate_template_input(
            rating_min,
            rating_max,
            sections,
            input.steps.as_deref().unwrap_or(&[]),
        )?;
    } else if rating_min >= rating_max {
        return Err(AppError::ValidationError(
            "คะแนนต่ำสุดต้องน้อยกว่าคะแนนสูงสุด".to_string(),
        ));
    }

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin update supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขแบบประเมินนิเทศได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE supervision_templates
        SET title = $2, description = $3, status = $4,
            rating_min = $5, rating_max = $6
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&title)
    .bind(&description)
    .bind(status.as_str())
    .bind(rating_min)
    .bind(rating_max)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update supervision template: {}", error);
        AppError::InternalServerError("ไม่สามารถแก้ไขแบบประเมินนิเทศได้".to_string())
    })?;

    if let Some(sections) = input.sections {
        sqlx::query("DELETE FROM supervision_template_sections WHERE template_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::error!("Failed to clear supervision template sections: {}", error);
                AppError::InternalServerError("ไม่สามารถแก้ไขหมวดแบบประเมินนิเทศได้".to_string())
            })?;
        insert_template_sections(&mut tx, id, &sections).await?;
    }

    if let Some(steps) = input.steps {
        sqlx::query("DELETE FROM supervision_template_steps WHERE template_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::error!("Failed to clear supervision template steps: {}", error);
                AppError::InternalServerError("ไม่สามารถแก้ไขขั้นตอนแบบประเมินนิเทศได้".to_string())
            })?;
        insert_template_steps(&mut tx, id, &steps).await?;
    }

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit update supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกแบบประเมินนิเทศได้".to_string())
    })?;

    get_template(pool, id).await
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

pub async fn replace_observation_evaluators(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: ReplaceObservationEvaluatorsRequest,
) -> Result<SupervisionObservation, AppError> {
    let current = get_observation(pool, observation_id).await?;
    if !manager_can_edit_observation(current.status) {
        return Err(AppError::ValidationError(
            "แก้ไขผู้ประเมินได้เฉพาะสถานะรออนุมัติ วางแผน หรือส่งกลับ".to_string(),
        ));
    }
    if input
        .evaluators
        .iter()
        .any(|evaluator| evaluator.evaluator_user_id == current.observed_user_id)
    {
        return Err(AppError::ValidationError(
            "ครูผู้ถูกนิเทศเป็นผู้ประเมินรายการของตนเองไม่ได้".to_string(),
        ));
    }

    let existing_states = current
        .evaluators
        .iter()
        .map(|evaluator| EvaluatorReplacementState {
            evaluator_user_id: evaluator.evaluator_user_id,
            submitted: evaluator.status == SupervisionEvaluatorStatus::Submitted,
        })
        .collect::<Vec<_>>();
    let submitted_user_ids = existing_states
        .iter()
        .filter(|state| state.submitted)
        .map(|state| state.evaluator_user_id)
        .collect::<HashSet<_>>();
    let replacement = normalize_evaluator_replacement(&existing_states, input.evaluators)?;
    let insert_rows = replacement
        .into_iter()
        .filter(|evaluator| !submitted_user_ids.contains(&evaluator.evaluator_user_id))
        .collect::<Vec<_>>();

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin replace supervision evaluators transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขผู้ประเมินได้".to_string())
    })?;

    sqlx::query(
        r#"
        DELETE FROM supervision_evaluators
        WHERE observation_id = $1
          AND status <> 'submitted'
        "#,
    )
    .bind(observation_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to clear non-submitted supervision evaluators: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถแก้ไขผู้ประเมินได้".to_string())
    })?;

    insert_supervision_evaluators(&mut tx, observation_id, &insert_rows).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit replace supervision evaluators transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกผู้ประเมินได้".to_string())
    })?;

    insert_observation_action(
        pool,
        observation_id,
        Some(actor_user_id),
        "evaluators_updated",
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

async fn insert_supervision_evaluators(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    observation_id: Uuid,
    evaluators: &[crate::modules::supervision::models::EvaluatorAssignmentInput],
) -> Result<(), AppError> {
    if evaluators.is_empty() {
        return Ok(());
    }

    let evaluator_user_ids: Vec<Uuid> = evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect();
    let role_labels: Vec<Option<String>> = evaluators
        .iter()
        .map(|evaluator| evaluator.role_label.clone())
        .collect();
    let required_flags: Vec<bool> = evaluators
        .iter()
        .map(|evaluator| evaluator.is_required.unwrap_or(true))
        .collect();

    sqlx::query(
        r#"
        INSERT INTO supervision_evaluators (
            observation_id, evaluator_user_id, role_label, is_required
        )
        SELECT $1, evaluator_user_id, role_label, is_required
        FROM UNNEST($2::uuid[], $3::text[], $4::bool[])
             AS rows(evaluator_user_id, role_label, is_required)
        "#,
    )
    .bind(observation_id)
    .bind(&evaluator_user_ids)
    .bind(&role_labels)
    .bind(&required_flags)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to assign supervision evaluators: {}", error);
        AppError::InternalServerError("ไม่สามารถกำหนดผู้ประเมินได้".to_string())
    })?;

    Ok(())
}

pub async fn approve_observation_request(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: ApproveObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    if input
        .evaluators
        .iter()
        .all(|evaluator| evaluator.is_required.unwrap_or(true) == false)
    {
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

pub async fn save_my_evaluation(
    pool: &PgPool,
    evaluator_user_id: Uuid,
    observation_id: Uuid,
    input: SaveEvaluationRequest,
) -> Result<SupervisionObservation, AppError> {
    let evaluator = load_evaluator_for_user(pool, observation_id, evaluator_user_id).await?;
    if evaluator.status == "submitted" {
        return Err(AppError::ValidationError(
            "ส่งผลประเมินแล้ว ไม่สามารถแก้ไขได้".to_string(),
        ));
    }

    let responses = dedupe_evaluation_responses(input.responses);
    let template_item_ids = responses
        .iter()
        .map(|response| response.template_item_id)
        .collect::<Vec<_>>();
    let item_specs = load_evaluation_item_specs(pool, observation_id, &template_item_ids).await?;
    let response_rows = build_evaluation_response_bulk_rows(&responses, &item_specs)?;
    bulk_upsert_evaluation_responses(pool, observation_id, evaluator.id, &response_rows).await?;

    sqlx::query(
        r#"
        UPDATE supervision_evaluators
        SET status = 'draft'
        WHERE id = $1 AND status = 'assigned'
        "#,
    )
    .bind(evaluator.id)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to mark supervision evaluation draft: {}", error);
        AppError::InternalServerError("ไม่สามารถบันทึกสถานะผลประเมินได้".to_string())
    })?;

    get_observation(pool, observation_id).await
}

pub async fn submit_my_evaluation(
    pool: &PgPool,
    evaluator_user_id: Uuid,
    observation_id: Uuid,
    input: SaveEvaluationRequest,
) -> Result<SupervisionObservation, AppError> {
    if !input.responses.is_empty() {
        save_my_evaluation(pool, evaluator_user_id, observation_id, input).await?;
    }

    let evaluator = load_evaluator_for_user(pool, observation_id, evaluator_user_id).await?;
    sqlx::query(
        r#"
        UPDATE supervision_evaluators
        SET status = 'submitted', submitted_at = now()
        WHERE id = $1
        "#,
    )
    .bind(evaluator.id)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to submit supervision evaluation: {}", error);
        AppError::InternalServerError("ไม่สามารถส่งผลประเมินได้".to_string())
    })?;

    let states = load_evaluator_submission_states(pool, observation_id).await?;
    if all_required_evaluators_submitted(&states) {
        let current = get_observation(pool, observation_id).await?;
        if can_transition_observation_status(
            current.status,
            SupervisionObservationStatus::EvaluatorsSubmitted,
        ) {
            sqlx::query(
                "UPDATE supervision_observations SET status = 'evaluators_submitted' WHERE id = $1",
            )
            .bind(observation_id)
            .execute(pool)
            .await
            .map_err(|error| {
                tracing::error!("Failed to mark supervision evaluators submitted: {}", error);
                AppError::InternalServerError("ไม่สามารถอัปเดตสถานะนิเทศได้".to_string())
            })?;
        }
    }

    insert_observation_action(
        pool,
        observation_id,
        Some(evaluator_user_id),
        "evaluator_submitted",
        None,
        None,
        None,
    )
    .await?;

    get_observation(pool, observation_id).await
}

pub async fn submit_observation_for_review(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    let states = load_evaluator_submission_states(pool, observation_id).await?;
    if !all_required_evaluators_submitted(&states) {
        return Err(AppError::ValidationError(
            "ผู้ประเมินหลักยังส่งผลไม่ครบ".to_string(),
        ));
    }

    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::UnderReview,
        "submitted_for_review",
        None,
    )
    .await
}

pub async fn approve_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Approved,
        "approved",
        None,
    )
    .await
}

pub async fn return_observation(
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
        "returned",
        input.comment,
    )
    .await
}

pub async fn publish_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Published,
        "published",
        None,
    )
    .await
}

pub async fn acknowledge_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: AcknowledgeObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let observation = get_observation(pool, observation_id).await?;
    if observation.observed_user_id != actor_user_id {
        return Err(AppError::Forbidden("รับทราบผลนิเทศของผู้อื่นไม่ได้".to_string()));
    }

    let action_kind = if input
        .comment
        .as_deref()
        .is_some_and(|comment| !comment.trim().is_empty())
    {
        "acknowledged_with_comment"
    } else {
        "acknowledged"
    };

    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Acknowledged,
        action_kind,
        input.comment,
    )
    .await
}

pub async fn cycle_progress(
    pool: &PgPool,
    cycle_id: Uuid,
) -> Result<SupervisionCycleProgress, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_observations,
            COUNT(*) FILTER (WHERE status = 'requested') AS requested_count,
            COUNT(*) FILTER (WHERE status = 'planned') AS planned_count,
            COUNT(*) FILTER (WHERE status = 'under_review') AS under_review_count,
            COUNT(*) FILTER (WHERE status = 'approved') AS approved_count,
            COUNT(*) FILTER (WHERE status = 'published') AS published_count,
            COUNT(*) FILTER (WHERE status IN ('acknowledged', 'completed')) AS completed_count,
            COUNT(*) FILTER (WHERE status = 'cancelled') AS cancelled_count
        FROM supervision_observations
        WHERE cycle_id = $1
        "#,
    )
    .bind(cycle_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle progress: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงรายงานรอบนิเทศได้".to_string())
    })?;

    let average_rating = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT AVG(evaluator_average)::double precision
        FROM (
            SELECT AVG(r.rating_score)::double precision AS evaluator_average
            FROM supervision_observations o
            JOIN supervision_evaluators e ON e.observation_id = o.id
            JOIN supervision_evaluator_responses r ON r.evaluator_id = e.id
            JOIN supervision_template_items i ON i.id = r.template_item_id
            WHERE o.cycle_id = $1
              AND e.status = 'submitted'
              AND i.item_type = 'rating'
              AND r.rating_score IS NOT NULL
            GROUP BY e.id
        ) evaluator_averages
        "#,
    )
    .bind(cycle_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle rating average: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงคะแนนเฉลี่ยรอบนิเทศได้".to_string())
    })?;

    Ok(SupervisionCycleProgress {
        cycle_id,
        total_observations: row.try_get("total_observations").unwrap_or(0),
        requested_count: row.try_get("requested_count").unwrap_or(0),
        planned_count: row.try_get("planned_count").unwrap_or(0),
        under_review_count: row.try_get("under_review_count").unwrap_or(0),
        approved_count: row.try_get("approved_count").unwrap_or(0),
        published_count: row.try_get("published_count").unwrap_or(0),
        completed_count: row.try_get("completed_count").unwrap_or(0),
        cancelled_count: row.try_get("cancelled_count").unwrap_or(0),
        average_rating,
    })
}

fn target_rule_matches(rule: &SupervisionTargetRule, staff_match: &SupervisionTargetMatch) -> bool {
    match rule.target_type {
        SupervisionTargetType::School => rule.target_id.is_none(),
        SupervisionTargetType::OrganizationUnit => rule
            .target_id
            .is_some_and(|id| staff_match.organization_unit_ids.contains(&id)),
        SupervisionTargetType::SubjectGroup => rule
            .target_id
            .is_some_and(|id| staff_match.subject_group_ids.contains(&id)),
        SupervisionTargetType::Staff => rule.target_id == Some(staff_match.staff_user_id),
    }
}

fn target_specificity_rank(target_type: SupervisionTargetType) -> i32 {
    match target_type {
        SupervisionTargetType::Staff => 0,
        SupervisionTargetType::SubjectGroup => 1,
        SupervisionTargetType::OrganizationUnit => 2,
        SupervisionTargetType::School => 3,
    }
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

fn validate_template_input(
    rating_min: i32,
    rating_max: i32,
    sections: &[CreateSupervisionTemplateSectionRequest],
    steps: &[CreateSupervisionTemplateStepRequest],
) -> Result<(), AppError> {
    if rating_min >= rating_max {
        return Err(AppError::ValidationError(
            "คะแนนต่ำสุดต้องน้อยกว่าคะแนนสูงสุด".to_string(),
        ));
    }

    if sections.is_empty() {
        return Err(AppError::ValidationError(
            "แบบประเมินต้องมีอย่างน้อย 1 หมวด".to_string(),
        ));
    }

    if sections.iter().all(|section| section.items.is_empty()) {
        return Err(AppError::ValidationError(
            "แบบประเมินต้องมีอย่างน้อย 1 หัวข้อ".to_string(),
        ));
    }

    for step in steps {
        match step.actor_kind {
            SupervisionTemplateStepActorKind::Permission if step.actor_permission.is_none() => {
                return Err(AppError::ValidationError(
                    "ขั้นตอนแบบ permission ต้องระบุ actorPermission".to_string(),
                ));
            }
            SupervisionTemplateStepActorKind::OrganizationPosition
                if step.organization_position_code.is_none() =>
            {
                return Err(AppError::ValidationError(
                    "ขั้นตอนแบบ organizationPosition ต้องระบุ organizationPositionCode".to_string(),
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

async fn insert_template_sections(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    template_id: Uuid,
    sections: &[CreateSupervisionTemplateSectionRequest],
) -> Result<(), AppError> {
    let (section_rows, item_rows) = build_template_section_bulk_rows(sections);
    bulk_insert_template_sections(tx, template_id, &section_rows).await?;
    bulk_insert_template_items(tx, &item_rows).await?;

    Ok(())
}

fn build_template_section_bulk_rows(
    sections: &[CreateSupervisionTemplateSectionRequest],
) -> (Vec<TemplateSectionBulkRow>, Vec<TemplateItemBulkRow>) {
    let mut section_rows = Vec::with_capacity(sections.len());
    let mut item_rows = Vec::new();

    for section in sections {
        let section_id = Uuid::new_v4();
        section_rows.push(TemplateSectionBulkRow {
            id: section_id,
            title: section.title.clone(),
            description: section.description.clone(),
            sort_order: section.sort_order,
        });

        item_rows.extend(section.items.iter().map(|item| TemplateItemBulkRow {
            section_id,
            label: item.label.clone(),
            description: item.description.clone(),
            item_type: item.item_type,
            required: item.required,
            sort_order: item.sort_order,
        }));
    }

    (section_rows, item_rows)
}

async fn bulk_insert_template_sections(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    template_id: Uuid,
    rows: &[TemplateSectionBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::new(
        r#"
        INSERT INTO supervision_template_sections (
            id, template_id, title, description, sort_order
        )
        "#,
    );
    builder.push_values(rows, |mut row_builder, row| {
        row_builder
            .push_bind(row.id)
            .push_bind(template_id)
            .push_bind(&row.title)
            .push_bind(&row.description)
            .push_bind(row.sort_order);
    });

    builder.build().execute(&mut **tx).await.map_err(|error| {
        tracing::error!(
            "Failed to bulk insert supervision template sections: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกหมวดแบบประเมินนิเทศได้".to_string())
    })?;

    Ok(())
}

async fn bulk_insert_template_items(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    rows: &[TemplateItemBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::new(
        r#"
        INSERT INTO supervision_template_items (
            section_id, label, description, item_type, required, sort_order
        )
        "#,
    );
    builder.push_values(rows, |mut row_builder, row| {
        row_builder
            .push_bind(row.section_id)
            .push_bind(&row.label)
            .push_bind(&row.description)
            .push_bind(row.item_type.as_str())
            .push_bind(row.required)
            .push_bind(row.sort_order);
    });

    builder.build().execute(&mut **tx).await.map_err(|error| {
        tracing::error!(
            "Failed to bulk insert supervision template items: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกหัวข้อแบบประเมินนิเทศได้".to_string())
    })?;

    Ok(())
}

async fn insert_template_steps(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    template_id: Uuid,
    steps: &[CreateSupervisionTemplateStepRequest],
) -> Result<(), AppError> {
    if steps.is_empty() {
        return Ok(());
    }

    let step_orders: Vec<i32> = steps.iter().map(|step| step.step_order).collect();
    let step_codes: Vec<String> = steps.iter().map(|step| step.step_code.clone()).collect();
    let labels: Vec<String> = steps.iter().map(|step| step.label.clone()).collect();
    let actor_kinds: Vec<String> = steps
        .iter()
        .map(|step| step.actor_kind.as_str().to_string())
        .collect();
    let actor_permissions: Vec<Option<String>> = steps
        .iter()
        .map(|step| step.actor_permission.clone())
        .collect();
    let organization_position_codes: Vec<Option<String>> = steps
        .iter()
        .map(|step| step.organization_position_code.clone())
        .collect();
    let action_kinds: Vec<String> = steps
        .iter()
        .map(|step| step.action_kind.as_str().to_string())
        .collect();
    let required_flags: Vec<bool> = steps.iter().map(|step| step.required).collect();

    sqlx::query(
        r#"
        INSERT INTO supervision_template_steps (
            template_id, step_order, step_code, label, actor_kind, actor_permission,
            organization_position_code, action_kind, required
        )
        SELECT $1, step_order, step_code, label, actor_kind, actor_permission,
               organization_position_code, action_kind, required
        FROM UNNEST(
            $2::int4[], $3::text[], $4::text[], $5::text[], $6::text[],
            $7::text[], $8::text[], $9::bool[]
        ) AS rows(
            step_order, step_code, label, actor_kind, actor_permission,
            organization_position_code, action_kind, required
        )
        "#,
    )
    .bind(template_id)
    .bind(&step_orders)
    .bind(&step_codes)
    .bind(&labels)
    .bind(&actor_kinds)
    .bind(&actor_permissions)
    .bind(&organization_position_codes)
    .bind(&action_kinds)
    .bind(&required_flags)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to insert supervision template steps: {}", error);
        AppError::InternalServerError("ไม่สามารถบันทึกขั้นตอนแบบประเมินนิเทศได้".to_string())
    })?;

    Ok(())
}

fn template_from_rows(
    row: SupervisionTemplateRow,
    section_rows: Vec<SupervisionTemplateSectionRow>,
    item_rows: Vec<SupervisionTemplateItemRow>,
    step_rows: Vec<SupervisionTemplateStepRow>,
) -> Result<SupervisionTemplate, AppError> {
    let mut items_by_section: HashMap<Uuid, Vec<SupervisionTemplateItem>> = HashMap::new();
    for item_row in item_rows {
        let section_id = item_row.section_id;
        items_by_section
            .entry(section_id)
            .or_default()
            .push(template_item_from_row(item_row)?);
    }

    let mut sections = Vec::with_capacity(section_rows.len());
    for section_row in section_rows {
        let section_id = section_row.id;
        sections.push(SupervisionTemplateSection {
            id: section_row.id,
            template_id: section_row.template_id,
            title: section_row.title,
            description: section_row.description,
            sort_order: section_row.sort_order,
            created_at: section_row.created_at,
            updated_at: section_row.updated_at,
            items: items_by_section.remove(&section_id).unwrap_or_default(),
        });
    }

    let mut steps = Vec::with_capacity(step_rows.len());
    for step_row in step_rows {
        steps.push(template_step_from_row(step_row)?);
    }

    Ok(SupervisionTemplate {
        id: row.id,
        title: row.title,
        description: row.description,
        status: parse_template_status(&row.status)?,
        rating_min: row.rating_min,
        rating_max: row.rating_max,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
        sections,
        steps,
    })
}

fn template_item_from_row(
    row: SupervisionTemplateItemRow,
) -> Result<SupervisionTemplateItem, AppError> {
    Ok(SupervisionTemplateItem {
        id: row.id,
        section_id: row.section_id,
        label: row.label,
        description: row.description,
        item_type: parse_template_item_type(&row.item_type)?,
        required: row.required,
        sort_order: row.sort_order,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn template_step_from_row(
    row: SupervisionTemplateStepRow,
) -> Result<SupervisionTemplateStep, AppError> {
    Ok(SupervisionTemplateStep {
        id: row.id,
        template_id: row.template_id,
        step_order: row.step_order,
        step_code: row.step_code,
        label: row.label,
        actor_kind: parse_step_actor_kind(&row.actor_kind)?,
        actor_permission: row.actor_permission,
        organization_position_code: row.organization_position_code,
        action_kind: parse_step_action_kind(&row.action_kind)?,
        required: row.required,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
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

async fn fetch_observation_average_rating(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Option<f64>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT e.id AS evaluator_id,
               e.status,
               CASE WHEN i.item_type = 'rating'
                    THEN r.rating_score::double precision
                    ELSE NULL
               END AS rating_score
        FROM supervision_evaluators e
        LEFT JOIN supervision_evaluator_responses r ON r.evaluator_id = e.id
        LEFT JOIN supervision_template_items i ON i.id = r.template_item_id
        WHERE e.observation_id = $1
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision observation average: {}", error);
        AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
    })?;

    let mut inputs: HashMap<Uuid, EvaluatorRatingInput> = HashMap::new();
    for row in rows {
        let evaluator_id: Uuid = row.try_get("evaluator_id").map_err(|error| {
            tracing::error!("Failed to read supervision evaluator id: {}", error);
            AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
        })?;
        let status: String = row.try_get("status").map_err(|error| {
            tracing::error!("Failed to read supervision evaluator status: {}", error);
            AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
        })?;
        let rating_score: Option<f64> = row.try_get("rating_score").ok();

        let input = inputs.entry(evaluator_id).or_insert(EvaluatorRatingInput {
            submitted: status == "submitted",
            rating_scores: Vec::new(),
        });
        input.rating_scores.push(rating_score);
    }

    let ratings = inputs.into_values().collect::<Vec<_>>();
    Ok(average_submitted_evaluator_rating(&ratings))
}

async fn load_cycle_for_request(
    pool: &PgPool,
    cycle_id: Uuid,
) -> Result<CycleForRequestRow, AppError> {
    sqlx::query_as::<_, CycleForRequestRow>(
        r#"
        SELECT id, template_id, status, booking_opens_at, booking_closes_at, starts_at, ends_at
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

async fn set_observation_status(
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

async fn insert_observation_action(
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

#[derive(Debug, sqlx::FromRow)]
struct EvaluatorForUserRow {
    id: Uuid,
    status: String,
}

async fn load_evaluator_for_user(
    pool: &PgPool,
    observation_id: Uuid,
    evaluator_user_id: Uuid,
) -> Result<EvaluatorForUserRow, AppError> {
    sqlx::query_as::<_, EvaluatorForUserRow>(
        r#"
        SELECT id, status
        FROM supervision_evaluators
        WHERE observation_id = $1 AND evaluator_user_id = $2
        "#,
    )
    .bind(observation_id)
    .bind(evaluator_user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision evaluator: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ประเมินได้".to_string())
    })?
    .ok_or_else(|| AppError::Forbidden("ไม่ได้รับมอบหมายให้ประเมินรายการนี้".to_string()))
}

fn dedupe_evaluation_responses(
    responses: Vec<EvaluationResponseInput>,
) -> Vec<EvaluationResponseInput> {
    let mut ordered = Vec::<EvaluationResponseInput>::with_capacity(responses.len());
    let mut index_by_item = HashMap::<Uuid, usize>::with_capacity(responses.len());

    for response in responses {
        if let Some(index) = index_by_item.get(&response.template_item_id).copied() {
            ordered[index] = response;
        } else {
            index_by_item.insert(response.template_item_id, ordered.len());
            ordered.push(response);
        }
    }

    ordered
}

async fn load_evaluation_item_specs(
    pool: &PgPool,
    observation_id: Uuid,
    template_item_ids: &[Uuid],
) -> Result<HashMap<Uuid, EvaluationItemSpec>, AppError> {
    if template_item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT i.id, i.item_type, t.rating_min, t.rating_max
        FROM supervision_template_items i
        JOIN supervision_template_sections s ON i.section_id = s.id
        JOIN supervision_templates t ON s.template_id = t.id
        JOIN supervision_observations o ON o.template_id = t.id
        WHERE o.id = $1 AND i.id = ANY($2)
        "#,
    )
    .bind(observation_id)
    .bind(template_item_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision response item specs: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
    })?;

    let mut specs = HashMap::with_capacity(rows.len());
    for row in rows {
        let item_id: Uuid = row.try_get("id").map_err(|error| {
            tracing::error!("Failed to read supervision item id: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
        })?;
        let item_type: String = row.try_get("item_type").map_err(|error| {
            tracing::error!("Failed to read supervision item type: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
        })?;
        let rating_min: i32 = row.try_get("rating_min").map_err(|error| {
            tracing::error!("Failed to read supervision rating min: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบคะแนนประเมินได้".to_string())
        })?;
        let rating_max: i32 = row.try_get("rating_max").map_err(|error| {
            tracing::error!("Failed to read supervision rating max: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบคะแนนประเมินได้".to_string())
        })?;

        specs.insert(
            item_id,
            EvaluationItemSpec {
                item_type: parse_template_item_type(&item_type)?,
                rating_min,
                rating_max,
            },
        );
    }

    Ok(specs)
}

fn build_evaluation_response_bulk_rows(
    responses: &[EvaluationResponseInput],
    item_specs: &HashMap<Uuid, EvaluationItemSpec>,
) -> Result<Vec<EvaluationResponseBulkRow>, AppError> {
    let mut rows = Vec::with_capacity(responses.len());

    for response in responses {
        let spec = item_specs
            .get(&response.template_item_id)
            .ok_or_else(|| AppError::ValidationError("หัวข้อประเมินไม่อยู่ในแบบประเมินนี้".to_string()))?;

        match spec.item_type {
            SupervisionTemplateItemType::Rating => {
                if response
                    .text_response
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    return Err(AppError::ValidationError(
                        "หัวข้อแบบคะแนนไม่รับคำตอบข้อความ".to_string(),
                    ));
                }
                if let Some(score) = response.rating_score {
                    if score < spec.rating_min as f64 || score > spec.rating_max as f64 {
                        return Err(AppError::ValidationError(
                            "คะแนนอยู่นอกช่วงที่แบบประเมินกำหนด".to_string(),
                        ));
                    }
                }
            }
            SupervisionTemplateItemType::Text => {
                if response.rating_score.is_some() {
                    return Err(AppError::ValidationError(
                        "หัวข้อแบบข้อความไม่รับคะแนน".to_string(),
                    ));
                }
            }
        }

        rows.push(EvaluationResponseBulkRow {
            template_item_id: response.template_item_id,
            rating_score: response.rating_score,
            text_response: response.text_response.clone(),
        });
    }

    Ok(rows)
}

async fn bulk_upsert_evaluation_responses(
    pool: &PgPool,
    observation_id: Uuid,
    evaluator_id: Uuid,
    rows: &[EvaluationResponseBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::new(
        r#"
        INSERT INTO supervision_evaluator_responses (
            observation_id, evaluator_id, template_item_id, rating_score, text_response
        )
        "#,
    );
    builder.push_values(rows, |mut row_builder, row| {
        row_builder
            .push_bind(observation_id)
            .push_bind(evaluator_id)
            .push_bind(row.template_item_id)
            .push_bind(row.rating_score)
            .push_unseparated("::numeric")
            .push_bind(&row.text_response);
    });
    builder.push(
        r#"
        ON CONFLICT (evaluator_id, template_item_id)
        DO UPDATE SET
            rating_score = EXCLUDED.rating_score,
            text_response = EXCLUDED.text_response,
            updated_at = now()
        "#,
    );

    builder.build().execute(pool).await.map_err(|error| {
        tracing::error!(
            "Failed to bulk upsert supervision evaluation responses: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกผลประเมินได้".to_string())
    })?;

    Ok(())
}

async fn load_evaluator_submission_states(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<EvaluatorSubmissionState>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT is_required, status
        FROM supervision_evaluators
        WHERE observation_id = $1
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision evaluator states: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะผู้ประเมินได้".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| EvaluatorSubmissionState {
            is_required: row.try_get("is_required").unwrap_or(false),
            submitted: row
                .try_get::<String, _>("status")
                .map(|status| status == "submitted")
                .unwrap_or(false),
        })
        .collect())
}

fn parse_cycle_status(code: &str) -> Result<SupervisionCycleStatus, AppError> {
    SupervisionCycleStatus::from_code(code)
        .ok_or_else(|| AppError::InternalServerError("สถานะรอบนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string()))
}

fn parse_template_status(code: &str) -> Result<SupervisionTemplateStatus, AppError> {
    SupervisionTemplateStatus::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("สถานะแบบประเมินนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

fn parse_target_type(code: &str) -> Result<SupervisionTargetType, AppError> {
    SupervisionTargetType::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("ประเภทเป้าหมายนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

fn parse_template_item_type(code: &str) -> Result<SupervisionTemplateItemType, AppError> {
    SupervisionTemplateItemType::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("ประเภทหัวข้อแบบประเมินในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

fn parse_step_actor_kind(code: &str) -> Result<SupervisionTemplateStepActorKind, AppError> {
    SupervisionTemplateStepActorKind::from_code(code)
        .ok_or_else(|| AppError::InternalServerError("ประเภทผู้ดำเนินการขั้นตอนนิเทศไม่ถูกต้อง".to_string()))
}

fn parse_step_action_kind(code: &str) -> Result<SupervisionTemplateStepActionKind, AppError> {
    SupervisionTemplateStepActionKind::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("ประเภทการดำเนินการขั้นตอนนิเทศไม่ถูกต้อง".to_string())
    })
}

fn parse_observation_status(code: &str) -> Result<SupervisionObservationStatus, AppError> {
    SupervisionObservationStatus::from_code(code)
        .ok_or_else(|| AppError::InternalServerError("สถานะรายการนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string()))
}

fn parse_optional_observation_status(
    code: Option<String>,
) -> Result<Option<SupervisionObservationStatus>, AppError> {
    code.map(|value| parse_observation_status(&value))
        .transpose()
}

fn parse_evaluator_status(code: &str) -> Result<SupervisionEvaluatorStatus, AppError> {
    SupervisionEvaluatorStatus::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("สถานะผู้ประเมินนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::supervision::models::CreateSupervisionTemplateItemRequest;

    fn target_rule(
        target_type: SupervisionTargetType,
        target_id: Option<Uuid>,
        priority: i32,
    ) -> SupervisionTargetRule {
        SupervisionTargetRule {
            target_type,
            target_id,
            required_observations: 1,
            priority,
        }
    }

    #[test]
    fn target_specificity_prefers_staff_then_subject_then_organization_then_school() {
        let staff_user_id = Uuid::new_v4();
        let subject_group_id = Uuid::new_v4();
        let organization_unit_id = Uuid::new_v4();
        let lower_priority_school_rule = target_rule(SupervisionTargetType::School, None, 0);
        let rules = vec![
            lower_priority_school_rule,
            target_rule(
                SupervisionTargetType::OrganizationUnit,
                Some(organization_unit_id),
                0,
            ),
            target_rule(
                SupervisionTargetType::SubjectGroup,
                Some(subject_group_id),
                0,
            ),
            target_rule(SupervisionTargetType::Staff, Some(staff_user_id), 100),
        ];
        let staff_match = SupervisionTargetMatch {
            staff_user_id,
            subject_group_ids: vec![subject_group_id],
            organization_unit_ids: vec![organization_unit_id],
        };

        let resolved =
            resolve_supervision_target_rule(&rules, &staff_match).expect("matching target rule");

        assert_eq!(resolved.target_type, SupervisionTargetType::Staff);
        assert_eq!(resolved.target_id, Some(staff_user_id));
        assert_ne!(*resolved, lower_priority_school_rule);
    }

    #[test]
    fn target_priority_breaks_ties_within_same_specificity() {
        let organization_unit_id = Uuid::new_v4();
        let rules = vec![
            target_rule(
                SupervisionTargetType::OrganizationUnit,
                Some(organization_unit_id),
                50,
            ),
            target_rule(
                SupervisionTargetType::OrganizationUnit,
                Some(organization_unit_id),
                10,
            ),
        ];
        let staff_match = SupervisionTargetMatch {
            staff_user_id: Uuid::new_v4(),
            subject_group_ids: Vec::new(),
            organization_unit_ids: vec![organization_unit_id],
        };

        let resolved = resolve_supervision_target_rule(&rules, &staff_match)
            .expect("matching organization target rule");

        assert_eq!(resolved.priority, 10);
    }

    #[test]
    fn teachers_may_edit_requests_only_while_requested() {
        assert!(teacher_can_edit_requested_observation(
            SupervisionObservationStatus::Requested
        ));
        assert!(!teacher_can_edit_requested_observation(
            SupervisionObservationStatus::Planned
        ));
        assert!(!teacher_can_edit_requested_observation(
            SupervisionObservationStatus::Returned
        ));
        assert!(!teacher_can_edit_requested_observation(
            SupervisionObservationStatus::Cancelled
        ));
    }

    #[test]
    fn manager_can_edit_only_manageable_observation_statuses() {
        assert!(manager_can_edit_observation(
            SupervisionObservationStatus::Requested
        ));
        assert!(manager_can_edit_observation(
            SupervisionObservationStatus::Planned
        ));
        assert!(manager_can_edit_observation(
            SupervisionObservationStatus::Returned
        ));
        assert!(!manager_can_edit_observation(
            SupervisionObservationStatus::UnderReview
        ));
        assert!(!manager_can_edit_observation(
            SupervisionObservationStatus::Approved
        ));
        assert!(!manager_can_edit_observation(
            SupervisionObservationStatus::Published
        ));
        assert!(!manager_can_edit_observation(
            SupervisionObservationStatus::Completed
        ));
        assert!(!manager_can_edit_observation(
            SupervisionObservationStatus::Cancelled
        ));
    }

    #[test]
    fn evaluator_replacement_keeps_submitted_evaluators() {
        let submitted_user_id = Uuid::new_v4();
        let requested_user_id = Uuid::new_v4();
        let retained = normalize_evaluator_replacement(
            &[EvaluatorReplacementState {
                evaluator_user_id: submitted_user_id,
                submitted: true,
            }],
            vec![EvaluatorAssignmentInput {
                evaluator_user_id: requested_user_id,
                role_label: None,
                is_required: Some(true),
            }],
        )
        .expect("replacement");

        assert!(retained
            .iter()
            .any(|evaluator| evaluator.evaluator_user_id == submitted_user_id));
        assert!(retained
            .iter()
            .any(|evaluator| evaluator.evaluator_user_id == requested_user_id));
    }

    #[test]
    fn average_rating_uses_equal_submitted_evaluator_weights() {
        let evaluator_a = EvaluatorRatingInput {
            submitted: true,
            rating_scores: vec![Some(5.0), Some(5.0)],
        };
        let evaluator_b = EvaluatorRatingInput {
            submitted: true,
            rating_scores: vec![Some(1.0), None],
        };
        let evaluator_c = EvaluatorRatingInput {
            submitted: false,
            rating_scores: vec![Some(5.0)],
        };

        let average = average_submitted_evaluator_rating(&[evaluator_a, evaluator_b, evaluator_c])
            .expect("submitted rating average");

        assert!((average - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn all_required_evaluators_must_submit_before_review() {
        let states = vec![
            EvaluatorSubmissionState {
                is_required: true,
                submitted: true,
            },
            EvaluatorSubmissionState {
                is_required: true,
                submitted: false,
            },
            EvaluatorSubmissionState {
                is_required: false,
                submitted: false,
            },
        ];

        assert!(!all_required_evaluators_submitted(&states));

        let submitted_states = vec![
            EvaluatorSubmissionState {
                is_required: true,
                submitted: true,
            },
            EvaluatorSubmissionState {
                is_required: true,
                submitted: true,
            },
            EvaluatorSubmissionState {
                is_required: false,
                submitted: false,
            },
        ];

        assert!(all_required_evaluators_submitted(&submitted_states));
    }

    #[test]
    fn template_bulk_rows_preserve_section_item_relationships() {
        let (section_rows, item_rows) =
            build_template_section_bulk_rows(&[CreateSupervisionTemplateSectionRequest {
                title: "ด้านการจัดกิจกรรม".to_string(),
                description: Some("ตรวจแผนและกิจกรรมการเรียนรู้".to_string()),
                sort_order: 1,
                items: vec![
                    CreateSupervisionTemplateItemRequest {
                        label: "จัดกิจกรรมตามแผน".to_string(),
                        description: None,
                        item_type: SupervisionTemplateItemType::Rating,
                        required: true,
                        sort_order: 1,
                    },
                    CreateSupervisionTemplateItemRequest {
                        label: "ข้อเสนอแนะ".to_string(),
                        description: Some("บันทึกเพิ่มเติม".to_string()),
                        item_type: SupervisionTemplateItemType::Text,
                        required: false,
                        sort_order: 2,
                    },
                ],
            }]);

        assert_eq!(section_rows.len(), 1);
        assert_eq!(item_rows.len(), 2);
        assert_ne!(section_rows[0].id, Uuid::nil());
        assert!(item_rows
            .iter()
            .all(|item| item.section_id == section_rows[0].id));
        assert_eq!(item_rows[0].item_type, SupervisionTemplateItemType::Rating);
        assert_eq!(item_rows[1].item_type, SupervisionTemplateItemType::Text);
    }

    #[test]
    fn duplicate_evaluation_responses_keep_the_latest_answer() {
        let item_id = Uuid::new_v4();
        let responses = dedupe_evaluation_responses(vec![
            EvaluationResponseInput {
                template_item_id: item_id,
                rating_score: Some(2.0),
                text_response: None,
            },
            EvaluationResponseInput {
                template_item_id: item_id,
                rating_score: Some(5.0),
                text_response: None,
            },
        ]);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].template_item_id, item_id);
        assert_eq!(responses[0].rating_score, Some(5.0));
    }

    #[test]
    fn evaluation_bulk_rows_validate_item_type_and_rating_range() {
        let rating_item_id = Uuid::new_v4();
        let text_item_id = Uuid::new_v4();
        let specs = HashMap::from([
            (
                rating_item_id,
                EvaluationItemSpec {
                    item_type: SupervisionTemplateItemType::Rating,
                    rating_min: 1,
                    rating_max: 5,
                },
            ),
            (
                text_item_id,
                EvaluationItemSpec {
                    item_type: SupervisionTemplateItemType::Text,
                    rating_min: 1,
                    rating_max: 5,
                },
            ),
        ]);

        let rows = build_evaluation_response_bulk_rows(
            &[
                EvaluationResponseInput {
                    template_item_id: rating_item_id,
                    rating_score: Some(4.0),
                    text_response: None,
                },
                EvaluationResponseInput {
                    template_item_id: text_item_id,
                    rating_score: None,
                    text_response: Some("จัดกิจกรรมได้ต่อเนื่อง".to_string()),
                },
            ],
            &specs,
        )
        .expect("valid bulk rows");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].rating_score, Some(4.0));
        assert_eq!(rows[1].text_response.as_deref(), Some("จัดกิจกรรมได้ต่อเนื่อง"));

        let invalid = build_evaluation_response_bulk_rows(
            &[EvaluationResponseInput {
                template_item_id: rating_item_id,
                rating_score: Some(6.0),
                text_response: None,
            }],
            &specs,
        );

        assert!(
            matches!(invalid, Err(AppError::ValidationError(message)) if message == "คะแนนอยู่นอกช่วงที่แบบประเมินกำหนด")
        );
    }

    #[test]
    fn invalid_status_transitions_are_rejected() {
        assert!(can_transition_observation_status(
            SupervisionObservationStatus::Requested,
            SupervisionObservationStatus::Planned
        ));
        assert!(can_transition_observation_status(
            SupervisionObservationStatus::Published,
            SupervisionObservationStatus::Acknowledged
        ));
        assert!(!can_transition_observation_status(
            SupervisionObservationStatus::Requested,
            SupervisionObservationStatus::Approved
        ));
        assert!(!can_transition_observation_status(
            SupervisionObservationStatus::Completed,
            SupervisionObservationStatus::Returned
        ));
    }
}
