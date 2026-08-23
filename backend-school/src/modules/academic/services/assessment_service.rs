use std::collections::{HashMap, HashSet};

use bigdecimal::BigDecimal;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::core::services::validate_canonical_decimal;
use crate::modules::academic::delivery::models::CourseGradingPolicy;
use crate::modules::academic::models::assessment::{
    AssessmentCategory, AssessmentCategoryRow, AssessmentItem, AssessmentItemRow,
    AssessmentOfferingScopeRow, AssessmentPlanDetail, AssessmentPlanListQuery, AssessmentPlanRow,
    AssessmentPlanSummary, AssessmentSettingsResponse, SaveAssessmentCategoryRequest,
    SaveAssessmentItemRequest, SaveAssessmentPlanRequest, UpdateAssessmentSettingsRequest,
};
use crate::modules::system::services::feature_toggle_service;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

pub const TEACHER_ACCESS_FEATURE_CODE: &str = "academic_assessment_teacher_access";

const VALID_CATEGORY_CODES: &[&str] = &[
    "before_midterm",
    "midterm",
    "after_midterm",
    "final",
    "custom",
];
const VALID_EXAM_MODES: &[&str] = &["none", "in_timetable", "outside_timetable", "practical"];
const SCHEDULED_EXAM_MODES: &[&str] = &["in_timetable", "outside_timetable"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationStatus {
    NotStarted,
    Complete,
    UnderAllocated,
    OverAllocated,
}

impl AllocationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Complete => "complete",
            Self::UnderAllocated => "under_allocated",
            Self::OverAllocated => "over_allocated",
        }
    }
}

#[derive(Debug, FromRow)]
struct AssessmentPlanSummaryRow {
    plan_id: Option<Uuid>,
    offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    subject_id: Uuid,
    subject_version_display_label: String,
    offering_code: String,
    offering_name: String,
    status: String,
    row_version: Option<i64>,
    learning_group_ids: Vec<Uuid>,
    learning_group_count: i64,
    category_count: i64,
    item_count: i64,
    total_score: BigDecimal,
    expected_total_score: String,
}

pub fn allocation_status(max_score: &BigDecimal, item_total: &BigDecimal) -> AllocationStatus {
    let zero = BigDecimal::from(0);
    if max_score == &zero && item_total == &zero {
        AllocationStatus::NotStarted
    } else if item_total == max_score {
        AllocationStatus::Complete
    } else if item_total < max_score {
        AllocationStatus::UnderAllocated
    } else {
        AllocationStatus::OverAllocated
    }
}

pub fn default_categories() -> Vec<SaveAssessmentCategoryRequest> {
    vec![
        default_category("before_midterm", "ก่อนกลางภาค", "none", 10),
        default_category("midterm", "กลางภาค", "in_timetable", 20),
        default_category("after_midterm", "หลังกลางภาค", "none", 30),
        default_category("final", "ปลายภาค", "in_timetable", 40),
    ]
}

fn default_category(
    code: &str,
    name: &str,
    exam_mode: &str,
    display_order: i32,
) -> SaveAssessmentCategoryRequest {
    SaveAssessmentCategoryRequest {
        id: None,
        code: Some(code.to_string()),
        name: name.to_string(),
        max_score: "0.00".to_string(),
        exam_mode: exam_mode.to_string(),
        exam_duration_minutes: None,
        display_order,
        items: Vec::new(),
    }
}

