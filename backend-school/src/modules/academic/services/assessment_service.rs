use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::models::assessment::{
    AssessmentCategory, AssessmentCategoryRow, AssessmentItem, AssessmentPlanDetail,
    AssessmentPlanListQuery, AssessmentPlanRow, AssessmentPlanScope, AssessmentPlanSummary,
    AssessmentSettingsResponse, SaveAssessmentCategoryRequest, SaveAssessmentItemRequest,
    SaveAssessmentPlanRequest, UpdateAssessmentSettingsRequest,
};
use crate::modules::system::services::feature_toggle_service;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{self, UserResourceListAccess};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssessmentPlanListAccess {
    pub read_school: bool,
    pub sort_actor_id: Uuid,
    pub assigned_instructor_id: Option<Uuid>,
    pub subject_group_ids: Vec<Uuid>,
    pub manage_school: bool,
    pub manage_assigned_instructor_id: Option<Uuid>,
}

impl AssessmentPlanListAccess {
    fn scoped(
        sort_actor_id: Uuid,
        assigned_instructor_id: Option<Uuid>,
        subject_group_ids: Vec<Uuid>,
        manage_assigned_instructor_id: Option<Uuid>,
    ) -> Self {
        Self {
            read_school: false,
            sort_actor_id,
            assigned_instructor_id,
            subject_group_ids,
            manage_school: false,
            manage_assigned_instructor_id,
        }
    }

    pub fn is_school(&self) -> bool {
        self.read_school
    }
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

pub fn not_configured_plan_detail(scope: &AssessmentPlanScope) -> AssessmentPlanDetail {
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
                exam_duration_minutes: category.exam_duration_minutes,
                display_order: category.display_order,
                item_total_score: 0.0,
                allocation_status: status.as_str().to_string(),
                items: Vec::new(),
            }
        })
        .collect();

    AssessmentPlanDetail {
        id: None,
        classroom_course_id: scope.classroom_course_id,
        subject_id: scope.subject_id,
        academic_semester_id: scope.academic_semester_id,
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
        exam_duration_minutes: None,
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
        validate_exam_duration_for_save(category)?;

        for item in &category.items {
            if item.name.trim().is_empty() {
                return Err(AppError::ValidationError("ต้องระบุชื่อรายการคะแนน".to_string()));
            }
            ensure_non_negative_score(item.max_score)?;
        }
    }

    Ok(())
}

fn validate_exam_duration_for_save(
    category: &SaveAssessmentCategoryRequest,
) -> Result<(), AppError> {
    if let Some(duration) = category.exam_duration_minutes {
        if duration <= 0 {
            return Err(AppError::ValidationError(
                "ระยะเวลาสอบต้องมากกว่า 0 นาที".to_string(),
            ));
        }
        if !is_scheduled_exam_mode(&category.exam_mode) {
            return Err(AppError::ValidationError(
                "ระยะเวลาสอบระบุได้เฉพาะหมวดที่เป็นการสอบในตารางหรือนอกตาราง".to_string(),
            ));
        }
    }
    Ok(())
}

