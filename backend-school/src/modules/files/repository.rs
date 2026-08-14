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
}

impl RepositoryError {
    pub const fn log_safe_code(self) -> &'static str {
        match self {
            Self::OperationFailed => "file_repository_operation_failed",
            Self::InvalidPersistedState => "file_repository_state_invalid",
            Self::MaterializationRevoked => "file_materialization_revoked",
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

#[derive(Clone)]
pub struct LeasedOperation {
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
        let row = sqlx::query(LOAD_DELIVERY_SQL)
            .bind(file_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(Self::database_error)?;
        row.map(delivery_from_row).transpose()
    }

    fn database_error(_: sqlx::Error) -> RepositoryError {
        RepositoryError::OperationFailed
    }

    async fn load_operation(&self, operation_id: Uuid) -> Result<LeasedOperation, RepositoryError> {
        let operation = sqlx::query(
            r#"
SELECT id, file_id, file_version_id, file_derivative_id, operation_type, attempt_count
FROM file_operations
WHERE id = $1 AND status = 'leased'
"#,
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::database_error)?
        .ok_or(RepositoryError::InvalidPersistedState)?;

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
                self.load_reconcile_work(file_id, version_id).await?
            }
            "generate_derivative" => {
                let derivative_id: Uuid = operation
                    .try_get("file_derivative_id")
                    .map_err(Self::database_error)?;
                OperationWork::GenerateDerivative(
                    self.load_derivative_work(file_id, derivative_id).await?,
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
                    self.load_delete_work(operation_id, file_id, version_id, derivative_id)
                        .await?,
                )
            }
            _ => return Err(RepositoryError::InvalidPersistedState),
        };

        Ok(LeasedOperation {
            id: operation_id,
            file_id,
            attempt_count,
            work,
        })
    }

