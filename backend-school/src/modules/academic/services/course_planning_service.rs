use crate::error::AppError;
use crate::modules::academic::models::course_planning::{
    AssignCoursesRequest, ClassroomCourse, CourseInstructor, OptionalUuidPatch, PlanQuery,
    UpdateCourseRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ClassroomActivity {
    pub slot_id: Uuid,
    pub activity_catalog_id: Uuid,
    pub name: String,
    pub activity_type: String,
    pub periods_per_week: i32,
    pub scheduling_mode: String,
    pub is_active: bool,
}

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

    if query.classroom_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.classroom_id = ${idx}"));
    }
    if query.instructor_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM classroom_course_instructors cci \
               WHERE cci.classroom_course_id = cc.id AND cci.instructor_id = ${idx})"
        ));
    }
    if query.academic_semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.academic_semester_id = ${idx}"));
    }
    if query.subject_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND cc.subject_id = ${idx}"));
    }

    if !plan_query_has_course_filter(query) {
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
        tracing::error!("Failed to fetch courses: {}", e);
        AppError::InternalServerError("Failed to fetch courses".to_string())
    })
}

pub async fn assign_courses(pool: &PgPool, payload: AssignCoursesRequest) -> Result<i64, AppError> {
    let subject_ids = ordered_unique_ids(payload.subject_ids);
    let mut tx = pool.begin().await?;

    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM class_rooms WHERE id = $1 FOR KEY SHARE")
        .bind(payload.classroom_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Classroom not found".to_string()));
    }
    if sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM academic_semesters WHERE id = $1 FOR KEY SHARE",
    )
    .bind(payload.academic_semester_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_none()
    {
        return Err(AppError::NotFound(
            "Academic semester not found".to_string(),
        ));
    }
    if !subject_ids.is_empty() {
        let existing_subject_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM subjects WHERE id = ANY($1)")
                .bind(&subject_ids)
                .fetch_one(&mut *tx)
                .await?;
        if existing_subject_count != subject_ids.len() as i64 {
            return Err(AppError::NotFound(
                "One or more subjects were not found".to_string(),
            ));
        }
    }

    let inserted = sqlx::query_scalar(
        r#"WITH inserted AS (
               INSERT INTO classroom_courses (classroom_id, academic_semester_id, subject_id, primary_instructor_id)
               SELECT $1, $2, s.id,
                      (SELECT sdi.instructor_id
                       FROM subject_default_instructors sdi
                       WHERE sdi.subject_id = s.id
                       ORDER BY (sdi.role = 'primary') DESC, sdi.created_at ASC
                       LIMIT 1)
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
    .bind(payload.classroom_id)
    .bind(payload.academic_semester_id)
    .bind(&subject_ids)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(inserted)
}

pub async fn remove_course(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM classroom_courses WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Classroom course not found".to_string()));
    }
    Ok(())
}

