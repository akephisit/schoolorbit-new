//! Authenticated all-tenant release operations, independent of server/runtime state.
use super::cli_output::json as output;
use school_database::DbSession;
use school_file_platform::{
    namespace_binding::{self, BindingError, BindingManifest, ConfiguredNamespaces},
    r2_storage_provider::R2StorageRouting,
};
use school_migrations::{MigrationErrorClass, MigrationStage};
use school_tenancy::{AdminClient, AdminClientConfig, OperationalTenant};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{collections::BTreeMap, env, fs::OpenOptions, io::Write};
use uuid::Uuid;

fn failure() -> BindingError {
    BindingError::Database
}

#[derive(Clone, Copy)]
enum FailureStage {
    MigrationUrl,
    OwnerConnection,
    OwnerIdentity,
    MigrationIdentity,
    MigrationExecution,
    PermissionSync,
    RuntimeGrants,
    OwnerSchema,
    OwnerAcademicCoreCutover,
    OwnerGradebookResultsCutover,
    OwnerDeliveryTimetableCutover,
    OwnerPublicationRevisionsCutover,
    OwnerStaffProfileCutover,
    RuntimeConnection,
    RuntimeIdentity,
    RuntimeSchema,
    RuntimeAcademicCoreCutover,
    RuntimeGradebookResultsCutover,
    RuntimeDeliveryTimetableCutover,
    RuntimePublicationRevisionsCutover,
    RuntimeStaffProfileCutover,
    AdminStatus,
}
impl FailureStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MigrationUrl => "migration_url",
            Self::OwnerConnection => "owner_connection",
            Self::OwnerIdentity => "owner_identity",
            Self::MigrationIdentity => "migration_identity",
            Self::MigrationExecution => "migration_execution",
            Self::PermissionSync => "permission_sync",
            Self::RuntimeGrants => "runtime_grants",
            Self::OwnerSchema => "owner_schema",
            Self::OwnerAcademicCoreCutover => "owner_academic_core_cutover",
            Self::OwnerGradebookResultsCutover => "owner_gradebook_results_cutover",
            Self::OwnerDeliveryTimetableCutover => "owner_delivery_timetable_cutover",
            Self::OwnerPublicationRevisionsCutover => "owner_publication_revisions_cutover",
            Self::OwnerStaffProfileCutover => "owner_staff_profile_cutover",
            Self::RuntimeConnection => "runtime_connection",
            Self::RuntimeIdentity => "runtime_identity",
            Self::RuntimeSchema => "runtime_schema",
            Self::RuntimeAcademicCoreCutover => "runtime_academic_core_cutover",
            Self::RuntimeGradebookResultsCutover => "runtime_gradebook_results_cutover",
            Self::RuntimeDeliveryTimetableCutover => "runtime_delivery_timetable_cutover",
            Self::RuntimePublicationRevisionsCutover => "runtime_publication_revisions_cutover",
            Self::RuntimeStaffProfileCutover => "runtime_staff_profile_cutover",
            Self::AdminStatus => "admin_status",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct FailureDiagnostic {
    stage: &'static str,
    version: Option<i64>,
    error_class: &'static str,
    sqlstate: Option<String>,
    code: &'static str,
}
#[derive(Debug)]
struct RolloutFailure {
    diagnostic: FailureDiagnostic,
    binding: BindingError,
}
impl RolloutFailure {
    fn plain(stage: FailureStage, binding: BindingError) -> Self {
        let class = match binding {
            BindingError::Configuration
            | BindingError::OwnerRequired
            | BindingError::SchemaMissing => MigrationErrorClass::Configuration,
            _ => MigrationErrorClass::Other,
        };
        Self {
            diagnostic: FailureDiagnostic {
                stage: stage.as_str(),
                version: None,
                error_class: class.as_str(),
                sqlstate: None,
                code: "unclassified",
            },
            binding,
        }
    }
    fn database(stage: FailureStage, error: &sqlx::Error) -> Self {
        let mut failure = Self::plain(stage, BindingError::Database);
        failure.diagnostic.error_class = MigrationErrorClass::from_sqlx(error).as_str();
        failure.diagnostic.sqlstate = school_migrations::diagnostic_sqlstate(error);
        failure
    }
    fn audit(stage: FailureStage, error: &school_errors::AppError) -> Self {
        let mut failure = Self::plain(stage, BindingError::Database);
        failure.diagnostic.sqlstate = error
            .database_code()
            .as_deref()
            .and_then(school_migrations::validated_sqlstate);
        if failure.diagnostic.sqlstate.is_some() {
            failure.diagnostic.error_class = MigrationErrorClass::Database.as_str();
        }
        failure
    }
    fn migration(error: school_migrations::MigrationFailure) -> Self {
        let diagnostic = error.diagnostic;
        let stage = match diagnostic.stage {
            MigrationStage::Identity => FailureStage::MigrationIdentity,
            MigrationStage::Execution => FailureStage::MigrationExecution,
            MigrationStage::PermissionSync => FailureStage::PermissionSync,
            MigrationStage::RuntimeGrants => FailureStage::RuntimeGrants,
        };
        Self {
            diagnostic: FailureDiagnostic {
                stage: stage.as_str(),
                version: diagnostic.version,
                error_class: diagnostic.error_class.as_str(),
                sqlstate: diagnostic.sqlstate,
                code: diagnostic.code,
            },
            binding: BindingError::Database,
        }
    }
}

