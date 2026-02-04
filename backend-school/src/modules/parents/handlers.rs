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

/// GET /api/parent/profile - ผู้ปกครองดูข้อมูลตนเองและบุตรหลาน
pub async fn get_own_parent_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
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
            si.student_id, si.grade_level, si.class_room,
            sp.relationship
        FROM student_parents sp
        INNER JOIN users u ON sp.student_user_id = u.id
        LEFT JOIN student_info si ON u.id = si.user_id
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

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
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
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender, u.address, u.profile_image_url,
            si.student_id, si.grade_level, si.class_room, si.student_number,
            si.blood_type, si.allergies, si.medical_conditions
        FROM users u
        INNER JOIN student_info si ON u.id = si.user_id
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