pub async fn update_course(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateCourseRequest,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let current_primary = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT primary_instructor_id FROM classroom_courses WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Classroom course not found".to_string()))?;

    if let Some(settings) = payload.settings {
        sqlx::query("UPDATE classroom_courses SET settings = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(sqlx::types::Json(settings))
            .execute(&mut *tx)
            .await?;
    }

    match payload.primary_instructor_id {
        OptionalUuidPatch::Unspecified => {}
        OptionalUuidPatch::Null => {
            if let Some(instructor_id) = current_primary {
                sqlx::query(
                    "DELETE FROM classroom_course_instructors
                     WHERE classroom_course_id = $1 AND instructor_id = $2",
                )
                .bind(id)
                .bind(instructor_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "DELETE FROM timetable_entry_instructors tei
                     USING academic_timetable_entries te
                     WHERE tei.entry_id = te.id
                       AND te.classroom_course_id = $1
                       AND tei.instructor_id = $2",
                )
                .bind(id)
                .bind(instructor_id)
                .execute(&mut *tx)
                .await?;
                reconcile_course_primary_role(&mut tx, id).await?;
                sync_existing_timetable_instructor_roles(&mut tx, id).await?;
                upsert_course_primary_timetable_assignments(&mut tx, id).await?;
            }
        }
        OptionalUuidPatch::Value(instructor_id) => {
            if sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id = $1 FOR KEY SHARE")
                .bind(instructor_id)
                .fetch_optional(&mut *tx)
                .await?
                .is_none()
            {
                return Err(AppError::NotFound("Instructor not found".to_string()));
            }
            sqlx::query(
                "INSERT INTO classroom_course_instructors
                    (classroom_course_id, instructor_id, role)
                 VALUES ($1, $2, 'primary')
                 ON CONFLICT (classroom_course_id, instructor_id)
                 DO UPDATE SET role = 'primary'",
            )
            .bind(id)
            .bind(instructor_id)
            .execute(&mut *tx)
            .await?;
            reconcile_course_primary_role(&mut tx, id).await?;
            sync_existing_timetable_instructor_roles(&mut tx, id).await?;
            upsert_course_instructor_timetable_assignments(&mut tx, id, instructor_id).await?;
        }
    }

    tx.commit().await?;
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

    Ok(group_course_instructors(rows))
}

pub async fn list_course_instructors(
    pool: &PgPool,
    course_id: Uuid,
) -> Result<Vec<CourseInstructor>, AppError> {
    ensure_course_exists(pool, course_id).await?;
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
    validate_course_instructor_role(role)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    ensure_course_exists_in_tx(&mut tx, course_id).await?;
    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id = $1 FOR KEY SHARE")
        .bind(instructor_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Instructor not found".to_string()));
    }

    sqlx::query(
        "INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (classroom_course_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await?;

    reconcile_course_primary_role(&mut tx, course_id).await?;
    sync_existing_timetable_instructor_roles(&mut tx, course_id).await?;
    upsert_course_instructor_timetable_assignments(&mut tx, course_id, instructor_id).await?;

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

    ensure_course_exists_in_tx(&mut tx, course_id).await?;
    let current_primary: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(course_id)
            .fetch_one(&mut *tx)
            .await?;

    let result = sqlx::query(
        "DELETE FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(course_id)
    .bind(instructor_id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Course instructor assignment not found".to_string(),
        ));
    }

    sqlx::query(
        "DELETE FROM timetable_entry_instructors tei
         USING academic_timetable_entries te
         WHERE tei.entry_id = te.id AND te.classroom_course_id = $1 AND tei.instructor_id = $2",
    )
    .bind(course_id)
    .bind(instructor_id)
    .execute(&mut *tx)
    .await?;

    reconcile_course_primary_role(&mut tx, course_id).await?;
    sync_existing_timetable_instructor_roles(&mut tx, course_id).await?;
    if current_primary == Some(instructor_id) {
        upsert_course_primary_timetable_assignments(&mut tx, course_id).await?;
    }

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
    validate_course_instructor_role(role)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    ensure_course_exists_in_tx(&mut tx, course_id).await?;
    let current_primary: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(course_id)
            .fetch_one(&mut *tx)
            .await?;
    if sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2 FOR UPDATE",
    )
    .bind(course_id)
    .bind(instructor_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_none()
    {
        return Err(AppError::NotFound(
            "Course instructor assignment not found".to_string(),
        ));
    }

    let replacement_primary = if role == "secondary" && current_primary == Some(instructor_id) {
        Some(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT instructor_id FROM classroom_course_instructors
                 WHERE classroom_course_id = $1 AND instructor_id <> $2
                 ORDER BY created_at, instructor_id
                 LIMIT 1
                 FOR UPDATE",
            )
            .bind(course_id)
            .bind(instructor_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                AppError::Conflict(
                    "A course team must keep one primary instructor while it is not empty"
                        .to_string(),
                )
            })?,
        )
    } else {
        None
    };

    sqlx::query(
        "UPDATE classroom_course_instructors SET role = $3
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await?;

    if let Some(replacement_id) = replacement_primary {
        sqlx::query(
            "UPDATE classroom_course_instructors SET role = 'primary'
             WHERE classroom_course_id = $1 AND instructor_id = $2",
        )
        .bind(course_id)
        .bind(replacement_id)
        .execute(&mut *tx)
        .await?;
    }

    reconcile_course_primary_role(&mut tx, course_id).await?;
    sync_existing_timetable_instructor_roles(&mut tx, course_id).await?;
    if role == "primary" {
        upsert_course_instructor_timetable_assignments(&mut tx, course_id, instructor_id).await?;
    }
    if let Some(replacement_id) = replacement_primary {
        upsert_course_instructor_timetable_assignments(&mut tx, course_id, replacement_id).await?;
    }

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

pub async fn list_classroom_activities(
    pool: &PgPool,
    classroom_id: Uuid,
    semester_id: Uuid,
) -> Result<Vec<ClassroomActivity>, AppError> {
    ensure_classroom_exists(pool, classroom_id).await?;
    ensure_semester_exists(pool, semester_id).await?;
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
        tracing::error!("list_classroom_activities error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })
}

pub async fn remove_classroom_from_slot(
    pool: &PgPool,
    classroom_id: Uuid,
    slot_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "DELETE FROM activity_slot_classrooms WHERE classroom_id = $1 AND slot_id = $2",
    )
    .bind(classroom_id)
    .bind(slot_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Classroom activity assignment not found".to_string(),
        ));
    }
    Ok(())
}

