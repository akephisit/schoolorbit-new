use crate::error::AppError;
use crate::modules::admission::services::pii;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamRoomRow {
    pub id: Uuid,
    pub room_id: Option<Uuid>,
    pub room_name: String,
    pub building_name: Option<String>,
    pub capacity: i32,
    pub display_order: i32,
    pub assigned_count: i64,
}

pub struct ListExamRoomsResult {
    pub rooms: Vec<ExamRoomRow>,
    pub total_capacity: i64,
    pub total_assigned: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct ExamConfigStorage {
    exam_id_type: Option<String>,
    exam_id_prefix: Option<String>,
    sort_order: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamConfigResponse {
    pub exam_id_type: Option<String>,
    pub exam_id_prefix: Option<String>,
    pub sort_order: Option<String>,
}

impl From<ExamConfigStorage> for ExamConfigResponse {
    fn from(config: ExamConfigStorage) -> Self {
        Self {
            exam_id_type: config.exam_id_type,
            exam_id_prefix: config.exam_id_prefix,
            sort_order: config.sort_order,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignSeatsRoomSummary {
    pub room_name: String,
    pub count: i32,
}

pub async fn list_exam_rooms(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ListExamRoomsResult, AppError> {
    let rooms = sqlx::query_as::<_, ExamRoomRow>(
        r#"SELECT er.id, er.room_id,
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
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to list exam rooms: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลห้องสอบได้".to_string())
    })?;

    let total_capacity: i64 = rooms.iter().map(|r| r.capacity as i64).sum();
    let total_assigned: i64 = rooms.iter().map(|r| r.assigned_count).sum();

    Ok(ListExamRoomsResult {
        rooms,
        total_capacity,
        total_assigned,
    })
}

pub async fn add_exam_room(
    pool: &PgPool,
    round_id: Uuid,
    room_id: Option<Uuid>,
    custom_name: Option<String>,
    capacity_override: Option<i32>,
    display_order: Option<i32>,
) -> Result<(), AppError> {
    if room_id.is_none() && custom_name.is_none() {
        return Err(AppError::BadRequest(
            "ต้องระบุ room_id หรือ custom_name".to_string(),
        ));
    }

    let max_order: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(display_order) FROM admission_exam_rooms WHERE admission_round_id = $1",
    )
    .bind(round_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .flatten();

    let display_order = display_order.unwrap_or_else(|| max_order.unwrap_or(-1) + 1);

    sqlx::query(
        r#"INSERT INTO admission_exam_rooms
           (admission_round_id, room_id, custom_name, capacity_override, display_order)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(round_id)
    .bind(room_id)
    .bind(&custom_name)
    .bind(capacity_override)
    .bind(display_order)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to add exam room: {}", e);
        AppError::InternalServerError("ไม่สามารถเพิ่มห้องสอบได้".to_string())
    })?;
    Ok(())
}

pub async fn update_exam_room(
    pool: &PgPool,
    round_id: Uuid,
    room_id: Uuid,
    capacity_override: Option<i32>,
    display_order: Option<i32>,
    custom_name: Option<String>,
) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"UPDATE admission_exam_rooms
           SET capacity_override = COALESCE($3, capacity_override),
               display_order = COALESCE($4, display_order),
               custom_name = COALESCE($5, custom_name)
           WHERE id = $1 AND admission_round_id = $2"#,
    )
    .bind(room_id)
    .bind(round_id)
    .bind(capacity_override)
    .bind(display_order)
    .bind(&custom_name)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update exam room: {}", e);
        AppError::InternalServerError("ไม่สามารถอัปเดตห้องสอบได้".to_string())
    })?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบห้องสอบ".to_string()));
    }
    Ok(())
}

pub async fn remove_exam_room(
    pool: &PgPool,
    round_id: Uuid,
    room_id: Uuid,
) -> Result<(), AppError> {
    let result =
        sqlx::query("DELETE FROM admission_exam_rooms WHERE id = $1 AND admission_round_id = $2")
            .bind(room_id)
            .bind(round_id)
            .execute(pool)
            .await
            .map_err(|e| {
                eprintln!("Failed to remove exam room: {}", e);
                AppError::InternalServerError("ไม่สามารถลบห้องสอบได้".to_string())
            })?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบห้องสอบ".to_string()));
    }
    Ok(())
}

