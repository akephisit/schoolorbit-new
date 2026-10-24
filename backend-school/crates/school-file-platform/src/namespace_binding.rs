//! Offline, explicitly attested legacy namespace binding. Never migrates or probes storage.
use super::{
    platform_types::StorageClass, storage_namespace::StorageNamespace,
    storage_provider::StorageProvider,
};
use chrono::{DateTime, Utc};
use school_database::DbSession;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Connection, FromRow, PgConnection, PgPool};
use uuid::Uuid;

pub struct ConfiguredNamespaces {
    public: StorageNamespace,
    private: StorageNamespace,
}
impl ConfiguredNamespaces {
    pub fn from_provider(provider: &dyn StorageProvider) -> Result<Self, BindingError> {
        Ok(Self {
            public: provider
                .namespace(StorageClass::Public)
                .map_err(|_| BindingError::Configuration)?,
            private: provider
                .namespace(StorageClass::Private)
                .map_err(|_| BindingError::Configuration)?,
        })
    }
    pub fn from_r2_config(
        config: &super::r2_storage_provider::R2StorageConfig,
    ) -> Result<Self, BindingError> {
        Ok(Self {
            public: config
                .namespace(StorageClass::Public)
                .map_err(|_| BindingError::Configuration)?,
            private: config
                .namespace(StorageClass::Private)
                .map_err(|_| BindingError::Configuration)?,
        })
    }
    pub fn from_r2_routing(
        routing: &super::r2_storage_provider::R2StorageRouting,
    ) -> Result<Self, BindingError> {
        Ok(Self {
            public: routing
                .namespace(StorageClass::Public)
                .map_err(|_| BindingError::Configuration)?,
            private: routing
                .namespace(StorageClass::Private)
                .map_err(|_| BindingError::Configuration)?,
        })
    }
    fn expected(&self, class: &str) -> Result<&str, BindingError> {
        match class {
            "public" => Ok(self.public.fingerprint()),
            "private" => Ok(self.private.fingerprint()),
            _ => Err(BindingError::Configuration),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingManifest {
    pub attestation_digest: String,
    pub watches: Vec<BindingEntry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingEntry {
    pub watch_id: Uuid,
    pub expected_identity_digest: String,
    pub expected_row_revision: i64,
    pub namespace_fingerprint: String,
}
#[derive(Clone, Copy)]
pub struct NamespaceReadiness {
    pub total: i64,
    pub unresolved: i64,
    pub mismatched: i64,
}
impl NamespaceReadiness {
    pub fn ready(&self) -> bool {
        self.unresolved == 0 && self.mismatched == 0
    }
}
#[derive(Debug, Clone, Copy)]
pub enum BindingError {
    InvalidManifest,
    SchemaMissing,
    Configuration,
    OwnerRequired,
    ChangedWatch,
    ActiveClaim,
    Database,
}
impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidManifest => "storage_binding_manifest_invalid",
            Self::SchemaMissing => "storage_watch_schema_missing",
            Self::Configuration => "storage_binding_namespace_invalid",
            Self::OwnerRequired => "storage_binding_owner_required",
            Self::ChangedWatch => "storage_binding_watch_changed",
            Self::ActiveClaim => "storage_binding_active_claim",
            Self::Database => "storage_binding_database_failed",
        })
    }
}
impl std::error::Error for BindingError {}
#[derive(FromRow)]
struct WatchIdentity {
    id: Uuid,
    file_id: Uuid,
    target_kind: String,
    target_id: Uuid,
    provider_code: String,
    storage_class: String,
    object_key: String,
    content_type: String,
    namespace_fingerprint: Option<String>,
    row_revision: i64,
    lease_expires_at: Option<DateTime<Utc>>,
}
fn digest(row: &WatchIdentity) -> String {
    let mut h = Sha256::new();
    for part in [
        "schoolorbit.storage_binding.identity.v1".to_string(),
        row.id.to_string(),
        row.file_id.to_string(),
        row.target_kind.clone(),
        row.target_id.to_string(),
        row.provider_code.clone(),
        row.storage_class.clone(),
        row.object_key.clone(),
        row.content_type.clone(),
    ] {
        h.update((part.len() as u64).to_be_bytes());
        h.update(part.as_bytes());
    }
    hex::encode(h.finalize())
}
fn hash_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn ids(manifest: &BindingManifest) -> Result<Vec<Uuid>, BindingError> {
    if !hash_valid(&manifest.attestation_digest)
        || manifest.watches.is_empty()
        || manifest.watches.len() > 1000
    {
        return Err(BindingError::InvalidManifest);
    }
    let mut result = manifest
        .watches
        .iter()
        .map(|w| w.watch_id)
        .collect::<Vec<_>>();
    result.sort_unstable();
    if result.windows(2).any(|w| w[0] == w[1])
        || manifest.watches.iter().any(|w| {
            !hash_valid(&w.expected_identity_digest)
                || !hash_valid(&w.namespace_fingerprint)
                || w.expected_row_revision < 0
        })
    {
        return Err(BindingError::InvalidManifest);
    }
    Ok(result)
}
async fn require_schema(connection: &mut PgConnection) -> Result<(), BindingError> {
    let exists=DbSession::new(connection).scalar::<bool>("storage_binding.schema", "SELECT EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid=to_regclass('file_storage_cleanup_watches') AND attname='prefer_watch_next' AND NOT attisdropped)").fetch_one().await.map_err(|_|BindingError::Database)?;
    if exists {
        Ok(())
    } else {
        Err(BindingError::SchemaMissing)
    }
}
async fn load(
    connection: &mut PgConnection,
    watch_ids: &[Uuid],
    lock: bool,
) -> Result<Vec<WatchIdentity>, BindingError> {
    let sql = if lock {
        "SELECT id,file_id,target_kind,target_id,provider_code,storage_class,object_key,content_type,namespace_fingerprint,row_revision,lease_expires_at FROM file_storage_cleanup_watches WHERE id=ANY($1) ORDER BY id FOR UPDATE"
    } else {
        "SELECT id,file_id,target_kind,target_id,provider_code,storage_class,object_key,content_type,namespace_fingerprint,row_revision,lease_expires_at FROM file_storage_cleanup_watches WHERE id=ANY($1) ORDER BY id"
    };
    DbSession::new(connection)
        .query_as::<WatchIdentity>("storage_binding.inventory", sql)
        .bind(watch_ids)
        .fetch_all()
        .await
        .map_err(|_| BindingError::Database)
}
async fn readiness(
    connection: &mut PgConnection,
    namespaces: &ConfiguredNamespaces,
) -> Result<NamespaceReadiness, BindingError> {
    require_schema(&mut *connection).await?;
    let (total,unresolved,mismatched)=DbSession::new(connection).query_as::<(i64,i64,i64)>("storage_binding.readiness", "SELECT count(*),count(*) FILTER(WHERE namespace_fingerprint IS NULL),count(*) FILTER(WHERE provider_code <> 'r2' OR (namespace_fingerprint IS NOT NULL AND namespace_fingerprint IS DISTINCT FROM CASE storage_class WHEN 'public' THEN $1 ELSE $2 END)) FROM file_storage_cleanup_watches")
        .bind(namespaces.public.fingerprint()).bind(namespaces.private.fingerprint()).fetch_one().await.map_err(|_|BindingError::Database)?;
    Ok(NamespaceReadiness {
        total,
        unresolved,
        mismatched,
    })
}
/// Read-only gate includes ALL retained watches, even after their business rows were purged.
pub async fn check_namespace_readiness(
    pool: &PgPool,
    namespaces: &ConfiguredNamespaces,
) -> Result<NamespaceReadiness, BindingError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "storage_binding.check")
            .await
            .map_err(|_| BindingError::Database)?;
    let mut transaction = connection
        .begin()
        .await
        .map_err(|_| BindingError::Database)?;
    DbSession::new(&mut *transaction)
        .query("storage_binding.read_only", "SET TRANSACTION READ ONLY")
        .execute()
        .await
        .map_err(|_| BindingError::Database)?;
    let result = readiness(&mut transaction, namespaces).await?;
    transaction
        .commit()
        .await
        .map_err(|_| BindingError::Database)?;
    Ok(result)
}
/// Produce only watch IDs/digests/revisions, never raw locators. Operator attestation is explicit.
pub async fn prepare_binding_manifest(
    pool: &PgPool,
    watch_ids: &[Uuid],
    attestation_digest: String,
    namespaces: &ConfiguredNamespaces,
) -> Result<BindingManifest, BindingError> {
    if watch_ids.is_empty() || watch_ids.len() > 1000 || !hash_valid(&attestation_digest) {
        return Err(BindingError::InvalidManifest);
    }
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "storage_binding.plan")
            .await
            .map_err(|_| BindingError::Database)?;
    let mut transaction = connection
        .begin()
        .await
        .map_err(|_| BindingError::Database)?;
    DbSession::new(&mut *transaction)
        .query("storage_binding.read_only", "SET TRANSACTION READ ONLY")
        .execute()
        .await
        .map_err(|_| BindingError::Database)?;
    require_schema(&mut transaction).await?;
    let rows = load(&mut transaction, watch_ids, false).await?;
    if rows.len() != watch_ids.len() {
        return Err(BindingError::ChangedWatch);
    }
    let mut watches = Vec::with_capacity(rows.len());
    for row in rows {
        if row.namespace_fingerprint.is_some() {
            return Err(BindingError::ChangedWatch);
        }
        watches.push(BindingEntry {
            watch_id: row.id,
            expected_identity_digest: digest(&row),
            expected_row_revision: row.row_revision,
            namespace_fingerprint: namespaces.expected(&row.storage_class)?.to_string(),
        });
    }
    let manifest = BindingManifest {
        attestation_digest,
        watches,
    };
    ids(&manifest)?;
    transaction
        .commit()
        .await
        .map_err(|_| BindingError::Database)?;
    Ok(manifest)
}
fn validate(
    rows: &[WatchIdentity],
    manifest: &BindingManifest,
    namespaces: &ConfiguredNamespaces,
    now: DateTime<Utc>,
) -> Result<(), BindingError> {
    if rows.len() != manifest.watches.len() {
        return Err(BindingError::ChangedWatch);
    }
    for row in rows {
        let entry = manifest
            .watches
            .iter()
            .find(|w| w.watch_id == row.id)
            .ok_or(BindingError::ChangedWatch)?;
        if row.provider_code != "r2"
            || entry.namespace_fingerprint != namespaces.expected(&row.storage_class)?
        {
            return Err(BindingError::Configuration);
        }
        if row.row_revision != entry.expected_row_revision
            || digest(row) != entry.expected_identity_digest
            || row.namespace_fingerprint.is_some()
        {
            return Err(BindingError::ChangedWatch);
        }
        if row.lease_expires_at.is_some_and(|expiry| expiry > now) {
            return Err(BindingError::ActiveClaim);
        }
    }
    Ok(())
}
/// Dry-run is read-only; apply is owner-authorized and validates the entire manifest before mutation.
pub async fn bind_attested_namespaces(
    pool: &PgPool,
    manifest: &BindingManifest,
    namespaces: &ConfiguredNamespaces,
    apply: bool,
) -> Result<usize, BindingError> {
    let watch_ids = ids(manifest)?;
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "storage_binding.attest")
            .await
            .map_err(|_| BindingError::Database)?;
    let mut transaction = connection
        .begin()
        .await
        .map_err(|_| BindingError::Database)?;
    if !apply {
        DbSession::new(&mut *transaction)
            .query("storage_binding.read_only", "SET TRANSACTION READ ONLY")
            .execute()
            .await
            .map_err(|_| BindingError::Database)?;
    }
    require_schema(&mut transaction).await?;
    if apply {
        let owner=DbSession::new(&mut *transaction).scalar::<bool>("storage_binding.owner", "SELECT pg_has_role(current_user,(SELECT relowner FROM pg_class WHERE oid='file_storage_cleanup_watches'::regclass),'USAGE')")
            .fetch_one().await.map_err(|_|BindingError::Database)?;
        if !owner {
            return Err(BindingError::OwnerRequired);
        }
        // No watch->file inversion: lock surviving file/operation rows FIRST.
        let initial = load(&mut transaction, &watch_ids, false).await?;
        let mut file_ids = initial.iter().map(|w| w.file_id).collect::<Vec<_>>();
        file_ids.sort_unstable();
        file_ids.dedup();
        DbSession::new(&mut *transaction)
            .query(
                "storage_binding.files_lock",
                "SELECT id FROM files WHERE id=ANY($1) ORDER BY id FOR UPDATE",
            )
            .bind(&file_ids)
            .execute()
            .await
            .map_err(|_| BindingError::Database)?;
        DbSession::new(&mut *transaction)
            .query(
                "storage_binding.operations_lock",
                "SELECT id FROM file_operations WHERE file_id=ANY($1) ORDER BY id FOR UPDATE",
            )
            .bind(&file_ids)
            .execute()
            .await
            .map_err(|_| BindingError::Database)?;
    }
    let rows = load(&mut transaction, &watch_ids, apply).await?;
    let now = DbSession::new(&mut *transaction)
        .scalar::<DateTime<Utc>>("storage_binding.wall_time", "SELECT statement_timestamp()")
        .fetch_one()
        .await
        .map_err(|_| BindingError::Database)?;
    validate(&rows, manifest, namespaces, now)?;
    let file_ids = rows.iter().map(|r| r.file_id).collect::<Vec<_>>();
    let active=DbSession::new(&mut *transaction).scalar::<bool>("storage_binding.active_write","SELECT EXISTS(SELECT 1 FROM file_operations WHERE file_id=ANY($1) AND status='leased' AND lease_expires_at>statement_timestamp())")
        .bind(file_ids).fetch_one().await.map_err(|_|BindingError::Database)?;
    if active {
        return Err(BindingError::ActiveClaim);
    }
    if apply {
        DbSession::new(&mut *transaction)
            .scalar::<String>(
                "storage_binding.attestation",
                "SELECT set_config('schoolorbit.storage_binding_attestation',$1,true)",
            )
            .bind(&manifest.attestation_digest)
            .fetch_one()
            .await
            .map_err(|_| BindingError::Database)?;
        for row in &rows {
            let entry = manifest
                .watches
                .iter()
                .find(|w| w.watch_id == row.id)
                .ok_or(BindingError::ChangedWatch)?;
            let changed=DbSession::new(&mut *transaction).query("storage_binding.apply","UPDATE file_storage_cleanup_watches SET namespace_fingerprint=$2,namespace_bound_at=statement_timestamp(),binding_evidence_digest=$3 WHERE id=$1 AND row_revision=$4 AND namespace_fingerprint IS NULL")
                .bind(row.id).bind(&entry.namespace_fingerprint).bind(&manifest.attestation_digest).bind(entry.expected_row_revision)
                .execute().await.map_err(|_|BindingError::Database)?.rows_affected();
            if changed != 1 {
                return Err(BindingError::ChangedWatch);
            }
        }
    }
    transaction
        .commit()
        .await
        .map_err(|_| BindingError::Database)?;
    Ok(rows.len())
}
