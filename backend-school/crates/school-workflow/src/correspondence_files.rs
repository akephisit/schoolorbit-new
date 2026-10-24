//! Correspondence owns file relationships and policy; File Platform owns storage.
use crate::{
    correspondence,
    process::{denied, invalid},
    process_models::*,
};
use school_authorization::ActorContext;
use school_database::DbSession;
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
    match purpose {
        FilePurpose::CorrespondenceSignature => {
            require_signature_owner(actor, id)?;
        }
        FilePurpose::CorrespondenceDocument => {
            correspondence::require_upload(pool, actor, id).await?;
        }
        _ => return Err(denied()),
    };
    Ok(actor.user_id)
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
    match file.purpose {
        FilePurpose::CorrespondenceSignature => {
            if resource == Some(actor.user_id) && file.owner_user_id == Some(actor.user_id) {
                return actor.require_permission(codes::CORRESPONDENCE_SIGN_OWN);
            }
            let correspondence = resource.ok_or_else(denied)?;
            correspondence::require_read(pool, actor, correspondence).await?;
            let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                pool,
                "workflow.correspondence_files.signed",
            )
            .await?;
            let signed:bool=DbSession::new(&mut *connection).scalar("workflow.src.correspondence_files.authorize_existing", "SELECT EXISTS(SELECT 1 FROM correspondence_signing_evidence WHERE correspondence_id=$1 AND asset_file_id=$2)").bind(correspondence).bind(file.id).fetch_one().await?;
            if signed {
                Ok(())
            } else {
                Err(denied())
            }
        }
        FilePurpose::CorrespondenceDocument => {
            let id = resource.ok_or_else(denied)?;
            correspondence::require_read(pool, actor, id).await?;
            let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                pool,
                "workflow.correspondence_files.related",
            )
            .await?;
            let related:bool=DbSession::new(&mut *connection).scalar("workflow.src.correspondence_files.authorize_existing", "SELECT EXISTS(SELECT 1 FROM correspondence_file_links WHERE correspondence_id=$1 AND file_id=$2) OR EXISTS(SELECT 1 FROM correspondence_file_uploads WHERE correspondence_id=$1 AND file_id=$2 AND uploaded_by=$3)").bind(id).bind(file.id).bind(actor.user_id).fetch_one().await?;
            if related {
                Ok(())
            } else {
                Err(denied())
            }
        }
        _ => Err(denied()),
    }
}
/// Record a resource-bound staging relationship after successful platform upload.
pub async fn record_upload(
    pool: &PgPool,
    actor: Uuid,
    resource: Uuid,
    file: Uuid,
) -> Result<(), AppError> {
    let mut database_stage_0 = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "workflow.correspondence_files.record_upload",
    )
    .await?;
    DbSession::new(&mut *database_stage_0).query("workflow.src.correspondence_files.record_upload", "INSERT INTO correspondence_file_uploads(file_id,correspondence_id,uploaded_by) VALUES($1,$2,$3)").bind(file).bind(resource).bind(actor).execute().await?;
    Ok(())
}
pub async fn delete_guard(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    resource: Option<Uuid>,
) -> Result<school_tenancy::ObservedTransaction, AppError> {
    if file.owner_user_id != Some(actor.user_id) || file.visibility != FileVisibility::Private {
        return Err(denied());
    }
    if file.purpose == FilePurpose::CorrespondenceSignature {
        if resource != Some(actor.user_id) {
            return Err(denied());
        }
        actor.require_permission(codes::CORRESPONDENCE_SIGN_OWN)?;
    } else {
        let id = resource.ok_or_else(denied)?;
        correspondence::require_upload(pool, actor, id).await?;
    }
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "workflow.correspondence_files.delete_guard",
    )
    .await?;
    let disposable:Option<bool>=DbSession::new(&mut *tx).scalar("workflow.src.correspondence_files.delete_guard", "SELECT created_by=$2 AND retention_class='temporary' AND NOT EXISTS(SELECT 1 FROM correspondence_file_links WHERE file_id=f.id) AND NOT EXISTS(SELECT 1 FROM correspondence_signature_assets WHERE file_id=f.id) AND NOT EXISTS(SELECT 1 FROM correspondence_signing_evidence WHERE asset_file_id=f.id) FROM files f WHERE id=$1 FOR UPDATE").bind(file.id).bind(actor.user_id).fetch_optional().await?;
    if disposable != Some(true) {
        return Err(denied());
    }
    Ok(tx)
}
/// Locks immutable source versions and promotes accepted documents in the caller transaction.
pub async fn attach_documents(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    revision: i64,
    files: &[Uuid],
) -> Result<Vec<CorrespondenceFileSnapshot>, AppError> {
    if files.len() > 50 {
        return Err(invalid("เอกสารแนบเกิน 50 ไฟล์"));
    }
    let mut unique = files.to_vec();
    unique.sort();
    unique.dedup();
    let rows:Vec<(Uuid,Uuid,String)>=DbSession::new(&mut **tx).query_as("workflow.src.correspondence_files.attach_documents", "SELECT f.id,v.id,v.checksum FROM files f JOIN file_versions v ON v.id=f.current_version_id AND v.file_id=f.id WHERE f.id=ANY($1) AND f.purpose_code='correspondence_document' AND f.visibility='private' AND f.lifecycle_status='ready' AND f.deleted_at IS NULL AND (EXISTS(SELECT 1 FROM correspondence_file_links l WHERE l.correspondence_id=$2 AND l.file_id=f.id) OR (f.created_by=$3 AND EXISTS(SELECT 1 FROM correspondence_file_uploads u WHERE u.file_id=f.id AND u.correspondence_id=$2 AND u.uploaded_by=$3) AND f.retention_class='temporary' AND f.expires_at>now())) ORDER BY f.id FOR UPDATE OF f").bind(&unique).bind(id).bind(actor).fetch_all().await?;
    if rows.len() != unique.len() {
        return Err(invalid("เอกสารยังไม่พร้อมใช้หรือไม่ได้อัปโหลดสำหรับหนังสือนี้"));
    }
    // Links are inserted after the immutable document row by the caller.
    let _revision = revision;
    DbSession::new(&mut **tx).query("workflow.src.correspondence_files.attach_documents", "UPDATE files SET retention_class='standard',expires_at=NULL,updated_at=now() WHERE id=ANY($1)").bind(unique).execute().await?;
    Ok(rows
        .into_iter()
        .map(
            |(file_id, version_id, checksum)| CorrespondenceFileSnapshot {
                file_id,
                version_id,
                checksum,
            },
        )
        .collect())
}
pub async fn attach_signature(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    file: Uuid,
) -> Result<(), AppError> {
    let valid:Option<bool>=DbSession::new(&mut **tx).scalar("workflow.src.correspondence_files.attach_signature", "SELECT owner_user_id=$2 AND created_by=$2 AND purpose_code='correspondence_signature' AND visibility='private' AND lifecycle_status='ready' AND deleted_at IS NULL AND retention_class='temporary' AND expires_at>now() FROM files WHERE id=$1 FOR UPDATE").bind(file).bind(actor).fetch_optional().await?;
    if valid != Some(true) {
        return Err(invalid("ไฟล์ลายเซ็นไม่พร้อมใช้งานหรือไม่ใช่ของคุณ"));
    }
    DbSession::new(&mut **tx).query("workflow.src.correspondence_files.attach_signature",
        "UPDATE files SET retention_class='standard',expires_at=NULL,updated_at=now() WHERE id=$1",
    )
    .bind(file)
    .execute()
    .await?;
    Ok(())
}
fn require_signature_owner(actor: &ActorContext, id: Uuid) -> Result<(), AppError> {
    actor.require_permission(codes::CORRESPONDENCE_SIGN_OWN)?;
    if id == actor.user_id {
        Ok(())
    } else {
        Err(denied())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn personal_signature_permission_never_grants_another_owners_asset() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::CORRESPONDENCE_SIGN_OWN.into()],
        };
        assert!(require_signature_owner(&actor, actor.user_id).is_ok());
        assert!(require_signature_owner(&actor, Uuid::new_v4()).is_err());
        let reader = ActorContext {
            user_id: actor.user_id,
            permissions: vec![codes::CORRESPONDENCE_READ_SCHOOL.into()],
        };
        assert!(require_signature_owner(&reader, reader.user_id).is_err());
    }
}

