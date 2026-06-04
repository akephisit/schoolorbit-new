use crate::error::AppError;
use crate::modules::academic::models::course_planning::{
    AssignCoursesRequest, ClassroomCourse, CourseInstructor, PlanQuery, UpdateCourseRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_classroom_courses(
    pool: &PgPool,
    query: &PlanQuery,
) -> Result<Vec<ClassroomCourse>, AppError> {
    let mut sql = String::from(
        r#"SELECT cc.*, s.code as subject_code, s.name_th as subject_name_th,
                  s.name_en as subject_name_en, s.credit as subject_credit,
                  s.hours_per_semester as subject_hours, s.type as subject_type,
                  concat(u.first_name, ' ', u.last_name) as instructor_name,
                  cr.name as classroom_name
           FROM classroom_courses cc
           JOIN subjects s ON cc.subject_id = s.id
           LEFT JOIN users u ON cc.primary_instructor_id = u.id
           JOIN class_rooms cr ON cc.classroom_id = cr.id
           WHERE 1=1"#,
    );

    let mut idx = 0u32;
    let mut has_filter = false;

    if query.classroom_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.classroom_id = ${idx}"));
        has_filter = true;
    }
    if query.instructor_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM classroom_course_instructors cci \
               WHERE cci.classroom_course_id = cc.id AND cci.instructor_id = ${idx})"
        ));
        has_filter = true;
    }
    if query.academic_semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.academic_semester_id = ${idx}"));
        has_filter = true;
    }
    if query.subject_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.subject_id = ${idx}"));
        has_filter = true;
    }

    if !has_filter {
        return Ok(Vec::new());
    }

    sql.push_str(" ORDER BY s.code ASC");

    let mut q = sqlx::query_as::<_, ClassroomCourse>(&sql);
    if let Some(id) = query.classroom_id {
        q = q.bind(id);
    }
    if let Some(id) = query.instructor_id {
        q = q.bind(id);
    }
    if let Some(id) = query.academic_semester_id {
        q = q.bind(id);
    }
    if let Some(id) = query.subject_id {
        q = q.bind(id);
    }

    q.fetch_all(pool).await.map_err(|e| {
        eprintln!("Failed to fetch courses: {}", e);
        AppError::InternalServerError("Failed to fetch courses".to_string())
    })
}

pub async fn assign_courses(pool: &PgPool, payload: AssignCoursesRequest) -> Result<i64, AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM class_rooms WHERE id = $1)")
        .bind(payload.classroom_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

    if !exists {
        return Err(AppError::NotFound("Classroom not found".to_string()));
    }

    sqlx::query_scalar(
        r#"WITH inserted AS (
               INSERT INTO classroom_courses (classroom_id, academic_semester_id, subject_id, primary_instructor_id)
               SELECT $1, $2, s.id,
                      COALESCE(
                          (SELECT sdi.instructor_id FROM subject_default_instructors sdi
                           WHERE sdi.subject_id = s.id AND sdi.role = 'primary' LIMIT 1),
                          s.default_instructor_id
                      )
               FROM subjects s WHERE s.id = ANY($3)
               ON CONFLICT (classroom_id, academic_semester_id, subject_id) DO NOTHING
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
           SELECT COUNT(*) FROM inserted"#
    )
    .bind(payload.classroom_id).bind(payload.academic_semester_id).bind(&payload.subject_ids)
    .fetch_one(pool).await
    .map_err(|e| { eprintln!("assign_courses failed: {}", e); AppError::InternalServerError("Failed to assign courses".to_string()) })
}

pub async fn remove_course(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM classroom_courses WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to remove course".to_string()))?;
    Ok(())
}

pub async fn update_course(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateCourseRequest,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE classroom_courses SET
            primary_instructor_id = COALESCE($1, primary_instructor_id),
            settings = COALESCE($2, settings),
            updated_at = NOW()
           WHERE id = $3"#,
    )
    .bind(payload.primary_instructor_id)
    .bind(payload.settings)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Update error: {}", e);
        AppError::InternalServerError("Failed to update course".to_string())
    })?;
    Ok(())
}

