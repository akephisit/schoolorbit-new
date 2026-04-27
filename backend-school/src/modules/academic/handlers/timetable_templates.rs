// Phase F: Timetable Templates
//
// Workflow:
// 1. user batch fixed slots (พัก/โฮมรูม/sync activity) → ตารางมี entries
// 2. POST /templates/from-current — snapshot ตารางปัจจุบัน → save เป็น template
// 3. user กดจัดอัตโนมัติ → ผลไม่ถูกใจ
// 4. DELETE /timetable/clear — เคลียร์ entries (ระบุประเภทได้)
// 5. POST /templates/{id}/apply — hydrate template เข้า semester ผ่าน batch logic
// 6. user จัดอัตโนมัติใหม่

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::AppState;
use crate::modules::academic::websockets::TimetableEvent;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ============================================
// Models
// ============================================

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TimetableTemplateView {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub entry_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TimetableTemplateEntry {
    pub id: Uuid,
    pub template_id: Uuid,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub entry_type: String,
    pub title: Option<String>,
    pub activity_slot_id: Option<Uuid>,
    pub grade_level_ids: serde_json::Value,
    pub classroom_ids: serde_json::Value,
    pub instructor_ids: serde_json::Value,
    pub room_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FromCurrentRequest {
    pub semester_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    /// Filter ประเภท entry ที่จะ snapshot (default: ทุกอย่างที่ไม่ใช่ COURSE)
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyTemplateRequest {
    pub semester_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ClearTimetableRequest {
    pub semester_id: Uuid,
    /// ประเภท entry ที่จะลบ (default: ลบหมด ยกเว้น COURSE)
    /// ระบุ ["COURSE"] เพื่อลบเฉพาะวิชา (เก็บกิจกรรม/พัก/โฮมรูม)
    pub entry_types: Option<Vec<String>>,
}

// ============================================
// Endpoints
// ============================================

/// GET /api/academic/timetable-templates
pub async fn list_templates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let rows = sqlx::query_as::<_, TimetableTemplateView>(
        r#"SELECT t.id, t.name, t.description, t.created_by, t.created_at, t.updated_at,
                  COUNT(e.id) AS entry_count
           FROM timetable_templates t
           LEFT JOIN timetable_template_entries e ON e.template_id = t.id
           GROUP BY t.id
           ORDER BY t.created_at DESC"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "data": rows })).into_response())
}

/// GET /api/academic/timetable-templates/{id}
pub async fn get_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let template = sqlx::query_as::<_, TimetableTemplateView>(
        r#"SELECT t.id, t.name, t.description, t.created_by, t.created_at, t.updated_at,
                  (SELECT COUNT(*) FROM timetable_template_entries WHERE template_id = t.id) AS entry_count
           FROM timetable_templates t WHERE t.id = $1"#
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Template not found".to_string()))?;

    let entries = sqlx::query_as::<_, TimetableTemplateEntry>(
        r#"SELECT * FROM timetable_template_entries WHERE template_id = $1
           ORDER BY day_of_week, period_id"#
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "template": template,
            "entries": entries,
        }
    })).into_response())
}

/// POST /api/academic/timetable-templates
pub async fn create_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO timetable_templates (name, description, created_by)
         VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "data": { "id": row.0 }})).into_response())
}

/// PUT /api/academic/timetable-templates/{id}
/// แก้ไขชื่อ/คำอธิบาย — ไม่แก้ entries ใน template (ใช้ from_current เพื่อ snapshot ใหม่)
#[derive(Debug, Deserialize)]
pub struct UpdateTemplateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

pub async fn update_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query(
        "UPDATE timetable_templates SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            updated_at = NOW()
         WHERE id = $1"
    )
    .bind(id)
    .bind(&payload.name)
    .bind(&payload.description)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })).into_response())
}

/// DELETE /api/academic/timetable-templates/{id}
pub async fn delete_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query("DELETE FROM timetable_templates WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })).into_response())
}