/// Read the exact authorized source versions before expensive rendering; attach rechecks under locks.
pub async fn prepare_documents(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    files: &[Uuid],
) -> Result<Vec<CorrespondenceFileSnapshot>, AppError> {
    if files.len() > 50 {
        return Err(invalid("เอกสารแนบเกิน 50 ไฟล์"));
    }
    let mut unique = files.to_vec();
    unique.sort();
    unique.dedup();
    let mut database_stage_0 = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "workflow.correspondence_files.prepare_documents",
    )
    .await?;
    let rows:Vec<(Uuid,Uuid,String)>=DbSession::new(&mut *database_stage_0).query_as("workflow.src.correspondence_files.prepare_documents", "SELECT f.id,v.id,v.checksum FROM files f JOIN file_versions v ON v.id=f.current_version_id AND v.file_id=f.id WHERE f.id=ANY($1) AND f.purpose_code='correspondence_document' AND f.visibility='private' AND f.lifecycle_status='ready' AND f.deleted_at IS NULL AND (EXISTS(SELECT 1 FROM correspondence_file_links l WHERE l.correspondence_id=$2 AND l.file_id=f.id) OR (f.created_by=$3 AND EXISTS(SELECT 1 FROM correspondence_file_uploads u WHERE u.file_id=f.id AND u.correspondence_id=$2 AND u.uploaded_by=$3) AND f.retention_class='temporary' AND f.expires_at>now())) ORDER BY f.id").bind(&unique).bind(id).bind(actor).fetch_all().await?;
    if rows.len() != unique.len() {
        return Err(invalid("เอกสารยังไม่พร้อมใช้หรือไม่ได้อัปโหลดสำหรับหนังสือนี้"));
    }
    Ok(rows
        .into_iter()
        .map(
            |(file_id, version_id, checksum)| CorrespondenceFileSnapshot {
                file_id,
                version_id,
                checksum,
            },
        )
        .collect())
}
