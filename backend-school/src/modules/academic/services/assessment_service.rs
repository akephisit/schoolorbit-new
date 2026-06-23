use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::models::assessment::{
    AssessmentCategory, AssessmentCategoryRow, AssessmentItem, AssessmentPlanDetail,
    AssessmentPlanListQuery, AssessmentPlanRow, AssessmentPlanSummary, AssessmentSettingsResponse,
    SaveAssessmentCategoryRequest, SaveAssessmentItemRequest, SaveAssessmentPlanRequest,
    UpdateAssessmentSettingsRequest,
};
use crate::modules::system::services::feature_toggle_service;
use crate::permissions::registry::codes;

pub const TEACHER_ACCESS_FEATURE_CODE: &str = "academic_assessment_teacher_access";

const VALID_CATEGORY_CODES: &[&str] = &[
    "before_midterm",
    "midterm",
    "after_midterm",
    "final",
    "custom",
];
const VALID_EXAM_MODES: &[&str] = &["none", "in_timetable", "outside_timetable", "practical"];

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
            AllocationStatus::NotStarted => "not_started",
            AllocationStatus::Complete => "complete",
            AllocationStatus::UnderAllocated => "under_allocated",
            AllocationStatus::OverAllocated => "over_allocated",
        }
    }
}

#[cfg(test)]
pub fn item_total_score(items: &[SaveAssessmentItemRequest]) -> f64 {
    items
        .iter()
        .filter(|item| item.is_active)
        .map(|item| item.max_score)
        .sum()
}

