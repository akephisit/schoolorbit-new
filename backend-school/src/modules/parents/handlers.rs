use axum::{
    extract::{State, Query, Path},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;
use crate::error::AppError;
use crate::middleware::auth::get_current_user;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::{
    subdomain::extract_subdomain_from_request,
    field_encryption,
};
use crate::AppState;
use super::models::{ParentProfile, ParentDbRow, ChildDto};
use crate::modules::students::models::{StudentProfile, StudentDbRow, ParentDto};
use crate::modules::academic::models::timetable::TimetableEntry;

/// GET /api/parent/profile - ผู้ปกครองดูข้อมูลตนเองและบุตรหลาน
pub async fn get_own_parent_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Get current user
    let user = get_current_user(&headers, &pool).await?;
    
    // Query parent profile
    let mut parent = sqlx::query_as::<_, ParentDbRow>(
        r#"
        SELECT 
            id, username, first_name, last_name, title, phone, email, national_id
        FROM users
        WHERE id = $1 AND status = 'active'
        "#
    )
    .bind(user.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?
    .ok_or(AppError::NotFound("Parent not found".to_string()))?;

    // Decrypt national_id
    if let Some(nid) = &parent.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            parent.national_id = Some(dec);
        }
    }

    // Fetch children
    let children = sqlx::query_as::<_, ChildDto>(
        r#"
        SELECT 
            u.id, u.first_name, u.last_name, u.profile_image_url,
            si.student_id, 
            CASE gl.level_type 
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level, 
            c.name as class_room,
            sp.relationship
        FROM student_parents sp
        INNER JOIN users u ON sp.student_user_id = u.id
        LEFT JOIN student_info si ON u.id = si.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE sp.parent_user_id = $1 AND u.status = 'active'
        ORDER BY u.first_name ASC
        "#
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| Vec::new());

    let profile = ParentProfile {
        id: parent.id,
        username: parent.username,
        first_name: parent.first_name,
        last_name: parent.last_name,
        title: parent.title,
        phone: parent.phone,
        email: parent.email,
        national_id: parent.national_id,
        children,
    };

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": profile
        })),
    ))
}

/// GET /api/parent/students/:student_id - ผู้ปกครองดูรายละเอียดบุตรหลาน
pub async fn get_child_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    // Get current user (Parent)
    let user = get_current_user(&headers, &pool).await?;

    // Check if Parent is linked to this student
    let is_linked = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM student_parents 
            WHERE parent_user_id = $1 AND student_user_id = $2
        )
        "#
    )
    .bind(user.id)
    .bind(student_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(false);

    if !is_linked {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้".to_string()));
    }

    // Reuse logic from get_student to fetch full profile
    // Query student profile
    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT 
            u.id, u.username, u.national_id, u.email, u.first_name, u.last_name, 
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender, u.address, u.profile_image_url, u.status,
            si.student_id, 
            CASE gl.level_type 
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level, 
            c.name as class_room,
            sce.class_number as student_number,
            si.blood_type, si.allergies, si.medical_conditions
        FROM users u
        INNER JOIN student_info si ON u.id = si.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.id = $1 AND u.status = 'active'
        "#
    )
    .bind(student_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูลนักเรียน".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    // Decrypt fields
    if let Some(nid) = &student_row.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            student_row.national_id = Some(dec);
        }
    }

    // Note: Assuming other sensitive fields are handled.

    // Fetch parents for this student
    let parents = sqlx::query_as::<_, ParentDto>(
        r#"
        SELECT 
            u.id, u.username, u.first_name, u.last_name, u.phone,
            sp.relationship, sp.is_primary
        FROM student_parents sp
        INNER JOIN users u ON sp.parent_user_id = u.id
        WHERE sp.student_user_id = $1
        "#
    )
    .bind(student_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| Vec::new());
    
    let student = StudentProfile {
        info: student_row,
        parents,
    };

    Ok((StatusCode::OK, Json(json!({"success": true, "data": student}))))
}

#[derive(Debug, serde::Deserialize)]
pub struct ChildTimetableQuery {
    pub academic_semester_id: Option<Uuid>,
}

/// GET /api/parent/students/:student_id/timetable
/// ผู้ปกครองดูตารางเรียนของบุตรหลาน — verify ownership ผ่าน student_parents
pub async fn get_child_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Query(query): Query<ChildTimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;

    let user = get_current_user(&headers, &pool).await?;

    // Verify parent owns this student
    let is_linked = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM student_parents
            WHERE parent_user_id = $1 AND student_user_id = $2
        )
        "#
    )
    .bind(user.id)
    .bind(student_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(false);

    if !is_linked {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้".to_string()));
    }

    // Query timetable scoped to the student's active classroom enrollments
    // (เลียน list_timetable_entries แต่ filter เฉพาะนักเรียนคนนี้)
    let mut sql = String::from(
        r#"
        SELECT
            te.*,
            s.code   AS subject_code,
            s.name_th AS subject_name_th,
            (SELECT ARRAY_AGG(concat(u2.first_name, ' ', u2.last_name) ORDER BY tei2.role, tei2.created_at)
             FROM timetable_entry_instructors tei2
             JOIN users u2 ON u2.id = tei2.instructor_id
             WHERE tei2.entry_id = te.id) AS instructor_names,
            (SELECT ARRAY_AGG(tei_id.instructor_id ORDER BY tei_id.role, tei_id.created_at)
             FROM timetable_entry_instructors tei_id
             WHERE tei_id.entry_id = te.id) AS instructor_ids,
            (SELECT concat(u3.first_name, ' ', u3.last_name)
             FROM timetable_entry_instructors tei3
             JOIN users u3 ON u3.id = tei3.instructor_id
             WHERE tei3.entry_id = te.id
             ORDER BY tei3.role, tei3.created_at
             LIMIT 1) AS instructor_name,
            cr.name  AS classroom_name,
            r.code   AS room_code,
            ap.name  AS period_name,
            ap.start_time,
            ap.end_time,
            asl_ac.name AS activity_slot_name,
            asl_ac.activity_type AS activity_type,
            asl_ac.scheduling_mode AS activity_scheduling_mode
        FROM academic_timetable_entries te
        LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
        LEFT JOIN subjects s ON cc.subject_id = s.id
        LEFT JOIN class_rooms cr ON te.classroom_id = cr.id
        JOIN academic_periods ap ON te.period_id = ap.id
        LEFT JOIN rooms r ON te.room_id = r.id
        LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
        LEFT JOIN activity_catalog asl_ac ON asl.activity_catalog_id = asl_ac.id
        WHERE te.is_active = true
          AND te.classroom_id IN (
              SELECT class_room_id FROM student_class_enrollments
              WHERE student_id = $1 AND status = 'active'
          )
        "#
    );

    if query.academic_semester_id.is_some() {
        sql.push_str(" AND te.academic_semester_id = $2");
    }

    sql.push_str(" ORDER BY te.day_of_week, ap.order_index");

    let mut q = sqlx::query_as::<_, TimetableEntry>(&sql).bind(student_id);
    if let Some(sem_id) = query.academic_semester_id {
        q = q.bind(sem_id);
    }

    let entries = q.fetch_all(&pool).await.map_err(|e| {
        eprintln!("Failed to fetch child timetable: {}", e);
        AppError::InternalServerError("Failed to fetch timetable".to_string())
    })?;

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": entries }))))
}
