use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{fmt, time::Duration};
use uuid::Uuid;

use super::{
    platform_types::{
        DerivativeRecipe, FileInspectionMetadata, FileLifecycleStatus, FilePurpose, FileVisibility,
        RetentionClass, StorageClass,
    },
    purpose_registry::{persisted_object_key, purpose_from_code},
    storage_provider::StoredObject,
};

pub const STORAGE_PROVIDER_CODE: &str = "r2";

const LOAD_DELIVERY_SQL: &str = r#"
SELECT f.id, f.owner_user_id, f.purpose_code, f.visibility, f.lifecycle_status,
       f.display_filename, f.current_version_id,
       current_version.version_number, current_version.object_key,
       current_version.storage_class, metadata.detected_mime_type,
       metadata.byte_size
FROM files f
LEFT JOIN file_versions current_version
  ON current_version.id = f.current_version_id
 AND current_version.file_id = f.id
LEFT JOIN LATERAL (
    SELECT candidate.detected_mime_type, candidate.byte_size
    FROM file_versions candidate
    WHERE candidate.file_id = f.id
    ORDER BY
        CASE WHEN candidate.id = f.current_version_id THEN 0 ELSE 1 END,
        candidate.version_number DESC
    LIMIT 1
) metadata ON true
WHERE f.id = $1 AND f.deleted_at IS NULL
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryError {
    OperationFailed,
    InvalidPersistedState,
    MaterializationRevoked,
    LeaseLost,
}

impl RepositoryError {
    pub const fn log_safe_code(self) -> &'static str {
        match self {
            Self::OperationFailed => "file_repository_operation_failed",
            Self::InvalidPersistedState => "file_repository_state_invalid",
            Self::MaterializationRevoked => "file_materialization_revoked",
            Self::LeaseLost => "file_operation_lease_lost",
        }
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.log_safe_code())
    }
}

impl std::error::Error for RepositoryError {}

#[derive(Clone)]
pub struct NewDerivative {
    pub id: Uuid,
    pub operation_id: Uuid,
    pub recipe: DerivativeRecipe,
    pub object: StoredObject,
    pub byte_size: i64,
    pub checksum: String,
    pub required: bool,
}

#[derive(Clone)]
pub struct NewUpload {
    pub file_id: Uuid,
    pub version_id: Uuid,
    pub reconcile_operation_id: Uuid,
    pub scan_operation_id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub purpose: FilePurpose,
    pub visibility: FileVisibility,
    pub retention_class: RetentionClass,
    pub display_filename: String,
    pub original: StoredObject,
    pub byte_size: i64,
    pub checksum: String,
    pub inspection_metadata: FileInspectionMetadata,
    pub derivatives: Vec<NewDerivative>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformFile {
    pub id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub purpose: FilePurpose,
    pub visibility: FileVisibility,
    pub lifecycle_status: FileLifecycleStatus,
    pub current_version: Option<u32>,
    pub display_filename: String,
    pub detected_mime_type: String,
    pub byte_size: i64,
}

#[derive(Clone)]
pub struct DeliveryRecord {
    pub file: PlatformFile,
    pub object: Option<StoredObject>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectTarget {
    Version(Uuid),
    Derivative(Uuid),
}

#[derive(Clone)]
pub struct DeleteWork {
    pub operation_id: Uuid,
    pub file_id: Uuid,
    pub target: ObjectTarget,
    pub object: StoredObject,
}

#[derive(Clone)]
pub struct DerivativeWork {
    pub derivative_id: Uuid,
    pub purpose: FilePurpose,
    pub recipe: DerivativeRecipe,
    pub required: bool,
    pub source: StoredObject,
    pub target: StoredObject,
    pub expected_byte_size: i64,
    pub expected_checksum: String,
}

#[derive(Clone)]
pub enum OperationWork {
    ReconcileUpload {
        version_id: Uuid,
        original: StoredObject,
        required_derivative_ids: Vec<Uuid>,
        required_derivatives: Vec<StoredObject>,
    },
    GenerateDerivative(DerivativeWork),
    DeleteObject(DeleteWork),
}

/// Owner plus the monotonically increasing attempt fences every reclaim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationClaim {
    pub owner: String,
    pub attempt_count: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeasedOperationResult {
    Succeeded,
    OriginalMissing,
    /// Provider PUT began, so bytes may exist even when its response was lost.
    DerivativeWriteUncertain {
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    },
    Failed {
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    },
}

#[derive(Clone)]
pub struct LeasedOperation {
    pub claim: OperationClaim,
    pub id: Uuid,
    pub file_id: Uuid,
    pub attempt_count: i32,
    pub work: OperationWork,
}

#[async_trait]
pub trait FileRepository: Send + Sync {
    async fn reserve_upload(&self, upload: &NewUpload) -> Result<(), RepositoryError>;
    async fn mark_original_stored(
        &self,
        file_id: Uuid,
        version_id: Uuid,
    ) -> Result<(), RepositoryError>;
    async fn mark_upload_failed(
        &self,
        file_id: Uuid,
        version_id: Uuid,
        error_code: &'static str,
    ) -> Result<(), RepositoryError>;
    async fn mark_derivative_stored(
        &self,
        file_id: Uuid,
        derivative_id: Uuid,
        operation_id: Uuid,
    ) -> Result<(), RepositoryError>;
    async fn mark_derivative_failed(
        &self,
        file_id: Uuid,
        derivative_id: Uuid,
        operation_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    ) -> Result<(), RepositoryError>;
    async fn finalize_ready(
        &self,
        file_id: Uuid,
        version_id: Uuid,
        required_derivative_ids: &[Uuid],
    ) -> Result<(), RepositoryError>;
    async fn mark_reconcile_pending(
        &self,
        file_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
    ) -> Result<(), RepositoryError>;
    async fn load_delivery(&self, file_id: Uuid)
        -> Result<Option<DeliveryRecord>, RepositoryError>;
    async fn request_delete(&self, file_id: Uuid) -> Result<Vec<DeleteWork>, RepositoryError>;
    async fn mark_delete_succeeded(&self, work: &DeleteWork) -> Result<(), RepositoryError>;
    async fn lease_due_operations(
        &self,
        worker: &str,
        lease_duration: Duration,
        limit: i64,
    ) -> Result<Vec<LeasedOperation>, RepositoryError>;
    /// Worker-only completion; validates the exact live claim before any metadata effect.
    async fn finish_leased_operation(
        &self,
        operation: &LeasedOperation,
        result: LeasedOperationResult,
    ) -> Result<(), RepositoryError>;
    async fn retry_operation(
        &self,
        operation_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    ) -> Result<(), RepositoryError>;
    async fn queue_delete_retry(
        &self,
        file_id: Uuid,
        target: ObjectTarget,
        error_code: &'static str,
        retry_delay: Duration,
    ) -> Result<(), RepositoryError>;
}

#[derive(Clone)]
pub struct SqlFileRepository {
    pool: PgPool,
}

impl SqlFileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn load_delivery_in_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        file_id: Uuid,
    ) -> Result<Option<DeliveryRecord>, RepositoryError> {
        let row = school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.load_delivery_in_transaction",
                LOAD_DELIVERY_SQL,
            )
            .bind(file_id)
            .fetch_optional()
            .await
            .map_err(Self::database_error)?;
        row.map(delivery_from_row).transpose()
    }

    fn database_error(_: sqlx::Error) -> RepositoryError {
        RepositoryError::OperationFailed
    }

    async fn load_operation(
        connection: &mut sqlx::PgConnection,
        operation_id: Uuid,
        claim: OperationClaim,
    ) -> Result<LeasedOperation, RepositoryError> {
        let operation = school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.load_operation",
                r#"
SELECT id, file_id, file_version_id, file_derivative_id, operation_type, attempt_count
FROM file_operations
WHERE id = $1 AND status = 'leased' AND lease_owner = $2
  AND attempt_count = $3 AND lease_expires_at > statement_timestamp()
"#,
            )
            .bind(operation_id)
            .bind(&claim.owner)
            .bind(claim.attempt_count)
            .fetch_optional()
            .await
            .map_err(Self::database_error)?
            .ok_or(RepositoryError::LeaseLost)?;

        let file_id: Uuid = operation.try_get("file_id").map_err(Self::database_error)?;
        let operation_type: String = operation
            .try_get("operation_type")
            .map_err(Self::database_error)?;
        let attempt_count: i32 = operation
            .try_get("attempt_count")
            .map_err(Self::database_error)?;

        let work = match operation_type.as_str() {
            "reconcile" => {
                let version_id: Uuid = operation
                    .try_get("file_version_id")
                    .map_err(Self::database_error)?;
                Self::load_reconcile_work(&mut *connection, file_id, version_id).await?
            }
            "generate_derivative" => {
                let derivative_id: Uuid = operation
                    .try_get("file_derivative_id")
                    .map_err(Self::database_error)?;
                OperationWork::GenerateDerivative(
                    Self::load_derivative_work(&mut *connection, file_id, derivative_id).await?,
                )
            }
            "delete_object" => {
                let version_id: Option<Uuid> = operation
                    .try_get("file_version_id")
                    .map_err(Self::database_error)?;
                let derivative_id: Option<Uuid> = operation
                    .try_get("file_derivative_id")
                    .map_err(Self::database_error)?;
                OperationWork::DeleteObject(
                    Self::load_delete_work(
                        &mut *connection,
                        operation_id,
                        file_id,
                        version_id,
                        derivative_id,
                    )
                    .await?,
                )
            }
            _ => return Err(RepositoryError::InvalidPersistedState),
        };

        Ok(LeasedOperation {
            claim,
            id: operation_id,
            file_id,
            attempt_count,
            work,
        })
    }

    async fn load_reconcile_work(
        connection: &mut sqlx::PgConnection,
        file_id: Uuid,
        version_id: Uuid,
    ) -> Result<OperationWork, RepositoryError> {
        let row = school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.load_reconcile_work",
                r#"
SELECT f.purpose_code, f.visibility,
       v.object_key, v.storage_class, v.detected_mime_type
FROM files f
JOIN file_versions v ON v.id = $2 AND v.file_id = f.id
WHERE f.id = $1
"#,
            )
            .bind(file_id)
            .bind(version_id)
            .fetch_optional()
            .await
            .map_err(Self::database_error)?
            .ok_or(RepositoryError::InvalidPersistedState)?;
        let original = stored_object_from_row(&row)?;
        let purpose = purpose_from_code(
            row.try_get::<String, _>("purpose_code")
                .map_err(Self::database_error)?
                .as_str(),
        )
        .map_err(|_| RepositoryError::InvalidPersistedState)?;

        let derivative_rows = school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.load_reconcile_work",
                r#"
