use crate::models::applications::*;
use school_errors::AppError;
use sqlx::PgPool;
use std::collections::HashMap;
use utoipa::ToSchema;
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

#[derive(sqlx::FromRow)]
struct ScoreRoomSeatRow {
    exam_room_id: Uuid,
    room_name: String,
    building_name: Option<String>,
    seat_number: i32,
    exam_id: Option<String>,
    application_id: Uuid,
    application_number: Option<String>,
    full_name: String,
}

#[derive(serde::Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreRoomSeat {
    pub seat_number: i32,
    pub exam_id: Option<String>,
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub full_name: String,
}

#[derive(serde::Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreRoomGroup {
    pub exam_room_id: Uuid,
    pub room_name: String,
    pub building_name: Option<String>,
    pub seats: Vec<ScoreRoomSeat>,
}

pub async fn get_score_room_roster(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ScoreRoomGroup>, AppError> {
    let rows = sqlx::query_as::<_, ScoreRoomSeatRow>(
        r#"SELECT er.id AS exam_room_id,
                  COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
                  b.name_th AS building_name,
                  sa.seat_number, sa.exam_id,
                  aa.id AS application_id, aa.application_number,
                  CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name
           FROM admission_exam_seat_assignments sa
           JOIN admission_exam_rooms er ON er.id = sa.exam_room_id
           JOIN admission_applications aa ON aa.id = sa.application_id
           LEFT JOIN rooms r ON r.id = er.room_id
           LEFT JOIN buildings b ON b.id = r.building_id
           WHERE er.admission_round_id = $1 AND aa.admission_round_id = $1
           ORDER BY er.display_order ASC, er.created_at ASC, er.id ASC, sa.seat_number ASC"#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch score room roster: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงรายชื่อห้องสอบสำหรับกรอกคะแนนได้".to_string())
    })?;

    let mut groups: Vec<ScoreRoomGroup> = Vec::new();
    for row in rows {
        let seat = ScoreRoomSeat {
            seat_number: row.seat_number,
            exam_id: row.exam_id,
            application_id: row.application_id,
            application_number: row.application_number,
            full_name: row.full_name,
        };
        if let Some(last) = groups.last_mut() {
            if last.exam_room_id == row.exam_room_id {
                last.seats.push(seat);
                continue;
            }
        }
        groups.push(ScoreRoomGroup {
            exam_room_id: row.exam_room_id,
            room_name: row.room_name,
            building_name: row.building_name,
            seats: vec![seat],
        });
    }
    Ok(groups)
}

fn bulk_score_entry_count(entries: &[BulkScoreEntry]) -> usize {
    entries.iter().map(|entry| entry.scores.len()).sum()
}

#[derive(Debug, Clone, PartialEq)]
struct ScoreBulkRow {
    application_id: Uuid,
    exam_subject_id: Uuid,
    score: Option<f64>,
}

fn push_score_bulk_row(
    rows: &mut Vec<ScoreBulkRow>,
    index_by_key: &mut HashMap<(Uuid, Uuid), usize>,
    row: ScoreBulkRow,
) {
    let key = (row.application_id, row.exam_subject_id);
    if let Some(index) = index_by_key.get(&key).copied() {
        rows[index] = row;
    } else {
        index_by_key.insert(key, rows.len());
        rows.push(row);
    }
}

fn score_entries_to_bulk_rows(
    application_id: Uuid,
    scores: &[UpdateScoreEntry],
) -> Vec<ScoreBulkRow> {
    let mut rows = Vec::with_capacity(scores.len());
    let mut index_by_key = HashMap::with_capacity(scores.len());
    for score in scores {
        push_score_bulk_row(
            &mut rows,
            &mut index_by_key,
            ScoreBulkRow {
                application_id,
                exam_subject_id: score.exam_subject_id,
                score: score.score,
            },
        );
    }
    rows
}

fn bulk_score_entries_to_rows(entries: &[BulkScoreEntry]) -> Vec<ScoreBulkRow> {
    let mut rows = Vec::with_capacity(bulk_score_entry_count(entries));
    let mut index_by_key = HashMap::with_capacity(rows.capacity());
    for bulk_entry in entries {
        for score in &bulk_entry.scores {
            push_score_bulk_row(
                &mut rows,
                &mut index_by_key,
                ScoreBulkRow {
                    application_id: bulk_entry.application_id,
                    exam_subject_id: score.exam_subject_id,
                    score: score.score,
                },
            );
        }
    }
    rows
}

