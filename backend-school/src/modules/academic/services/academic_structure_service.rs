use crate::error::AppError;
use crate::modules::academic::models::*;
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct AcademicStructure {
    pub years: Vec<AcademicYear>,
    pub semesters: Vec<Semester>,
    pub levels: Vec<GradeLevelResponse>,
}

#[derive(sqlx::FromRow)]
struct StudentForNumbering {
    id: Uuid,
    student_code: String,
    first_name: String,
    title: Option<String>,
}

struct ClassroomIdentity {
    name: String,
    code: String,
}

fn ordered_unique_uuids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = HashSet::with_capacity(ids.len());
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

async fn bulk_mark_existing_enrollments_moved_out(
    tx: &mut Transaction<'_, Postgres>,
    student_ids: &[Uuid],
) -> Result<(), AppError> {
    if student_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "UPDATE student_class_enrollments SET status = 'moved_out', updated_at = NOW()
         WHERE student_id = ANY($1) AND status = 'active'",
    )
    .bind(student_ids)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn bulk_upsert_class_enrollments(
    tx: &mut Transaction<'_, Postgres>,
    class_room_id: Uuid,
    enrollment_date: chrono::NaiveDate,
    student_ids: &[Uuid],
) -> Result<u64, AppError> {
    if student_ids.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query(
        "INSERT INTO student_class_enrollments (student_id, class_room_id, enrollment_date, status)
         SELECT student_id, $2, $3, 'active'
         FROM UNNEST($1::uuid[]) AS rows(student_id)
         ON CONFLICT (student_id, class_room_id)
         DO UPDATE SET status = 'active', enrollment_date = $3, updated_at = NOW()",
    )
    .bind(student_ids)
    .bind(class_room_id)
    .bind(enrollment_date)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

pub async fn list_academic_structure(pool: &PgPool) -> Result<AcademicStructure, AppError> {
    let years =
        sqlx::query_as::<_, AcademicYear>("SELECT * FROM academic_years ORDER BY year DESC")
            .fetch_all(pool)
            .await?;

    let semesters =
        sqlx::query_as::<_, Semester>("SELECT * FROM academic_semesters ORDER BY start_date DESC")
            .fetch_all(pool)
            .await?;

    let levels_raw = sqlx::query_as::<_, GradeLevel>(
        "SELECT * FROM grade_levels ORDER BY 
         CASE level_type 
            WHEN 'kindergarten' THEN 1 
            WHEN 'primary' THEN 2 
            WHEN 'secondary' THEN 3 
            ELSE 4 
         END, 
         year ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(AcademicStructure {
        years,
        semesters,
        levels: levels_raw
            .into_iter()
            .map(GradeLevelResponse::from)
            .collect(),
    })
}

pub async fn create_academic_year(
    pool: &PgPool,
    payload: CreateAcademicYearRequest,
) -> Result<AcademicYear, AppError> {
    if payload.is_active.unwrap_or(false) {
        sqlx::query("UPDATE academic_years SET is_active = false")
            .execute(pool)
            .await?;
    }

    let school_days = payload
        .school_days
        .unwrap_or_else(|| "MON,TUE,WED,THU,FRI".to_string());

    sqlx::query_as::<_, AcademicYear>(
        "INSERT INTO academic_years (year, name, start_date, end_date, is_active, school_days)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(payload.year)
    .bind(payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active.unwrap_or(false))
    .bind(&school_days)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_academic_year(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateAcademicYearRequest,
) -> Result<AcademicYear, AppError> {
    if payload.is_active.unwrap_or(false) {
        sqlx::query("UPDATE academic_years SET is_active = false")
            .execute(pool)
            .await?;
    }

    sqlx::query_as::<_, AcademicYear>(
        r#"UPDATE academic_years SET
            year = COALESCE($2, year),
            name = COALESCE($3, name),
            start_date = COALESCE($4, start_date),
            end_date = COALESCE($5, end_date),
            is_active = COALESCE($6, is_active),
            school_days = COALESCE($7, school_days),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *"#,
    )
    .bind(id)
    .bind(payload.year)
    .bind(&payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active)
    .bind(&payload.school_days)
    .fetch_one(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::RowNotFound => AppError::NotFound("Academic year not found".to_string()),
        error => AppError::from(error),
    })
}

pub async fn toggle_active_year(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let target_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM academic_years WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !target_exists {
        return Err(AppError::NotFound("Academic year not found".to_string()));
    }

    sqlx::query("UPDATE academic_years SET is_active = false")
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE academic_years SET is_active = true WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn create_semester(
    pool: &PgPool,
    payload: CreateSemesterRequest,
) -> Result<Semester, AppError> {
    if payload.is_active.unwrap_or(false) {
        sqlx::query("UPDATE academic_semesters SET is_active = false")
            .execute(pool)
            .await?;
    }

    sqlx::query_as::<_, Semester>(
        "INSERT INTO academic_semesters (academic_year_id, term, name, start_date, end_date, is_active) 
         VALUES ($1, $2, $3, $4, $5, $6) 
         RETURNING *",
    )
    .bind(payload.academic_year_id)
    .bind(payload.term)
    .bind(payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active.unwrap_or(false))
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_semester(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateSemesterRequest,
) -> Result<Semester, AppError> {
    if payload.is_active.unwrap_or(false) {
        sqlx::query("UPDATE academic_semesters SET is_active = false")
            .execute(pool)
            .await?;
    }

    sqlx::query_as::<_, Semester>(
        "UPDATE academic_semesters SET 
            term = COALESCE($1, term),
            name = COALESCE($2, name),
            start_date = COALESCE($3, start_date),
            end_date = COALESCE($4, end_date),
            is_active = COALESCE($5, is_active),
            updated_at = NOW()
         WHERE id = $6
         RETURNING *",
    )
    .bind(payload.term)
    .bind(payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::RowNotFound => AppError::NotFound("Semester not found".to_string()),
        error => AppError::from(error),
    })
}

pub async fn delete_semester(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM academic_semesters WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| {
            if error.to_string().contains("foreign key constraint") {
                AppError::BadRequest("ไม่สามารถลบภาคเรียนที่มีการใช้งานได้".to_string())
            } else {
                AppError::from(error)
            }
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Semester not found".to_string()));
    }

    Ok(())
}

pub async fn list_classrooms(
    pool: &PgPool,
    filter: ClassroomQuery,
) -> Result<Vec<Classroom>, AppError> {
    let mut query = String::from(
        "SELECT c.*,
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                ay.name as academic_year_label,
                (SELECT COUNT(*) FROM student_class_enrollments ske WHERE ske.class_room_id = c.id AND ske.status = 'active') as student_count,
                COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'user_id', ca.user_id,
                            'role', ca.role,
                            'name', CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)
                        ) ORDER BY ca.role, u.first_name
                    )
                    FROM classroom_advisors ca
                    JOIN users u ON u.id = ca.user_id
                    WHERE ca.classroom_id = c.id
                ), '[]'::jsonb) as advisors
         FROM class_rooms c
         JOIN grade_levels gl ON c.grade_level_id = gl.id
         JOIN academic_years ay ON c.academic_year_id = ay.id
         WHERE 1=1",
    );

    let mut idx = 0u32;

    if filter.year_id.is_some() {
        idx += 1;
        query.push_str(&format!(" AND c.academic_year_id = ${idx}"));
    }

    query.push_str(
        " ORDER BY
         CASE gl.level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
         END,
         gl.year ASC,
         c.room_number ASC",
    );

    let mut q = sqlx::query_as::<_, Classroom>(&query);
    if let Some(year_id) = filter.year_id {
        q = q.bind(year_id);
    }

    q.fetch_all(pool).await.map_err(AppError::from)
}