fn is_scheduled_exam_mode(exam_mode: &str) -> bool {
    SCHEDULED_EXAM_MODES.contains(&exam_mode)
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
    access: &AssessmentPlanListAccess,
) -> Result<Vec<AssessmentPlanSummary>, AppError> {
    let mut scoped_filters = String::new();
    let mut idx = 0u32;
    if query.academic_semester_id.is_some() {
        idx += 1;
        scoped_filters.push_str(&format!(" AND cc.academic_semester_id = ${idx}"));
    }
    if query.classroom_id.is_some() {
        idx += 1;
        scoped_filters.push_str(&format!(" AND cc.classroom_id = ${idx}"));
    }
    if query.subject_id.is_some() {
        idx += 1;
        scoped_filters.push_str(&format!(" AND cc.subject_id = ${idx}"));
    }
    if query.instructor_id.is_some() {
        idx += 1;
        scoped_filters.push_str(&format!(" AND cc.primary_instructor_id = ${idx}"));
    }

    if !access.read_school {
        let mut read_predicates = Vec::new();
        if access.assigned_instructor_id.is_some() {
            idx += 1;
            read_predicates.push(format!("cc.primary_instructor_id = ${idx}"));
        }
        if !access.subject_group_ids.is_empty() {
            idx += 1;
            read_predicates.push(format!("s.group_id = ANY(${idx})"));
        }

        if read_predicates.is_empty() {
            scoped_filters.push_str(" AND FALSE");
        } else {
            scoped_filters.push_str(" AND (");
            scoped_filters.push_str(&read_predicates.join(" OR "));
            scoped_filters.push(')');
        }
    }

    let can_manage_expression = if access.manage_school {
        "TRUE".to_string()
    } else if access.manage_assigned_instructor_id.is_some() {
        idx += 1;
        format!(
            r#"EXISTS (
        SELECT 1
        FROM classroom_courses editable_cc
        WHERE editable_cc.academic_semester_id = rc.academic_semester_id
          AND editable_cc.subject_id = rc.subject_id
          AND editable_cc.primary_instructor_id = ${idx}
    )"#
        )
    } else {
        "FALSE".to_string()
    };

    let sql_prefix = format!(
        r#"
WITH scoped_courses AS (
    SELECT
        cc.id AS classroom_course_id,
        cc.classroom_id,
        cc.subject_id,
        cc.academic_semester_id,
        cc.primary_instructor_id,
        s.code AS subject_code,
        s.name_th AS subject_name_th,
        s.name_en AS subject_name_en,
        cr.name AS classroom_name,
        CASE gl.level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
        END AS grade_level_sort,
        gl.year AS grade_year,
        cr.room_number AS classroom_room_number,
        NULLIF(CONCAT_WS(' ', u.first_name, u.last_name), '') AS instructor_name
    FROM classroom_courses cc
    JOIN subjects s ON s.id = cc.subject_id
    JOIN class_rooms cr ON cr.id = cc.classroom_id
    JOIN grade_levels gl ON gl.id = cr.grade_level_id
    LEFT JOIN users u ON u.id = cc.primary_instructor_id
    WHERE 1 = 1{scoped_filters}"#,
    );

    let mut sql = sql_prefix;
    sql.push_str(
        r#"
),
representative_courses AS (
    SELECT DISTINCT ON (academic_semester_id, subject_id)
        classroom_course_id,
        classroom_id,
        subject_id,
        academic_semester_id,
        primary_instructor_id,
        subject_code,
        subject_name_th,
        subject_name_en,
        grade_level_sort,
        grade_year,
        classroom_room_number
    FROM scoped_courses
    ORDER BY
        academic_semester_id,
        subject_id,
        grade_level_sort ASC,
        grade_year ASC,
        classroom_room_number ASC NULLS LAST,
        classroom_name ASC,
        classroom_course_id ASC
),
course_rollup AS (
    SELECT
        academic_semester_id,
        subject_id,
        COUNT(DISTINCT classroom_id)::BIGINT AS classroom_count,
        STRING_AGG(DISTINCT classroom_name, ', ' ORDER BY classroom_name) AS classroom_name,
        STRING_AGG(DISTINCT instructor_name, ', ' ORDER BY instructor_name)
            FILTER (WHERE instructor_name IS NOT NULL) AS instructor_name
    FROM scoped_courses
    GROUP BY academic_semester_id, subject_id
),
category_stats AS (
    SELECT
        plan_id,
        COUNT(*)::BIGINT AS category_count,
        COALESCE(SUM(max_score), 0)::FLOAT8 AS total_score,
        COALESCE(MAX(max_score) FILTER (WHERE code = 'before_midterm'), 0)::FLOAT8 AS before_midterm_score,
        COALESCE(MAX(max_score) FILTER (WHERE code = 'midterm'), 0)::FLOAT8 AS midterm_score,
        COALESCE(MAX(max_score) FILTER (WHERE code = 'after_midterm'), 0)::FLOAT8 AS after_midterm_score,
        COALESCE(MAX(max_score) FILTER (WHERE code = 'final'), 0)::FLOAT8 AS final_score,
        COUNT(*) FILTER (WHERE exam_mode = 'outside_timetable')::BIGINT AS outside_timetable_count,
        COUNT(*) FILTER (WHERE exam_mode = 'in_timetable')::BIGINT AS in_timetable_count,
        COALESCE(MAX(exam_mode) FILTER (WHERE code = 'midterm'), 'in_timetable') AS midterm_exam_mode,
        COALESCE(MAX(exam_mode) FILTER (WHERE code = 'final'), 'in_timetable') AS final_exam_mode,
        MAX(exam_duration_minutes) FILTER (WHERE code = 'midterm') AS midterm_exam_duration_minutes,
        MAX(exam_duration_minutes) FILTER (WHERE code = 'final') AS final_exam_duration_minutes
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
    rc.classroom_course_id,
    rc.classroom_id,
    rc.subject_id,
    rc.academic_semester_id,
    rc.primary_instructor_id,
    COALESCE(ap.status, 'not_configured') AS status,
    rc.subject_code,
    rc.subject_name_th,
    rc.subject_name_en,
    rollup.classroom_name,
    rollup.classroom_count,
    rollup.instructor_name,
    COALESCE(cs.category_count, 0)::BIGINT AS category_count,
    COALESCE(ist.item_count, 0)::BIGINT AS item_count,
    COALESCE(cs.total_score, 0)::FLOAT8 AS total_score,
    COALESCE(cs.before_midterm_score, 0)::FLOAT8 AS before_midterm_score,
    COALESCE(cs.midterm_score, 0)::FLOAT8 AS midterm_score,
    COALESCE(cs.after_midterm_score, 0)::FLOAT8 AS after_midterm_score,
    COALESCE(cs.final_score, 0)::FLOAT8 AS final_score,
    COALESCE(cs.outside_timetable_count, 0)::BIGINT AS outside_timetable_count,
    COALESCE(cs.in_timetable_count, 0)::BIGINT AS in_timetable_count,
    COALESCE(cs.midterm_exam_mode, 'in_timetable') AS midterm_exam_mode,
    COALESCE(cs.final_exam_mode, 'in_timetable') AS final_exam_mode,
    cs.midterm_exam_duration_minutes,
    cs.final_exam_duration_minutes,
    COALESCE(als.has_unallocated_categories, FALSE) AS has_unallocated_categories,
"#,
    );
    sql.push_str(&format!("    {can_manage_expression} AS can_manage"));
    sql.push_str(
        r#"
FROM representative_courses rc
JOIN course_rollup rollup
  ON rollup.academic_semester_id = rc.academic_semester_id
 AND rollup.subject_id = rc.subject_id
LEFT JOIN academic_assessment_plans ap
  ON ap.academic_semester_id = rc.academic_semester_id
 AND ap.subject_id = rc.subject_id
LEFT JOIN category_stats cs ON cs.plan_id = ap.id
LEFT JOIN item_stats ist ON ist.plan_id = ap.id
LEFT JOIN allocation_stats als ON als.plan_id = ap.id
WHERE 1 = 1"#,
    );

    if query.status.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND COALESCE(ap.status, 'not_configured') = ${idx}"
        ));
    }
    idx += 1;
    let sort_actor_idx = idx;
    sql.push_str(&format!(
        r#" ORDER BY
    CASE WHEN EXISTS (
        SELECT 1
        FROM classroom_courses sort_cc
        WHERE sort_cc.academic_semester_id = rc.academic_semester_id
          AND sort_cc.subject_id = rc.subject_id
          AND sort_cc.primary_instructor_id = ${sort_actor_idx}
    ) THEN 0 ELSE 1 END,
    LOWER(COALESCE(rollup.instructor_name, '')) ASC,
    rc.grade_level_sort ASC,
    rc.grade_year ASC,
    rc.classroom_room_number ASC NULLS LAST,
    rc.subject_code ASC NULLS LAST,
    rc.subject_name_th ASC NULLS LAST"#
    ));

    let mut db_query = sqlx::query_as::<_, AssessmentPlanSummary>(&sql);
    if let Some(id) = query.academic_semester_id {
        db_query = db_query.bind(id);
    }
    if let Some(id) = query.classroom_id {
        db_query = db_query.bind(id);
    }
    if let Some(id) = query.subject_id {
        db_query = db_query.bind(id);
    }
    if let Some(id) = query.instructor_id {
        db_query = db_query.bind(id);
    }
    if !access.read_school {
        if let Some(id) = access.assigned_instructor_id {
            db_query = db_query.bind(id);
        }
        if !access.subject_group_ids.is_empty() {
            db_query = db_query.bind(&access.subject_group_ids);
        }
    }
    if !access.manage_school {
        if let Some(id) = access.manage_assigned_instructor_id {
            db_query = db_query.bind(id);
        }
    }
    if let Some(status) = &query.status {
        db_query = db_query.bind(status);
    }
    db_query = db_query.bind(access.sort_actor_id);

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
        codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
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