pub fn validate_plan_payload(
    payload: &SaveAssessmentPlanRequest,
    expected_total: &BigDecimal,
    require_complete: bool,
) -> Result<(), AppError> {
    let zero = BigDecimal::from(0);
    let mut category_ids = HashSet::new();
    let mut item_ids = HashSet::new();
    let mut category_codes = HashSet::new();
    let mut category_total = BigDecimal::from(0);

    for category in &payload.categories {
        if category.name.trim().is_empty() {
            return Err(AppError::ValidationError("ต้องระบุชื่อหมวดคะแนน".to_string()));
        }
        if category.id.is_some_and(|id| !category_ids.insert(id)) {
            return Err(AppError::ValidationError("มีหมวดคะแนนซ้ำในคำขอ".to_string()));
        }
        if let Some(code) = category.code.as_deref() {
            if !VALID_CATEGORY_CODES.contains(&code) {
                return Err(AppError::ValidationError("รหัสหมวดคะแนนไม่ถูกต้อง".to_string()));
            }
            if code != "custom" && !category_codes.insert(code) {
                return Err(AppError::ValidationError("รหัสหมวดคะแนนซ้ำกัน".to_string()));
            }
        }
        if !VALID_EXAM_MODES.contains(&category.exam_mode.as_str()) {
            return Err(AppError::ValidationError("รูปแบบการสอบไม่ถูกต้อง".to_string()));
        }
        validate_exam_duration(category, require_complete)?;

        let max_score = validate_canonical_decimal(&category.max_score, 2)?;
        if max_score < zero {
            return Err(AppError::ValidationError("คะแนนต้องไม่ติดลบ".to_string()));
        }
        category_total += max_score.clone();

        let mut active_item_total = BigDecimal::from(0);
        let mut has_active_item = false;
        for item in &category.items {
            if item.name.trim().is_empty() {
                return Err(AppError::ValidationError("ต้องระบุชื่อรายการคะแนน".to_string()));
            }
            if item.id.is_some_and(|id| !item_ids.insert(id)) {
                return Err(AppError::ValidationError(
                    "มีรายการคะแนนซ้ำในคำขอ".to_string(),
                ));
            }
            let item_score = validate_canonical_decimal(&item.max_score, 2)?;
            if item_score < zero {
                return Err(AppError::ValidationError("คะแนนต้องไม่ติดลบ".to_string()));
            }
            if item.is_active {
                has_active_item = true;
                active_item_total += item_score;
            }
        }
        if active_item_total > max_score {
            return Err(AppError::ValidationError(format!(
                "คะแนนรายการในหมวด {} เกินคะแนนหมวด",
                category.name.trim()
            )));
        }
        if require_complete && has_active_item && active_item_total != max_score {
            return Err(AppError::ValidationError(format!(
                "คะแนนรายการในหมวด {} ต้องรวมเท่ากับคะแนนหมวดก่อนส่ง",
                category.name.trim()
            )));
        }
    }

    if category_total > *expected_total {
        return Err(AppError::ValidationError(
            "คะแนนรวมเกินคะแนนตามนโยบายของรายวิชาที่เปิดสอน".to_string(),
        ));
    }
    if require_complete && category_total != *expected_total {
        return Err(AppError::ValidationError(
            "คะแนนรวมต้องเท่ากับคะแนนตามนโยบายก่อนส่ง".to_string(),
        ));
    }
    Ok(())
}

fn validate_exam_duration(
    category: &SaveAssessmentCategoryRequest,
    require_complete: bool,
) -> Result<(), AppError> {
    if let Some(duration) = category.exam_duration_minutes {
        if duration <= 0 {
            return Err(AppError::ValidationError(
                "ระยะเวลาสอบต้องมากกว่า 0 นาที".to_string(),
            ));
        }
        if !SCHEDULED_EXAM_MODES.contains(&category.exam_mode.as_str()) {
            return Err(AppError::ValidationError(
                "ระยะเวลาสอบระบุได้เฉพาะหมวดสอบในตารางหรือนอกตาราง".to_string(),
            ));
        }
    } else if require_complete && SCHEDULED_EXAM_MODES.contains(&category.exam_mode.as_str()) {
        return Err(AppError::ValidationError(
            "ต้องระบุระยะเวลาของหมวดสอบก่อนส่ง".to_string(),
        ));
    }
    Ok(())
}