    async fn load_reconcile_work(
        &self,
        file_id: Uuid,
        version_id: Uuid,
    ) -> Result<OperationWork, RepositoryError> {
        let row = sqlx::query(
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
        .fetch_optional(&self.pool)
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

        let derivative_rows = sqlx::query(
            r#"
SELECT id, derivative_kind, object_key, storage_class, detected_mime_type
FROM file_derivatives
WHERE file_id = $1 AND source_version_id = $2 AND deleted_at IS NULL
ORDER BY derivative_kind
"#,
        )
        .bind(file_id)
        .bind(version_id)
        .fetch_all(&self.pool)
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
        &self,
        file_id: Uuid,
        derivative_id: Uuid,
    ) -> Result<DerivativeWork, RepositoryError> {
        let row = sqlx::query(
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
        .fetch_optional(&self.pool)
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
        &self,
        operation_id: Uuid,
        file_id: Uuid,
        version_id: Option<Uuid>,
        derivative_id: Option<Uuid>,
    ) -> Result<DeleteWork, RepositoryError> {
        match (version_id, derivative_id) {
            (Some(version_id), None) => {
                let row = sqlx::query(
                    "SELECT object_key, storage_class, detected_mime_type FROM file_versions WHERE id = $1 AND file_id = $2",
                )
                .bind(version_id)
                .bind(file_id)
                .fetch_optional(&self.pool)
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
                let row = sqlx::query(
                    "SELECT object_key, storage_class, detected_mime_type FROM file_derivatives WHERE id = $1 AND file_id = $2",
                )
                .bind(derivative_id)
                .bind(file_id)
                .fetch_optional(&self.pool)
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
        let lifecycle = sqlx::query_scalar::<_, String>(
            "SELECT lifecycle_status FROM files WHERE id = $1 FOR UPDATE",
        )
        .bind(file_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(Self::database_error)?;
        let Some(lifecycle) = lifecycle else {
            return Ok(Vec::new());
        };
        if lifecycle == "deleted" {
            return Ok(Vec::new());
        }

        sqlx::query(
            r#"
UPDATE files
SET lifecycle_status = 'delete_requested',
    delete_requested_at = COALESCE(delete_requested_at, now()),
    updated_at = now()
WHERE id = $1 AND lifecycle_status <> 'deleted'
"#,
        )
        .bind(file_id)
        .execute(&mut **transaction)
        .await
        .map_err(Self::database_error)?;

        sqlx::query(
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
        .execute(&mut **transaction)
        .await
        .map_err(Self::database_error)?;
        sqlx::query(
            r#"
UPDATE file_versions
SET storage_status = 'delete_requested'
WHERE file_id = $1 AND storage_status <> 'deleted'
"#,
        )
        .bind(file_id)
        .execute(&mut **transaction)
        .await
        .map_err(Self::database_error)?;
        sqlx::query(
            r#"
UPDATE file_derivatives
SET storage_status = 'delete_requested'
WHERE file_id = $1 AND storage_status <> 'deleted'
"#,
        )
        .bind(file_id)
        .execute(&mut **transaction)
        .await
        .map_err(Self::database_error)?;

        sqlx::query(
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
        .execute(&mut **transaction)
        .await
        .map_err(Self::database_error)?;
        sqlx::query(
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
        .execute(&mut **transaction)
        .await
        .map_err(Self::database_error)?;

        let rows = sqlx::query(
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
        .fetch_all(&mut **transaction)
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
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        let visibility = visibility_code(upload.visibility);
        let retention = retention_code(upload.retention_class);

        sqlx::query(
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
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;

        sqlx::query(
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
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;

        sqlx::query(
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
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;

        sqlx::query(
            r#"
INSERT INTO file_operations (
    id, file_id, file_version_id, operation_type, status, next_retry_at
) VALUES ($1, $2, $3, 'reconcile', 'pending', now())
"#,
        )
        .bind(upload.reconcile_operation_id)
        .bind(upload.file_id)
        .bind(upload.version_id)
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;

        for derivative in &upload.derivatives {
            sqlx::query(
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
            .execute(&mut *transaction)
            .await
            .map_err(Self::database_error)?;

            sqlx::query(
                r#"
INSERT INTO file_operations (
    id, file_id, file_derivative_id, operation_type, status, next_retry_at
) VALUES ($1, $2, $3, 'generate_derivative', 'pending', now())
"#,
            )
            .bind(derivative.operation_id)
            .bind(upload.file_id)
            .bind(derivative.id)
            .execute(&mut *transaction)
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
        let result = sqlx::query(
            "UPDATE file_versions SET storage_status = 'stored' WHERE id = $1 AND file_id = $2 AND storage_status = 'pending'",
        )
        .bind(version_id)
        .bind(file_id)
        .execute(&self.pool)
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
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        sqlx::query(
            "UPDATE file_versions SET storage_status = 'failed' WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
        )
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        sqlx::query(
            "UPDATE files SET lifecycle_status = 'failed', updated_at = now() WHERE id = $1 AND lifecycle_status <> 'deleted'",
        )
        .bind(file_id)
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        sqlx::query(
            r#"
UPDATE file_operations
SET status = 'failed', last_error_code = $2, completed_at = now(),
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE file_id = $1 AND operation_type = 'reconcile' AND status <> 'succeeded'
"#,
        )
        .bind(file_id)
        .bind(error_code)
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn mark_derivative_stored(
        &self,
        file_id: Uuid,
        derivative_id: Uuid,
        operation_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        let result = sqlx::query(
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
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::MaterializationRevoked);
        }
        mark_operation_succeeded(&mut transaction, operation_id).await?;
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
        let retry_microseconds = duration_microseconds(retry_delay)?;
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        sqlx::query(
            "UPDATE file_derivatives SET storage_status = 'failed', lifecycle_status = 'failed' WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
        )
        .bind(derivative_id)
        .bind(file_id)
        .execute(&mut *transaction)
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

    async fn finalize_ready(
        &self,
        file_id: Uuid,
        version_id: Uuid,
        required_derivative_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        let original_ready = sqlx::query_scalar::<_, bool>(
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
        .fetch_one(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        if !original_ready {
            return Err(RepositoryError::InvalidPersistedState);
        }

        if !required_derivative_ids.is_empty() {
            let ready_count = sqlx::query_scalar::<_, i64>(
                r#"
SELECT COUNT(*)
FROM file_derivatives
WHERE file_id = $1 AND id = ANY($2)
  AND storage_status = 'stored' AND lifecycle_status = 'ready' AND deleted_at IS NULL
"#,
            )
            .bind(file_id)
            .bind(required_derivative_ids)
            .fetch_one(&mut *transaction)
            .await
            .map_err(Self::database_error)?;
            if ready_count != required_derivative_ids.len() as i64 {
                return Err(RepositoryError::InvalidPersistedState);
            }
        }

        let result = sqlx::query(
            r#"
UPDATE files
SET current_version_id = $2, lifecycle_status = 'ready', updated_at = now()
WHERE id = $1 AND lifecycle_status IN ('processing', 'failed', 'ready')
"#,
        )
        .bind(file_id)
        .bind(version_id)
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::MaterializationRevoked);
        }
        sqlx::query(
            r#"
UPDATE file_operations
SET status = 'succeeded', completed_at = now(), last_error_code = NULL,
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE file_id = $1 AND operation_type = 'reconcile' AND status <> 'succeeded'
"#,
        )
        .bind(file_id)
        .execute(&mut *transaction)
        .await
        .map_err(Self::database_error)?;
        transaction.commit().await.map_err(Self::database_error)
    }

    async fn mark_reconcile_pending(
        &self,
        file_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
    ) -> Result<(), RepositoryError> {
        let retry_microseconds = duration_microseconds(retry_delay)?;
        sqlx::query(
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
        .execute(&self.pool)
        .await
        .map_err(Self::database_error)?;
        Ok(())
    }

    async fn load_delivery(
        &self,
        file_id: Uuid,
    ) -> Result<Option<DeliveryRecord>, RepositoryError> {
        let row = sqlx::query(LOAD_DELIVERY_SQL)
            .bind(file_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::database_error)?;
        row.map(delivery_from_row).transpose()
    }

    async fn request_delete(&self, file_id: Uuid) -> Result<Vec<DeleteWork>, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        let work = self
            .request_delete_in_transaction(&mut transaction, file_id)
            .await?;
        transaction.commit().await.map_err(Self::database_error)?;
        Ok(work)
    }

    async fn mark_delete_succeeded(&self, work: &DeleteWork) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
        match work.target {
            ObjectTarget::Version(version_id) => {
                sqlx::query(
                    "UPDATE file_versions SET storage_status = 'deleted', deleted_at = COALESCE(deleted_at, now()) WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
                )
                .bind(version_id)
                .bind(work.file_id)
                .execute(&mut *transaction)
                .await
                .map_err(Self::database_error)?;
            }
            ObjectTarget::Derivative(derivative_id) => {
                sqlx::query(
                    "UPDATE file_derivatives SET storage_status = 'deleted', lifecycle_status = 'deleted', deleted_at = COALESCE(deleted_at, now()) WHERE id = $1 AND file_id = $2 AND storage_status <> 'deleted'",
                )
                .bind(derivative_id)
                .bind(work.file_id)
                .execute(&mut *transaction)
                .await
                .map_err(Self::database_error)?;
            }
        }
        mark_operation_succeeded(&mut transaction, work.operation_id).await?;
        finalize_deleted_if_absent(&mut transaction, work.file_id).await?;
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
        let rows = sqlx::query(
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
RETURNING o.id
"#,
        )
        .bind(limit)
        .bind(worker)
        .bind(lease_microseconds)
        .fetch_all(&self.pool)
        .await
        .map_err(Self::database_error)?;

        let mut operations = Vec::with_capacity(rows.len());
        for row in rows {
            operations.push(
                self.load_operation(row.try_get("id").map_err(Self::database_error)?)
                    .await?,
            );
        }
        Ok(operations)
    }

    async fn retry_operation(
        &self,
        operation_id: Uuid,
        error_code: &'static str,
        retry_delay: Duration,
        terminal: bool,
    ) -> Result<(), RepositoryError> {
        let retry_microseconds = duration_microseconds(retry_delay)?;
        let mut transaction = self.pool.begin().await.map_err(Self::database_error)?;
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
        sqlx::query(
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
        .execute(&self.pool)
        .await
        .map_err(Self::database_error)?;
        Ok(())
    }
}

async fn mark_operation_succeeded(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operation_id: Uuid,
) -> Result<(), RepositoryError> {
    sqlx::query(
        r#"
UPDATE file_operations
SET status = 'succeeded', completed_at = now(), last_error_code = NULL,
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE id = $1
"#,
    )
    .bind(operation_id)
    .execute(&mut **transaction)
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
    sqlx::query(
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
    .execute(&mut **transaction)
    .await
    .map_err(SqlFileRepository::database_error)?;
    Ok(())
}

async fn finalize_deleted_if_absent(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    file_id: Uuid,
) -> Result<(), RepositoryError> {
    sqlx::query(
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
    .execute(&mut **transaction)
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
        modules::files::{
            platform_types::{DerivativeRecipe, DetectedContent, FileInspectionMetadata},
            purpose_registry::{derivative_object_key, original_object_key},
        },
        test_helpers::{create_named_test_pool, run_test_migrations},
    };

    static REPOSITORY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn repository_or_skip() -> Option<SqlFileRepository> {
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
        let pool = create_named_test_pool("file_repository").await;
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
}
