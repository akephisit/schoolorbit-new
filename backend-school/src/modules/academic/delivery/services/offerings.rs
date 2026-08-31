use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::models::RequirementKind;
use crate::modules::academic::core::services::validate_canonical_decimal;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    ActivityAttendanceRequirement, ActivityOfferingSnapshot, ActivityPassCriteria,
    ApplyCurriculumOfferingsRequest, ApplyCurriculumOfferingsResult, CourseGradingPolicy,
    CourseOfferingSnapshot, CreateActivityOfferingRequest, CreateCourseOfferingRequest,
    CreateLearningOfferingRequest, CurriculumGroupProposal, CurriculumOfferingPreview,
    CurriculumPreparationChoice, CurriculumPreparationProposal, CurriculumPreviewAction,
    LearningOffering, LearningOfferingKind, LearningOfferingQuery, LearningOfferingRow,
    LearningOfferingSnapshot, LearningOfferingStatus, LearningOfferingTarget, OfferingTargetInput,
    OfferingTargetKind, PreparationAction, PreparationConflict, PreparationGroupingState,
    PreviewCurriculumOfferingsRequest, PublishLearningOfferingRequest,
    UpdateLearningOfferingRequest,
};
use super::{
    append_audit, require_active_owner, require_writable_term, stable_hash, validate_row_version,
    TermContext,
};

const OFFERING_COLUMNS: &str = r#"
    id, academic_term_id, academic_year_id, kind, code_snapshot, name_snapshot,
    source_requirement_kind, source_requirement_id, status, published_at,
    starts_on, ends_on, stop_reason,
    owning_organization_unit_id, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

const DELIVERY_NAMESPACE: Uuid = Uuid::from_u128(0x83c8_46da_e34e_5146_8ff1_f7ca_aa6e_20a4);
const MAX_TERM_OFFERINGS: usize = 500;