pub async fn resolve_assessment_plan_list_access(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<AssessmentPlanListAccess, AppError> {
    let can_manage_assigned = can_manage_assigned(actor);
    let manage_assigned_instructor_id = can_manage_assigned.then_some(actor.user_id);

    if can_read_school(actor) {
        return Ok(AssessmentPlanListAccess {
            read_school: true,
            sort_actor_id: actor.user_id,
            assigned_instructor_id: None,
            subject_group_ids: Vec::new(),
            manage_school: can_manage_school(actor),
            manage_assigned_instructor_id,
        });
    }

    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
    ])?;

    let assigned_instructor_id = can_read_assigned(actor).then_some(actor.user_id);
    let subject_group_ids = if can_read_subject_group(actor) {
        actor_subject_group_ids(pool, actor.user_id).await?
    } else {
        Vec::new()
    };

    Ok(AssessmentPlanListAccess::scoped(
        actor.user_id,
        assigned_instructor_id,
        subject_group_ids,
        manage_assigned_instructor_id,
    ))
}

pub async fn require_course_read_access(
    pool: &PgPool,
    actor: &ActorContext,
    course_id: Uuid,
) -> Result<(), AppError> {
    if can_read_school(actor) {
        return Ok(());
    }
    if can_read_assigned(actor)
        && course_subject_plan_is_assigned_instructor(pool, course_id, actor.user_id).await?
    {
        return Ok(());
    }
    if can_read_subject_group(actor)
        && course_subject_group_is_accessible(pool, course_id, actor.user_id).await?
    {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "ไม่มีสิทธิ์ดูโครงสร้างคะแนนรายวิชานี้".to_string(),
    ))
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