pub async fn copy_exam_rooms_from_round(
    pool: &PgPool,
    round_id: Uuid,
    from_round_id: Uuid,
) -> Result<u64, AppError> {
    sqlx::query("DELETE FROM admission_exam_rooms WHERE admission_round_id = $1")
        .bind(round_id)
        .execute(pool)
        .await
        .ok();

    let result = sqlx::query(
        r#"INSERT INTO admission_exam_rooms
           (admission_round_id, room_id, custom_name, capacity_override, display_order)
           SELECT $1, room_id, custom_name, capacity_override, display_order
           FROM admission_exam_rooms WHERE admission_round_id = $2
           ORDER BY display_order ASC"#,
    )
    .bind(round_id)
    .bind(from_round_id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to copy exam rooms: {}", e);
        AppError::InternalServerError("ไม่สามารถ copy ห้องสอบได้".to_string())
    })?;
    Ok(result.rows_affected())
}

async fn load_exam_config_storage(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Option<ExamConfigStorage>, AppError> {
    let config = sqlx::query_scalar::<_, Json<ExamConfigStorage>>(
        "SELECT COALESCE(exam_config, '{}'::jsonb) FROM admission_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to load exam config: {}", e);
        AppError::InternalServerError("ไม่สามารถดึง config ได้".to_string())
    })?;

    let config = config.map(|Json(config)| config);
    Ok(config)
}

pub async fn update_exam_config(
    pool: &PgPool,
    round_id: Uuid,
    exam_id_type: Option<String>,
    exam_id_prefix: Option<String>,
    sort_order: Option<String>,
) -> Result<(), AppError> {
    let mut config = load_exam_config_storage(pool, round_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()))?;

    if let Some(value) = exam_id_type {
        config.exam_id_type = Some(value);
    }
    if let Some(value) = exam_id_prefix {
        config.exam_id_prefix = Some(value);
    }
    if let Some(value) = sort_order {
        config.sort_order = Some(value);
    }

    sqlx::query("UPDATE admission_rounds SET exam_config = $2 WHERE id = $1")
        .bind(round_id)
        .bind(Json(config))
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to update exam config: {}", e);
            AppError::InternalServerError("ไม่สามารถอัปเดต config ได้".to_string())
        })?;
    Ok(())
}

pub async fn get_exam_config(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamConfigResponse, AppError> {
    let config = load_exam_config_storage(pool, round_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()))?;
    Ok(config.into())
}

pub struct AssignSeatsResult {
    pub assigned_count: usize,
    pub rooms: Vec<AssignSeatsRoomSummary>,
    pub message: String,
}

