use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ==========================================
// Models
// ==========================================

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExamRoomRow {
    id: Uuid,
    room_id: Option<Uuid>,
    room_name: String,
    building_name: Option<String>,
    capacity: i32,
    display_order: i32,
    assigned_count: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddExamRoomRequest {
    room_id: Option<Uuid>,       // จาก rooms table
    custom_name: Option<String>, // หรือชื่อ custom
    capacity_override: Option<i32>,
    display_order: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamConfigRequest {
    /// "application_number" | "sequential" | "custom_prefix"
    exam_id_type: Option<String>,
    exam_id_prefix: Option<String>,
    /// "by_application" | "by_track" | "random"
    sort_order: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignExamSeatsRequest {
    /// override config ตอนรัน (ไม่บันทึก)
    exam_id_type: Option<String>,
    exam_id_prefix: Option<String>,
    sort_order: Option<String>,
    /// "full" = ลบเดิมแล้วจัดใหม่ทั้งหมด (default), "append" = เพิ่มเฉพาะคนที่ยังไม่มีที่นั่ง
    mode: Option<String>,
}

// ==========================================
// GET /rounds/:id/exam-rooms
// ==========================================

pub async fn list_exam_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    let rooms = sqlx::query_as::<_, ExamRoomRow>(
        r#"SELECT
            er.id,
            er.room_id,
            COALESCE(er.custom_name, r.name_th, r.name_en) AS room_name,
            b.name_th AS building_name,
            COALESCE(er.capacity_override, r.capacity, 40)::INT AS capacity,
            er.display_order,
            COUNT(sa.id)::BIGINT AS assigned_count
           FROM admission_exam_rooms er
           LEFT JOIN rooms r ON r.id = er.room_id
           LEFT JOIN buildings b ON b.id = r.building_id
           LEFT JOIN admission_exam_seat_assignments sa ON sa.exam_room_id = er.id
           WHERE er.admission_round_id = $1
           GROUP BY er.id, er.room_id, er.custom_name, r.name_th, r.name_en, b.name_th,
                    er.capacity_override, r.capacity, er.display_order
           ORDER BY er.display_order ASC, er.created_at ASC"#,
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to list exam rooms: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลห้องสอบได้".to_string())
    })?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let total_assigned: i64 = rooms.iter().map(|r| r.assigned_count).sum();

    Ok(Json(json!({
        "success": true,
        "data": {
            "rooms": rooms,
            "totalCapacity": total_capacity,
            "totalAssigned": total_assigned,
        }
    })).into_response())
}

// ==========================================
// POST /rounds/:id/exam-rooms
// ==========================================

pub async fn add_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AddExamRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    if payload.room_id.is_none() && payload.custom_name.is_none() {
        return Err(AppError::BadRequest("ต้องระบุ room_id หรือ custom_name".to_string()));
    }

    // หา display_order ต่อจากของที่มีอยู่
    let max_order: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(display_order) FROM admission_exam_rooms WHERE admission_round_id = $1"
    )
    .bind(round_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None)
    .flatten();

    let display_order = payload.display_order.unwrap_or_else(|| max_order.unwrap_or(-1) + 1);

    sqlx::query(
        r#"INSERT INTO admission_exam_rooms
           (admission_round_id, room_id, custom_name, capacity_override, display_order)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(round_id)
    .bind(payload.room_id)
    .bind(&payload.custom_name)
    .bind(payload.capacity_override)
    .bind(display_order)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to add exam room: {}", e);
        AppError::InternalServerError("ไม่สามารถเพิ่มห้องสอบได้".to_string())
    })?;

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// PUT /rounds/:id/exam-rooms/:room_id  (แก้ความจุ)
// ==========================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamRoomRequest {
    capacity_override: Option<i32>,
    display_order: Option<i32>,
    custom_name: Option<String>,
}

pub async fn update_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, room_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateExamRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    let result = sqlx::query(
        r#"UPDATE admission_exam_rooms
           SET capacity_override = COALESCE($3, capacity_override),
               display_order = COALESCE($4, display_order),
               custom_name = COALESCE($5, custom_name)
           WHERE id = $1 AND admission_round_id = $2"#,
    )
    .bind(room_id)
    .bind(round_id)
    .bind(payload.capacity_override)
    .bind(payload.display_order)
    .bind(&payload.custom_name)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update exam room: {}", e);
        AppError::InternalServerError("ไม่สามารถอัปเดตห้องสอบได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบห้องสอบ".to_string()));
    }

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// DELETE /rounds/:id/exam-rooms/:room_id
// ==========================================