pub async fn create_classroom(
    pool: &PgPool,
    payload: CreateClassroomRequest,
) -> Result<Classroom, AppError> {
    let grade_level = sqlx::query_as::<_, GradeLevel>("SELECT * FROM grade_levels WHERE id = $1")
        .bind(payload.grade_level_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::BadRequest("Invalid grade level".to_string()))?;

    let year = sqlx::query_as::<_, AcademicYear>("SELECT * FROM academic_years WHERE id = $1")
        .bind(payload.academic_year_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::BadRequest("Invalid academic year".to_string()))?;

    let advisors = validate_advisors(pool, &payload.advisors).await?;
    let identity = classroom_identity(year.year, &grade_level, &payload.room_number);

    let mut tx = pool.begin().await?;

    let classroom_id: Uuid = sqlx::query_scalar(
        "INSERT INTO class_rooms (code, name, academic_year_id, grade_level_id, room_number, study_plan_version_id, capacity)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(identity.code)
    .bind(identity.name)
    .bind(payload.academic_year_id)
    .bind(payload.grade_level_id)
    .bind(&payload.room_number)
    .bind(payload.study_plan_version_id)
    .bind(payload.capacity.unwrap_or(40))
    .fetch_one(&mut *tx)
    .await
    .map_err(classroom_create_error)?;

    insert_advisors(&mut tx, classroom_id, &advisors).await?;

    tx.commit().await?;

    fetch_classroom_full(pool, classroom_id).await
}

pub async fn update_classroom(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateClassroomRequest,
) -> Result<Classroom, AppError> {
    let advisors_opt = if payload.advisors.is_some() {
        Some(validate_advisors(pool, &payload.advisors).await?)
    } else {
        None
    };

    let mut tx = pool.begin().await?;

    if let Some(ref new_room) = payload.room_number {
        let current: (Uuid, Uuid) = sqlx::query_as(
            "SELECT grade_level_id, academic_year_id FROM class_rooms WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| AppError::NotFound("Classroom not found".to_string()))?;

        let grade_level = sqlx::query_as::<_, GradeLevel>(
            "SELECT id, level_type, year, next_grade_level_id, is_active FROM grade_levels WHERE id = $1",
        )
        .bind(current.0)
        .fetch_one(&mut *tx)
        .await?;

        let year = sqlx::query_as::<_, AcademicYear>(
            "SELECT id, year, name, start_date, end_date, is_active, school_days, metadata, created_at, updated_at FROM academic_years WHERE id = $1",
        )
        .bind(current.1)
        .fetch_one(&mut *tx)
        .await?;

        let identity = classroom_identity(year.year, &grade_level, new_room);

        sqlx::query("UPDATE class_rooms SET name = $1, code = $2, room_number = $3 WHERE id = $4")
            .bind(identity.name)
            .bind(identity.code)
            .bind(new_room)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                if error.to_string().contains("unique") {
                    AppError::BadRequest("ชื่อ/รหัสห้องเรียนซ้ำ".to_string())
                } else {
                    AppError::from(error)
                }
            })?;
    }

    sqlx::query(
        "UPDATE class_rooms SET
            study_plan_version_id = COALESCE($1, study_plan_version_id),
            capacity = COALESCE($2, capacity),
            is_active = COALESCE($3, is_active),
            updated_at = NOW()
         WHERE id = $4",
    )
    .bind(payload.study_plan_version_id)
    .bind(payload.capacity)
    .bind(payload.is_active)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if let Some(advisors) = advisors_opt {
        sqlx::query("DELETE FROM classroom_advisors WHERE classroom_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        insert_advisors(&mut tx, id, &advisors).await?;
    }

    tx.commit().await?;

    fetch_classroom_full(pool, id).await
}

pub async fn create_grade_level(
    pool: &PgPool,
    payload: CreateGradeLevelRequest,
) -> Result<GradeLevelResponse, AppError> {
    if !["kindergarten", "primary", "secondary"].contains(&payload.level_type.as_str()) {
        return Err(AppError::BadRequest("ประเภทระดับชั้นไม่ถูกต้อง".to_string()));
    }

    let level = sqlx::query_as::<_, GradeLevel>(
        "INSERT INTO grade_levels (level_type, year, next_grade_level_id, is_active)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(&payload.level_type)
    .bind(payload.year)
    .bind(payload.next_grade_level_id)
    .bind(payload.is_active.unwrap_or(true))
    .fetch_one(pool)
    .await
    .map_err(|error| {
        if error.to_string().contains("unique") {
            AppError::BadRequest("ระดับชั้นนี้มีอยู่แล้ว".to_string())
        } else {
            AppError::from(error)
        }
    })?;

    Ok(GradeLevelResponse::from(level))
}

pub async fn delete_grade_level(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let usage_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM class_rooms WHERE grade_level_id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;

    if usage_count > 0 {
        return Err(AppError::BadRequest(format!(
            "ไม่สามารถลบระดับชั้นได้เนื่องจากมีการใช้งานอยู่ {} ห้องเรียน",
            usage_count
        )));
    }

    let result = sqlx::query("DELETE FROM grade_levels WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Grade level not found".to_string()));
    }

    Ok(())
}

