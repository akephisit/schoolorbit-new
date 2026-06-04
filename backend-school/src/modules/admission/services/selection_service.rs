use crate::error::AppError;
use crate::modules::admission::models::applications::{
    AssignRoomsGlobalRequest, AssignRoomsRequest,
};
use crate::modules::admission::models::rounds::UpdateSelectionSettingsRequest;
use crate::modules::admission::services::pii;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Compute (room_idx, rank_in_room) for each student given room capacities + method.
pub fn compute_room_assignments(
    count: usize,
    capacities: &[i32],
    method: &str,
) -> Vec<(usize, i32)> {
    let n = capacities.len();
    let mut result = Vec::with_capacity(count);
    if n == 0 {
        return result;
    }

    if method == "round_robin" {
        let mut rr_counts = vec![0i32; n];
        let mut rr_idx = 0usize;
        for _ in 0..count {
            let start = rr_idx;
            let mut found_idx = 0usize;
            let mut found_rir = 1i32;
            for step in 0..n {
                let idx = (start + step) % n;
                if rr_counts[idx] < capacities[idx] {
                    rr_counts[idx] += 1;
                    found_idx = idx;
                    found_rir = rr_counts[idx];
                    rr_idx = (idx + 1) % n;
                    break;
                }
            }
            result.push((found_idx, found_rir));
        }
    } else {
        let mut room_idx = 0usize;
        let mut count_in_room = 0i32;
        for _ in 0..count {
            let rir = count_in_room + 1;
            count_in_room += 1;
            let idx = room_idx;
            if count_in_room >= capacities[room_idx] && room_idx + 1 < n {
                room_idx += 1;
                count_in_room = 0;
            }
            result.push((idx, rir));
        }
    }
    result
}

pub fn parse_subject_ids(s: &str) -> Vec<Uuid> {
    s.split(',')
        .filter_map(|p| Uuid::parse_str(p.trim()).ok())
        .collect()
}