pub async fn assign_exam_seats(
    pool: &PgPool,
    round_id: Uuid,
    user_id: Uuid,
    exam_id_type_override: Option<String>,
    exam_id_prefix_override: Option<String>,
    sort_order_override: Option<String>,
    mode: Option<String>,
) -> Result<AssignSeatsResult, AppError> {
    let config = load_exam_config_storage(pool, round_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()))?;

    let exam_id_type = exam_id_type_override
        .or(config.exam_id_type)
        .unwrap_or_else(|| "application_number".to_string());
    let exam_id_prefix = exam_id_prefix_override
        .or(config.exam_id_prefix)
        .unwrap_or_default();
    let sort_order = sort_order_override
        .or(config.sort_order)
        .unwrap_or_else(|| "by_application".to_string());

    #[derive(sqlx::FromRow)]
    struct AppRow {
        id: Uuid,
        application_number: Option<String>,
        #[allow(dead_code)]
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
           WHERE aa.admission_round_id = $1 AND aa.status = 'verified' {}"#,
        order_clause
    );

    let applicants = sqlx::query_as::<_, AppRow>(&query)
        .bind(round_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch applicants: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้สมัครได้".to_string())
        })?;

    if applicants.is_empty() {
        return Err(AppError::BadRequest("ไม่มีผู้สมัครที่ eligible".to_string()));
    }

    #[derive(sqlx::FromRow)]
    struct RoomCapRow {
        id: Uuid,
        room_name: String,
        capacity: i32,
    }

    let rooms = sqlx::query_as::<_, RoomCapRow>(
        r#"SELECT er.id,
                  COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
                  COALESCE(er.capacity_override, r.capacity, 40)::INT AS capacity
           FROM admission_exam_rooms er
           LEFT JOIN rooms r ON r.id = er.room_id
           WHERE er.admission_round_id = $1
           ORDER BY er.display_order ASC, er.created_at ASC"#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch exam rooms: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลห้องสอบได้".to_string())
    })?;

    if rooms.is_empty() {
        return Err(AppError::BadRequest(
            "ยังไม่มีห้องสอบ กรุณาเพิ่มห้องสอบก่อน".to_string(),
        ));
    }

    let mode = mode.as_deref().unwrap_or("full");

    if mode == "append" {
        #[derive(sqlx::FromRow)]
        struct ExistingRow {
            application_id: Uuid,
            exam_room_id: Uuid,
        }

        let existing: Vec<ExistingRow> = sqlx::query_as(
            r#"SELECT application_id, exam_room_id
               FROM admission_exam_seat_assignments
               WHERE exam_room_id IN (SELECT id FROM admission_exam_rooms WHERE admission_round_id = $1)"#
        )
        .bind(round_id).fetch_all(pool).await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถดึงข้อมูลที่นั่งเดิมได้".to_string()))?;

        let existing_app_ids: std::collections::HashSet<Uuid> =
            existing.iter().map(|r| r.application_id).collect();
        let existing_total = existing.len() as i32;

        let mut existing_counts: std::collections::HashMap<Uuid, i32> =
            std::collections::HashMap::new();
        for r in &existing {
            *existing_counts.entry(r.exam_room_id).or_insert(0) += 1;
        }

        let new_applicants: Vec<_> = applicants
            .into_iter()
            .filter(|a| !existing_app_ids.contains(&a.id))
            .collect();

        if new_applicants.is_empty() {
            return Ok(AssignSeatsResult {
                assigned_count: 0,
                rooms: vec![],
                message: "ไม่มีผู้สมัครใหม่ที่ต้องจัดที่นั่ง".to_string(),
            });
        }

        let remaining_capacity: i32 = rooms
            .iter()
            .map(|r| r.capacity - existing_counts.get(&r.id).copied().unwrap_or(0))
            .sum();
        if remaining_capacity < new_applicants.len() as i32 {
            return Err(AppError::BadRequest(format!(
                "ที่นั่งว่างเหลือ ({}) น้อยกว่าจำนวนผู้สมัครใหม่ ({}) — กรุณาเพิ่มห้องสอบหรือจัดใหม่ทั้งหมด",
                remaining_capacity,
                new_applicants.len()
            )));
        }

        let pad_width = format!("{}", existing_total + new_applicants.len() as i32)
            .len()
            .max(4);
        let mut new_assignments: Vec<(Uuid, Uuid, i32, String)> = Vec::new();
        let mut room_iter = rooms.iter();
        let mut current_room = room_iter
            .next()
            .ok_or_else(|| AppError::InternalServerError("ไม่พบห้องสอบสำหรับจัดที่นั่ง".to_string()))?;
        while existing_counts.get(&current_room.id).copied().unwrap_or(0) >= current_room.capacity {
            current_room = room_iter
                .next()
                .ok_or_else(|| AppError::InternalServerError("ห้องสอบเต็มทั้งหมด".to_string()))?;
        }
        let mut seat_in_room = existing_counts.get(&current_room.id).copied().unwrap_or(0);
        let mut global_seq = existing_total;

        for app in &new_applicants {
            while seat_in_room >= current_room.capacity {
                current_room = room_iter
                    .next()
                    .ok_or_else(|| AppError::InternalServerError("ห้องสอบเต็มทั้งหมด".to_string()))?;
                seat_in_room = existing_counts.get(&current_room.id).copied().unwrap_or(0);
            }
            seat_in_room += 1;
            global_seq += 1;
            let exam_id = match exam_id_type.as_str() {
                "sequential" => format!("{:0>width$}", global_seq, width = pad_width),
                "custom_prefix" => format!(
                    "{}{:0>width$}",
                    exam_id_prefix,
                    global_seq,
                    width = pad_width
                ),
                _ => app
                    .application_number
                    .clone()
                    .unwrap_or_else(|| format!("{}", global_seq)),
            };
            new_assignments.push((app.id, current_room.id, seat_in_room, exam_id));
        }

        let mut tx = pool
            .begin()
            .await
            .map_err(|_| AppError::InternalServerError("Transaction error".to_string()))?;
        for (app_id, rid, seat, eid) in &new_assignments {
            sqlx::query(
                r#"INSERT INTO admission_exam_seat_assignments
                   (application_id, exam_room_id, seat_number, exam_id, assigned_by)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(app_id)
            .bind(rid)
            .bind(seat)
            .bind(eid)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("Failed to insert seat assignment (append): {}", e);
                AppError::InternalServerError("ไม่สามารถบันทึกที่นั่งสอบได้".to_string())
            })?;
        }
        tx.commit()
            .await
            .map_err(|_| AppError::InternalServerError("Transaction commit failed".to_string()))?;

        let mut room_summary: std::collections::HashMap<Uuid, (String, i32)> =
            std::collections::HashMap::new();
        for r in &rooms {
            room_summary.insert(r.id, (r.room_name.clone(), 0));
        }
        for (_, rid, _, _) in &new_assignments {
            if let Some(e) = room_summary.get_mut(rid) {
                e.1 += 1;
            }
        }
        let summary: Vec<AssignSeatsRoomSummary> = rooms
            .iter()
            .filter_map(|r| {
                room_summary
                    .get(&r.id)
                    .map(|(room_name, count)| AssignSeatsRoomSummary {
                        room_name: room_name.clone(),
                        count: *count,
                    })
            })
            .collect();

        return Ok(AssignSeatsResult {
            assigned_count: new_assignments.len(),
            rooms: summary,
            message: format!("เพิ่มที่นั่งสอบสำเร็จ {} คน", new_assignments.len()),
        });
    }

    // Full mode
    let total_capacity: i32 = rooms.iter().map(|r| r.capacity).sum();
    if total_capacity < applicants.len() as i32 {
        return Err(AppError::BadRequest(format!(
            "ความจุห้องสอบรวม ({}) น้อยกว่าจำนวนผู้สมัคร ({}) — ขาดอีก {} ที่นั่ง",
            total_capacity,
            applicants.len(),
            applicants.len() as i32 - total_capacity
        )));
    }

    let pad_width = format!("{}", applicants.len()).len().max(4);
    let mut assignments: Vec<(Uuid, Uuid, i32, String)> = Vec::new();
    let mut room_iter = rooms.iter();
    let mut current_room = room_iter
        .next()
        .ok_or_else(|| AppError::InternalServerError("ไม่พบห้องสอบสำหรับจัดที่นั่ง".to_string()))?;
    let mut seat_in_room = 0i32;
    let mut global_seq = 0i32;

    for app in &applicants {
        while seat_in_room >= current_room.capacity {
            current_room = room_iter
                .next()
                .ok_or_else(|| AppError::InternalServerError("ห้องสอบเต็ม".to_string()))?;
            seat_in_room = 0;
        }
        seat_in_room += 1;
        global_seq += 1;
        let exam_id = match exam_id_type.as_str() {
            "sequential" => format!("{:0>width$}", global_seq, width = pad_width),
            "custom_prefix" => format!(
                "{}{:0>width$}",
                exam_id_prefix,
                global_seq,
                width = pad_width
            ),
            _ => app
                .application_number
                .clone()
                .unwrap_or_else(|| format!("{}", global_seq)),
        };
        assignments.push((app.id, current_room.id, seat_in_room, exam_id));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction error".to_string()))?;

    sqlx::query(
        r#"DELETE FROM admission_exam_seat_assignments
           WHERE exam_room_id IN (SELECT id FROM admission_exam_rooms WHERE admission_round_id = $1)"#
    )
    .bind(round_id).execute(&mut *tx).await
    .map_err(|_| AppError::InternalServerError("ไม่สามารถล้างข้อมูลเดิมได้".to_string()))?;

    for (app_id, rid, seat, eid) in &assignments {
        sqlx::query(
            r#"INSERT INTO admission_exam_seat_assignments
               (application_id, exam_room_id, seat_number, exam_id, assigned_by)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(app_id)
        .bind(rid)
        .bind(seat)
        .bind(eid)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to insert seat assignment: {}", e);
            AppError::InternalServerError("ไม่สามารถบันทึกที่นั่งสอบได้".to_string())
        })?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction commit failed".to_string()))?;

    let mut room_summary: std::collections::HashMap<Uuid, (String, i32)> =
        std::collections::HashMap::new();
    for r in &rooms {
        room_summary.insert(r.id, (r.room_name.clone(), 0));
    }
    for (_, rid, _, _) in &assignments {
        if let Some(e) = room_summary.get_mut(rid) {
            e.1 += 1;
        }
    }
    let summary: Vec<AssignSeatsRoomSummary> = rooms
        .iter()
        .filter_map(|r| {
            room_summary
                .get(&r.id)
                .map(|(room_name, count)| AssignSeatsRoomSummary {
                    room_name: room_name.clone(),
                    count: *count,
                })
        })
        .collect();

    Ok(AssignSeatsResult {
        assigned_count: assignments.len(),
        rooms: summary,
        message: format!("จัดที่นั่งสอบสำเร็จ {} คน", assignments.len()),
    })
}

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeatRow {
    pub exam_room_id: Uuid,
    pub room_name: String,
    pub building_name: Option<String>,
    pub capacity: i32,
    pub seat_number: i32,
    pub exam_id: Option<String>,
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub full_name: String,
    pub national_id: String,
    pub track_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomGroup {
    pub exam_room_id: Uuid,
    pub room_name: String,
    pub building_name: Option<String>,
    pub capacity: i32,
    pub seats: Vec<SeatRow>,
}

fn pii_error(context: &str, error: String) -> AppError {
    eprintln!("Admission exam room PII {} failed: {}", context, error);
    AppError::InternalServerError("ไม่สามารถประมวลผลข้อมูลส่วนบุคคลได้".to_string())
}

pub async fn get_exam_seats(pool: &PgPool, round_id: Uuid) -> Result<Vec<RoomGroup>, AppError> {
    let mut rows = sqlx::query_as::<_, SeatRow>(
        r#"SELECT
            er.id AS exam_room_id,
            COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
            b.name_th AS building_name,
            COALESCE(er.capacity_override, r.capacity, 40)::INT AS capacity,
            sa.seat_number, sa.exam_id,
            aa.id AS application_id, aa.application_number,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            aa.national_id, at.name AS track_name
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
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch exam seats: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลที่นั่งสอบได้".to_string())
    })?;

    for row in &mut rows {
        row.national_id = pii::decrypt_required(&row.national_id)
            .map_err(|error| pii_error("decrypt national_id", error))?;
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
    Ok(groups)
}

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamSeatDetail {
    pub seat_number: i32,
    pub exam_id: Option<String>,
    pub room_name: String,
    pub building_name: Option<String>,
    pub exam_date: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_application_exam_seat(
    pool: &PgPool,
    application_id: Uuid,
) -> Result<Option<ExamSeatDetail>, AppError> {
    sqlx::query_as::<_, ExamSeatDetail>(
        r#"SELECT sa.seat_number, sa.exam_id,
                  COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
                  b.name_th AS building_name, ar.exam_date
           FROM admission_exam_seat_assignments sa
           JOIN admission_exam_rooms er ON er.id = sa.exam_room_id
           JOIN admission_rounds ar ON ar.id = er.admission_round_id
           LEFT JOIN rooms r ON r.id = er.room_id
           LEFT JOIN buildings b ON b.id = r.building_id
           WHERE sa.application_id = $1"#,
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))
}
