use crate::error::AppError;
use crate::modules::staff::models::{
    CreateOrganizationUnitRequest, OrganizationUnit, UpdateOrganizationUnitRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_organization_units(pool: &PgPool) -> Result<Vec<OrganizationUnit>, AppError> {
    sqlx::query_as::<_, OrganizationUnit>(
        "SELECT * FROM organization_units WHERE is_active = true ORDER BY display_order, name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })
}

pub async fn get_organization_unit(
    pool: &PgPool,
    unit_id: Uuid,
) -> Result<OrganizationUnit, AppError> {
    sqlx::query_as::<_, OrganizationUnit>("SELECT * FROM organization_units WHERE id = $1")
        .bind(unit_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบหน่วยงาน".to_string()))
}

pub async fn create_organization_unit(
    pool: &PgPool,
    payload: CreateOrganizationUnitRequest,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        "INSERT INTO organization_units (
            code, name, name_en, description, parent_unit_id,
            phone, email, location, category, unit_type, subject_group_id
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id",
    )
    .bind(&payload.code)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(payload.parent_unit_id)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.location)
    .bind(organization_category_or_default(payload.category))
    .bind(organization_unit_type_or_default(payload.unit_type))
    .bind(payload.subject_group_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("❌ Failed to create organization unit: {}", e);
        let err_msg = e.to_string();
        if err_msg.contains("duplicate key value") && err_msg.contains("code") {
            AppError::BadRequest("รหัสหน่วยงานนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::InternalServerError("ไม่สามารถสร้างหน่วยงานได้".to_string())
        }
    })
}

pub async fn update_organization_unit(
    pool: &PgPool,
    unit_id: Uuid,
    payload: UpdateOrganizationUnitRequest,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE organization_units
         SET
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            parent_unit_id = COALESCE($5, parent_unit_id),
            phone = COALESCE($6, phone),
            email = COALESCE($7, email),
            location = COALESCE($8, location),
            category = COALESCE($9, category),
            unit_type = COALESCE($10, unit_type),
            is_active = COALESCE($11, is_active),
            subject_group_id = COALESCE($12, subject_group_id),
            updated_at = NOW()
         WHERE id = $1",
    )
    .bind(unit_id)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(payload.parent_unit_id)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.location)
    .bind(&payload.category)
    .bind(&payload.unit_type)
    .bind(payload.is_active)
    .bind(payload.subject_group_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("❌ Database error: {}", e);
        let err_msg = e.to_string();
        if err_msg.contains("duplicate key value") && err_msg.contains("code") {
            AppError::BadRequest("รหัสหน่วยงานนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตหน่วยงาน".to_string())
        }
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบหน่วยงาน".to_string()));
    }

    Ok(())
}

fn organization_category_or_default(category: Option<String>) -> String {
    category.unwrap_or_else(|| "general".to_string())
}

fn organization_unit_type_or_default(unit_type: Option<String>) -> String {
    unit_type.unwrap_or_else(|| "division".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_category_or_default_uses_general_when_missing() {
        assert_eq!(organization_category_or_default(None), "general");
        assert_eq!(
            organization_category_or_default(Some("academic".to_string())),
            "academic"
        );
    }

    #[test]
    fn organization_unit_type_or_default_uses_division_when_missing() {
        assert_eq!(organization_unit_type_or_default(None), "division");
        assert_eq!(
            organization_unit_type_or_default(Some("management_group".to_string())),
            "management_group"
        );
    }
}