pub async fn enroll_students(
    pool: &PgPool,
    payload: EnrollStudentRequest,
) -> Result<usize, AppError> {
    ensure_classroom_exists(pool, payload.class_room_id).await?;

    let enroll_date = payload
        .enrollment_date
        .unwrap_or(chrono::Local::now().date_naive());

    let student_ids = ordered_unique_uuids(&payload.student_ids);
    let mut tx = pool.begin().await?;
    bulk_mark_existing_enrollments_moved_out(&mut tx, &student_ids).await?;
    let enrolled_count =
        bulk_upsert_class_enrollments(&mut tx, payload.class_room_id, enroll_date, &student_ids)
            .await? as usize;

    tx.commit().await?;

    assign_numbers_after_enrollment(pool, &payload).await?;

    Ok(enrolled_count)
}

pub async fn get_class_enrollments(
    pool: &PgPool,
    class_id: Uuid,
) -> Result<Vec<StudentEnrollment>, AppError> {
    ensure_classroom_exists(pool, class_id).await?;

    sqlx::query_as::<_, StudentEnrollment>(
        "SELECT ske.*, 
                CONCAT(u.first_name, ' ', u.last_name) as student_name,
                c.name as class_name,
                s.student_id as student_code
         FROM student_class_enrollments ske
         LEFT JOIN users u ON ske.student_id = u.id
         LEFT JOIN student_info s ON u.id = s.user_id
         LEFT JOIN class_rooms c ON ske.class_room_id = c.id
         WHERE ske.class_room_id = $1 AND ske.status = 'active'
         ORDER BY ske.class_number ASC NULLS LAST, s.student_id ASC",
    )
    .bind(class_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn remove_enrollment(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let result = sqlx::query("DELETE FROM student_class_enrollments WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Enrollment not found".to_string()));
    }

    tx.commit().await?;
    Ok(())
}