fn can_read_assigned(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
    ])
}

fn can_read_subject_group(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT)
}

fn can_manage_assigned(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED)
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
    if course_subject_plan_is_assigned_instructor(pool, course_id, actor_id).await? {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "ไม่มีสิทธิ์จัดการรายวิชาที่ไม่ได้รับผิดชอบ".to_string(),
    ))
}

async fn actor_subject_group_ids(pool: &PgPool, actor_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let Some(organization_unit_ids) = resource_access_policy::accessible_organization_unit_ids(
        pool,
        UserResourceListAccess::OrganizationUnit(actor_id),
    )
    .await?
    else {
        return Ok(Vec::new());
    };

    subject_group_ids_for_organization_units(pool, &organization_unit_ids).await
}

async fn subject_group_ids_for_organization_units(
    pool: &PgPool,
    organization_unit_ids: &[Uuid],
) -> Result<Vec<Uuid>, AppError> {
    if organization_unit_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_scalar(
        r#"SELECT DISTINCT subject_group_id
           FROM organization_units
           WHERE id = ANY($1)
             AND subject_group_id IS NOT NULL
             AND is_active = true"#,
    )
    .bind(organization_unit_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch assessment subject group access: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบกลุ่มสาระได้".to_string())
    })
}

async fn course_subject_group_is_accessible(
    pool: &PgPool,
    course_id: Uuid,
    actor_id: Uuid,
) -> Result<bool, AppError> {
    let subject_group_id = course_subject_group_id(pool, course_id).await?;
    let Some(subject_group_id) = subject_group_id else {
        return Ok(false);
    };
    let subject_group_ids = actor_subject_group_ids(pool, actor_id).await?;
    Ok(subject_group_ids.contains(&subject_group_id))
}

