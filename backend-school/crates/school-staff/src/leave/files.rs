//! Staff owns private evidence relationships; File Platform owns inspection and delivery.
use super::{
    models::LeaveFileSnapshot,
    policy::{denied, invalid},
    service,
};
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

pub async fn authorize_create(
    pool: &PgPool,
    actor: &ActorContext,
    purpose: FilePurpose,
    resource: Option<Uuid>,
) -> Result<Uuid, AppError> {
    let id = resource.ok_or_else(denied)?;
    if purpose == FilePurpose::StaffLeaveSignature {
        actor.require_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED)?;
        if id != actor.user_id {
            return Err(denied());
        }
        return Ok(actor.user_id);
    }
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    let d = service::detail(pool, actor, id).await?;
    if d.request.owner_id != actor.user_id || !d.can_edit {
        return Err(denied());
    }
    Ok(actor.user_id)
}
pub async fn record_upload(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    file: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO staff_leave_file_uploads(file_id,request_id,uploaded_by) VALUES($1,$2,$3)",
    )
    .bind(file)
    .bind(id)
    .bind(actor)
    .execute(pool)
    .await?;
    Ok(())
}
pub async fn authorize_existing(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    action: FilePolicyAction,
    resource: Option<Uuid>,
) -> Result<(), AppError> {
    if file.visibility != FileVisibility::Private || action != FilePolicyAction::Read {
        return Err(denied());
    }
    let id = resource.ok_or_else(denied)?;
    if file.purpose == FilePurpose::StaffLeaveSignature
        && id == actor.user_id
        && file.owner_user_id == Some(actor.user_id)
    {
        return actor.require_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED);
    }
    service::detail(pool, actor, id).await?;
    let related:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_leave_file_links WHERE request_id=$1 AND file_id=$2) OR EXISTS(SELECT 1 FROM staff_leave_file_uploads WHERE request_id=$1 AND file_id=$2 AND uploaded_by=$3)").bind(id).bind(file.id).bind(actor.user_id).fetch_one(pool).await?;
    if !related {
        return Err(denied());
    }
    Ok(())
}
pub async fn delete_guard<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    resource: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, AppError> {
    authorize_create(pool, actor, file.purpose, resource).await?;
    if file.owner_user_id != Some(actor.user_id) || file.visibility != FileVisibility::Private {
        return Err(denied());
    }
    let mut tx = pool.begin().await?;
    let allowed:Option<bool>=sqlx::query_scalar("SELECT created_by=$2 AND retention_class='temporary' AND NOT EXISTS(SELECT 1 FROM staff_leave_file_links WHERE file_id=f.id) FROM files f WHERE id=$1 FOR UPDATE").bind(file.id).bind(actor.user_id).fetch_optional(&mut *tx).await?;
    if allowed != Some(true) {
        return Err(denied());
    }
    Ok(tx)
}
pub async fn validate_evidence(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    files: &[Uuid],
) -> Result<Vec<LeaveFileSnapshot>, AppError> {
    if files.len() > 20 {
        return Err(invalid("แนบหลักฐานได้ไม่เกิน 20 ไฟล์"));
    }
    let mut ids = files.to_vec();
    ids.sort();
    ids.dedup();
    if ids.len() != files.len() {
        return Err(invalid("ไฟล์หลักฐานซ้ำ"));
    }
    let rows:Vec<(Uuid,Uuid,String)>=sqlx::query_as("SELECT f.id,v.id,v.checksum FROM files f JOIN file_versions v ON v.id=f.current_version_id AND v.file_id=f.id WHERE f.id=ANY($1) AND f.purpose_code='staff_leave_evidence' AND f.visibility='private' AND f.lifecycle_status='ready' AND f.deleted_at IS NULL AND ((f.created_by=$3 AND EXISTS(SELECT 1 FROM staff_leave_file_uploads u WHERE u.file_id=f.id AND u.request_id=$2 AND u.uploaded_by=$3) AND f.retention_class='temporary' AND f.expires_at>now()) OR EXISTS(SELECT 1 FROM staff_leave_file_links l WHERE l.request_id=$2 AND l.file_id=f.id)) ORDER BY f.id FOR UPDATE OF f").bind(&ids).bind(id).bind(actor).fetch_all(&mut **tx).await?;
    if rows.len() != ids.len() {
        return Err(invalid("หลักฐานไม่พร้อมใช้งานหรือไม่ได้อัปโหลดสำหรับคำขอนี้"));
    }
    Ok(rows
        .into_iter()
        .map(|(file_id, version_id, checksum)| LeaveFileSnapshot {
            file_id,
            version_id,
            checksum,
        })
        .collect())
}
pub async fn signature_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
) -> Result<LeaveFileSnapshot, AppError> {
    let (file_id, version_id, checksum) =
        school_workflow::signatures::private_image_snapshot_in_transaction(
            tx,
            actor,
            id,
            FilePurpose::StaffLeaveSignature,
        )
        .await?;
    Ok(LeaveFileSnapshot {
        file_id,
        version_id,
        checksum,
    })
}
pub async fn retain_snapshots(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    revision: i64,
    files: &[LeaveFileSnapshot],
) -> Result<(), AppError> {
    for f in files {
        sqlx::query("INSERT INTO staff_leave_file_links(request_id,revision,file_id,version_id,checksum) VALUES($1,$2,$3,$4,$5)").bind(id).bind(revision).bind(f.file_id).bind(f.version_id).bind(&f.checksum).execute(&mut **tx).await?;
    }
    let ids: Vec<_> = files.iter().map(|f| f.file_id).collect();
    sqlx::query("UPDATE files SET retention_class='standard',expires_at=NULL,updated_at=now() WHERE id=ANY($1)").bind(ids).execute(&mut **tx).await?;
    Ok(())
}