SELECT id, derivative_kind, object_key, storage_class, detected_mime_type
FROM file_derivatives
WHERE file_id = $1 AND source_version_id = $2 AND deleted_at IS NULL
ORDER BY derivative_kind
"#,
            )
            .bind(file_id)
            .bind(version_id)
            .fetch_all()
            .await
            .map_err(Self::database_error)?;

        let mut required_derivative_ids = Vec::new();
        let mut required_derivatives = Vec::new();
        for derivative in derivative_rows {
            let recipe = recipe_from_kind(
                derivative
                    .try_get::<String, _>("derivative_kind")
                    .map_err(Self::database_error)?
                    .as_str(),
            )?;
            if derivative_is_required(purpose, recipe) {
                required_derivative_ids
                    .push(derivative.try_get("id").map_err(Self::database_error)?);
                required_derivatives.push(stored_object_from_row(&derivative)?);
            }
        }

        Ok(OperationWork::ReconcileUpload {
            version_id,
            original,
            required_derivative_ids,
            required_derivatives,
        })
    }

    async fn load_derivative_work(
        connection: &mut sqlx::PgConnection,
        file_id: Uuid,
        derivative_id: Uuid,
    ) -> Result<DerivativeWork, RepositoryError> {
        let row = school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.load_derivative_work",
                r#"
SELECT f.purpose_code,
       d.derivative_kind,
       d.object_key AS target_object_key,
       d.storage_class AS target_storage_class,
       d.detected_mime_type AS target_detected_mime_type,
       d.byte_size AS target_byte_size,
       d.checksum AS target_checksum,
       v.object_key AS source_object_key,
       v.storage_class AS source_storage_class,
       v.detected_mime_type AS source_detected_mime_type
FROM file_derivatives d
JOIN files f ON f.id = d.file_id
JOIN file_versions v ON v.id = d.source_version_id AND v.file_id = d.file_id
WHERE d.id = $2 AND d.file_id = $1
"#,
            )
            .bind(file_id)
            .bind(derivative_id)
            .fetch_optional()
            .await
            .map_err(Self::database_error)?
            .ok_or(RepositoryError::InvalidPersistedState)?;

        let purpose = purpose_from_code(
            row.try_get::<String, _>("purpose_code")
                .map_err(Self::database_error)?
                .as_str(),
        )
        .map_err(|_| RepositoryError::InvalidPersistedState)?;
        let recipe = recipe_from_kind(
            row.try_get::<String, _>("derivative_kind")
                .map_err(Self::database_error)?
                .as_str(),
        )?;

        Ok(DerivativeWork {
            derivative_id,
            purpose,
            recipe,
            required: derivative_is_required(purpose, recipe),
            source: stored_object_from_aliased_row(&row, "source")?,
            target: stored_object_from_aliased_row(&row, "target")?,
            expected_byte_size: row
                .try_get("target_byte_size")
                .map_err(Self::database_error)?,
            expected_checksum: row
                .try_get("target_checksum")
                .map_err(Self::database_error)?,
        })
    }

    async fn load_delete_work(
        connection: &mut sqlx::PgConnection,
        operation_id: Uuid,
        file_id: Uuid,
        version_id: Option<Uuid>,
        derivative_id: Option<Uuid>,
    ) -> Result<DeleteWork, RepositoryError> {
        match (version_id, derivative_id) {
            (Some(version_id), None) => {
                let row = school_database::DbSession::new(&mut *connection).query("file-platform.src.repository.load_delete_work",
                    "SELECT object_key, storage_class, detected_mime_type FROM file_versions WHERE id = $1 AND file_id = $2",
                )
                .bind(version_id)
                .bind(file_id)
                .fetch_optional()
                .await
                .map_err(Self::database_error)?
                .ok_or(RepositoryError::InvalidPersistedState)?;
                Ok(DeleteWork {
                    operation_id,
                    file_id,
                    target: ObjectTarget::Version(version_id),
                    object: stored_object_from_row(&row)?,
                })
            }
            (None, Some(derivative_id)) => {
                let row = school_database::DbSession::new(&mut *connection).query("file-platform.src.repository.load_delete_work",
                    "SELECT object_key, storage_class, detected_mime_type FROM file_derivatives WHERE id = $1 AND file_id = $2",
                )
                .bind(derivative_id)
                .bind(file_id)
                .fetch_optional()
                .await
                .map_err(Self::database_error)?
                .ok_or(RepositoryError::InvalidPersistedState)?;
                Ok(DeleteWork {
                    operation_id,
                    file_id,
                    target: ObjectTarget::Derivative(derivative_id),
                    object: stored_object_from_row(&row)?,
                })
            }
            _ => Err(RepositoryError::InvalidPersistedState),
        }
    }

    /// Performs only the durable lifecycle transition using a caller-owned
    /// transaction. Domain services can keep their relationship locks and this
    /// transition on one connection, then run provider cleanup after commit.
    pub async fn request_delete_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        file_id: Uuid,
    ) -> Result<Vec<DeleteWork>, RepositoryError> {
        let lifecycle = school_database::DbSession::new(&mut **transaction)
            .scalar::<String>(
                "file-platform.src.repository.request_delete_in_transaction",
                "SELECT lifecycle_status FROM files WHERE id = $1 FOR UPDATE",
            )
            .bind(file_id)
            .fetch_optional()
            .await
            .map_err(Self::database_error)?;
        let Some(lifecycle) = lifecycle else {
            return Ok(Vec::new());
        };
        if lifecycle == "deleted" {
            return Ok(Vec::new());
        }

        school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
UPDATE files
SET lifecycle_status = 'delete_requested',
    delete_requested_at = COALESCE(delete_requested_at, now()),
    updated_at = now()
WHERE id = $1 AND lifecycle_status <> 'deleted'
"#,
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;

        school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
UPDATE file_operations
SET status = 'cancelled',
    last_error_code = 'file_delete_requested',
    completed_at = now(),
    lease_owner = NULL,
    leased_at = NULL,
    lease_expires_at = NULL
WHERE file_id = $1
  AND operation_type IN ('reconcile', 'generate_derivative')
  AND status IN ('pending', 'leased', 'retryable_failure')
"#,
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;
        school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
UPDATE file_versions
SET storage_status = 'delete_requested'
WHERE file_id = $1 AND storage_status <> 'deleted'
"#,
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;
        school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
UPDATE file_derivatives
SET storage_status = 'delete_requested'
WHERE file_id = $1 AND storage_status <> 'deleted'
"#,
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;

        school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
INSERT INTO file_operations (
    file_id, file_version_id, operation_type, status, next_retry_at
)
SELECT v.file_id, v.id, 'delete_object', 'pending', now()
FROM file_versions v
WHERE v.file_id = $1 AND v.storage_status <> 'deleted'
  AND NOT EXISTS (
      SELECT 1 FROM file_operations o
      WHERE o.file_version_id = v.id AND o.operation_type = 'delete_object'
        AND o.status IN ('pending', 'leased', 'retryable_failure', 'succeeded')
  )
"#,
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;
        school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
INSERT INTO file_operations (
    file_id, file_derivative_id, operation_type, status, next_retry_at
)
SELECT d.file_id, d.id, 'delete_object', 'pending', now()
FROM file_derivatives d
WHERE d.file_id = $1 AND d.storage_status <> 'deleted'
  AND NOT EXISTS (
      SELECT 1 FROM file_operations o
      WHERE o.file_derivative_id = d.id AND o.operation_type = 'delete_object'
        AND o.status IN ('pending', 'leased', 'retryable_failure', 'succeeded')
  )
"#,
            )
            .bind(file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;

        // A temporary upload can fail before its first physical version is
        // persisted. Complete that logical deletion in the same caller-owned
        // transaction instead of leaving it stuck at delete_requested forever.
        finalize_deleted_if_absent(transaction, file_id).await?;

        let rows = school_database::DbSession::new(&mut **transaction)
            .query(
                "file-platform.src.repository.request_delete_in_transaction",
                r#"
SELECT operation.id,
       operation.file_version_id,
       operation.file_derivative_id,
       COALESCE(version.object_key, derivative.object_key) AS object_key,
       COALESCE(version.storage_class, derivative.storage_class) AS storage_class,
       COALESCE(version.detected_mime_type, derivative.detected_mime_type) AS detected_mime_type
FROM file_operations operation
LEFT JOIN file_versions version
  ON version.id = operation.file_version_id AND version.file_id = operation.file_id
LEFT JOIN file_derivatives derivative
  ON derivative.id = operation.file_derivative_id AND derivative.file_id = operation.file_id
WHERE operation.file_id = $1 AND operation.operation_type = 'delete_object'
  AND operation.status IN ('pending', 'retryable_failure')
ORDER BY operation.created_at, operation.id
"#,
            )
            .bind(file_id)
            .fetch_all()
            .await
            .map_err(Self::database_error)?;

        rows.into_iter()
            .map(|row| {
                let operation_id = row.try_get("id").map_err(Self::database_error)?;
                let version_id = row
                    .try_get("file_version_id")
                    .map_err(Self::database_error)?;
                let derivative_id = row
                    .try_get("file_derivative_id")
                    .map_err(Self::database_error)?;
                let target = match (version_id, derivative_id) {
                    (Some(id), None) => ObjectTarget::Version(id),
                    (None, Some(id)) => ObjectTarget::Derivative(id),
                    _ => return Err(RepositoryError::InvalidPersistedState),
                };
                Ok(DeleteWork {
                    operation_id,
                    file_id,
                    target,
                    object: stored_object_from_row(&row)?,
                })
            })
            .collect()
    }
}

fn duration_microseconds(duration: Duration) -> Result<i64, RepositoryError> {
    i64::try_from(duration.as_micros()).map_err(|_| RepositoryError::InvalidPersistedState)
}

#[async_trait]
impl FileRepository for SqlFileRepository {
    async fn reserve_upload(&self, upload: &NewUpload) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        let visibility = visibility_code(upload.visibility);
        let retention = retention_code(upload.retention_class);

        school_database::DbSession::new(&mut *transaction)
            .query(
                "file-platform.src.repository.reserve_upload",
                r#"
INSERT INTO files (
    id, owner_user_id, display_filename, created_by, purpose_code,
    visibility, lifecycle_status, retention_class, expires_at, inspection_metadata
) VALUES (
    $1, $2, $3, $4, $5, $6, 'processing', $7,
    CASE WHEN $7 = 'temporary' THEN now() + INTERVAL '24 hours' ELSE NULL END,
    $8
)
"#,
            )
            .bind(upload.file_id)
            .bind(upload.owner_user_id)
            .bind(&upload.display_filename)
            .bind(upload.created_by)
            .bind(upload.purpose.code())
            .bind(visibility)
            .bind(retention)
            .bind(sqlx::types::Json(&upload.inspection_metadata))
            .execute()
            .await
            .map_err(Self::database_error)?;

