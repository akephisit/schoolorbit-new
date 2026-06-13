use crate::error::AppError;
use crate::modules::academic::models::study_plans::*;
use chrono::{DateTime, Utc};
use sqlx::{types::Json, FromRow, PgPool};
use uuid::Uuid;

fn grade_level_ids_json(ids: Option<&[Uuid]>) -> Option<Json<&[Uuid]>> {
    ids.map(Json)
}

fn grade_level_ids_from_jsonb(value: Option<Json<Vec<Uuid>>>) -> Option<Vec<Uuid>> {
    value.map(|Json(ids)| ids)
}

fn study_plan_subject_display_order(display_order: Option<i32>) -> i32 {
    display_order.unwrap_or(0)
}

fn validate_catalog_instructor_role(role: &str) -> Result<(), AppError> {
    if role == "primary" || role == "secondary" {
        return Ok(());
    }

    Err(AppError::BadRequest(
        "role must be 'primary' or 'secondary'".to_string(),
    ))
}

#[derive(Debug, FromRow)]
struct StudyPlanRow {
    id: Uuid,
    code: String,
    name_th: String,
    name_en: Option<String>,
    description: Option<String>,
    grade_level_ids: Option<Json<Vec<Uuid>>>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StudyPlanVersionActivityRow {
    id: Uuid,
    study_plan_version_id: Uuid,
    activity_catalog_id: Uuid,
    grade_level_id: Uuid,
    term: Option<String>,
    display_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[sqlx(default)]
    catalog_name: Option<String>,
    #[sqlx(default)]
    catalog_activity_type: Option<String>,
    #[sqlx(default)]
    catalog_description: Option<String>,
    #[sqlx(default)]
    catalog_periods_per_week: Option<i32>,
    #[sqlx(default)]
    catalog_scheduling_mode: Option<String>,
    #[sqlx(default)]
    catalog_term: Option<String>,
    #[sqlx(default)]
    catalog_grade_level_ids: Option<Json<Vec<Uuid>>>,
}

#[derive(Debug, FromRow)]
struct ActivityCatalogRow {
    id: Uuid,
    name: String,
    start_academic_year_id: Uuid,
    activity_type: String,
    description: Option<String>,
    periods_per_week: i32,
    scheduling_mode: String,
    is_active: bool,
    term: Option<String>,
    grade_level_ids: Option<Json<Vec<Uuid>>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn study_plan_from_row(row: StudyPlanRow) -> Result<StudyPlan, AppError> {
    Ok(StudyPlan {
        id: row.id,
        code: row.code,
        name_th: row.name_th,
        name_en: row.name_en,
        description: row.description,
        grade_level_ids: grade_level_ids_from_jsonb(row.grade_level_ids),
        is_active: row.is_active,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn plan_activity_from_row(
    row: StudyPlanVersionActivityRow,
) -> Result<StudyPlanVersionActivity, AppError> {
    Ok(StudyPlanVersionActivity {
        id: row.id,
        study_plan_version_id: row.study_plan_version_id,
        activity_catalog_id: row.activity_catalog_id,
        grade_level_id: row.grade_level_id,
        term: row.term,
        display_order: row.display_order,
        created_at: row.created_at,
        updated_at: row.updated_at,
        catalog_name: row.catalog_name,
        catalog_activity_type: row.catalog_activity_type,
        catalog_description: row.catalog_description,
        catalog_periods_per_week: row.catalog_periods_per_week,
        catalog_scheduling_mode: row.catalog_scheduling_mode,
        catalog_term: row.catalog_term,
        catalog_grade_level_ids: grade_level_ids_from_jsonb(row.catalog_grade_level_ids),
    })
}

fn activity_catalog_from_row(row: ActivityCatalogRow) -> Result<ActivityCatalog, AppError> {
    Ok(ActivityCatalog {
        id: row.id,
        name: row.name,
        start_academic_year_id: row.start_academic_year_id,
        activity_type: row.activity_type,
        description: row.description,
        periods_per_week: row.periods_per_week,
        scheduling_mode: row.scheduling_mode,
        is_active: row.is_active,
        term: row.term,
        grade_level_ids: grade_level_ids_from_jsonb(row.grade_level_ids),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

// ============================================
// Study Plans CRUD
// ============================================

pub async fn list_plans(pool: &PgPool, query: StudyPlanQuery) -> Result<Vec<StudyPlan>, AppError> {
    let mut sql = String::from("SELECT * FROM study_plans WHERE 1=1");
    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND is_active = true");
    }
    sql.push_str(" ORDER BY code");
    let rows = sqlx::query_as::<_, StudyPlanRow>(&sql)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;

    rows.into_iter().map(study_plan_from_row).collect()
}

pub async fn get_plan(pool: &PgPool, plan_id: Uuid) -> Result<StudyPlan, AppError> {
    let row = sqlx::query_as::<_, StudyPlanRow>("SELECT * FROM study_plans WHERE id = $1")
        .bind(plan_id)
        .fetch_one(pool)
        .await
        .map_err(AppError::from)?;

    study_plan_from_row(row)
}

pub async fn create_plan(
    pool: &PgPool,
    req: CreateStudyPlanRequest,
) -> Result<StudyPlan, AppError> {
    let grade_ids = grade_level_ids_json(req.grade_level_ids.as_deref());
    let row = sqlx::query_as::<_, StudyPlanRow>(
        "INSERT INTO study_plans (code, name_th, name_en, description, grade_level_ids)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(&req.code)
    .bind(&req.name_th)
    .bind(&req.name_en)
    .bind(&req.description)
    .bind(grade_ids)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;

    study_plan_from_row(row)
}

pub async fn update_plan(
    pool: &PgPool,
    plan_id: Uuid,
    req: UpdateStudyPlanRequest,
) -> Result<StudyPlan, AppError> {
    let mut updates = Vec::new();
    let mut param_count = 1;
    if req.code.is_some() {
        updates.push(format!("code = ${}", param_count));
        param_count += 1;
    }
    if req.name_th.is_some() {
        updates.push(format!("name_th = ${}", param_count));
        param_count += 1;
    }
    if req.name_en.is_some() {
        updates.push(format!("name_en = ${}", param_count));
        param_count += 1;
    }
    if req.description.is_some() {
        updates.push(format!("description = ${}", param_count));
        param_count += 1;
    }
    if req.grade_level_ids.is_some() {
        updates.push(format!("grade_level_ids = ${}", param_count));
        param_count += 1;
    }
    if req.is_active.is_some() {
        updates.push(format!("is_active = ${}", param_count));
        param_count += 1;
    }
    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let sql = format!(
        "UPDATE study_plans SET {} WHERE id = ${} RETURNING *",
        updates.join(", "),
        param_count
    );
    let mut q = sqlx::query_as::<_, StudyPlanRow>(&sql);
    if let Some(ref v) = req.code {
        q = q.bind(v);
    }
    if let Some(ref v) = req.name_th {
        q = q.bind(v);
    }
    if let Some(ref v) = req.name_en {
        q = q.bind(v);
    }
    if let Some(ref v) = req.description {
        q = q.bind(v);
    }
    if req.grade_level_ids.is_some() {
        q = q.bind(grade_level_ids_json(req.grade_level_ids.as_deref()));
    }
    if let Some(v) = req.is_active {
        q = q.bind(v);
    }
    q = q.bind(plan_id);
    let row = q.fetch_one(pool).await.map_err(AppError::from)?;
    study_plan_from_row(row)
}

pub async fn delete_plan(pool: &PgPool, plan_id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM study_plans WHERE id = $1")
        .bind(plan_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ============================================
// Study Plan Versions
// ============================================

pub async fn list_versions(
    pool: &PgPool,
    query: StudyPlanVersionQuery,
) -> Result<Vec<StudyPlanVersion>, AppError> {
    let mut sql = String::from(
        "SELECT spv.*, sp.name_th as study_plan_name_th, ay.name as start_year_name
         FROM study_plan_versions spv
         LEFT JOIN study_plans sp ON sp.id = spv.study_plan_id
         LEFT JOIN academic_years ay ON ay.id = spv.start_academic_year_id
         WHERE 1=1",
    );
    let mut idx = 0u32;
    if query.study_plan_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND spv.study_plan_id = ${idx}"));
    }
    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND spv.is_active = true");
    }
    sql.push_str(" ORDER BY spv.created_at DESC");

    let mut q = sqlx::query_as::<_, StudyPlanVersion>(&sql);
    if let Some(v) = query.study_plan_id {
        q = q.bind(v);
    }
    Ok(q.fetch_all(pool).await.unwrap_or_default())
}

pub async fn get_version(pool: &PgPool, version_id: Uuid) -> Result<StudyPlanVersion, AppError> {
    sqlx::query_as::<_, StudyPlanVersion>(
        "SELECT spv.*, sp.name_th as study_plan_name_th, ay.name as start_year_name
         FROM study_plan_versions spv
         LEFT JOIN study_plans sp ON sp.id = spv.study_plan_id
         LEFT JOIN academic_years ay ON ay.id = spv.start_academic_year_id
         WHERE spv.id = $1",
    )
    .bind(version_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create_version(
    pool: &PgPool,
    req: CreateStudyPlanVersionRequest,
) -> Result<StudyPlanVersion, AppError> {
    sqlx::query_as::<_, StudyPlanVersion>(
        "INSERT INTO study_plan_versions
         (study_plan_id, version_name, start_academic_year_id, end_academic_year_id, description)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(req.study_plan_id)
    .bind(&req.version_name)
    .bind(req.start_academic_year_id)
    .bind(req.end_academic_year_id)
    .bind(&req.description)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_version(
    pool: &PgPool,
    version_id: Uuid,
    req: UpdateStudyPlanVersionRequest,
) -> Result<StudyPlanVersion, AppError> {
    sqlx::query_as::<_, StudyPlanVersion>(
        "UPDATE study_plan_versions SET
            version_name = COALESCE($1, version_name),
            start_academic_year_id = COALESCE($2, start_academic_year_id),
            end_academic_year_id = COALESCE($3, end_academic_year_id),
            description = COALESCE($4, description),
            is_active = COALESCE($5, is_active)
         WHERE id = $6 RETURNING *",
    )
    .bind(&req.version_name)
    .bind(req.start_academic_year_id)
    .bind(req.end_academic_year_id)
    .bind(&req.description)
    .bind(req.is_active)
    .bind(version_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn delete_version(pool: &PgPool, version_id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM study_plan_versions WHERE id = $1")
        .bind(version_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ============================================
// Study Plan Subjects
// ============================================

pub async fn list_plan_subjects(
    pool: &PgPool,
    query: StudyPlanSubjectQuery,
) -> Result<Vec<StudyPlanSubject>, AppError> {
    let mut sql = String::from(
        "SELECT sps.id, sps.study_plan_version_id, sps.grade_level_id, sps.term,
                sps.subject_id, s.code as subject_code, sps.display_order, sps.metadata,
                sps.created_at, sps.updated_at,
                s.name_th as subject_name_th, s.name_en as subject_name_en,
                s.credit as subject_credit, s.type as subject_type,
                s.hours_per_semester as subject_hours,
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name
         FROM study_plan_subjects sps
         LEFT JOIN subjects s ON s.id = sps.subject_id
         LEFT JOIN grade_levels gl ON gl.id = sps.grade_level_id
         WHERE 1=1",
    );
    let mut idx = 0u32;
    if query.study_plan_version_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND sps.study_plan_version_id = ${idx}"));
    }
    if query.grade_level_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND sps.grade_level_id = ${idx}"));
    }
    if query.term.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND sps.term = ${idx}"));
    }
    sql.push_str(" ORDER BY sps.display_order, s.code");

    let mut q = sqlx::query_as::<_, StudyPlanSubject>(&sql);
    if let Some(v) = query.study_plan_version_id {
        q = q.bind(v);
    }
    if let Some(v) = query.grade_level_id {
        q = q.bind(v);
    }
    if let Some(ref v) = query.term {
        q = q.bind(v);
    }
    Ok(q.fetch_all(pool).await.unwrap_or_default())
}

pub async fn add_subjects_to_version(
    pool: &PgPool,
    version_id: Uuid,
    req: AddSubjectsToVersionRequest,
) -> Result<usize, AppError> {
    let mut tx = pool.begin().await?;
    for subject in &req.subjects {
        sqlx::query(
            "INSERT INTO study_plan_subjects
             (study_plan_version_id, grade_level_id, term, subject_id, display_order)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (study_plan_version_id, grade_level_id, term, subject_id) DO NOTHING",
        )
        .bind(version_id)
        .bind(subject.grade_level_id)
        .bind(&subject.term)
        .bind(subject.subject_id)
        .bind(study_plan_subject_display_order(subject.display_order))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(req.subjects.len())
}

pub async fn delete_plan_subject(pool: &PgPool, sps_id: Uuid) -> Result<(), AppError> {
    let context: Option<(Uuid, Uuid, String, Uuid)> = sqlx::query_as(
        "SELECT study_plan_version_id, grade_level_id, term, subject_id
         FROM study_plan_subjects WHERE id = $1",
    )
    .bind(sps_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sqlx::query("DELETE FROM study_plan_subjects WHERE id = $1")
        .bind(sps_id)
        .execute(&mut *tx)
        .await?;

    if let Some((plan_id, grade_id, term, subject_id)) = context {
        sqlx::query(
            "DELETE FROM classroom_courses cc
             USING class_rooms cr, academic_semesters sem
             WHERE cc.classroom_id = cr.id
               AND cc.academic_semester_id = sem.id
               AND cr.study_plan_version_id = $1
               AND cr.grade_level_id = $2
               AND sem.term = $3
               AND cc.subject_id = $4
               AND sem.end_date >= CURRENT_DATE",
        )
        .bind(plan_id)
        .bind(grade_id)
        .bind(&term)
        .bind(subject_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

// ============================================
// Generate Courses from Plan
// ============================================

pub struct GenerateCoursesResult {
    pub courses_created: i32,
    pub courses_skipped: i32,
    pub activities_created: i32,
    pub activities_skipped: i32,
}

pub async fn generate_courses_from_plan(
    pool: &PgPool,
    req: GenerateCoursesFromPlanRequest,
    user_id: Option<Uuid>,
) -> Result<GenerateCoursesResult, AppError> {
    let mut tx = pool.begin().await?;

    let classroom: (Option<Uuid>, Uuid) = sqlx::query_as(
        "SELECT study_plan_version_id, grade_level_id FROM class_rooms WHERE id = $1",
    )
    .bind(req.classroom_id)
    .fetch_one(&mut *tx)
    .await?;

    let plan_version_id = classroom.0.ok_or_else(|| {
        AppError::BadRequest("Classroom does not have a study plan assigned".to_string())
    })?;
    let grade_level_id = classroom.1;

    let (semester_term, target_academic_year_id): (String, Uuid) =
        sqlx::query_as("SELECT term, academic_year_id FROM academic_semesters WHERE id = $1")
            .bind(req.academic_semester_id)
            .fetch_one(&mut *tx)
            .await?;

    let counts: (i64, i64) = sqlx::query_as(
        r#"
        WITH plan_subjects AS (
            SELECT sps.subject_id, s.default_instructor_id
            FROM study_plan_subjects sps
            JOIN subjects s ON s.id = sps.subject_id
            WHERE sps.study_plan_version_id = $1
              AND sps.grade_level_id = $2
              AND sps.term = $3
        ),
        inserted AS (
            INSERT INTO classroom_courses
                (classroom_id, subject_id, academic_semester_id, settings, primary_instructor_id)
            SELECT $4, ps.subject_id, $5, '{}'::jsonb,
                COALESCE(
                    (SELECT sdi.instructor_id FROM subject_default_instructors sdi
                     WHERE sdi.subject_id = ps.subject_id AND sdi.role = 'primary' LIMIT 1),
                    ps.default_instructor_id
                )
            FROM plan_subjects ps
            ON CONFLICT (classroom_id, subject_id, academic_semester_id) DO NOTHING
            RETURNING id, subject_id
        ),
        sec_copy AS (
            INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
            SELECT i.id, sdi.instructor_id, sdi.role
            FROM inserted i
            JOIN subject_default_instructors sdi
              ON sdi.subject_id = i.subject_id AND sdi.role = 'secondary'
            ON CONFLICT (classroom_course_id, instructor_id) DO NOTHING
            RETURNING 1
        )
        SELECT
            (SELECT COUNT(*) FROM plan_subjects) AS total,
            (SELECT COUNT(*) FROM inserted) AS added
        "#,
    )
    .bind(plan_version_id)
    .bind(grade_level_id)
    .bind(&semester_term)
    .bind(req.classroom_id)
    .bind(req.academic_semester_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("generate_courses_from_plan failed: {}", e);
        AppError::InternalServerError("Failed to generate courses".to_string())
    })?;

    let added = counts.1 as i32;
    let skipped = (counts.0 - counts.1) as i32;
    let _ = req.skip_existing;

    let activity_counts: (i64, i64) = sqlx::query_as(
        r#"
        WITH plan_acts AS (
            SELECT DISTINCT sva.activity_catalog_id AS src_catalog_id
            FROM study_plan_version_activities sva
            JOIN class_rooms cr ON cr.id = $5
            WHERE sva.study_plan_version_id = $1
              AND sva.grade_level_id = cr.grade_level_id
        ),
        resolved AS (
            SELECT DISTINCT ON (src.name)
                src.name AS catalog_name,
                latest.id AS catalog_id,
                latest.grade_level_ids
            FROM plan_acts pa
            JOIN activity_catalog src ON src.id = pa.src_catalog_id
            JOIN activity_catalog latest ON latest.name = src.name
            JOIN academic_years ay ON ay.id = latest.start_academic_year_id
            JOIN academic_years ay_target ON ay_target.id = $4
            WHERE latest.is_active = true
              AND ay.year <= ay_target.year
            ORDER BY src.name, ay.year DESC
        ),
        inserted_slots AS (
            INSERT INTO activity_slots
                (activity_catalog_id, semester_id, registration_type, created_by)
            SELECT r.catalog_id, $2, 'assigned', $3
            FROM resolved r
            ON CONFLICT (activity_catalog_id, semester_id) DO NOTHING
            RETURNING id, activity_catalog_id
        ),
        existing_slots AS (
            SELECT s.id, s.activity_catalog_id
            FROM activity_slots s
            JOIN resolved r ON r.catalog_id = s.activity_catalog_id
            WHERE s.semester_id = $2
        ),
        all_slots AS (
            SELECT id, activity_catalog_id FROM inserted_slots
            UNION
            SELECT id, activity_catalog_id FROM existing_slots
        ),
        classroom_grade AS (
            SELECT cr.grade_level_id::text AS grade_str
            FROM class_rooms cr WHERE cr.id = $5
        ),
        inserted_junction AS (
            INSERT INTO activity_slot_classrooms (slot_id, classroom_id)
            SELECT s.id, $5
            FROM all_slots s
            JOIN resolved r ON r.catalog_id = s.activity_catalog_id
            LEFT JOIN classroom_grade cg ON true
            WHERE r.grade_level_ids IS NULL
               OR r.grade_level_ids ? cg.grade_str
            ON CONFLICT (slot_id, classroom_id) DO NOTHING
            RETURNING 1
        ),
        copy_sync_instructors AS (
            INSERT INTO activity_slot_instructors (slot_id, user_id)
            SELECT s.id, acdi.instructor_id
            FROM all_slots s
            JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
            JOIN activity_catalog_default_instructors acdi ON acdi.catalog_id = s.activity_catalog_id
            WHERE ac.scheduling_mode = 'synchronized'
            ON CONFLICT (slot_id, user_id) DO NOTHING
            RETURNING 1
        ),
        copy_independent_instructor AS (
            INSERT INTO activity_slot_classroom_assignments (slot_id, classroom_id, instructor_id)
            SELECT s.id, $5, acdi.instructor_id
            FROM all_slots s
            JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
            JOIN activity_catalog_default_instructors acdi
              ON acdi.catalog_id = s.activity_catalog_id AND acdi.role = 'primary'
            WHERE ac.scheduling_mode = 'independent'
            ON CONFLICT (slot_id, classroom_id) DO NOTHING
            RETURNING 1
        )
        SELECT
            (SELECT COUNT(*) FROM resolved) AS total,
            (SELECT COUNT(*) FROM inserted_junction) AS added
        "#
    )
    .bind(plan_version_id).bind(req.academic_semester_id).bind(user_id)
    .bind(target_academic_year_id).bind(req.classroom_id)
    .fetch_one(&mut *tx).await
    .unwrap_or((0, 0));

    let activities_created = activity_counts.1 as i32;
    let activities_skipped = (activity_counts.0 - activity_counts.1) as i32;

    tx.commit().await?;

    Ok(GenerateCoursesResult {
        courses_created: added,
        courses_skipped: skipped,
        activities_created,
        activities_skipped,
    })
}

// ============================================
// Study Plan Version Activities
// ============================================

pub async fn list_plan_activities(
    pool: &PgPool,
    version_id: Uuid,
) -> Result<Vec<StudyPlanVersionActivity>, AppError> {
    let rows = sqlx::query_as::<_, StudyPlanVersionActivityRow>(
        "SELECT sva.*,
                ac.name AS catalog_name,
                ac.activity_type AS catalog_activity_type,
                ac.description AS catalog_description,
                ac.periods_per_week AS catalog_periods_per_week,
                ac.scheduling_mode AS catalog_scheduling_mode,
                ac.term AS catalog_term,
                ac.grade_level_ids AS catalog_grade_level_ids
         FROM study_plan_version_activities sva
         JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
         WHERE sva.study_plan_version_id = $1
         ORDER BY sva.display_order, ac.name",
    )
    .bind(version_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    rows.into_iter().map(plan_activity_from_row).collect()
}

pub async fn add_plan_activity(
    pool: &PgPool,
    version_id: Uuid,
    req: CreatePlanActivityRequest,
) -> Result<StudyPlanVersionActivity, AppError> {
    let row = sqlx::query_as::<_, StudyPlanVersionActivityRow>(
        "INSERT INTO study_plan_version_activities
         (study_plan_version_id, activity_catalog_id, grade_level_id, term, display_order)
         SELECT $1, ac.id, $5, COALESCE($4, ac.term), COALESCE($3, 0)
         FROM activity_catalog ac WHERE ac.id = $2
         ON CONFLICT (study_plan_version_id, grade_level_id, term, activity_catalog_id) DO NOTHING
         RETURNING *",
    )
    .bind(version_id)
    .bind(req.activity_catalog_id)
    .bind(req.display_order)
    .bind(&req.term)
    .bind(req.grade_level_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::BadRequest("กิจกรรมนี้อยู่ในหลักสูตรสำหรับชั้น+เทอมนี้แล้ว".to_string()))?;

    plan_activity_from_row(row)
}

pub async fn update_plan_activity(
    pool: &PgPool,
    id: Uuid,
    req: UpdatePlanActivityRequest,
) -> Result<StudyPlanVersionActivity, AppError> {
    let row = sqlx::query_as::<_, StudyPlanVersionActivityRow>(
        "UPDATE study_plan_version_activities SET
            display_order = COALESCE($2, display_order),
            term = $3,
            updated_at = NOW()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(req.display_order)
    .bind(&req.term)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::NotFound(e.to_string()))?;

    plan_activity_from_row(row)
}

pub async fn delete_plan_activity(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let context: Option<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT ac.name, sva.study_plan_version_id, sva.grade_level_id
         FROM study_plan_version_activities sva
         JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
         WHERE sva.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sqlx::query("DELETE FROM study_plan_version_activities WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if let Some((catalog_name, plan_id, grade_id)) = context {
        sqlx::query(
            "DELETE FROM activity_slot_classrooms asc_row
             USING activity_slots s, activity_catalog ac, academic_semesters sem, class_rooms cr
             WHERE asc_row.slot_id = s.id
               AND s.activity_catalog_id = ac.id
               AND s.semester_id = sem.id
               AND asc_row.classroom_id = cr.id
               AND ac.name = $1
               AND cr.study_plan_version_id = $2
               AND cr.grade_level_id = $3
               AND sem.end_date >= CURRENT_DATE",
        )
        .bind(&catalog_name)
        .bind(plan_id)
        .bind(grade_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn generate_activities_from_plan(
    pool: &PgPool,
    req: GenerateActivitiesFromPlanRequest,
    user_id: Option<Uuid>,
) -> Result<(i32, i32, i64), AppError> {
    let counts: (i64, i64) = sqlx::query_as(
        r#"
        WITH target_year AS (
            SELECT academic_year_id FROM academic_semesters WHERE id = $2
        ),
        plan_acts AS (
            SELECT DISTINCT sva.activity_catalog_id AS src_catalog_id
            FROM study_plan_version_activities sva
            WHERE sva.study_plan_version_id = $1
        ),
        resolved AS (
            SELECT DISTINCT ON (src.name)
                src.name AS catalog_name,
                latest.id AS catalog_id,
                latest.grade_level_ids
            FROM plan_acts pa
            JOIN activity_catalog src ON src.id = pa.src_catalog_id
            JOIN activity_catalog latest ON latest.name = src.name
            JOIN academic_years ay ON ay.id = latest.start_academic_year_id
            JOIN target_year ty ON true
            JOIN academic_years ay_target ON ay_target.id = ty.academic_year_id
            WHERE latest.is_active = true
              AND ay.year <= ay_target.year
            ORDER BY src.name, ay.year DESC
        ),
        inserted_slots AS (
            INSERT INTO activity_slots
                (activity_catalog_id, semester_id, registration_type, created_by)
            SELECT r.catalog_id, $2, 'assigned', $3
            FROM resolved r
            ON CONFLICT (activity_catalog_id, semester_id) DO NOTHING
            RETURNING id, activity_catalog_id
        ),
        existing_slots AS (
            SELECT s.id, s.activity_catalog_id
            FROM activity_slots s
            JOIN resolved r ON r.catalog_id = s.activity_catalog_id
            WHERE s.semester_id = $2
        ),
        all_slots AS (
            SELECT id, activity_catalog_id FROM inserted_slots
            UNION
            SELECT id, activity_catalog_id FROM existing_slots
        ),
        target_classrooms AS (
            SELECT cr.id, cr.grade_level_id::text AS grade_str
            FROM class_rooms cr
            JOIN target_year ty ON ty.academic_year_id = cr.academic_year_id
            WHERE cr.study_plan_version_id = $1
        ),
        inserted_junction AS (
            INSERT INTO activity_slot_classrooms (slot_id, classroom_id)
            SELECT s.id, tc.id
            FROM all_slots s
            JOIN activity_catalog ac_slot ON ac_slot.id = s.activity_catalog_id
            CROSS JOIN target_classrooms tc
            WHERE EXISTS (
                SELECT 1 FROM study_plan_version_activities sva
                JOIN activity_catalog sva_ac ON sva_ac.id = sva.activity_catalog_id
                WHERE sva.study_plan_version_id = $1
                  AND sva_ac.name = ac_slot.name
                  AND sva.grade_level_id::text = tc.grade_str
            )
            ON CONFLICT (slot_id, classroom_id) DO NOTHING
            RETURNING 1
        ),
        copy_sync_instructors AS (
            INSERT INTO activity_slot_instructors (slot_id, user_id)
            SELECT s.id, acdi.instructor_id
            FROM all_slots s
            JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
            JOIN activity_catalog_default_instructors acdi ON acdi.catalog_id = s.activity_catalog_id
            WHERE ac.scheduling_mode = 'synchronized'
            ON CONFLICT (slot_id, user_id) DO NOTHING
            RETURNING 1
        ),
        copy_independent_instructor AS (
            INSERT INTO activity_slot_classroom_assignments (slot_id, classroom_id, instructor_id)
            SELECT s.id, tc.id, acdi.instructor_id
            FROM all_slots s
            JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
            JOIN activity_catalog_default_instructors acdi
              ON acdi.catalog_id = s.activity_catalog_id AND acdi.role = 'primary'
            CROSS JOIN target_classrooms tc
            JOIN resolved r ON r.catalog_id = s.activity_catalog_id
            WHERE ac.scheduling_mode = 'independent'
              AND (r.grade_level_ids IS NULL OR r.grade_level_ids ? tc.grade_str)
            ON CONFLICT (slot_id, classroom_id) DO NOTHING
            RETURNING 1
        )
        SELECT
            (SELECT COUNT(*) FROM resolved) AS total,
            (SELECT COUNT(*) FROM inserted_slots) AS added
        "#
    )
    .bind(req.study_plan_version_id).bind(req.semester_id).bind(user_id)
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let created = counts.1 as i32;
    let skipped = (counts.0 - counts.1) as i32;
    Ok((created, skipped, counts.0))
}

// ============================================
// Activity Catalog
// ============================================

#[derive(Debug, serde::Deserialize)]
pub struct ActivityCatalogFilter {
    pub latest_only: Option<bool>,
}

pub async fn list_activity_catalog(
    pool: &PgPool,
    latest_only: bool,
) -> Result<Vec<ActivityCatalog>, AppError> {
    let sql = if latest_only {
        "SELECT DISTINCT ON (ac.name) ac.*
         FROM activity_catalog ac
         JOIN academic_years ay ON ay.id = ac.start_academic_year_id
         WHERE ac.is_active = true
         ORDER BY ac.name, ay.year DESC"
    } else {
        "SELECT ac.*
         FROM activity_catalog ac
         JOIN academic_years ay ON ay.id = ac.start_academic_year_id
         WHERE ac.is_active = true
         ORDER BY ac.name, ay.year DESC"
    };
    let rows = sqlx::query_as::<_, ActivityCatalogRow>(sql)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    rows.into_iter().map(activity_catalog_from_row).collect()
}

pub async fn create_activity_catalog(
    pool: &PgPool,
    req: CreateCatalogRequest,
) -> Result<ActivityCatalog, AppError> {
    let allowed = grade_level_ids_json(req.grade_level_ids.as_deref());
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let row: ActivityCatalogRow = sqlx::query_as(
        "INSERT INTO activity_catalog
             (name, start_academic_year_id, activity_type, description,
              periods_per_week, scheduling_mode, term, grade_level_ids)
         VALUES ($1, $2, $3, $4, COALESCE($5, 1), COALESCE($6, 'synchronized'), $7, $8)
         RETURNING *",
    )
    .bind(&req.name)
    .bind(req.start_academic_year_id)
    .bind(&req.activity_type)
    .bind(&req.description)
    .bind(req.periods_per_week)
    .bind(&req.scheduling_mode)
    .bind(&req.term)
    .bind(allowed)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::BadRequest(e.to_string()))?;

    if let Some(team) = &req.default_instructors {
        for t in team {
            validate_catalog_instructor_role(&t.role)?;
            sqlx::query(
                "INSERT INTO activity_catalog_default_instructors (catalog_id, instructor_id, role)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (catalog_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
            )
            .bind(row.id)
            .bind(t.instructor_id)
            .bind(&t.role)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to save team: {}", e)))?;
        }
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    activity_catalog_from_row(row)
}

pub async fn update_activity_catalog(
    pool: &PgPool,
    id: Uuid,
    req: UpdateCatalogRequest,
) -> Result<ActivityCatalog, AppError> {
    let allowed = grade_level_ids_json(req.grade_level_ids.as_deref());
    let row = sqlx::query_as::<_, ActivityCatalogRow>(
        "UPDATE activity_catalog SET
            name = COALESCE($2, name),
            activity_type = COALESCE($3, activity_type),
            description = COALESCE($4, description),
            periods_per_week = COALESCE($5, periods_per_week),
            scheduling_mode = COALESCE($6, scheduling_mode),
            is_active = COALESCE($7, is_active),
            term = $8,
            grade_level_ids = COALESCE($9, grade_level_ids),
            updated_at = NOW()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.activity_type)
    .bind(&req.description)
    .bind(req.periods_per_week)
    .bind(&req.scheduling_mode)
    .bind(req.is_active)
    .bind(&req.term)
    .bind(allowed)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::NotFound(e.to_string()))?;

    activity_catalog_from_row(row)
}

pub async fn delete_activity_catalog(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM activity_catalog WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            AppError::BadRequest(if e.to_string().contains("foreign key") {
                "ไม่สามารถลบได้ มีหลักสูตรที่ใช้กิจกรรมนี้อยู่".to_string()
            } else {
                e.to_string()
            })
        })?;
    Ok(())
}

// ============================================
// Activity Catalog Default Instructors
// ============================================

pub async fn list_catalog_default_instructors(
    pool: &PgPool,
    catalog_id: Uuid,
) -> Result<Vec<CatalogDefaultInstructor>, AppError> {
    sqlx::query_as::<_, CatalogDefaultInstructor>(
        "SELECT acdi.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
         FROM activity_catalog_default_instructors acdi
         JOIN users u ON u.id = acdi.instructor_id
         WHERE acdi.catalog_id = $1
         ORDER BY acdi.role, acdi.created_at",
    )
    .bind(catalog_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn add_catalog_default_instructor(
    pool: &PgPool,
    catalog_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    validate_catalog_instructor_role(role)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    if role == "primary" {
        sqlx::query(
            "UPDATE activity_catalog_default_instructors SET role = 'secondary'
             WHERE catalog_id = $1 AND instructor_id <> $2 AND role = 'primary'",
        )
        .bind(catalog_id)
        .bind(instructor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    sqlx::query(
        "INSERT INTO activity_catalog_default_instructors (catalog_id, instructor_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (catalog_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(catalog_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(())
}

pub async fn remove_catalog_default_instructor(
    pool: &PgPool,
    catalog_id: Uuid,
    instructor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "DELETE FROM activity_catalog_default_instructors WHERE catalog_id = $1 AND instructor_id = $2"
    ).bind(catalog_id).bind(instructor_id).execute(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn update_catalog_default_instructor_role(
    pool: &PgPool,
    catalog_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    validate_catalog_instructor_role(role)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    if role == "primary" {
        sqlx::query(
            "UPDATE activity_catalog_default_instructors SET role = 'secondary'
             WHERE catalog_id = $1 AND instructor_id <> $2 AND role = 'primary'",
        )
        .bind(catalog_id)
        .bind(instructor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    sqlx::query(
        "UPDATE activity_catalog_default_instructors SET role = $3
         WHERE catalog_id = $1 AND instructor_id = $2",
    )
    .bind(catalog_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_level_ids_json_serializes_some_values_and_preserves_none() {
        let id = Uuid::new_v4();
        let ids = vec![id];

        assert!(grade_level_ids_json(None).is_none());
        let Some(encoded) = grade_level_ids_json(Some(&ids)) else {
            panic!("ids should encode");
        };
        assert_eq!(encoded.0, ids.as_slice());
    }

    #[test]
    fn grade_level_ids_from_jsonb_decodes_uuid_arrays() {
        let id = Uuid::new_v4();

        assert_eq!(
            grade_level_ids_from_jsonb(Some(Json(vec![id]))),
            Some(vec![id])
        );
        assert_eq!(grade_level_ids_from_jsonb(None), None);
    }

    #[test]
    fn study_plan_subject_display_order_defaults_to_zero() {
        assert_eq!(study_plan_subject_display_order(None), 0);
        assert_eq!(study_plan_subject_display_order(Some(7)), 7);
    }

    #[test]
    fn validate_catalog_instructor_role_accepts_supported_roles() {
        assert!(validate_catalog_instructor_role("primary").is_ok());
        assert!(validate_catalog_instructor_role("secondary").is_ok());
    }

    #[test]
    fn validate_catalog_instructor_role_rejects_unknown_roles() {
        assert!(matches!(
            validate_catalog_instructor_role("owner"),
            Err(AppError::BadRequest(_))
        ));
    }
}