pub async fn list_assessment_plans(
    pool: &PgPool,
    query: &AssessmentPlanListQuery,
    access: &AcademicResourceListFilter,
) -> Result<Vec<AssessmentPlanSummary>, AppError> {
    let owner_ids = access.allowed_organization_unit_ids();
    let rows: Vec<AssessmentPlanSummaryRow> = sqlx::query_as(
        r#"
        SELECT plan.id AS plan_id,
               offering.id AS offering_id,
               offering.academic_term_id,
               offering.academic_year_id,
               detail.subject_id,
               concat(
                   coalesce(version.name_th, version.name_en, offering.name_snapshot),
                   ' · v', version.version_no
               ) AS subject_version_display_label,
               offering.code_snapshot AS offering_code,
               offering.name_snapshot AS offering_name,
               coalesce(plan.status, 'not_configured') AS status,
               plan.row_version,
               ARRAY(
                   SELECT learning_group.id
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
                   ORDER BY learning_group.id
               ) AS learning_group_ids,
               (
                   SELECT count(*)
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
               )::bigint AS learning_group_count,
               (
                   SELECT count(*)
                   FROM course_assessment_categories category
                   WHERE category.plan_id = plan.id
               )::bigint AS category_count,
               (
                   SELECT count(*)
                   FROM course_assessment_items item
                   JOIN course_assessment_categories category ON category.id = item.category_id
                   WHERE category.plan_id = plan.id
               )::bigint AS item_count,
               coalesce((
                   SELECT sum(category.max_score)
                   FROM course_assessment_categories category
                   WHERE category.plan_id = plan.id
               ), 0::numeric) AS total_score,
               coalesce(detail.grading_policy ->> 'totalScore', '100.00')
                   AS expected_total_score
        FROM learning_offerings offering
        JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
        JOIN subject_versions version ON version.id = detail.subject_version_id
        LEFT JOIN course_assessment_plans plan ON plan.learning_offering_id = offering.id
        WHERE offering.academic_term_id = $1
          AND offering.kind = 'course'
          AND ($2::uuid IS NULL OR detail.subject_id = $2)
          AND ($3::uuid IS NULL OR EXISTS (
              SELECT 1
              FROM learning_groups learning_group
              JOIN learning_group_teachers teacher
                ON teacher.learning_group_id = learning_group.id
              WHERE learning_group.learning_offering_id = offering.id
                AND teacher.teacher_id = $3
          ))
          AND ($4::text IS NULL OR coalesce(plan.status, 'not_configured') = $4)
          AND ($5 OR offering.owning_organization_unit_id = ANY($6) OR EXISTS (
              SELECT 1
              FROM learning_groups learning_group
              JOIN learning_group_teachers teacher
                ON teacher.learning_group_id = learning_group.id
              WHERE learning_group.learning_offering_id = offering.id
                AND teacher.teacher_id = $7
          ))
        ORDER BY offering.code_snapshot, offering.id
        LIMIT 500
        "#,
    )
    .bind(query.academic_term_id)
    .bind(query.subject_id)
    .bind(query.instructor_id)
    .bind(query.status.as_deref())
    .bind(access.includes_school_owned)
    .bind(owner_ids)
    .bind(access.assigned_actor_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AssessmentPlanSummary {
            plan_id: row.plan_id,
            offering_id: row.offering_id,
            academic_term_id: row.academic_term_id,
            academic_year_id: row.academic_year_id,
            subject_id: row.subject_id,
            subject_version_display_label: row.subject_version_display_label,
            offering_code: row.offering_code,
            offering_name: row.offering_name,
            status: row.status,
            row_version: row.row_version,
            learning_group_ids: row.learning_group_ids,
            learning_group_count: row.learning_group_count,
            category_count: row.category_count,
            item_count: row.item_count,
            total_score: decimal_string(&row.total_score),
            expected_total_score: row.expected_total_score,
        })
        .collect())
}

pub async fn get_assessment_settings(
    pool: &PgPool,
) -> Result<AssessmentSettingsResponse, AppError> {
    let teacher_access_enabled =
        feature_toggle_service::is_feature_enabled(pool, TEACHER_ACCESS_FEATURE_CODE).await?;
    Ok(AssessmentSettingsResponse {
        teacher_access_enabled,
    })
}

pub async fn update_assessment_settings(
    pool: &PgPool,
    payload: UpdateAssessmentSettingsRequest,
) -> Result<AssessmentSettingsResponse, AppError> {
    feature_toggle_service::update_feature_enabled_by_code(
        pool,
        TEACHER_ACCESS_FEATURE_CODE,
        payload.teacher_access_enabled,
    )
    .await?;
    Ok(AssessmentSettingsResponse {
        teacher_access_enabled: payload.teacher_access_enabled,
    })
}

