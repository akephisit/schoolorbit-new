use crate::error::AppError;
use crate::modules::academic::models::activity::{
    ActivityGroup, ActivityGroupFilter, ActivitySlot, ActivitySlotFilter, SlotClassroomAssignment,
};
use crate::modules::academic::models::study_plans::*;
use crate::modules::academic::services::activity_service::{self, SlotInstructorInfo};
use crate::policies::resource_access_policy::UserResourceListAccess;
use chrono::{DateTime, Utc};
use sqlx::{types::Json, FromRow, PgPool};
use std::collections::HashMap;
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

fn study_plan_subject_bulk_rows(
    subjects: &[SubjectInPlan],
) -> (Vec<Uuid>, Vec<String>, Vec<Uuid>, Vec<i32>) {
    (
        subjects
            .iter()
            .map(|subject| subject.grade_level_id)
            .collect(),
        subjects
            .iter()
            .map(|subject| subject.term.clone())
            .collect(),
        subjects.iter().map(|subject| subject.subject_id).collect(),
        subjects
            .iter()
            .map(|subject| study_plan_subject_display_order(subject.display_order))
            .collect(),
    )
}

#[derive(Debug, FromRow)]
struct SlotInstructorForSlotRow {
    slot_id: Uuid,
    id: Uuid,
    user_id: Uuid,
    instructor_name: Option<String>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct GenerateActivitiesFromPlanOutcome {
    pub created: i32,
    pub skipped: i32,
    pub total_templates: i64,
    pub slots: Vec<ActivitySlot>,
    pub groups: Vec<ActivityGroup>,
    pub slot_instructors: HashMap<Uuid, Vec<SlotInstructorInfo>>,
    pub slot_classroom_assignments: HashMap<Uuid, Vec<SlotClassroomAssignment>>,
}

async fn list_slot_instructors_for_slots(
    pool: &PgPool,
    slot_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<SlotInstructorInfo>>, AppError> {
    let mut grouped = slot_ids
        .iter()
        .copied()
        .map(|slot_id| (slot_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    if slot_ids.is_empty() {
        return Ok(grouped);
    }

    let slot_ids = slot_ids.to_vec();
    let rows = sqlx::query_as::<_, SlotInstructorForSlotRow>(
        r#"SELECT asi.slot_id, asi.id, asi.user_id,
                  u.first_name || ' ' || u.last_name AS instructor_name
           FROM activity_slot_instructors asi
           JOIN users u ON u.id = asi.user_id
           WHERE asi.slot_id = ANY($1)
           ORDER BY u.first_name"#,
    )
    .bind(&slot_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    for row in rows {
        grouped
            .entry(row.slot_id)
            .or_default()
            .push(SlotInstructorInfo {
                id: row.id,
                user_id: row.user_id,
                instructor_name: row.instructor_name,
            });
    }

    Ok(grouped)
}

async fn list_slot_classroom_assignments_for_slots(
    pool: &PgPool,
    slot_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<SlotClassroomAssignment>>, AppError> {
    let mut grouped = slot_ids
        .iter()
        .copied()
        .map(|slot_id| (slot_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    if slot_ids.is_empty() {
        return Ok(grouped);
    }

    let slot_ids = slot_ids.to_vec();
    let rows = sqlx::query_as::<_, SlotClassroomAssignment>(
        r#"SELECT asca.*, cr.name AS classroom_name,
                  concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM activity_slot_classroom_assignments asca
           JOIN class_rooms cr ON cr.id = asca.classroom_id
           JOIN users u ON u.id = asca.instructor_id
           WHERE asca.slot_id = ANY($1)
           ORDER BY cr.name"#,
    )
    .bind(&slot_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("list generated slot classroom assignments error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    for assignment in rows {
        grouped
            .entry(assignment.slot_id)
            .or_default()
            .push(assignment);
    }

    Ok(grouped)
}

async fn bulk_insert_study_plan_subjects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
    grade_level_ids: &[Uuid],
    terms: &[String],
    subject_ids: &[Uuid],
    display_orders: &[i32],
) -> Result<u64, AppError> {
    if grade_level_ids.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query(
        "INSERT INTO study_plan_subjects
         (study_plan_version_id, grade_level_id, term, subject_id, display_order)
         SELECT $1, grade_level_id, term, subject_id, display_order
         FROM UNNEST($2::uuid[], $3::text[], $4::uuid[], $5::int4[])
              AS rows(grade_level_id, term, subject_id, display_order)
         ON CONFLICT (study_plan_version_id, grade_level_id, term, subject_id) DO NOTHING",
    )
    .bind(version_id)
    .bind(grade_level_ids)
    .bind(terms)
    .bind(subject_ids)
    .bind(display_orders)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

fn validate_catalog_instructor_role(role: &str) -> Result<(), AppError> {
    if role == "primary" || role == "secondary" {
        return Ok(());
    }

    Err(AppError::BadRequest(
        "role must be 'primary' or 'secondary'".to_string(),
    ))
}

fn unique_catalog_default_instructors(
    team: &[CatalogDefaultInstructorInput],
) -> Result<Vec<CatalogDefaultInstructorInput>, AppError> {
    let mut rows: Vec<CatalogDefaultInstructorInput> = Vec::with_capacity(team.len());
    let mut index_by_instructor = HashMap::with_capacity(team.len());

    for instructor in team {
        validate_catalog_instructor_role(&instructor.role)?;
        let row = CatalogDefaultInstructorInput {
            instructor_id: instructor.instructor_id,
            role: instructor.role.clone(),
        };
        if let Some(index) = index_by_instructor.get(&instructor.instructor_id).copied() {
            rows[index] = row;
        } else {
            index_by_instructor.insert(instructor.instructor_id, rows.len());
            rows.push(row);
        }
    }

    Ok(rows)
}

async fn bulk_upsert_catalog_default_instructors(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    catalog_id: Uuid,
    team: &[CatalogDefaultInstructorInput],
) -> Result<(), AppError> {
    let team = unique_catalog_default_instructors(team)?;
    if team.is_empty() {
        return Ok(());
    }

    let instructor_ids: Vec<Uuid> = team.iter().map(|item| item.instructor_id).collect();
    let roles: Vec<String> = team.iter().map(|item| item.role.clone()).collect();

    sqlx::query(
        "INSERT INTO activity_catalog_default_instructors (catalog_id, instructor_id, role)
         SELECT $1, instructor_id, role
         FROM UNNEST($2::uuid[], $3::text[]) AS rows(instructor_id, role)
         ON CONFLICT (catalog_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(catalog_id)
    .bind(&instructor_ids)
    .bind(&roles)
    .execute(&mut **tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to save team: {}", e)))?;

    Ok(())
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
        .map_err(|error| not_found_or(error, "Study plan not found"))?;

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
    let row = q
        .fetch_one(pool)
        .await
        .map_err(|error| not_found_or(error, "Study plan not found"))?;
    study_plan_from_row(row)
}

pub async fn delete_plan(pool: &PgPool, plan_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM study_plans WHERE id = $1")
        .bind(plan_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Study plan not found".to_string()));
    }
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
    q.fetch_all(pool).await.map_err(AppError::from)
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
    .map_err(|error| not_found_or(error, "Study-plan version not found"))
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
    .map_err(|error| not_found_or(error, "Study-plan version not found"))
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
    .map_err(|error| not_found_or(error, "Study-plan version not found"))
}

pub async fn delete_version(pool: &PgPool, version_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM study_plan_versions WHERE id = $1")
        .bind(version_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Study-plan version not found".to_string(),
        ));
    }
    Ok(())
}

// ============================================
// Study Plan Subjects
// ============================================

pub async fn list_plan_subjects(
    pool: &PgPool,
    query: StudyPlanSubjectQuery,
) -> Result<Vec<StudyPlanSubject>, AppError> {
    if let Some(version_id) = query.study_plan_version_id {
        ensure_study_plan_version_exists(pool, version_id).await?;
    }
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
    q.fetch_all(pool).await.map_err(AppError::from)
}

pub async fn add_subjects_to_version(
    pool: &PgPool,
    version_id: Uuid,
    req: AddSubjectsToVersionRequest,
) -> Result<usize, AppError> {
    let mut tx = pool.begin().await?;
    let version_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM study_plan_versions WHERE id = $1 FOR SHARE")
            .bind(version_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !version_exists {
        return Err(AppError::NotFound(
            "Study-plan version not found".to_string(),
        ));
    }
    let (grade_level_ids, terms, subject_ids, display_orders) =
        study_plan_subject_bulk_rows(&req.subjects);
    let inserted = bulk_insert_study_plan_subjects(
        &mut tx,
        version_id,
        &grade_level_ids,
        &terms,
        &subject_ids,
        &display_orders,
    )
    .await?;
    tx.commit().await?;
    Ok(inserted as usize)
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

    let Some((plan_id, grade_id, term, subject_id)) = context else {
        return Err(AppError::NotFound(
            "Study-plan subject not found".to_string(),
        ));
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sqlx::query("DELETE FROM study_plan_subjects WHERE id = $1")
        .bind(sps_id)
        .execute(&mut *tx)
        .await?;

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

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

fn not_found_or(error: sqlx::Error, message: &str) -> AppError {
    match error {
        sqlx::Error::RowNotFound => AppError::NotFound(message.to_string()),
        error => AppError::from(error),
    }
}

async fn ensure_study_plan_version_exists(pool: &PgPool, version_id: Uuid) -> Result<(), AppError> {
    ensure_entity_exists(
        pool,
        "SELECT EXISTS(SELECT 1 FROM study_plan_versions WHERE id = $1)",
        version_id,
        "Study-plan version not found",
    )
    .await
}

async fn ensure_activity_catalog_exists(pool: &PgPool, catalog_id: Uuid) -> Result<(), AppError> {
    ensure_entity_exists(
        pool,
        "SELECT EXISTS(SELECT 1 FROM activity_catalog WHERE id = $1)",
        catalog_id,
        "Activity catalog not found",
    )
    .await
}

async fn ensure_academic_year_exists(pool: &PgPool, year_id: Uuid) -> Result<(), AppError> {
    ensure_entity_exists(
        pool,
        "SELECT EXISTS(SELECT 1 FROM academic_years WHERE id = $1)",
        year_id,
        "Academic year not found",
    )
    .await
}

async fn ensure_academic_semester_exists(pool: &PgPool, semester_id: Uuid) -> Result<(), AppError> {
    ensure_entity_exists(
        pool,
        "SELECT EXISTS(SELECT 1 FROM academic_semesters WHERE id = $1)",
        semester_id,
        "Academic semester not found",
    )
    .await
}

async fn ensure_entity_exists(
    pool: &PgPool,
    query: &'static str,
    id: Uuid,
    not_found_message: &'static str,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(query).bind(id).fetch_one(pool).await?;

    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(not_found_message.to_string()))
    }
}

async fn ensure_grade_levels_exist(
    pool: &PgPool,
    grade_level_ids: &[Uuid],
) -> Result<(), AppError> {
    let all_exist: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (
             SELECT requested.id
             FROM UNNEST($1::uuid[]) AS requested(id)
             LEFT JOIN grade_levels gl ON gl.id = requested.id
             WHERE gl.id IS NULL
         )",
    )
    .bind(grade_level_ids)
    .fetch_one(pool)
    .await?;
    if all_exist {
        Ok(())
    } else {
        Err(AppError::NotFound("Grade level not found".to_string()))
    }
}

async fn ensure_users_exist(pool: &PgPool, user_ids: &[Uuid]) -> Result<(), AppError> {
    let all_exist: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (
             SELECT requested.id
             FROM UNNEST($1::uuid[]) AS requested(id)
             LEFT JOIN users u ON u.id = requested.id
             WHERE u.id IS NULL
         )",
    )
    .bind(user_ids)
    .fetch_one(pool)
    .await?;
    if all_exist {
        Ok(())
    } else {
        Err(AppError::NotFound("Instructor not found".to_string()))
    }
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
            SELECT sps.subject_id
            FROM study_plan_subjects sps
            WHERE sps.study_plan_version_id = $1
              AND sps.grade_level_id = $2
              AND sps.term = $3
        ),
        inserted AS (
            INSERT INTO classroom_courses
                (classroom_id, subject_id, academic_semester_id, settings, primary_instructor_id)
            SELECT $4, ps.subject_id, $5, '{}'::jsonb,
                (SELECT sdi.instructor_id
                 FROM subject_default_instructors sdi
                 WHERE sdi.subject_id = ps.subject_id
                 ORDER BY (sdi.role = 'primary') DESC, sdi.created_at ASC
                 LIMIT 1)
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
    ensure_study_plan_version_exists(pool, version_id).await?;
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
    let mut tx = pool.begin().await?;
    let version_exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM study_plan_versions WHERE id = $1 FOR KEY SHARE",
    )
    .bind(version_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !version_exists {
        return Err(AppError::NotFound(
            "Study-plan version not found".to_string(),
        ));
    }
    let catalog_exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM activity_catalog WHERE id = $1 FOR KEY SHARE",
    )
    .bind(req.activity_catalog_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !catalog_exists {
        return Err(AppError::NotFound("Activity catalog not found".to_string()));
    }
    let grade_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM grade_levels WHERE id = $1 FOR KEY SHARE")
            .bind(req.grade_level_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !grade_exists {
        return Err(AppError::NotFound("Grade level not found".to_string()));
    }

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
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::BadRequest("กิจกรรมนี้อยู่ในหลักสูตรสำหรับชั้น+เทอมนี้แล้ว".to_string()))?;

    tx.commit().await?;
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
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Study-plan activity not found".to_string()))?;

    plan_activity_from_row(row)
}

pub async fn delete_plan_activity(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let context: Option<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT ac.name, sva.study_plan_version_id, sva.grade_level_id
         FROM study_plan_version_activities sva
         JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
         WHERE sva.id = $1
         FOR UPDATE OF sva",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let Some((catalog_name, plan_id, grade_id)) = context else {
        return Err(AppError::NotFound(
            "Study-plan activity not found".to_string(),
        ));
    };
    sqlx::query("DELETE FROM study_plan_version_activities WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

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

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn generate_activities_from_plan(
    pool: &PgPool,
    req: GenerateActivitiesFromPlanRequest,
    user_id: Option<Uuid>,
) -> Result<GenerateActivitiesFromPlanOutcome, AppError> {
    ensure_study_plan_version_exists(pool, req.study_plan_version_id).await?;
    ensure_academic_semester_exists(pool, req.semester_id).await?;
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
    let slots = activity_service::list_slots(
        pool,
        ActivitySlotFilter {
            semester_id: Some(req.semester_id),
            activity_type: None,
            teacher_reg_open: None,
            student_reg_open: None,
        },
        UserResourceListAccess::School,
    )
    .await?;
    let groups = activity_service::list_groups(
        pool,
        ActivityGroupFilter {
            slot_id: None,
            semester_id: Some(req.semester_id),
            activity_type: None,
            instructor_id: None,
            registration_open: None,
            search: None,
        },
        UserResourceListAccess::School,
    )
    .await?;
    let slot_ids = slots.iter().map(|slot| slot.id).collect::<Vec<_>>();
    let slot_instructors = list_slot_instructors_for_slots(pool, &slot_ids).await?;
    let slot_classroom_assignments =
        list_slot_classroom_assignments_for_slots(pool, &slot_ids).await?;

    Ok(GenerateActivitiesFromPlanOutcome {
        created,
        skipped,
        total_templates: counts.0,
        slots,
        groups,
        slot_instructors,
        slot_classroom_assignments,
    })
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
    ensure_academic_year_exists(pool, req.start_academic_year_id).await?;
    if let Some(grade_level_ids) = &req.grade_level_ids {
        ensure_grade_levels_exist(pool, grade_level_ids).await?;
    }
    if let Some(team) = &req.default_instructors {
        let team = unique_catalog_default_instructors(team)?;
        let instructor_ids = team
            .iter()
            .map(|instructor| instructor.instructor_id)
            .collect::<Vec<_>>();
        ensure_users_exist(pool, &instructor_ids).await?;
    }
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
        bulk_upsert_catalog_default_instructors(&mut tx, row.id, team).await?;
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
    if let Some(grade_level_ids) = &req.grade_level_ids {
        ensure_grade_levels_exist(pool, grade_level_ids).await?;
    }
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
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Activity catalog not found".to_string()))?;

    activity_catalog_from_row(row)
}

pub async fn delete_activity_catalog(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM activity_catalog WHERE id = $1")
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
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Activity catalog not found".to_string()));
    }
    Ok(())
}

// ============================================
// Activity Catalog Default Instructors
// ============================================

pub async fn list_catalog_default_instructors(
    pool: &PgPool,
    catalog_id: Uuid,
) -> Result<Vec<CatalogDefaultInstructor>, AppError> {
    ensure_activity_catalog_exists(pool, catalog_id).await?;
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
    let catalog_exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM activity_catalog WHERE id = $1 FOR KEY SHARE",
    )
    .bind(catalog_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !catalog_exists {
        return Err(AppError::NotFound("Activity catalog not found".to_string()));
    }
    let instructor_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id = $1 FOR KEY SHARE")
            .bind(instructor_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !instructor_exists {
        return Err(AppError::NotFound("Instructor not found".to_string()));
    }

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
    let result = sqlx::query(
        "DELETE FROM activity_catalog_default_instructors WHERE catalog_id = $1 AND instructor_id = $2"
    ).bind(catalog_id).bind(instructor_id).execute(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Activity catalog instructor assignment not found".to_string(),
        ));
    }
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
    let assignment_exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM activity_catalog_default_instructors
         WHERE catalog_id = $1 AND instructor_id = $2
         FOR UPDATE",
    )
    .bind(catalog_id)
    .bind(instructor_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !assignment_exists {
        return Err(AppError::NotFound(
            "Activity catalog instructor assignment not found".to_string(),
        ));
    }

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
    fn study_plan_from_row_maps_typed_grade_level_ids() {
        let grade_level_id = Uuid::new_v4();
        let now = Utc::now();
        let row = StudyPlanRow {
            id: Uuid::new_v4(),
            code: "SCI-MATH".to_string(),
            name_th: "วิทย์-คณิต".to_string(),
            name_en: Some("Science Math".to_string()),
            description: Some("แผนการเรียนวิทยาศาสตร์คณิตศาสตร์".to_string()),
            grade_level_ids: Some(Json(vec![grade_level_id])),
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let plan = study_plan_from_row(row).expect("study plan row should map");

        assert_eq!(plan.code, "SCI-MATH");
        assert_eq!(plan.grade_level_ids, Some(vec![grade_level_id]));
        assert!(plan.is_active);
    }

    #[test]
    fn plan_activity_from_row_maps_catalog_grade_levels_from_typed_json() {
        let grade_level_id = Uuid::new_v4();
        let catalog_grade_level_id = Uuid::new_v4();
        let now = Utc::now();
        let row = StudyPlanVersionActivityRow {
            id: Uuid::new_v4(),
            study_plan_version_id: Uuid::new_v4(),
            activity_catalog_id: Uuid::new_v4(),
            grade_level_id,
            term: Some("1".to_string()),
            display_order: 3,
            created_at: now,
            updated_at: now,
            catalog_name: Some("ลูกเสือ".to_string()),
            catalog_activity_type: Some("scout".to_string()),
            catalog_description: None,
            catalog_periods_per_week: Some(1),
            catalog_scheduling_mode: Some("fixed".to_string()),
            catalog_term: Some("1".to_string()),
            catalog_grade_level_ids: Some(Json(vec![catalog_grade_level_id])),
        };

        let activity = plan_activity_from_row(row).expect("activity row should map");

        assert_eq!(activity.grade_level_id, grade_level_id);
        assert_eq!(activity.catalog_name.as_deref(), Some("ลูกเสือ"));
        assert_eq!(
            activity.catalog_grade_level_ids,
            Some(vec![catalog_grade_level_id])
        );
    }

    #[test]
    fn activity_catalog_from_row_maps_optional_grade_levels() {
        let now = Utc::now();
        let row = ActivityCatalogRow {
            id: Uuid::new_v4(),
            name: "ชุมนุม".to_string(),
            start_academic_year_id: Uuid::new_v4(),
            activity_type: "club".to_string(),
            description: None,
            periods_per_week: 2,
            scheduling_mode: "flexible".to_string(),
            is_active: true,
            term: None,
            grade_level_ids: None,
            created_at: now,
            updated_at: now,
        };

        let catalog = activity_catalog_from_row(row).expect("catalog row should map");

        assert_eq!(catalog.name, "ชุมนุม");
        assert_eq!(catalog.grade_level_ids, None);
        assert_eq!(catalog.periods_per_week, 2);
    }

    #[test]
    fn study_plan_subject_display_order_defaults_to_zero() {
        assert_eq!(study_plan_subject_display_order(None), 0);
        assert_eq!(study_plan_subject_display_order(Some(7)), 7);
    }

    #[test]
    fn study_plan_subject_bulk_rows_maps_payload_for_batch_insert() {
        let grade_level_id = Uuid::new_v4();
        let subject_id = Uuid::new_v4();

        let (grade_level_ids, terms, subject_ids, display_orders) =
            study_plan_subject_bulk_rows(&[SubjectInPlan {
                grade_level_id,
                term: "1".to_string(),
                subject_id,
                display_order: None,
            }]);

        assert_eq!(grade_level_ids, vec![grade_level_id]);
        assert_eq!(terms, vec!["1".to_string()]);
        assert_eq!(subject_ids, vec![subject_id]);
        assert_eq!(display_orders, vec![0]);
    }

    #[test]
    fn validate_catalog_instructor_role_accepts_supported_roles() {
        assert!(validate_catalog_instructor_role("primary").is_ok());
        assert!(validate_catalog_instructor_role("secondary").is_ok());
    }

    #[test]
    fn unique_catalog_default_instructors_keeps_latest_role_per_instructor() {
        let instructor_id = Uuid::new_v4();

        let rows = unique_catalog_default_instructors(&[
            CatalogDefaultInstructorInput {
                instructor_id,
                role: "secondary".to_string(),
            },
            CatalogDefaultInstructorInput {
                instructor_id,
                role: "primary".to_string(),
            },
        ])
        .expect("roles should be valid");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].instructor_id, instructor_id);
        assert_eq!(rows[0].role, "primary");
    }

    #[test]
    fn validate_catalog_instructor_role_rejects_unknown_roles() {
        assert!(matches!(
            validate_catalog_instructor_role("owner"),
            Err(AppError::BadRequest(_))
        ));
    }
}
