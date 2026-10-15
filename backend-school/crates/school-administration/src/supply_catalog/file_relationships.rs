use crate::supply_catalog::access_policy::require_action;
use crate::supply_catalog::access_policy::SupplyAction;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_file_platform::access::FilePolicyAction;
use school_file_platform::{
    platform_service::FilePlatform,
    platform_types::{FilePurpose, FileVisibility},
    repository::{DeleteWork, PlatformFile, SqlFileRepository},
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

fn denied() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงไฟล์วัสดุนี้".into())
}
fn require_upload(actor: &ActorContext, purpose: FilePurpose) -> Result<(), AppError> {
    match purpose {
        FilePurpose::SupplyCatalogImage => actor.require_any_permission(&[
            school_permissions::registry::codes::SUPPLY_CATALOG_MANAGE_SCHOOL,
            school_permissions::registry::codes::SUPPLY_CATALOG_REQUEST_OWN,
            school_permissions::registry::codes::SUPPLY_CATALOG_APPROVE_SCHOOL,
        ]),
        FilePurpose::SupplyProposalSource => actor.require_any_permission(&[
            school_permissions::registry::codes::SUPPLY_CATALOG_REQUEST_OWN,
            school_permissions::registry::codes::SUPPLY_CATALOG_APPROVE_SCHOOL,
        ]),
        _ => Err(denied()),
    }
}
/// Generic upload is deliberately staged under the authenticated creator, without a domain resource.
pub fn authorize_create(
    actor: &ActorContext,
    purpose: FilePurpose,
    resource_id: Option<Uuid>,
) -> Result<Uuid, AppError> {
    if resource_id.is_some() {
        return Err(denied());
    }
    require_upload(actor, purpose)?;
    Ok(actor.user_id)
}
/// File rows are always locked in UUID order before any owning domain row.
pub async fn lock_files(
    tx: &mut Transaction<'_, Postgres>,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<(), AppError> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    for id in ids {
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM files WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| AppError::BadRequest("ไม่พบไฟล์".into()))?;
    }
    Ok(())
}
/// Caller holds the file lock. A proposal permits reviewers to retain only its exact submitted photo.
pub async fn attach(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    purpose: FilePurpose,
    allowed_relation: Option<Uuid>,
) -> Result<(), AppError> {
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM files f JOIN file_versions v ON v.id=f.current_version_id AND v.file_id=f.id WHERE f.id=$1 AND f.purpose_code=$2 AND f.visibility='private' AND f.lifecycle_status='ready' AND f.deleted_at IS NULL AND v.deleted_at IS NULL AND v.storage_status='stored' AND v.scan_status='clean' AND v.storage_class='private' AND (f.expires_at IS NULL OR f.expires_at>now()) AND (f.created_by=$3 OR EXISTS(SELECT 1 FROM supply_proposals p WHERE p.id=$4 AND p.status='pending' AND p.image_file_id=f.id AND $2='supply_catalog_image')))").bind(id).bind(purpose.code()).bind(actor.user_id).bind(allowed_relation).fetch_one(&mut **tx).await?;
    if !valid {
        return Err(AppError::BadRequest(
            "ไฟล์ต้องเป็นไฟล์ส่วนตัวที่พร้อมใช้งานและอัปโหลดโดยผู้ดำเนินการ".into(),
        ));
    }
    let used:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM supply_items WHERE image_file_id=$1 AND id IS DISTINCT FROM $2) OR EXISTS(SELECT 1 FROM supply_proposals WHERE (image_file_id=$1 OR source_file_id=$1) AND id IS DISTINCT FROM $2)").bind(id).bind(allowed_relation).fetch_one(&mut **tx).await?;
    if used {
        return Err(AppError::Conflict("ไฟล์นี้ถูกใช้ในรายการอื่นแล้ว".into()));
    }
    sqlx::query(
        "UPDATE files SET retention_class='standard',expires_at=NULL,updated_at=now() WHERE id=$1",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
pub async fn require_file_access(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    action: FilePolicyAction,
    resource_id: Option<Uuid>,
) -> Result<(), AppError> {
    if file.visibility != FileVisibility::Private
        || !matches!(
            file.purpose,
            FilePurpose::SupplyCatalogImage | FilePurpose::SupplyProposalSource
        )
    {
        return Err(denied());
    }
    if action == FilePolicyAction::Delete {
        let tx = authorize_delete_guard(pool, actor, file, resource_id).await?;
        tx.rollback().await?;
        return Ok(());
    }
    if action != FilePolicyAction::Read {
        return Err(denied());
    }
    if let Some(resource) = resource_id {
        let item: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM supply_items WHERE id=$1 AND image_file_id=$2)",
        )
        .bind(resource)
        .bind(file.id)
        .fetch_one(pool)
        .await?;
        if item && file.purpose == FilePurpose::SupplyCatalogImage {
            return require_action(actor, SupplyAction::Read, None, false);
        }
        let owner:Option<Uuid>=sqlx::query_scalar("SELECT proposed_by FROM supply_proposals WHERE id=$1 AND status='pending' AND (image_file_id=$2 OR source_file_id=$2)").bind(resource).bind(file.id).fetch_optional(pool).await?;
        if let Some(owner) = owner {
            return if actor
                .has_permission(school_permissions::registry::codes::SUPPLY_CATALOG_APPROVE_SCHOOL)
            {
                require_action(actor, SupplyAction::Review, None, false)
            } else {
                require_action(actor, SupplyAction::Propose, Some(owner), false)
            };
        }
        return Err(denied());
    }
    require_upload(actor, file.purpose)?;
    let own_unlinked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM files WHERE id=$1 AND created_by=$2) AND NOT EXISTS(SELECT 1 FROM supply_items WHERE image_file_id=$1) AND NOT EXISTS(SELECT 1 FROM supply_proposals WHERE image_file_id=$1 OR source_file_id=$1)").bind(file.id).bind(actor.user_id).fetch_one(pool).await?;
    if own_unlinked {
        Ok(())
    } else {
        Err(denied())
    }
}
/// Retains the file row lock until the platform's durable lifecycle transition commits.
pub async fn authorize_delete_guard<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    resource_id: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, AppError> {
    if resource_id.is_some() || file.visibility != FileVisibility::Private {
        return Err(denied());
    }
    require_upload(actor, file.purpose)?;
    let mut tx = pool.begin().await?;
    lock_files(&mut tx, [file.id]).await?;
    let own:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM files WHERE id=$1 AND created_by=$2 AND purpose_code=$3 AND visibility='private')").bind(file.id).bind(actor.user_id).bind(file.purpose.code()).fetch_one(&mut *tx).await?;
    if !own {
        return Err(denied());
    }
    let attached:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM supply_items WHERE image_file_id=$1) OR EXISTS(SELECT 1 FROM supply_proposals WHERE image_file_id=$1 OR source_file_id=$1)").bind(file.id).fetch_one(&mut *tx).await?;
    if attached {
        return Err(AppError::Conflict("กรุณาลบหรือแทนที่ไฟล์ผ่านทะเบียนวัสดุ".into()));
    }
    Ok(tx)
}
pub async fn prepare_delete(
    pool: &PgPool,
    tx: &mut Transaction<'_, Postgres>,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<Vec<DeleteWork>, AppError> {
    let repository = SqlFileRepository::new(pool.clone());
    let mut work = Vec::new();
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    for id in ids {
        work.extend(
            repository
                .request_delete_in_transaction(tx, id)
                .await
                .map_err(|_| {
                    AppError::InternalServerError("supply_file_delete_request_failed".into())
                })?,
        );
    }
    Ok(work)
}
pub async fn finish_delete(platform: &FilePlatform, pool: &PgPool, work: Vec<DeleteWork>) {
    if work.is_empty() {
        return;
    }
    let repository = SqlFileRepository::new(pool.clone());
    if let Err(error) = platform.complete_prepared_delete(&repository, work).await {
        tracing::warn!(
            reason = error.log_safe_code(),
            "Supply file cleanup remains durable retry work"
        );
    }
}
