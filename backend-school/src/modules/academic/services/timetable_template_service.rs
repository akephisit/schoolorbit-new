use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::academic::handlers::timetable_templates::{TimetableTemplateEntry, TimetableTemplateView};

pub async fn list_templates(pool: &PgPool) -> Result<Vec<TimetableTemplateView>, AppError> {
    sqlx::query_as::<_, TimetableTemplateView>(
        r#"SELECT t.id, t.name, t.description, t.created_by, t.created_at, t.updated_at,
                  COUNT(e.id) AS entry_count
           FROM timetable_templates t
           LEFT JOIN timetable_template_entries e ON e.template_id = t.id
           GROUP BY t.id
           ORDER BY t.created_at DESC"#
    )
    .fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn get_template(pool: &PgPool, id: Uuid) -> Result<(TimetableTemplateView, Vec<TimetableTemplateEntry>), AppError> {
    let template = sqlx::query_as::<_, TimetableTemplateView>(
        r#"SELECT t.id, t.name, t.description, t.created_by, t.created_at, t.updated_at,
                  (SELECT COUNT(*) FROM timetable_template_entries WHERE template_id = t.id) AS entry_count
           FROM timetable_templates t WHERE t.id = $1"#
    )
    .bind(id).fetch_optional(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Template not found".to_string()))?;

    let entries = sqlx::query_as::<_, TimetableTemplateEntry>(
        "SELECT * FROM timetable_template_entries WHERE template_id = $1 ORDER BY day_of_week, period_id"
    )
    .bind(id).fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok((template, entries))
}

pub async fn create_template(pool: &PgPool, name: &str, description: Option<&str>, user_id: Option<Uuid>) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO timetable_templates (name, description, created_by) VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(name).bind(description).bind(user_id)
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(row.0)
}

pub async fn update_template(pool: &PgPool, id: Uuid, name: Option<&str>, description: Option<&str>) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE timetable_templates SET name = COALESCE($2, name), description = COALESCE($3, description), updated_at = NOW() WHERE id = $1"
    )
    .bind(id).bind(name).bind(description)
    .execute(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn delete_template(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM timetable_templates WHERE id = $1")
        .bind(id).execute(pool).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn from_current(
    pool: &PgPool,
    semester_id: Uuid,
    name: &str,
    description: Option<&str>,
    entry_types: Vec<String>,
    user_id: Option<Uuid>,
) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let template_id: Uuid = sqlx::query_scalar(
        "INSERT INTO timetable_templates (name, description, created_by) VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(name).bind(description).bind(user_id)
    .fetch_one(&mut *tx).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"WITH grouped AS (
            SELECT te.day_of_week, te.period_id, te.entry_type, te.title, te.room_id,
                   (te.classroom_id IS NULL) AS is_instructor_only,
                   ARRAY_AGG(DISTINCT te.classroom_id) FILTER (WHERE te.classroom_id IS NOT NULL) AS classroom_ids,
                   ARRAY_AGG(DISTINCT tei.instructor_id) FILTER (WHERE tei.instructor_id IS NOT NULL) AS instructor_ids
            FROM academic_timetable_entries te
            LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
            WHERE te.academic_semester_id = $1 AND te.is_active = true
              AND te.entry_type = ANY($2) AND te.activity_slot_id IS NULL
            GROUP BY te.day_of_week, te.period_id, te.entry_type, te.title, te.room_id, (te.classroom_id IS NULL)
        )
        INSERT INTO timetable_template_entries
            (template_id, day_of_week, period_id, entry_type, title,
             activity_slot_id, classroom_ids, instructor_ids, room_id)
        SELECT $3, g.day_of_week, g.period_id, g.entry_type, g.title, NULL,
               COALESCE(to_jsonb(g.classroom_ids), '[]'::jsonb),
               COALESCE(to_jsonb(g.instructor_ids), '[]'::jsonb),
               g.room_id
        FROM grouped g
        WHERE COALESCE(array_length(g.classroom_ids, 1), 0) > 0
           OR COALESCE(array_length(g.instructor_ids, 1), 0) > 0"#
    )
    .bind(semester_id).bind(&entry_types).bind(template_id)
    .execute(&mut *tx).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(template_id)
}

