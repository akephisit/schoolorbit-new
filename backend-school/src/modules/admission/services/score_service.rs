use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreRow {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub full_name: String,
    pub track_name: Option<String>,
    pub status: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub subject_code: Option<String>,
    pub max_score: f64,
    pub score: Option<f64>,
}

fn should_mark_application_scored(total_subjects: i64, scored_subjects: i64) -> bool {
    total_subjects > 0 && scored_subjects >= total_subjects
}

fn bulk_score_entry_count(entries: &[BulkScoreEntry]) -> usize {
    entries.iter().map(|entry| entry.scores.len()).sum()
}

pub async fn get_all_scores(pool: &PgPool, round_id: Uuid) -> Result<Vec<ScoreRow>, AppError> {
    sqlx::query_as::<_, ScoreRow>(
        r#"SELECT aa.id AS application_id, aa.application_number,
                  CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
                  at2.name AS track_name, aa.status,
                  aes.id AS subject_id, aes.name AS subject_name, aes.code AS subject_code,
                  aes.max_score::FLOAT8 AS max_score, esc.score
           FROM admission_applications aa
           JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
           CROSS JOIN admission_exam_subjects aes
           LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id AND esc.exam_subject_id = aes.id
           WHERE aa.admission_round_id = $1
             AND aes.admission_round_id = $1
             AND aa.status NOT IN ('rejected', 'withdrawn')
           ORDER BY at2.display_order ASC, aa.application_number ASC, aes.display_order ASC"#
    )
    .bind(round_id).fetch_all(pool).await
    .map_err(|e| {
        tracing::error!("Failed to fetch scores: {}", e);
        AppError::InternalServerError("Failed to fetch scores".to_string())
    })
}

pub async fn get_application_scores(pool: &PgPool, id: Uuid) -> Result<Vec<ExamScore>, AppError> {
    sqlx::query_as::<_, ExamScore>(
        r#"SELECT esc.id, esc.application_id, esc.exam_subject_id, esc.score,
                  esc.entered_by, esc.entered_at, esc.updated_at,
                  aes.name AS subject_name, aes.code AS subject_code,
                  aes.max_score::FLOAT8 AS max_score
           FROM admission_exam_subjects aes
           LEFT JOIN admission_exam_scores esc ON esc.exam_subject_id = aes.id AND esc.application_id = $1
           WHERE aes.admission_round_id = (
               SELECT admission_round_id FROM admission_applications WHERE id = $1
           )
           ORDER BY aes.display_order ASC"#
    )
    .bind(id).fetch_all(pool).await
    .map_err(|e| {
        tracing::error!("Failed to fetch application scores: {}", e);
        AppError::InternalServerError("Failed to fetch scores".to_string())
    })
}

pub async fn update_application_scores(
    pool: &PgPool,
    application_id: Uuid,
    user_id: Uuid,
    scores: &[UpdateScoreEntry],
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    for entry in scores {
        sqlx::query(
            r#"INSERT INTO admission_exam_scores (application_id, exam_subject_id, score, entered_by, entered_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())
               ON CONFLICT (application_id, exam_subject_id)
               DO UPDATE SET score = $3, entered_by = $4, updated_at = NOW()"#
        )
        .bind(application_id).bind(entry.exam_subject_id).bind(entry.score).bind(user_id)
        .execute(&mut *tx).await
        .map_err(|e| {
            tracing::error!("Failed to upsert score: {}", e);
            AppError::InternalServerError("Failed to update score".to_string())
        })?;
    }

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admission_exam_subjects WHERE admission_round_id = (SELECT admission_round_id FROM admission_applications WHERE id = $1)"
    )
    .bind(application_id).fetch_one(&mut *tx).await.unwrap_or(0);

    let scored: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admission_exam_scores WHERE application_id = $1 AND score IS NOT NULL"
    )
    .bind(application_id).fetch_one(&mut *tx).await.unwrap_or(0);

    if should_mark_application_scored(total, scored) {
        sqlx::query(
            "UPDATE admission_applications SET status = 'scored', updated_at = NOW() WHERE id = $1 AND status = 'verified'"
        )
        .bind(application_id).execute(&mut *tx).await.ok();
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(())
}

pub async fn bulk_update_scores(
    pool: &PgPool,
    round_id: Uuid,
    user_id: Uuid,
    entries: &[BulkScoreEntry],
) -> Result<usize, AppError> {
    let mut app_ids: Vec<Uuid> = Vec::new();
    let mut sub_ids: Vec<Uuid> = Vec::new();
    let mut score_vals: Vec<Option<f64>> = Vec::new();

    for entry in entries {
        for score in &entry.scores {
            app_ids.push(entry.application_id);
            sub_ids.push(score.exam_subject_id);
            score_vals.push(score.score);
        }
    }

    let updated = bulk_score_entry_count(entries);

    if updated > 0 {
        sqlx::query(
            r#"INSERT INTO admission_exam_scores (application_id, exam_subject_id, score, entered_by, entered_at, updated_at)
               SELECT a, s, sc, $4, NOW(), NOW()
               FROM UNNEST($1::uuid[], $2::uuid[], $3::float8[]) AS t(a, s, sc)
               ON CONFLICT (application_id, exam_subject_id)
               DO UPDATE SET score = EXCLUDED.score, entered_by = EXCLUDED.entered_by, updated_at = NOW()"#
        )
        .bind(&app_ids).bind(&sub_ids).bind(&score_vals).bind(user_id)
        .execute(pool).await
        .map_err(|e| {
            tracing::error!("Bulk score error: {}", e);
            AppError::InternalServerError("Failed to bulk update scores".to_string())
        })?;
    }

    let app_id_set: Vec<Uuid> = entries.iter().map(|e| e.application_id).collect();
    sqlx::query(
        r#"UPDATE admission_applications aa
           SET status = 'scored', updated_at = NOW()
           WHERE aa.id = ANY($1) AND aa.status = 'verified'
             AND (
                 SELECT COUNT(*) FROM admission_exam_scores esc
                 WHERE esc.application_id = aa.id AND esc.score IS NOT NULL
             ) >= (
                 SELECT COUNT(*) FROM admission_exam_subjects WHERE admission_round_id = $2
             )"#,
    )
    .bind(&app_id_set)
    .bind(round_id)
    .execute(pool)
    .await
    .ok();

    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_mark_application_scored_requires_at_least_one_subject_and_all_scores() {
        assert!(!should_mark_application_scored(0, 0));
        assert!(!should_mark_application_scored(3, 2));
        assert!(should_mark_application_scored(3, 3));
        assert!(should_mark_application_scored(3, 4));
    }

    #[test]
    fn bulk_score_entry_count_counts_nested_scores() {
        let subject_a = Uuid::new_v4();
        let subject_b = Uuid::new_v4();
        let entries = vec![
            BulkScoreEntry {
                application_id: Uuid::new_v4(),
                scores: vec![
                    UpdateScoreEntry {
                        exam_subject_id: subject_a,
                        score: Some(10.0),
                    },
                    UpdateScoreEntry {
                        exam_subject_id: subject_b,
                        score: None,
                    },
                ],
            },
            BulkScoreEntry {
                application_id: Uuid::new_v4(),
                scores: vec![],
            },
        ];

        assert_eq!(bulk_score_entry_count(&entries), 2);
    }
}