#[derive(Serialize)]
struct FailureCount {
    #[serde(flatten)]
    diagnostic: FailureDiagnostic,
    count: usize,
}
#[derive(Default)]
struct FailureHistogram {
    details: BTreeMap<FailureDiagnostic, usize>,
    overflow_count: usize,
}
impl FailureHistogram {
    fn record(&mut self, diagnostic: FailureDiagnostic) {
        if let Some(count) = self.details.get_mut(&diagnostic) {
            *count += 1;
        } else if self.details.len() < 64 {
            self.details.insert(diagnostic, 1);
        } else {
            self.overflow_count += 1;
        }
    }
    fn into_report(self, tenants: usize, migrated: usize) -> MigrationReport {
        let failed = self.details.values().sum::<usize>() + self.overflow_count;
        MigrationReport {
            schema_version: 1,
            tenants,
            migrated,
            failed,
            latest_version: school_migrations::latest_migration_version(),
            failures: self
                .details
                .into_iter()
                .map(|(diagnostic, count)| FailureCount { diagnostic, count })
                .collect(),
            overflow_count: self.overflow_count,
        }
    }
}
#[derive(Serialize)]
struct MigrationReport {
    schema_version: u8,
    tenants: usize,
    migrated: usize,
    failed: usize,
    latest_version: i64,
    failures: Vec<FailureCount>,
    overflow_count: usize,
}

