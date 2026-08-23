use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::ActivityResult;

pub async fn get_result(
    pool: &PgPool,
    learning_group_id: Uuid,
    student_academic_year_id: Uuid,
) -> Result<Option<ActivityResult>, AppError> {
    Ok(sqlx::query_as::<_, ActivityResultRow>(
        r#"SELECT result.id AS learning_result_id,
                  member.id AS learning_group_student_id,
                  detail.outcome,
                  result.updated_at AS finalized_at
           FROM learning_group_students member
           JOIN learning_results result
             ON result.learning_group_id = member.learning_group_id
            AND result.student_academic_year_id = member.student_academic_year_id
            AND result.kind = 'activity' AND result.status = 'recorded'
           JOIN activity_result_details detail ON detail.learning_result_id = result.id
           WHERE member.learning_group_id = $1
             AND member.student_academic_year_id = $2
           ORDER BY result.updated_at DESC, result.id
           LIMIT 1"#,
    )
    .bind(learning_group_id)
    .bind(student_academic_year_id)
    .fetch_optional(pool)
    .await?
    .map(Into::into))
}

#[derive(sqlx::FromRow)]
struct ActivityResultRow {
    learning_result_id: Uuid,
    learning_group_student_id: Uuid,
    outcome: String,
    finalized_at: chrono::DateTime<chrono::Utc>,
}

impl From<ActivityResultRow> for ActivityResult {
    fn from(row: ActivityResultRow) -> Self {
        Self {
            learning_result_id: row.learning_result_id,
            learning_group_student_id: row.learning_group_student_id,
            outcome: Some(row.outcome),
            attendance_percent: None,
            teacher_comment: None,
            finalized_at: Some(row.finalized_at),
        }
    }
}
