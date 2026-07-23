use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

/// Row สำหรับ /timetable/occupancy — ใช้ frontend สร้าง index เอง
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct OccupancyRow {
    pub id: Uuid,
    pub classroom_id: Option<Uuid>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub instructor_ids: Vec<Uuid>,
}

/// Get occupancy snapshot — frontend ใช้สร้าง index คำนวณ drop validity client-side
pub async fn get_occupancy(
    pool: &PgPool,
    semester_id: Uuid,
) -> Result<Vec<OccupancyRow>, AppError> {
    sqlx::query_as::<_, OccupancyRow>(
        r#"SELECT
            te.id,
            te.classroom_id,
            te.day_of_week,
            te.period_id,
            te.room_id,
            COALESCE(
                ARRAY_AGG(tei.instructor_id) FILTER (WHERE tei.instructor_id IS NOT NULL),
                '{}'::uuid[]
            ) AS instructor_ids
           FROM academic_timetable_entries te
           LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
           WHERE te.academic_semester_id = $1 AND te.is_active = true
           GROUP BY te.id"#,
    )
    .bind(semester_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}