fn ordered_unique_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

fn validate_course_instructor_role(role: &str) -> Result<(), AppError> {
    if role == "primary" || role == "secondary" {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "role must be 'primary' or 'secondary'".to_string(),
        ))
    }
}

async fn ensure_course_exists(pool: &PgPool, course_id: Uuid) -> Result<(), AppError> {
    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM classroom_courses WHERE id = $1")
        .bind(course_id)
        .fetch_optional(pool)
        .await?
        .is_some()
    {
        Ok(())
    } else {
        Err(AppError::NotFound("Classroom course not found".to_string()))
    }
}

async fn ensure_course_exists_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_id: Uuid,
) -> Result<(), AppError> {
    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM classroom_courses WHERE id = $1 FOR UPDATE")
        .bind(course_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some()
    {
        Ok(())
    } else {
        Err(AppError::NotFound("Classroom course not found".to_string()))
    }
}

async fn reconcile_course_primary_role(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE classroom_course_instructors cci
         SET role = 'primary'
         FROM classroom_courses cc
         WHERE cc.id = $1
           AND cci.classroom_course_id = cc.id
           AND cci.instructor_id = cc.primary_instructor_id
           AND cci.role IS DISTINCT FROM 'primary'",
    )
    .bind(course_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn sync_existing_timetable_instructor_roles(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE timetable_entry_instructors tei
         SET role = cci.role
         FROM academic_timetable_entries te, classroom_course_instructors cci
         WHERE tei.entry_id = te.id
           AND te.classroom_course_id = $1
           AND cci.classroom_course_id = $1
           AND cci.instructor_id = tei.instructor_id
           AND tei.role IS DISTINCT FROM cci.role",
    )
    .bind(course_id)
    .execute(&mut **tx)
    .await
    .map_err(map_timetable_conflict)?;
    Ok(())
}

async fn upsert_course_instructor_timetable_assignments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_id: Uuid,
    instructor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         SELECT te.id, cci.instructor_id, cci.role
         FROM academic_timetable_entries te
         JOIN classroom_course_instructors cci
           ON cci.classroom_course_id = te.classroom_course_id
          AND cci.instructor_id = $2
         WHERE te.classroom_course_id = $1
         ON CONFLICT (entry_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(course_id)
    .bind(instructor_id)
    .execute(&mut **tx)
    .await
    .map_err(map_timetable_conflict)?;
    Ok(())
}

async fn upsert_course_primary_timetable_assignments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_id: Uuid,
) -> Result<(), AppError> {
    let primary_instructor_id: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(course_id)
            .fetch_one(&mut **tx)
            .await?;
    if let Some(instructor_id) = primary_instructor_id {
        upsert_course_instructor_timetable_assignments(tx, course_id, instructor_id).await?;
    }
    Ok(())
}

