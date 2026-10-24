//! Authenticated all-tenant release operations, independent of server/runtime state.
use school_database::DbSession;
use school_file_platform::{
    namespace_binding::{self, BindingError, BindingManifest, ConfiguredNamespaces},
    r2_storage_provider::R2StorageRouting,
};
use school_tenancy::{AdminClient, AdminClientConfig, OperationalTenant};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{env, fs::OpenOptions, io::Write};
use uuid::Uuid;

fn failure() -> BindingError {
    BindingError::Database
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
async fn identity(
    connection: &mut PgConnection,
    tenant: Uuid,
    require_owner: bool,
) -> Result<(), BindingError> {
    let (owner,marker,authority):(String,Option<String>,bool)=DbSession::new(connection).query_as("rollout.database_identity","SELECT r.rolname,shobj_description(r.oid,'pg_authid'),pg_has_role(current_user,r.oid,'USAGE') FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database()")
        .fetch_one().await.map_err(|_|failure())?;
    if owner != format!("so_{}_owner", tenant.simple())
        || marker.as_deref() != Some(format!("schoolorbit:{tenant}:migration").as_str())
    {
        return Err(BindingError::Configuration);
    }
    if require_owner && !authority {
        return Err(BindingError::OwnerRequired);
    }
    Ok(())
}
async fn cutovers(connection: &mut PgConnection) -> Result<(), BindingError> {
    if !school_academic_core::reconciliation::read_academic_core_cleanup_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|_| failure())?
    .completed
    {
        return Err(failure());
    }
    if !school_academic_results::services::read_gradebook_results_cutover_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|_| failure())?
    .completed
    {
        return Err(failure());
    }
    if !school_academic_delivery::services::versions::read_cutover_audit_on_connection(
        &mut *connection,
    )
    .await
    .map_err(|_| failure())?
    .completed
    {
        return Err(failure());
    }
    if !school_staff::profile_cutover::read_profile_cutover_on_connection(connection)
        .await
        .map_err(|_| failure())?
        .passed
    {
        return Err(failure());
    }
    Ok(())
}
async fn verify_pool(
    pool: &PgPool,
    tenant: Uuid,
    owner: bool,
    full_schema: bool,
) -> Result<(), BindingError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "rollout.verify_database")
            .await
            .map_err(|_| failure())?;
    identity(&mut connection, tenant, owner).await?;
    if full_schema {
        school_migrations::verify_tenant_schema(&mut *connection)
            .await
            .map_err(|_| BindingError::SchemaMissing)?;
        cutovers(&mut connection).await?;
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
fn output(value: &impl Serialize) -> Result<(), BindingError> {
    println!("{}", serde_json::to_string(value).map_err(|_| failure())?);
    Ok(())
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
    let mut failed = 0;
    for tenant in &tenants {
        let mut applied_version = None;
        let result = async {
            let url = admin
                .get_operational_migration_database_url(&tenant.subdomain)
                .await
                .map_err(|_| failure())?;
            let pool = school_tenancy::connect_privileged_pool(&url, 1)
                .await
                .map_err(|_| failure())?;
            let migration = async {
                verify_pool(pool.as_ref(), tenant.id, true, false).await?;
                let migrated = school_migrations::run_tenant_migrations(pool.as_ref()).await;
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
                migrated.map_err(|_| failure())?;
                verify_pool(pool.as_ref(), tenant.id, true, true).await
            }
            .await;
            pool.close().await;
            migration?;
            let runtime = school_tenancy::connect_privileged_pool(&tenant.database_url, 1)
                .await
                .map_err(|_| failure())?;
            let verified = verify_pool(runtime.as_ref(), tenant.id, false, true).await;
            runtime.close().await;
            verified?;
            let version = i32::try_from(school_migrations::latest_migration_version())
                .map_err(|_| failure())?;
            admin
                .update_migration_status(&tenant.subdomain, version, "migrated", None)
                .await
                .map_err(|_| failure())?;
            Ok::<_, BindingError>(())
        }
        .await;
        if result.is_ok() {
            migrated += 1;
        } else {
            failed += 1;
            if let Some(version) = applied_version.and_then(|version| i32::try_from(version).ok()) {
                let _ = admin
                    .update_migration_status(
                        &tenant.subdomain,
                        version,
                        "failed",
                        Some("tenant_migration_failed"),
                    )
                    .await;
            }
            // If no exact current history could be verified, retain prior Admin metadata.
        }
    }
    unchanged(&admin, &tenants).await?;
    output(
        &serde_json::json!({"schema_version":1,"tenants":tenants.len(),"migrated":migrated,"failed":failed,"latest_version":school_migrations::latest_migration_version()}),
    )?;
    Ok(failed == 0)
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