pub async fn update_enrollment_number(
    pool: &PgPool,
    id: Uuid,
    class_number: Option<i32>,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE student_class_enrollments SET class_number = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(class_number)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Enrollment not found".to_string()));
    }

    Ok(())
}

pub async fn auto_assign_class_numbers(
    pool: &PgPool,
    class_id: Uuid,
    sort_by: &str,
) -> Result<usize, AppError> {
    ensure_classroom_exists(pool, class_id).await?;
    let students = sorted_students_for_numbering(pool, class_id, sort_by).await?;
    update_class_numbers(pool, &students).await?;
    Ok(students.len())
}

pub async fn get_year_levels(pool: &PgPool, year_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    ensure_academic_year_exists(pool, year_id).await?;

    sqlx::query_scalar::<_, Uuid>(
        "SELECT grade_level_id FROM academic_year_grade_levels WHERE academic_year_id = $1",
    )
    .bind(year_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_year_levels(
    pool: &PgPool,
    year_id: Uuid,
    grade_level_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let target_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM academic_years WHERE id = $1 FOR UPDATE")
            .bind(year_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !target_exists {
        return Err(AppError::NotFound("Academic year not found".to_string()));
    }

    sqlx::query("DELETE FROM academic_year_grade_levels WHERE academic_year_id = $1")
        .bind(year_id)
        .execute(&mut *tx)
        .await?;

    bulk_insert_year_levels(&mut tx, year_id, &grade_level_ids).await?;

    tx.commit().await?;
    Ok(())
}

async fn bulk_insert_year_levels(
    tx: &mut Transaction<'_, Postgres>,
    year_id: Uuid,
    grade_level_ids: &[Uuid],
) -> Result<(), AppError> {
    let grade_level_ids = ordered_unique_uuids(grade_level_ids);
    if grade_level_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO academic_year_grade_levels (academic_year_id, grade_level_id)
         SELECT $1, grade_level_id
         FROM UNNEST($2::uuid[]) AS rows(grade_level_id)
         ON CONFLICT DO NOTHING",
    )
    .bind(year_id)
    .bind(&grade_level_ids)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn validate_advisors(
    pool: &PgPool,
    advisors: &Option<Vec<ClassroomAdvisorInput>>,
) -> Result<Vec<ClassroomAdvisorInput>, AppError> {
    let Some(list) = advisors else {
        return Ok(vec![]);
    };

    let primary_count = list
        .iter()
        .filter(|advisor| advisor.role == "primary")
        .count();
    if primary_count > 1 {
        return Err(AppError::BadRequest(
            "ครูที่ปรึกษาหลักต้องมีได้ไม่เกิน 1 คน".to_string(),
        ));
    }

    for advisor in list {
        if advisor.role != "primary" && advisor.role != "secondary" {
            return Err(AppError::BadRequest(
                "role ต้องเป็น 'primary' หรือ 'secondary' เท่านั้น".to_string(),
            ));
        }
    }

    if list.is_empty() {
        return Ok(vec![]);
    }

    let unique_ids: HashSet<Uuid> = list.iter().map(|advisor| advisor.user_id).collect();
    let ids_vec: Vec<Uuid> = unique_ids.iter().copied().collect();

    let staff_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ANY($1) AND user_type = 'staff'")
            .bind(&ids_vec)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    if staff_count != unique_ids.len() as i64 {
        return Err(AppError::BadRequest(
            "ครูที่ปรึกษาต้องเป็นบุคลากร (Staff)".to_string(),
        ));
    }

    Ok(list.clone())
}

async fn insert_advisors(
    tx: &mut Transaction<'_, Postgres>,
    classroom_id: Uuid,
    advisors: &[ClassroomAdvisorInput],
) -> Result<(), AppError> {
    if advisors.is_empty() {
        return Ok(());
    }

    let user_ids: Vec<Uuid> = advisors.iter().map(|advisor| advisor.user_id).collect();
    let roles: Vec<String> = advisors
        .iter()
        .map(|advisor| advisor.role.clone())
        .collect();

    sqlx::query(
        "INSERT INTO classroom_advisors (classroom_id, user_id, role)
         SELECT $1, user_id, role
         FROM UNNEST($2::uuid[], $3::text[]) AS rows(user_id, role)
         ON CONFLICT (classroom_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(classroom_id)
    .bind(&user_ids)
    .bind(&roles)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn fetch_classroom_full(pool: &PgPool, id: Uuid) -> Result<Classroom, AppError> {
    sqlx::query_as::<_, Classroom>(
        "SELECT c.*,
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                ay.name as academic_year_label,
                (SELECT COUNT(*) FROM student_class_enrollments ske WHERE ske.class_room_id = c.id AND ske.status = 'active') as student_count,
                COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'user_id', ca.user_id,
                            'role', ca.role,
                            'name', CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)
                        ) ORDER BY ca.role, u.first_name
                    )
                    FROM classroom_advisors ca
                    JOIN users u ON u.id = ca.user_id
                    WHERE ca.classroom_id = c.id
                ), '[]'::jsonb) as advisors
         FROM class_rooms c
         JOIN grade_levels gl ON c.grade_level_id = gl.id
         JOIN academic_years ay ON c.academic_year_id = ay.id
         WHERE c.id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

async fn ensure_classroom_exists(pool: &PgPool, class_room_id: Uuid) -> Result<(), AppError> {
    sqlx::query_as::<_, Classroom>(
        "SELECT c.*, 
                CASE gl.level_type 
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                NULL::text as academic_year_label,
                NULL::bigint as student_count,
                '[]'::jsonb as advisors
         FROM class_rooms c
         JOIN grade_levels gl ON c.grade_level_id = gl.id
         WHERE c.id = $1",
    )
    .bind(class_room_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("Classroom not found".to_string()))?;

    Ok(())
}

async fn ensure_academic_year_exists(pool: &PgPool, year_id: Uuid) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_years WHERE id = $1)")
            .bind(year_id)
            .fetch_one(pool)
            .await?;

    if !exists {
        return Err(AppError::NotFound("Academic year not found".to_string()));
    }

    Ok(())
}

async fn assign_numbers_after_enrollment(
    pool: &PgPool,
    payload: &EnrollStudentRequest,
) -> Result<(), AppError> {
    let numbering_method = payload.numbering_method.as_deref().unwrap_or("append");

    match numbering_method {
        "none" => Ok(()),
        "append" => append_class_numbers(pool, payload).await,
        "student_code" | "name" | "gender_name" => {
            let students =
                sorted_students_for_numbering(pool, payload.class_room_id, numbering_method)
                    .await?;
            update_class_numbers(pool, &students).await
        }
        _ => Ok(()),
    }
}

async fn append_class_numbers(
    pool: &PgPool,
    payload: &EnrollStudentRequest,
) -> Result<(), AppError> {
    let max_number: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(class_number) FROM student_class_enrollments 
         WHERE class_room_id = $1 AND status = 'active'",
    )
    .bind(payload.class_room_id)
    .fetch_one(pool)
    .await
    .unwrap_or(None);

    let start_number = max_number.unwrap_or(0) + 1;

    let student_ids = ordered_unique_uuids(&payload.student_ids);
    let class_numbers: Vec<i32> = student_ids
        .iter()
        .enumerate()
        .map(|(index, _)| start_number + index as i32)
        .collect();
    bulk_update_class_numbers_by_student_ids(
        pool,
        payload.class_room_id,
        &student_ids,
        &class_numbers,
    )
    .await?;

    Ok(())
}