fn map_timetable_conflict(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23514") {
            return AppError::Conflict(
                "Instructor has another timetable entry in the same period".to_string(),
            );
        }
    }
    AppError::DbError(error)
}

async fn ensure_classroom_exists(pool: &PgPool, classroom_id: Uuid) -> Result<(), AppError> {
    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM class_rooms WHERE id = $1")
        .bind(classroom_id)
        .fetch_optional(pool)
        .await?
        .is_some()
    {
        Ok(())
    } else {
        Err(AppError::NotFound("Classroom not found".to_string()))
    }
}

async fn ensure_semester_exists(pool: &PgPool, semester_id: Uuid) -> Result<(), AppError> {
    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM academic_semesters WHERE id = $1")
        .bind(semester_id)
        .fetch_optional(pool)
        .await?
        .is_some()
    {
        Ok(())
    } else {
        Err(AppError::NotFound(
            "Academic semester not found".to_string(),
        ))
    }
}

fn plan_query_has_course_filter(query: &PlanQuery) -> bool {
    query.classroom_id.is_some()
        || query.instructor_id.is_some()
        || query.academic_semester_id.is_some()
        || query.subject_id.is_some()
}

fn group_course_instructors(
    rows: Vec<CourseInstructor>,
) -> std::collections::HashMap<Uuid, Vec<CourseInstructor>> {
    let mut grouped: std::collections::HashMap<Uuid, Vec<CourseInstructor>> =
        std::collections::HashMap::new();
    for row in rows {
        grouped
            .entry(row.classroom_course_id)
            .or_default()
            .push(row);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course_instructor(classroom_course_id: Uuid, role: &str) -> CourseInstructor {
        CourseInstructor {
            id: Uuid::new_v4(),
            classroom_course_id,
            instructor_id: Uuid::new_v4(),
            role: role.to_string(),
            created_at: chrono::Utc::now(),
            instructor_name: None,
        }
    }

    #[test]
    fn plan_query_has_course_filter_when_any_filter_is_present() {
        assert!(!plan_query_has_course_filter(&PlanQuery {
            classroom_id: None,
            instructor_id: None,
            academic_semester_id: None,
            subject_id: None,
        }));
        assert!(plan_query_has_course_filter(&PlanQuery {
            classroom_id: Some(Uuid::new_v4()),
            instructor_id: None,
            academic_semester_id: None,
            subject_id: None,
        }));
        assert!(plan_query_has_course_filter(&PlanQuery {
            classroom_id: None,
            instructor_id: Some(Uuid::new_v4()),
            academic_semester_id: None,
            subject_id: None,
        }));
        assert!(plan_query_has_course_filter(&PlanQuery {
            classroom_id: None,
            instructor_id: None,
            academic_semester_id: Some(Uuid::new_v4()),
            subject_id: None,
        }));
        assert!(plan_query_has_course_filter(&PlanQuery {
            classroom_id: None,
            instructor_id: None,
            academic_semester_id: None,
            subject_id: Some(Uuid::new_v4()),
        }));
    }

    #[test]
    fn group_course_instructors_groups_rows_by_classroom_course_id() {
        let course_a = Uuid::new_v4();
        let course_b = Uuid::new_v4();
        let grouped = group_course_instructors(vec![
            course_instructor(course_a, "primary"),
            course_instructor(course_a, "secondary"),
            course_instructor(course_b, "primary"),
        ]);

        assert_eq!(grouped[&course_a].len(), 2);
        assert_eq!(grouped[&course_b].len(), 1);
    }

    #[test]
    fn group_course_instructors_preserves_row_order_inside_each_course() {
        let course_id = Uuid::new_v4();
        let grouped = group_course_instructors(vec![
            course_instructor(course_id, "primary"),
            course_instructor(course_id, "secondary"),
            course_instructor(course_id, "assistant"),
        ]);

        let roles: Vec<_> = grouped[&course_id]
            .iter()
            .map(|instructor| instructor.role.as_str())
            .collect();
        assert_eq!(roles, vec!["primary", "secondary", "assistant"]);
    }
}
