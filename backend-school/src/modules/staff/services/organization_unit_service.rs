use crate::error::AppError;
use crate::modules::staff::models::{
    CreateOrganizationUnitRequest, OrganizationUnit, UpdateOrganizationUnitRequest,
};
use crate::utils::audit::AuditLogBuilder;
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::StatusTransitionOutcome;

pub async fn list_organization_units(
    pool: &PgPool,
    include_inactive: bool,
) -> Result<Vec<OrganizationUnit>, AppError> {
    let active_filter = if include_inactive {
        ""
    } else {
        "WHERE is_active = true"
    };
    let sql = format!(
        "SELECT * FROM organization_units {} ORDER BY display_order, name",
        active_filter
    );
    sqlx::query_as::<_, OrganizationUnit>(&sql)
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
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(%error, "failed to start organization unit create transaction");
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;

    if let Some(parent_unit_id) = payload.parent_unit_id {
        require_active_parent(&mut tx, parent_unit_id).await?;
    }

    let unit_id = sqlx::query_scalar(
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
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("❌ Failed to create organization unit: {}", e);
        let err_msg = e.to_string();
        if err_msg.contains("duplicate key value") && err_msg.contains("code") {
            AppError::BadRequest("รหัสหน่วยงานนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::InternalServerError("ไม่สามารถสร้างหน่วยงานได้".to_string())
        }
    })?;

    tx.commit().await.map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to commit organization unit create transaction");
        AppError::InternalServerError("ไม่สามารถบันทึกหน่วยงานได้".to_string())
    })?;

    Ok(unit_id)
}

pub async fn update_organization_unit(
    pool: &PgPool,
    unit_id: Uuid,
    payload: UpdateOrganizationUnitRequest,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError> {
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to start organization unit update transaction");
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;
    let unit = lock_organization_unit(&mut tx, unit_id).await?;
    let effective_parent_unit_id = payload.parent_unit_id.or(unit.parent_unit_id);
    let resulting_is_active = payload.is_active.unwrap_or(unit.is_active);

    if resulting_is_active {
        if let Some(parent_unit_id) = effective_parent_unit_id {
            require_active_parent(&mut tx, parent_unit_id).await?;
        }
    }

    let status_outcome = if let Some(is_active) = payload.is_active {
        transition_locked_organization_unit_status(
            &mut tx,
            unit_id,
            unit,
            effective_parent_unit_id,
            is_active,
            actor_user_id,
        )
        .await?
    } else {
        StatusTransitionOutcome::Unchanged
    };

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
            subject_group_id = COALESCE($11, subject_group_id),
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
    .bind(payload.subject_group_id)
    .execute(&mut *tx)
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

    tx.commit().await.map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to commit organization unit update transaction");
        AppError::InternalServerError("ไม่สามารถบันทึกหน่วยงานได้".to_string())
    })?;

    Ok(status_outcome)
}

#[derive(Debug, FromRow)]
struct OrganizationUnitStatusRow {
    name: String,
    parent_unit_id: Option<Uuid>,
    is_active: bool,
    is_system: bool,
}

async fn lock_organization_unit(
    tx: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> Result<OrganizationUnitStatusRow, AppError> {
    sqlx::query_as::<_, OrganizationUnitStatusRow>(
        "SELECT name, parent_unit_id, is_active, is_system
         FROM organization_units
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(unit_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to lock organization unit");
        AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะหน่วยงานได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบหน่วยงาน".to_string()))
}

async fn require_active_parent(
    tx: &mut Transaction<'_, Postgres>,
    parent_unit_id: Uuid,
) -> Result<(), AppError> {
    let parent_is_active = sqlx::query_scalar::<_, bool>(
        "SELECT is_active
         FROM organization_units
         WHERE id = $1
         FOR SHARE",
    )
    .bind(parent_unit_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!(%parent_unit_id, %error, "failed to validate organization unit parent");
        AppError::InternalServerError("ไม่สามารถตรวจสอบหน่วยงานต้นสังกัดได้".to_string())
    })?;

    match parent_is_active {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Conflict("หน่วยงานต้นสังกัดถูกปิดใช้งานอยู่".to_string())),
        None => Err(AppError::BadRequest("ไม่พบหน่วยงานต้นสังกัดที่ระบุ".to_string())),
    }
}

pub(super) async fn ensure_organization_unit_active(
    tx: &mut Transaction<'_, Postgres>,
    organization_unit_id: Uuid,
) -> Result<(), AppError> {
    let is_active = sqlx::query_scalar::<_, bool>(
        "SELECT is_active
         FROM organization_units
         WHERE id = $1
         FOR SHARE",
    )
    .bind(organization_unit_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!(%organization_unit_id, %error, "failed to validate organization unit status");
        AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะหน่วยงานได้".to_string())
    })?;

    match is_active {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Conflict("หน่วยงานนี้ถูกปิดใช้งานอยู่".to_string())),
        None => Err(AppError::BadRequest("ไม่พบหน่วยงานที่ระบุ".to_string())),
    }
}

