//! Historical group membership projection; current status alone is insufficient.
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceGroupStudent {
    pub learning_group_id: Uuid,
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
}
pub async fn students(
    pool: &PgPool,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<Vec<AttendanceGroupStudent>, AppError> {
    if groups.len() > 2000 {
        return Err(AppError::ValidationError("กลุ่มเรียนเกินขนาดที่รองรับ".into()));
    }
    let rows:Vec<AttendanceGroupStudent>=sqlx::query_as("SELECT learning_group_id,student_id,student_academic_year_id FROM learning_group_students WHERE learning_group_id=ANY($1) AND membership_status<>'removed' AND joined_at<=$2 AND (left_at IS NULL OR left_at>=$2) ORDER BY learning_group_id,student_id LIMIT 100001").bind(groups).bind(date).fetch_all(pool).await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError("สมาชิกกลุ่มเกินขนาดที่รองรับ".into()));
    }
    Ok(rows)
}