/// POST /api/academic/timetable-templates/from-current
/// Snapshot ตาราง semester ปัจจุบัน → template ใหม่
pub async fn from_current(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FromCurrentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Default: ทุกอย่างที่ไม่ใช่ COURSE
    let entry_types = payload.entry_types.unwrap_or_else(|| vec![
        "BREAK".to_string(),
        "HOMEROOM".to_string(),
        "ACTIVITY".to_string(),
        "ACADEMIC".to_string(),
    ]);

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // 1. Create template
    let template_id: Uuid = sqlx::query_scalar(
        "INSERT INTO timetable_templates (name, description, created_by)
         VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // 2. Snapshot entries — เฉพาะ classroom entries (classroom_id IS NOT NULL)
    //    เพื่อกัน leak tei จาก instructor-only entries (เช่น TEXT batch ที่ user
    //    เลือกครู — backend สร้าง 2 ชุด: classroom entries (no tei) + instructor-only
    //    entries (with tei) ที่มี (day,period,type,title) เหมือนกัน → ถ้านับ
    //    instructor-only เข้ามาด้วย apply จะใส่ครูทับ classroom entries → ผิดเจตนาเดิม)
    //
    //    Group by (day, period, entry_type, title, activity_slot_id, room_id)
    //    instructor_ids = union ของ tei เฉพาะที่ผูกกับ classroom entries
    sqlx::query(
        r#"
        WITH grouped AS (
            SELECT
                te.day_of_week,
                te.period_id,
                te.entry_type,
                te.title,
                te.activity_slot_id,
                te.room_id,
                ARRAY_AGG(DISTINCT te.classroom_id)
                    AS classroom_ids,
                ARRAY_AGG(DISTINCT tei.instructor_id) FILTER (WHERE tei.instructor_id IS NOT NULL)
                    AS instructor_ids
            FROM academic_timetable_entries te
            LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
            WHERE te.academic_semester_id = $1
              AND te.is_active = true
              AND te.entry_type = ANY($2)
              AND te.classroom_id IS NOT NULL
            GROUP BY te.day_of_week, te.period_id, te.entry_type, te.title,
                     te.activity_slot_id, te.room_id
        )
        INSERT INTO timetable_template_entries
            (template_id, day_of_week, period_id, entry_type, title,
             activity_slot_id, classroom_ids, instructor_ids, room_id)
        SELECT $3,
               g.day_of_week, g.period_id, g.entry_type, g.title,
               g.activity_slot_id,
               COALESCE(to_jsonb(g.classroom_ids), '[]'::jsonb),
               COALESCE(to_jsonb(g.instructor_ids), '[]'::jsonb),
               g.room_id
        FROM grouped g
        "#
    )
    .bind(payload.semester_id)
    .bind(&entry_types)
    .bind(template_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": { "id": template_id }
    })).into_response())
}