async fn record_failed_tenant<F>(
    failures: &mut FailureHistogram,
    error: RolloutFailure,
    update_status: F,
) where
    F: std::future::Future<Output = Result<(), String>>,
{
    // Keep the original cause even when secondary metadata reporting fails.
    failures.record(error.diagnostic);
    if update_status.await.is_err() {
        tracing::warn!(
            operation = "tenant_migration_status",
            reason = "failure_status_update_failed"
        );
    }
}
fn admin_client() -> Result<AdminClient, BindingError> {
    Ok(AdminClient::new(
        env::var("BACKEND_ADMIN_URL").map_err(|_| BindingError::Configuration)?,
        env::var("INTERNAL_API_SECRET").map_err(|_| BindingError::Configuration)?,
        AdminClientConfig::from_env().map_err(|_| BindingError::Configuration)?,
    ))
}
fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn inventory_digest(tenants: &[OperationalTenant]) -> Result<String, BindingError> {
    let mut hash = Sha256::new();
    for tenant in tenants {
        let route =
            url::Url::parse(&tenant.database_url).map_err(|_| BindingError::Configuration)?;
        if !matches!(route.scheme(), "postgres" | "postgresql")
            || route.host_str().is_none()
            || route.path().len() < 2
        {
            return Err(BindingError::Configuration);
        }
        // Stable physical routing only: passwords, URL queries and credentials are excluded.
        for value in [
            tenant.id.to_string(),
            tenant.subdomain.clone(),
            tenant.status.clone(),
            route.host_str().unwrap_or_default().to_ascii_lowercase(),
            route.port().unwrap_or(5432).to_string(),
            route.path().to_string(),
        ] {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
    }
    Ok(hex::encode(hash.finalize()))
}
async fn snapshot(admin: &AdminClient) -> Result<Vec<OperationalTenant>, BindingError> {
    admin
        .list_operational_tenants()
        .await
        .map_err(|_| failure())
}
async fn unchanged(admin: &AdminClient, before: &[OperationalTenant]) -> Result<(), BindingError> {
    // Equality additionally catches private credential/route changes; nothing secret is hashed/logged.
    if snapshot(admin).await? == before {
        Ok(())
    } else {
        Err(BindingError::ChangedWatch)
    }
}
async fn identity_detailed(
    connection: &mut PgConnection,
    tenant: Uuid,
    require_owner: bool,
    stage: FailureStage,
) -> Result<(), RolloutFailure> {
    let (owner,marker,authority):(String,Option<String>,bool)=DbSession::new(connection).query_as("rollout.database_identity","SELECT r.rolname,shobj_description(r.oid,'pg_authid'),pg_has_role(current_user,r.oid,'USAGE') FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database()")
        .fetch_one().await.map_err(|error|RolloutFailure::database(stage, &error))?;
    if owner != format!("so_{}_owner", tenant.simple())
        || marker.as_deref() != Some(format!("schoolorbit:{tenant}:migration").as_str())
    {
        return Err(RolloutFailure::plain(stage, BindingError::Configuration));
    }
    if require_owner && !authority {
        return Err(RolloutFailure::plain(stage, BindingError::OwnerRequired));
    }
    Ok(())
}
async fn cutovers_detailed(
    connection: &mut PgConnection,
    owner: bool,
) -> Result<(), RolloutFailure> {
    let stage = if owner {
        FailureStage::OwnerAcademicCoreCutover
    } else {
        FailureStage::RuntimeAcademicCoreCutover
    };
    if !school_academic_core::reconciliation::read_academic_core_cleanup_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|error| RolloutFailure::audit(stage, &error))?
    .completed
    {
        return Err(RolloutFailure::plain(stage, BindingError::Database));
    }
    let stage = if owner {
        FailureStage::OwnerGradebookResultsCutover
    } else {
        FailureStage::RuntimeGradebookResultsCutover
    };
    if !school_academic_results::services::read_gradebook_results_cutover_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|error| RolloutFailure::audit(stage, &error))?
    .completed
    {
        return Err(RolloutFailure::plain(stage, BindingError::Database));
    }
    let stage = if owner {
        FailureStage::OwnerDeliveryTimetableCutover
    } else {
        FailureStage::RuntimeDeliveryTimetableCutover
    };
    if !school_academic_delivery::services::versions::read_cutover_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|error| RolloutFailure::audit(stage, &error))?
    .completed
    {
        return Err(RolloutFailure::plain(stage, BindingError::Database));
    }
    let stage = if owner {
        FailureStage::OwnerPublicationRevisionsCutover
    } else {
        FailureStage::RuntimePublicationRevisionsCutover
    };
    if !school_academic_delivery::services::publication::read_revisions_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|error| RolloutFailure::audit(stage, &error))?
    .completed
    {
        return Err(RolloutFailure::plain(stage, BindingError::Database));
    }
    let stage = if owner {
        FailureStage::OwnerStaffProfileCutover
    } else {
        FailureStage::RuntimeStaffProfileCutover
    };
    if !school_staff::profile_cutover::read_profile_cutover_on_connection(connection)
        .await
        .map_err(|error| RolloutFailure::audit(stage, &error))?
        .passed
    {
        return Err(RolloutFailure::plain(stage, BindingError::Database));
    }
    Ok(())
}
async fn verify_pool(
    pool: &PgPool,
    tenant: Uuid,
    owner: bool,
    full_schema: bool,
) -> Result<(), BindingError> {
    verify_pool_detailed(pool, tenant, owner, full_schema)
        .await
        .map_err(|failure| failure.binding)
}
async fn verify_pool_detailed(
    pool: &PgPool,
    tenant: Uuid,
    owner: bool,
    full_schema: bool,
) -> Result<(), RolloutFailure> {
    let connection_stage = if owner {
        FailureStage::OwnerConnection
    } else {
        FailureStage::RuntimeConnection
    };
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "rollout.verify_database")
            .await
            .map_err(|error| RolloutFailure::database(connection_stage, &error))?;
    let identity_stage = if owner {
        FailureStage::OwnerIdentity
    } else {
        FailureStage::RuntimeIdentity
    };
    identity_detailed(&mut connection, tenant, owner, identity_stage).await?;
    if full_schema {
        school_migrations::verify_tenant_schema(&mut *connection)
            .await
            .map_err(|error| {
                let stage = if owner {
                    FailureStage::OwnerSchema
                } else {
                    FailureStage::RuntimeSchema
                };
                let mut failure = RolloutFailure::plain(stage, BindingError::SchemaMissing);
                if error == "Tenant migration history unavailable" {
                    failure.diagnostic.error_class = MigrationErrorClass::Database.as_str();
                }
                failure
            })?;
        cutovers_detailed(&mut connection, owner).await?;
    }
    Ok(())
}
#[derive(Serialize)]
struct RoutingOutput {
    schema_version: u8,
    public_namespace_sha256: String,
    private_namespace_sha256: String,
}
#[derive(Serialize)]
struct Context {
    schema_version: u8,
    tenants: usize,
    inventory_sha256: String,
    public_namespace_sha256: String,
    private_namespace_sha256: String,
}
#[derive(Serialize)]
pub struct CheckResult {
    schema_version: u8,
    tenants: usize,
    checked: usize,
    failed: usize,
    watches: i64,
    unresolved: i64,
    mismatched: i64,
}
impl CheckResult {
    pub fn ready(&self) -> bool {
        self.checked == self.tenants
            && self.failed == 0
            && self.unresolved == 0
            && self.mismatched == 0
    }
}
async fn check_snapshot(
    tenants: &[OperationalTenant],
    namespaces: &ConfiguredNamespaces,
) -> Result<CheckResult, BindingError> {
    let mut result = CheckResult {
        schema_version: 1,
        tenants: tenants.len(),
        checked: 0,
        failed: 0,
        watches: 0,
        unresolved: 0,
        mismatched: 0,
    };
    for tenant in tenants {
        let observation = match school_tenancy::connect_privileged_pool(&tenant.database_url, 1)
            .await
        {
            Ok(pool) => {
                let result = async {
                    verify_pool(pool.as_ref(), tenant.id, false, true).await?;
                    namespace_binding::check_namespace_readiness(pool.as_ref(), namespaces).await
                }
                .await;
                pool.close().await;
                result
            }
            Err(_) => Err(failure()),
        };
        match observation {
            Ok(observation) => {
                result.checked += 1;
                result.watches += observation.total;
                result.unresolved += observation.unresolved;
                result.mismatched += observation.mismatched;
            }
            Err(_) => result.failed += 1,
        }
    }
    Ok(result)
}
/// Required pre-start read-only barrier: no task, listener, cache warmer or writer is started here.
pub async fn startup_check(admin: &AdminClient) -> Result<(), BindingError> {
    let routing = R2StorageRouting::from_env().map_err(|_| BindingError::Configuration)?;
    let namespaces = ConfiguredNamespaces::from_r2_routing(&routing)?;
    let tenants = snapshot(admin).await?;
    let result = check_snapshot(&tenants, &namespaces).await?;
    unchanged(admin, &tenants).await?;
    if result.ready() {
        Ok(())
    } else {
        Err(BindingError::SchemaMissing)
    }
}

