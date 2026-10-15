//! Staff owns licence relationships; Files owns inspection, storage and delivery.
use crate::{personnel::ReplaceStaffTeachingLicenseFile, staff_access_policy};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_file_platform::{
    access::FilePolicyAction,
    platform_types::{FilePurpose, FileVisibility},
    repository::PlatformFile,
};
use school_permissions::registry::codes;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

fn denied() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงเอกสารใบประกอบวิชาชีพนี้".into())
}

pub fn require_manage(actor: &ActorContext, staff_id: Uuid) -> Result<(), AppError> {
    actor.require_permission(codes::STAFF_UPDATE_ALL)?;
    if !staff_access_policy::can_read_staff_pii(actor, staff_id) {
        return Err(denied());
    }
    Ok(())
}

pub async fn authorize_upload(
    pool: &PgPool,
    actor: &ActorContext,
    resource: Option<Uuid>,
) -> Result<Uuid, AppError> {
    let staff_id = resource.ok_or_else(denied)?;
    require_manage(actor, staff_id)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='staff')")
            .bind(staff_id)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(denied());
    }
    Ok(staff_id)
}

pub async fn authorize_file(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    action: FilePolicyAction,
    resource: Option<Uuid>,
) -> Result<(), AppError> {
    let staff_id = file.owner_user_id.ok_or_else(denied)?;
    if file.purpose != FilePurpose::TeachingLicense
        || file.visibility != FileVisibility::Private
        || resource != Some(staff_id)
        || action != FilePolicyAction::Read
        || !staff_access_policy::can_read_staff_pii(actor, staff_id)
    {
        return Err(denied());
    }
    let relation: Option<(bool, bool)> = sqlx::query_as(
        "SELECT coalesce(i.teaching_license_file_id=f.id,false),
           f.created_by=$3 AND f.retention_class='temporary' AND f.expires_at>now()
         FROM files f JOIN users u ON u.id=f.owner_user_id AND u.user_type='staff'
         LEFT JOIN staff_info i ON i.user_id=u.id
         WHERE f.id=$1 AND u.id=$2 AND f.lifecycle_status='ready' AND f.deleted_at IS NULL",
    )
    .bind(file.id)
    .bind(staff_id)
    .bind(actor.user_id)
    .fetch_optional(pool)
    .await?;
    match relation {
        Some((true, _)) => staff_access_policy::can_read_staff_profile(pool, actor, staff_id).await,
        Some((false, true)) => require_manage(actor, staff_id),
        _ => Err(denied()),
    }
}

/// Lock order matches staff mutation: user, then file. Hold until Files transitions lifecycle.
pub async fn delete_guard<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    resource: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, AppError> {
    let staff_id = file.owner_user_id.ok_or_else(denied)?;
    if resource != Some(staff_id)
        || file.purpose != FilePurpose::TeachingLicense
        || file.visibility != FileVisibility::Private
    {
        return Err(denied());
    }
    require_manage(actor, staff_id)?;
    let mut tx = pool.begin().await?;
    let user: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND user_type='staff' FOR UPDATE")
            .bind(staff_id)
            .fetch_optional(&mut *tx)
            .await?;
    if user.is_none() {
        return Err(denied());
    }
    let disposable: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM files f WHERE f.id=$1 AND f.owner_user_id=$2
          AND f.created_by=$3 AND f.purpose_code='teaching_license' AND f.visibility='private'
          AND f.retention_class='temporary' AND NOT EXISTS(SELECT 1 FROM staff_info i WHERE i.teaching_license_file_id=f.id))")
        .bind(file.id).bind(staff_id).bind(actor.user_id).fetch_one(&mut *tx).await?;
    if !disposable {
        return Err(denied());
    }
    Ok(tx)
}

/// Caller holds the staff-user row lock. A stale form cannot replace a newer document.
pub async fn replace_file(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    actor_id: Uuid,
    patch: &ReplaceStaffTeachingLicenseFile,
) -> Result<(), AppError> {
    let current: Option<Uuid> = sqlx::query_scalar(
        "SELECT teaching_license_file_id FROM staff_info WHERE user_id=$1 FOR UPDATE",
    )
    .bind(staff_id)
    .fetch_one(&mut **tx)
    .await?;
    if current != patch.expected_file_id {
        return Err(AppError::Conflict(
            "เอกสารใบประกอบวิชาชีพเปลี่ยนแล้ว กรุณาตรวจข้อมูลล่าสุดก่อนบันทึก".into(),
        ));
    }
    if current == patch.file_id {
        return Ok(());
    }
    if let Some(id) = patch.file_id {
        let valid: Option<bool> = sqlx::query_scalar(
            "SELECT owner_user_id=$2 AND created_by=$3 AND purpose_code='teaching_license'
              AND visibility='private' AND lifecycle_status='ready' AND deleted_at IS NULL
              AND retention_class='temporary' AND expires_at>now()
             FROM files WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .bind(staff_id)
        .bind(actor_id)
        .fetch_optional(&mut **tx)
        .await?;
        if valid != Some(true) {
            return Err(AppError::BadRequest(
                "ไฟล์ใบประกอบวิชาชีพไม่พร้อมใช้งานหรือไม่ใช่ของบุคลากรนี้".into(),
            ));
        }
        sqlx::query("UPDATE files SET retention_class='standard',expires_at=NULL,updated_at=now() WHERE id=$1")
            .bind(id).execute(&mut **tx).await?;
    }
    sqlx::query(
        "UPDATE staff_info SET teaching_license_file_id=$2,updated_at=now() WHERE user_id=$1",
    )
    .bind(staff_id)
    .bind(patch.file_id)
    .execute(&mut **tx)
    .await?;
    if let Some(id) = current {
        // Existing Files retention worker owns durable cleanup after this transaction commits.
        sqlx::query("UPDATE files SET retention_class='temporary',expires_at=now(),updated_at=now() WHERE id=$1")
            .bind(id).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_licence_management_requires_update_and_exact_pii_scope() {
        let user = Uuid::new_v4();
        let actor = |permissions: &[&str]| ActorContext {
            user_id: user,
            permissions: permissions.iter().map(|v| v.to_string()).collect(),
        };
        assert!(require_manage(&actor(&[codes::STAFF_UPDATE_ALL]), user).is_err());
        assert!(require_manage(&actor(&[codes::STAFF_PII_READ_SCHOOL]), user).is_err());
        let own = actor(&[codes::STAFF_UPDATE_ALL, codes::STAFF_PII_READ_OWN]);
        assert!(require_manage(&own, user).is_ok());
        assert!(require_manage(&own, Uuid::new_v4()).is_err());
        assert!(require_manage(
            &actor(&[codes::STAFF_UPDATE_ALL, codes::STAFF_PII_READ_SCHOOL]),
            Uuid::new_v4()
        )
        .is_ok());
    }
}