/// POST /api/academic/timetable-templates/{id}/apply
/// Hydrate template → INSERT เข้า academic_timetable_entries (ใช้ batch_id เดียวต่อ entry)
pub async fn apply_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<ApplyTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Load template entries
    let entries = sqlx::query_as::<_, TimetableTemplateEntry>(
        "SELECT * FROM timetable_template_entries WHERE template_id = $1"
    )
    .bind(template_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if entries.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": true,
            "data": { "applied": 0, "message": "Template has no entries" }
        })).into_response());
    }

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let mut total_inserted: u64 = 0;

    // Group template entries by (title, activity_slot_id) — กิจกรรมเดียวกันใช้ batch_id ร่วม
    // → "ลบทั้งหมด" จะลบเฉพาะ entries ของกิจกรรมนั้น (ไม่กระทบกิจกรรมอื่นใน template)
    use std::collections::HashMap;
    let mut group_batch_ids: HashMap<(Option<String>, Option<Uuid>), Uuid> = HashMap::new();

    // Resolve grade_level_ids → classroom_ids ใน semester นี้
    for entry in &entries {
        // Build classroom_ids: union ของ specific + resolved from grade_level
        let specific_classrooms: Vec<Uuid> = serde_json::from_value(entry.classroom_ids.clone())
            .unwrap_or_default();
        let grade_level_ids: Vec<Uuid> = serde_json::from_value(entry.grade_level_ids.clone())
            .unwrap_or_default();
        let instructor_ids: Vec<Uuid> = serde_json::from_value(entry.instructor_ids.clone())
            .unwrap_or_default();

        // Resolve grade_level_ids → classroom_ids
        let mut resolved_classrooms: Vec<Uuid> = if !grade_level_ids.is_empty() {
            sqlx::query_scalar(
                r#"SELECT cr.id FROM class_rooms cr
                   JOIN academic_semesters s ON s.academic_year_id = cr.academic_year_id
                   WHERE s.id = $1 AND cr.grade_level_id = ANY($2)"#
            )
            .bind(payload.semester_id)
            .bind(&grade_level_ids)
            .fetch_all(&mut *tx)
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        // union with specific
        for c in specific_classrooms {
            if !resolved_classrooms.contains(&c) {
                resolved_classrooms.push(c);
            }
        }

        if resolved_classrooms.is_empty() {
            continue; // skip — no targets
        }

        // batch_uuid ต่อ "กิจกรรม" — ใช้ key (title, activity_slot_id)
        // - TEXT batch (ไม่มี slot): group ตาม title
        // - SLOT-sync (มี slot): group ตาม slot_id
        let group_key = (entry.title.clone(), entry.activity_slot_id);
        let batch_uuid = *group_batch_ids
            .entry(group_key)
            .or_insert_with(Uuid::new_v4);

        // Bulk insert entries — 1 query per template entry × N classrooms
        // ON CONFLICT DO NOTHING (กัน clash กับ entries เดิม)
        let inserted_count: u64 = sqlx::query(
            r#"INSERT INTO academic_timetable_entries
                   (id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                    entry_type, title, is_active, created_by, updated_by,
                    classroom_course_id, note, activity_slot_id, batch_id)
               SELECT gen_random_uuid(), c, $1, $2, $3, $4,
                      $5, $6, true, $7, $7,
                      NULL, NULL, $8, $9
               FROM UNNEST($10::uuid[]) AS c
               ON CONFLICT DO NOTHING"#
        )
        .bind(payload.semester_id)
        .bind(&entry.day_of_week)
        .bind(entry.period_id)
        .bind(entry.room_id)
        .bind(&entry.entry_type)
        .bind(&entry.title)
        .bind(user_id)
        .bind(entry.activity_slot_id)
        .bind(batch_uuid)
        .bind(&resolved_classrooms)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .rows_affected();

        total_inserted += inserted_count;

        // Insert tei (ครู) — ผูกกับ entries ที่ "เพิ่ง insert" จาก template entry นี้
        // ใช้ batch_id + day_of_week + period_id เพื่อ scope ให้แน่ — กัน leak ไป entries
        // อื่นใน batch เดียวกัน (เช่น template entry อื่นใน batch group เดียวกันแต่คาบ
        // ต่างกัน อาจมี instructor_ids ต่างกัน)
        if !instructor_ids.is_empty() {
            sqlx::query(
                r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                   SELECT te.id, instr.v, 'primary'
                   FROM academic_timetable_entries te
                   CROSS JOIN UNNEST($1::uuid[]) AS instr(v)
                   WHERE te.batch_id = $2
                     AND te.day_of_week = $3
                     AND te.period_id = $4
                   ON CONFLICT DO NOTHING"#
            )
            .bind(&instructor_ids)
            .bind(batch_uuid)
            .bind(&entry.day_of_week)
            .bind(entry.period_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        }
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast TableRefresh — affected entries เยอะ
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.semester_id,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "data": { "applied": total_inserted }
    })).into_response())
}

/// DELETE /api/academic/timetable/clear
/// Clear entries ใน semester — ระบุประเภทได้
pub async fn clear_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ClearTimetableRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Default: ลบทุกอย่าง ยกเว้น COURSE (เก็บไว้)
    // ระบุ ["COURSE"] → ลบเฉพาะวิชา
    // ระบุ [] หรือ ทุกประเภท → ลบทั้งหมด
    let entry_types = payload.entry_types.unwrap_or_else(|| vec![
        "BREAK".to_string(),
        "HOMEROOM".to_string(),
        "ACTIVITY".to_string(),
        "ACADEMIC".to_string(),
    ]);

    let result = sqlx::query(
        "DELETE FROM academic_timetable_entries
         WHERE academic_semester_id = $1 AND entry_type = ANY($2)"
    )
    .bind(payload.semester_id)
    .bind(&entry_types)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast TableRefresh
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.semester_id,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "data": { "deleted": result.rows_affected() }
    })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UnusedQuery;
// Placeholder to keep imports happy if needed
#[allow(dead_code)]
fn _unused(_q: Query<UnusedQuery>) {}