pub async fn migrate_tenants() -> Result<bool, BindingError> {
    let admin = admin_client()?;
    let tenants = snapshot(&admin).await?;
    let mut migrated = 0;
    let mut failures = FailureHistogram::default();
    for tenant in &tenants {
        let mut applied_version = None;
        let result = async {
            let url = admin
                .get_operational_migration_database_url(&tenant.subdomain)
                .await
                .map_err(|_| {
                    RolloutFailure::plain(FailureStage::MigrationUrl, BindingError::Database)
                })?;
            let pool = school_tenancy::connect_privileged_pool(&url, 1)
                .await
                .map_err(|_| {
                    RolloutFailure::plain(FailureStage::OwnerConnection, BindingError::Database)
                })?;
            let migration = async {
                verify_pool_detailed(pool.as_ref(), tenant.id, true, false).await?;
                let migrated =
                    school_migrations::run_tenant_migrations_detailed(pool.as_ref()).await;
                // A grant or later audit can fail after the migration transaction committed.
                // Record only a canonical checksum-verified version, never a fabricated zero.
                if let Ok(mut connection) = school_tenancy::TenantPool::observed_checkout_pool(
                    pool.as_ref(),
                    "rollout.applied_history",
                )
                .await
                {
                    if let Ok(version) =
                        school_migrations::verify_tenant_schema(&mut *connection).await
                    {
                        applied_version = Some(version);
                    }
                }
                migrated.map_err(RolloutFailure::migration)?;
                verify_pool_detailed(pool.as_ref(), tenant.id, true, true).await
            }
            .await;
            pool.close().await;
            migration?;
            let runtime = school_tenancy::connect_privileged_pool(&tenant.database_url, 1)
                .await
                .map_err(|_| {
                    RolloutFailure::plain(FailureStage::RuntimeConnection, BindingError::Database)
                })?;
            let verified = verify_pool_detailed(runtime.as_ref(), tenant.id, false, true).await;
            runtime.close().await;
            verified?;
            let version =
                i32::try_from(school_migrations::latest_migration_version()).map_err(|_| {
                    RolloutFailure::plain(FailureStage::AdminStatus, BindingError::Database)
                })?;
            admin
                .update_migration_status(&tenant.subdomain, version, "migrated", None)
                .await
                .map_err(|_| {
                    RolloutFailure::plain(FailureStage::AdminStatus, BindingError::Database)
                })?;
            Ok::<_, RolloutFailure>(())
        }
        .await;
        match result {
            Ok(()) => migrated += 1,
            Err(error) => {
                record_failed_tenant(&mut failures, error, async {
                    if let Some(version) =
                        applied_version.and_then(|version| i32::try_from(version).ok())
                    {
                        admin
                            .update_migration_status(
                                &tenant.subdomain,
                                version,
                                "failed",
                                Some("tenant_migration_failed"),
                            )
                            .await
                    } else {
                        // No exact current history: retain prior Admin metadata.
                        Ok(())
                    }
                })
                .await;
            }
        }
    }
    unchanged(&admin, &tenants).await?;
    let report = failures.into_report(tenants.len(), migrated);
    output(&report)?;
    Ok(report.failed == 0)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attestation {
    schema_version: u8,
    evidence_digest: String,
    expected_inventory_sha256: String,
    expected_public_namespace_sha256: String,
    expected_private_namespace_sha256: String,
    scope: String,
}
#[derive(Serialize)]
struct PlannedTenant {
    tenant_id: Uuid,
    subdomain: String,
    manifests: Vec<BindingManifest>,
}
#[derive(Serialize)]
struct PlannedAttestation {
    schema_version: u8,
    inventory_sha256: String,
    tenants: Vec<PlannedTenant>,
}

pub async fn namespace_operation(mode: &str) -> Result<bool, BindingError> {
    let routing = R2StorageRouting::from_env().map_err(|_| BindingError::Configuration)?;
    let (public, private) = routing
        .namespace_fingerprints()
        .map_err(|_| BindingError::Configuration)?;
    if mode == "routing-fingerprints" {
        output(&RoutingOutput {
            schema_version: 1,
            public_namespace_sha256: public,
            private_namespace_sha256: private,
        })?;
        return Ok(true);
    }
    let admin = admin_client()?;
    let tenants = snapshot(&admin).await?;
    let digest = inventory_digest(&tenants)?;
    if mode == "context-admin" {
        unchanged(&admin, &tenants).await?;
        output(&Context {
            schema_version: 1,
            tenants: tenants.len(),
            inventory_sha256: digest,
            public_namespace_sha256: public,
            private_namespace_sha256: private,
        })?;
        return Ok(true);
    }
    let namespaces = ConfiguredNamespaces::from_r2_routing(&routing)?;
    if mode == "check-admin" {
        let result = check_snapshot(&tenants, &namespaces).await?;
        unchanged(&admin, &tenants).await?;
        output(&result)?;
        return Ok(result.ready());
    }
    if mode != "attest-admin" {
        return Err(BindingError::InvalidManifest);
    }
    let value = env::var("FILE_STORAGE_NAMESPACE_ATTESTATION_JSON")
        .map_err(|_| BindingError::InvalidManifest)?;
    if value.len() > 4096 {
        return Err(BindingError::InvalidManifest);
    }
    let attestation: Attestation =
        serde_json::from_str(&value).map_err(|_| BindingError::InvalidManifest)?;
    if attestation.schema_version != 1
        || attestation.scope != "all_unbound"
        || !hash_valid(&attestation.evidence_digest)
        || !hash_valid(&attestation.expected_inventory_sha256)
        || !hash_valid(&attestation.expected_public_namespace_sha256)
        || !hash_valid(&attestation.expected_private_namespace_sha256)
        || attestation.expected_inventory_sha256 != digest
        || attestation.expected_public_namespace_sha256 != public
        || attestation.expected_private_namespace_sha256 != private
    {
        return Err(BindingError::InvalidManifest);
    }
    let mut plan = PlannedAttestation {
        schema_version: 1,
        inventory_sha256: digest,
        tenants: Vec::new(),
    };
    for tenant in &tenants {
        let url = admin
            .get_operational_migration_database_url(&tenant.subdomain)
            .await
            .map_err(|_| failure())?;
        let pool = school_tenancy::connect_privileged_pool(&url, 1)
            .await
            .map_err(|_| failure())?;
        let planned = async {
            verify_pool(pool.as_ref(), tenant.id, true, true).await?;
            let ids = {
                let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                    pool.as_ref(),
                    "rollout.unbound_inventory",
                )
                .await
                .map_err(|_| failure())?;
                namespace_binding::unbound_watch_ids_on_connection(&mut connection).await?
            };
            let mut manifests = Vec::new();
            for chunk in ids.chunks(1000) {
                let manifest = namespace_binding::prepare_binding_manifest(
                    pool.as_ref(),
                    chunk,
                    attestation.evidence_digest.clone(),
                    &namespaces,
                )
                .await?;
                // Validate every chunk/tenant before ANY apply; final apply rechecks each exact revision.
                namespace_binding::bind_attested_namespaces(
                    pool.as_ref(),
                    &manifest,
                    &namespaces,
                    false,
                )
                .await?;
                manifests.push(manifest);
            }
            Ok::<_, BindingError>(PlannedTenant {
                tenant_id: tenant.id,
                subdomain: tenant.subdomain.clone(),
                manifests,
            })
        }
        .await;
        pool.close().await;
        plan.tenants.push(planned?);
    }
    unchanged(&admin, &tenants).await?;
    let directory = std::path::PathBuf::from(
        env::var_os("FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY")
            .ok_or(BindingError::Configuration)?,
    );
    let metadata =
        std::fs::symlink_metadata(&directory).map_err(|_| BindingError::Configuration)?;
    if !directory.is_absolute() || !metadata.is_dir() {
        return Err(BindingError::Configuration);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(BindingError::Configuration);
        }
    }
    let path = directory.join(format!("namespace-attestation-{}.json", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| BindingError::Configuration)?;
    file.write_all(&serde_json::to_vec(&plan).map_err(|_| failure())?)
        .map_err(|_| failure())?;
    file.sync_all().map_err(|_| failure())?;
    std::fs::File::open(&directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| failure())?;
    let mut applied = 0;
    for planned in &plan.tenants {
        let url = admin
            .get_operational_migration_database_url(&planned.subdomain)
            .await
            .map_err(|_| failure())?;
        let pool = school_tenancy::connect_privileged_pool(&url, 1)
            .await
            .map_err(|_| failure())?;
        let result = async {
            verify_pool(pool.as_ref(), planned.tenant_id, true, true).await?;
            for manifest in &planned.manifests {
                applied += namespace_binding::bind_attested_namespaces(
                    pool.as_ref(),
                    manifest,
                    &namespaces,
                    true,
                )
                .await?;
            }
            Ok::<_, BindingError>(())
        }
        .await;
        pool.close().await;
        result?;
    }
    unchanged(&admin, &tenants).await?;
    let result = check_snapshot(&tenants, &namespaces).await?;
    unchanged(&admin, &tenants).await?;
    output(
        &serde_json::json!({"schema_version":1,"tenants":tenants.len(),"applied":applied,"ready":result.ready()}),
    )?;
    Ok(result.ready())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_failure_histogram_is_sorted_exact_and_contains_no_tenant_fields() {
        let mut histogram = FailureHistogram::default();
        let primary =
            RolloutFailure::plain(FailureStage::RuntimeSchema, BindingError::SchemaMissing)
                .diagnostic;
        histogram.record(primary.clone());
        histogram.record(
            RolloutFailure::plain(FailureStage::MigrationUrl, BindingError::Database).diagnostic,
        );
        histogram.record(primary);
        let report = histogram.into_report(4, 1);
        assert_eq!(
            (
                report.tenants,
                report.migrated,
                report.failed,
                report.overflow_count
            ),
            (4, 1, 3, 0)
        );
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 7);
        assert_eq!(json["failures"].as_array().unwrap().len(), 2);
        assert_eq!(json["failures"][0]["stage"], "migration_url");
        assert_eq!(json["failures"][1]["stage"], "runtime_schema");
        assert_eq!(json["failures"][1]["count"], 2);
        for bucket in json["failures"].as_array().unwrap() {
            assert_eq!(bucket.as_object().unwrap().len(), 6);
            assert!(
                bucket.get("tenant").is_none()
                    && bucket.get("subdomain").is_none()
                    && bucket.get("error").is_none()
            );
        }
        let successful = FailureHistogram::default().into_report(2, 2);
        assert_eq!(successful.failed, 0);
        assert!(successful.failures.is_empty());
        assert_eq!(successful.overflow_count, 0);
    }

    #[tokio::test]
    async fn secondary_admin_status_failure_preserves_the_original_primary_diagnostic() {
        let mut histogram = FailureHistogram::default();
        let primary =
            RolloutFailure::plain(FailureStage::RuntimeSchema, BindingError::SchemaMissing);
        record_failed_tenant(&mut histogram, primary, async {
            Err("private-sentinel postgres://credentials".into())
        })
        .await;
        let report = histogram.into_report(1, 0);
        assert_eq!(report.failed, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].diagnostic.stage, "runtime_schema");
        assert!(!serde_json::to_string(&report)
            .unwrap()
            .contains("private-sentinel"));
    }

    #[test]
    fn migration_failure_histogram_caps_details_but_keeps_existing_bucket_counts() {
        let mut histogram = FailureHistogram::default();
        for version in 1..=70 {
            let mut diagnostic =
                RolloutFailure::plain(FailureStage::MigrationExecution, BindingError::Database)
                    .diagnostic;
            diagnostic.version = Some(version);
            histogram.record(diagnostic);
        }
        let mut existing =
            RolloutFailure::plain(FailureStage::MigrationExecution, BindingError::Database)
                .diagnostic;
        existing.version = Some(1);
        histogram.record(existing);
        let report = histogram.into_report(71, 0);
        assert_eq!(report.failures.len(), 64);
        assert_eq!(report.overflow_count, 6);
        assert_eq!(report.failed, 71);
        assert_eq!(report.failures[0].count, 2);
        assert_eq!(
            report
                .failures
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>()
                + report.overflow_count,
            report.failed
        );
    }

    #[tokio::test]
    async fn canonical_migration_refusal_keeps_native_version_code_and_legacy_message() {
        use school_test_db::cutover_test_support::{
            apply_migrations_through, apply_phase_b_runtime_migrations,
            seed_academic_cutover_fixture, CutoverFixture,
        };
        let pool = school_test_db::create_named_test_pool("rollout_native_147_refusal").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();
        apply_migrations_through(&pool, 146).await.unwrap();
        sqlx::query("UPDATE academic_years SET status='ready' WHERE year=2025")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id WHERE term.status IN ('active','closing') AND year.status NOT IN ('active','closing')",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        let error = school_migrations::run_tenant_migrations_detailed(&pool)
            .await
            .unwrap_err();
        let failure = RolloutFailure::migration(error);
        assert_eq!(failure.diagnostic.stage, "migration_execution");
        assert_eq!(failure.diagnostic.version, Some(147));
        assert_eq!(failure.diagnostic.error_class, "database");
        assert_eq!(failure.diagnostic.sqlstate.as_deref(), Some("P0001"));
        assert_eq!(
            failure.diagnostic.code,
            "ACADEMIC_CONTEXT_147_RUNNING_CONTEXT_MISMATCH"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
                .fetch_one(&pool)
                .await
                .unwrap(),
            146
        );
        assert_eq!(
            school_migrations::run_tenant_migrations(&pool)
                .await
                .unwrap_err(),
            "Migration failed: database (version 147)"
        );
    }

    #[tokio::test]
    async fn committed_schema_permission_failure_is_not_reported_as_a_migration_refusal() {
        let pool = school_test_db::create_named_test_pool("rollout_post_commit_permission").await;
        school_test_db::run_test_migrations(&pool).await;
        sqlx::raw_sql("CREATE FUNCTION reject_permission_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-sentinel postgres://credentials' USING ERRCODE='42501'; END $$; CREATE TRIGGER reject_permission_fixture BEFORE INSERT OR UPDATE ON permissions FOR EACH ROW EXECUTE FUNCTION reject_permission_fixture();")
            .execute(&pool).await.unwrap();
        let error = school_migrations::run_tenant_migrations_detailed(&pool)
            .await
            .unwrap_err();
        let legacy = error.diagnostic.clone();
        assert_eq!(
            error.into_legacy_message(),
            "Permission sync failed after migrations: database"
        );
        let failure = RolloutFailure::migration(
            school_migrations::run_tenant_migrations_detailed(&pool)
                .await
                .unwrap_err(),
        );
        assert_eq!(legacy.stage, MigrationStage::PermissionSync);
        assert_eq!(failure.diagnostic.stage, "permission_sync");
        assert_eq!(failure.diagnostic.version, None);
        assert_eq!(failure.diagnostic.sqlstate.as_deref(), Some("42501"));
        assert_eq!(failure.diagnostic.code, "unclassified");
        assert_eq!(
            school_migrations::verify_tenant_schema(&pool)
                .await
                .unwrap(),
            school_migrations::latest_migration_version()
        );
        assert!(!serde_json::to_string(&failure.diagnostic)
            .unwrap()
            .contains("private-sentinel"));
    }
    #[test]
    fn inventory_fingerprint_excludes_passwords_but_fences_tenant_identity_and_route() {
        let tenant = OperationalTenant {
            id: Uuid::new_v4(),
            subdomain: "sandbox".into(),
            status: "inactive".into(),
            database_url: "postgresql://runtime:first-secret@localhost:5432/school".into(),
        };
        let original = inventory_digest(std::slice::from_ref(&tenant)).unwrap();
        let mut rotated = tenant.clone();
        rotated.database_url = "postgresql://runtime:second-secret@localhost:5432/school".into();
        assert_eq!(
            original,
            inventory_digest(std::slice::from_ref(&rotated)).unwrap()
        );
        assert!(
            tenant != rotated,
            "private snapshot equality still catches credential drift"
        );
        rotated.database_url = "postgresql://runtime:second-secret@localhost:5432/other".into();
        assert_ne!(original, inventory_digest(&[rotated]).unwrap());
        assert_eq!(
            inventory_digest(&[]).unwrap(),
            hex::encode(Sha256::digest([]))
        );
    }
}