pub fn allocation_status(max_score: f64, item_total: f64) -> AllocationStatus {
    let epsilon = 0.0001;
    if max_score.abs() <= epsilon && item_total.abs() <= epsilon {
        return AllocationStatus::NotStarted;
    }
    if (max_score - item_total).abs() <= epsilon {
        return AllocationStatus::Complete;
    }
    if item_total < max_score {
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

pub fn not_configured_plan_detail(course_id: Uuid) -> AssessmentPlanDetail {
    let categories = default_categories()
        .into_iter()
        .map(|category| {
            let status = allocation_status(category.max_score, 0.0);
            AssessmentCategory {
                id: None,
                code: category.code,
                name: category.name,
                max_score: category.max_score,
                exam_mode: category.exam_mode,
                display_order: category.display_order,
                item_total_score: 0.0,
                allocation_status: status.as_str().to_string(),
                items: Vec::new(),
            }
        })
        .collect();

    AssessmentPlanDetail {
        id: None,
        classroom_course_id: course_id,
        status: "not_configured".to_string(),
        submitted_at: None,
        locked_at: None,
        categories,
    }
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
        max_score: 0.0,
        exam_mode: exam_mode.to_string(),
        display_order,
        items: Vec::new(),
    }
}

pub fn validate_plan_payload(payload: &SaveAssessmentPlanRequest) -> Result<(), AppError> {
    for category in &payload.categories {
        if category.name.trim().is_empty() {
            return Err(AppError::ValidationError("ต้องระบุชื่อหมวดคะแนน".to_string()));
        }
        ensure_non_negative_score(category.max_score)?;
        if let Some(code) = category.code.as_deref() {
            if !VALID_CATEGORY_CODES.contains(&code) {
                return Err(AppError::ValidationError("รหัสหมวดคะแนนไม่ถูกต้อง".to_string()));
            }
        }
        if !VALID_EXAM_MODES.contains(&category.exam_mode.as_str()) {
            return Err(AppError::ValidationError("รูปแบบการสอบไม่ถูกต้อง".to_string()));
        }

        for item in &category.items {
            if item.name.trim().is_empty() {
                return Err(AppError::ValidationError("ต้องระบุชื่อรายการคะแนน".to_string()));
            }
            ensure_non_negative_score(item.max_score)?;
        }
    }

    Ok(())
}

fn ensure_non_negative_score(score: f64) -> Result<(), AppError> {
    if !score.is_finite() || score < 0.0 {
        return Err(AppError::ValidationError(
            "คะแนนต้องเป็นตัวเลขที่ไม่ติดลบ".to_string(),
        ));
    }
    Ok(())
}

pub async fn list_assessment_plans(
    pool: &PgPool,
    query: &AssessmentPlanListQuery,
    assigned_instructor_id: Option<Uuid>,
) -> Result<Vec<AssessmentPlanSummary>, AppError> {
    let mut sql = String::from(
        r#"
WITH category_stats AS (
    SELECT
        plan_id,
        COUNT(*)::BIGINT AS category_count,
        COALESCE(SUM(max_score), 0)::FLOAT8 AS total_score,
        COUNT(*) FILTER (WHERE exam_mode = 'outside_timetable')::BIGINT AS outside_timetable_count,
        COUNT(*) FILTER (WHERE exam_mode = 'in_timetable')::BIGINT AS in_timetable_count
    FROM academic_assessment_categories
    GROUP BY plan_id
),
item_stats AS (
    SELECT
        c.plan_id,
        COUNT(i.id)::BIGINT AS item_count
    FROM academic_assessment_categories c
    LEFT JOIN academic_assessment_items i ON i.category_id = c.id
    GROUP BY c.plan_id
),
allocation_stats AS (
    SELECT
        c.plan_id,
        BOOL_OR(
            item_totals.item_count > 0
            AND ABS(c.max_score - item_totals.item_total_score) > 0.0001
        ) AS has_unallocated_categories
    FROM academic_assessment_categories c
    LEFT JOIN LATERAL (
        SELECT
            COUNT(i.id) AS item_count,
            COALESCE(SUM(i.max_score) FILTER (WHERE i.is_active), 0)::FLOAT8 AS item_total_score
        FROM academic_assessment_items i
        WHERE i.category_id = c.id
    ) item_totals ON TRUE
    GROUP BY c.plan_id
)
SELECT
    ap.id AS plan_id,
    cc.id AS classroom_course_id,
    cc.classroom_id,
    cc.subject_id,
    cc.academic_semester_id,
    cc.primary_instructor_id,
    COALESCE(ap.status, 'not_configured') AS status,
    s.code AS subject_code,
    s.name_th AS subject_name_th,
    s.name_en AS subject_name_en,
    cr.name AS classroom_name,
    NULLIF(CONCAT_WS(' ', u.first_name, u.last_name), '') AS instructor_name,
    COALESCE(cs.category_count, 0)::BIGINT AS category_count,
    COALESCE(ist.item_count, 0)::BIGINT AS item_count,
    COALESCE(cs.total_score, 0)::FLOAT8 AS total_score,
    COALESCE(cs.outside_timetable_count, 0)::BIGINT AS outside_timetable_count,
    COALESCE(cs.in_timetable_count, 0)::BIGINT AS in_timetable_count,
    COALESCE(als.has_unallocated_categories, FALSE) AS has_unallocated_categories
FROM classroom_courses cc
JOIN subjects s ON s.id = cc.subject_id
JOIN class_rooms cr ON cr.id = cc.classroom_id
LEFT JOIN users u ON u.id = cc.primary_instructor_id
LEFT JOIN academic_assessment_plans ap ON ap.classroom_course_id = cc.id
LEFT JOIN category_stats cs ON cs.plan_id = ap.id
LEFT JOIN item_stats ist ON ist.plan_id = ap.id
LEFT JOIN allocation_stats als ON als.plan_id = ap.id
WHERE 1 = 1"#,
    );

    let mut idx = 0u32;
    if query.academic_semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.academic_semester_id = ${idx}"));
    }
    if query.classroom_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.classroom_id = ${idx}"));
    }
    if query.instructor_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND (cc.primary_instructor_id = ${idx} OR EXISTS (SELECT 1 FROM classroom_course_instructors cci WHERE cci.classroom_course_id = cc.id AND cci.instructor_id = ${idx}))"
        ));
    }
    if query.status.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND COALESCE(ap.status, 'not_configured') = ${idx}"
        ));
    }
    if assigned_instructor_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND (cc.primary_instructor_id = ${idx} OR EXISTS (SELECT 1 FROM classroom_course_instructors cci WHERE cci.classroom_course_id = cc.id AND cci.instructor_id = ${idx}))"
        ));
    }
    sql.push_str(" ORDER BY cr.name ASC, s.code ASC, s.name_th ASC");

    let mut db_query = sqlx::query_as::<_, AssessmentPlanSummary>(&sql);
    if let Some(id) = query.academic_semester_id {
        db_query = db_query.bind(id);
    }
    if let Some(id) = query.classroom_id {
        db_query = db_query.bind(id);
    }
    if let Some(id) = query.instructor_id {
        db_query = db_query.bind(id);
    }
    if let Some(status) = &query.status {
        db_query = db_query.bind(status);
    }
    if let Some(id) = assigned_instructor_id {
        db_query = db_query.bind(id);
    }

    db_query.fetch_all(pool).await.map_err(|error| {
        tracing::error!("Failed to list assessment plans: {}", error);
        AppError::InternalServerError("Failed to list assessment plans".to_string())
    })
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
        codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
    ])
}