pub fn require_assessment_settings_read_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_READ_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ])
}

pub fn require_assessment_settings_manage_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ])
}

pub async fn require_teacher_access_enabled_for_reader(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<(), AppError> {
    if actor.has_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_READ_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ]) {
        return Ok(());
    }
    require_teacher_access_enabled(pool).await
}

pub async fn require_teacher_access_enabled_for_manager(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<(), AppError> {
    if actor.has_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ]) {
        return Ok(());
    }
    require_teacher_access_enabled(pool).await
}

async fn require_teacher_access_enabled(pool: &PgPool) -> Result<(), AppError> {
    if get_assessment_settings(pool).await?.teacher_access_enabled {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "ยังไม่เปิดให้ครูกรอกโครงสร้างคะแนน".to_string(),
        ))
    }
}

pub async fn get_plan_detail(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    fetch_plan_detail(pool, offering_id).await
}

pub async fn save_plan(
    pool: &PgPool,
    offering_id: Uuid,
    actor_user_id: Uuid,
    payload: SaveAssessmentPlanRequest,
) -> Result<AssessmentPlanDetail, AppError> {
    let mut transaction = pool.begin().await?;
    let scope = resolve_offering_scope_in_tx(&mut transaction, offering_id, true).await?;
    let (_, expected_total) = grading_policy(&scope)?;
    validate_plan_payload(&payload, &expected_total, false)?;

    let existing: Option<AssessmentPlanRow> = sqlx::query_as(
        r#"SELECT id, learning_offering_id, academic_term_id, academic_year_id,
                  status, row_version, submitted_at, locked_at
           FROM course_assessment_plans
           WHERE learning_offering_id = $1
           FOR UPDATE"#,
    )
    .bind(offering_id)
    .fetch_optional(&mut *transaction)
    .await?;

    let plan_id = match existing {
        Some(plan) => {
            if plan.status == "locked" {
                return Err(AppError::Conflict("แผนคะแนนถูกล็อกแล้ว".to_string()));
            }
            let expected_version = payload.row_version.ok_or_else(|| {
                AppError::Conflict("ต้องระบุ rowVersion ของแผนคะแนนเดิม".to_string())
            })?;
            if expected_version != plan.row_version {
                return Err(AppError::Conflict(
                    "แผนคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string(),
                ));
            }
            let updated = sqlx::query(
                r#"UPDATE course_assessment_plans
                   SET status = 'saved', submitted_at = NULL, submitted_by = NULL,
                       row_version = row_version + 1, updated_at = now()
                   WHERE id = $1 AND row_version = $2"#,
            )
            .bind(plan.id)
            .bind(expected_version)
            .execute(&mut *transaction)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "แผนคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string(),
                ));
            }
            plan.id
        }
        None => {
            if payload.row_version.is_some() {
                return Err(AppError::Conflict(
                    "ยังไม่มีแผนคะแนนสำหรับ offering นี้".to_string(),
                ));
            }
            let plan_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO course_assessment_plans (
                       id, academic_term_id, subject_version_id,
                       learning_offering_id, academic_year_id, status, row_version,
                       migration_provenance, created_at, updated_at
                   ) VALUES ($1, $2, $3, $4, $5, 'saved', 1, '{}'::jsonb, now(), now())"#,
            )
            .bind(plan_id)
            .bind(scope.academic_term_id)
            .bind(scope.subject_version_id)
            .bind(scope.offering_id)
            .bind(scope.academic_year_id)
            .execute(&mut *transaction)
            .await?;
            plan_id
        }
    };

    replace_plan_structure(
        &mut transaction,
        plan_id,
        actor_user_id,
        &payload.categories,
    )
    .await?;
    transaction.commit().await?;
    fetch_plan_detail(pool, offering_id).await
}