async fn transition_locked_organization_unit_status(
    tx: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
    unit: OrganizationUnitStatusRow,
    effective_parent_unit_id: Option<Uuid>,
    is_active: bool,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError> {
    if !is_active && unit.is_system {
        return Err(AppError::Conflict(
            "ไม่สามารถปิดใช้งานหน่วยงานระบบได้".to_string(),
        ));
    }

    if is_active {
        if let Some(parent_unit_id) = effective_parent_unit_id {
            require_active_parent(tx, parent_unit_id).await?;
        }
    }

    if unit.is_active == is_active {
        return Ok(StatusTransitionOutcome::Unchanged);
    }

    if !is_active {
        let has_active_child: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1
                FROM organization_units
                WHERE parent_unit_id = $1 AND is_active = true
            )",
        )
        .bind(unit_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| {
            tracing::error!(%unit_id, %error, "failed to check active organization unit children");
            AppError::InternalServerError("ไม่สามารถตรวจสอบหน่วยงานย่อยได้".to_string())
        })?;
        if has_active_child {
            return Err(AppError::Conflict("กรุณาปิดใช้งานหน่วยงานย่อยก่อน".to_string()));
        }
    }

    sqlx::query(
        "UPDATE organization_units
         SET is_active = $2, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(unit_id)
    .bind(is_active)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to update organization unit status");
        AppError::InternalServerError("ไม่สามารถเปลี่ยนสถานะหน่วยงานได้".to_string())
    })?;

    let action = if is_active {
        "reactivate"
    } else {
        "deactivate"
    };
    AuditLogBuilder::new(action, "organization_unit")
        .user(actor_user_id, None, None)
        .entity(unit_id, Some(unit.name))
        .old_values(json!({ "is_active": unit.is_active }))
        .new_values(json!({ "is_active": is_active }))
        .changes(json!({
            "is_active": {
                "from": unit.is_active,
                "to": is_active
            }
        }))
        .description(if is_active {
            "เปิดใช้งานหน่วยงาน"
        } else {
            "ปิดใช้งานหน่วยงาน"
        })
        .save_in_transaction(tx)
        .await
        .map_err(|error| {
            tracing::error!(%unit_id, %error, "failed to save organization unit status audit");
            AppError::InternalServerError("ไม่สามารถบันทึกประวัติการเปลี่ยนสถานะได้".to_string())
        })?;

    Ok(StatusTransitionOutcome::Changed { is_active })
}

pub async fn set_organization_unit_active(
    pool: &PgPool,
    unit_id: Uuid,
    is_active: bool,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError> {
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to start organization unit status transaction");
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;
    let unit = lock_organization_unit(&mut tx, unit_id).await?;
    let parent_unit_id = unit.parent_unit_id;
    let outcome = transition_locked_organization_unit_status(
        &mut tx,
        unit_id,
        unit,
        parent_unit_id,
        is_active,
        actor_user_id,
    )
    .await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(%unit_id, %error, "failed to commit organization unit status transaction");
        AppError::InternalServerError("ไม่สามารถบันทึกสถานะหน่วยงานได้".to_string())
    })?;

    Ok(outcome)
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