pub fn require_assessment_settings_manage_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
    ])
}

pub async fn require_teacher_access_enabled_for_assigned_reader(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<(), AppError> {
    if can_read_school(actor) {
        return Ok(());
    }
    require_teacher_access_enabled(pool).await
}

pub async fn require_teacher_access_enabled_for_assigned_manager(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<(), AppError> {
    if can_manage_school(actor) {
        return Ok(());
    }
    require_teacher_access_enabled(pool).await
}

async fn require_teacher_access_enabled(pool: &PgPool) -> Result<(), AppError> {
    let settings = get_assessment_settings(pool).await?;
    if settings.teacher_access_enabled {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "ยังไม่เปิดให้ครูกรอกโครงสร้างคะแนน".to_string(),
    ))
}

pub fn assigned_instructor_filter_for_list(actor: &ActorContext) -> Result<Option<Uuid>, AppError> {
    if can_read_school(actor) {
        return Ok(None);
    }
    actor.require_permission(codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED)?;
    Ok(Some(actor.user_id))
}

pub async fn require_course_read_access(
    pool: &PgPool,
    actor: &ActorContext,
    course_id: Uuid,
) -> Result<(), AppError> {
    if can_read_school(actor) {
        return Ok(());
    }
    actor.require_permission(codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED)?;
    require_assigned_course(pool, actor.user_id, course_id).await
}

pub async fn require_course_manage_access(
    pool: &PgPool,
    actor: &ActorContext,
    course_id: Uuid,
) -> Result<(), AppError> {
    if can_manage_school(actor) {
        return Ok(());
    }
    actor.require_permission(codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED)?;
    require_assigned_course(pool, actor.user_id, course_id).await
}

fn can_read_school(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
    ])
}

fn can_manage_school(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
    ])
}

async fn require_assigned_course(
    pool: &PgPool,
    actor_id: Uuid,
    course_id: Uuid,
) -> Result<(), AppError> {
    if course_is_assigned_instructor(pool, course_id, actor_id).await? {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "ไม่มีสิทธิ์จัดการรายวิชาที่ไม่ได้รับผิดชอบ".to_string(),
    ))
}

pub async fn get_plan_detail(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    ensure_course_exists(pool, course_id).await?;
    match fetch_plan_detail_optional(pool, course_id).await? {
        Some(detail) => Ok(detail),
        None => Ok(not_configured_plan_detail(course_id)),
    }
}