pub async fn submit_plan(
    pool: &PgPool,
    offering_id: Uuid,
    actor_user_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let mut transaction = pool.begin().await?;
    let scope = resolve_offering_scope_in_tx(&mut transaction, offering_id, true).await?;
    let (_, expected_total) = grading_policy(&scope)?;
    let plan: AssessmentPlanRow = sqlx::query_as(
        r#"SELECT id, learning_offering_id, academic_term_id, academic_year_id,
                  status, row_version, submitted_at, locked_at
           FROM course_assessment_plans
           WHERE learning_offering_id = $1
           FOR UPDATE"#,
    )
    .bind(offering_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::ValidationError("ยังไม่มีแผนคะแนนให้ส่ง".to_string()))?;
    if plan.status == "locked" {
        return Err(AppError::Conflict("แผนคะแนนถูกล็อกแล้ว".to_string()));
    }

    let payload = load_plan_payload_in_tx(&mut transaction, plan.id, plan.row_version).await?;
    validate_plan_payload(&payload, &expected_total, true)?;
    sqlx::query(
        r#"UPDATE course_assessment_plans
           SET status = 'submitted', submitted_at = now(), submitted_by = $2,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $1"#,
    )
    .bind(plan.id)
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    fetch_plan_detail(pool, offering_id).await
}

async fn resolve_offering_scope(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentOfferingScopeRow, AppError> {
    sqlx::query_as(&offering_scope_sql(false))
        .bind(offering_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))
}

async fn resolve_offering_scope_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    require_writable: bool,
) -> Result<AssessmentOfferingScopeRow, AppError> {
    let scope: AssessmentOfferingScopeRow = sqlx::query_as(&offering_scope_sql(true))
        .bind(offering_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))?;
    if require_writable && matches!(scope.academic_term_status.as_str(), "closed" | "archived") {
        return Err(AppError::Conflict(
            "ภาคเรียนนี้ปิดแล้ว ไม่สามารถแก้ไขโครงสร้างคะแนนได้".to_string(),
        ));
    }
    Ok(scope)
}

fn offering_scope_sql(for_update: bool) -> String {
    format!(
        r#"SELECT offering.id AS offering_id,
                  offering.academic_term_id,
                  offering.academic_year_id,
                  term.status AS academic_term_status,
                  detail.subject_version_id,
                  detail.subject_id,
                  concat(
                      coalesce(version.name_th, version.name_en, offering.name_snapshot),
                      ' · v', version.version_no
                  ) AS subject_version_display_label,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  detail.grading_policy
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
           JOIN subject_versions version ON version.id = detail.subject_version_id
           WHERE offering.id = $1 AND offering.kind = 'course'{}"#,
        if for_update {
            " FOR UPDATE OF offering"
        } else {
            ""
        }
    )
}

fn grading_policy(
    scope: &AssessmentOfferingScopeRow,
) -> Result<(CourseGradingPolicy, BigDecimal), AppError> {
    let policy = scope.grading_policy.0.clone();
    let expected_total = validate_canonical_decimal(&policy.total_score, 2).map_err(|error| {
        tracing::error!(reason = "invalid_course_total_score_snapshot", ?error);
        AppError::InternalServerError("คะแนนรวมตามนโยบายไม่ถูกต้อง".to_string())
    })?;
    Ok((policy, expected_total))
}

