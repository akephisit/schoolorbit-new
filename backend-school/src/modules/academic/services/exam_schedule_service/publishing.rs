use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::ExamRound;

use super::workspace::{build_readiness, fetch_workspace_counts_in_tx};

pub async fn publish_round(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let mut tx = pool.begin().await?;

    let _locked_round_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id
        FROM academic_exam_rounds
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    let readiness = build_readiness(fetch_workspace_counts_in_tx(&mut tx, round_id).await?);
    if !readiness.can_publish {
        return Err(AppError::BadRequest(format!(
            "Exam round is not ready to publish: {}",
            readiness.blockers.join("; ")
        )));
    }

    let round = sqlx::query_as::<_, ExamRound>(
        r#"
        UPDATE academic_exam_rounds
        SET status = 'published',
            published_at = now(),
            published_by = $2,
            updated_by = $2,
            updated_at = now()
        WHERE id = $1
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  exam_kind,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(round_id)
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(round)
}