        school_database::DbSession::new(&mut *transaction)
            .query(
                "file-platform.src.repository.reserve_upload",
                r#"
INSERT INTO file_versions (
    id, file_id, version_number, provider_code, storage_class,
    storage_status, object_key, detected_mime_type, canonical_extension,
    byte_size, checksum, scan_status, scanner_result_code, scanned_at, created_by
) VALUES (
    $1, $2, 1, $3, $4, 'pending', $5, $6, $7,
    $8, $9, 'clean', 'clean', now(), $10
)
"#,
            )
            .bind(upload.version_id)
            .bind(upload.file_id)
            .bind(STORAGE_PROVIDER_CODE)
            .bind(storage_class_code(upload.original.storage_class()))
            .bind(upload.original.object_key.as_str())
            .bind(&upload.original.content_type)
            .bind(canonical_extension_from_mime(
                &upload.original.content_type,
            )?)
            .bind(upload.byte_size)
            .bind(&upload.checksum)
            .bind(upload.created_by)
            .execute()
            .await
            .map_err(Self::database_error)?;

        school_database::DbSession::new(&mut *transaction)
            .query(
                "file-platform.src.repository.reserve_upload",
                r#"
INSERT INTO file_operations (
    id, file_id, file_version_id, operation_type, status,
    attempt_count, next_retry_at, started_at, completed_at
) VALUES ($1, $2, $3, 'scan', 'succeeded', 1, now(), now(), now())
"#,
            )
            .bind(upload.scan_operation_id)
            .bind(upload.file_id)
            .bind(upload.version_id)
            .execute()
            .await
            .map_err(Self::database_error)?;

        school_database::DbSession::new(&mut *transaction)
            .query(
                "file-platform.src.repository.reserve_upload",
                r#"
INSERT INTO file_operations (
    id, file_id, file_version_id, operation_type, status, next_retry_at
) VALUES ($1, $2, $3, 'reconcile', 'pending', now())
"#,
            )
            .bind(upload.reconcile_operation_id)
            .bind(upload.file_id)
            .bind(upload.version_id)
            .execute()
            .await
            .map_err(Self::database_error)?;

        for derivative in &upload.derivatives {
            school_database::DbSession::new(&mut *transaction)
                .query(
                    "file-platform.src.repository.reserve_upload",
                    r#"
INSERT INTO file_derivatives (
    id, file_id, source_version_id, derivative_kind, provider_code,
    storage_class, storage_status, object_key, detected_mime_type,
    canonical_extension, byte_size, checksum, lifecycle_status
) VALUES (
    $1, $2, $3, $4, $5, $6, 'pending', $7, $8, $9, $10, $11, 'processing'
)
"#,
                )
                .bind(derivative.id)
                .bind(upload.file_id)
                .bind(upload.version_id)
                .bind(derivative.recipe.variant())
                .bind(STORAGE_PROVIDER_CODE)
                .bind(storage_class_code(derivative.object.storage_class()))
                .bind(derivative.object.object_key.as_str())
                .bind(&derivative.object.content_type)
                .bind(canonical_extension_from_mime(
                    &derivative.object.content_type,
                )?)
                .bind(derivative.byte_size)
                .bind(&derivative.checksum)
                .execute()
                .await
                .map_err(Self::database_error)?;

            school_database::DbSession::new(&mut *transaction)
                .query(
                    "file-platform.src.repository.reserve_upload",
                    r#"
INSERT INTO file_operations (
    id, file_id, file_derivative_id, operation_type, status, next_retry_at
) VALUES ($1, $2, $3, 'generate_derivative', 'pending', now())
"#,
                )
                .bind(derivative.operation_id)
                .bind(upload.file_id)
                .bind(derivative.id)
                .execute()
                .await
                .map_err(Self::database_error)?;
        }

        transaction.commit().await.map_err(Self::database_error)
    }

    async fn mark_original_stored(
        &self,
        file_id: Uuid,
        version_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            &self.pool,
            "file_platform.mark_original_stored",
        )
        .await
        .map_err(Self::database_error)?;
        let result = school_database::DbSession::new(&mut *connection).query("file-platform.src.repository.mark_original_stored",
            "UPDATE file_versions SET storage_status = 'stored' WHERE id = $1 AND file_id = $2 AND storage_status = 'pending'",
        )
        .bind(version_id)
        .bind(file_id)
        .execute()
        .await
        .map_err(Self::database_error)?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(RepositoryError::MaterializationRevoked)
        }
    }

    async fn mark_upload_failed(
        &self,
        file_id: Uuid,
        version_id: Uuid,
        error_code: &'static str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        lock_file_operations(&mut transaction, file_id, None, "reconcile").await?;
        mark_upload_failed_in_transaction(&mut transaction, file_id, version_id, error_code, None)
            .await?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn mark_derivative_stored(
        &self,
        file_id: Uuid,
        derivative_id: Uuid,
        operation_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        lock_file_operations(
            &mut transaction,
            file_id,
            Some(operation_id),
            "generate_derivative",
        )
        .await?;
        mark_derivative_stored_in_transaction(
            &mut transaction,
            file_id,
            derivative_id,
            operation_id,
        )
        .await?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn mark_derivative_failed(
        &self,
        file_id: Uuid,
        derivative_id: Uuid,
        operation_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    ) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        lock_file_operations(
            &mut transaction,
            file_id,
            Some(operation_id),
            "generate_derivative",
        )
        .await?;
        mark_derivative_failed_in_transaction(
            &mut transaction,
            file_id,
            derivative_id,
            operation_id,
            error_code,
            retry_delay,
            terminal,
        )
        .await?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn finalize_ready(
        &self,
        file_id: Uuid,
        version_id: Uuid,
        required_derivative_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        lock_file_operations(&mut transaction, file_id, None, "reconcile").await?;
        finalize_ready_in_transaction(
            &mut transaction,
            file_id,
            version_id,
            required_derivative_ids,
            None,
        )
        .await?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn mark_reconcile_pending(
        &self,
        file_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
    ) -> Result<(), RepositoryError> {
        let retry_microseconds = duration_microseconds(retry_delay)?;
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            &self.pool,
            "file_platform.mark_reconcile_pending",
        )
        .await
        .map_err(Self::database_error)?;
        school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.mark_reconcile_pending",
                r#"
UPDATE file_operations
SET status = 'retryable_failure', last_error_code = $2,
    next_retry_at = statement_timestamp() + ($3 * INTERVAL '1 microsecond'),
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE file_id = $1 AND operation_type = 'reconcile' AND status <> 'succeeded'
"#,
            )
            .bind(file_id)
            .bind(error_code)
            .bind(retry_microseconds)
            .execute()
            .await
            .map_err(Self::database_error)?;
        Ok(())
    }

    async fn load_delivery(
        &self,
        file_id: Uuid,
    ) -> Result<Option<DeliveryRecord>, RepositoryError> {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            &self.pool,
            "file_platform.load_delivery",
        )
        .await
        .map_err(Self::database_error)?;
        let row = school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.load_delivery",
                LOAD_DELIVERY_SQL,
            )
            .bind(file_id)
            .fetch_optional()
            .await
            .map_err(Self::database_error)?;
        row.map(delivery_from_row).transpose()
    }

    async fn request_delete(&self, file_id: Uuid) -> Result<Vec<DeleteWork>, RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        let work = self
            .request_delete_in_transaction(&mut transaction, file_id)
            .await?;
        transaction.commit().await.map_err(Self::database_error)?;
        Ok(work)
    }

    async fn mark_delete_succeeded(&self, work: &DeleteWork) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        lock_file_operations(
            &mut transaction,
            work.file_id,
            Some(work.operation_id),
            "delete_object",
        )
        .await?;
        mark_delete_succeeded_in_transaction(&mut transaction, work).await?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn lease_due_operations(
        &self,
        worker: &str,
        lease_duration: Duration,
        limit: i64,
    ) -> Result<Vec<LeasedOperation>, RepositoryError> {
        if worker.trim().is_empty() || !(1..=100).contains(&limit) {
            return Err(RepositoryError::InvalidPersistedState);
        }
        let lease_microseconds = duration_microseconds(lease_duration)?;
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            &self.pool,
            "file_platform.lease_due_operations",
        )
        .await
        .map_err(Self::database_error)?;
        let rows = school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.lease_due_operations",
                r#"
WITH due AS (
    SELECT id
    FROM file_operations
    WHERE (
        status IN ('pending', 'retryable_failure')
        AND next_retry_at <= statement_timestamp()
    ) OR (
        status = 'leased'
        AND lease_expires_at <= statement_timestamp()
    )
    ORDER BY next_retry_at, created_at, id
    FOR UPDATE SKIP LOCKED
    LIMIT $1
)
UPDATE file_operations o
SET status = 'leased', attempt_count = attempt_count + 1,
    lease_owner = $2,
    leased_at = statement_timestamp(),
    lease_expires_at = statement_timestamp()
        + ($3 * INTERVAL '1 microsecond'),
    started_at = COALESCE(started_at, statement_timestamp())
