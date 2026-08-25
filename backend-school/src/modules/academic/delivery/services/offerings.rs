use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::services::validate_canonical_decimal;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    ActivityAttendanceRequirement, ActivityOfferingSnapshot, ActivityPassCriteria,
    ApplyCurriculumOfferingsRequest, ApplyCurriculumOfferingsResult, CourseGradingPolicy,
    CourseOfferingSnapshot, CreateActivityOfferingRequest, CreateCourseOfferingRequest,
    CreateLearningOfferingRequest, CurriculumOfferingPreview, CurriculumOfferingPreviewItem,
    CurriculumPreviewAction, LearningOffering, LearningOfferingKind, LearningOfferingQuery,
    LearningOfferingRow, LearningOfferingSnapshot, LearningOfferingStatus, LearningOfferingTarget,
    OfferingTargetInput, OfferingTargetKind, PreviewCurriculumOfferingsRequest,
    PublishLearningOfferingRequest, UpdateLearningOfferingRequest,
};
use super::{
    append_audit, require_active_owner, require_writable_term, stable_hash, validate_row_version,
    TermContext,
};

const OFFERING_COLUMNS: &str = r#"
    id, academic_term_id, academic_year_id, kind, code_snapshot, name_snapshot,
    source_requirement_kind, source_requirement_id, status, published_at,
    owning_organization_unit_id, row_version,
    migration_provenance <> '{}'::jsonb AS migrated, created_at, updated_at
"#;

const DELIVERY_NAMESPACE: Uuid = Uuid::from_u128(0x83c8_46da_e34e_5146_8ff1_f7ca_aa6e_20a4);