pub async fn remove_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, room_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    let result = sqlx::query(
        "DELETE FROM admission_exam_rooms WHERE id = $1 AND admission_round_id = $2"
    )
    .bind(room_id)
    .bind(round_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to remove exam room: {}", e);
        AppError::InternalServerError("ไม่สามารถลบห้องสอบได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบห้องสอบ".to_string()));
    }

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// POST /rounds/:id/exam-rooms/copy-from/:from_id
// ==========================================

pub async fn copy_exam_rooms_from_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, from_round_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    // ลบห้องเดิมออกก่อน (ถ้ามี)
    sqlx::query("DELETE FROM admission_exam_rooms WHERE admission_round_id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .ok();

    let result = sqlx::query(
        r#"INSERT INTO admission_exam_rooms
           (admission_round_id, room_id, custom_name, capacity_override, display_order)
           SELECT $1, room_id, custom_name, capacity_override, display_order
           FROM admission_exam_rooms
           WHERE admission_round_id = $2
           ORDER BY display_order ASC"#,
    )
    .bind(round_id)
    .bind(from_round_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to copy exam rooms: {}", e);
        AppError::InternalServerError("ไม่สามารถ copy ห้องสอบได้".to_string())
    })?;

    Ok(Json(json!({
        "success": true,
        "message": format!("copy ห้องสอบ {} ห้องเรียบร้อย", result.rows_affected())
    })).into_response())
}

// ==========================================
// PUT /rounds/:id/exam-config
// ==========================================

pub async fn update_exam_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateExamConfigRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    // Merge กับ config ที่มีอยู่
    let mut updates = serde_json::Map::new();
    if let Some(v) = payload.exam_id_type {
        updates.insert("exam_id_type".to_string(), json!(v));
    }
    if let Some(v) = payload.exam_id_prefix {
        updates.insert("exam_id_prefix".to_string(), json!(v));
    }
    if let Some(v) = payload.sort_order {
        updates.insert("sort_order".to_string(), json!(v));
    }

    sqlx::query(
        "UPDATE admission_rounds SET exam_config = exam_config || $2 WHERE id = $1"
    )
    .bind(round_id)
    .bind(serde_json::Value::Object(updates))
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update exam config: {}", e);
        AppError::InternalServerError("ไม่สามารถอัปเดต config ได้".to_string())
    })?;

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// GET /rounds/:id/exam-config
// ==========================================

pub async fn get_exam_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    let config: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT exam_config FROM admission_rounds WHERE id = $1"
    )
    .bind(round_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .flatten();

    Ok(Json(json!({
        "success": true,
        "data": config.unwrap_or_else(|| json!({}))
    })).into_response())
}

// ==========================================
// POST /rounds/:id/assign-exam-seats
// ==========================================