pub async fn all_subject_ids_for_track(pool: &PgPool, track_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM admission_exam_subjects WHERE admission_round_id = (SELECT admission_round_id FROM admission_tracks WHERE id = $1)",
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn all_subject_ids_for_round(pool: &PgPool, round_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar("SELECT id FROM admission_exam_subjects WHERE admission_round_id = $1")
        .bind(round_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
}

#[derive(sqlx::FromRow)]
pub struct RankRow {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub selection_score: Option<f64>,
    pub total_score: Option<f64>,
}

#[derive(sqlx::FromRow)]
pub struct RankRowDetailed {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub selection_score: Option<f64>,
    pub total_score: Option<f64>,
    pub original_track_name: Option<String>,
    pub is_track_overridden: Option<bool>,
    pub saved_room_id: Option<Uuid>,
    pub saved_room_name: Option<String>,
    pub gender: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundRankingEntry {
    pub rank: usize,
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub total_score: f64,
    pub selection_score: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundRankingResult {
    pub track_id: Uuid,
    pub track_name: String,
    pub applications: Vec<RoundRankingEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingRoomSummary {
    pub room_id: String,
    pub room_name: String,
    pub capacity: i32,
    pub student_count: i64,
    pub male_count: i64,
    pub female_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRankingEntry {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub selection_score: f64,
    pub total_score: f64,
    pub selection_rank: i64,
    pub final_rank: Option<i64>,
    pub assigned_room: Option<String>,
    pub assigned_room_id: Option<Uuid>,
    pub room_saved: bool,
    pub is_overflow: bool,
    pub is_track_overridden: bool,
    pub original_track_name: Option<String>,
    pub gender: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRankingResult {
    pub track_id: Uuid,
    pub track_name: String,
    pub rooms: Vec<RankingRoomSummary>,
    pub applications: Vec<TrackRankingEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalRankingEntry {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub total_score: f64,
    pub global_rank: i64,
    pub rank_in_room: Option<i64>,
    pub assigned_room: Option<String>,
    pub assigned_room_id: Option<Uuid>,
    pub room_saved: bool,
    pub is_overflow: bool,
    pub original_track_name: Option<String>,
    pub gender: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalRankingResult {
    pub rooms: Vec<RankingRoomSummary>,
    pub applications: Vec<GlobalRankingEntry>,
}

fn pii_error(context: &str, error: String) -> AppError {
    eprintln!("Admission selection PII {} failed: {}", context, error);
    AppError::InternalServerError("ไม่สามารถประมวลผลข้อมูลส่วนบุคคลได้".to_string())
}

fn decrypt_rank_row(mut row: RankRow) -> Result<RankRow, AppError> {
    row.national_id = pii::decrypt_required(&row.national_id)
        .map_err(|error| pii_error("decrypt national_id", error))?;
    Ok(row)
}

fn decrypt_rank_row_detailed(mut row: RankRowDetailed) -> Result<RankRowDetailed, AppError> {
    row.national_id = pii::decrypt_required(&row.national_id)
        .map_err(|error| pii_error("decrypt national_id", error))?;
    Ok(row)
}

/// GET /api/admission/rounds/:id/ranking
pub async fn get_round_ranking(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<RoundRankingResult>, AppError> {
    let tracks: Vec<(Uuid, String, serde_json::Value, String)> = sqlx::query_as(
        "SELECT id, name, scoring_subject_ids, tiebreak_method FROM admission_tracks WHERE admission_round_id = $1 ORDER BY display_order ASC"
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch tracks".to_string()))?;

    let mut all_rankings: Vec<RoundRankingResult> = Vec::new();

    for (track_id, track_name, _scoring_ids, tiebreak) in tracks {
        let tiebreak_order = if tiebreak == "gpa" {
            "aa.previous_gpa DESC NULLS LAST"
        } else {
            "aa.created_at ASC"
        };

        let all_ids = all_subject_ids_for_round(pool, round_id).await;

        let query = format!(
            r#"
            SELECT
                aa.id AS application_id,
                aa.application_number,
                aa.national_id,
                CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
                COALESCE(SUM(esc.score), 0) AS selection_score,
                COALESCE(SUM(esc.score), 0) AS total_score
            FROM admission_applications aa
            LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
            WHERE aa.admission_track_id = $2
              AND aa.status NOT IN ('rejected', 'withdrawn', 'absent')
            GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.previous_gpa, aa.created_at
            ORDER BY total_score DESC, {}
            "#,
            tiebreak_order
        );

        let rows = sqlx::query_as::<_, RankRow>(&query)
            .bind(&all_ids)
            .bind(track_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        let rows: Vec<RankRow> = rows
            .into_iter()
            .map(decrypt_rank_row)
            .collect::<Result<_, _>>()?;

        let ranked: Vec<RoundRankingEntry> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| RoundRankingEntry {
                rank: i + 1,
                application_id: row.application_id,
                application_number: row.application_number,
                national_id: row.national_id,
                full_name: row.full_name,
                total_score: row.total_score.unwrap_or(0.0),
                selection_score: row.selection_score.unwrap_or(0.0),
            })
            .collect();

        all_rankings.push(RoundRankingResult {
            track_id,
            track_name,
            applications: ranked,
        });
    }

    Ok(all_rankings)
}

pub async fn get_track_ranking(
    pool: &PgPool,
    track_id: Uuid,
    selection_subject_ids_param: Option<String>,
    room_assignment_method: Option<String>,
) -> Result<TrackRankingResult, AppError> {
    let (track_name, tiebreak): (String, String) =
        sqlx::query_as("SELECT name, tiebreak_method FROM admission_tracks WHERE id = $1")
            .bind(track_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
            .ok_or_else(|| AppError::NotFound("ไม่พบสายการเรียน".to_string()))?;

    let tiebreak_order = if tiebreak == "gpa" {
        "aa.previous_gpa DESC NULLS LAST"
    } else {
        "aa.created_at ASC"
    };

    let selection_ids: Vec<Uuid> = match &selection_subject_ids_param {
        Some(s) if !s.is_empty() => parse_subject_ids(s),
        _ => all_subject_ids_for_track(pool, track_id).await,
    };

    #[derive(sqlx::FromRow)]
    struct RoomRow {
        room_id: Uuid,
        room_name: String,
        capacity: i32,
    }

    let rooms = sqlx::query_as::<_, RoomRow>(
        r#"
        SELECT cr.id AS room_id, cr.name AS room_name, cr.capacity
        FROM admission_tracks t
        JOIN study_plans sp ON t.study_plan_id = sp.id
        JOIN study_plan_versions spv ON spv.study_plan_id = sp.id AND spv.is_active = true
        JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
        WHERE t.id = $1
          AND cr.academic_year_id = (
              SELECT academic_year_id FROM admission_rounds WHERE id = t.admission_round_id
          )
          AND cr.grade_level_id = (
              SELECT grade_level_id FROM admission_rounds WHERE id = t.admission_round_id
          )
        ORDER BY cr.name ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let query = format!(
        r#"
        SELECT
            aa.id AS application_id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            COALESCE(SUM(CASE WHEN esc.exam_subject_id = ANY($1) THEN esc.score ELSE 0 END), 0) AS selection_score,
            COALESCE(SUM(esc.score), 0) AS total_score,
            at_orig.name AS original_track_name,
            aa.room_assignment_track_id IS NOT NULL AS is_track_overridden,
            ara.class_room_id AS saved_room_id,
            cr_saved.name AS saved_room_name,
            aa.gender
        FROM admission_applications aa
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
        LEFT JOIN admission_tracks at_orig ON at_orig.id = aa.admission_track_id
        LEFT JOIN admission_room_assignments ara ON ara.application_id = aa.id
        LEFT JOIN class_rooms cr_saved ON cr_saved.id = ara.class_room_id
        WHERE COALESCE(aa.room_assignment_track_id, aa.admission_track_id) = $2
          AND aa.status NOT IN ('rejected', 'withdrawn', 'absent')
        GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.previous_gpa, aa.created_at, at_orig.name, aa.room_assignment_track_id, ara.class_room_id, cr_saved.name, aa.gender
        ORDER BY selection_score DESC, total_score DESC, {}
        "#,
        tiebreak_order
    );

    let rows = sqlx::query_as::<_, RankRowDetailed>(&query)
        .bind(&selection_ids)
        .bind(track_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let rows: Vec<RankRowDetailed> = rows
        .into_iter()
        .map(decrypt_rank_row_detailed)
        .collect::<Result<_, _>>()?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let capacity_usize = if total_capacity > 0 {
        total_capacity as usize
    } else {
        usize::MAX
    };
    let any_saved = rows.iter().any(|r| r.saved_room_id.is_some());
    let (accepted_rows, overflow_rows): (Vec<_>, Vec<_>) =
        rows.into_iter().enumerate().partition(|(i, r)| {
            if any_saved {
                r.saved_room_id.is_some()
            } else {
                *i < capacity_usize
            }
        });

    let mut accepted_sorted: Vec<(usize, RankRowDetailed)> = accepted_rows;
    accepted_sorted.sort_by(|(_, a), (_, b)| {
        b.total_score
            .unwrap_or(0.0)
            .partial_cmp(&a.total_score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let method = room_assignment_method.as_deref().unwrap_or("sequential");
    let room_caps: Vec<i32> = rooms.iter().map(|r| r.capacity).collect();
    let room_slots = compute_room_assignments(accepted_sorted.len(), &room_caps, method);

    let mut room_stats: std::collections::HashMap<String, (i64, i64, i64)> =
        std::collections::HashMap::new();

    let accepted_json: Vec<TrackRankingEntry> = accepted_sorted
        .into_iter()
        .enumerate()
        .map(|(final_i, (sel_i, row))| {
            let final_rank = (final_i + 1) as i64;
            let selection_rank = (sel_i + 1) as i64;

            let room_saved = row.saved_room_id.is_some();
            let (assigned_room, assigned_room_id) =
                if let (Some(name), Some(id)) = (&row.saved_room_name, &row.saved_room_id) {
                    (Some(name.clone()), Some(*id))
                } else if rooms.is_empty() {
                    (None, None)
                } else {
                    let (ri, _rir) = room_slots[final_i];
                    (Some(rooms[ri].room_name.clone()), Some(rooms[ri].room_id))
                };

            if let Some(rid) = assigned_room_id {
                let entry = room_stats.entry(rid.to_string()).or_insert((0, 0, 0));
                entry.0 += 1;
                match row.gender.as_deref() {
                    Some(g) if g.eq_ignore_ascii_case("male") || g == "ชาย" => entry.1 += 1,
                    Some(g) if g.eq_ignore_ascii_case("female") || g == "หญิง" => {
                        entry.2 += 1
                    }
                    _ => {}
                }
            }

            let is_overridden = row.is_track_overridden.unwrap_or(false);
            TrackRankingEntry {
                application_id: row.application_id,
                application_number: row.application_number,
                national_id: row.national_id,
                full_name: row.full_name,
                selection_score: row.selection_score.unwrap_or(0.0),
                total_score: row.total_score.unwrap_or(0.0),
                selection_rank,
                final_rank: Some(final_rank),
                assigned_room,
                assigned_room_id,
                room_saved,
                is_overflow: false,
                is_track_overridden: is_overridden,
                original_track_name: if is_overridden {
                    row.original_track_name
                } else {
                    None
                },
                gender: row.gender,
            }
        })
        .collect();

    let overflow_json: Vec<TrackRankingEntry> = overflow_rows
        .into_iter()
        .map(|(sel_i, row)| {
            let is_overridden = row.is_track_overridden.unwrap_or(false);
            TrackRankingEntry {
                application_id: row.application_id,
                application_number: row.application_number,
                national_id: row.national_id,
                full_name: row.full_name,
                selection_score: row.selection_score.unwrap_or(0.0),
                total_score: row.total_score.unwrap_or(0.0),
                selection_rank: (sel_i + 1) as i64,
                final_rank: None,
                assigned_room: None,
                assigned_room_id: None,
                room_saved: false,
                is_overflow: true,
                is_track_overridden: is_overridden,
                original_track_name: if is_overridden {
                    row.original_track_name
                } else {
                    None
                },
                gender: row.gender,
            }
        })
        .collect();

    let mut all_apps = accepted_json;
    all_apps.extend(overflow_json);

    Ok(TrackRankingResult {
        track_id,
        track_name,
        rooms: rooms
            .iter()
            .map(|r| {
                let room_id = r.room_id.to_string();
                let (total, male, female) = room_stats.get(&room_id).copied().unwrap_or((0, 0, 0));
                RankingRoomSummary {
                    room_id,
                    room_name: r.room_name.clone(),
                    capacity: r.capacity,
                    student_count: total,
                    male_count: male,
                    female_count: female,
                }
            })
            .collect(),
        applications: all_apps,
    })
}

pub async fn assign_rooms(
    pool: &PgPool,
    round_id: Uuid,
    payload: AssignRoomsRequest,
    user_id: Uuid,
) -> Result<usize, AppError> {
    let track_id = payload.track_id;

    let tiebreak: String = sqlx::query_scalar(
        "SELECT tiebreak_method FROM admission_tracks WHERE id = $1 AND admission_round_id = $2",
    )
    .bind(track_id)
    .bind(round_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบสายการเรียน".to_string()))?;

    let tiebreak_order = if tiebreak == "gpa" {
        "aa.previous_gpa DESC NULLS LAST"
    } else {
        "aa.created_at ASC"
    };

    let selection_ids: Vec<Uuid> = match &payload.selection_subject_ids {
        Some(ids) if !ids.is_empty() => ids.clone(),
        _ => all_subject_ids_for_round(pool, round_id).await,
    };

    #[derive(sqlx::FromRow)]
    struct RoomRow {
        room_id: Uuid,
        capacity: i32,
    }

    let rooms = sqlx::query_as::<_, RoomRow>(
        r#"
        SELECT cr.id AS room_id, cr.capacity
        FROM admission_tracks t
        JOIN study_plans sp ON t.study_plan_id = sp.id
        JOIN study_plan_versions spv ON spv.study_plan_id = sp.id AND spv.is_active = true
        JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
        WHERE t.id = $1
          AND cr.academic_year_id = (
              SELECT academic_year_id FROM admission_rounds WHERE id = t.admission_round_id
          )
          AND cr.grade_level_id = (
              SELECT grade_level_id FROM admission_rounds WHERE id = t.admission_round_id
          )
        ORDER BY cr.name ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch rooms".to_string()))?;

    if rooms.is_empty() {
        return Err(AppError::BadRequest(
            "ไม่พบห้องเรียนสำหรับสายนี้ กรุณาสร้างห้องเรียนก่อน".to_string(),
        ));
    }

    let query = format!(
        r#"
        SELECT
            aa.id AS application_id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            COALESCE(SUM(CASE WHEN esc.exam_subject_id = ANY($1) THEN esc.score ELSE 0 END), 0) AS selection_score,
            COALESCE(SUM(esc.score), 0) AS total_score
        FROM admission_applications aa
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
        WHERE COALESCE(aa.room_assignment_track_id, aa.admission_track_id) = $2
          AND aa.status NOT IN ('rejected', 'withdrawn', 'absent')
        GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.previous_gpa, aa.created_at
        ORDER BY selection_score DESC, total_score DESC, {}
        "#,
        tiebreak_order
    );

    let rows = sqlx::query_as::<_, RankRow>(&query)
        .bind(&selection_ids)
        .bind(track_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to compute ranking".to_string()))?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let capacity_usize = total_capacity as usize;

    let (mut accepted, _overflow): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .enumerate()
        .partition(|(i, _)| *i < capacity_usize);

    accepted.sort_by(|(_, a), (_, b)| {
        b.total_score
            .unwrap_or(0.0)
            .partial_cmp(&a.total_score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let method = payload
        .room_assignment_method
        .as_deref()
        .unwrap_or("sequential");
    let room_caps: Vec<i32> = rooms.iter().map(|r| r.capacity).collect();
    let room_slots = compute_room_assignments(accepted.len(), &room_caps, method);

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    sqlx::query(
        "DELETE FROM admission_room_assignments WHERE application_id IN (SELECT id FROM admission_applications WHERE COALESCE(room_assignment_track_id, admission_track_id) = $1)"
    )
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .ok();

    let assigned_count = accepted.len();

    let mut app_ids: Vec<Uuid> = Vec::with_capacity(assigned_count);
    let mut room_ids: Vec<Uuid> = Vec::with_capacity(assigned_count);
    let mut ranks_in_track: Vec<i32> = Vec::with_capacity(assigned_count);
    let mut ranks_in_room: Vec<i32> = Vec::with_capacity(assigned_count);
    let mut scores: Vec<f64> = Vec::with_capacity(assigned_count);
    for (final_rank, (_, row)) in accepted.iter().enumerate() {
        let (ri, rank_in_room) = room_slots[final_rank];
        app_ids.push(row.application_id);
        room_ids.push(rooms[ri].room_id);
        ranks_in_track.push((final_rank + 1) as i32);
        ranks_in_room.push(rank_in_room);
        scores.push(row.total_score.unwrap_or(0.0));
    }
    let assigned_bys: Vec<Uuid> = vec![user_id; assigned_count];

    if !app_ids.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO admission_room_assignments (
                application_id, class_room_id,
                rank_in_track, rank_in_room,
                total_score, full_score,
                assigned_by, assigned_at
            )
            SELECT * FROM UNNEST(
                $1::uuid[], $2::uuid[], $3::int[], $4::int[],
                $5::double precision[], $5::double precision[],
                $6::uuid[], array_fill(NOW()::timestamptz, ARRAY[array_length($1::uuid[], 1)])
            )
            ON CONFLICT (application_id) DO UPDATE SET
                class_room_id = EXCLUDED.class_room_id,
                rank_in_track = EXCLUDED.rank_in_track,
                rank_in_room  = EXCLUDED.rank_in_room,
                total_score   = EXCLUDED.total_score,
                full_score    = EXCLUDED.full_score,
                assigned_by   = EXCLUDED.assigned_by,
                assigned_at   = EXCLUDED.assigned_at
            "#,
        )
        .bind(&app_ids)
        .bind(&room_ids)
        .bind(&ranks_in_track)
        .bind(&ranks_in_room)
        .bind(&scores)
        .bind(&assigned_bys)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to bulk assign rooms: {}", e);
            AppError::InternalServerError("Failed to assign rooms".to_string())
        })?;

        sqlx::query(
            "UPDATE admission_applications SET status = 'accepted', updated_at = NOW() WHERE id = ANY($1::uuid[]) AND status NOT IN ('rejected', 'withdrawn')"
        )
        .bind(&app_ids)
        .execute(&mut *tx)
        .await
        .ok();
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(assigned_count)
}

pub async fn reset_all_room_assignments(pool: &PgPool, round_id: Uuid) -> Result<i64, AppError> {
    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        WITH deleted AS (
            DELETE FROM admission_room_assignments
            WHERE application_id IN (
                SELECT id FROM admission_applications WHERE admission_round_id = $1
            )
            RETURNING 1
        )
        SELECT COUNT(*) FROM deleted
        "#,
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    sqlx::query(
        "UPDATE admission_applications SET room_assignment_track_id = NULL, updated_at = NOW() WHERE admission_round_id = $1"
    )
    .bind(round_id)
    .execute(pool).await.ok();

    sqlx::query(
        r#"
        UPDATE admission_applications aa
        SET status = CASE
            WHEN (SELECT COUNT(*) FROM admission_exam_subjects WHERE admission_round_id = aa.admission_round_id) > 0
             AND (SELECT COUNT(*) FROM admission_exam_scores WHERE application_id = aa.id AND score IS NOT NULL)
                 >= (SELECT COUNT(*) FROM admission_exam_subjects WHERE admission_round_id = aa.admission_round_id)
            THEN 'scored'
            ELSE 'verified'
        END,
        updated_at = NOW()
        WHERE aa.admission_round_id = $1
          AND aa.status = 'accepted'
        "#
    )
    .bind(round_id)
    .execute(pool).await.ok();

    Ok(deleted)
}

pub async fn assign_rooms_global(
    pool: &PgPool,
    round_id: Uuid,
    payload: AssignRoomsGlobalRequest,
    user_id: Uuid,
) -> Result<usize, AppError> {
    #[derive(sqlx::FromRow)]
    struct RoomRow {
        room_id: Uuid,
        room_name: String,
        capacity: i32,
        track_id: Uuid,
    }

    let rooms_raw = sqlx::query_as::<_, RoomRow>(
        r#"
        SELECT cr.id AS room_id, cr.name AS room_name, cr.capacity, t.id AS track_id
        FROM admission_tracks t
        JOIN study_plans sp ON t.study_plan_id = sp.id
        JOIN study_plan_versions spv ON spv.study_plan_id = sp.id AND spv.is_active = true
        JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
        WHERE t.admission_round_id = $1
          AND cr.academic_year_id = (SELECT academic_year_id FROM admission_rounds WHERE id = $1)
          AND cr.grade_level_id   = (SELECT grade_level_id   FROM admission_rounds WHERE id = $1)
        ORDER BY cr.name ASC
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch rooms".to_string()))?;

    let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    let mut rooms: Vec<RoomRow> = Vec::new();
    for r in rooms_raw {
        if seen.insert(r.room_id) {
            rooms.push(r);
        }
    }
    rooms.sort_by(|a, b| a.room_name.cmp(&b.room_name));

    if rooms.is_empty() {
        return Err(AppError::BadRequest(
            "ไม่พบห้องเรียนในรอบนี้ กรุณาสร้างห้องเรียนก่อน".to_string(),
        ));
    }

    if let Some(ref order) = payload.room_order {
        if !order.is_empty() {
            let order_map: std::collections::HashMap<Uuid, usize> =
                order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
            rooms.sort_by_key(|r| order_map.get(&r.room_id).copied().unwrap_or(usize::MAX));
        }
    }

    let rows = sqlx::query_as::<_, RankRow>(
        r#"
        SELECT
            aa.id AS application_id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            COALESCE(SUM(esc.score), 0) AS selection_score,
            COALESCE(SUM(esc.score), 0) AS total_score
        FROM admission_applications aa
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
        WHERE aa.admission_round_id = $1
          AND aa.status NOT IN ('rejected', 'withdrawn', 'absent')
        GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.created_at
        ORDER BY total_score DESC, aa.created_at ASC
        "#
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch students".to_string()))?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let capacity_usize = total_capacity as usize;

    let (accepted, _overflow): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .enumerate()
        .partition(|(i, _)| *i < capacity_usize);

    let method = payload
        .room_assignment_method
        .as_deref()
        .unwrap_or("sequential");
    let room_caps: Vec<i32> = rooms.iter().map(|r| r.capacity).collect();
    let room_slots = compute_room_assignments(accepted.len(), &room_caps, method);

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    sqlx::query(
        "DELETE FROM admission_room_assignments WHERE application_id IN (SELECT id FROM admission_applications WHERE admission_round_id = $1)"
    )
    .bind(round_id)
    .execute(&mut *tx)
    .await
    .ok();

    let assigned_count = accepted.len();

    let mut app_ids: Vec<Uuid> = Vec::with_capacity(assigned_count);
    let mut room_ids: Vec<Uuid> = Vec::with_capacity(assigned_count);
    let mut track_ids: Vec<Uuid> = Vec::with_capacity(assigned_count);
    let mut ranks_in_track: Vec<i32> = Vec::with_capacity(assigned_count);
    let mut ranks_in_room: Vec<i32> = Vec::with_capacity(assigned_count);
    let mut scores: Vec<f64> = Vec::with_capacity(assigned_count);
    for (final_rank, (_, row)) in accepted.iter().enumerate() {
        let (ri, rank_in_room) = room_slots[final_rank];
        app_ids.push(row.application_id);
        room_ids.push(rooms[ri].room_id);
        track_ids.push(rooms[ri].track_id);
        ranks_in_track.push((final_rank + 1) as i32);
        ranks_in_room.push(rank_in_room);
        scores.push(row.total_score.unwrap_or(0.0));
    }
    let assigned_bys: Vec<Uuid> = vec![user_id; assigned_count];

    if !app_ids.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO admission_room_assignments (
                application_id, class_room_id,
                rank_in_track, rank_in_room,
                total_score, full_score,
                assigned_by, assigned_at
            )
            SELECT * FROM UNNEST(
                $1::uuid[], $2::uuid[], $3::int[], $4::int[],
                $5::double precision[], $5::double precision[],
                $6::uuid[], array_fill(NOW()::timestamptz, ARRAY[array_length($1::uuid[], 1)])
            )
            ON CONFLICT (application_id) DO UPDATE SET
                class_room_id = EXCLUDED.class_room_id,
                rank_in_track = EXCLUDED.rank_in_track,
                rank_in_room  = EXCLUDED.rank_in_room,
                total_score   = EXCLUDED.total_score,
                full_score    = EXCLUDED.full_score,
                assigned_by   = EXCLUDED.assigned_by,
                assigned_at   = EXCLUDED.assigned_at
            "#,
        )
        .bind(&app_ids)
        .bind(&room_ids)
        .bind(&ranks_in_track)
        .bind(&ranks_in_room)
        .bind(&scores)
        .bind(&assigned_bys)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to bulk assign rooms (global): {}", e);
            AppError::InternalServerError("Failed to assign rooms".to_string())
        })?;

        sqlx::query(
            "UPDATE admission_applications SET status = 'accepted', updated_at = NOW() WHERE id = ANY($1::uuid[]) AND status NOT IN ('rejected', 'withdrawn')"
        )
        .bind(&app_ids)
        .execute(&mut *tx)
        .await
        .ok();
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    if !app_ids.is_empty() {
        sqlx::query(
            r#"
            UPDATE admission_applications aa
            SET room_assignment_track_id = data.track_id,
                updated_at = NOW()
            FROM (SELECT * FROM UNNEST($1::uuid[], $2::uuid[]) AS t(app_id, track_id)) data
            WHERE aa.id = data.app_id
              AND data.track_id IS DISTINCT FROM aa.admission_track_id
            "#,
        )
        .bind(&app_ids)
        .bind(&track_ids)
        .execute(pool)
        .await
        .ok();
    }

    Ok(assigned_count)
}

pub async fn get_global_ranking(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<GlobalRankingResult, AppError> {
    let rows = sqlx::query_as::<_, RankRowDetailed>(
        r#"
        SELECT
            aa.id AS application_id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            COALESCE(SUM(esc.score), 0) AS selection_score,
            COALESCE(SUM(esc.score), 0) AS total_score,
            at_orig.name AS original_track_name,
            aa.room_assignment_track_id IS NOT NULL AS is_track_overridden,
            ara.class_room_id AS saved_room_id,
            cr_saved.name AS saved_room_name,
            aa.gender
        FROM admission_applications aa
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
        LEFT JOIN admission_tracks at_orig ON at_orig.id = aa.admission_track_id
        LEFT JOIN admission_room_assignments ara ON ara.application_id = aa.id
        LEFT JOIN class_rooms cr_saved ON cr_saved.id = ara.class_room_id
        WHERE aa.admission_round_id = $1
          AND aa.status NOT IN ('rejected', 'withdrawn', 'absent')
        GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.created_at,
                 at_orig.name, aa.room_assignment_track_id, ara.class_room_id, cr_saved.name, aa.gender
        ORDER BY total_score DESC, aa.created_at ASC
        "#
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch global ranking".to_string()))?;
    let rows: Vec<RankRowDetailed> = rows
        .into_iter()
        .map(decrypt_rank_row_detailed)
        .collect::<Result<_, _>>()?;

    #[derive(sqlx::FromRow)]
    struct CapRow {
        room_id: Uuid,
        capacity: i32,
    }

    let cap_rows = sqlx::query_as::<_, CapRow>(
        r#"
        SELECT DISTINCT cr.id AS room_id, cr.capacity
        FROM admission_room_assignments ara
        JOIN class_rooms cr ON cr.id = ara.class_room_id
        WHERE ara.application_id IN (
            SELECT id FROM admission_applications WHERE admission_round_id = $1
        )
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let cap_map: std::collections::HashMap<String, i32> = cap_rows
        .into_iter()
        .map(|r| (r.room_id.to_string(), r.capacity))
        .collect();

    let mut room_map: std::collections::BTreeMap<String, (String, i64, i64, i64, i64)> =
        std::collections::BTreeMap::new();

    let apps_json: Vec<GlobalRankingEntry> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let is_overflow = row.saved_room_id.is_none();
            let room_saved = row.saved_room_id.is_some();

            let (assigned_room, assigned_room_id, rank_in_room) = if let (Some(name), Some(id)) =
                (&row.saved_room_name, &row.saved_room_id)
            {
                let entry = room_map
                    .entry(id.to_string())
                    .or_insert((name.clone(), 0, 0, 0, 0));
                entry.1 += 1;
                entry.4 += 1;
                let rank = entry.4;
                match row.gender.as_deref() {
                    Some(g) if g.eq_ignore_ascii_case("male") || g == "ชาย" => entry.2 += 1,
                    Some(g) if g.eq_ignore_ascii_case("female") || g == "หญิง" => {
                        entry.3 += 1
                    }
                    _ => {}
                }
                (Some(name.clone()), Some(*id), Some(rank))
            } else {
                (None, None, None)
            };

            GlobalRankingEntry {
                application_id: row.application_id,
                application_number: row.application_number,
                national_id: row.national_id,
                full_name: row.full_name,
                total_score: row.total_score.unwrap_or(0.0),
                global_rank: (i + 1) as i64,
                rank_in_room,
                assigned_room,
                assigned_room_id,
                room_saved,
                is_overflow,
                original_track_name: row.original_track_name,
                gender: row.gender,
            }
        })
        .collect();

    let rooms_json: Vec<RankingRoomSummary> = room_map
        .iter()
        .map(|(rid, (name, total, male, female, _))| {
            let cap = cap_map.get(rid).copied().unwrap_or(0);
            RankingRoomSummary {
                room_id: rid.clone(),
                room_name: name.clone(),
                capacity: cap,
                student_count: *total,
                male_count: *male,
                female_count: *female,
            }
        })
        .collect();

    Ok(GlobalRankingResult {
        rooms: rooms_json,
        applications: apps_json,
    })
}

#[derive(serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RoomBasic {
    pub room_id: Uuid,
    pub room_name: String,
    pub capacity: i32,
}

pub async fn get_round_rooms(pool: &PgPool, round_id: Uuid) -> Result<Vec<RoomBasic>, AppError> {
    sqlx::query_as::<_, RoomBasic>(
        r#"
        SELECT DISTINCT cr.id AS room_id, cr.name AS room_name, cr.capacity
        FROM admission_tracks t
        JOIN study_plans sp ON t.study_plan_id = sp.id
        JOIN study_plan_versions spv ON spv.study_plan_id = sp.id AND spv.is_active = true
        JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
        WHERE t.admission_round_id = $1
          AND cr.academic_year_id = (SELECT academic_year_id FROM admission_rounds WHERE id = $1)
          AND cr.grade_level_id   = (SELECT grade_level_id   FROM admission_rounds WHERE id = $1)
        ORDER BY cr.name ASC
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch rooms".to_string()))
}

pub async fn update_selection_settings(
    pool: &PgPool,
    round_id: Uuid,
    payload: UpdateSelectionSettingsRequest,
) -> Result<(), AppError> {
    let mut settings = serde_json::json!({
        "subjectsByTrack": payload.subjects_by_track.unwrap_or(serde_json::json!({})),
        "methodByTrack": payload.method_by_track.unwrap_or(serde_json::json!({})),
        "method": payload.room_assignment_method.unwrap_or_else(|| "sequential".to_string()),
    });
    if let Some(mode) = payload.assignment_mode {
        settings["assignmentMode"] = serde_json::json!(mode);
    }
    if let Some(show) = payload.show_scores {
        settings["showScores"] = serde_json::json!(show);
    }

    sqlx::query(
        "UPDATE admission_rounds SET selection_settings = COALESCE(selection_settings, '{}'::jsonb) || $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(&settings)
    .bind(round_id)
    .execute(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to update settings".to_string()))?;

    Ok(())
}
