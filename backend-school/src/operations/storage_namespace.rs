//! Offline explicit namespace attestation. This command never migrates or calls storage.
use school_file_platform::{
    namespace_binding::{self, BindingError, BindingManifest, ConfiguredNamespaces},
    r2_storage_provider::R2StorageRouting,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
};
use uuid::Uuid;

struct Command {
    mode: String,
    manifest: Option<String>,
    output: Option<String>,
    watch_ids: Vec<Uuid>,
    attestation: Option<String>,
    confirmed: bool,
    inventory: Option<String>,
    inventory_digest: Option<String>,
}
fn arguments(arguments: Vec<String>) -> Result<Command, BindingError> {
    let mut args = arguments.into_iter();
    let mode = args.next().ok_or(BindingError::InvalidManifest)?;
    if !matches!(mode.as_str(), "check" | "plan" | "dry-run" | "apply") {
        return Err(BindingError::InvalidManifest);
    }
    let mut command = Command {
        mode,
        manifest: None,
        output: None,
        watch_ids: Vec::new(),
        attestation: None,
        confirmed: false,
        inventory: None,
        inventory_digest: None,
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--inventory" => {
                command.inventory = Some(args.next().ok_or(BindingError::InvalidManifest)?)
            }
            "--expected-inventory-sha256" => {
                command.inventory_digest = Some(args.next().ok_or(BindingError::InvalidManifest)?)
            }
            "--manifest" => {
                command.manifest = Some(args.next().ok_or(BindingError::InvalidManifest)?)
            }
            "--output" => command.output = Some(args.next().ok_or(BindingError::InvalidManifest)?),
            "--watch-id" => command.watch_ids.push(
                args.next()
                    .ok_or(BindingError::InvalidManifest)?
                    .parse()
                    .map_err(|_| BindingError::InvalidManifest)?,
            ),
            "--attestation-digest" => {
                command.attestation = Some(args.next().ok_or(BindingError::InvalidManifest)?)
            }
            "--attest-historical-namespace" => command.confirmed = true,
            _ => return Err(BindingError::InvalidManifest),
        }
    }
    Ok(command)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenantInventory {
    tenants: Vec<TenantEntry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenantEntry {
    tenant_id: Uuid,
    database_url_env: String,
    r2_account_id: String,
    public_bucket_name: String,
    private_bucket_name: String,
}
fn inventory(data: &[u8], expected_digest: &str) -> Result<TenantInventory, BindingError> {
    if data.len() > 1024 * 1024
        || expected_digest.len() != 64
        || hex::encode(Sha256::digest(data)) != expected_digest
    {
        return Err(BindingError::InvalidManifest);
    }
    let inventory: TenantInventory =
        serde_json::from_slice(data).map_err(|_| BindingError::InvalidManifest)?;
    if inventory.tenants.is_empty() || inventory.tenants.len() > 10000 {
        return Err(BindingError::InvalidManifest);
    }
    let mut ids = HashSet::new();
    let mut variables = HashSet::new();
    for tenant in &inventory.tenants {
        if tenant.tenant_id.is_nil()
            || !ids.insert(tenant.tenant_id)
            || !variables.insert(&tenant.database_url_env)
            || !tenant
                .database_url_env
                .starts_with("FILE_STORAGE_BINDING_URL_")
            || !tenant
                .database_url_env
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(BindingError::InvalidManifest);
        }
        R2StorageRouting::from_values(
            &tenant.r2_account_id,
            &tenant.public_bucket_name,
            &tenant.private_bucket_name,
        )
        .map_err(|_| BindingError::Configuration)?;
    }
    Ok(inventory)
}
async fn check_inventory(command: &Command) -> Result<bool, BindingError> {
    if command.manifest.is_some()
        || command.output.is_some()
        || !command.watch_ids.is_empty()
        || command.attestation.is_some()
        || command.confirmed
    {
        return Err(BindingError::InvalidManifest);
    }
    let path = command
        .inventory
        .as_ref()
        .ok_or(BindingError::InvalidManifest)?;
    if fs::metadata(path)
        .map_err(|_| BindingError::InvalidManifest)?
        .len()
        > 1024 * 1024
    {
        return Err(BindingError::InvalidManifest);
    }
    let data = fs::read(path).map_err(|_| BindingError::InvalidManifest)?;
    let inventory = inventory(
        &data,
        command
            .inventory_digest
            .as_deref()
            .ok_or(BindingError::InvalidManifest)?,
    )?;
    // Freeze and validate every explicit tenant URL before checking any database.
    // Inventory completeness comes from the operator's independently trusted digest.
    let mut urls = HashSet::new();
    let mut entries = Vec::new();
    for tenant in inventory.tenants {
        let url = env::var(&tenant.database_url_env).map_err(|_| BindingError::Configuration)?;
        if url.trim().is_empty() || !urls.insert(url.clone()) {
            return Err(BindingError::InvalidManifest);
        }
        let routing = R2StorageRouting::from_values(
            &tenant.r2_account_id,
            &tenant.public_bucket_name,
            &tenant.private_bucket_name,
        )
        .map_err(|_| BindingError::Configuration)?;
        entries.push((
            tenant.tenant_id,
            url,
            ConfiguredNamespaces::from_r2_routing(&routing)?,
        ));
    }
    let total_tenants = entries.len();
    let mut checked = 0;
    let mut failed = 0;
    let mut watches = 0;
    let mut unresolved = 0;
    let mut mismatched = 0;
    let mut actual_identities = HashSet::new();
    for (tenant_id, url, namespaces) in entries {
        let result = match school_tenancy::connect_privileged_pool(&url, 1).await {
            Ok(pool) => {
                let result = async {
                    let identity = {
                        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(pool.as_ref(), "storage_binding.database_identity").await.map_err(|_| BindingError::Database)?;
                        school_migrations::verify_tenant_schema(&mut *connection).await.map_err(|_| BindingError::SchemaMissing)?;
                        school_database::DbSession::new(&mut connection).query_as::<(String, Option<String>)>("storage_binding.database_identity", "SELECT r.rolname,shobj_description(r.oid,'pg_authid') FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database()")
                            .fetch_one().await.map_err(|_| BindingError::Database)?
                    };
                    if identity.0 != format!("so_{}_owner", tenant_id.simple())
                        || identity.1.as_deref() != Some(format!("schoolorbit:{tenant_id}:migration").as_str())
                        || !actual_identities.insert(identity) { return Err(BindingError::Configuration); }
                    namespace_binding::check_namespace_readiness(pool.as_ref(), &namespaces).await
                }.await;
                // All entry outcomes, including identity/checkout failures, drain before advancing.
                pool.close().await;
                result
            }
            Err(_) => Err(BindingError::Database),
        };
        match result {
            Ok(result) => {
                checked += 1;
                watches += result.total;
                unresolved += result.unresolved;
                mismatched += result.mismatched;
            }
            Err(_) => {
                failed += 1;
            }
        }
    }
    println!("storage_namespace_inventory tenants={} checked={} failed={} watches={} unresolved={} mismatched={}",total_tenants,checked,failed,watches,unresolved,mismatched);
    Ok(checked == total_tenants && failed == 0 && unresolved == 0 && mismatched == 0)
}