FROM due
WHERE o.id = due.id AND o.attempt_count < 100
RETURNING o.id, o.lease_owner, o.attempt_count
"#,
            )
            .bind(limit)
            .bind(worker)
            .bind(lease_microseconds)
            .fetch_all()
            .await
            .map_err(Self::database_error)?;

        let mut operations = Vec::with_capacity(rows.len());
        for row in rows {
            operations.push(
                Self::load_operation(
                    &mut *connection,
                    row.try_get("id").map_err(Self::database_error)?,
                    OperationClaim {
                        owner: row.try_get("lease_owner").map_err(Self::database_error)?,
                        attempt_count: row
                            .try_get("attempt_count")
                            .map_err(Self::database_error)?,
                    },
                )
                .await?,
            );
        }
        Ok(operations)
    }

    async fn finish_leased_operation(
        &self,
        operation: &LeasedOperation,
        result: LeasedOperationResult,
    ) -> Result<(), RepositoryError> {
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        // Deletion also locks file before operation; retain that lock order.
        school_database::DbSession::new(&mut *transaction)
            .query(
                "file-platform.src.repository.finish_leased_operation",
                "SELECT id FROM files WHERE id=$1 FOR UPDATE",
            )
            .bind(operation.file_id)
            .execute()
            .await
            .map_err(Self::database_error)?;
        school_database::DbSession::new(&mut *transaction)
            .query(
                "file-platform.src.repository.finish_leased_operation",
                "SELECT id FROM file_operations WHERE id=$1 FOR UPDATE",
            )
            .bind(operation.id)
            .execute()
            .await
            .map_err(Self::database_error)?;
        let (kind, version, derivative) = match &operation.work {
            OperationWork::ReconcileUpload { version_id, .. } => {
                ("reconcile", Some(*version_id), None)
            }
            OperationWork::GenerateDerivative(work) => {
                ("generate_derivative", None, Some(work.derivative_id))
            }
            OperationWork::DeleteObject(work) => {
                if work.file_id != operation.file_id || work.operation_id != operation.id {
                    return Err(RepositoryError::InvalidPersistedState);
                }
                match work.target {
                    ObjectTarget::Version(id) => ("delete_object", Some(id), None),
                    ObjectTarget::Derivative(id) => ("delete_object", None, Some(id)),
                }
            }
        };
        // A separate statement checks wall time after any row-lock wait.
        let live: bool = school_database::DbSession::new(&mut *transaction).scalar("file-platform.src.repository.finish_leased_operation", "SELECT EXISTS(SELECT 1 FROM file_operations WHERE id=$1 AND file_id=$2 AND status='leased' AND lease_owner=$3 AND attempt_count=$4 AND lease_expires_at>statement_timestamp() AND operation_type=$5 AND file_version_id IS NOT DISTINCT FROM $6 AND file_derivative_id IS NOT DISTINCT FROM $7)")
            .bind(operation.id).bind(operation.file_id).bind(&operation.claim.owner).bind(operation.claim.attempt_count)
            .bind(kind).bind(version).bind(derivative)
            .fetch_one().await.map_err(Self::database_error)?;
        if operation.attempt_count != operation.claim.attempt_count {
            return Err(RepositoryError::LeaseLost);
        }
        if !live {
            // A late PUT can recreate a derivative after deletion finished. Only
            // the exact cancelled generation may request a NEW deletion of its
            // immutable target. Reclaimed/expired claims cause no effects.
            if let (
                OperationWork::GenerateDerivative(work),
                LeasedOperationResult::Succeeded
                | LeasedOperationResult::DerivativeWriteUncertain { .. },
            ) = (&operation.work, result)
            {
                let cancelled: bool = school_database::DbSession::new(&mut *transaction).scalar("file-platform.src.repository.finish_leased_operation", "SELECT EXISTS(SELECT 1 FROM file_operations o JOIN files f ON f.id=o.file_id JOIN file_derivatives d ON d.id=o.file_derivative_id AND d.file_id=f.id WHERE o.id=$1 AND o.file_id=$2 AND o.attempt_count=$3 AND o.status='cancelled' AND o.last_error_code='file_delete_requested' AND o.operation_type='generate_derivative' AND d.id=$4 AND d.object_key=$5 AND f.lifecycle_status IN ('delete_requested','deleted'))")
                    .bind(operation.id).bind(operation.file_id).bind(operation.claim.attempt_count).bind(work.derivative_id).bind(work.target.object_key.as_str())
                    .fetch_one().await.map_err(Self::database_error)?;
                if cancelled {
                    school_database::DbSession::new(&mut *transaction).query("file-platform.src.repository.finish_leased_operation", "INSERT INTO file_operations(file_id,file_derivative_id,operation_type,status,next_retry_at,last_error_code) VALUES($1,$2,'delete_object','retryable_failure',statement_timestamp(),'file_delete_requested')")
                        .bind(operation.file_id).bind(work.derivative_id).execute().await.map_err(Self::database_error)?;
                    transaction.commit().await.map_err(Self::database_error)?;
                }
            }
            return Err(RepositoryError::LeaseLost);
        }
        match (&operation.work, result) {
            (
                OperationWork::ReconcileUpload {
                    version_id,
                    required_derivative_ids,
                    ..
                },
                LeasedOperationResult::Succeeded,
            ) => {
                finalize_ready_in_transaction(
                    &mut transaction,
                    operation.file_id,
                    *version_id,
                    required_derivative_ids,
                    Some(operation.id),
                )
                .await?;
            }
            (
                OperationWork::ReconcileUpload { version_id, .. },
                LeasedOperationResult::OriginalMissing,
            ) => {
                mark_upload_failed_in_transaction(
                    &mut transaction,
                    operation.file_id,
                    *version_id,
                    "file_original_missing",
                    Some(operation.id),
                )
                .await?;
            }
            (OperationWork::GenerateDerivative(work), LeasedOperationResult::Succeeded) => {
                mark_derivative_stored_in_transaction(
                    &mut transaction,
                    operation.file_id,
                    work.derivative_id,
                    operation.id,
                )
                .await?;
            }
            (OperationWork::DeleteObject(work), LeasedOperationResult::Succeeded) => {
                mark_delete_succeeded_in_transaction(&mut transaction, work).await?;
            }
            (
                OperationWork::GenerateDerivative(work),
                LeasedOperationResult::Failed {
                    error_code,
                    retry_delay,
                    terminal,
                }
                | LeasedOperationResult::DerivativeWriteUncertain {
                    error_code,
                    retry_delay,
                    terminal,
                },
            ) => {
                mark_derivative_failed_in_transaction(
                    &mut transaction,
                    operation.file_id,
                    work.derivative_id,
                    operation.id,
                    error_code,
                    retry_delay,
                    terminal,
                )
                .await?;
            }
            (
                _,
                LeasedOperationResult::Failed {
                    error_code,
                    retry_delay,
                    terminal,
                },
            ) => {
                retry_operation_query(
                    &mut transaction,
                    operation.id,
                    error_code,
                    duration_microseconds(retry_delay)?,
                    terminal,
                )
                .await?;
            }
            _ => return Err(RepositoryError::InvalidPersistedState),
        }
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn retry_operation(
        &self,
        operation_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    ) -> Result<(), RepositoryError> {
        let retry_microseconds = duration_microseconds(retry_delay)?;
        let mut transaction = school_tenancy::TenantPool::begin_observed(
            &self.pool,
            "file_platform.repository.transaction",
        )
        .await
        .map_err(Self::database_error)?;
        retry_operation_query(
            &mut transaction,
            operation_id,
            error_code,
            retry_microseconds,
            terminal,
        )
        .await?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn queue_delete_retry(
        &self,
        file_id: Uuid,
        target: ObjectTarget,
        error_code: &'static str,
        retry_delay: Duration,
    ) -> Result<(), RepositoryError> {
        let retry_microseconds = duration_microseconds(retry_delay)?;
        let (version_id, derivative_id) = match target {
            ObjectTarget::Version(version_id) => (Some(version_id), None),
            ObjectTarget::Derivative(derivative_id) => (None, Some(derivative_id)),
        };
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            &self.pool,
            "file_platform.queue_delete_retry",
        )
        .await
        .map_err(Self::database_error)?;
        school_database::DbSession::new(&mut *connection)
            .query(
                "file-platform.src.repository.queue_delete_retry",
                r#"
INSERT INTO file_operations (
    file_id, file_version_id, file_derivative_id, operation_type,
    status, next_retry_at, last_error_code
)
VALUES (
    $1, $2, $3, 'delete_object', 'retryable_failure',
    statement_timestamp() + ($4 * INTERVAL '1 microsecond'), $5
)
"#,
            )
            .bind(file_id)
            .bind(version_id)
            .bind(derivative_id)
            .bind(retry_microseconds)
            .bind(error_code)
            .execute()
            .await
            .map_err(Self::database_error)?;
        Ok(())
    }
}

/// Synchronous provider completions use the same order as worker completion and
/// deletion: file, operation, then physical target metadata.
async fn lock_file_operations(
    transaction: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
    operation_id: Option<Uuid>,
    kind: &str,
) -> Result<(), RepositoryError> {
    school_database::DbSession::new(&mut **transaction)
        .query(
            "file-platform.src.repository.lock_file_operations",
            "SELECT id FROM files WHERE id=$1 FOR UPDATE",
        )
        .bind(file_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.lock_file_operations", "SELECT id FROM file_operations WHERE file_id=$1 AND (($2::uuid IS NOT NULL AND id=$2) OR ($2::uuid IS NULL AND operation_type=$3)) ORDER BY id FOR UPDATE")
        .bind(file_id).bind(operation_id).bind(kind).execute().await.map_err(SqlFileRepository::database_error)?;
    Ok(())
}

async fn mark_upload_failed_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
    version_id: Uuid,
    error_code: &'static str,
    operation_id: Option<Uuid>,
) -> Result<(), RepositoryError> {
    school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.mark_upload_failed_in_transaction",
            "UPDATE file_versions SET storage_status = 'failed' WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
        )
        .bind(version_id)
        .bind(file_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.mark_upload_failed_in_transaction",
            "UPDATE files SET lifecycle_status = 'failed', updated_at = now() WHERE id = $1 AND lifecycle_status <> 'deleted'",
        )
        .bind(file_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.mark_upload_failed_in_transaction",
            r#"
UPDATE file_operations
SET status = 'failed', last_error_code = $2, completed_at = now(),
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE file_id = $1 AND operation_type = 'reconcile' AND status <> 'succeeded' AND ($3::uuid IS NULL OR id=$3)
"#,
        )
        .bind(file_id)
        .bind(error_code)
        .bind(operation_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    Ok(())
}

async fn mark_derivative_stored_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
    derivative_id: Uuid,
    operation_id: Uuid,
) -> Result<(), RepositoryError> {
    let result = school_database::DbSession::new(&mut **transaction)
        .query(
            "file-platform.src.repository.mark_derivative_stored_in_transaction",
            r#"
UPDATE file_derivatives d
SET storage_status = 'stored', lifecycle_status = 'ready'
FROM files f
WHERE d.id = $1
  AND d.file_id = $2
  AND f.id = d.file_id
  AND f.lifecycle_status IN ('processing', 'failed', 'ready')
  AND d.storage_status IN ('pending', 'failed', 'stored')
"#,
        )
        .bind(derivative_id)
        .bind(file_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    if result.rows_affected() != 1 {
        return Err(RepositoryError::MaterializationRevoked);
    }
    mark_operation_succeeded(transaction, operation_id).await?;
    Ok(())
}

async fn mark_derivative_failed_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
    derivative_id: Uuid,
    operation_id: Uuid,
    error_code: &'static str,
    retry_delay: Duration,
    terminal: bool,
) -> Result<(), RepositoryError> {
    let retry_microseconds = duration_microseconds(retry_delay)?;
    school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.mark_derivative_failed_in_transaction",
            "UPDATE file_derivatives SET storage_status = 'failed', lifecycle_status = 'failed' WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
        )
        .bind(derivative_id)
        .bind(file_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    retry_operation_query(
        transaction,
        operation_id,
        error_code,
        retry_microseconds,
        terminal,
    )
    .await?;
    Ok(())
}