pub async fn assign_exam_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignExamSeatsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user = match check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    // ดึง exam_config
    let config: serde_json::Value = sqlx::query_scalar(
        "SELECT exam_config FROM admission_rounds WHERE id = $1"
    )
    .bind(round_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .flatten()
    .unwrap_or_else(|| json!({}));

    let exam_id_type = payload.exam_id_type
        .or_else(|| config["exam_id_type"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "application_number".to_string());
    let exam_id_prefix = payload.exam_id_prefix
        .or_else(|| config["exam_id_prefix"].as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let sort_order = payload.sort_order
        .or_else(|| config["sort_order"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "by_application".to_string());

    // ดึงผู้สมัคร
    #[derive(sqlx::FromRow)]
    struct AppRow {
        id: Uuid,
        application_number: Option<String>,
        track_name: Option<String>,
    }

    let order_clause = match sort_order.as_str() {
        "by_track" => "ORDER BY at.name ASC NULLS LAST, aa.application_number ASC",
        "random" => "ORDER BY random()",
        _ => "ORDER BY aa.application_number ASC",
    };

    let query = format!(
        r#"SELECT aa.id, aa.application_number, at.name AS track_name
           FROM admission_applications aa
           LEFT JOIN admission_tracks at ON at.id = aa.admission_track_id
           WHERE aa.admission_round_id = $1
             AND aa.status = 'verified'
           {}"#,
        order_clause
    );

    let applicants = sqlx::query_as::<_, AppRow>(&query)
        .bind(round_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch applicants: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้สมัครได้".to_string())
        })?;

    if applicants.is_empty() {
        return Err(AppError::BadRequest("ไม่มีผู้สมัครที่ eligible".to_string()));
    }

    // ดึงห้องสอบพร้อม capacity
    #[derive(sqlx::FromRow)]
    struct RoomCapRow {
        id: Uuid,
        room_name: String,
        capacity: i32,
    }

    let rooms = sqlx::query_as::<_, RoomCapRow>(
        r#"SELECT
            er.id,
            COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
            COALESCE(er.capacity_override, r.capacity, 40)::INT AS capacity
           FROM admission_exam_rooms er
           LEFT JOIN rooms r ON r.id = er.room_id
           WHERE er.admission_round_id = $1
           ORDER BY er.display_order ASC, er.created_at ASC"#,
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch exam rooms: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลห้องสอบได้".to_string())
    })?;

    if rooms.is_empty() {
        return Err(AppError::BadRequest("ยังไม่มีห้องสอบ กรุณาเพิ่มห้องสอบก่อน".to_string()));
    }

    let mode = payload.mode.as_deref().unwrap_or("full");

    // ===== Append mode: เพิ่มเฉพาะคนที่ยังไม่มีที่นั่ง =====
    if mode == "append" {
        #[derive(sqlx::FromRow)]
        struct ExistingRow {
            application_id: Uuid,
            exam_room_id: Uuid,
        }

        let existing: Vec<ExistingRow> = sqlx::query_as(
            r#"SELECT application_id, exam_room_id
               FROM admission_exam_seat_assignments
               WHERE exam_room_id IN (
                   SELECT id FROM admission_exam_rooms WHERE admission_round_id = $1
               )"#
        )
        .bind(round_id)
        .fetch_all(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถดึงข้อมูลที่นั่งเดิมได้".to_string()))?;

        let existing_app_ids: std::collections::HashSet<Uuid> =
            existing.iter().map(|r| r.application_id).collect();
        let existing_total = existing.len() as i32;

        // นับจำนวนที่นั่งที่ใช้แล้วต่อห้อง
        let mut existing_counts: std::collections::HashMap<Uuid, i32> = std::collections::HashMap::new();
        for r in &existing {
            *existing_counts.entry(r.exam_room_id).or_insert(0) += 1;
        }

        let new_applicants: Vec<_> = applicants.into_iter()
            .filter(|a| !existing_app_ids.contains(&a.id))
            .collect();

        if new_applicants.is_empty() {
            return Ok(Json(json!({
                "success": true,
                "message": "ไม่มีผู้สมัครใหม่ที่ต้องจัดที่นั่ง",
                "data": { "assignedCount": 0, "rooms": [] }
            })).into_response());
        }

        // ตรวจ remaining capacity
        let remaining_capacity: i32 = rooms.iter()
            .map(|r| r.capacity - existing_counts.get(&r.id).copied().unwrap_or(0))
            .sum();
        if remaining_capacity < new_applicants.len() as i32 {
            return Err(AppError::BadRequest(format!(
                "ที่นั่งว่างเหลือ ({}) น้อยกว่าจำนวนผู้สมัครใหม่ ({}) — กรุณาเพิ่มห้องสอบหรือจัดใหม่ทั้งหมด",
                remaining_capacity,
                new_applicants.len()
            )));
        }

        let pad_width = format!("{}", existing_total + new_applicants.len() as i32).len().max(4);
        let mut new_assignments: Vec<(Uuid, Uuid, i32, String)> = Vec::new();
        let mut room_iter = rooms.iter();
        let mut current_room = room_iter.next().unwrap();
        // ข้ามห้องที่เต็มแล้ว
        while existing_counts.get(&current_room.id).copied().unwrap_or(0) >= current_room.capacity {
            current_room = room_iter.next().unwrap();
        }
        let mut seat_in_room = existing_counts.get(&current_room.id).copied().unwrap_or(0);
        let mut global_seq = existing_total;

        for app in &new_applicants {
            while seat_in_room >= current_room.capacity {
                current_room = room_iter.next().unwrap();
                seat_in_room = existing_counts.get(&current_room.id).copied().unwrap_or(0);
            }
            seat_in_room += 1;
            global_seq += 1;

            let exam_id = match exam_id_type.as_str() {
                "sequential" => format!("{:0>width$}", global_seq, width = pad_width),
                "custom_prefix" => format!("{}{:0>width$}", exam_id_prefix, global_seq, width = pad_width),
                _ => app.application_number.clone().unwrap_or_else(|| format!("{}", global_seq)),
            };
            new_assignments.push((app.id, current_room.id, seat_in_room, exam_id));
        }

        let mut tx = pool.begin().await
            .map_err(|_| AppError::InternalServerError("Transaction error".to_string()))?;

        for (app_id, room_id, seat_num, exam_id) in &new_assignments {
            sqlx::query(
                r#"INSERT INTO admission_exam_seat_assignments
                   (application_id, exam_room_id, seat_number, exam_id, assigned_by)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(app_id)
            .bind(room_id)
            .bind(seat_num)
            .bind(exam_id)
            .bind(user.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("Failed to insert seat assignment (append): {}", e);
                AppError::InternalServerError("ไม่สามารถบันทึกที่นั่งสอบได้".to_string())
            })?;
        }

        tx.commit().await
            .map_err(|_| AppError::InternalServerError("Transaction commit failed".to_string()))?;

        let mut room_summary: std::collections::HashMap<Uuid, (String, i32)> = std::collections::HashMap::new();
        for r in &rooms {
            room_summary.insert(r.id, (r.room_name.clone(), 0));
        }
        for (_, room_id, _, _) in &new_assignments {
            if let Some(entry) = room_summary.get_mut(room_id) {
                entry.1 += 1;
            }
        }
        let summary: Vec<serde_json::Value> = rooms.iter()
            .filter_map(|r| room_summary.get(&r.id).map(|(name, count)| json!({
                "roomName": name,
                "count": count
            })))
            .collect();

        return Ok(Json(json!({
            "success": true,
            "message": format!("เพิ่มที่นั่งสอบสำเร็จ {} คน", new_assignments.len()),
            "data": { "assignedCount": new_assignments.len(), "rooms": summary }
        })).into_response());
    }

    // ===== Full mode (default): จัดใหม่ทั้งหมด =====
    let total_capacity: i32 = rooms.iter().map(|r| r.capacity).sum();
    if total_capacity < applicants.len() as i32 {
        return Err(AppError::BadRequest(format!(
            "ความจุห้องสอบรวม ({}) น้อยกว่าจำนวนผู้สมัคร ({}) — ขาดอีก {} ที่นั่ง",
            total_capacity,
            applicants.len(),
            applicants.len() as i32 - total_capacity
        )));
    }

    // Algorithm: เติมทีละห้อง
    let pad_width = format!("{}", applicants.len()).len().max(4);
    let mut assignments: Vec<(Uuid, Uuid, i32, String)> = Vec::new(); // (app_id, room_id, seat_num, exam_id)
    let mut room_iter = rooms.iter();
    let mut current_room = room_iter.next().unwrap();
    let mut seat_in_room = 0i32;
    let mut global_seq = 0i32;

    for app in &applicants {
        // ย้ายไปห้องถัดไปถ้าเต็ม
        while seat_in_room >= current_room.capacity {
            current_room = room_iter.next().unwrap(); // ไม่ panic เพราะตรวจ capacity แล้ว
            seat_in_room = 0;
        }

        seat_in_room += 1;
        global_seq += 1;

        let exam_id = match exam_id_type.as_str() {
            "sequential" => format!("{:0>width$}", global_seq, width = pad_width),
            "custom_prefix" => format!("{}{:0>width$}", exam_id_prefix, global_seq, width = pad_width),
            _ => app.application_number.clone().unwrap_or_else(|| format!("{}", global_seq)),
        };

        assignments.push((app.id, current_room.id, seat_in_room, exam_id));
    }

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction error".to_string()))?;

    // ลบ seat assignments เดิมทั้งหมดของรอบนี้
    sqlx::query(
        r#"DELETE FROM admission_exam_seat_assignments
           WHERE exam_room_id IN (
               SELECT id FROM admission_exam_rooms WHERE admission_round_id = $1
           )"#
    )
    .bind(round_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("ไม่สามารถล้างข้อมูลเดิมได้".to_string()))?;

    // Insert ใหม่
    for (app_id, room_id, seat_num, exam_id) in &assignments {
        sqlx::query(
            r#"INSERT INTO admission_exam_seat_assignments
               (application_id, exam_room_id, seat_number, exam_id, assigned_by)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(app_id)
        .bind(room_id)
        .bind(seat_num)
        .bind(exam_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to insert seat assignment: {}", e);
            AppError::InternalServerError("ไม่สามารถบันทึกที่นั่งสอบได้".to_string())
        })?;
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Transaction commit failed".to_string()))?;

    // สรุปต่อห้อง
    let mut room_summary: std::collections::HashMap<Uuid, (String, i32)> = std::collections::HashMap::new();
    for r in &rooms {
        room_summary.insert(r.id, (r.room_name.clone(), 0));
    }
    for (_, room_id, _, _) in &assignments {
        if let Some(entry) = room_summary.get_mut(room_id) {
            entry.1 += 1;
        }
    }

    let summary: Vec<serde_json::Value> = rooms.iter()
        .filter_map(|r| room_summary.get(&r.id).map(|(name, count)| json!({
            "roomName": name,
            "count": count
        })))
        .collect();

    Ok(Json(json!({
        "success": true,
        "message": format!("จัดที่นั่งสอบสำเร็จ {} คน", assignments.len()),
        "data": {
            "assignedCount": assignments.len(),
            "rooms": summary
        }
    })).into_response())
}

// ==========================================
// GET /rounds/:id/exam-seats  (ดูผลจัดกลุ่มตามห้อง)
// ==========================================

pub async fn get_exam_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL).await {
        return Ok(r);
    }

    #[derive(sqlx::FromRow, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SeatRow {
        exam_room_id: Uuid,
        room_name: String,
        building_name: Option<String>,
        capacity: i32,
        seat_number: i32,
        exam_id: Option<String>,
        application_id: Uuid,
        application_number: Option<String>,
        full_name: String,
        national_id: String,
        track_name: Option<String>,
    }

    let rows = sqlx::query_as::<_, SeatRow>(
        r#"SELECT
            er.id AS exam_room_id,
            COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
            b.name_th AS building_name,
            COALESCE(er.capacity_override, r.capacity, 40)::INT AS capacity,
            sa.seat_number,
            sa.exam_id,
            aa.id AS application_id,
            aa.application_number,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            aa.national_id,
            at.name AS track_name
           FROM admission_exam_seat_assignments sa
           JOIN admission_exam_rooms er ON er.id = sa.exam_room_id
           JOIN admission_applications aa ON aa.id = sa.application_id
           LEFT JOIN admission_tracks at ON at.id = aa.admission_track_id
           LEFT JOIN rooms r ON r.id = er.room_id
           LEFT JOIN buildings b ON b.id = r.building_id
           WHERE er.admission_round_id = $1
           ORDER BY er.display_order ASC, er.created_at ASC, sa.seat_number ASC"#,
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch exam seats: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลที่นั่งสอบได้".to_string())
    })?;

    // จัดกลุ่มตามห้อง
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct RoomGroup {
        exam_room_id: Uuid,
        room_name: String,
        building_name: Option<String>,
        capacity: i32,
        seats: Vec<SeatRow>,
    }

    let mut groups: Vec<RoomGroup> = Vec::new();
    for row in rows {
        if let Some(last) = groups.last_mut() {
            if last.exam_room_id == row.exam_room_id {
                last.seats.push(row);
                continue;
            }
        }
        groups.push(RoomGroup {
            exam_room_id: row.exam_room_id,
            room_name: row.room_name.clone(),
            building_name: row.building_name.clone(),
            capacity: row.capacity,
            seats: vec![row],
        });
    }

    Ok(Json(json!({
        "success": true,
        "data": groups
    })).into_response())
}

// ==========================================
// GET /applications/:id/exam-seat
// ==========================================

pub async fn get_application_exam_seat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL).await {
        return Ok(r);
    }

    #[derive(sqlx::FromRow, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ExamSeatDetail {
        seat_number: i32,
        exam_id: Option<String>,
        room_name: String,
        building_name: Option<String>,
        exam_date: Option<chrono::DateTime<chrono::Utc>>,
    }

    let seat = sqlx::query_as::<_, ExamSeatDetail>(
        r#"SELECT
            sa.seat_number,
            sa.exam_id,
            COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
            b.name_th AS building_name,
            ar.exam_date
           FROM admission_exam_seat_assignments sa
           JOIN admission_exam_rooms er ON er.id = sa.exam_room_id
           JOIN admission_rounds ar ON ar.id = er.admission_round_id
           LEFT JOIN rooms r ON r.id = er.room_id
           LEFT JOIN buildings b ON b.id = r.building_id
           WHERE sa.application_id = $1"#,
    )
    .bind(application_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "data": seat
    })).into_response())
}
