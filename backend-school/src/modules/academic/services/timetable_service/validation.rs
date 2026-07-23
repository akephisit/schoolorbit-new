use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    ConflictInfo, CreateTimetableEntryRequest, TimetableValidationResponse,
};
use sqlx::PgPool;
use uuid::Uuid;

/// ตรวจ conflict ของ entry ที่กำลังจะสร้าง (instructor + classroom + room)
pub async fn validate_entry(
    pool: &PgPool,
    payload: &CreateTimetableEntryRequest,
) -> Result<TimetableValidationResponse, AppError> {
    let mut conflicts = Vec::new();

    // Unified instructor conflict check via junction
    let candidate_instructors: Vec<Uuid> = if let Some(cc_id) = payload.classroom_course_id {
        sqlx::query_scalar(
            "SELECT instructor_id FROM classroom_course_instructors WHERE classroom_course_id = $1",
        )
        .bind(cc_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else if let Some(slot_id) = payload.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1",
        )
        .bind(slot_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if mode.as_deref() == Some("independent") {
            if let Some(cls_id) = payload.classroom_id {
                sqlx::query_scalar(
                    "SELECT instructor_id FROM activity_slot_classroom_assignments
                     WHERE slot_id = $1 AND classroom_id = $2",
                )
                .bind(slot_id)
                .bind(cls_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            sqlx::query_scalar("SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1")
                .bind(slot_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    if !candidate_instructors.is_empty() {
        let conflict_instructors: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name)
               FROM academic_timetable_entries te
               JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
               JOIN users u ON u.id = tei.instructor_id
               WHERE tei.instructor_id = ANY($1)
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true"#,
        )
        .bind(&candidate_instructors)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (name,) in &conflict_instructors {
            conflicts.push(ConflictInfo {
                conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
                message: format!("{} มีสอนในคาบนี้อยู่แล้ว", name),
                existing_entry: None,
            });
        }
    }

    // Classroom conflict check (resolves classroom_id from course if needed)
    let classroom_for_check: Option<Uuid> = if let Some(course_id) = payload.classroom_course_id {
        let cls: Option<Uuid> =
            sqlx::query_scalar("SELECT classroom_id FROM classroom_courses WHERE id = $1")
                .bind(course_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        if cls.is_none() {
            return Err(AppError::NotFound("Classroom course not found".to_string()));
        }
        cls
    } else {
        payload.classroom_id
    };

    if let Some(cls_id) = classroom_for_check {
        let classroom_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries te
                LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                WHERE (te.classroom_id = $1 OR cc.classroom_id = $1)
                  AND te.day_of_week = $2
                  AND te.period_id = $3
                  AND te.is_active = true
            )
            "#,
        )
        .bind(cls_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if classroom_conflict {
            conflicts.push(ConflictInfo {
                conflict_type: "CLASSROOM_CONFLICT".to_string(),
                message: "ห้องเรียนนี้มีตารางในคาบนี้อยู่แล้ว".to_string(),
                existing_entry: None,
            });
        }
    }

    // Room conflict
    if let Some(room_id) = payload.room_id {
        let has_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries
                WHERE room_id = $1
                  AND day_of_week = $2
                  AND period_id = $3
                  AND is_active = true
            )
            "#,
        )
        .bind(room_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if has_conflict {
            conflicts.push(ConflictInfo {
                conflict_type: "ROOM_CONFLICT".to_string(),
                message: "ห้องเรียนถูกใช้ในคาบนี้อยู่แล้ว".to_string(),
                existing_entry: None,
            });
        }
    }

    Ok(TimetableValidationResponse {
        is_valid: conflicts.is_empty(),
        conflicts,
    })
}