async fn finalize_ready_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
    version_id: Uuid,
    required_derivative_ids: &[Uuid],
    operation_id: Option<Uuid>,
) -> Result<(), RepositoryError> {
    let original_ready = school_database::DbSession::new(&mut **transaction)
        .scalar::<bool>(
            "file-platform.src.repository.finalize_ready_in_transaction",
            r#"
SELECT EXISTS (
    SELECT 1 FROM file_versions
    WHERE id = $1 AND file_id = $2
      AND storage_status = 'stored' AND scan_status = 'clean' AND deleted_at IS NULL
)
"#,
        )
        .bind(version_id)
        .bind(file_id)
        .fetch_one()
        .await
        .map_err(SqlFileRepository::database_error)?;
    if !original_ready {
        return Err(RepositoryError::InvalidPersistedState);
    }

    if !required_derivative_ids.is_empty() {
        let ready_count = school_database::DbSession::new(&mut **transaction)
            .scalar::<i64>(
                "file-platform.src.repository.finalize_ready_in_transaction",
                r#"
SELECT COUNT(*)
FROM file_derivatives
WHERE file_id = $1 AND id = ANY($2)
  AND storage_status = 'stored' AND lifecycle_status = 'ready' AND deleted_at IS NULL
"#,
            )
            .bind(file_id)
            .bind(required_derivative_ids)
            .fetch_one()
            .await
            .map_err(SqlFileRepository::database_error)?;
        if ready_count != required_derivative_ids.len() as i64 {
            return Err(RepositoryError::InvalidPersistedState);
        }
    }

    let result = school_database::DbSession::new(&mut **transaction)
        .query(
            "file-platform.src.repository.finalize_ready_in_transaction",
            r#"
UPDATE files
SET current_version_id = $2, lifecycle_status = 'ready', updated_at = now()
WHERE id = $1 AND lifecycle_status IN ('processing', 'failed', 'ready')
"#,
        )
        .bind(file_id)
        .bind(version_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    if result.rows_affected() != 1 {
        return Err(RepositoryError::MaterializationRevoked);
    }
    school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.finalize_ready_in_transaction",
            r#"
UPDATE file_operations
SET status = 'succeeded', completed_at = now(), last_error_code = NULL,
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE file_id = $1 AND operation_type = 'reconcile' AND status <> 'succeeded' AND ($2::uuid IS NULL OR id=$2)
"#,
        )
        .bind(file_id)
        .bind(operation_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    Ok(())
}

async fn mark_delete_succeeded_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    work: &DeleteWork,
) -> Result<(), RepositoryError> {
    match work.target {
        ObjectTarget::Version(version_id) => {
            school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.mark_delete_succeeded_in_transaction",
                    "UPDATE file_versions SET storage_status = 'deleted', deleted_at = COALESCE(deleted_at, now()) WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
                )
                .bind(version_id)
                .bind(work.file_id)
                .execute()
                .await
                .map_err(SqlFileRepository::database_error)?;
        }
        ObjectTarget::Derivative(derivative_id) => {
            school_database::DbSession::new(&mut **transaction).query("file-platform.src.repository.mark_delete_succeeded_in_transaction",
                    "UPDATE file_derivatives SET storage_status = 'deleted', lifecycle_status = 'deleted', deleted_at = COALESCE(deleted_at, now()) WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
                )
                .bind(derivative_id)
                .bind(work.file_id)
                .execute()
                .await
                .map_err(SqlFileRepository::database_error)?;
        }
    }
    mark_operation_succeeded(transaction, work.operation_id).await?;
    finalize_deleted_if_absent(transaction, work.file_id).await?;
    Ok(())
}

async fn mark_operation_succeeded(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operation_id: Uuid,
) -> Result<(), RepositoryError> {
    school_database::DbSession::new(&mut **transaction)
        .query(
            "file-platform.src.repository.mark_operation_succeeded",
            r#"
UPDATE file_operations
SET status = 'succeeded', completed_at = now(), last_error_code = NULL,
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE id = $1
"#,
        )
        .bind(operation_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    Ok(())
}

async fn retry_operation_query(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operation_id: Uuid,
    error_code: &'static str,
    retry_microseconds: i64,
    terminal: bool,
) -> Result<(), RepositoryError> {
    school_database::DbSession::new(&mut **transaction)
        .query(
            "file-platform.src.repository.retry_operation_query",
            r#"
UPDATE file_operations
SET status = CASE WHEN $4 THEN 'failed' ELSE 'retryable_failure' END,
    last_error_code = $2,
    next_retry_at = statement_timestamp() + ($3 * INTERVAL '1 microsecond'),
    completed_at = CASE WHEN $4 THEN statement_timestamp() ELSE NULL END,
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE id = $1
"#,
        )
        .bind(operation_id)
        .bind(error_code)
        .bind(retry_microseconds)
        .bind(terminal)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    Ok(())
}

async fn finalize_deleted_if_absent(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    file_id: Uuid,
) -> Result<(), RepositoryError> {
    school_database::DbSession::new(&mut **transaction)
        .query(
            "file-platform.src.repository.finalize_deleted_if_absent",
            r#"
UPDATE files f
SET lifecycle_status = 'deleted', deleted_at = COALESCE(deleted_at, now()), updated_at = now()
WHERE f.id = $1 AND f.lifecycle_status = 'delete_requested'
  AND NOT EXISTS (
      SELECT 1 FROM file_versions v
      WHERE v.file_id = f.id AND v.storage_status <> 'deleted'
  )
  AND NOT EXISTS (
      SELECT 1 FROM file_derivatives d
      WHERE d.file_id = f.id AND d.storage_status <> 'deleted'
  )
"#,
        )
        .bind(file_id)
        .execute()
        .await
        .map_err(SqlFileRepository::database_error)?;
    Ok(())
}

fn stored_object_from_row(row: &sqlx::postgres::PgRow) -> Result<StoredObject, RepositoryError> {
    let storage_class = parse_storage_class(
        row.try_get::<String, _>("storage_class")
            .map_err(SqlFileRepository::database_error)?
            .as_str(),
    )?;
    let key = persisted_object_key(
        row.try_get("object_key")
            .map_err(SqlFileRepository::database_error)?,
        storage_class,
    )
    .map_err(|_| RepositoryError::InvalidPersistedState)?;
    Ok(StoredObject::new(
        key,
        row.try_get::<String, _>("detected_mime_type")
            .map_err(SqlFileRepository::database_error)?,
    ))
}

fn stored_object_from_aliased_row(
    row: &sqlx::postgres::PgRow,
    prefix: &str,
) -> Result<StoredObject, RepositoryError> {
    let storage_class_column = format!("{prefix}_storage_class");
    let object_key_column = format!("{prefix}_object_key");
    let mime_column = format!("{prefix}_detected_mime_type");
    let storage_class = parse_storage_class(
        row.try_get::<String, _>(storage_class_column.as_str())
            .map_err(SqlFileRepository::database_error)?
            .as_str(),
    )?;
    let key = persisted_object_key(
        row.try_get(object_key_column.as_str())
            .map_err(SqlFileRepository::database_error)?,
        storage_class,
    )
    .map_err(|_| RepositoryError::InvalidPersistedState)?;
    Ok(StoredObject::new(
        key,
        row.try_get::<String, _>(mime_column.as_str())
            .map_err(SqlFileRepository::database_error)?,
    ))
}

fn delivery_from_row(row: sqlx::postgres::PgRow) -> Result<DeliveryRecord, RepositoryError> {
    let purpose = purpose_from_code(
        row.try_get::<String, _>("purpose_code")
            .map_err(SqlFileRepository::database_error)?
            .as_str(),
    )
    .map_err(|_| RepositoryError::InvalidPersistedState)?;
    let visibility = parse_visibility(
        row.try_get::<String, _>("visibility")
            .map_err(SqlFileRepository::database_error)?
            .as_str(),
    )?;
    let lifecycle_status = parse_lifecycle(
        row.try_get::<String, _>("lifecycle_status")
            .map_err(SqlFileRepository::database_error)?
            .as_str(),
    )?;
    let current_version = row
        .try_get::<Option<i32>, _>("version_number")
        .map_err(SqlFileRepository::database_error)?
        .map(u32::try_from)
        .transpose()
        .map_err(|_| RepositoryError::InvalidPersistedState)?;
    let object = if row
        .try_get::<Option<String>, _>("object_key")
        .map_err(SqlFileRepository::database_error)?
        .is_some()
    {
        Some(stored_object_from_row(&row)?)
    } else {
        None
    };

    Ok(DeliveryRecord {
        file: PlatformFile {
            id: row
                .try_get("id")
                .map_err(SqlFileRepository::database_error)?,
            owner_user_id: row
                .try_get("owner_user_id")
                .map_err(SqlFileRepository::database_error)?,
            purpose,
            visibility,
            lifecycle_status,
            current_version,
            display_filename: row
                .try_get("display_filename")
                .map_err(SqlFileRepository::database_error)?,
            detected_mime_type: object
                .as_ref()
                .map(|object| object.content_type.clone())
                .unwrap_or_default(),
            byte_size: row
                .try_get("byte_size")
                .map_err(SqlFileRepository::database_error)?,
        },
        object,
    })
}

pub const fn derivative_is_required(purpose: FilePurpose, _recipe: DerivativeRecipe) -> bool {
    matches!(purpose, FilePurpose::SchoolLogo | FilePurpose::SchoolBanner)
}

fn recipe_from_kind(kind: &str) -> Result<DerivativeRecipe, RepositoryError> {
    match kind {
        "thumbnail-256" => Ok(DerivativeRecipe::Thumbnail256Webp),
        "thumbnail-1024" => Ok(DerivativeRecipe::Thumbnail1024Webp),
        _ => Err(RepositoryError::InvalidPersistedState),
    }
}

fn canonical_extension_from_mime(mime: &str) -> Result<&'static str, RepositoryError> {
    match mime {
        "image/jpeg" => Ok("jpg"),
        "image/png" => Ok("png"),
        "image/webp" => Ok("webp"),
        "application/pdf" => Ok("pdf"),
        "font/ttf" => Ok("ttf"),
        "font/otf" => Ok("otf"),
        _ => Err(RepositoryError::InvalidPersistedState),
    }
}

const fn visibility_code(visibility: FileVisibility) -> &'static str {
    match visibility {
        FileVisibility::Public => "public",
        FileVisibility::Private => "private",
    }
}

const fn storage_class_code(storage_class: StorageClass) -> &'static str {
    match storage_class {
        StorageClass::Public => "public",
        StorageClass::Private => "private",
    }
}

const fn retention_code(retention: RetentionClass) -> &'static str {
    match retention {
        RetentionClass::Standard => "standard",
        RetentionClass::Temporary => "temporary",
        RetentionClass::LegalHold => "legal_hold",
    }
}

fn parse_storage_class(value: &str) -> Result<StorageClass, RepositoryError> {
    match value {
        "public" => Ok(StorageClass::Public),
        "private" => Ok(StorageClass::Private),
        _ => Err(RepositoryError::InvalidPersistedState),
    }
}