async fn replace_plan_structure(
    transaction: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    actor_user_id: Uuid,
    categories: &[SaveAssessmentCategoryRequest],
) -> Result<(), AppError> {
    let existing_category_ids: HashSet<Uuid> = sqlx::query_scalar(
        "SELECT id FROM course_assessment_categories WHERE plan_id = $1 FOR UPDATE",
    )
    .bind(plan_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect();
    let mut retained_category_ids = Vec::with_capacity(categories.len());

    for category in categories {
        let category_id = match category.id {
            Some(id) if existing_category_ids.contains(&id) => id,
            Some(_) => {
                return Err(AppError::ValidationError(
                    "หมวดคะแนนไม่ได้อยู่ในแผนนี้".to_string(),
                ));
            }
            None => Uuid::new_v4(),
        };
        let max_score = validate_canonical_decimal(&category.max_score, 2)?;
        sqlx::query(
            r#"INSERT INTO course_assessment_categories (
                   id, plan_id, code, name, max_score, exam_mode, exam_duration_minutes,
                   display_order, created_by, updated_by, created_at, updated_at
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, now(), now())
               ON CONFLICT (id) DO UPDATE
               SET code = EXCLUDED.code,
                   name = EXCLUDED.name,
                   max_score = EXCLUDED.max_score,
                   exam_mode = EXCLUDED.exam_mode,
                   exam_duration_minutes = EXCLUDED.exam_duration_minutes,
                   display_order = EXCLUDED.display_order,
                   updated_by = EXCLUDED.updated_by,
                   updated_at = now()
               WHERE course_assessment_categories.plan_id = EXCLUDED.plan_id"#,
        )
        .bind(category_id)
        .bind(plan_id)
        .bind(category.code.as_deref())
        .bind(category.name.trim())
        .bind(max_score)
        .bind(&category.exam_mode)
        .bind(category.exam_duration_minutes)
        .bind(category.display_order)
        .bind(actor_user_id)
        .execute(&mut **transaction)
        .await?;

        replace_category_items(transaction, category_id, &category.items).await?;
        retained_category_ids.push(category_id);
    }

    sqlx::query(
        r#"DELETE FROM course_assessment_categories
           WHERE plan_id = $1 AND NOT (id = ANY($2))"#,
    )
    .bind(plan_id)
    .bind(&retained_category_ids)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn replace_category_items(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
    items: &[SaveAssessmentItemRequest],
) -> Result<(), AppError> {
    let existing_item_ids: HashSet<Uuid> = sqlx::query_scalar(
        "SELECT id FROM course_assessment_items WHERE category_id = $1 FOR UPDATE",
    )
    .bind(category_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect();
    let mut retained_item_ids = Vec::with_capacity(items.len());
    for item in items {
        let item_id = match item.id {
            Some(id) if existing_item_ids.contains(&id) => id,
            Some(_) => {
                return Err(AppError::ValidationError(
                    "รายการคะแนนไม่ได้อยู่ในหมวดนี้".to_string(),
                ));
            }
            None => Uuid::new_v4(),
        };
        let max_score = validate_canonical_decimal(&item.max_score, 2)?;
        sqlx::query(
            r#"INSERT INTO course_assessment_items (
                   id, category_id, name, max_score, display_order, is_active,
                   created_at, updated_at
               ) VALUES ($1, $2, $3, $4, $5, $6, now(), now())
               ON CONFLICT (id) DO UPDATE
               SET name = EXCLUDED.name,
                   max_score = EXCLUDED.max_score,
                   display_order = EXCLUDED.display_order,
                   is_active = EXCLUDED.is_active,
                   updated_at = now()
               WHERE course_assessment_items.category_id = EXCLUDED.category_id"#,
        )
        .bind(item_id)
        .bind(category_id)
        .bind(item.name.trim())
        .bind(max_score)
        .bind(item.display_order)
        .bind(item.is_active)
        .execute(&mut **transaction)
        .await?;
        retained_item_ids.push(item_id);
    }
    sqlx::query(
        r#"DELETE FROM course_assessment_items
           WHERE category_id = $1 AND NOT (id = ANY($2))"#,
    )
    .bind(category_id)
    .bind(&retained_item_ids)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn load_plan_payload_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    row_version: i64,
) -> Result<SaveAssessmentPlanRequest, AppError> {
    let categories: Vec<AssessmentCategoryRow> = sqlx::query_as(
        r#"SELECT id, code, name, max_score, exam_mode, exam_duration_minutes, display_order
           FROM course_assessment_categories
           WHERE plan_id = $1
           ORDER BY display_order, id"#,
    )
    .bind(plan_id)
    .fetch_all(&mut **transaction)
    .await?;
    let category_ids: Vec<Uuid> = categories.iter().map(|category| category.id).collect();
    let items: Vec<AssessmentItemRow> = if category_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT id, category_id, name, max_score, display_order, is_active
               FROM course_assessment_items
               WHERE category_id = ANY($1)
               ORDER BY category_id, display_order, id"#,
        )
        .bind(&category_ids)
        .fetch_all(&mut **transaction)
        .await?
    };
    let mut items_by_category: HashMap<Uuid, Vec<SaveAssessmentItemRequest>> = HashMap::new();
    for item in items {
        items_by_category
            .entry(item.category_id)
            .or_default()
            .push(SaveAssessmentItemRequest {
                id: Some(item.id),
                name: item.name,
                max_score: decimal_string(&item.max_score),
                display_order: item.display_order,
                is_active: item.is_active,
            });
    }
    Ok(SaveAssessmentPlanRequest {
        row_version: Some(row_version),
        categories: categories
            .into_iter()
            .map(|category| SaveAssessmentCategoryRequest {
                id: Some(category.id),
                code: category.code,
                name: category.name,
                max_score: decimal_string(&category.max_score),
                exam_mode: category.exam_mode,
                exam_duration_minutes: category.exam_duration_minutes,
                display_order: category.display_order,
                items: items_by_category.remove(&category.id).unwrap_or_default(),
            })
            .collect(),
    })
}

