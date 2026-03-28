use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::modules::admission::models::applications::{AssignRoomsRequest, AssignRoomsGlobalRequest};
use crate::modules::admission::models::rounds::UpdateSelectionSettingsRequest;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

#[derive(serde::Deserialize)]
pub struct RankingQuery {
    /// วิชาที่ใช้คัดเลือก (pass 1) — comma-separated UUIDs
    /// ถ้าไม่ส่ง = ใช้ทุกวิชา
    selection_subject_ids: Option<String>,
    /// "sequential" (default) หรือ "round_robin"
    room_assignment_method: Option<String>,
}

/// คำนวณ (room_idx, rank_in_room) สำหรับ student แต่ละคน ตาม method
/// capacities: capacity ของแต่ละห้อง ตามลำดับ
fn compute_room_assignments(count: usize, capacities: &[i32], method: &str) -> Vec<(usize, i32)> {
    let n = capacities.len();
    let mut result = Vec::with_capacity(count);
    if n == 0 { return result; }

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
        // sequential (default)
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

/// แปลง comma-separated UUIDs → Vec<Uuid>
fn parse_subject_ids(s: &str) -> Vec<Uuid> {
    s.split(',')
        .filter_map(|p| Uuid::parse_str(p.trim()).ok())
        .collect()
}

/// ดึง subject ids ทั้งหมดของรอบที่ track นี้อยู่
async fn all_subject_ids_for_track(pool: &sqlx::PgPool, track_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM admission_exam_subjects WHERE admission_round_id = (SELECT admission_round_id FROM admission_tracks WHERE id = $1)"
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// ดึง subject ids ทั้งหมดของรอบ
async fn all_subject_ids_for_round(pool: &sqlx::PgPool, round_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM admission_exam_subjects WHERE admission_round_id = $1"
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

#[derive(sqlx::FromRow)]
struct RankRow {
    application_id: Uuid,
    application_number: Option<String>,
    national_id: String,
    full_name: String,
    selection_score: Option<f64>,
    total_score: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct RankRowDetailed {
    application_id: Uuid,
    application_number: Option<String>,
    national_id: String,
    full_name: String,
    selection_score: Option<f64>,
    total_score: Option<f64>,
    original_track_name: Option<String>,
    is_track_overridden: Option<bool>,
    saved_room_id: Option<Uuid>,
    saved_room_name: Option<String>,
    gender: Option<String>,
}

/// GET /api/admission/rounds/:id/ranking — เรียงคะแนนทุกสายในรอบ (Preview)
pub async fn get_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    let tracks: Vec<(Uuid, String, serde_json::Value, String)> = sqlx::query_as(
        "SELECT id, name, scoring_subject_ids, tiebreak_method FROM admission_tracks WHERE admission_round_id = $1 ORDER BY display_order ASC"
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch tracks".to_string()))?;

    let mut all_rankings: Vec<serde_json::Value> = Vec::new();

    for (track_id, track_name, _scoring_ids, tiebreak) in tracks {
        let tiebreak_order = if tiebreak == "gpa" {
            "aa.previous_gpa DESC NULLS LAST"
        } else {
            "aa.created_at ASC"
        };

        let all_ids = all_subject_ids_for_round(&pool, round_id).await;

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
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

        let ranked: Vec<serde_json::Value> = rows.into_iter().enumerate().map(|(i, row)| {
            json!({
                "rank": i + 1,
                "applicationId": row.application_id,
                "applicationNumber": row.application_number,
                "nationalId": row.national_id,
                "fullName": row.full_name,
                "totalScore": row.total_score.unwrap_or(0.0),
                "selectionScore": row.selection_score.unwrap_or(0.0),
            })
        }).collect();

        all_rankings.push(json!({
            "trackId": track_id,
            "trackName": track_name,
            "applications": ranked,
        }));
    }

    Ok(Json(json!({ "success": true, "data": all_rankings })).into_response())
}

/// GET /api/admission/tracks/:id/ranking — two-pass ranking สายเดียว (Preview)
pub async fn get_track_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
    Query(params): Query<RankingQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    // ดึงข้อมูล track
    let (track_name, tiebreak): (String, String) = sqlx::query_as(
        "SELECT name, tiebreak_method FROM admission_tracks WHERE id = $1"
    )
    .bind(track_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบสายการเรียน".to_string()))?;

    let tiebreak_order = if tiebreak == "gpa" {
        "aa.previous_gpa DESC NULLS LAST"
    } else {
        "aa.created_at ASC"
    };

    // selection_subject_ids: ถ้าส่งมา = ใช้ pass 1 คัดเลือก, ถ้าไม่ส่ง = ใช้ทุกวิชา
    let selection_ids: Vec<Uuid> = match &params.selection_subject_ids {
        Some(s) if !s.is_empty() => parse_subject_ids(s),
        _ => all_subject_ids_for_track(&pool, track_id).await,
    };

    // ดึง rooms ของ track นี้
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
        "#
    )
    .bind(track_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Query: selection_score = คะแนนวิชาที่เลือก, total_score = คะแนนทุกวิชา
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
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    // Two-pass ranking
    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();

    // แบ่ง accepted / overflow
    // ถ้ามีการจัดห้องบันทึกไว้แล้ว (any_saved) → ใช้ saved_room_id เป็นตัวตัดสิน
    // ถ้ายังไม่มี → ใช้ algorithmic preview ตาม capacity ของสายนี้
    let capacity_usize = if total_capacity > 0 { total_capacity as usize } else { usize::MAX };
    let any_saved = rows.iter().any(|r| r.saved_room_id.is_some());
    let (accepted_rows, overflow_rows): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .enumerate()
        .partition(|(i, r)| {
            if any_saved { r.saved_room_id.is_some() } else { *i < capacity_usize }
        });

    // Pass 2: เรียง accepted ด้วย total_score DESC → assign ห้อง
    let mut accepted_sorted: Vec<(usize, RankRowDetailed)> = accepted_rows;
    accepted_sorted.sort_by(|(_, a), (_, b)| {
        b.total_score.unwrap_or(0.0)
            .partial_cmp(&a.total_score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let method = params.room_assignment_method.as_deref().unwrap_or("sequential");
    let room_caps: Vec<i32> = rooms.iter().map(|r| r.capacity).collect();
    let room_slots = compute_room_assignments(accepted_sorted.len(), &room_caps, method);

    // room_id (as string) → (total, male, female)
    let mut room_stats: std::collections::HashMap<String, (i64, i64, i64)> = std::collections::HashMap::new();

    let accepted_json: Vec<serde_json::Value> = accepted_sorted
        .into_iter()
        .enumerate()
        .map(|(final_i, (sel_i, row))| {
            let final_rank = (final_i + 1) as i64;
            let selection_rank = (sel_i + 1) as i64;

            let room_saved = row.saved_room_id.is_some();
            let (assigned_room, assigned_room_id) = if let (Some(name), Some(id)) = (&row.saved_room_name, &row.saved_room_id) {
                // มี record ใน DB → ใช้ห้องที่บันทึกไว้ (รวมที่ย้ายมือ)
                (json!(name), json!(id))
            } else if rooms.is_empty() {
                (serde_json::Value::Null, serde_json::Value::Null)
            } else {
                let (ri, _rir) = room_slots[final_i];
                (json!(rooms[ri].room_name), json!(rooms[ri].room_id))
            };

            // สะสม stats ต่อห้อง
            if let serde_json::Value::String(rid) = &assigned_room_id {
                let entry = room_stats.entry(rid.clone()).or_insert((0, 0, 0));
                entry.0 += 1;
                match row.gender.as_deref() {
                    Some(g) if g.eq_ignore_ascii_case("male") || g == "ชาย" => entry.1 += 1,
                    Some(g) if g.eq_ignore_ascii_case("female") || g == "หญิง" => entry.2 += 1,
                    _ => {}
                }
            }

            let is_overridden = row.is_track_overridden.unwrap_or(false);
            json!({
                "applicationId": row.application_id,
                "applicationNumber": row.application_number,
                "nationalId": row.national_id,
                "fullName": row.full_name,
                "selectionScore": row.selection_score.unwrap_or(0.0),
                "totalScore": row.total_score.unwrap_or(0.0),
                "selectionRank": selection_rank,
                "finalRank": final_rank,
                "assignedRoom": assigned_room,
                "assignedRoomId": assigned_room_id,
                "roomSaved": room_saved,
                "isOverflow": false,
                "isTrackOverridden": is_overridden,
                "originalTrackName": if is_overridden { json!(row.original_track_name) } else { serde_json::Value::Null },
            })
        })
        .collect();

    let overflow_json: Vec<serde_json::Value> = overflow_rows
        .into_iter()
        .map(|(sel_i, row)| {
            let is_overridden = row.is_track_overridden.unwrap_or(false);
            json!({
                "applicationId": row.application_id,
                "applicationNumber": row.application_number,
                "nationalId": row.national_id,
                "fullName": row.full_name,
                "selectionScore": row.selection_score.unwrap_or(0.0),
                "totalScore": row.total_score.unwrap_or(0.0),
                "selectionRank": (sel_i + 1) as i64,
                "finalRank": serde_json::Value::Null,
                "assignedRoom": serde_json::Value::Null,
                "assignedRoomId": serde_json::Value::Null,
                "isOverflow": true,
                "isTrackOverridden": is_overridden,
                "originalTrackName": if is_overridden { json!(row.original_track_name) } else { serde_json::Value::Null },
            })
        })
        .collect();

    let mut all_apps = accepted_json;
    all_apps.extend(overflow_json);

    Ok(Json(json!({
        "success": true,
        "data": {
            "trackId": track_id,
            "trackName": track_name,
            "rooms": rooms.iter().map(|r| {
                let key = r.room_id.to_string();
                let (total, male, female) = room_stats.get(&key).copied().unwrap_or((0, 0, 0));
                json!({
                    "roomId": r.room_id,
                    "roomName": r.room_name,
                    "capacity": r.capacity,
                    "studentCount": total,
                    "maleCount": male,
                    "femaleCount": female,
                })
            }).collect::<Vec<_>>(),
            "applications": all_apps,
        }
    })).into_response())
}

/// POST /api/admission/rounds/:id/assign-rooms — บันทึกการจัดห้องทุกสาย (two-pass)
pub async fn assign_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = match check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    let track_id = payload.track_id;

    // ตรวจว่า track อยู่ในรอบนี้
    let tiebreak: String = sqlx::query_scalar(
        "SELECT tiebreak_method FROM admission_tracks WHERE id = $1 AND admission_round_id = $2"
    )
    .bind(track_id)
    .bind(round_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบสายการเรียน".to_string()))?;

    let tiebreak_order = if tiebreak == "gpa" {
        "aa.previous_gpa DESC NULLS LAST"
    } else {
        "aa.created_at ASC"
    };

    // selection_subject_ids: ถ้าส่งมา = ใช้ pass 1 คัดเลือก, ถ้าไม่ส่ง = ใช้ทุกวิชา
    let selection_ids: Vec<Uuid> = match &payload.selection_subject_ids {
        Some(ids) if !ids.is_empty() => ids.clone(),
        _ => all_subject_ids_for_round(&pool, round_id).await,
    };

    // ดึง rooms
    #[derive(sqlx::FromRow)]
    struct RoomRow { room_id: Uuid, capacity: i32 }

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
        "#
    )
    .bind(track_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch rooms".to_string()))?;

    if rooms.is_empty() {
        return Err(AppError::BadRequest(
            "ไม่พบห้องเรียนสำหรับสายนี้ กรุณาสร้างห้องเรียนก่อน".to_string()
        ));
    }

    // Query: เรียงตาม selection_score DESC (pass 1)
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
        .fetch_all(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to compute ranking".to_string()))?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let capacity_usize = total_capacity as usize;

    // Pass 1: แบ่ง accepted/overflow
    let (mut accepted, _overflow): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .enumerate()
        .partition(|(i, _)| *i < capacity_usize);

    // Pass 2: เรียง accepted ด้วย total_score DESC
    accepted.sort_by(|(_, a), (_, b)| {
        b.total_score.unwrap_or(0.0)
            .partial_cmp(&a.total_score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // จัดห้อง + บันทึก
    let method = payload.room_assignment_method.as_deref().unwrap_or("sequential");
    let room_caps: Vec<i32> = rooms.iter().map(|r| r.capacity).collect();
    let room_slots = compute_room_assignments(accepted.len(), &room_caps, method);

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // ลบ assignments เดิมของ track นี้ (รวมนักเรียนที่ถูก override มาจากสายอื่น)
    sqlx::query(
        "DELETE FROM admission_room_assignments WHERE application_id IN (SELECT id FROM admission_applications WHERE COALESCE(room_assignment_track_id, admission_track_id) = $1)"
    )
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .ok();

    let assigned_count = accepted.len();

    for (final_rank, (_, row)) in accepted.iter().enumerate() {
        let (ri, rank_in_room) = room_slots[final_rank];
        let room = &rooms[ri];

        sqlx::query(
            r#"
            INSERT INTO admission_room_assignments (
                application_id, class_room_id,
                rank_in_track, rank_in_room,
                total_score, full_score,
                assigned_by, assigned_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            ON CONFLICT (application_id) DO UPDATE SET
                class_room_id = $2,
                rank_in_track = $3,
                rank_in_room  = $4,
                total_score   = $5,
                full_score    = $6,
                assigned_by   = $7,
                assigned_at   = NOW()
            "#
        )
        .bind(row.application_id)
        .bind(room.room_id)
        .bind((final_rank + 1) as i32)
        .bind(rank_in_room)
        .bind(row.total_score)
        .bind(row.total_score)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to assign room: {}", e);
            AppError::InternalServerError("Failed to assign rooms".to_string())
        })?;

        sqlx::query(
            "UPDATE admission_applications SET status = 'accepted', updated_at = NOW() WHERE id = $1 AND status NOT IN ('rejected', 'withdrawn')"
        )
        .bind(row.application_id)
        .execute(&mut *tx)
        .await
        .ok();
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("จัดห้องสำเร็จ {} คน", assigned_count),
        "data": { "assigned_count": assigned_count }
    })).into_response())
}

/// DELETE /api/admission/tracks/:id/room-assignments — ล้างการจัดห้องของ track นี้ทั้งหมด
pub async fn reset_room_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        WITH deleted AS (
            DELETE FROM admission_room_assignments
            WHERE application_id IN (
                SELECT id FROM admission_applications
                WHERE COALESCE(room_assignment_track_id, admission_track_id) = $1
            )
            RETURNING 1
        )
        SELECT COUNT(*) FROM deleted
        "#
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    Ok(Json(json!({ "success": true, "deleted": deleted })).into_response())
}

/// POST /api/admission/rounds/:id/assign-rooms-global — จัดห้องรวมทุกสาย (ไม่แยกตามสาย)
pub async fn assign_rooms_global(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsGlobalRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = match check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    // ดึงห้องทั้งหมดจากทุก track ในรอบ (DISTINCT เพื่อไม่ซ้ำ) เรียงตามชื่อห้อง
    #[derive(sqlx::FromRow)]
    struct RoomRow { room_id: Uuid, room_name: String, capacity: i32 }

    let rooms = sqlx::query_as::<_, RoomRow>(
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
        "#
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch rooms".to_string()))?;

    if rooms.is_empty() {
        return Err(AppError::BadRequest(
            "ไม่พบห้องเรียนในรอบนี้ กรุณาสร้างห้องเรียนก่อน".to_string()
        ));
    }

    // ดึงนักเรียนทั้งหมดในรอบ (ทุกสาย) เรียงตาม total_score DESC
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
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch students".to_string()))?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let capacity_usize = total_capacity as usize;

    // แบ่ง accepted / overflow ตาม total capacity รวม
    let (accepted, _overflow): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .enumerate()
        .partition(|(i, _)| *i < capacity_usize);

    let method = payload.room_assignment_method.as_deref().unwrap_or("sequential");
    let room_caps: Vec<i32> = rooms.iter().map(|r| r.capacity).collect();
    let room_slots = compute_room_assignments(accepted.len(), &room_caps, method);

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // ลบ assignments เดิมของทุกคนในรอบ
    sqlx::query(
        "DELETE FROM admission_room_assignments WHERE application_id IN (SELECT id FROM admission_applications WHERE admission_round_id = $1)"
    )
    .bind(round_id)
    .execute(&mut *tx)
    .await
    .ok();

    let assigned_count = accepted.len();

    for (final_rank, (_, row)) in accepted.iter().enumerate() {
        let (ri, rank_in_room) = room_slots[final_rank];
        let room = &rooms[ri];

        sqlx::query(
            r#"
            INSERT INTO admission_room_assignments (
                application_id, class_room_id,
                rank_in_track, rank_in_room,
                total_score, full_score,
                assigned_by, assigned_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            ON CONFLICT (application_id) DO UPDATE SET
                class_room_id = $2,
                rank_in_track = $3,
                rank_in_room  = $4,
                total_score   = $5,
                full_score    = $6,
                assigned_by   = $7,
                assigned_at   = NOW()
            "#
        )
        .bind(row.application_id)
        .bind(room.room_id)
        .bind((final_rank + 1) as i32)
        .bind(rank_in_room)
        .bind(row.total_score)
        .bind(row.total_score)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to assign room (global): {}", e);
            AppError::InternalServerError("Failed to assign rooms".to_string())
        })?;

        sqlx::query(
            "UPDATE admission_applications SET status = 'accepted', updated_at = NOW() WHERE id = $1 AND status NOT IN ('rejected', 'withdrawn')"
        )
        .bind(row.application_id)
        .execute(&mut *tx)
        .await
        .ok();
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("จัดห้องรวมสำเร็จ {} คน", assigned_count),
        "data": { "assigned_count": assigned_count }
    })).into_response())
}

/// GET /api/admission/rounds/:id/global-ranking — ดูผลจัดห้องรวมทุกสาย
pub async fn get_global_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    // ดึงนักเรียนทุกสายในรอบ พร้อม saved room
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
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch global ranking".to_string()))?;

    // ดึง capacity ของห้องที่มีนักเรียน
    #[derive(sqlx::FromRow)]
    struct CapRow { room_id: Uuid, capacity: i32 }

    let cap_rows = sqlx::query_as::<_, CapRow>(
        r#"
        SELECT DISTINCT cr.id AS room_id, cr.capacity
        FROM admission_room_assignments ara
        JOIN class_rooms cr ON cr.id = ara.class_room_id
        WHERE ara.application_id IN (
            SELECT id FROM admission_applications WHERE admission_round_id = $1
        )
        "#
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let cap_map: std::collections::HashMap<String, i32> = cap_rows
        .into_iter()
        .map(|r| (r.room_id.to_string(), r.capacity))
        .collect();

    // สะสม room stats
    let mut room_map: std::collections::BTreeMap<String, (String, i64, i64, i64)> =
        std::collections::BTreeMap::new();

    let apps_json: Vec<serde_json::Value> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let is_overflow = row.saved_room_id.is_none();
            let room_saved = row.saved_room_id.is_some();

            let (assigned_room, assigned_room_id) =
                if let (Some(name), Some(id)) = (&row.saved_room_name, &row.saved_room_id) {
                    let entry = room_map.entry(id.to_string()).or_insert((name.clone(), 0, 0, 0));
                    entry.1 += 1;
                    match row.gender.as_deref() {
                        Some(g) if g.eq_ignore_ascii_case("male") || g == "ชาย" => entry.2 += 1,
                        Some(g) if g.eq_ignore_ascii_case("female") || g == "หญิง" => entry.3 += 1,
                        _ => {}
                    }
                    (json!(name), json!(id))
                } else {
                    (serde_json::Value::Null, serde_json::Value::Null)
                };

            json!({
                "applicationId": row.application_id,
                "applicationNumber": row.application_number,
                "nationalId": row.national_id,
                "fullName": row.full_name,
                "totalScore": row.total_score.unwrap_or(0.0),
                "globalRank": (i + 1) as i64,
                "assignedRoom": assigned_room,
                "assignedRoomId": assigned_room_id,
                "roomSaved": room_saved,
                "isOverflow": is_overflow,
                "originalTrackName": row.original_track_name,
            })
        })
        .collect();

    let rooms_json: Vec<serde_json::Value> = room_map
        .iter()
        .map(|(rid, (name, total, male, female))| {
            let cap = cap_map.get(rid).copied().unwrap_or(0);
            json!({
                "roomId": rid,
                "roomName": name,
                "capacity": cap,
                "studentCount": total,
                "maleCount": male,
                "femaleCount": female,
            })
        })
        .collect();

    Ok(Json(json!({
        "success": true,
        "data": {
            "rooms": rooms_json,
            "applications": apps_json,
        }
    })).into_response())
}

/// PATCH /api/admission/rounds/:id/selection-settings — บันทึกการตั้งค่า selections ลง DB (แชร์ระหว่าง staff)
pub async fn update_selection_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateSelectionSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    let settings = serde_json::json!({
        "subjectsByTrack": payload.subjects_by_track.unwrap_or(serde_json::json!({})),
        "methodByTrack": payload.method_by_track.unwrap_or(serde_json::json!({})),
        "method": payload.room_assignment_method.unwrap_or_else(|| "sequential".to_string()),
        "assignmentMode": payload.assignment_mode.unwrap_or_else(|| "per_track".to_string()),
    });

    sqlx::query(
        "UPDATE admission_rounds SET selection_settings = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(&settings)
    .bind(round_id)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to update settings".to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}