pub async fn apply_template(
    pool: &PgPool,
    template_id: Uuid,
    semester_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<u64, AppError> {
    let entries = sqlx::query_as::<_, TimetableTemplateEntry>(
        "SELECT * FROM timetable_template_entries WHERE template_id = $1"
    )
    .bind(template_id).fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if entries.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let mut total_inserted: u64 = 0;

    use std::collections::HashMap;
    let mut group_batch_ids: HashMap<(Option<String>, Option<Uuid>), Uuid> = HashMap::new();

    for entry in &entries {
        let specific_classrooms: Vec<Uuid> = serde_json::from_value(entry.classroom_ids.clone()).unwrap_or_default();
        let grade_level_ids: Vec<Uuid> = serde_json::from_value(entry.grade_level_ids.clone()).unwrap_or_default();
        let instructor_ids: Vec<Uuid> = serde_json::from_value(entry.instructor_ids.clone()).unwrap_or_default();

        let mut resolved_classrooms: Vec<Uuid> = if !grade_level_ids.is_empty() {
            sqlx::query_scalar(
                r#"SELECT cr.id FROM class_rooms cr
                   JOIN academic_semesters s ON s.academic_year_id = cr.academic_year_id
                   WHERE s.id = $1 AND cr.grade_level_id = ANY($2)"#
            )
            .bind(semester_id).bind(&grade_level_ids)
            .fetch_all(&mut *tx).await.unwrap_or_default()
        } else { Vec::new() };

        for c in specific_classrooms {
            if !resolved_classrooms.contains(&c) {
                resolved_classrooms.push(c);
            }
        }

        let group_key = (entry.title.clone(), entry.activity_slot_id);
        let batch_uuid = *group_batch_ids.entry(group_key).or_insert_with(Uuid::new_v4);

        if !resolved_classrooms.is_empty() {
            let inserted_count: u64 = sqlx::query(
                r#"INSERT INTO academic_timetable_entries
                       (id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                        entry_type, title, is_active, created_by, updated_by,
                        classroom_course_id, note, activity_slot_id, batch_id)
                   SELECT gen_random_uuid(), c, $1, $2, $3, $4, $5, $6, true, $7, $7,
                          NULL, NULL, $8, $9
                   FROM UNNEST($10::uuid[]) AS c
                   ON CONFLICT DO NOTHING"#
            )
            .bind(semester_id).bind(&entry.day_of_week).bind(entry.period_id).bind(entry.room_id)
            .bind(&entry.entry_type).bind(&entry.title).bind(user_id)
            .bind(entry.activity_slot_id).bind(batch_uuid).bind(&resolved_classrooms)
            .execute(&mut *tx).await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?
            .rows_affected();

            total_inserted += inserted_count;

            if !instructor_ids.is_empty() {
                sqlx::query(
                    r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                       SELECT te.id, instr.v, 'primary'
                       FROM academic_timetable_entries te
                       CROSS JOIN UNNEST($1::uuid[]) AS instr(v)
                       WHERE te.batch_id = $2 AND te.day_of_week = $3
                         AND te.period_id = $4 AND te.classroom_id IS NOT NULL
                       ON CONFLICT DO NOTHING"#
                )
                .bind(&instructor_ids).bind(batch_uuid)
                .bind(&entry.day_of_week).bind(entry.period_id)
                .execute(&mut *tx).await
                .map_err(|e| AppError::InternalServerError(e.to_string()))?;
            }
        } else if !instructor_ids.is_empty() {
            let entry_ids: Vec<Uuid> = (0..instructor_ids.len()).map(|_| Uuid::new_v4()).collect();

            let inserted_count: u64 = sqlx::query(
                r#"INSERT INTO academic_timetable_entries
                       (id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                        entry_type, title, is_active, created_by, updated_by,
                        classroom_course_id, note, activity_slot_id, batch_id)
                   SELECT id, NULL, $1, $2, $3, $4, $5, $6, true, $7, $7,
                          NULL, NULL, NULL, $8
                   FROM UNNEST($9::uuid[]) AS t(id)"#
            )
            .bind(semester_id).bind(&entry.day_of_week).bind(entry.period_id).bind(entry.room_id)
            .bind(&entry.entry_type).bind(&entry.title).bind(user_id)
            .bind(batch_uuid).bind(&entry_ids)
            .execute(&mut *tx).await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?
            .rows_affected();

            total_inserted += inserted_count;

            sqlx::query(
                r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                   SELECT eid, iid, 'primary'
                   FROM UNNEST($1::uuid[], $2::uuid[]) AS t(eid, iid)
                   ON CONFLICT DO NOTHING"#
            )
            .bind(&entry_ids).bind(&instructor_ids)
            .execute(&mut *tx).await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        }
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(total_inserted)
}

pub async fn clear_timetable(pool: &PgPool, semester_id: Uuid, entry_types: Vec<String>) -> Result<u64, AppError> {
    let result = sqlx::query(
        "DELETE FROM academic_timetable_entries WHERE academic_semester_id = $1 AND entry_type = ANY($2)"
    )
    .bind(semester_id).bind(&entry_types)
    .execute(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(result.rows_affected())
}
