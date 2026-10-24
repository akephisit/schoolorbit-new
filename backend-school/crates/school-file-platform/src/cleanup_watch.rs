//! Durable physical locators outlive business metadata. Absence never proves settlement.
use super::{
    platform_types::StorageClass,
    purpose_registry::persisted_object_key,
    repository::{NewUpload, RepositoryError, SqlFileRepository, STORAGE_PROVIDER_CODE},
    storage_namespace::StorageNamespace,
    storage_provider::{StorageError, StorageProvider, StoredObject},
};
use school_database::DbSession;
use sqlx::{FromRow, PgConnection};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct WritePermit {
    pub(crate) _watch_id: Uuid,
    pub(crate) object: StoredObject,
    pub(crate) namespace: StorageNamespace,
}
#[derive(Clone)]
pub struct UploadWritePermits {
    pub(crate) original: WritePermit,
    pub(crate) derivatives: Vec<(Uuid, WritePermit)>,
}
impl UploadWritePermits {
    #[cfg(test)]
    pub(crate) fn for_fixture(upload: &NewUpload) -> Self {
        Self {
            original: WritePermit {
                _watch_id: upload.version_id,
                object: upload.original.clone(),
                namespace: upload.namespace.clone(),
            },
            derivatives: upload
                .derivatives
                .iter()
                .map(|d| {
                    (
                        d.id,
                        WritePermit {
                            _watch_id: d.id,
                            object: d.object.clone(),
                            namespace: upload.namespace.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

pub(crate) async fn register_upload(
    connection: &mut PgConnection,
    upload: &NewUpload,
) -> Result<UploadWritePermits, RepositoryError> {
    let original = register_new_target(
        connection,
        upload.file_id,
        "version",
        upload.version_id,
        &upload.original,
        &upload.namespace,
    )
    .await?;
    let mut derivatives = Vec::with_capacity(upload.derivatives.len());
    for derivative in &upload.derivatives {
        derivatives.push((
            derivative.id,
            register_new_target(
                connection,
                upload.file_id,
                "derivative",
                derivative.id,
                &derivative.object,
                &upload.namespace,
            )
            .await?,
        ));
    }
    Ok(UploadWritePermits {
        original,
        derivatives,
    })
}
async fn register_new_target(
    connection: &mut PgConnection,
    file_id: Uuid,
    kind: &'static str,
    target_id: Uuid,
    object: &StoredObject,
    namespace: &StorageNamespace,
) -> Result<WritePermit, RepositoryError> {
    // No ON CONFLICT binding: an existing legacy or revoked identity must never
    // become a current-config write permit merely because its locator matches.
    let watch_id = DbSession::new(connection).scalar::<Uuid>("file_watch.register_new",
        "INSERT INTO file_storage_cleanup_watches(file_id,target_kind,target_id,provider_code,storage_class,object_key,content_type,namespace_fingerprint,namespace_bound_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,statement_timestamp()) RETURNING id")
        .bind(file_id).bind(kind).bind(target_id).bind(STORAGE_PROVIDER_CODE)
        .bind(class_code(object.storage_class())).bind(object.object_key.as_str()).bind(&object.content_type)
        .bind(namespace.fingerprint()).fetch_one().await.map_err(|_| RepositoryError::OperationFailed)?;
    Ok(WritePermit {
        _watch_id: watch_id,
        object: object.clone(),
        namespace: namespace.clone(),
    })
}

pub(crate) async fn namespace_for_target(
    connection: &mut PgConnection,
    kind: &'static str,
    target_id: Uuid,
) -> Result<Option<StorageNamespace>, RepositoryError> {
    let value = DbSession::new(connection).scalar::<Option<String>>("file_watch.target_namespace",
        "SELECT namespace_fingerprint FROM file_storage_cleanup_watches WHERE target_kind=$1 AND target_id=$2")
        .bind(kind).bind(target_id).fetch_optional().await.map_err(|_| RepositoryError::OperationFailed)?.flatten();
    value
        .map(StorageNamespace::parse)
        .transpose()
        .map_err(|_| RepositoryError::InvalidPersistedState)
}

pub(crate) fn verify_namespace(
    provider: &dyn StorageProvider,
    object: &StoredObject,
    expected: Option<&StorageNamespace>,
) -> Result<(), StorageError> {
    let actual = provider.namespace(object.storage_class())?;
    if expected.is_some_and(|expected| *expected == actual) {
        Ok(())
    } else {
        Err(StorageError::NamespaceUnverified)
    }
}

#[derive(Clone)]
pub struct CleanupClaim {
    pub owner: String,
    pub generation: i64,
}
#[derive(Clone)]
pub struct LeasedCleanupWatch {
    pub id: Uuid,
    pub file_id: Uuid,
    pub object: StoredObject,
    pub namespace: StorageNamespace,
    pub claim: CleanupClaim,
}
#[derive(Clone, Copy)]
pub enum CleanupObservation {
    Absent,
    Deleted,
    Failed { error_code: &'static str },
}
#[derive(FromRow)]
struct ClaimedWatch {
    id: Uuid,
    file_id: Uuid,
    storage_class: String,
    object_key: String,
    content_type: String,
    namespace_fingerprint: String,
    lease_owner: String,
    lease_generation: i64,
}

impl SqlFileRepository {
    /// Persist the first queue opportunity, even if this sweep later times out
    /// before completing its first operation. No authority or claim is changed.
    pub async fn cleanup_starting_preference(
        &self,
        namespaces: &[StorageNamespace],
    ) -> Result<bool, RepositoryError> {
        let allowed = namespaces
            .iter()
            .map(|n| n.fingerprint().to_string())
            .collect::<Vec<_>>();
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            self.pool(),
            "file_watch.starting_queue",
        )
        .await
        .map_err(|_| RepositoryError::OperationFailed)?;
        let value = DbSession::new(&mut *connection).scalar::<bool>("file_watch.starting_queue", "WITH due AS (SELECT id FROM file_storage_cleanup_watches WHERE revoked_at IS NOT NULL AND namespace_fingerprint=ANY($1) AND next_probe_at<=statement_timestamp() AND (lease_owner IS NULL OR lease_expires_at<=statement_timestamp()) ORDER BY next_probe_at,id FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE file_storage_cleanup_watches w SET prefer_watch_next=NOT prefer_watch_next FROM due WHERE w.id=due.id RETURNING NOT w.prefer_watch_next")
            .bind(allowed).fetch_optional().await.map_err(|_|RepositoryError::OperationFailed)?;
        Ok(value.unwrap_or(false))
    }
    /// Explicit recovery work entry; no file-row dependency survives Certificate purge.
    pub async fn claim_cleanup_watch(
        &self,
        worker: &str,
        lease: Duration,
        namespaces: &[StorageNamespace],
    ) -> Result<Option<LeasedCleanupWatch>, RepositoryError> {
        if worker.trim().is_empty() || worker.len() > 100 {
            return Err(RepositoryError::InvalidPersistedState);
        }
        let lease_us =
            i64::try_from(lease.as_micros()).map_err(|_| RepositoryError::InvalidPersistedState)?;
        if lease_us <= 0 {
            return Err(RepositoryError::InvalidPersistedState);
        }
        let allowed = namespaces
            .iter()
            .map(|n| n.fingerprint().to_string())
            .collect::<Vec<_>>();
        let mut connection =
            school_tenancy::TenantPool::observed_checkout_pool(self.pool(), "file_watch.claim")
                .await
                .map_err(|_| RepositoryError::OperationFailed)?;
        let row=DbSession::new(&mut *connection).query_as::<ClaimedWatch>("file_watch.claim", "WITH due AS (SELECT id FROM file_storage_cleanup_watches WHERE revoked_at IS NOT NULL AND namespace_fingerprint=ANY($1) AND next_probe_at<=statement_timestamp() AND (lease_owner IS NULL OR lease_expires_at<=statement_timestamp()) ORDER BY next_probe_at,id FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE file_storage_cleanup_watches w SET lease_owner=$2,lease_generation=lease_generation+1,leased_at=statement_timestamp(),lease_expires_at=statement_timestamp()+($3*interval '1 microsecond'),next_probe_at=GREATEST(w.next_probe_at,statement_timestamp())+($3*interval '1 microsecond') FROM due WHERE w.id=due.id RETURNING w.id,w.file_id,w.storage_class,w.object_key,w.content_type,w.namespace_fingerprint,w.lease_owner,w.lease_generation")
            .bind(allowed).bind(worker).bind(lease_us).fetch_optional().await.map_err(|_|RepositoryError::OperationFailed)?;
        row.map(|row| {
            let class = parse_class(&row.storage_class)?;
            Ok(LeasedCleanupWatch {
                id: row.id,
                file_id: row.file_id,
                object: StoredObject::new(
                    persisted_object_key(row.object_key, class)
                        .map_err(|_| RepositoryError::InvalidPersistedState)?,
                    row.content_type,
                ),
                namespace: StorageNamespace::parse(row.namespace_fingerprint)
                    .map_err(|_| RepositoryError::InvalidPersistedState)?,
                claim: CleanupClaim {
                    owner: row.lease_owner,
                    generation: row.lease_generation,
                },
            })
        })
        .transpose()
    }

    pub async fn finish_cleanup_watch(
        &self,
        work: &LeasedCleanupWatch,
        observation: CleanupObservation,
        delay: Duration,
    ) -> Result<(), RepositoryError> {
        let delay_us =
            i64::try_from(delay.as_micros()).map_err(|_| RepositoryError::InvalidPersistedState)?;
        if delay_us <= 0 {
            return Err(RepositoryError::InvalidPersistedState);
        }
        let error = match observation {
            CleanupObservation::Failed { error_code } => Some(error_code),
            _ => None,
        };
        let mut transaction =
            school_tenancy::TenantPool::begin_observed(self.pool(), "file_watch.finish")
                .await
                .map_err(|_| RepositoryError::OperationFailed)?;
        DbSession::new(&mut *transaction)
            .query(
                "file_watch.lock",
                "SELECT id FROM file_storage_cleanup_watches WHERE id=$1 FOR UPDATE",
            )
            .bind(work.id)
            .execute()
            .await
            .map_err(|_| RepositoryError::OperationFailed)?;
        // A new statement evaluates wall time AFTER waiting for a row lock.
        let changed=DbSession::new(&mut *transaction).query("file_watch.finish", "UPDATE file_storage_cleanup_watches SET last_probe_at=statement_timestamp(),last_absent_at=CASE WHEN $5 THEN statement_timestamp() ELSE last_absent_at END,last_error_code=$6,next_probe_at=statement_timestamp()+($7*interval '1 microsecond'),lease_owner=NULL,leased_at=NULL,lease_expires_at=NULL WHERE id=$1 AND lease_owner=$2 AND lease_generation=$3 AND lease_expires_at>statement_timestamp() AND revoked_at IS NOT NULL AND namespace_fingerprint=$4")
            .bind(work.id).bind(&work.claim.owner).bind(work.claim.generation).bind(work.namespace.fingerprint())
            .bind(matches!(observation,CleanupObservation::Absent)).bind(error).bind(delay_us)
            .execute().await.map_err(|_|RepositoryError::OperationFailed)?.rows_affected();
        if changed != 1 {
            return Err(RepositoryError::LeaseLost);
        }
        transaction
            .commit()
            .await
            .map_err(|_| RepositoryError::OperationFailed)
    }

    /// Recheck expiry after locking; a completed domain promotion wins over stale discovery.
    pub async fn request_expired_delete(&self, file_id: Uuid) -> Result<bool, RepositoryError> {
        let mut transaction =
            school_tenancy::TenantPool::begin_observed(self.pool(), "file_platform.expired_delete")
                .await
                .map_err(|_| RepositoryError::OperationFailed)?;
        DbSession::new(&mut *transaction)
            .query(
                "file_platform.expired_delete_lock",
                "SELECT id FROM files WHERE id=$1 FOR UPDATE",
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(|_| RepositoryError::OperationFailed)?;
        let expired=DbSession::new(&mut *transaction).scalar::<bool>("file_platform.expired_delete_check", "SELECT EXISTS(SELECT 1 FROM files WHERE id=$1 AND retention_class='temporary' AND expires_at<=statement_timestamp() AND deleted_at IS NULL AND lifecycle_status NOT IN ('delete_requested','deleted'))")
            .bind(file_id).fetch_one().await.map_err(|_|RepositoryError::OperationFailed)?;
        if expired {
            self.request_delete_in_transaction(&mut transaction, file_id)
                .await?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| RepositoryError::OperationFailed)?;
        Ok(expired)
    }
}

pub async fn perform_cleanup_watch(
    platform: &super::platform_service::FilePlatform,
    work: &LeasedCleanupWatch,
    timeout: Duration,
) -> CleanupObservation {
    let provider = platform.provider().as_ref();
    let result = tokio::time::timeout(timeout, async {
        verify_namespace(provider, &work.object, Some(&work.namespace))?;
        if provider.head(&work.object).await?.is_none() {
            return Ok(CleanupObservation::Absent);
        }
        provider.delete(&work.object).await?;
        Ok::<_, StorageError>(CleanupObservation::Deleted)
    })
    .await;
    match result {
        Ok(Ok(observation)) => observation,
        Ok(Err(error)) => CleanupObservation::Failed {
            error_code: error.log_safe_code(),
        },
        Err(_) => CleanupObservation::Failed {
            error_code: "storage_cleanup_deadline",
        },
    }
}

pub(crate) const fn class_code(class: StorageClass) -> &'static str {
    match class {
        StorageClass::Public => "public",
        StorageClass::Private => "private",
    }
}
fn parse_class(value: &str) -> Result<StorageClass, RepositoryError> {
    match value {
        "public" => Ok(StorageClass::Public),
        "private" => Ok(StorageClass::Private),
        _ => Err(RepositoryError::InvalidPersistedState),
    }
}

pub(crate) async fn authorize_worker_write(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operation_id: Uuid,
    file_id: Uuid,
    derivative_id: Uuid,
    claim: &super::repository::OperationClaim,
    object: &StoredObject,
) -> Result<WritePermit, RepositoryError> {
    // Preserve file -> operation -> physical target order. Separate statements
    // validate claim wall time after every potentially blocking row lock.
    DbSession::new(&mut **transaction)
        .query(
            "file_watch.write_file_lock",
            "SELECT id FROM files WHERE id=$1 FOR UPDATE",
        )
        .bind(file_id)
        .execute()
        .await
        .map_err(|_| RepositoryError::OperationFailed)?;
    DbSession::new(&mut **transaction)
        .query(
            "file_watch.write_operation_lock",
            "SELECT id FROM file_operations WHERE id=$1 FOR UPDATE",
        )
        .bind(operation_id)
        .execute()
        .await
        .map_err(|_| RepositoryError::OperationFailed)?;
    DbSession::new(&mut **transaction).query("file_watch.write_target_lock","SELECT id FROM file_storage_cleanup_watches WHERE target_kind='derivative' AND target_id=$1 FOR UPDATE").bind(derivative_id).execute().await.map_err(|_|RepositoryError::OperationFailed)?;
    let result=DbSession::new(&mut **transaction).query_as::<(Uuid,String)>("file_watch.write_permission", "UPDATE file_storage_cleanup_watches w SET write_generation=write_generation+1 WHERE target_kind='derivative' AND target_id=$1 AND file_id=$2 AND revoked_at IS NULL AND namespace_fingerprint IS NOT NULL AND object_key=$6 AND storage_class=$7 AND EXISTS(SELECT 1 FROM file_operations o WHERE o.id=$3 AND o.file_id=$2 AND o.file_derivative_id=$1 AND o.operation_type='generate_derivative' AND o.status='leased' AND o.lease_owner=$4 AND o.attempt_count=$5 AND o.lease_expires_at>statement_timestamp()) RETURNING w.id,w.namespace_fingerprint")
        .bind(derivative_id).bind(file_id).bind(operation_id).bind(&claim.owner).bind(claim.attempt_count).bind(object.object_key.as_str()).bind(class_code(object.storage_class()))
        .fetch_optional().await.map_err(|_|RepositoryError::OperationFailed)?.ok_or(RepositoryError::LeaseLost)?;
    Ok(WritePermit {
        _watch_id: result.0,
        object: object.clone(),
        namespace: StorageNamespace::parse(result.1)
            .map_err(|_| RepositoryError::InvalidPersistedState)?,
    })
}