fn parse_visibility(value: &str) -> Result<FileVisibility, RepositoryError> {
    match value {
        "public" => Ok(FileVisibility::Public),
        "private" => Ok(FileVisibility::Private),
        _ => Err(RepositoryError::InvalidPersistedState),
    }
}

fn parse_lifecycle(value: &str) -> Result<FileLifecycleStatus, RepositoryError> {
    match value {
        "pending" => Ok(FileLifecycleStatus::Pending),
        "processing" => Ok(FileLifecycleStatus::Processing),
        "ready" => Ok(FileLifecycleStatus::Ready),
        "delete_requested" => Ok(FileLifecycleStatus::DeleteRequested),
        "deleted" => Ok(FileLifecycleStatus::Deleted),
        "failed" => Ok(FileLifecycleStatus::Failed),
        "quarantined" => Ok(FileLifecycleStatus::Quarantined),
        _ => Err(RepositoryError::InvalidPersistedState),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        platform_types::{DerivativeRecipe, DetectedContent, FileInspectionMetadata},
        purpose_registry::{derivative_object_key, original_object_key},
    };
    use school_test_db::{create_named_test_pool_with_max_connections, run_test_migrations};

    static REPOSITORY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn repository_or_skip() -> Option<SqlFileRepository> {
        repository_with_connections_or_skip(1).await
    }
    async fn repository_with_connections_or_skip(
        max_connections: u32,
    ) -> Option<SqlFileRepository> {
        dotenvy::dotenv().ok();
        if std::env::var("TEST_DATABASE_URL")
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            tracing::warn!(
                "SKIPPED: TEST_DATABASE_URL is not set; repository lease tests require PostgreSQL"
            );
            return None;
        }
        let pool =
            create_named_test_pool_with_max_connections("file_repository", max_connections).await;
        run_test_migrations(&pool).await;
        Some(SqlFileRepository::new(pool))
    }

    #[test]
    fn duration_microseconds_is_checked_before_sql_binding() {
        assert_eq!(duration_microseconds(Duration::from_micros(25)), Ok(25));
        assert_eq!(
            duration_microseconds(Duration::MAX),
            Err(RepositoryError::InvalidPersistedState)
        );
    }

    #[tokio::test]
    async fn request_delete_in_transaction_finalizes_zero_object_file() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let file_id: Uuid = sqlx::query_scalar(
            "INSERT INTO files (
                display_filename, purpose_code, visibility,
                lifecycle_status, retention_class
             ) VALUES (
                'empty-upload.bin', 'generic_private_document', 'private',
                'processing', 'temporary'
             ) RETURNING id",
        )
        .fetch_one(repository.pool())
        .await
        .expect("zero-object file fixture should insert");

        let mut transaction = repository
            .pool()
            .begin()
            .await
            .expect("caller-owned delete transaction should begin");
        let work = repository
            .request_delete_in_transaction(&mut transaction, file_id)
            .await
            .expect("zero-object deletion should prepare");
        assert!(work.is_empty());
        transaction
            .commit()
            .await
            .expect("caller-owned delete transaction should commit");

        let state: (String, Option<chrono::DateTime<chrono::Utc>>) =
            sqlx::query_as("SELECT lifecycle_status, deleted_at FROM files WHERE id = $1")
                .bind(file_id)
                .fetch_one(repository.pool())
                .await
                .expect("zero-object file should remain inspectable");
        assert_eq!(state.0, "deleted");
        assert!(state.1.is_some());
    }

    #[tokio::test]
    async fn sql_repository_reserves_reclaims_finalizes_and_deletes_durably() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let actor_id = Uuid::new_v4();
        sqlx::query(
            r#"
INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
VALUES ($1, 'test-only-hash', 'Synthetic', 'User', 'staff', 'active')
"#,
        )
        .bind(actor_id)
        .execute(repository.pool())
        .await
        .expect("synthetic actor should insert");

        let tenant_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let original = StoredObject::new(
            original_object_key(
                tenant_id,
                FilePurpose::ProfileImage,
                file_id,
                1,
                DetectedContent::Png,
            )
            .unwrap(),
            "image/png",
        );
        let upload = NewUpload {
            file_id,
            version_id,
            reconcile_operation_id: Uuid::new_v4(),
            scan_operation_id: Uuid::new_v4(),
            owner_user_id: Some(actor_id),
            created_by: Some(actor_id),
            purpose: FilePurpose::ProfileImage,
            visibility: FileVisibility::Private,
            retention_class: RetentionClass::Standard,
            display_filename: "profile.png".to_string(),
            original,
            byte_size: 4,
            checksum: "a".repeat(64),
            inspection_metadata: FileInspectionMetadata::Image {
                width_px: 1,
                height_px: 1,
            },
            derivatives: Vec::new(),
        };

        repository.reserve_upload(&upload).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, sqlx::types::Json<FileInspectionMetadata>>(
                "SELECT inspection_metadata FROM files WHERE id = $1"
            )
            .bind(file_id)
            .fetch_one(repository.pool())
            .await
            .unwrap()
            .0,
            FileInspectionMetadata::Image {
                width_px: 1,
                height_px: 1,
            },
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id = $1")
                .bind(file_id)
                .fetch_one(repository.pool())
                .await
                .unwrap(),
            "processing"
        );
        let processing = repository
            .load_delivery(file_id)
            .await
            .expect("processing file metadata should load")
            .expect("processing file should exist");
        assert_eq!(
            processing.file.lifecycle_status,
            FileLifecycleStatus::Processing
        );
        assert_eq!(processing.file.byte_size, 4);
        assert!(processing.object.is_none());

        let first = repository
            .lease_due_operations("worker-one", Duration::from_secs(60), 10)
            .await
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].attempt_count, 1);
        assert!(repository
            .lease_due_operations("worker-two", Duration::from_secs(60), 10)
            .await
            .unwrap()
            .is_empty());

        sqlx::query(
            "UPDATE file_operations \
             SET leased_at = statement_timestamp() - INTERVAL '61 seconds', \
                 lease_expires_at = statement_timestamp() - INTERVAL '1 second' \
             WHERE id = $1",
        )
        .bind(first[0].id)
        .execute(repository.pool())
        .await
        .unwrap();

        let reclaimed = repository
            .lease_due_operations("worker-two", Duration::from_secs(60), 10)
            .await
            .unwrap();
        assert_eq!(reclaimed.len(), 1);
        assert_eq!(reclaimed[0].attempt_count, 2);

        repository
            .mark_original_stored(file_id, version_id)
            .await
            .unwrap();
        repository
            .finalize_ready(file_id, version_id, &[])
            .await
            .unwrap();
        let delivery = repository
            .load_delivery(file_id)
            .await
            .unwrap()
            .expect("ready file should load");
        assert_eq!(delivery.file.lifecycle_status, FileLifecycleStatus::Ready);
        assert!(delivery.object.is_some());

        let work = repository.request_delete(file_id).await.unwrap();
        assert_eq!(work.len(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id = $1")
                .bind(file_id)
                .fetch_one(repository.pool())
                .await
                .unwrap(),
            "delete_requested"
        );
        repository.mark_delete_succeeded(&work[0]).await.unwrap();
        assert!(repository.load_delivery(file_id).await.unwrap().is_none());
        assert!(repository.request_delete(file_id).await.unwrap().is_empty());

        repository
            .queue_delete_retry(
                file_id,
                ObjectTarget::Version(version_id),
                "storage_operation_failed",
                Duration::ZERO,
            )
            .await
            .unwrap();
        let terminal = repository
            .lease_due_operations("terminal-worker", Duration::from_secs(60), 10)
            .await
            .unwrap();
        assert_eq!(terminal.len(), 1);
        repository
            .retry_operation(
                terminal[0].id,
                "storage_operation_failed",
                Duration::ZERO,
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT status, last_error_code FROM file_operations WHERE id = $1"
            )
            .bind(terminal[0].id)
            .fetch_one(repository.pool())
            .await
            .unwrap(),
            (
                "failed".to_string(),
                Some("storage_operation_failed".to_string())
            ),
        );
    }

    #[tokio::test]
    async fn delete_request_revokes_materialization_before_object_deletion() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let actor_id = Uuid::new_v4();
        sqlx::query(
            r#"
INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
VALUES ($1, 'test-only-hash', 'Synthetic', 'User', 'staff', 'active')
"#,
        )
        .bind(actor_id)
        .execute(repository.pool())
        .await
        .expect("synthetic actor should insert");

        let tenant_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let derivative_id = Uuid::new_v4();
        let upload = NewUpload {
            file_id,
            version_id,
            reconcile_operation_id: Uuid::new_v4(),
            scan_operation_id: Uuid::new_v4(),
            owner_user_id: Some(actor_id),
            created_by: Some(actor_id),
            purpose: FilePurpose::SchoolLogo,
            visibility: FileVisibility::Public,
            retention_class: RetentionClass::Standard,
            display_filename: "logo.png".to_string(),
            original: StoredObject::new(
                original_object_key(
                    tenant_id,
                    FilePurpose::SchoolLogo,
                    file_id,
                    1,
                    DetectedContent::Png,
                )
                .unwrap(),
                "image/png",
            ),
            byte_size: 4,
            checksum: "a".repeat(64),
            inspection_metadata: FileInspectionMetadata::Image {
                width_px: 1,
                height_px: 1,
            },
            derivatives: vec![NewDerivative {
                id: derivative_id,
                operation_id: Uuid::new_v4(),
                recipe: DerivativeRecipe::Thumbnail256Webp,
                object: StoredObject::new(
                    derivative_object_key(
                        tenant_id,
                        FilePurpose::SchoolLogo,
                        file_id,
                        1,
                        DerivativeRecipe::Thumbnail256Webp,
                    )
                    .unwrap(),
                    "image/webp",
                ),
                byte_size: 4,
                checksum: "b".repeat(64),
                required: true,
            }],
        };

        repository.reserve_upload(&upload).await.unwrap();
        let deletion = repository.request_delete(file_id).await.unwrap();
        assert_eq!(deletion.len(), 2);
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT storage_status FROM file_versions WHERE id = $1"
            )
            .bind(version_id)
            .fetch_one(repository.pool())
            .await
            .unwrap(),
            "delete_requested",
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT storage_status FROM file_derivatives WHERE id = $1"
            )
            .bind(derivative_id)
            .fetch_one(repository.pool())
            .await
            .unwrap(),
            "delete_requested",
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                r#"
SELECT COUNT(*)
FROM file_operations
WHERE file_id = $1
  AND operation_type IN ('reconcile', 'generate_derivative')
  AND status = 'cancelled'