async fn upsert_application_scores(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    rows: &[ScoreBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let app_ids: Vec<Uuid> = rows.iter().map(|row| row.application_id).collect();
    let sub_ids: Vec<Uuid> = rows.iter().map(|row| row.exam_subject_id).collect();
    let score_vals: Vec<Option<f64>> = rows.iter().map(|row| row.score).collect();

    let upsert = sqlx::query(
        r#"INSERT INTO admission_exam_scores (application_id, exam_subject_id, score, entered_by, entered_at, updated_at)
           SELECT t.application_id, t.exam_subject_id, t.score, $4, NOW(), NOW()
           FROM UNNEST($1::uuid[], $2::uuid[], $3::float8[]) AS t(application_id, exam_subject_id, score)
           JOIN admission_applications aa ON aa.id = t.application_id
           JOIN admission_exam_subjects aes
             ON aes.id = t.exam_subject_id AND aes.admission_round_id = aa.admission_round_id
           ON CONFLICT (application_id, exam_subject_id)
           DO UPDATE SET score = EXCLUDED.score, entered_by = EXCLUDED.entered_by, updated_at = NOW()"#,
    )
    .bind(&app_ids)
    .bind(&sub_ids)
    .bind(&score_vals)
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to upsert scores: {}", e);
        AppError::InternalServerError("Failed to update score".to_string())
    })?;
    if upsert.rows_affected() != rows.len() as u64 {
        return Err(AppError::BadRequest(
            "ข้อมูลคะแนนบางรายการไม่อยู่ในรอบรับสมัครของใบสมัครนี้".to_string(),
        ));
    }

    Ok(())
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

    let score_rows = score_entries_to_bulk_rows(application_id, scores);
    upsert_application_scores(&mut tx, user_id, &score_rows).await?;

    sqlx::query(
        r#"UPDATE admission_applications aa
           SET status = 'scored', updated_at = NOW()
           WHERE aa.id = $1 AND aa.status = 'verified'
             AND EXISTS (
                 SELECT 1 FROM admission_exam_subjects aes
                 WHERE aes.admission_round_id = aa.admission_round_id
             )
             AND NOT EXISTS (
                 SELECT 1 FROM admission_exam_subjects aes
                 LEFT JOIN admission_exam_scores esc
                   ON esc.exam_subject_id = aes.id AND esc.application_id = aa.id
                 WHERE aes.admission_round_id = aa.admission_round_id
                   AND esc.score IS NULL
             )"#,
    )
    .bind(application_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update scored application status: {}", error);
        AppError::InternalServerError("Failed to update score".to_string())
    })?;

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
    let rows = bulk_score_entries_to_rows(entries);
    if rows.is_empty() {
        return Ok(0);
    }

    let app_ids: Vec<Uuid> = rows.iter().map(|row| row.application_id).collect();
    let sub_ids: Vec<Uuid> = rows.iter().map(|row| row.exam_subject_id).collect();
    let score_vals: Vec<Option<f64>> = rows.iter().map(|row| row.score).collect();
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to start bulk score transaction: {}", error);
        AppError::InternalServerError("Failed to update scores".to_string())
    })?;
    let upsert = sqlx::query(
        r#"INSERT INTO admission_exam_scores (application_id, exam_subject_id, score, entered_by, entered_at, updated_at)
           SELECT t.a, t.s, t.sc, $4, NOW(), NOW()
           FROM UNNEST($1::uuid[], $2::uuid[], $3::float8[]) AS t(a, s, sc)
           JOIN admission_applications aa ON aa.id = t.a AND aa.admission_round_id = $5
           JOIN admission_exam_subjects aes ON aes.id = t.s AND aes.admission_round_id = $5
           ON CONFLICT (application_id, exam_subject_id)
           DO UPDATE SET score = EXCLUDED.score, entered_by = EXCLUDED.entered_by, updated_at = NOW()"#,
    )
    .bind(&app_ids)
    .bind(&sub_ids)
    .bind(&score_vals)
    .bind(user_id)
    .bind(round_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Bulk score error: {}", error);
        AppError::InternalServerError("Failed to update scores".to_string())
    })?;
    if upsert.rows_affected() != rows.len() as u64 {
        return Err(AppError::BadRequest(
            "ข้อมูลคะแนนบางรายการไม่อยู่ในรอบรับสมัครนี้".to_string(),
        ));
    }

    let mut app_id_set = app_ids;
    app_id_set.sort_unstable();
    app_id_set.dedup();
    sqlx::query(
        r#"WITH subject_total AS (
               SELECT COUNT(*) AS total FROM admission_exam_subjects WHERE admission_round_id = $2
           ), scored_by_app AS (
               SELECT esc.application_id, COUNT(*) AS scored
               FROM admission_exam_scores esc
               JOIN admission_exam_subjects aes
                 ON aes.id = esc.exam_subject_id AND aes.admission_round_id = $2
               WHERE esc.application_id = ANY($1) AND esc.score IS NOT NULL
               GROUP BY esc.application_id
           )
           UPDATE admission_applications aa
           SET status = 'scored', updated_at = NOW()
           FROM subject_total, scored_by_app
           WHERE aa.id = scored_by_app.application_id
             AND aa.id = ANY($1) AND aa.admission_round_id = $2 AND aa.status = 'verified'
             AND subject_total.total > 0 AND scored_by_app.scored >= subject_total.total"#,
    )
    .bind(&app_id_set)
    .bind(round_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update scored application statuses: {}", error);
        AppError::InternalServerError("Failed to update scores".to_string())
    })?;

    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit bulk score transaction: {}", error);
        AppError::InternalServerError("Failed to update scores".to_string())
    })?;

    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn score_entries_to_bulk_rows_dedupes_subjects_with_latest_score() {
        let application_id = Uuid::new_v4();
        let subject_id = Uuid::new_v4();

        let rows = score_entries_to_bulk_rows(
            application_id,
            &[
                UpdateScoreEntry {
                    exam_subject_id: subject_id,
                    score: Some(8.0),
                },
                UpdateScoreEntry {
                    exam_subject_id: subject_id,
                    score: Some(9.5),
                },
            ],
        );

        assert_eq!(
            rows,
            vec![ScoreBulkRow {
                application_id,
                exam_subject_id: subject_id,
                score: Some(9.5),
            }]
        );
    }
}