pub async fn get_plan_detail(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let scope = resolve_plan_scope(pool, course_id).await?;
    match fetch_plan_detail_optional(pool, &scope).await? {
        Some(detail) => Ok(detail),
        None => Ok(not_configured_plan_detail(&scope)),
    }
}

pub async fn save_plan(
    pool: &PgPool,
    course_id: Uuid,
    actor_id: Uuid,
    payload: SaveAssessmentPlanRequest,
) -> Result<AssessmentPlanDetail, AppError> {
    validate_plan_payload(&payload)?;
    let scope = resolve_plan_scope(pool, course_id).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    let plan = ensure_plan_in_tx(&mut tx, &scope).await?;
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
           SET status = CASE WHEN status = 'submitted' THEN status ELSE 'draft' END,
               submitted_at = CASE WHEN status = 'submitted' THEN submitted_at ELSE NULL END,
               submitted_by = CASE WHEN status = 'submitted' THEN submitted_by ELSE NULL END,
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(plan.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;

    tx.commit()
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    fetch_plan_detail(pool, &scope).await
}

pub async fn submit_plan(
    pool: &PgPool,
    course_id: Uuid,
    actor_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let scope = resolve_plan_scope(pool, course_id).await?;
    let detail = fetch_plan_detail(pool, &scope).await?;
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
        if is_scheduled_exam_mode(&category.exam_mode) && category.exam_duration_minutes.is_none() {
            return Err(AppError::ValidationError(format!(
                "ต้องระบุระยะเวลาสอบของหมวด {} ก่อนส่งโครงสร้างคะแนน",
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

    fetch_plan_detail(pool, &scope).await
}

async fn course_subject_plan_is_assigned_instructor(
    pool: &PgPool,
    course_id: Uuid,
    instructor_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"WITH target_course AS (
            SELECT subject_id, academic_semester_id
            FROM classroom_courses
            WHERE id = $1
        )
        SELECT EXISTS(
            SELECT 1
            FROM classroom_courses cc
            JOIN target_course target
              ON target.subject_id = cc.subject_id
             AND target.academic_semester_id = cc.academic_semester_id
            WHERE cc.primary_instructor_id = $2
        )"#,
    )
    .bind(course_id)
    .bind(instructor_id)
    .fetch_one(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))
}

async fn course_subject_group_id(pool: &PgPool, course_id: Uuid) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"SELECT s.group_id
           FROM classroom_courses cc
           JOIN subjects s ON s.id = cc.subject_id
           WHERE cc.id = $1"#,
    )
    .bind(course_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))
    .map(|value| value.flatten())
}

async fn resolve_plan_scope(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<AssessmentPlanScope, AppError> {
    sqlx::query_as::<_, AssessmentPlanScope>(
        r#"SELECT
               id AS classroom_course_id,
               subject_id,
               academic_semester_id
           FROM classroom_courses
           WHERE id = $1"#,
    )
    .bind(course_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))
}

async fn ensure_plan_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    scope: &AssessmentPlanScope,
) -> Result<AssessmentPlanRow, AppError> {
    sqlx::query_as::<_, AssessmentPlanRow>(
        r#"INSERT INTO academic_assessment_plans
               (classroom_course_id, academic_semester_id, subject_id)
           VALUES ($1, $2, $3)
           ON CONFLICT (academic_semester_id, subject_id)
           DO UPDATE SET
               classroom_course_id = COALESCE(
                   academic_assessment_plans.classroom_course_id,
                   EXCLUDED.classroom_course_id
               ),
               updated_at = academic_assessment_plans.updated_at
           RETURNING
               id,
               classroom_course_id,
               subject_id,
               academic_semester_id,
               status,
               submitted_at,
               locked_at"#,
    )
    .bind(scope.classroom_course_id)
    .bind(scope.academic_semester_id)
    .bind(scope.subject_id)
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
                   exam_duration_minutes = $7,
                   display_order = $8,
                   updated_by = $9,
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
        .bind(category.exam_duration_minutes)
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
           (
               plan_id,
               code,
               name,
               max_score,
               exam_mode,
               exam_duration_minutes,
               display_order,
               created_by,
               updated_by
           )
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
           RETURNING id"#,
    )
    .bind(plan_id)
    .bind(category.code)
    .bind(category.name.trim())
    .bind(category.max_score)
    .bind(category.exam_mode)
    .bind(category.exam_duration_minutes)
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
    scope: &AssessmentPlanScope,
) -> Result<AssessmentPlanDetail, AppError> {
    fetch_plan_detail_optional(pool, scope)
        .await?
        .ok_or_else(|| AppError::NotFound("ยังไม่มีโครงสร้างคะแนนรายวิชานี้".to_string()))
}