async fn fetch_plan_detail(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let scope = resolve_offering_scope(pool, offering_id).await?;
    let (policy, expected_total) = grading_policy(&scope)?;
    let plan: Option<AssessmentPlanRow> = sqlx::query_as(
        r#"SELECT id, learning_offering_id, academic_term_id, academic_year_id,
                  status, row_version, submitted_at, locked_at
           FROM course_assessment_plans
           WHERE learning_offering_id = $1"#,
    )
    .bind(offering_id)
    .fetch_optional(pool)
    .await?;
    let learning_group_ids = sqlx::query_scalar(
        "SELECT id FROM learning_groups WHERE learning_offering_id = $1 ORDER BY id",
    )
    .bind(offering_id)
    .fetch_all(pool)
    .await?;

    let (id, status, row_version, submitted_at, locked_at, categories) = match plan {
        Some(plan) => {
            let categories = fetch_categories(pool, plan.id).await?;
            (
                Some(plan.id),
                plan.status,
                Some(plan.row_version),
                plan.submitted_at,
                plan.locked_at,
                categories,
            )
        }
        None => (
            None,
            "not_configured".to_string(),
            None,
            None,
            None,
            virtual_default_categories(),
        ),
    };

    Ok(AssessmentPlanDetail {
        id,
        offering_id: scope.offering_id,
        academic_term_id: scope.academic_term_id,
        academic_year_id: scope.academic_year_id,
        subject_id: scope.subject_id,
        subject_version_display_label: scope.subject_version_display_label,
        offering_code: scope.offering_code,
        offering_name: scope.offering_name,
        grading_policy: policy,
        expected_total_score: decimal_string(&expected_total),
        status,
        row_version,
        submitted_at,
        locked_at,
        learning_group_ids,
        categories,
    })
}

async fn fetch_categories(
    pool: &PgPool,
    plan_id: Uuid,
) -> Result<Vec<AssessmentCategory>, AppError> {
    let categories: Vec<AssessmentCategoryRow> = sqlx::query_as(
        r#"SELECT id, code, name, max_score, exam_mode, exam_duration_minutes, display_order
           FROM course_assessment_categories
           WHERE plan_id = $1
           ORDER BY display_order, id"#,
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await?;
    let category_ids: Vec<Uuid> = categories.iter().map(|category| category.id).collect();
    let items: Vec<AssessmentItemRow> = if category_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT id, category_id, name, max_score, display_order, is_active
               FROM course_assessment_items
               WHERE category_id = ANY($1)
               ORDER BY category_id, display_order, id"#,
        )
        .bind(&category_ids)
        .fetch_all(pool)
        .await?
    };
    let mut items_by_category: HashMap<Uuid, Vec<(AssessmentItem, BigDecimal)>> = HashMap::new();
    for item in items {
        let score = item.max_score.clone();
        items_by_category
            .entry(item.category_id)
            .or_default()
            .push((
                AssessmentItem {
                    id: item.id,
                    category_id: item.category_id,
                    name: item.name,
                    max_score: decimal_string(&item.max_score),
                    display_order: item.display_order,
                    is_active: item.is_active,
                },
                score,
            ));
    }

    Ok(categories
        .into_iter()
        .map(|category| {
            let item_values = items_by_category.remove(&category.id).unwrap_or_default();
            let active_total = item_values
                .iter()
                .filter(|(item, _)| item.is_active)
                .fold(BigDecimal::from(0), |sum, (_, score)| sum + score.clone());
            let effective_total = if item_values.iter().any(|(item, _)| item.is_active) {
                active_total.clone()
            } else {
                category.max_score.clone()
            };
            AssessmentCategory {
                id: Some(category.id),
                code: category.code,
                name: category.name,
                max_score: decimal_string(&category.max_score),
                exam_mode: category.exam_mode,
                exam_duration_minutes: category.exam_duration_minutes,
                display_order: category.display_order,
                item_total_score: decimal_string(&active_total),
                allocation_status: allocation_status(&category.max_score, &effective_total)
                    .as_str()
                    .to_string(),
                items: item_values.into_iter().map(|(item, _)| item).collect(),
            }
        })
        .collect())
}

