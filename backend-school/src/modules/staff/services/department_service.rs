use crate::error::AppError;
use crate::modules::staff::models::{CreateDepartmentRequest, Department, UpdateDepartmentRequest};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_departments(pool: &PgPool) -> Result<Vec<Department>, AppError> {
    sqlx::query_as::<_, Department>(
        "SELECT * FROM departments WHERE is_active = true ORDER BY display_order, name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })
}

pub async fn get_department(pool: &PgPool, dept_id: Uuid) -> Result<Department, AppError> {
    sqlx::query_as::<_, Department>("SELECT * FROM departments WHERE id = $1")
        .bind(dept_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบฝ่าย".to_string()))
}

pub async fn create_department(
    pool: &PgPool,
    payload: CreateDepartmentRequest,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        "INSERT INTO departments (code, name, name_en, description, parent_department_id, phone, email, location, category, org_type)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id",
    )
    .bind(&payload.code)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(payload.parent_department_id)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.location)
    .bind(payload.category.unwrap_or_else(|| "administrative".to_string()))
    .bind(payload.org_type.unwrap_or_else(|| "unit".to_string()))
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to create department: {}", e);
        let err_msg = e.to_string();
        if err_msg.contains("duplicate key value") && err_msg.contains("code") {
            AppError::BadRequest("รหัสฝ่ายนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::InternalServerError("ไม่สามารถสร้างฝ่ายได้".to_string())
        }
    })
}

pub async fn update_department(
    pool: &PgPool,
    dept_id: Uuid,
    payload: UpdateDepartmentRequest,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE departments
         SET
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            parent_department_id = COALESCE($5, parent_department_id),
            phone = COALESCE($6, phone),
            email = COALESCE($7, email),
            location = COALESCE($8, location),
            category = COALESCE($9, category),
            org_type = COALESCE($10, org_type),
            is_active = COALESCE($11, is_active),
            updated_at = NOW()
         WHERE id = $1",
    )
    .bind(dept_id)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(payload.parent_department_id)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.location)
    .bind(&payload.category)
    .bind(&payload.org_type)
    .bind(payload.is_active)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        let err_msg = e.to_string();
        if err_msg.contains("duplicate key value") && err_msg.contains("code") {
            AppError::BadRequest("รหัสฝ่ายนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตฝ่าย".to_string())
        }
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบฝ่าย".to_string()));
    }

    Ok(())
}