pub async fn batch_list_course_instructors(
    pool: &PgPool,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<CourseInstructor>>, AppError> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<CourseInstructor> = sqlx::query_as(
        r#"SELECT cci.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM classroom_course_instructors cci
           JOIN users u ON u.id = cci.instructor_id
           WHERE cci.classroom_course_id = ANY($1)
           ORDER BY cci.classroom_course_id, cci.role, cci.created_at"#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut grouped: std::collections::HashMap<Uuid, Vec<CourseInstructor>> =
        std::collections::HashMap::new();
    for row in rows {
        grouped
            .entry(row.classroom_course_id)
            .or_default()
            .push(row);
    }
    Ok(grouped)
}

pub async fn list_course_instructors(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<Vec<CourseInstructor>, AppError> {
    sqlx::query_as(
        r#"SELECT cci.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM classroom_course_instructors cci
           JOIN users u ON u.id = cci.instructor_id
           WHERE cci.classroom_course_id = $1
           ORDER BY cci.role, cci.created_at"#,
    )
    .bind(course_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn add_course_instructor(
    pool: &PgPool,
    course_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        "INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (classroom_course_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
           SELECT te.id, $2, $3
           FROM academic_timetable_entries te
           WHERE te.classroom_course_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM academic_timetable_entries te2
                 JOIN timetable_entry_instructors tei2 ON tei2.entry_id = te2.id
                 WHERE tei2.instructor_id = $2
                   AND te2.day_of_week = te.day_of_week
                   AND te2.period_id = te.period_id
                   AND te2.id <> te.id
             )
           ON CONFLICT (entry_id, instructor_id) DO UPDATE SET role = EXCLUDED.role"#,
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn remove_course_instructor(
    pool: &PgPool,
    course_id: Uuid,
    instructor_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query("DELETE FROM classroom_course_instructors WHERE classroom_course_id = $1 AND instructor_id = $2")
        .bind(course_id).bind(instructor_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        "DELETE FROM timetable_entry_instructors tei
         USING academic_timetable_entries te
         WHERE tei.entry_id = te.id AND te.classroom_course_id = $1 AND tei.instructor_id = $2",
    )
    .bind(course_id)
    .bind(instructor_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn update_course_instructor_role(
    pool: &PgPool,
    course_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        "UPDATE classroom_course_instructors SET role = $3
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        "UPDATE timetable_entry_instructors SET role = $3
         FROM academic_timetable_entries te
         WHERE timetable_entry_instructors.entry_id = te.id
           AND te.classroom_course_id = $1
           AND timetable_entry_instructors.instructor_id = $2",
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn get_course_semester_id(pool: &PgPool, course_id: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT academic_semester_id FROM classroom_courses WHERE id = $1")
        .bind(course_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

use crate::modules::academic::handlers::course_planning::ClassroomActivity;

pub async fn list_classroom_activities(
    pool: &PgPool,
    classroom_id: Uuid,
    semester_id: Uuid,
) -> Result<Vec<ClassroomActivity>, AppError> {
    sqlx::query_as(
        r#"SELECT s.id AS slot_id, s.activity_catalog_id,
                  ac.name, ac.activity_type, ac.periods_per_week, ac.scheduling_mode,
                  s.is_active
           FROM activity_slot_classrooms asc_row
           JOIN activity_slots s ON s.id = asc_row.slot_id
           JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
           WHERE asc_row.classroom_id = $1 AND s.semester_id = $2
           ORDER BY ac.activity_type, ac.name"#,
    )
    .bind(classroom_id)
    .bind(semester_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("list_classroom_activities error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })
}

pub async fn remove_classroom_from_slot(
    pool: &PgPool,
    classroom_id: Uuid,
    slot_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM activity_slot_classrooms WHERE classroom_id = $1 AND slot_id = $2")
        .bind(classroom_id)
        .bind(slot_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}