async fn fetch_plan_detail_optional(
    pool: &PgPool,
    scope: &AssessmentPlanScope,
) -> Result<Option<AssessmentPlanDetail>, AppError> {
    let plan = sqlx::query_as::<_, AssessmentPlanRow>(
        r#"SELECT
               id,
               classroom_course_id,
               subject_id,
               academic_semester_id,
               status,
               submitted_at,
               locked_at
           FROM academic_assessment_plans
           WHERE academic_semester_id = $1
             AND subject_id = $2"#,
    )
    .bind(scope.academic_semester_id)
    .bind(scope.subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    let Some(plan) = plan else {
        return Ok(None);
    };

    let category_rows = sqlx::query_as::<_, AssessmentCategoryRow>(
        r#"SELECT
               id,
               code,
               name,
               max_score,
               exam_mode,
               exam_duration_minutes,
               display_order
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
                exam_duration_minutes: row.exam_duration_minutes,
                display_order: row.display_order,
                item_total_score,
                allocation_status: status.as_str().to_string(),
                items,
            }
        })
        .collect();

    Ok(Some(AssessmentPlanDetail {
        id: Some(plan.id),
        classroom_course_id: plan
            .classroom_course_id
            .unwrap_or(scope.classroom_course_id),
        subject_id: plan.subject_id,
        academic_semester_id: plan.academic_semester_id,
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
        AssessmentPlanScope, SaveAssessmentCategoryRequest, SaveAssessmentItemRequest,
        SaveAssessmentPlanRequest,
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
            exam_duration_minutes: None,
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
        let scope = AssessmentPlanScope {
            classroom_course_id: uuid::Uuid::new_v4(),
            subject_id: uuid::Uuid::new_v4(),
            academic_semester_id: uuid::Uuid::new_v4(),
        };

        let detail = not_configured_plan_detail(&scope);

        assert_eq!(detail.id, None);
        assert_eq!(detail.classroom_course_id, scope.classroom_course_id);
        assert_eq!(detail.subject_id, scope.subject_id);
        assert_eq!(detail.academic_semester_id, scope.academic_semester_id);
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

    #[test]
    fn validates_exam_duration_only_for_scheduled_exam_categories() {
        let mut scheduled_exam = category("กลางภาค", 30.0, "in_timetable", vec![]);
        scheduled_exam.exam_duration_minutes = Some(60);
        assert!(validate_plan_payload(&SaveAssessmentPlanRequest {
            categories: vec![scheduled_exam],
        })
        .is_ok());

        let mut missing_duration_draft = category("กลางภาค", 30.0, "in_timetable", vec![]);
        missing_duration_draft.exam_duration_minutes = None;
        assert!(validate_plan_payload(&SaveAssessmentPlanRequest {
            categories: vec![missing_duration_draft],
        })
        .is_ok());

        let mut zero_duration = category("กลางภาค", 30.0, "in_timetable", vec![]);
        zero_duration.exam_duration_minutes = Some(0);
        assert!(matches!(
            validate_plan_payload(&SaveAssessmentPlanRequest {
                categories: vec![zero_duration],
            }),
            Err(AppError::ValidationError(message)) if message.contains("ระยะเวลาสอบ")
        ));

        let mut non_exam_duration = category("ก่อนกลางภาค", 10.0, "none", vec![]);
        non_exam_duration.exam_duration_minutes = Some(30);
        assert!(matches!(
            validate_plan_payload(&SaveAssessmentPlanRequest {
                categories: vec![non_exam_duration],
            }),
            Err(AppError::ValidationError(message)) if message.contains("เฉพาะหมวด")
        ));
    }
}