fn virtual_default_categories() -> Vec<AssessmentCategory> {
    default_categories()
        .into_iter()
        .map(|category| AssessmentCategory {
            id: None,
            code: category.code,
            name: category.name,
            max_score: category.max_score,
            exam_mode: category.exam_mode,
            exam_duration_minutes: category.exam_duration_minutes,
            display_order: category.display_order,
            item_total_score: "0.00".to_string(),
            allocation_status: AllocationStatus::NotStarted.as_str().to_string(),
            items: Vec::new(),
        })
        .collect()
}

fn decimal_string(value: &BigDecimal) -> String {
    value.normalized().to_string()
}

#[cfg(test)]
mod tests {
    use super::{allocation_status, default_categories, validate_plan_payload, AllocationStatus};
    use crate::error::AppError;
    use crate::modules::academic::models::assessment::{
        SaveAssessmentCategoryRequest, SaveAssessmentItemRequest, SaveAssessmentPlanRequest,
    };
    use bigdecimal::BigDecimal;

    fn item(score: &str) -> SaveAssessmentItemRequest {
        SaveAssessmentItemRequest {
            id: None,
            name: "รายการ".to_string(),
            max_score: score.to_string(),
            display_order: 10,
            is_active: true,
        }
    }

    fn category(
        score: &str,
        items: Vec<SaveAssessmentItemRequest>,
    ) -> SaveAssessmentCategoryRequest {
        SaveAssessmentCategoryRequest {
            id: None,
            code: Some("custom".to_string()),
            name: "หมวด".to_string(),
            max_score: score.to_string(),
            exam_mode: "none".to_string(),
            exam_duration_minutes: None,
            display_order: 10,
            items,
        }
    }

    #[test]
    fn uses_decimal_allocation_without_float_rounding() {
        let maximum = "0.30".parse::<BigDecimal>().unwrap();
        let allocated =
            "0.10".parse::<BigDecimal>().unwrap() + "0.20".parse::<BigDecimal>().unwrap();
        assert_eq!(
            allocation_status(&maximum, &allocated),
            AllocationStatus::Complete
        );
    }

    #[test]
    fn draft_may_be_incomplete_but_never_exceed_policy() {
        let expected = "100.00".parse::<BigDecimal>().unwrap();
        let draft = SaveAssessmentPlanRequest {
            row_version: None,
            categories: vec![category("40.00", vec![item("10.00")])],
        };
        assert!(validate_plan_payload(&draft, &expected, false).is_ok());
        assert!(matches!(
            validate_plan_payload(&draft, &expected, true),
            Err(AppError::ValidationError(message)) if message.contains("ต้องรวมเท่ากับ")
        ));

        let overflow = SaveAssessmentPlanRequest {
            row_version: None,
            categories: vec![category("100.01", Vec::new())],
        };
        assert!(matches!(
            validate_plan_payload(&overflow, &expected, false),
            Err(AppError::ValidationError(message)) if message.contains("เกินคะแนน")
        ));
    }

    #[test]
    fn default_categories_are_virtual_and_term_reusable() {
        let categories = default_categories();
        assert_eq!(categories.len(), 4);
        assert!(categories.iter().all(|category| {
            category.id.is_none() && category.max_score == "0.00" && category.items.is_empty()
        }));
    }
}