async fn bulk_update_class_numbers_by_student_ids(
    pool: &PgPool,
    class_room_id: Uuid,
    student_ids: &[Uuid],
    class_numbers: &[i32],
) -> Result<(), AppError> {
    if student_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "UPDATE student_class_enrollments AS enrollment
         SET class_number = updates.class_number, updated_at = NOW()
         FROM UNNEST($1::uuid[], $2::int4[]) AS updates(student_id, class_number)
         WHERE enrollment.student_id = updates.student_id
           AND enrollment.class_room_id = $3
           AND enrollment.status = 'active'",
    )
    .bind(student_ids)
    .bind(class_numbers)
    .bind(class_room_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn sorted_students_for_numbering(
    pool: &PgPool,
    class_id: Uuid,
    sort_by: &str,
) -> Result<Vec<StudentForNumbering>, AppError> {
    let students = sqlx::query_as::<_, StudentForNumbering>(
        "SELECT ske.id, s.student_id as student_code, u.first_name, u.title
         FROM student_class_enrollments ske
         LEFT JOIN users u ON ske.student_id = u.id
         LEFT JOIN student_info s ON u.id = s.user_id
         WHERE ske.class_room_id = $1 AND ske.status = 'active'",
    )
    .bind(class_id)
    .fetch_all(pool)
    .await?;

    sort_students_for_numbering(students, sort_by)
}

fn sort_students_for_numbering(
    mut students: Vec<StudentForNumbering>,
    sort_by: &str,
) -> Result<Vec<StudentForNumbering>, AppError> {
    match sort_by {
        "student_code" => {
            students.sort_by(|a, b| a.student_code.cmp(&b.student_code));
        }
        "name" => {
            students.sort_by(|a, b| a.first_name.cmp(&b.first_name));
        }
        "gender_name" => {
            students.sort_by(|a, b| {
                let a_male = is_male_title(&a.title);
                let b_male = is_male_title(&b.title);

                match (a_male, b_male) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.first_name.cmp(&b.first_name),
                }
            });
        }
        _ => {
            return Err(AppError::BadRequest(
                "Invalid sort_by parameter".to_string(),
            ));
        }
    }

    Ok(students)
}