pub async fn save_plan(
    pool: &PgPool,
    course_id: Uuid,
    actor_id: Uuid,
    payload: SaveAssessmentPlanRequest,
) -> Result<AssessmentPlanDetail, AppError> {
    validate_plan_payload(&payload)?;
    ensure_course_exists(pool, course_id).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    let plan = ensure_plan_in_tx(&mut tx, course_id).await?;
    if plan.status == "locked" {
        return Err(AppError::ValidationError(
            "โครงสร้างคะแนนถูกล็อกแล้ว ไม่สามารถแก้ไขได้".to_string(),
        ));
    }

    let mut kept_category_ids = Vec::new();
    for category in payload.categories {
        let category_id =
            upsert_category_in_tx(&mut tx, plan.id, actor_id, category.clone()).await?;
        kept_category_ids.push(category_id);

        let mut kept_item_ids = Vec::new();
        for item in category.items {
            let item_id = upsert_item_in_tx(&mut tx, category_id, item).await?;
            kept_item_ids.push(item_id);
        }
        delete_missing_items_in_tx(&mut tx, category_id, &kept_item_ids).await?;
    }
    delete_missing_categories_in_tx(&mut tx, plan.id, &kept_category_ids).await?;

    sqlx::query(
        r#"UPDATE academic_assessment_plans
           SET status = 'draft', submitted_at = NULL, submitted_by = NULL, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(plan.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;

    tx.commit()
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    fetch_plan_detail(pool, course_id).await
}

pub async fn submit_plan(
    pool: &PgPool,
    course_id: Uuid,
    actor_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let detail = fetch_plan_detail(pool, course_id).await?;
    let plan_id = detail
        .id
        .ok_or_else(|| AppError::NotFound("ยังไม่มีโครงสร้างคะแนนรายวิชานี้".to_string()))?;
    if detail.categories.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องมีหมวดคะแนนอย่างน้อย 1 หมวดก่อนส่งโครงสร้างคะแนน".to_string(),
        ));
    }
    for category in &detail.categories {
        if !category.items.is_empty() && category.allocation_status != "complete" {
            return Err(AppError::ValidationError(format!(
                "คะแนนย่อยของหมวด {} ต้องรวมเท่ากับคะแนนเต็มหมวดก่อนส่ง",
                category.name
            )));
        }
    }

    sqlx::query(
        r#"UPDATE academic_assessment_plans
           SET status = 'submitted', submitted_at = NOW(), submitted_by = $2, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(plan_id)
    .bind(actor_id)
    .execute(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;

    fetch_plan_detail(pool, course_id).await
}

pub async fn course_is_assigned_instructor(
    pool: &PgPool,
    course_id: Uuid,
    instructor_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"SELECT EXISTS(
            SELECT 1
            FROM classroom_courses cc
            WHERE cc.id = $1
              AND (
                cc.primary_instructor_id = $2
                OR EXISTS (
                    SELECT 1
                    FROM classroom_course_instructors cci
                    WHERE cci.classroom_course_id = cc.id
                      AND cci.instructor_id = $2
                )
              )
        )"#,
    )
    .bind(course_id)
    .bind(instructor_id)
    .fetch_one(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))
}

async fn ensure_course_exists(pool: &PgPool, course_id: Uuid) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM classroom_courses WHERE id = $1)")
            .bind(course_id)
            .fetch_one(pool)
            .await
            .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    if !exists {
        return Err(AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()));
    }
    Ok(())
}

async fn ensure_plan_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    course_id: Uuid,
) -> Result<AssessmentPlanRow, AppError> {
    sqlx::query_as::<_, AssessmentPlanRow>(
        r#"INSERT INTO academic_assessment_plans (classroom_course_id)
           VALUES ($1)
           ON CONFLICT (classroom_course_id)
           DO UPDATE SET updated_at = academic_assessment_plans.updated_at
           RETURNING id, classroom_course_id, status, submitted_at, locked_at"#,
    )
    .bind(course_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))
}

async fn upsert_category_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    actor_id: Uuid,
    category: SaveAssessmentCategoryRequest,
) -> Result<Uuid, AppError> {
    if let Some(id) = category.id {
        let updated_id = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE academic_assessment_categories
               SET code = $3,
                   name = $4,
                   max_score = $5,
                   exam_mode = $6,
                   display_order = $7,
                   updated_by = $8,
                   updated_at = NOW()
               WHERE id = $1 AND plan_id = $2
               RETURNING id"#,
        )
        .bind(id)
        .bind(plan_id)
        .bind(category.code)
        .bind(category.name.trim())
        .bind(category.max_score)
        .bind(category.exam_mode)
        .bind(category.display_order)
        .bind(actor_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
        return updated_id
            .ok_or_else(|| AppError::NotFound("ไม่พบหมวดคะแนนในโครงสร้างนี้".to_string()));
    }

    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO academic_assessment_categories
           (plan_id, code, name, max_score, exam_mode, display_order, created_by, updated_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
           RETURNING id"#,
    )
    .bind(plan_id)
    .bind(category.code)
    .bind(category.name.trim())
    .bind(category.max_score)
    .bind(category.exam_mode)
    .bind(category.display_order)
    .bind(actor_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))
}

async fn upsert_item_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
    item: SaveAssessmentItemRequest,
) -> Result<Uuid, AppError> {
    if let Some(id) = item.id {
        let updated_id = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE academic_assessment_items
               SET name = $3,
                   max_score = $4,
                   display_order = $5,
                   is_active = $6,
                   updated_at = NOW()
               WHERE id = $1 AND category_id = $2
               RETURNING id"#,
        )
        .bind(id)
        .bind(category_id)
        .bind(item.name.trim())
        .bind(item.max_score)
        .bind(item.display_order)
        .bind(item.is_active)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
        return updated_id.ok_or_else(|| AppError::NotFound("ไม่พบรายการคะแนนในหมวดนี้".to_string()));
    }

    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO academic_assessment_items
           (category_id, name, max_score, display_order, is_active)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id"#,
    )
    .bind(category_id)
    .bind(item.name.trim())
    .bind(item.max_score)
    .bind(item.display_order)
    .bind(item.is_active)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))
}