#[derive(Debug, sqlx::FromRow)]
struct CourseVersionSource {
    subject_version_id: Uuid,
    subject_id: Uuid,
    code: String,
    name: String,
    credit: String,
    hours: Option<String>,
    standard_periods_per_week: Option<i32>,
    effective_from: chrono::NaiveDate,
    effective_until: Option<chrono::NaiveDate>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ActivityVersionSource {
    activity_version_id: Uuid,
    activity_id: Uuid,
    code: String,
    name: String,
    hours: String,
    scheduling_mode: super::super::models::ActivitySchedulingMode,
    effective_from: chrono::NaiveDate,
    effective_until: Option<chrono::NaiveDate>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct CourseDetailRow {
    learning_offering_id: Uuid,
    subject_version_id: Uuid,
    subject_id: Uuid,
    curriculum_course_requirement_id: Option<Uuid>,
    credit: String,
    hours: Option<String>,
    standard_periods_per_week: i32,
    grading_policy: sqlx::types::Json<CourseGradingPolicy>,
}

#[derive(Debug, sqlx::FromRow)]
struct ActivityDetailRow {
    learning_offering_id: Uuid,
    activity_version_id: Uuid,
    activity_id: Uuid,
    curriculum_activity_requirement_id: Option<Uuid>,
    registration_type: super::super::models::ActivityRegistrationType,
    scheduling_mode: super::super::models::ActivitySchedulingMode,
    hours: String,
    capacity: Option<i32>,
    attendance_requirement: sqlx::types::Json<ActivityAttendanceRequirement>,
    pass_criteria: sqlx::types::Json<ActivityPassCriteria>,
}

#[derive(Debug, sqlx::FromRow)]
struct PreviewRequirementRow {
    resource_kind: LearningOfferingKind,
    catalog_version_id: Uuid,
    requirement_id: Uuid,
    study_program_id: Uuid,
    grade_level_id: Uuid,
    requirement_kind: RequirementKind,
    code: String,
    name: String,
    credit: Option<String>,
    hours: Option<String>,
    version_status: String,
    effective_from: chrono::NaiveDate,
    effective_until: Option<chrono::NaiveDate>,
}

#[derive(Debug, sqlx::FromRow)]
struct ExistingPreviewOfferingRow {
    resource_kind: LearningOfferingKind,
    catalog_version_id: Uuid,
    offering_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct PreviewHomeroomRow {
    id: Uuid,
    name: String,
    grade_level_id: Uuid,
    study_program_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ExistingPreparationGroupRow {
    id: Uuid,
    learning_offering_id: Uuid,
    name: String,
    generation_source: String,
    generation_key: Option<String>,
    homeroom_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LearningOfferingSignalDescriptor {
    pub learning_offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub row_version: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewHashInput<'a> {
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    term_row_version: i64,
    term_code: &'a str,
    study_program_ids: &'a [Uuid],
    proposals: &'a [CurriculumPreparationProposal],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyRequestHashInput<'a> {
    academic_term_id: Uuid,
    study_program_ids: &'a [Uuid],
    owning_organization_unit_id: Uuid,
    source_hash: &'a str,
    choices: &'a [CurriculumPreparationChoice],
}

pub async fn list(
    pool: &PgPool,
    query: LearningOfferingQuery,
    filter: &AcademicResourceListFilter,
) -> Result<Vec<LearningOffering>, AppError> {
    let owner_ids = filter.allowed_organization_unit_ids();
    let sql = format!(
        "SELECT {OFFERING_COLUMNS} FROM learning_offerings \
         WHERE academic_term_id = $1 \
           AND status <> 'cancelled' \
           AND ($2 OR owning_organization_unit_id = ANY($3) OR EXISTS (\
               SELECT 1 FROM learning_groups learning_group \
               JOIN learning_group_teachers teacher \
                 ON teacher.learning_group_id = learning_group.id \
               WHERE learning_group.learning_offering_id = learning_offerings.id \
                 AND teacher.teacher_id = $4\
           )) \
         ORDER BY kind, code_snapshot, id LIMIT $5"
    );
    let rows: Vec<LearningOfferingRow> = sqlx::query_as(&sql)
        .bind(query.academic_term_id)
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .bind(filter.assigned_actor_id)
        .bind((MAX_TERM_OFFERINGS + 1) as i64)
        .fetch_all(pool)
        .await?;
    if rows.len() > MAX_TERM_OFFERINGS {
        return Err(AppError::ValidationError(
            "จำนวนรายการเปิดสอนในภาคเรียนเกิน 500 รายการ กรุณาแบ่งข้อมูลก่อนเปิดพื้นที่ทำงาน".to_string(),
        ));
    }
    hydrate_many(pool, rows).await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<LearningOffering, AppError> {
    let sql = format!("SELECT {OFFERING_COLUMNS} FROM learning_offerings WHERE id = $1");
    let row: LearningOfferingRow = sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาหรือกิจกรรมที่เปิดสอน".to_string()))?;
    hydrate(pool, row).await
}

pub async fn create(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateLearningOfferingRequest,
) -> Result<LearningOffering, AppError> {
    let term_id = request.academic_term_id();
    let mut transaction = pool.begin().await?;
    let term = require_writable_term(&mut transaction, term_id, false).await?;
    require_active_owner(&mut transaction, request.owning_organization_unit_id()).await?;
    validate_targets(&mut transaction, &term, request.targets()).await?;
    let id = Uuid::new_v4();
    match request {
        CreateLearningOfferingRequest::Course(request) => {
            insert_course(&mut transaction, id, &term, request).await?;
        }
        CreateLearningOfferingRequest::Activity(request) => {
            insert_activity(&mut transaction, id, &term, request).await?;
        }
    }
    transaction.commit().await?;
    let offering = get(pool, id).await?;
    append_audit(
        pool,
        "learning_offering.created",
        "learning_offering",
        id,
        offering.academic_year_id,
        offering.academic_term_id,
        actor_user_id,
        serde_json::json!({ "kind": offering.kind }),
    )
    .await?;
    Ok(offering)
}

pub async fn update(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: UpdateLearningOfferingRequest,
) -> Result<LearningOffering, AppError> {
    validate_row_version(request.row_version)?;
    let mut transaction = pool.begin().await?;
    let row: (
        Uuid,
        Uuid,
        LearningOfferingKind,
        LearningOfferingStatus,
        i64,
    ) = sqlx::query_as(
        "SELECT academic_term_id, academic_year_id, kind, status, row_version \
         FROM learning_offerings WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาหรือกิจกรรมที่เปิดสอน".to_string()))?;
    if row.3 != LearningOfferingStatus::Draft {
        return Err(AppError::Conflict(
            "ข้อมูล snapshot ที่เผยแพร่แล้วแก้ไขไม่ได้".to_string(),
        ));
    }
    if row.4 != request.row_version {
        return Err(AppError::Conflict(
            "รายการเปิดสอนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let term = require_writable_term(&mut transaction, row.0, false).await?;
    require_active_owner(&mut transaction, request.owning_organization_unit_id).await?;
    validate_targets(&mut transaction, &term, &request.targets).await?;
    sqlx::query("DELETE FROM learning_offering_targets WHERE learning_offering_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    insert_targets(&mut transaction, id, &term, &request.targets).await?;
    sqlx::query(
        "UPDATE learning_offerings SET owning_organization_unit_id = $1, \
         row_version = row_version + 1, updated_at = now() WHERE id = $2",
    )
    .bind(request.owning_organization_unit_id)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "learning_offering.updated",
        "learning_offering",
        id,
        row.1,
        row.0,
        actor_user_id,
        serde_json::json!({ "rowVersion": request.row_version }),
    )
    .await?;
    get(pool, id).await
}

pub async fn publish(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    request: PublishLearningOfferingRequest,
) -> Result<LearningOffering, AppError> {
    validate_row_version(request.row_version)?;
    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM learning_offerings WHERE publish_idempotency_key = $1",
    )
    .bind(request.idempotency_key)
    .fetch_optional(pool)
    .await?
    {
        if existing_id == id {
            return get(pool, id).await;
        }
        return Err(AppError::Conflict(
            "idempotencyKey ถูกใช้กับรายการอื่นแล้ว".to_string(),
        ));
    }

    let mut transaction = pool.begin().await?;
    let row: (
        Uuid,
        Uuid,
        LearningOfferingKind,
        LearningOfferingStatus,
        i64,
    ) = sqlx::query_as(
        "SELECT academic_term_id, academic_year_id, kind, status, row_version \
         FROM learning_offerings WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาหรือกิจกรรมที่เปิดสอน".to_string()))?;
    if row.3 != LearningOfferingStatus::Draft {
        return Err(AppError::Conflict("รายการนี้ไม่ได้อยู่ในสถานะ draft".to_string()));
    }
    if row.4 != request.row_version {
        return Err(AppError::Conflict(
            "รายการเปิดสอนถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string(),
        ));
    }
    let term = require_writable_term(&mut transaction, row.0, false).await?;
    validate_publishable(&mut transaction, id, row.2, &term).await?;
    sqlx::query(
        "UPDATE learning_groups SET status = 'published', row_version = row_version + 1, \
         updated_at = now() WHERE learning_offering_id = $1 AND status = 'draft'",
    )
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE learning_offerings SET status = 'published', published_at = now(), \
         publish_idempotency_key = $1, row_version = row_version + 1, updated_at = now() \
         WHERE id = $2",
    )
    .bind(request.idempotency_key)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    append_audit(
        pool,
        "learning_offering.published",
        "learning_offering",
        id,
        row.1,
        row.0,
        actor_user_id,
        serde_json::json!({ "kind": row.2 }),
    )
    .await?;
    get(pool, id).await
}

pub async fn preview_from_curriculum(
    pool: &PgPool,
    request: PreviewCurriculumOfferingsRequest,
) -> Result<CurriculumOfferingPreview, AppError> {
    let program_ids = normalized_program_ids(&request.study_program_ids)?;
    let mut transaction = pool.begin().await?;
    let preview =
        build_curriculum_preview(&mut transaction, request.academic_term_id, &program_ids).await?;
    transaction.commit().await?;
    Ok(preview)
}

pub async fn apply_from_curriculum(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: ApplyCurriculumOfferingsRequest,
) -> Result<ApplyCurriculumOfferingsResult, AppError> {
    let program_ids = normalized_program_ids(&request.study_program_ids)?;
    let choices = normalized_preparation_choices(&request.choices)?;
    let request_hash = stable_hash(&ApplyRequestHashInput {
        academic_term_id: request.academic_term_id,
        study_program_ids: &program_ids,
        owning_organization_unit_id: request.owning_organization_unit_id,
        source_hash: &request.source_hash,
        choices: &choices,
    })?;

    if let Some((
        stored_request_hash,
        source_hash,
        offering_ids,
        group_ids,
        created_offering_count,
        retained_offering_count,
        created_group_count,
        retained_group_count,
        skipped_count,
    )) = sqlx::query_as::<
        _,
        (
            String,
            String,
            Vec<Uuid>,
            Vec<Uuid>,
            i32,
            i32,
            i32,
            i32,
            i32,
        ),
    >(
        "SELECT request_hash::text, source_hash::text, offering_ids, group_ids, \
                created_offering_count, retained_offering_count, \
                created_group_count, retained_group_count, skipped_count \
         FROM learning_delivery_apply_runs WHERE idempotency_key = $1",
    )
    .bind(request.idempotency_key)
    .fetch_optional(pool)
    .await?
    {
        if stored_request_hash.trim_end() != request_hash {
            return Err(AppError::Conflict(
                "idempotencyKey ถูกใช้กับคำขออื่นแล้ว".to_string(),
            ));
        }
        return Ok(ApplyCurriculumOfferingsResult {
            academic_term_id: request.academic_term_id,
            source_hash: source_hash.trim_end().to_string(),
            offering_ids,
            group_ids,
            created_offering_count: created_offering_count as usize,
            retained_offering_count: retained_offering_count as usize,
            created_group_count: created_group_count as usize,
            retained_group_count: retained_group_count as usize,
            skipped_count: skipped_count as usize,
        });
    }

    let mut transaction = pool.begin().await?;
    require_active_owner(&mut transaction, request.owning_organization_unit_id).await?;
    let preview =
        build_curriculum_preview(&mut transaction, request.academic_term_id, &program_ids).await?;
    if preview.source_hash != request.source_hash {
        return Err(AppError::Conflict(
            "โครงสร้างหลักสูตรหรือภาคเรียนเปลี่ยนไป กรุณา preview ใหม่".to_string(),
        ));
    }
    validate_preparation_choices(&preview.proposals, &choices)?;

    let term = require_writable_term(&mut transaction, request.academic_term_id, true).await?;
    let mut offering_ids = Vec::new();
    let mut group_ids = Vec::new();
    let mut created_offering_count = 0_usize;
    let mut retained_offering_count = 0_usize;
    let mut created_group_count = 0_usize;
    let mut retained_group_count = 0_usize;
    let mut skipped_count = 0_usize;
    for proposal in &preview.proposals {
        let choice = choices
            .iter()
            .find(|choice| choice.proposal_id == proposal.proposal_id)
            .ok_or_else(|| AppError::ValidationError("ตัวเลือกการเตรียมรายการไม่ครบ".to_string()))?;
        if choice.action == PreparationAction::Skip {
            skipped_count += 1;
            continue;
        }
        if proposal.offering_action == CurriculumPreviewAction::Conflict {
            return Err(AppError::Conflict(format!(
                "รายการ {} ขัดแย้งกับข้อมูลเปิดสอนเดิม",
                proposal.code
            )));
        }
        if choice.action == PreparationAction::Apply && !proposal.conflicts.is_empty() {
            return Err(AppError::Conflict(proposal.conflicts[0].message.clone()));
        }

        let offering_id = if let Some(existing) = proposal.existing_offering_id {
            retained_offering_count += 1;
            existing
        } else {
            created_offering_count += 1;
            insert_generated_offering(
                &mut transaction,
                &term,
                proposal,
                request.owning_organization_unit_id,
            )
            .await?
        };
        insert_homeroom_targets(&mut transaction, offering_id, &term, proposal).await?;
        offering_ids.push(offering_id);

        if choice.action == PreparationAction::Apply {
            for group in &choice.groups {
                let outcome = super::groups::apply_curriculum_generated_group(
                    &mut transaction,
                    offering_id,
                    term.id,
                    term.academic_year_id,
                    group,
                )
                .await?;
                group_ids.push(outcome.id);
                if outcome.created {
                    created_group_count += 1;
                } else {
                    retained_group_count += 1;
                }
            }
        }
    }
    offering_ids.sort_unstable();
    offering_ids.dedup();
    group_ids.sort_unstable();
    group_ids.dedup();
    sqlx::query(
        "INSERT INTO learning_delivery_apply_runs (
             idempotency_key, academic_term_id, request_hash, source_hash,
             offering_ids, group_ids, created_offering_count,
             retained_offering_count, created_group_count, retained_group_count,
             skipped_count, actor_user_id
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(request.idempotency_key)
    .bind(request.academic_term_id)
    .bind(&request_hash)
    .bind(&preview.source_hash)
    .bind(&offering_ids)
    .bind(&group_ids)
    .bind(created_offering_count as i32)
    .bind(retained_offering_count as i32)
    .bind(created_group_count as i32)
    .bind(retained_group_count as i32)
    .bind(skipped_count as i32)
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(ApplyCurriculumOfferingsResult {
        academic_term_id: request.academic_term_id,
        source_hash: preview.source_hash,
        offering_ids,
        group_ids,
        created_offering_count,
        retained_offering_count,
        created_group_count,
        retained_group_count,
        skipped_count,
    })
}

pub async fn signal_descriptors(
    pool: &PgPool,
    offering_ids: &[Uuid],
) -> Result<Vec<LearningOfferingSignalDescriptor>, AppError> {
    if offering_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<LearningOfferingSignalDescriptor> = sqlx::query_as(
        r#"SELECT id AS learning_offering_id, academic_term_id, row_version
           FROM learning_offerings
           WHERE id = ANY($1)
           ORDER BY id"#,
    )
    .bind(offering_ids)
    .fetch_all(pool)
    .await?;
    let mut rows_by_id: HashMap<Uuid, LearningOfferingSignalDescriptor> = rows
        .into_iter()
        .map(|row| (row.learning_offering_id, row))
        .collect();
    offering_ids
        .iter()
        .map(|offering_id| {
            rows_by_id.remove(offering_id).ok_or_else(|| {
                AppError::InternalServerError("ไม่พบรายการเปิดสอนหลังนำโครงสร้างหลักสูตรมาใช้".to_string())
            })
        })
        .collect()
}

pub async fn operational_change_offering_ids(
    pool: &PgPool,
    change_set_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let change_set_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM academic_term_change_sets WHERE id = $1)")
            .bind(change_set_id)
            .fetch_one(pool)
            .await?;
    if !change_set_exists {
        return Err(AppError::NotFound("ไม่พบชุดการเปลี่ยนแปลงภาคเรียน".to_string()));
    }
    Ok(sqlx::query_scalar(
        r#"SELECT item.learning_offering_id
           FROM academic_term_change_items item
           WHERE item.change_set_id = $1
             AND item.learning_offering_id IS NOT NULL
           UNION
           SELECT learning_group.learning_offering_id
           FROM academic_term_change_items item
           JOIN learning_groups learning_group ON learning_group.id = item.learning_group_id
           WHERE item.change_set_id = $1
           ORDER BY 1"#,
    )
    .bind(change_set_id)
    .fetch_all(pool)
    .await?)
}

async fn hydrate(pool: &PgPool, row: LearningOfferingRow) -> Result<LearningOffering, AppError> {
    hydrate_many(pool, vec![row])
        .await?
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถโหลดรายการเปิดสอนได้".to_string()))
}

async fn hydrate_many(
    pool: &PgPool,
    rows: Vec<LearningOfferingRow>,
) -> Result<Vec<LearningOffering>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let offering_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let target_rows: Vec<TargetRow> = sqlx::query_as(
        "SELECT learning_offering_id, id, target_kind, homeroom_id, grade_level_id, \
         study_program_id FROM learning_offering_targets \
         WHERE learning_offering_id = ANY($1) \
         ORDER BY learning_offering_id, target_kind, homeroom_id, grade_level_id, study_program_id",
    )
    .bind(&offering_ids)
    .fetch_all(pool)
    .await?;
    let mut targets_by_offering: HashMap<Uuid, Vec<LearningOfferingTarget>> = HashMap::new();
    for target in target_rows {
        targets_by_offering
            .entry(target.learning_offering_id)
            .or_default()
            .push(target.into());
    }

    let course_rows: Vec<CourseDetailRow> = sqlx::query_as(
        "SELECT detail.learning_offering_id, detail.subject_version_id, detail.subject_id, \
         detail.curriculum_course_requirement_id, detail.credit::text AS credit, \
         detail.hours::text AS hours, \
         version.periods_per_week AS standard_periods_per_week, \
         detail.grading_policy \
         FROM course_offering_details detail \
         JOIN subject_versions version ON version.id = detail.subject_version_id \
         WHERE detail.learning_offering_id = ANY($1)",
    )
    .bind(&offering_ids)
    .fetch_all(pool)
    .await?;
    let mut courses_by_offering: HashMap<Uuid, CourseDetailRow> = course_rows
        .into_iter()
        .map(|detail| (detail.learning_offering_id, detail))
        .collect();

    let activity_rows: Vec<ActivityDetailRow> = sqlx::query_as(
        "SELECT learning_offering_id, activity_version_id, activity_id, \
         curriculum_activity_requirement_id, registration_type, scheduling_mode, \
         hours::text AS hours, capacity, attendance_requirement, pass_criteria \
         FROM activity_offering_details WHERE learning_offering_id = ANY($1)",
    )
    .bind(&offering_ids)
    .fetch_all(pool)
    .await?;
    let mut activities_by_offering: HashMap<Uuid, ActivityDetailRow> = activity_rows
        .into_iter()
        .map(|detail| (detail.learning_offering_id, detail))
        .collect();

    rows.into_iter()
        .map(|row| {
            let snapshot = match row.kind {
                LearningOfferingKind::Course => {
                    let detail = courses_by_offering.remove(&row.id).ok_or_else(|| {
                        AppError::InternalServerError("ไม่พบ snapshot ของรายวิชาที่เปิดสอน".to_string())
                    })?;
                    LearningOfferingSnapshot::Course(CourseOfferingSnapshot {
                        subject_version_id: detail.subject_version_id,
                        subject_id: detail.subject_id,
                        curriculum_course_requirement_id: detail.curriculum_course_requirement_id,
                        credit: detail.credit,
                        hours: detail.hours,
                        standard_periods_per_week: detail.standard_periods_per_week,
                        grading_policy: detail.grading_policy.0,
                    })
                }
                LearningOfferingKind::Activity => {
                    let detail = activities_by_offering.remove(&row.id).ok_or_else(|| {
                        AppError::InternalServerError("ไม่พบ snapshot ของกิจกรรมที่เปิดสอน".to_string())
                    })?;
                    LearningOfferingSnapshot::Activity(ActivityOfferingSnapshot {
                        activity_version_id: detail.activity_version_id,
                        activity_id: detail.activity_id,
                        curriculum_activity_requirement_id: detail
                            .curriculum_activity_requirement_id,
                        registration_type: detail.registration_type,
                        scheduling_mode: detail.scheduling_mode,
                        hours: detail.hours,
                        capacity: detail.capacity,
                        attendance_requirement: detail.attendance_requirement.0,
                        pass_criteria: detail.pass_criteria.0,
                    })
                }
            };
            Ok(LearningOffering {
                id: row.id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                kind: row.kind,
                code_snapshot: row.code_snapshot,
                name_snapshot: row.name_snapshot,
                source_requirement_kind: row.source_requirement_kind,
                source_requirement_id: row.source_requirement_id,
                status: row.status,
                published_at: row.published_at,
                starts_on: row.starts_on,
                ends_on: row.ends_on,
                stop_reason: row.stop_reason,
                owning_organization_unit_id: row.owning_organization_unit_id,
                row_version: row.row_version,
                migrated: row.migrated,
                created_at: row.created_at,
                updated_at: row.updated_at,
                snapshot,
                targets: targets_by_offering.remove(&row.id).unwrap_or_default(),
            })
        })
        .collect()
}

#[derive(Debug, sqlx::FromRow)]
struct TargetRow {
    learning_offering_id: Uuid,
    id: Uuid,
    target_kind: OfferingTargetKind,
    homeroom_id: Option<Uuid>,
    grade_level_id: Uuid,
    study_program_id: Uuid,
}

impl From<TargetRow> for LearningOfferingTarget {
    fn from(row: TargetRow) -> Self {
        Self {
            id: row.id,
            target_kind: row.target_kind,
            homeroom_id: row.homeroom_id,
            grade_level_id: row.grade_level_id,
            study_program_id: row.study_program_id,
        }
    }
}

pub(super) async fn insert_course(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    term: &TermContext,
    request: CreateCourseOfferingRequest,
) -> Result<(), AppError> {
    if request.grading_policy.policy_code.trim().is_empty() {
        return Err(AppError::ValidationError(
            "ต้องระบุนโยบายโครงสร้างคะแนน".to_string(),
        ));
    }
    let total_score = validate_canonical_decimal(&request.grading_policy.total_score, 2)?;
    if total_score <= bigdecimal::BigDecimal::from(0) {
        return Err(AppError::ValidationError(
            "คะแนนรวมตามนโยบายต้องมากกว่า 0".to_string(),
        ));
    }
    if let Some(score) = &request.grading_policy.passing_score {
        let passing_score = validate_canonical_decimal(score, 2)?;
        if passing_score < bigdecimal::BigDecimal::from(0) || passing_score > total_score {
            return Err(AppError::ValidationError(
                "คะแนนผ่านต้องอยู่ระหว่าง 0 ถึงคะแนนรวม".to_string(),
            ));
        }
    }
    let source = course_version_source(transaction, request.subject_version_id).await?;
    validate_version_for_term(
        &source.status,
        source.effective_from,
        source.effective_until,
        term,
    )?;
    required_standard_periods_per_week(source.standard_periods_per_week)?;
    let (credit, hours) = if let Some(requirement_id) = request.curriculum_course_requirement_id {
        let requirement: (Uuid, Uuid, Uuid, String, i32, String, Option<String>) = sqlx::query_as(
            "SELECT requirement.subject_version_id, requirement.grade_level_id, \
                 requirement.study_program_id, slot.term_type, slot.type_occurrence, \
                 version.credit::text, version.hours_per_semester::text \
                 FROM curriculum_course_requirements requirement \
                 JOIN curriculum_term_slots slot ON slot.id = requirement.term_slot_id \
                 JOIN subject_versions version ON version.id = requirement.subject_version_id \
                 WHERE requirement.id = $1",
        )
        .bind(requirement_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::ValidationError("ไม่พบข้อกำหนดรายวิชา".to_string()))?;
        validate_requirement_target(
            source.subject_version_id,
            requirement.0,
            requirement.1,
            requirement.2,
            &requirement.3,
            requirement.4,
            term,
            &request.targets,
        )?;
        (requirement.5, requirement.6)
    } else {
        (source.credit.clone(), source.hours.clone())
    };
    sqlx::query(
        r#"INSERT INTO learning_offerings (
               id, academic_term_id, academic_year_id, kind, code_snapshot,
               name_snapshot, source_requirement_kind, source_requirement_id,
               status, owning_organization_unit_id
           ) VALUES ($1, $2, $3, 'course', $4, $5, $6, $7, 'draft', $8)"#,
    )
    .bind(id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(source.code)
    .bind(source.name)
    .bind(
        request
            .curriculum_course_requirement_id
            .map(|_| "curriculum_course_requirement"),
    )
    .bind(request.curriculum_course_requirement_id)
    .bind(request.owning_organization_unit_id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO course_offering_details (
               learning_offering_id, academic_term_id, academic_year_id,
               subject_version_id, subject_id, curriculum_course_requirement_id,
               credit, hours, grading_policy
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(source.subject_version_id)
    .bind(source.subject_id)
    .bind(request.curriculum_course_requirement_id)
    .bind(validate_canonical_decimal(&credit, 2)?)
    .bind(
        hours
            .as_deref()
            .map(|value| validate_canonical_decimal(value, 2))
            .transpose()?,
    )
    .bind(sqlx::types::Json(request.grading_policy))
    .execute(&mut **transaction)
    .await?;
    insert_targets(transaction, id, term, &request.targets).await
}

pub(super) async fn insert_activity(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    term: &TermContext,
    request: CreateActivityOfferingRequest,
) -> Result<(), AppError> {
    if request.capacity.is_some_and(|capacity| capacity <= 0) {
        return Err(AppError::ValidationError("ความจุต้องมากกว่าศูนย์".to_string()));
    }
    validate_activity_rules(&request.attendance_requirement, &request.pass_criteria)?;
    let source = activity_version_source(transaction, request.activity_version_id).await?;
    validate_version_for_term(
        &source.status,
        source.effective_from,
        source.effective_until,
        term,
    )?;
    if source.scheduling_mode != request.scheduling_mode {
        return Err(AppError::ValidationError(
            "รูปแบบตารางกิจกรรมต้องตรงกับเวอร์ชันกิจกรรม".to_string(),
        ));
    }
    let hours = if let Some(requirement_id) = request.curriculum_activity_requirement_id {
        let requirement: (Uuid, Uuid, Uuid, String, i32, String) = sqlx::query_as(
            "SELECT requirement.activity_version_id, requirement.grade_level_id, \
             requirement.study_program_id, slot.term_type, slot.type_occurrence, \
             version.hours_per_week::text \
             FROM curriculum_activity_requirements requirement \
             JOIN curriculum_term_slots slot ON slot.id = requirement.term_slot_id \
             JOIN activity_versions version ON version.id = requirement.activity_version_id \
             WHERE requirement.id = $1",
        )
        .bind(requirement_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::ValidationError("ไม่พบข้อกำหนดกิจกรรม".to_string()))?;
        validate_requirement_target(
            source.activity_version_id,
            requirement.0,
            requirement.1,
            requirement.2,
            &requirement.3,
            requirement.4,
            term,
            &request.targets,
        )?;
        requirement.5
    } else {
        source.hours.clone()
    };
    sqlx::query(
        r#"INSERT INTO learning_offerings (
               id, academic_term_id, academic_year_id, kind, code_snapshot,
               name_snapshot, source_requirement_kind, source_requirement_id,
               status, owning_organization_unit_id
           ) VALUES ($1, $2, $3, 'activity', $4, $5, $6, $7, 'draft', $8)"#,
    )
    .bind(id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(source.code)
    .bind(source.name)
    .bind(
        request
            .curriculum_activity_requirement_id
            .map(|_| "curriculum_activity_requirement"),
    )
    .bind(request.curriculum_activity_requirement_id)
    .bind(request.owning_organization_unit_id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO activity_offering_details (
               learning_offering_id, academic_term_id, academic_year_id,
               activity_version_id, activity_id, curriculum_activity_requirement_id,
               registration_type, scheduling_mode, hours, capacity,
               attendance_requirement, pass_criteria
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(source.activity_version_id)
    .bind(source.activity_id)
    .bind(request.curriculum_activity_requirement_id)
    .bind(request.registration_type)
    .bind(request.scheduling_mode)
    .bind(validate_canonical_decimal(&hours, 2)?)
    .bind(request.capacity)
    .bind(sqlx::types::Json(request.attendance_requirement))
    .bind(sqlx::types::Json(request.pass_criteria))
    .execute(&mut **transaction)
    .await?;
    insert_targets(transaction, id, term, &request.targets).await
}

async fn course_version_source(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<CourseVersionSource, AppError> {
    sqlx::query_as(
        r#"SELECT version.id AS subject_version_id, version.subject_id,
                  version.code, version.name_th AS name, version.credit::text AS credit,
                  version.hours_per_semester::text AS hours,
                  version.periods_per_week AS standard_periods_per_week, version.effective_from,
                  version.effective_until, version.status
           FROM subject_versions version WHERE version.id = $1"#,
    )
    .bind(version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("ไม่พบเวอร์ชันรายวิชา".to_string()))
}

fn required_standard_periods_per_week(value: Option<i32>) -> Result<i32, AppError> {
    value.filter(|periods| *periods > 0).ok_or_else(|| {
        AppError::ValidationError("เวอร์ชันรายวิชาต้องมีคาบมาตรฐานต่อสัปดาห์มากกว่า 0 ก่อนเปิดสอน".to_string())
    })
}

async fn activity_version_source(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<ActivityVersionSource, AppError> {
    sqlx::query_as(
        r#"SELECT version.id AS activity_version_id, version.activity_id,
                  stable.code, version.name, version.hours_per_week::text AS hours,
                  version.scheduling_mode, version.effective_from,
                  version.effective_until, version.status
           FROM activity_versions version
           JOIN activities stable ON stable.id = version.activity_id
           WHERE version.id = $1"#,
    )
    .bind(version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("ไม่พบเวอร์ชันกิจกรรม".to_string()))
}

fn validate_version_for_term(
    status: &str,
    effective_from: chrono::NaiveDate,
    effective_until: Option<chrono::NaiveDate>,
    term: &TermContext,
) -> Result<(), AppError> {
    if status != "published" {
        return Err(AppError::ValidationError(
            "เลือกได้เฉพาะเวอร์ชันที่เผยแพร่แล้ว".to_string(),
        ));
    }
    if term.start_date < effective_from
        || effective_until.is_some_and(|until| term.start_date >= until)
    {
        return Err(AppError::ValidationError(
            "เวอร์ชันนี้ไม่มีผลในภาคเรียนที่เลือก".to_string(),
        ));
    }
    Ok(())
}

fn validate_requirement_target(
    selected_version_id: Uuid,
    requirement_version_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    requirement_term_type: &str,
    requirement_type_occurrence: i32,
    term: &TermContext,
    targets: &[OfferingTargetInput],
) -> Result<(), AppError> {
    if selected_version_id != requirement_version_id {
        return Err(AppError::ValidationError(
            "ข้อกำหนดอ้างอิงคนละเวอร์ชัน".to_string(),
        ));
    }
    if requirement_term_type != term.term_type
        || requirement_type_occurrence != term.type_occurrence
    {
        return Err(AppError::ValidationError(
            "ข้อกำหนดไม่อยู่ในภาคเรียนที่เลือก".to_string(),
        ));
    }
    if !targets.iter().any(|target| {
        target.grade_level_id == grade_level_id && target.study_program_id == study_program_id
    }) {
        return Err(AppError::ValidationError(
            "เป้าหมายไม่ครอบคลุมระดับชั้นและแผนการเรียนของข้อกำหนด".to_string(),
        ));
    }
    Ok(())
}

pub(super) async fn validate_targets(
    transaction: &mut Transaction<'_, Postgres>,
    term: &TermContext,
    targets: &[OfferingTargetInput],
) -> Result<(), AppError> {
    if targets.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องระบุเป้าหมายการเปิดสอนอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    let mut unique = HashSet::new();
    for target in targets {
        let key = (
            target.target_kind,
            target.homeroom_id,
            target.grade_level_id,
            target.study_program_id,
        );
        if !unique.insert(key) {
            return Err(AppError::ValidationError(
                "เป้าหมายการเปิดสอนซ้ำกัน".to_string(),
            ));
        }
        match target.target_kind {
            OfferingTargetKind::Homeroom => {
                let homeroom_id = target.homeroom_id.ok_or_else(|| {
                    AppError::ValidationError("เป้าหมาย homeroom ต้องระบุห้องเรียน".to_string())
                })?;
                let valid: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM homerooms WHERE id = $1 \
                     AND academic_year_id = $2 AND grade_level_id = $3 \
                     AND study_program_id = $4)",
                )
                .bind(homeroom_id)
                .bind(term.academic_year_id)
                .bind(target.grade_level_id)
                .bind(target.study_program_id)
                .fetch_one(&mut **transaction)
                .await?;
                if !valid {
                    return Err(AppError::ValidationError(
                        "ห้องเรียนเป้าหมายไม่ตรงปี ระดับชั้น หรือแผนการเรียน".to_string(),
                    ));
                }
            }
            OfferingTargetKind::GradeProgram => {
                if target.homeroom_id.is_some() {
                    return Err(AppError::ValidationError(
                        "เป้าหมาย grade_program ต้องไม่ระบุ homeroomId".to_string(),
                    ));
                }
                let valid: bool = sqlx::query_scalar(
                    "SELECT EXISTS (
                         SELECT 1 FROM study_programs program
                         JOIN curriculum_versions version ON version.id = program.curriculum_version_id
                         JOIN curricula curriculum ON curriculum.id = version.curriculum_id
                         WHERE program.id = $1 AND $2 = ANY(
                             SELECT jsonb_array_elements_text(curriculum.grade_level_ids)::uuid
                         )
                     )",
                )
                .bind(target.study_program_id)
                .bind(target.grade_level_id)
                .fetch_one(&mut **transaction)
                .await?;
                if !valid {
                    return Err(AppError::ValidationError(
                        "ระดับชั้นหรือแผนการเรียนของเป้าหมายไม่ถูกต้อง".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

async fn insert_targets(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    term: &TermContext,
    targets: &[OfferingTargetInput],
) -> Result<(), AppError> {
    for target in targets {
        sqlx::query(
            r#"INSERT INTO learning_offering_targets (
                   id, learning_offering_id, academic_term_id, academic_year_id,
                   target_kind, homeroom_id, grade_level_id, study_program_id
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(Uuid::new_v4())
        .bind(offering_id)
        .bind(term.id)
        .bind(term.academic_year_id)
        .bind(target.target_kind)
        .bind(target.homeroom_id)
        .bind(target.grade_level_id)
        .bind(target.study_program_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

fn validate_activity_rules(
    attendance: &ActivityAttendanceRequirement,
    pass: &ActivityPassCriteria,
) -> Result<(), AppError> {
    if let Some(percent) = &attendance.minimum_percent {
        let value = validate_canonical_decimal(percent, 2)?;
        if value < bigdecimal::BigDecimal::from(0) || value > bigdecimal::BigDecimal::from(100) {
            return Err(AppError::ValidationError(
                "เกณฑ์เวลาเข้าร่วมต้องอยู่ระหว่าง 0 ถึง 100".to_string(),
            ));
        }
    }
    if attendance
        .required_sessions
        .is_some_and(|sessions| sessions < 0)
    {
        return Err(AppError::ValidationError(
            "จำนวนครั้งที่ต้องเข้าร่วมต้องไม่ติดลบ".to_string(),
        ));
    }
    if pass.outcomes != ["pass".to_string(), "fail".to_string()] {
        return Err(AppError::ValidationError(
            "ผลกิจกรรม Release 1 ต้องเป็น pass และ fail เท่านั้น".to_string(),
        ));
    }
    Ok(())
}

async fn validate_publishable(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    kind: LearningOfferingKind,
    term: &TermContext,
) -> Result<(), AppError> {
    let target_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_offering_targets WHERE learning_offering_id = $1",
    )
    .bind(offering_id)
    .fetch_one(&mut **transaction)
    .await?;
    let group_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_groups WHERE learning_offering_id = $1")
            .bind(offering_id)
            .fetch_one(&mut **transaction)
            .await?;
    if target_count == 0 || group_count == 0 {
        return Err(AppError::ValidationError(
            "ต้องมีเป้าหมายและกลุ่มเรียนอย่างน้อยหนึ่งรายการก่อนเผยแพร่".to_string(),
        ));
    }
    let groups_without_primary_teacher: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM learning_groups learning_group
           WHERE learning_group.learning_offering_id = $1
             AND NOT EXISTS (
                 SELECT 1
                 FROM learning_group_teachers assignment
                 JOIN users teacher ON teacher.id = assignment.teacher_id
                 WHERE assignment.learning_group_id = learning_group.id
                   AND assignment.role = 'primary'
                   AND teacher.user_type = 'staff'
                   AND teacher.status = 'active'
             )"#,
    )
    .bind(offering_id)
    .fetch_one(&mut **transaction)
    .await?;
    if groups_without_primary_teacher != 0 {
        return Err(AppError::ValidationError(
            "ทุกกลุ่มเรียนต้องมีครูหลักที่ใช้งานอยู่ก่อนเผยแพร่".to_string(),
        ));
    }
    match kind {
        LearningOfferingKind::Course => {
            let (status, from, until): (String, chrono::NaiveDate, Option<chrono::NaiveDate>) =
                sqlx::query_as(
                    "SELECT version.status, version.effective_from, version.effective_until \
                     FROM course_offering_details detail \
                     JOIN subject_versions version ON version.id = detail.subject_version_id \
                     WHERE detail.learning_offering_id = $1",
                )
                .bind(offering_id)
                .fetch_one(&mut **transaction)
                .await?;
            validate_version_for_term(&status, from, until, term)?;
        }
        LearningOfferingKind::Activity => {
            let (status, from, until): (String, chrono::NaiveDate, Option<chrono::NaiveDate>) =
                sqlx::query_as(
                    "SELECT version.status, version.effective_from, version.effective_until \
                     FROM activity_offering_details detail \
                     JOIN activity_versions version ON version.id = detail.activity_version_id \
                     WHERE detail.learning_offering_id = $1",
                )
                .bind(offering_id)
                .fetch_one(&mut **transaction)
                .await?;
            validate_version_for_term(&status, from, until, term)?;
        }
    }
    Ok(())
}

fn normalized_program_ids(ids: &[Uuid]) -> Result<Vec<Uuid>, AppError> {
    if ids.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องเลือกแผนการเรียนอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    let mut values = ids.to_vec();
    values.sort_unstable();
    values.dedup();
    if values.len() != ids.len() {
        return Err(AppError::ValidationError("แผนการเรียนซ้ำกัน".to_string()));
    }
    Ok(values)
}

fn normalized_preparation_choices(
    choices: &[CurriculumPreparationChoice],
) -> Result<Vec<CurriculumPreparationChoice>, AppError> {
    let mut values = choices.to_vec();
    for choice in &mut values {
        choice.proposal_id = choice.proposal_id.trim().to_string();
        if choice.proposal_id.is_empty() {
            return Err(AppError::ValidationError("proposalId ต้องไม่ว่าง".to_string()));
        }
        if choice.action != PreparationAction::Apply && !choice.groups.is_empty() {
            return Err(AppError::ValidationError(
                "ตัวเลือกข้ามหรือรอจัดกลุ่มต้องไม่ส่งกลุ่มเรียนมาด้วย".to_string(),
            ));
        }
        let mut group_keys = HashSet::new();
        for group in &mut choice.groups {
            group.group_key = group.group_key.trim().to_string();
            group.name = group.name.trim().to_string();
            group.homeroom_ids.sort_unstable();
            group.homeroom_ids.dedup();
            if group.group_key.len() != 64
                || !group
                    .group_key
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
                || group.name.is_empty()
                || group.homeroom_ids.is_empty()
            {
                return Err(AppError::ValidationError(
                    "กลุ่มที่เตรียมต้องมี groupKey แบบ source hash ชื่อ และห้องประจำชั้น".to_string(),
                ));
            }
            if !group_keys.insert(group.group_key.clone()) {
                return Err(AppError::ValidationError(
                    "groupKey ซ้ำกันในข้อเสนอเดียวกัน".to_string(),
                ));
            }
        }
        choice
            .groups
            .sort_by(|left, right| left.group_key.cmp(&right.group_key));
    }
    values.sort_by(|left, right| left.proposal_id.cmp(&right.proposal_id));
    if values
        .windows(2)
        .any(|pair| pair[0].proposal_id == pair[1].proposal_id)
    {
        return Err(AppError::ValidationError(
            "proposalId ซ้ำกันในคำขอ".to_string(),
        ));
    }
    Ok(values)
}

fn validate_preparation_choices(
    proposals: &[CurriculumPreparationProposal],
    choices: &[CurriculumPreparationChoice],
) -> Result<(), AppError> {
    if proposals.len() != choices.len() {
        return Err(AppError::ValidationError(
            "ต้องส่งตัวเลือกให้ครบทุกข้อเสนอจาก preview ล่าสุด".to_string(),
        ));
    }
    for proposal in proposals {
        let choice = choices
            .iter()
            .find(|choice| choice.proposal_id == proposal.proposal_id)
            .ok_or_else(|| {
                AppError::ValidationError(format!("ไม่พบตัวเลือกสำหรับรายการ {}", proposal.code))
            })?;
        if choice.action == PreparationAction::Apply && choice.groups.is_empty() {
            return Err(AppError::ValidationError(format!(
                "รายการ {} ต้องมีกลุ่ม หรือเลือกไว้จัดกลุ่มภายหลัง",
                proposal.code
            )));
        }
        for group in &choice.groups {
            if group
                .homeroom_ids
                .iter()
                .any(|id| !proposal.target_homeroom_ids.contains(id))
            {
                return Err(AppError::ValidationError(format!(
                    "กลุ่มของรายการ {} มีห้องที่อยู่นอกเป้าหมายจากหลักสูตร",
                    proposal.code
                )));
            }
        }
    }
    Ok(())
}

fn generated_group_key(proposal_id: &str, homeroom_ids: &[Uuid]) -> Result<String, AppError> {
    let mut normalized_ids = homeroom_ids.to_vec();
    normalized_ids.sort_unstable();
    normalized_ids.dedup();
    stable_hash(&(proposal_id, normalized_ids))
}

fn offering_kind_order(kind: LearningOfferingKind) -> u8 {
    match kind {
        LearningOfferingKind::Course => 1,
        LearningOfferingKind::Activity => 2,
    }
}

async fn build_curriculum_preview(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    program_ids: &[Uuid],
) -> Result<CurriculumOfferingPreview, AppError> {
    let term = require_writable_term(transaction, academic_term_id, false).await?;
    let valid_program_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT program.id FROM study_programs program \
         JOIN curriculum_versions version ON version.id = program.curriculum_version_id \
         WHERE program.id = ANY($1) AND program.status = 'published' \
           AND version.status = 'published' \
           AND (SELECT start_date FROM academic_years WHERE id = version.start_academic_year_id) \
               <= (SELECT start_date FROM academic_years WHERE id = $2) \
           AND (version.end_academic_year_id IS NULL OR \
                (SELECT end_date FROM academic_years WHERE id = version.end_academic_year_id) \
                    >= (SELECT start_date FROM academic_years WHERE id = $2)) \
         FOR SHARE",
    )
    .bind(program_ids)
    .bind(term.academic_year_id)
    .fetch_all(&mut **transaction)
    .await?;
    if valid_program_ids.len() != program_ids.len() {
        return Err(AppError::ValidationError(
            "แผนการเรียนต้องเผยแพร่และอยู่ในหลักสูตรที่เผยแพร่แล้ว".to_string(),
        ));
    }
    let rows: Vec<PreviewRequirementRow> = sqlx::query_as(
        r#"SELECT 'course'::text AS resource_kind,
                  requirement.subject_version_id AS catalog_version_id,
                  requirement.id AS requirement_id, requirement.study_program_id,
                  requirement.grade_level_id, requirement.requirement_kind,
                  version.code, version.name_th AS name,
                  version.credit::text AS credit, version.hours_per_semester::text AS hours,
                  version.status AS version_status, version.effective_from,
                  version.effective_until
           FROM curriculum_course_requirements requirement
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           JOIN curriculum_term_slots slot ON slot.id = requirement.term_slot_id
           WHERE requirement.study_program_id = ANY($1)
             AND slot.term_type = $2
             AND slot.type_occurrence = $3
           UNION ALL
           SELECT 'activity'::text AS resource_kind,
                  requirement.activity_version_id AS catalog_version_id,
                  requirement.id AS requirement_id, requirement.study_program_id,
                  requirement.grade_level_id, requirement.requirement_kind,
                  stable.code, version.name,
                  NULL::text AS credit, version.hours_per_week::text AS hours,
                  version.status AS version_status, version.effective_from,
                  version.effective_until
           FROM curriculum_activity_requirements requirement
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           JOIN activities stable ON stable.id = version.activity_id
           JOIN curriculum_term_slots slot ON slot.id = requirement.term_slot_id
           WHERE requirement.study_program_id = ANY($1)
             AND slot.term_type = $2
             AND slot.type_occurrence = $3
           ORDER BY resource_kind, catalog_version_id, study_program_id, grade_level_id, requirement_id"#,
    )
    .bind(program_ids)
    .bind(&term.term_type)
    .bind(term.type_occurrence)
    .fetch_all(&mut **transaction)
    .await?;
    let homerooms: Vec<PreviewHomeroomRow> = sqlx::query_as(
        r#"SELECT id, name, grade_level_id, study_program_id
           FROM homerooms
           WHERE academic_year_id = $1
             AND study_program_id = ANY($2)
             AND is_active
           ORDER BY grade_level_id, study_program_id, name, id"#,
    )
    .bind(term.academic_year_id)
    .bind(program_ids)
    .fetch_all(&mut **transaction)
    .await?;
    let mut homerooms_by_program_grade: HashMap<(Uuid, Uuid), Vec<&PreviewHomeroomRow>> =
        HashMap::new();
    for homeroom in &homerooms {
        homerooms_by_program_grade
            .entry((homeroom.study_program_id, homeroom.grade_level_id))
            .or_default()
            .push(homeroom);
    }
    let existing_offerings =
        load_existing_preview_offerings(transaction, academic_term_id, &rows).await?;
    let existing_offering_ids = existing_offerings.values().copied().collect::<Vec<_>>();
    let existing_groups =
        load_existing_preparation_groups(transaction, &existing_offering_ids).await?;
    let mut proposals = Vec::<CurriculumPreparationProposal>::new();
    let mut proposal_indexes = HashMap::<(LearningOfferingKind, Uuid), usize>::new();
    for row in rows {
        validate_version_for_term(
            &row.version_status,
            row.effective_from,
            row.effective_until,
            &term,
        )?;
        let key = (row.resource_kind, row.catalog_version_id);
        let proposal_index = if let Some(index) = proposal_indexes.get(&key) {
            *index
        } else {
            let existing_offering_id = existing_offerings.get(&key).copied();
            let proposal_id =
                stable_hash(&(academic_term_id, row.resource_kind, row.catalog_version_id))?;
            let index = proposals.len();
            proposals.push(CurriculumPreparationProposal {
                proposal_id,
                offering_action: if existing_offering_id.is_some() {
                    CurriculumPreviewAction::Retain
                } else {
                    CurriculumPreviewAction::Create
                },
                resource_kind: row.resource_kind,
                catalog_version_id: row.catalog_version_id,
                requirement_ids: Vec::new(),
                target_homeroom_ids: Vec::new(),
                code: row.code.clone(),
                name: row.name.clone(),
                credit: row.credit.clone(),
                hours: row.hours.clone(),
                existing_offering_id,
                grouping_state: PreparationGroupingState::Deferred,
                default_groups: Vec::new(),
                conflicts: Vec::new(),
            });
            proposal_indexes.insert(key, index);
            index
        };
        let proposal = &mut proposals[proposal_index];
        proposal.requirement_ids.push(row.requirement_id);
        let applicable_homerooms = homerooms_by_program_grade
            .get(&(row.study_program_id, row.grade_level_id))
            .cloned()
            .unwrap_or_default();
        for homeroom in applicable_homerooms {
            proposal.target_homeroom_ids.push(homeroom.id);
            if row.requirement_kind == RequirementKind::Required {
                let group_key = generated_group_key(&proposal.proposal_id, &[homeroom.id])?;
                if !proposal
                    .default_groups
                    .iter()
                    .any(|group| group.group_key == group_key)
                {
                    proposal.default_groups.push(CurriculumGroupProposal {
                        group_key,
                        name: format!("{} · {}", proposal.code, homeroom.name),
                        homeroom_ids: vec![homeroom.id],
                    });
                }
            }
        }
    }

    proposals.retain(|proposal| !proposal.target_homeroom_ids.is_empty());
    if proposals.is_empty() {
        return Err(AppError::ValidationError(
            "ไม่พบห้องประจำชั้นที่ใช้แผนการเรียนนี้ในปีการศึกษาของภาคเรียนที่เลือก".to_string(),
        ));
    }
    for proposal in &mut proposals {
        proposal.requirement_ids.sort_unstable();
        proposal.requirement_ids.dedup();
        proposal.target_homeroom_ids.sort_unstable();
        proposal.target_homeroom_ids.dedup();
        proposal.default_groups.sort_by(|left, right| {
            left.homeroom_ids
                .cmp(&right.homeroom_ids)
                .then(left.group_key.cmp(&right.group_key))
        });
        if let Some(offering_id) = proposal.existing_offering_id {
            let generated_groups = existing_groups
                .iter()
                .filter(|group| {
                    group.learning_offering_id == offering_id
                        && group.generation_source == "curriculum_prepare"
                        && group
                            .homeroom_ids
                            .iter()
                            .any(|id| proposal.target_homeroom_ids.contains(id))
                })
                .collect::<Vec<_>>();
            if !generated_groups.is_empty() {
                proposal.default_groups = generated_groups
                    .iter()
                    .filter_map(|group| {
                        group
                            .generation_key
                            .as_ref()
                            .map(|group_key| CurriculumGroupProposal {
                                group_key: group_key.clone(),
                                name: group.name.clone(),
                                homeroom_ids: group.homeroom_ids.clone(),
                            })
                    })
                    .collect();
            }
            for group in existing_groups
                .iter()
                .filter(|group| group.learning_offering_id == offering_id)
            {
                let overlaps_target = group
                    .homeroom_ids
                    .iter()
                    .any(|id| proposal.target_homeroom_ids.contains(id));
                if overlaps_target && group.generation_source == "manual" {
                    proposal.conflicts.push(PreparationConflict {
                        code: "manual_group_overlap".to_string(),
                        message: format!(
                            "รายการ {} มีกลุ่มที่จัดเองครอบคลุมห้องเป้าหมายอยู่แล้ว ระบบจะไม่แก้กลุ่มนี้",
                            proposal.code
                        ),
                        offering_id: Some(offering_id),
                        group_id: Some(group.id),
                    });
                } else if group.generation_source == "curriculum_prepare"
                    && group
                        .homeroom_ids
                        .iter()
                        .any(|id| !proposal.target_homeroom_ids.contains(id))
                {
                    proposal.conflicts.push(PreparationConflict {
                        code: "generated_group_target_mismatch".to_string(),
                        message: format!(
                            "กลุ่มที่ระบบเคยเตรียมสำหรับ {} ครอบคลุมห้องนอกเป้าหมายหลักสูตร กรุณาตรวจด้วยตนเอง",
                            proposal.code
                        ),
                        offering_id: Some(offering_id),
                        group_id: Some(group.id),
                    });
                }
            }
        }
        proposal.grouping_state = if !proposal.conflicts.is_empty() {
            PreparationGroupingState::Conflict
        } else if proposal.default_groups.is_empty() {
            PreparationGroupingState::Deferred
        } else {
            PreparationGroupingState::Proposed
        };
    }
    proposals.sort_by(|left, right| {
        offering_kind_order(left.resource_kind)
            .cmp(&offering_kind_order(right.resource_kind))
            .then(left.code.cmp(&right.code))
            .then(left.catalog_version_id.cmp(&right.catalog_version_id))
    });
    let source_hash = stable_hash(&PreviewHashInput {
        academic_term_id,
        academic_year_id: term.academic_year_id,
        term_row_version: term.row_version,
        term_code: &term.code,
        study_program_ids: program_ids,
        proposals: &proposals,
    })?;
    Ok(CurriculumOfferingPreview {
        academic_term_id,
        source_hash,
        proposals,
    })
}

async fn load_existing_preview_offerings(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    requirements: &[PreviewRequirementRow],
) -> Result<HashMap<(LearningOfferingKind, Uuid), Uuid>, AppError> {
    let course_version_ids: Vec<Uuid> = requirements
        .iter()
        .filter(|row| row.resource_kind == LearningOfferingKind::Course)
        .map(|row| row.catalog_version_id)
        .collect();
    let activity_version_ids: Vec<Uuid> = requirements
        .iter()
        .filter(|row| row.resource_kind == LearningOfferingKind::Activity)
        .map(|row| row.catalog_version_id)
        .collect();
    let rows: Vec<ExistingPreviewOfferingRow> = sqlx::query_as(
        r#"SELECT 'course'::text AS resource_kind,
                  detail.subject_version_id AS catalog_version_id,
                  offering.id AS offering_id
           FROM learning_offerings offering
           JOIN course_offering_details detail
             ON detail.learning_offering_id = offering.id
           WHERE offering.academic_term_id = $1
             AND detail.subject_version_id = ANY($2)
           UNION ALL
           SELECT 'activity'::text AS resource_kind,
                  detail.activity_version_id AS catalog_version_id,
                  offering.id AS offering_id
           FROM learning_offerings offering
           JOIN activity_offering_details detail
             ON detail.learning_offering_id = offering.id
           WHERE offering.academic_term_id = $1
             AND detail.activity_version_id = ANY($3)
           ORDER BY resource_kind, catalog_version_id, offering_id"#,
    )
    .bind(academic_term_id)
    .bind(&course_version_ids)
    .bind(&activity_version_ids)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = HashMap::new();
    for row in rows {
        existing
            .entry((row.resource_kind, row.catalog_version_id))
            .or_insert(row.offering_id);
    }
    Ok(existing)
}

async fn load_existing_preparation_groups(
    transaction: &mut Transaction<'_, Postgres>,
    offering_ids: &[Uuid],
) -> Result<Vec<ExistingPreparationGroupRow>, AppError> {
    if offering_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_as(
        r#"SELECT learning_group.id,
                  learning_group.learning_offering_id,
                  learning_group.name,
                  learning_group.generation_source,
                  learning_group.generation_key,
                  coalesce(
                      array_agg(coverage.homeroom_id ORDER BY coverage.homeroom_id)
                          FILTER (WHERE coverage.homeroom_id IS NOT NULL),
                      ARRAY[]::uuid[]
                  ) AS homeroom_ids
           FROM learning_groups learning_group
           LEFT JOIN learning_group_homerooms coverage
             ON coverage.learning_group_id = learning_group.id
           WHERE learning_group.learning_offering_id = ANY($1)
           GROUP BY learning_group.id
           ORDER BY learning_group.learning_offering_id, learning_group.id"#,
    )
    .bind(offering_ids)
    .fetch_all(&mut **transaction)
    .await?)
}

async fn insert_generated_offering(
    transaction: &mut Transaction<'_, Postgres>,
    term: &TermContext,
    proposal: &CurriculumPreparationProposal,
    owner_id: Uuid,
) -> Result<Uuid, AppError> {
    let id = Uuid::new_v5(
        &DELIVERY_NAMESPACE,
        format!(
            "offering:{}:{:?}:{}",
            term.id, proposal.resource_kind, proposal.catalog_version_id
        )
        .as_bytes(),
    );
    sqlx::query(
        r#"INSERT INTO learning_offerings (
               id, academic_term_id, academic_year_id, kind, code_snapshot,
               name_snapshot, source_requirement_kind, source_requirement_id,
               status, owning_organization_unit_id
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'draft', $9)"#,
    )
    .bind(id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(proposal.resource_kind)
    .bind(&proposal.code)
    .bind(&proposal.name)
    .bind(match proposal.resource_kind {
        LearningOfferingKind::Course => "curriculum_course_requirement",
        LearningOfferingKind::Activity => "curriculum_activity_requirement",
    })
    .bind(proposal.requirement_ids.first().copied())
    .bind(owner_id)
    .execute(&mut **transaction)
    .await?;
    match proposal.resource_kind {
        LearningOfferingKind::Course => {
            let source = course_version_source(transaction, proposal.catalog_version_id).await?;
            validate_version_for_term(
                &source.status,
                source.effective_from,
                source.effective_until,
                term,
            )?;
            required_standard_periods_per_week(source.standard_periods_per_week)?;
            sqlx::query(
                r#"INSERT INTO course_offering_details (
                       learning_offering_id, academic_term_id, academic_year_id,
                       subject_version_id, subject_id, curriculum_course_requirement_id,
                       credit, hours, grading_policy
                   ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                             '{"policyCode":"school_default","passingScore":null}'::jsonb)"#,
            )
            .bind(id)
            .bind(term.id)
            .bind(term.academic_year_id)
            .bind(source.subject_version_id)
            .bind(source.subject_id)
            .bind(proposal.requirement_ids.first().copied())
            .bind(validate_canonical_decimal(
                proposal.credit.as_deref().unwrap_or(&source.credit),
                2,
            )?)
            .bind(
                proposal
                    .hours
                    .as_deref()
                    .map(|value| validate_canonical_decimal(value, 2))
                    .transpose()?,
            )
            .execute(&mut **transaction)
            .await?;
        }
        LearningOfferingKind::Activity => {
            let source = activity_version_source(transaction, proposal.catalog_version_id).await?;
            validate_version_for_term(
                &source.status,
                source.effective_from,
                source.effective_until,
                term,
            )?;
            sqlx::query(
                r#"INSERT INTO activity_offering_details (
                       learning_offering_id, academic_term_id, academic_year_id,
                       activity_version_id, activity_id, curriculum_activity_requirement_id,
                       registration_type, scheduling_mode, hours,
                       attendance_requirement, pass_criteria
                   ) VALUES ($1, $2, $3, $4, $5, $6, 'assigned', $7, $8,
                             '{"minimumPercent":null,"requiredSessions":null}'::jsonb,
                             '{"requireAttendance":false,"requireTeacherConfirmation":true,
                               "outcomes":["pass","fail"]}'::jsonb)"#,
            )
            .bind(id)
            .bind(term.id)
            .bind(term.academic_year_id)
            .bind(source.activity_version_id)
            .bind(source.activity_id)
            .bind(proposal.requirement_ids.first().copied())
            .bind(source.scheduling_mode)
            .bind(validate_canonical_decimal(
                proposal.hours.as_deref().unwrap_or(&source.hours),
                2,
            )?)
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(id)
}

async fn insert_homeroom_targets(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    term: &TermContext,
    proposal: &CurriculumPreparationProposal,
) -> Result<(), AppError> {
    for homeroom_id in &proposal.target_homeroom_ids {
        sqlx::query(
            r#"INSERT INTO learning_offering_targets (
                   id, learning_offering_id, academic_term_id, academic_year_id,
                   target_kind, homeroom_id, grade_level_id, study_program_id
               )
               SELECT $1, $2, $3, $4, 'homeroom', homeroom.id,
                      homeroom.grade_level_id, homeroom.study_program_id
               FROM homerooms homeroom
               WHERE homeroom.id = $5
                 AND homeroom.academic_year_id = $4
               ON CONFLICT ON CONSTRAINT learning_offering_targets_unique_key DO NOTHING"#,
        )
        .bind(Uuid::new_v5(
            &DELIVERY_NAMESPACE,
            format!("target:{offering_id}:{homeroom_id}").as_bytes(),
        ))
        .bind(offering_id)
        .bind(term.id)
        .bind(term.academic_year_id)
        .bind(homeroom_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod preparation_choice_tests {
    use super::*;

    #[test]
    fn apply_action_requires_a_reviewed_group_even_when_defaults_are_deferred() {
        let proposal = CurriculumPreparationProposal {
            proposal_id: "proposal".to_string(),
            offering_action: CurriculumPreviewAction::Create,
            resource_kind: LearningOfferingKind::Course,
            catalog_version_id: Uuid::new_v4(),
            requirement_ids: vec![Uuid::new_v4()],
            target_homeroom_ids: vec![Uuid::new_v4()],
            code: "ค21101".to_string(),
            name: "คณิตศาสตร์พื้นฐาน".to_string(),
            credit: Some("1.00".to_string()),
            hours: Some("40.00".to_string()),
            existing_offering_id: None,
            grouping_state: PreparationGroupingState::Deferred,
            default_groups: Vec::new(),
            conflicts: Vec::new(),
        };
        let choice = CurriculumPreparationChoice {
            proposal_id: proposal.proposal_id.clone(),
            action: PreparationAction::Apply,
            groups: Vec::new(),
        };

        assert!(matches!(
            validate_preparation_choices(&[proposal], &[choice]),
            Err(AppError::ValidationError(_))
        ));
    }
}