fn classroom_identity(year: i32, grade_level: &GradeLevel, room_number: &str) -> ClassroomIdentity {
    let short_year = year % 100;
    ClassroomIdentity {
        name: format!("{}/{}", grade_level.short_name(), room_number),
        code: format!(
            "{}-{}-{}",
            short_year,
            grade_level.code(),
            room_number.replace(' ', "")
        ),
    }
}

async fn update_class_numbers(
    pool: &PgPool,
    students: &[StudentForNumbering],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let enrollment_ids: Vec<Uuid> = students.iter().map(|student| student.id).collect();
    let class_numbers: Vec<i32> = students
        .iter()
        .enumerate()
        .map(|(index, _)| (index + 1) as i32)
        .collect();
    bulk_update_class_numbers_by_enrollment_ids(&mut tx, &enrollment_ids, &class_numbers).await?;

    tx.commit().await?;
    Ok(())
}

async fn bulk_update_class_numbers_by_enrollment_ids(
    tx: &mut Transaction<'_, Postgres>,
    enrollment_ids: &[Uuid],
    class_numbers: &[i32],
) -> Result<(), AppError> {
    if enrollment_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "UPDATE student_class_enrollments AS enrollment
         SET class_number = updates.class_number, updated_at = NOW()
         FROM UNNEST($1::uuid[], $2::int4[]) AS updates(id, class_number)
         WHERE enrollment.id = updates.id",
    )
    .bind(enrollment_ids)
    .bind(class_numbers)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn is_male_title(title: &Option<String>) -> bool {
    matches!(title.as_deref(), Some("เด็กชาย" | "นาย"))
}