async fn delete_missing_items_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    category_id: Uuid,
    kept_item_ids: &[Uuid],
) -> Result<(), AppError> {
    if kept_item_ids.is_empty() {
        sqlx::query("DELETE FROM academic_assessment_items WHERE category_id = $1")
            .bind(category_id)
            .execute(&mut **tx)
            .await
    } else {
        sqlx::query(
            "DELETE FROM academic_assessment_items WHERE category_id = $1 AND NOT (id = ANY($2))",
        )
        .bind(category_id)
        .bind(kept_item_ids)
        .execute(&mut **tx)
        .await
    }
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    Ok(())
}

async fn delete_missing_categories_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    kept_category_ids: &[Uuid],
) -> Result<(), AppError> {
    if kept_category_ids.is_empty() {
        sqlx::query("DELETE FROM academic_assessment_categories WHERE plan_id = $1")
            .bind(plan_id)
            .execute(&mut **tx)
            .await
    } else {
        sqlx::query(
            "DELETE FROM academic_assessment_categories WHERE plan_id = $1 AND NOT (id = ANY($2))",
        )
        .bind(plan_id)
        .bind(kept_category_ids)
        .execute(&mut **tx)
        .await
    }
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    Ok(())
}

async fn fetch_plan_detail(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    fetch_plan_detail_optional(pool, course_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ยังไม่มีโครงสร้างคะแนนรายวิชานี้".to_string()))
}

async fn fetch_plan_detail_optional(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<Option<AssessmentPlanDetail>, AppError> {
    let plan = sqlx::query_as::<_, AssessmentPlanRow>(
        r#"SELECT id, classroom_course_id, status, submitted_at, locked_at
           FROM academic_assessment_plans
           WHERE classroom_course_id = $1"#,
    )
    .bind(course_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    let Some(plan) = plan else {
        return Ok(None);
    };

    let category_rows = sqlx::query_as::<_, AssessmentCategoryRow>(
        r#"SELECT id, code, name, max_score, exam_mode, display_order
           FROM academic_assessment_categories
           WHERE plan_id = $1
           ORDER BY display_order ASC, created_at ASC"#,
    )
    .bind(plan.id)
    .fetch_all(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;

    let category_ids: Vec<Uuid> = category_rows.iter().map(|category| category.id).collect();
    let mut items_by_category: HashMap<Uuid, Vec<AssessmentItem>> = HashMap::new();
    if !category_ids.is_empty() {
        let items = sqlx::query_as::<_, AssessmentItem>(
            r#"SELECT id, category_id, name, max_score, display_order, is_active
               FROM academic_assessment_items
               WHERE category_id = ANY($1)
               ORDER BY display_order ASC, created_at ASC"#,
        )
        .bind(&category_ids)
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;

        for item in items {
            items_by_category
                .entry(item.category_id)
                .or_default()
                .push(item);
        }
    }

    let categories = category_rows
        .into_iter()
        .map(|row| {
            let items = items_by_category.remove(&row.id).unwrap_or_default();
            let item_total_score: f64 = items
                .iter()
                .filter(|item| item.is_active)
                .map(|item| item.max_score)
                .sum();
            let status = if items.is_empty() {
                allocation_status(row.max_score, row.max_score)
            } else {
                allocation_status(row.max_score, item_total_score)
            };
            AssessmentCategory {
                id: Some(row.id),
                code: row.code,
                name: row.name,
                max_score: row.max_score,
                exam_mode: row.exam_mode,
                display_order: row.display_order,
                item_total_score,
                allocation_status: status.as_str().to_string(),
                items,
            }
        })
        .collect();

    Ok(Some(AssessmentPlanDetail {
        id: Some(plan.id),
        classroom_course_id: plan.classroom_course_id,
        status: plan.status,
        submitted_at: plan.submitted_at,
        locked_at: plan.locked_at,
        categories,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        allocation_status, default_categories, item_total_score, not_configured_plan_detail,
        validate_plan_payload, AllocationStatus,
    };
    use crate::error::AppError;
    use crate::modules::academic::models::assessment::{
        SaveAssessmentCategoryRequest, SaveAssessmentItemRequest, SaveAssessmentPlanRequest,
    };

    fn item(name: &str, max_score: f64, display_order: i32) -> SaveAssessmentItemRequest {
        SaveAssessmentItemRequest {
            id: None,
            name: name.to_string(),
            max_score,
            display_order,
            is_active: true,
        }
    }

    fn category(
        name: &str,
        max_score: f64,
        exam_mode: &str,
        items: Vec<SaveAssessmentItemRequest>,
    ) -> SaveAssessmentCategoryRequest {
        SaveAssessmentCategoryRequest {
            id: None,
            code: Some("custom".to_string()),
            name: name.to_string(),
            max_score,
            exam_mode: exam_mode.to_string(),
            display_order: 10,
            items,
        }
    }

    #[test]
    fn calculates_item_total_from_active_items_only() {
        let items = vec![
            item("ข้อสอบกลางภาค", 20.0, 10),
            item("ใบงาน", 5.0, 20),
            SaveAssessmentItemRequest {
                is_active: false,
                ..item("งานที่ยกเลิก", 99.0, 30)
            },
        ];

        assert_eq!(item_total_score(&items), 25.0);
    }

    #[test]
    fn classifies_category_allocation_status() {
        assert_eq!(allocation_status(30.0, 30.0), AllocationStatus::Complete);
        assert_eq!(
            allocation_status(30.0, 25.0),
            AllocationStatus::UnderAllocated
        );
        assert_eq!(
            allocation_status(30.0, 35.0),
            AllocationStatus::OverAllocated
        );
        assert_eq!(allocation_status(0.0, 0.0), AllocationStatus::NotStarted);
    }

    #[test]
    fn default_categories_match_score_periods() {
        let categories = default_categories();

        let codes: Vec<&str> = categories
            .iter()
            .filter_map(|category| category.code.as_deref())
            .collect();
        assert_eq!(
            codes,
            vec!["before_midterm", "midterm", "after_midterm", "final"]
        );
        assert!(categories
            .iter()
            .all(|category| category.max_score == 0.0 && category.items.is_empty()));
        assert_eq!(categories[1].name, "กลางภาค");
        assert_eq!(categories[1].exam_mode, "in_timetable");
        assert_eq!(categories[3].exam_mode, "in_timetable");
    }

    #[test]
    fn missing_plan_detail_uses_virtual_defaults_without_persisted_ids() {
        let course_id = uuid::Uuid::new_v4();

        let detail = not_configured_plan_detail(course_id);

        assert_eq!(detail.id, None);
        assert_eq!(detail.classroom_course_id, course_id);
        assert_eq!(detail.status, "not_configured");
        assert_eq!(detail.categories.len(), 4);
        assert!(detail
            .categories
            .iter()
            .all(|category| category.id.is_none() && category.items.is_empty()));
    }

    #[test]
    fn validates_names_scores_and_exam_modes() {
        let valid = SaveAssessmentPlanRequest {
            categories: vec![category(
                "กลางภาค",
                30.0,
                "outside_timetable",
                vec![item("รายงาน", 10.0, 10), item("สอบ", 20.0, 20)],
            )],
        };
        assert!(validate_plan_payload(&valid).is_ok());

        let blank_name = SaveAssessmentPlanRequest {
            categories: vec![category(" ", 30.0, "outside_timetable", vec![])],
        };
        assert!(matches!(
            validate_plan_payload(&blank_name),
            Err(AppError::ValidationError(message)) if message.contains("ชื่อหมวดคะแนน")
        ));

        let negative_score = SaveAssessmentPlanRequest {
            categories: vec![category("กลางภาค", -1.0, "outside_timetable", vec![])],
        };
        assert!(matches!(
            validate_plan_payload(&negative_score),
            Err(AppError::ValidationError(message)) if message.contains("คะแนน")
        ));

        let invalid_exam_mode = SaveAssessmentPlanRequest {
            categories: vec![category("กลางภาค", 30.0, "weekend", vec![])],
        };
        assert!(matches!(
            validate_plan_payload(&invalid_exam_mode),
            Err(AppError::ValidationError(message)) if message.contains("รูปแบบการสอบ")
        ));
    }
}
