use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::check_user_permission;
use crate::utils::{
    database::get_school_database_url, encryption as bcrypt, field_encryption,
    subdomain::extract_subdomain_from_request,
};
use crate::AppState;

use super::models::CreateParentRequest;

// -----------------------------------------------------------------------------
// Parent Management Handlers (New)
// -----------------------------------------------------------------------------

/// POST /api/students/:id/parents - เพิ่มผู้ปกครองให้นักเรียนที่มีอยู่
pub async fn add_parent_to_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<CreateParentRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check permission
    check_user_permission(&headers, &state.admin_pool, "student.update").await?;

    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to begin transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่มต้น transaction".to_string())
    })?;

    // 1. Check/Create Parent User
    // Check if parent exists by phone (username)
    let existing_parent = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE username = $1")
        .bind(&payload.phone)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to check for existing parent: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการตรวจสอบผู้ปกครอง".to_string())
        })?;

    let parent_id = if let Some(pid) = existing_parent {
        pid
    } else {
        // Create new parent user
        let parent_password_hash =
            bcrypt::hash(&payload.phone, bcrypt::DEFAULT_COST).map_err(|e| {
                eprintln!("❌ Parent password hashing failed: {}", e);
                AppError::InternalServerError(
                    "เกิดข้อผิดพลาดในการสร้างรหัสผ่านผู้ปกครอง".to_string(),
                )
            })?;

        // Encrypt parent national id if provided
        let (parent_enc_nid, parent_nid_hash) = if let Some(nid) = &payload.national_id {
            if !nid.is_empty() {
                let enc = field_encryption::encrypt(nid).map_err(|e| {
                    eprintln!("❌ Parent Encryption failed: {}", e);
                    AppError::InternalServerError(
                        "เกิดข้อผิดพลาดในการประมวลผลข้อมูลผู้ปกครอง".to_string(),
                    )
                })?;
                let hash = field_encryption::hash_for_search(nid);
                (Some(enc), Some(hash))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let new_parent_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (
                username, national_id, national_id_hash, email, password_hash,
                first_name, last_name, phone,
                user_type, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'parent', 'active')
            RETURNING id
            "#,
        )
        .bind(&payload.phone) // Username as phone
        .bind(parent_enc_nid)
        .bind(parent_nid_hash)
        .bind(&payload.email)
        .bind(parent_password_hash)
        .bind(&payload.first_name)
        .bind(&payload.last_name)
        .bind(&payload.phone)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to create parent: {}", e);
            AppError::InternalServerError("ไม่สามารถสร้างบัญชีผู้ปกครองได้".to_string())
        })?;

        // Assign PARENT role
        let parent_role_id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM roles WHERE code = 'PARENT' AND is_active = true")
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();

        if let Some(rid) = parent_role_id {
            let _ = sqlx::query(
                r#"
                INSERT INTO user_roles (user_id, role_id, is_primary)
                VALUES ($1, $2, true)
                "#,
            )
            .bind(new_parent_id)
            .bind(rid)
            .execute(&mut *tx)
            .await;
        }

        new_parent_id
    };

    // 2. Link Parent to Student
    sqlx::query(
        r#"
        INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
        VALUES ($1, $2, $3, false)
        ON CONFLICT (student_user_id, parent_user_id) 
        DO UPDATE SET relationship = EXCLUDED.relationship
        "#,
    )
    .bind(student_id)
    .bind(parent_id)
    .bind(&payload.relationship)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to link parent: {}", e);
        AppError::InternalServerError("ไม่สามารถเชื่อมโยงผู้ปกครองได้".to_string())
    })?;

    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "เพิ่มผู้ปกครองสำเร็จ"
        })),
    ))
}

/// DELETE /api/students/:id/parents/:parentId - ลบความสัมพันธ์ผู้ปกครอง
pub async fn remove_parent_from_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((student_id, parent_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    // Check permission
    check_user_permission(&headers, &state.admin_pool, "student.update").await?;

    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    // Delete from student_parents table
    let result =
        sqlx::query("DELETE FROM student_parents WHERE student_user_id = $1 AND parent_user_id = $2")
            .bind(student_id)
            .bind(parent_id)
            .execute(&pool)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to remove parent link: {}", e);
                AppError::InternalServerError("ไม่สามารถลบผู้ปกครองได้".to_string())
            })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "ไม่พบข้อมูลความสัมพันธ์ผู้ปกครอง".to_string(),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "ลบผู้ปกครองสำเร็จ"
        })),
    ))
}