fn classroom_create_error(error: sqlx::Error) -> AppError {
    let message = error.to_string();
    if message.contains("unique constraint") {
        AppError::BadRequest("ห้องเรียนนี้มีอยู่แล้วในระบบ".to_string())
    } else if message.contains("violates foreign key constraint") {
        AppError::BadRequest("ข้อมูลอ้างอิงไม่ถูกต้อง (FK Violation)".to_string())
    } else {
        AppError::from(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn student(student_code: &str, first_name: &str, title: Option<&str>) -> StudentForNumbering {
        StudentForNumbering {
            id: Uuid::new_v4(),
            student_code: student_code.to_string(),
            first_name: first_name.to_string(),
            title: title.map(str::to_string),
        }
    }

    #[test]
    fn classroom_identity_uses_short_year_grade_code_and_trimmed_room_number_for_code() {
        let grade_level = GradeLevel {
            id: Uuid::new_v4(),
            level_type: "secondary".to_string(),
            year: 1,
            next_grade_level_id: None,
            is_active: true,
        };

        let identity = classroom_identity(2567, &grade_level, "EP 1");

        assert_eq!(identity.name, "ม.1/EP 1");
        assert_eq!(identity.code, "67-M1-EP1");
    }

    #[test]
    fn classroom_identity_uses_grade_level_display_for_primary_and_kindergarten() {
        let primary = GradeLevel {
            id: Uuid::new_v4(),
            level_type: "primary".to_string(),
            year: 4,
            next_grade_level_id: None,
            is_active: true,
        };
        let kindergarten = GradeLevel {
            id: Uuid::new_v4(),
            level_type: "kindergarten".to_string(),
            year: 2,
            next_grade_level_id: None,
            is_active: true,
        };

        let primary_identity = classroom_identity(2568, &primary, "1");
        let kindergarten_identity = classroom_identity(2568, &kindergarten, "2");

        assert_eq!(primary_identity.name, "ป.4/1");
        assert_eq!(primary_identity.code, "68-P4-1");
        assert_eq!(kindergarten_identity.name, "อ.2/2");
        assert_eq!(kindergarten_identity.code, "68-K2-2");
    }

    #[test]
    fn sort_students_for_numbering_orders_by_student_code() {
        let students = vec![
            student("S003", "Gamma", None),
            student("S001", "Alpha", None),
            student("S002", "Beta", None),
        ];

        let sorted = sort_students_for_numbering(students, "student_code").unwrap();

        let codes: Vec<_> = sorted.iter().map(|row| row.student_code.as_str()).collect();
        assert_eq!(codes, vec!["S001", "S002", "S003"]);
    }

    #[test]
    fn sort_students_for_numbering_orders_by_name() {
        let students = vec![
            student("S003", "วิชัย", Some("นาย")),
            student("S001", "กมล", Some("เด็กชาย")),
            student("S002", "สมหญิง", Some("เด็กหญิง")),
        ];

        let sorted = sort_students_for_numbering(students, "name").unwrap();

        let names: Vec<_> = sorted.iter().map(|row| row.first_name.as_str()).collect();
        assert_eq!(names, vec!["กมล", "วิชัย", "สมหญิง"]);
    }

    #[test]
    fn sort_students_for_numbering_places_male_titles_before_other_titles_then_name() {
        let students = vec![
            student("S001", "มาลี", Some("เด็กหญิง")),
            student("S002", "ก้อง", Some("เด็กชาย")),
            student("S003", "บอย", Some("นาย")),
            student("S004", "อร", Some("นางสาว")),
        ];

        let sorted = sort_students_for_numbering(students, "gender_name").unwrap();

        let names: Vec<_> = sorted.iter().map(|row| row.first_name.as_str()).collect();
        assert_eq!(names, vec!["ก้อง", "บอย", "มาลี", "อร"]);
    }

    #[test]
    fn sort_students_for_numbering_rejects_unknown_method() {
        let result = sort_students_for_numbering(Vec::new(), "random");

        assert!(
            matches!(result, Err(AppError::BadRequest(message)) if message == "Invalid sort_by parameter")
        );
    }

    #[test]
    fn ordered_unique_uuids_preserves_first_seen_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert_eq!(
            ordered_unique_uuids(&[first, second, first]),
            vec![first, second]
        );
    }

    #[test]
    fn is_male_title_accepts_only_supported_male_titles() {
        assert!(is_male_title(&Some("เด็กชาย".to_string())));
        assert!(is_male_title(&Some("นาย".to_string())));
        assert!(!is_male_title(&Some("เด็กหญิง".to_string())));
        assert!(!is_male_title(&Some("นางสาว".to_string())));
        assert!(!is_male_title(&None));
    }
}