pub fn valid_arguments(values: &[String]) -> bool {
    if matches!(
        values.first().map(String::as_str),
        Some("context-admin" | "routing-fingerprints" | "check-admin" | "attest-admin")
    ) {
        return values.len() == 1;
    }
    arguments(values.to_vec()).is_ok()
}
pub async fn execute(command_arguments: Vec<String>) -> Result<bool, BindingError> {
    if let Some(mode) = command_arguments.first().map(String::as_str) {
        if matches!(
            mode,
            "context-admin" | "routing-fingerprints" | "check-admin" | "attest-admin"
        ) {
            if command_arguments.len() != 1 {
                return Err(BindingError::InvalidManifest);
            }
            return super::tenant_rollout::namespace_operation(mode).await;
        }
    }
    let command = arguments(command_arguments)?;
    if command.mode == "check" {
        return check_inventory(&command).await;
    }
    if command.inventory.is_some() || command.inventory_digest.is_some() {
        return Err(BindingError::InvalidManifest);
    }
    let routing = R2StorageRouting::from_env().map_err(|_| BindingError::Configuration)?;
    let namespaces = ConfiguredNamespaces::from_r2_routing(&routing)?;
    // Dedicated explicit operator URL. No DATABASE_URL discovery or migration/permission sync.
    let database_url =
        env::var("FILE_STORAGE_BINDING_DATABASE_URL").map_err(|_| BindingError::Configuration)?;
    let pool = school_tenancy::connect_privileged_pool(&database_url, 1)
        .await
        .map_err(|_| BindingError::Database)?;
    match command.mode.as_str() {
        "plan" => {
            let attestation = command.attestation.ok_or(BindingError::InvalidManifest)?;
            let manifest = namespace_binding::prepare_binding_manifest(
                pool.as_ref(),
                &command.watch_ids,
                attestation,
                &namespaces,
            )
            .await?;
            let data =
                serde_json::to_vec_pretty(&manifest).map_err(|_| BindingError::InvalidManifest)?;
            let output = command.output.ok_or(BindingError::InvalidManifest)?;
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(output)
                .map_err(|_| BindingError::InvalidManifest)?;
            file.write_all(&data)
                .map_err(|_| BindingError::InvalidManifest)?;
            println!(
                "storage_namespace_manifest watches={}",
                manifest.watches.len()
            );
            Ok(true)
        }
        "dry-run" | "apply" => {
            let path = command.manifest.ok_or(BindingError::InvalidManifest)?;
            if fs::metadata(&path)
                .map_err(|_| BindingError::InvalidManifest)?
                .len()
                > 1024 * 1024
            {
                return Err(BindingError::InvalidManifest);
            }
            let data = fs::read(path).map_err(|_| BindingError::InvalidManifest)?;
            let manifest: BindingManifest =
                serde_json::from_slice(&data).map_err(|_| BindingError::InvalidManifest)?;
            let apply = command.mode == "apply";
            if apply && !command.confirmed {
                return Err(BindingError::InvalidManifest);
            }
            let selected = namespace_binding::bind_attested_namespaces(
                pool.as_ref(),
                &manifest,
                &namespaces,
                apply,
            )
            .await?;
            println!(
                "storage_namespace_binding selected={} applied={}",
                selected,
                if apply { selected } else { 0 }
            );
            Ok(true)
        }
        _ => Err(BindingError::InvalidManifest),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operational_commands_refuse_extra_or_unknown_arguments() {
        assert!(valid_arguments(&["context-admin".into()]));
        assert!(valid_arguments(&["routing-fingerprints".into()]));
        assert!(!valid_arguments(&["check-admin".into(), "--apply".into()]));
        assert!(!valid_arguments(&["unknown-operation".into()]));
        assert!(!valid_arguments(&[]));
    }
    #[test]
    fn inventory_requires_complete_trusted_bytes_and_unique_tenants() {
        let id = Uuid::new_v4();
        let entry = serde_json::json!({"tenant_id":id,"database_url_env":"FILE_STORAGE_BINDING_URL_ONE","r2_account_id":"synthetic-account","public_bucket_name":"public-fixture","private_bucket_name":"private-fixture"});
        let bytes = serde_json::to_vec(&serde_json::json!({"tenants":[entry.clone()]})).unwrap();
        let digest = hex::encode(Sha256::digest(&bytes));
        assert_eq!(inventory(&bytes, &digest).unwrap().tenants.len(), 1);
        assert!(matches!(
            inventory(&bytes, &"a".repeat(64)),
            Err(BindingError::InvalidManifest)
        ));
        let duplicate =
            serde_json::to_vec(&serde_json::json!({"tenants":[entry.clone(),entry]})).unwrap();
        assert!(matches!(
            inventory(&duplicate, &hex::encode(Sha256::digest(&duplicate))),
            Err(BindingError::InvalidManifest)
        ));
        let empty = br#"{"tenants":[]}"#;
        assert!(matches!(
            inventory(empty, &hex::encode(Sha256::digest(empty))),
            Err(BindingError::InvalidManifest)
        ));
    }
}