"#,
            )
            .bind(file_id)
            .fetch_one(repository.pool())
            .await
            .unwrap(),
            2,
        );
        assert_eq!(
            repository
                .mark_derivative_stored(file_id, derivative_id, upload.derivatives[0].operation_id)
                .await,
            Err(RepositoryError::MaterializationRevoked),
        );
        for work in &deletion {
            repository.mark_delete_succeeded(work).await.unwrap();
        }
    }
    fn worker_upload() -> NewUpload {
        let tenant = Uuid::new_v4();
        let file_id = Uuid::new_v4();
        NewUpload {
            file_id,
            version_id: Uuid::new_v4(),
            reconcile_operation_id: Uuid::new_v4(),
            scan_operation_id: Uuid::new_v4(),
            owner_user_id: None,
            created_by: None,
            purpose: FilePurpose::SchoolLogo,
            visibility: FileVisibility::Public,
            retention_class: RetentionClass::Standard,
            display_filename: "worker.png".into(),
            original: StoredObject::new(
                original_object_key(
                    tenant,
                    FilePurpose::SchoolLogo,
                    file_id,
                    1,
                    DetectedContent::Png,
                )
                .unwrap(),
                "image/png",
            ),
            byte_size: 4,
            checksum: "a".repeat(64),
            inspection_metadata: FileInspectionMetadata::Image {
                width_px: 1,
                height_px: 1,
            },
            derivatives: vec![NewDerivative {
                id: Uuid::new_v4(),
                operation_id: Uuid::new_v4(),
                recipe: DerivativeRecipe::Thumbnail256Webp,
                object: StoredObject::new(
                    derivative_object_key(
                        tenant,
                        FilePurpose::SchoolLogo,
                        file_id,
                        1,
                        DerivativeRecipe::Thumbnail256Webp,
                    )
                    .unwrap(),
                    "image/webp",
                ),
                byte_size: 4,
                checksum: "b".repeat(64),
                required: true,
            }],
        }
    }
    async fn expire_claim(repository: &SqlFileRepository, id: Uuid) {
        sqlx::query("UPDATE file_operations SET leased_at=statement_timestamp()-interval '2 minutes',lease_expires_at=statement_timestamp()-interval '1 second' WHERE id=$1")
            .bind(id).execute(repository.pool()).await.unwrap();
    }
    async fn worker_snapshot(repository: &SqlFileRepository, file: Uuid) -> serde_json::Value {
        sqlx::query_scalar("SELECT jsonb_build_object('file',to_jsonb(f),'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY v.id) FROM file_versions v WHERE v.file_id=f.id),'derivatives',(SELECT jsonb_agg(to_jsonb(d) ORDER BY d.id) FROM file_derivatives d WHERE d.file_id=f.id),'operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY o.id) FROM file_operations o WHERE o.file_id=f.id)) FROM files f WHERE f.id=$1")
            .bind(file).fetch_one(repository.pool()).await.unwrap()
    }
    fn worker_retry() -> LeasedOperationResult {
        LeasedOperationResult::Failed {
            error_code: "storage_operation_failed",
            retry_delay: Duration::ZERO,
            terminal: false,
        }
    }

    #[tokio::test]
    async fn worker_claims_fence_reclaimed_completion_and_retry_before_metadata_effects() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let upload = worker_upload();
        repository.reserve_upload(&upload).await.unwrap();
        repository
            .mark_original_stored(upload.file_id, upload.version_id)
            .await
            .unwrap();
        let first = repository
            .lease_due_operations("same-owner", Duration::from_secs(60), 10)
            .await
            .unwrap();
        assert_eq!(first.len(), 2);
        for operation in &first {
            expire_claim(&repository, operation.id).await;
        }
        // Even an expired lease that has not been reclaimed must cause no write.
        let expired = worker_snapshot(&repository, upload.file_id).await;
        for operation in &first {
            assert_eq!(
                repository
                    .finish_leased_operation(operation, worker_retry())
                    .await,
                Err(RepositoryError::LeaseLost)
            );
        }
        assert_eq!(worker_snapshot(&repository, upload.file_id).await, expired);
        // Reuse the worker name to prove the attempt generation is required.
        let second = repository
            .lease_due_operations("same-owner", Duration::from_secs(60), 10)
            .await
            .unwrap();
        assert!(second.iter().all(|operation| operation.attempt_count == 2));
        let derivative = second
            .iter()
            .find(|operation| matches!(operation.work, OperationWork::GenerateDerivative(_)))
            .unwrap();
        repository
            .finish_leased_operation(derivative, LeasedOperationResult::Succeeded)
            .await
            .unwrap();
        let reconcile = second
            .iter()
            .find(|operation| matches!(operation.work, OperationWork::ReconcileUpload { .. }))
            .unwrap();
        repository
            .finish_leased_operation(reconcile, LeasedOperationResult::Succeeded)
            .await
            .unwrap();
        let committed = worker_snapshot(&repository, upload.file_id).await;
        for operation in &first {
            assert_eq!(
                repository
                    .finish_leased_operation(operation, LeasedOperationResult::Succeeded)
                    .await,
                Err(RepositoryError::LeaseLost)
            );
            assert_eq!(
                repository
                    .finish_leased_operation(operation, worker_retry())
                    .await,
                Err(RepositoryError::LeaseLost)
            );
            if matches!(operation.work, OperationWork::GenerateDerivative(_)) {
                assert_eq!(
                    repository
                        .finish_leased_operation(
                            operation,
                            LeasedOperationResult::DerivativeWriteUncertain {
                                error_code: "file_operation_deadline",
                                retry_delay: Duration::ZERO,
                                terminal: false
                            }
                        )
                        .await,
                    Err(RepositoryError::LeaseLost)
                );
            }
            if matches!(operation.work, OperationWork::ReconcileUpload { .. }) {
                assert_eq!(
                    repository
                        .finish_leased_operation(operation, LeasedOperationResult::OriginalMissing)
                        .await,
                    Err(RepositoryError::LeaseLost)
                );
            }
        }
        assert_eq!(
            worker_snapshot(&repository, upload.file_id).await,
            committed
        );
        let deletion = repository.request_delete(upload.file_id).await.unwrap();
        assert_eq!(deletion.len(), 2);
        let first = repository
            .lease_due_operations("delete-a", Duration::from_secs(60), 10)
            .await
            .unwrap();
        for operation in &first {
            expire_claim(&repository, operation.id).await;
        }
        let second = repository
            .lease_due_operations("delete-b", Duration::from_secs(60), 10)
            .await
            .unwrap();
        for operation in &second {
            repository
                .finish_leased_operation(operation, LeasedOperationResult::Succeeded)
                .await
                .unwrap();
        }
        let deleted = worker_snapshot(&repository, upload.file_id).await;
        for operation in &first {
            assert_eq!(
                repository
                    .finish_leased_operation(operation, worker_retry())
                    .await,
                Err(RepositoryError::LeaseLost)
            );
            assert_eq!(
                repository
                    .finish_leased_operation(operation, LeasedOperationResult::Succeeded)
                    .await,
                Err(RepositoryError::LeaseLost)
            );
        }
        assert_eq!(worker_snapshot(&repository, upload.file_id).await, deleted);
    }

    #[tokio::test]
    async fn cancelled_derivative_late_put_queues_new_delete_after_prior_delete_succeeded() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let upload = worker_upload();
        repository.reserve_upload(&upload).await.unwrap();
        let claimed = repository
            .lease_due_operations("put-a", Duration::from_secs(60), 10)
            .await
            .unwrap();
        let derivative = claimed
            .iter()
            .find(|operation| matches!(operation.work, OperationWork::GenerateDerivative(_)))
            .unwrap();
        let deletion = repository.request_delete(upload.file_id).await.unwrap();
        for work in &deletion {
            repository.mark_delete_succeeded(work).await.unwrap();
        }
        let state: (String, String) = sqlx::query_as("SELECT f.lifecycle_status,d.storage_status FROM files f JOIN file_derivatives d ON d.file_id=f.id WHERE f.id=$1")
            .bind(upload.file_id).fetch_one(repository.pool()).await.unwrap();
        assert_eq!(state, ("deleted".into(), "deleted".into()));
        assert_eq!(
            repository
                .finish_leased_operation(derivative, LeasedOperationResult::Succeeded)
                .await,
            Err(RepositoryError::LeaseLost)
        );
        let status: (String, i32) =
            sqlx::query_as("SELECT status,attempt_count FROM file_operations WHERE id=$1")
                .bind(derivative.id)
                .fetch_one(repository.pool())
                .await
                .unwrap();
        assert_eq!(status, ("cancelled".into(), derivative.attempt_count));
        let cleanup = repository
            .lease_due_operations("late-put-cleanup", Duration::from_secs(60), 10)
            .await
            .unwrap();
        assert_eq!(cleanup.len(), 1);
        assert!(
            matches!(&cleanup[0].work, OperationWork::DeleteObject(work) if work.target == ObjectTarget::Derivative(upload.derivatives[0].id))
        );
        repository
            .finish_leased_operation(&cleanup[0], LeasedOperationResult::Succeeded)
            .await
            .unwrap();
        let unchanged: (String, String) = sqlx::query_as("SELECT f.lifecycle_status,d.storage_status FROM files f JOIN file_derivatives d ON d.file_id=f.id WHERE f.id=$1")
            .bind(upload.file_id).fetch_one(repository.pool()).await.unwrap();
        assert_eq!(unchanged, state);
    }

    #[tokio::test]
    async fn worker_finish_rechecks_expiry_after_waiting_for_the_operation_lock() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_with_connections_or_skip(3).await else {
            return;
        };
        let mut upload = worker_upload();
        upload.derivatives.clear();
        repository.reserve_upload(&upload).await.unwrap();
        repository
            .mark_original_stored(upload.file_id, upload.version_id)
            .await
            .unwrap();
        let operation = repository
            .lease_due_operations("blocked-worker", Duration::from_secs(60), 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        sqlx::query("UPDATE file_operations SET lease_expires_at=statement_timestamp()+interval '1 second' WHERE id=$1")
            .bind(operation.id).execute(repository.pool()).await.unwrap();
        let mut blocker = repository.pool().begin().await.unwrap();
        sqlx::query("SELECT id FROM file_operations WHERE id=$1 FOR UPDATE")
            .bind(operation.id)
            .execute(&mut *blocker)
            .await
            .unwrap();
        let worker_repository = repository.clone();
        let worker_operation = operation.clone();
        let worker = tokio::spawn(async move {
            worker_repository
                .finish_leased_operation(&worker_operation, LeasedOperationResult::Succeeded)
                .await
        });
        // Observe the actual blocked backend, rather than guessing scheduling.
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *blocker).await.unwrap();
                let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT id FROM file_operations WHERE id=%FOR UPDATE%')")
                    .fetch_one(&mut *blocker).await.unwrap();
                if waiting { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("worker should wait for the operation lock");
        // Do not change the locked row: its lease expires by wall time while
        // the finishing statement waits. A pre-lock statement timestamp (and
        // transaction-stable now()) would incorrectly accept this completion.
        sqlx::query("SELECT pg_sleep(1.1)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        blocker.commit().await.unwrap();
        let expired = worker_snapshot(&repository, upload.file_id).await;
        assert_eq!(worker.await.unwrap(), Err(RepositoryError::LeaseLost));
        assert_eq!(worker_snapshot(&repository, upload.file_id).await, expired);
    }
    async fn wait_for_blocked_backend(connection: &mut sqlx::PgConnection, blocker: i32) -> i32 {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *connection).await.unwrap();
                let backend: Option<i32> = sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1")
                    .bind(blocker).fetch_optional(&mut *connection).await.unwrap();
                if let Some(backend) = backend { break backend; }
                tokio::task::yield_now().await;
            }
        }).await.expect("a real backend should reach the lock barrier")
    }

    #[tokio::test]
    async fn synchronous_and_worker_derivative_completion_share_file_operation_target_lock_order() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_with_connections_or_skip(3).await else {
            return;
        };
        let upload = worker_upload();
        repository.reserve_upload(&upload).await.unwrap();
        let claimed = repository
            .lease_due_operations("worker-before-sync", Duration::from_secs(60), 10)
            .await
            .unwrap();
        let operation = claimed
            .into_iter()
            .find(|operation| matches!(operation.work, OperationWork::GenerateDerivative(_)))
            .unwrap();
        let mut blocker = repository.pool().begin().await.unwrap();
        let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM file_operations WHERE id=$1 FOR UPDATE")
            .bind(operation.id)
            .execute(&mut *blocker)
            .await
            .unwrap();
        let worker_repository = repository.clone();
        let worker_operation = operation.clone();
        let worker = tokio::spawn(async move {
            worker_repository
                .finish_leased_operation(&worker_operation, LeasedOperationResult::Succeeded)
                .await
        });
        let worker_pid = wait_for_blocked_backend(&mut blocker, blocker_pid).await;
        let sync_repository = repository.clone();
        let derivative_id = upload.derivatives[0].id;
        let sync = tokio::spawn(async move {
            sync_repository
                .mark_derivative_stored(operation.file_id, derivative_id, operation.id)
                .await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *blocker).await.unwrap();
                let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid<>$1 AND ($1=ANY(pg_blocking_pids(pid)) OR $2=ANY(pg_blocking_pids(pid))))")
                    .bind(worker_pid).bind(blocker_pid).fetch_one(&mut *blocker).await.unwrap();
                if waiting { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("synchronous completion should also reach the lock barrier");
        blocker.commit().await.unwrap();
        let (worker, sync) =
            tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(worker, sync) })
                .await
                .unwrap();
        worker.unwrap().unwrap();
        sync.unwrap().unwrap();
        let state: (String,String,String) = sqlx::query_as("SELECT d.storage_status,d.lifecycle_status,o.status FROM file_derivatives d JOIN file_operations o ON o.file_derivative_id=d.id WHERE o.id=$1")
            .bind(upload.derivatives[0].operation_id).fetch_one(repository.pool()).await.unwrap();
        assert_eq!(state, ("stored".into(), "ready".into(), "succeeded".into()));
    }

    struct CleanScanner;
    #[async_trait::async_trait]
    impl crate::malware_scanner::MalwareScanner for CleanScanner {
        async fn scan(&self, _: &[u8]) -> crate::malware_scanner::ScanOutcome {
            crate::malware_scanner::ScanOutcome::Clean
        }
    }
    struct AmbiguousPutProvider {
        source: bytes::Bytes,
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
        stored: tokio::sync::Notify,
        objects: std::sync::Mutex<std::collections::HashSet<String>>,
    }
    #[async_trait::async_trait]
    impl crate::storage_provider::StorageProvider for AmbiguousPutProvider {
        async fn check_readiness(&self) -> Result<(), crate::storage_provider::StorageError> {
            Ok(())
        }
        async fn put(
            &self,
            object: &StoredObject,
            _: bytes::Bytes,
        ) -> Result<(), crate::storage_provider::StorageError> {
            self.started.notify_one();
            self.release.notified().await;
            self.objects
                .lock()
                .unwrap()
                .insert(object.object_key.as_str().into());
            self.stored.notify_one();
            std::future::pending().await
        }
        async fn get(
            &self,
            _: &StoredObject,
            _: u64,
        ) -> Result<bytes::Bytes, crate::storage_provider::StorageError> {
            Ok(self.source.clone())
        }
        async fn head(
            &self,
            object: &StoredObject,
        ) -> Result<
            Option<crate::storage_provider::ObjectMetadata>,
            crate::storage_provider::StorageError,
        > {
            Ok(self
                .objects
                .lock()
                .unwrap()
                .contains(object.object_key.as_str())
                .then(|| crate::storage_provider::ObjectMetadata {
                    content_type: Some(object.content_type.clone()),
                    content_length: Some(4),
                }))
        }
        async fn delete(
            &self,
            object: &StoredObject,
        ) -> Result<(), crate::storage_provider::StorageError> {
            self.objects
                .lock()
                .unwrap()
                .remove(object.object_key.as_str());
            Ok(())
        }
        async fn private_download_grant(
            &self,
            _: &StoredObject,
            _: &str,
            _: Duration,
        ) -> Result<crate::platform_types::DownloadGrant, crate::storage_provider::StorageError>
        {
            Err(crate::storage_provider::StorageError::OperationFailed)
        }
        fn public_location(
            &self,
            _: &StoredObject,
        ) -> Result<url::Url, crate::storage_provider::StorageError> {
            Err(crate::storage_provider::StorageError::OperationFailed)
        }
    }

    #[tokio::test]
    async fn cancelled_derivative_ambiguous_put_timeout_queues_durable_cleanup_after_delete() {
        use crate::{
            file_hash::FileHasher,
            platform_service::{generate_derivative_body, FilePlatform},
            reconciler::perform_leased_operation,
            runtime_config::FilePlatformRuntimeConfig,
            storage_provider::StorageProvider,
        };
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let mut source = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([4, 8, 15, 255]),
        ))
        .write_to(
            &mut std::io::Cursor::new(&mut source),
            image::ImageFormat::Png,
        )
        .unwrap();
        let mut upload = worker_upload();
        let body = generate_derivative_body(upload.purpose, upload.derivatives[0].recipe, &source)
            .unwrap();
        upload.derivatives[0].byte_size = i64::try_from(body.len()).unwrap();
        upload.derivatives[0].checksum = FileHasher::sha256(&body);
        repository.reserve_upload(&upload).await.unwrap();
        let operation = repository
            .lease_due_operations("ambiguous-put", Duration::from_secs(60), 10)
            .await
            .unwrap()
            .into_iter()
            .find(|operation| matches!(operation.work, OperationWork::GenerateDerivative(_)))
            .unwrap();
        let provider = std::sync::Arc::new(AmbiguousPutProvider {
            source: source.into(),
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            stored: tokio::sync::Notify::new(),
            objects: std::sync::Mutex::new(std::collections::HashSet::new()),
        });
        let platform = std::sync::Arc::new(FilePlatform::with_config(
            provider.clone(),
            std::sync::Arc::new(CleanScanner),
            FilePlatformRuntimeConfig {
                reconciliation_lease: Duration::from_secs(4),
                ..FilePlatformRuntimeConfig::default()
            },
        ));
        let performing_platform = platform.clone();
        let performing_operation = operation.clone();
        let performing = tokio::spawn(async move {
            perform_leased_operation(&performing_platform, &performing_operation).await
        });
        tokio::time::timeout(Duration::from_secs(1), provider.started.notified())
            .await
            .unwrap();
        let deletion = repository.request_delete(upload.file_id).await.unwrap();
        for work in deletion {
            provider.delete(&work.object).await.unwrap();
            repository.mark_delete_succeeded(&work).await.unwrap();
        }
        provider.release.notify_one();
        tokio::time::timeout(Duration::from_secs(1), provider.stored.notified())
            .await
            .unwrap();
        assert_eq!(provider.objects.lock().unwrap().len(), 1);
        let result = tokio::time::timeout(Duration::from_secs(3), performing)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            LeasedOperationResult::DerivativeWriteUncertain {
                error_code: "file_operation_deadline",
                ..
            }
        ));
        assert_eq!(
            repository.finish_leased_operation(&operation, result).await,
            Err(RepositoryError::LeaseLost)
        );
        let metadata: (String,String,String) = sqlx::query_as("SELECT f.lifecycle_status,d.lifecycle_status,o.status FROM files f JOIN file_derivatives d ON d.file_id=f.id JOIN file_operations o ON o.file_derivative_id=d.id WHERE o.id=$1")
            .bind(operation.id).fetch_one(repository.pool()).await.unwrap();
        assert_eq!(
            metadata,
            ("deleted".into(), "deleted".into(), "cancelled".into())
        );
        let cleanup = repository
            .lease_due_operations("cleanup", Duration::from_secs(60), 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        let result = perform_leased_operation(&platform, &cleanup).await;
        assert_eq!(result, LeasedOperationResult::Succeeded);
        repository
            .finish_leased_operation(&cleanup, result)
            .await
            .unwrap();
        assert!(provider.objects.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn live_derivative_write_uncertainty_retries_without_creating_delete_work() {
        let _guard = REPOSITORY_TEST_LOCK.lock().await;
        let Some(repository) = repository_or_skip().await else {
            return;
        };
        let upload = worker_upload();
        repository.reserve_upload(&upload).await.unwrap();
        let operation = repository
            .lease_due_operations("uncertain-live", Duration::from_secs(60), 10)
            .await
            .unwrap()
            .into_iter()
            .find(|operation| matches!(operation.work, OperationWork::GenerateDerivative(_)))
            .unwrap();
        repository
            .finish_leased_operation(
                &operation,
                LeasedOperationResult::DerivativeWriteUncertain {
                    error_code: "file_operation_deadline",
                    retry_delay: Duration::ZERO,
                    terminal: false,
                },
            )
            .await
            .unwrap();
        let state: (String,String,String) = sqlx::query_as("SELECT f.lifecycle_status,d.storage_status,o.status FROM files f JOIN file_derivatives d ON d.file_id=f.id JOIN file_operations o ON o.file_derivative_id=d.id WHERE o.id=$1")
            .bind(operation.id).fetch_one(repository.pool()).await.unwrap();
        assert_eq!(
            state,
            (
                "processing".into(),
                "failed".into(),
                "retryable_failure".into()
            )
        );
        let deletes: i64 = sqlx::query_scalar("SELECT count(*) FROM file_operations WHERE file_id=$1 AND operation_type='delete_object'")
            .bind(upload.file_id).fetch_one(repository.pool()).await.unwrap();
        assert_eq!(deletes, 0);
    }
}