#[derive(Debug, sqlx::FromRow)]
struct CourseVersionSource {
    subject_version_id: Uuid,
    subject_id: Uuid,
    code: String,
    name: String,
    credit: String,
    hours: Option<String>,
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
    source_requirement_id: Option<Uuid>,
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
    items: &'a [CurriculumOfferingPreviewItem],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyRequestHashInput<'a> {
    academic_term_id: Uuid,
    study_program_ids: &'a [Uuid],
    owning_organization_unit_id: Uuid,
    source_hash: &'a str,
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
           AND ($2 OR owning_organization_unit_id = ANY($3) OR EXISTS (\
               SELECT 1 FROM learning_groups learning_group \
               JOIN learning_group_teachers teacher \
                 ON teacher.learning_group_id = learning_group.id \
               WHERE learning_group.learning_offering_id = learning_offerings.id \
                 AND teacher.teacher_id = $4\
           )) \
         ORDER BY kind, code_snapshot, id LIMIT 500"
    );
    let rows: Vec<LearningOfferingRow> = sqlx::query_as(&sql)
        .bind(query.academic_term_id)
        .bind(filter.includes_school_owned)
        .bind(owner_ids)
        .bind(filter.assigned_actor_id)
        .fetch_all(pool)
        .await?;
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
    let row: (Uuid, Uuid, LearningOfferingStatus, i64) = sqlx::query_as(
        "SELECT academic_term_id, academic_year_id, status, row_version \
         FROM learning_offerings WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาหรือกิจกรรมที่เปิดสอน".to_string()))?;
    if row.2 != LearningOfferingStatus::Draft {
        return Err(AppError::Conflict(
            "ข้อมูล snapshot ที่เผยแพร่แล้วแก้ไขไม่ได้".to_string(),
        ));
    }
    if row.3 != request.row_version {
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
    let request_hash = stable_hash(&ApplyRequestHashInput {
        academic_term_id: request.academic_term_id,
        study_program_ids: &program_ids,
        owning_organization_unit_id: request.owning_organization_unit_id,
        source_hash: &request.source_hash,
    })?;

    if let Some((stored_request_hash, source_hash, offering_ids)) =
        sqlx::query_as::<_, (String, String, Vec<Uuid>)>(
            "SELECT request_hash::text, source_hash::text, offering_ids \
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
            created_count: 0,
            retained_count: offering_ids.len(),
            offering_ids,
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
    if preview
        .items
        .iter()
        .any(|item| item.action == CurriculumPreviewAction::Conflict)
    {
        return Err(AppError::Conflict(
            "มีรายการเปิดสอนที่ขัดแย้งกับหลักสูตร".to_string(),
        ));
    }

    let term = require_writable_term(&mut transaction, request.academic_term_id, true).await?;
    let mut offering_ids = Vec::new();
    let mut created_count = 0_usize;
    let mut retained_count = 0_usize;
    let mut resource_offerings = HashMap::new();
    for item in &preview.items {
        let key = (item.resource_kind, item.catalog_version_id);
        let offering_id = if let Some(existing) = resource_offerings.get(&key) {
            *existing
        } else if let Some(existing) = item.existing_offering_id {
            retained_count += 1;
            existing
        } else {
            created_count += 1;
            insert_generated_offering(
                &mut transaction,
                &term,
                item,
                request.owning_organization_unit_id,
            )
            .await?
        };
        resource_offerings.insert(key, offering_id);
        insert_grade_program_target(&mut transaction, offering_id, &term, item).await?;
        offering_ids.push(offering_id);
    }
    offering_ids.sort_unstable();
    offering_ids.dedup();
    sqlx::query(
        "INSERT INTO learning_delivery_apply_runs (
             idempotency_key, academic_term_id, request_hash, source_hash,
             offering_ids, actor_user_id
         ) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(request.idempotency_key)
    .bind(request.academic_term_id)
    .bind(&request_hash)
    .bind(&preview.source_hash)
    .bind(&offering_ids)
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(ApplyCurriculumOfferingsResult {
        academic_term_id: request.academic_term_id,
        source_hash: preview.source_hash,
        offering_ids,
        created_count,
        retained_count,
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
        "SELECT learning_offering_id, subject_version_id, subject_id, \
         curriculum_course_requirement_id, credit::text AS credit, hours::text AS hours, \
         grading_policy FROM course_offering_details WHERE learning_offering_id = ANY($1)",
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

async fn insert_course(
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
    let (credit, hours) = if let Some(requirement_id) = request.curriculum_course_requirement_id {
        let requirement: (Uuid, Uuid, Uuid, Option<String>, String, Option<String>) =
            sqlx::query_as(
                "SELECT subject_version_id, grade_level_id, study_program_id, \
                 recommended_term_code, credit::text, hours::text \
                 FROM curriculum_course_requirements WHERE id = $1",
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
            requirement.3.as_deref(),
            term,
            &request.targets,
        )?;
        (requirement.4, requirement.5)
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

async fn insert_activity(
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
        let requirement: (Uuid, Uuid, Uuid, Option<String>, String) = sqlx::query_as(
            "SELECT activity_version_id, grade_level_id, study_program_id, \
             recommended_term_code, hours::text \
             FROM curriculum_activity_requirements WHERE id = $1",
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
            requirement.3.as_deref(),
            term,
            &request.targets,
        )?;
        requirement.4
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
                  version.hours_per_semester::text AS hours, version.effective_from,
                  version.effective_until, version.status
           FROM subject_versions version WHERE version.id = $1"#,
    )
    .bind(version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("ไม่พบเวอร์ชันรายวิชา".to_string()))
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
    recommended_term_code: Option<&str>,
    term: &TermContext,
    targets: &[OfferingTargetInput],
) -> Result<(), AppError> {
    if selected_version_id != requirement_version_id {
        return Err(AppError::ValidationError(
            "ข้อกำหนดอ้างอิงคนละเวอร์ชัน".to_string(),
        ));
    }
    if recommended_term_code.is_some_and(|code| !code.eq_ignore_ascii_case(&term.code)) {
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

async fn validate_targets(
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
                  requirement.grade_level_id, version.code, version.name_th AS name,
                  requirement.credit::text AS credit, requirement.hours::text AS hours,
                  version.status AS version_status, version.effective_from,
                  version.effective_until
           FROM curriculum_course_requirements requirement
           JOIN subject_versions version ON version.id = requirement.subject_version_id
           WHERE requirement.study_program_id = ANY($1)
             AND (requirement.recommended_term_code IS NULL
                  OR lower(requirement.recommended_term_code) = lower($2))
           UNION ALL
           SELECT 'activity'::text AS resource_kind,
                  requirement.activity_version_id AS catalog_version_id,
                  requirement.id AS requirement_id, requirement.study_program_id,
                  requirement.grade_level_id, stable.code, version.name,
                  NULL::text AS credit, requirement.hours::text AS hours,
                  version.status AS version_status, version.effective_from,
                  version.effective_until
           FROM curriculum_activity_requirements requirement
           JOIN activity_versions version ON version.id = requirement.activity_version_id
           JOIN activities stable ON stable.id = version.activity_id
           WHERE requirement.study_program_id = ANY($1)
             AND (requirement.recommended_term_code IS NULL
                  OR lower(requirement.recommended_term_code) = lower($2))
           ORDER BY resource_kind, catalog_version_id, study_program_id, grade_level_id, requirement_id"#,
    )
    .bind(program_ids)
    .bind(&term.code)
    .fetch_all(&mut **transaction)
    .await?;
    let existing_offerings =
        load_existing_preview_offerings(transaction, academic_term_id, &rows).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        validate_version_for_term(
            &row.version_status,
            row.effective_from,
            row.effective_until,
            &term,
        )?;
        let existing = existing_offerings
            .get(&(row.resource_kind, row.catalog_version_id))
            .copied();
        let (action, existing_offering_id, conflict_reason) = match existing {
            None => (CurriculumPreviewAction::Create, None, None),
            Some((id, source_id)) if source_id == Some(row.requirement_id) => {
                (CurriculumPreviewAction::Retain, Some(id), None)
            }
            Some((id, _)) => (
                CurriculumPreviewAction::Conflict,
                Some(id),
                Some("รายการเดิมอ้างอิงข้อกำหนดคนละรายการ".to_string()),
            ),
        };
        items.push(CurriculumOfferingPreviewItem {
            action,
            resource_kind: row.resource_kind,
            catalog_version_id: row.catalog_version_id,
            requirement_id: row.requirement_id,
            study_program_id: row.study_program_id,
            grade_level_id: row.grade_level_id,
            code: row.code,
            name: row.name,
            credit: row.credit,
            hours: row.hours,
            existing_offering_id,
            conflict_reason,
        });
    }
    let source_hash = stable_hash(&PreviewHashInput {
        academic_term_id,
        academic_year_id: term.academic_year_id,
        term_row_version: term.row_version,
        term_code: &term.code,
        study_program_ids: program_ids,
        items: &items,
    })?;
    Ok(CurriculumOfferingPreview {
        academic_term_id,
        source_hash,
        items,
    })
}

async fn load_existing_preview_offerings(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    requirements: &[PreviewRequirementRow],
) -> Result<HashMap<(LearningOfferingKind, Uuid), (Uuid, Option<Uuid>)>, AppError> {
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
                  offering.id AS offering_id,
                  offering.source_requirement_id
           FROM learning_offerings offering
           JOIN course_offering_details detail
             ON detail.learning_offering_id = offering.id
           WHERE offering.academic_term_id = $1
             AND detail.subject_version_id = ANY($2)
           UNION ALL
           SELECT 'activity'::text AS resource_kind,
                  detail.activity_version_id AS catalog_version_id,
                  offering.id AS offering_id,
                  offering.source_requirement_id
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
            .or_insert((row.offering_id, row.source_requirement_id));
    }
    Ok(existing)
}

async fn insert_generated_offering(
    transaction: &mut Transaction<'_, Postgres>,
    term: &TermContext,
    item: &CurriculumOfferingPreviewItem,
    owner_id: Uuid,
) -> Result<Uuid, AppError> {
    let id = Uuid::new_v5(
        &DELIVERY_NAMESPACE,
        format!(
            "offering:{}:{:?}:{}",
            term.id, item.resource_kind, item.catalog_version_id
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
    .bind(item.resource_kind)
    .bind(&item.code)
    .bind(&item.name)
    .bind(match item.resource_kind {
        LearningOfferingKind::Course => "curriculum_course_requirement",
        LearningOfferingKind::Activity => "curriculum_activity_requirement",
    })
    .bind(item.requirement_id)
    .bind(owner_id)
    .execute(&mut **transaction)
    .await?;
    match item.resource_kind {
        LearningOfferingKind::Course => {
            let source = course_version_source(transaction, item.catalog_version_id).await?;
            validate_version_for_term(
                &source.status,
                source.effective_from,
                source.effective_until,
                term,
            )?;
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
            .bind(item.requirement_id)
            .bind(validate_canonical_decimal(
                item.credit.as_deref().unwrap_or(&source.credit),
                2,
            )?)
            .bind(
                item.hours
                    .as_deref()
                    .map(|value| validate_canonical_decimal(value, 2))
                    .transpose()?,
            )
            .execute(&mut **transaction)
            .await?;
        }
        LearningOfferingKind::Activity => {
            let source = activity_version_source(transaction, item.catalog_version_id).await?;
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
            .bind(item.requirement_id)
            .bind(source.scheduling_mode)
            .bind(validate_canonical_decimal(
                item.hours.as_deref().unwrap_or(&source.hours),
                2,
            )?)
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(id)
}

async fn insert_grade_program_target(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    term: &TermContext,
    item: &CurriculumOfferingPreviewItem,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO learning_offering_targets (
               id, learning_offering_id, academic_term_id, academic_year_id,
               target_kind, grade_level_id, study_program_id
           ) VALUES ($1, $2, $3, $4, 'grade_program', $5, $6)
           ON CONFLICT (learning_offering_id, grade_level_id, study_program_id)
           WHERE target_kind = 'grade_program' AND homeroom_id IS NULL
           DO NOTHING"#,
    )
    .bind(Uuid::new_v5(
        &DELIVERY_NAMESPACE,
        format!(
            "target:{offering_id}:{}:{}",
            item.grade_level_id, item.study_program_id
        )
        .as_bytes(),
    ))
    .bind(offering_id)
    .bind(term.id)
    .bind(term.academic_year_id)
    .bind(item.grade_level_id)
    .bind(item.study_program_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
