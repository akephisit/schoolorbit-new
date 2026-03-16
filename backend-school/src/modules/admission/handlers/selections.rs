use axum::{
    extract::{Path, State},
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
use crate::modules::admission::models::applications::*;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// GET /api/admission/rounds/:id/ranking — เรียงคะแนนทุกสายในรอบ (Preview)
pub async fn get_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        return Ok(r);
    }

    // ดึงทุก track ในรอบนี้
    let tracks: Vec<(Uuid, String, serde_json::Value, String)> = sqlx::query_as(
        "SELECT id, name, scoring_subject_ids, tiebreak_method FROM admission_tracks WHERE admission_round_id = $1 ORDER BY display_order ASC"
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch tracks".to_string()))?;

    let mut all_rankings: Vec<serde_json::Value> = Vec::new();

    for (track_id, track_name, scoring_ids, tiebreak) in tracks {
        let scoring_uuids: Vec<String> = if scoring_ids.is_array() {
            scoring_ids.as_array().unwrap()
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            Vec::new()
        };

        // สร้าง SQL ดึงคะแนนรวม
        let tiebreak_order = if tiebreak == "gpa" {
            "aa.previous_gpa DESC NULLS LAST"
        } else {
            "aa.created_at ASC"
        };

        let mut scoring_uuids_parsed: Vec<Uuid> = scoring_uuids.iter()
            .filter_map(|s| Uuid::parse_str(s).ok())
            .collect();

        // ถ้าไม่ได้ตั้งค่าวิชาคะแนน ใช้ทุกวิชาของรอบ
        if scoring_uuids_parsed.is_empty() {
            scoring_uuids_parsed = sqlx::query_scalar(
                "SELECT id FROM admission_exam_subjects WHERE admission_round_id = $1"
            )
            .bind(round_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();
        }

        #[derive(sqlx::FromRow)]
        struct RankRow {
            application_id: Uuid,
            application_number: Option<String>,
            national_id: String,
            full_name: String,
            total_score: Option<f64>,
            full_score: Option<f64>,
        }

        let query = format!(
            r#"
            SELECT
                aa.id AS application_id,
                aa.application_number,
                aa.national_id,
                CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
                COALESCE(SUM(CASE WHEN esc.exam_subject_id = ANY($1) THEN esc.score ELSE 0 END), 0) AS total_score,
                COALESCE(SUM(esc.score), 0) AS full_score
            FROM admission_applications aa
            LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
            WHERE aa.admission_track_id = $2
              AND aa.status NOT IN ('rejected', 'withdrawn')
            GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.previous_gpa, aa.created_at
            ORDER BY total_score DESC, {}
            "#,
            tiebreak_order
        );

        let rows = sqlx::query_as::<_, RankRow>(&query)
            .bind(&scoring_uuids_parsed)
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
                "fullScore": row.full_score.unwrap_or(0.0),
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

/// GET /api/admission/tracks/:id/ranking — เรียงคะแนนสายเดียว (Preview)
pub async fn get_track_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        return Ok(r);
    }

    // ดึงข้อมูล track
    let (track_name, scoring_ids, tiebreak): (String, serde_json::Value, String) = sqlx::query_as(
        "SELECT name, scoring_subject_ids, tiebreak_method FROM admission_tracks WHERE id = $1"
    )
    .bind(track_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบสายการเรียน".to_string()))?;

    let scoring_uuids: Vec<String> = if scoring_ids.is_array() {
        scoring_ids.as_array().unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else {
        Vec::new()
    };

    let tiebreak_order = if tiebreak == "gpa" {
        "aa.previous_gpa DESC NULLS LAST"
    } else {
        "aa.created_at ASC"
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
        JOIN study_plan_versions spv ON spv.study_plan_id = sp.id
        JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
        WHERE t.id = $1
        ORDER BY cr.name ASC
        "#
    )
    .bind(track_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct RankRow {
        application_id: Uuid,
        application_number: Option<String>,
        national_id: String,
        full_name: String,
        total_score: Option<f64>,
        full_score: Option<f64>,
    }

    let mut scoring_uuids_parsed: Vec<Uuid> = scoring_uuids.iter()
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();

    // ถ้าไม่ได้ตั้งค่าวิชาคะแนน ใช้ทุกวิชาของรอบ
    if scoring_uuids_parsed.is_empty() {
        scoring_uuids_parsed = sqlx::query_scalar(
            "SELECT id FROM admission_exam_subjects WHERE admission_round_id = (SELECT admission_round_id FROM admission_tracks WHERE id = $1)"
        )
        .bind(track_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
    }

    let query = format!(
        r#"
        SELECT
            aa.id AS application_id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            COALESCE(SUM(CASE WHEN esc.exam_subject_id = ANY($1) THEN esc.score ELSE 0 END), 0) AS total_score,
            COALESCE(SUM(esc.score), 0) AS full_score
        FROM admission_applications aa
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
        WHERE aa.admission_track_id = $2
          AND aa.status NOT IN ('rejected', 'withdrawn')
        GROUP BY aa.id, aa.application_number, aa.national_id, aa.first_name, aa.last_name, aa.title, aa.previous_gpa, aa.created_at
        ORDER BY total_score DESC, {}
        "#,
        tiebreak_order
    );

    let rows = sqlx::query_as::<_, RankRow>(&query)
        .bind(&scoring_uuids_parsed)
        .bind(track_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    // Preview การจัดห้อง — คนที่เกินความจุรวมได้ assignedRoom = null
    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let mut room_idx = 0usize;
    let mut count_in_room = 0i64;

    let ranked: Vec<serde_json::Value> = rows.into_iter().enumerate().map(|(i, row)| {
        let rank = (i + 1) as i64;
        if rooms.is_empty() || rank > total_capacity {
            return json!({
                "rank": rank,
                "applicationId": row.application_id,
                "applicationNumber": row.application_number,
                "nationalId": row.national_id,
                "fullName": row.full_name,
                "totalScore": row.total_score.unwrap_or(0.0),
                "fullScore": row.full_score.unwrap_or(0.0),
                "assignedRoom": null,
                "assignedRoomId": null,
            });
        }

        let assigned_room = rooms[room_idx].room_name.clone();
        let current_room_id = rooms[room_idx].room_id;

        count_in_room += 1;
        if count_in_room >= rooms[room_idx].capacity as i64 && room_idx + 1 < rooms.len() {
            room_idx += 1;
            count_in_room = 0;
        }

        json!({
            "rank": rank,
            "applicationId": row.application_id,
            "applicationNumber": row.application_number,
            "nationalId": row.national_id,
            "fullName": row.full_name,
            "totalScore": row.total_score.unwrap_or(0.0),
            "fullScore": row.full_score.unwrap_or(0.0),
            "assignedRoom": assigned_room,
            "assignedRoomId": current_room_id,
        })
    }).collect();

    Ok(Json(json!({
        "success": true,
        "data": {
            "trackId": track_id,
            "trackName": track_name,
            "rooms": rooms.iter().map(|r| json!({
                "roomId": r.room_id,
                "roomName": r.room_name,
                "capacity": r.capacity,
            })).collect::<Vec<_>>(),
            "applications": ranked,
        }
    })).into_response())
}

/// POST /api/admission/rounds/:id/assign-rooms — บันทึกการจัดห้องทุกสาย
pub async fn assign_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user = match check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    let track_id = payload.track_id;

    // ดึงข้อมูล track + scoring
    let (scoring_ids, tiebreak): (serde_json::Value, String) = sqlx::query_as(
        "SELECT scoring_subject_ids, tiebreak_method FROM admission_tracks WHERE id = $1 AND admission_round_id = $2"
    )
    .bind(track_id)
    .bind(round_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบสายการเรียน".to_string()))?;

    let scoring_uuids: Vec<String> = if scoring_ids.is_array() {
        scoring_ids.as_array().unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else {
        Vec::new()
    };

    let tiebreak_order = if tiebreak == "gpa" {
        "aa.previous_gpa DESC NULLS LAST"
    } else {
        "aa.created_at ASC"
    };

    // ดึง rooms
    #[derive(sqlx::FromRow)]
    struct RoomRow { room_id: Uuid, capacity: i32 }

    let rooms = sqlx::query_as::<_, RoomRow>(
        r#"
        SELECT cr.id AS room_id, cr.capacity
        FROM admission_tracks t
        JOIN study_plans sp ON t.study_plan_id = sp.id
        JOIN study_plan_versions spv ON spv.study_plan_id = sp.id
        JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
        WHERE t.id = $1
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

    // ดึงผู้สมัครเรียงคะแนน
    #[derive(sqlx::FromRow)]
    struct RankRow {
        application_id: Uuid,
        total_score: Option<f64>,
        full_score: Option<f64>,
    }

    let mut scoring_uuids_parsed: Vec<Uuid> = scoring_uuids.iter()
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();

    // ถ้าไม่ได้ตั้งค่าวิชาคะแนน ใช้ทุกวิชาของรอบ
    if scoring_uuids_parsed.is_empty() {
        scoring_uuids_parsed = sqlx::query_scalar(
            "SELECT id FROM admission_exam_subjects WHERE admission_round_id = $1"
        )
        .bind(round_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
    }

    let query = format!(
        r#"
        SELECT
            aa.id AS application_id,
            COALESCE(SUM(CASE WHEN esc.exam_subject_id = ANY($1) THEN esc.score ELSE 0 END), 0) AS total_score,
            COALESCE(SUM(esc.score), 0) AS full_score
        FROM admission_applications aa
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id
        WHERE aa.admission_track_id = $2
          AND aa.status NOT IN ('rejected', 'withdrawn')
        GROUP BY aa.id, aa.previous_gpa, aa.created_at
        ORDER BY total_score DESC, {}
        "#,
        tiebreak_order
    );

    let ranked = sqlx::query_as::<_, RankRow>(&query)
        .bind(&scoring_uuids_parsed)
        .bind(track_id)
        .fetch_all(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to compute ranking".to_string()))?;

    // คำนวณความจุรวม — คนที่เกินจะถูก skip (ไม่ได้รับการจัดห้อง)
    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();

    // จัดห้อง + บันทึก
    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // ลบ assignments เดิมของ track นี้
    sqlx::query(
        "DELETE FROM admission_room_assignments WHERE application_id IN (SELECT id FROM admission_applications WHERE admission_track_id = $1)"
    )
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .ok();

    let mut room_idx = 0usize;
    let mut count_in_room = 0i64;
    let mut assigned_count = 0usize;

    for (rank, row) in ranked.iter().enumerate() {
        // ข้ามคนที่เกินความจุรวม
        if (rank as i64) >= total_capacity {
            continue;
        }

        let room = &rooms[room_idx];

        let rank_in_room = count_in_room + 1;
        count_in_room += 1;
        assigned_count += 1;

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
        .bind((rank + 1) as i32)
        .bind(rank_in_room as i32)
        .bind(row.total_score)
        .bind(row.full_score)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to assign room: {}", e);
            AppError::InternalServerError("Failed to assign rooms".to_string())
        })?;

        // อัปเดต application status → accepted
        sqlx::query(
            "UPDATE admission_applications SET status = 'accepted', updated_at = NOW() WHERE id = $1 AND status NOT IN ('rejected', 'withdrawn')"
        )
        .bind(row.application_id)
        .execute(&mut *tx)
        .await
        .ok();

        // เลื่อนห้อง
        if count_in_room >= room.capacity as i64 && room_idx + 1 < rooms.len() {
            room_idx += 1;
            count_in_room = 0;
        }
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("จัดห้องสำเร็จ {} คน (ทั้งหมด {} คน, เกินความจุ {} คน)",
            assigned_count, ranked.len(), ranked.len().saturating_sub(assigned_count)),
        "data": { "assigned_count": assigned_count, "total_applicants": ranked.len() }
    })).into_response())
}
