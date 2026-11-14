//! Private release evidence. No HTTP request can supply a cutover date or skip.
use school_database::DbSession;
use school_migrations::CurrentPreparationExpectation;
use school_tenancy::{AdminClient, OperationalTenant};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    env, fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};
use uuid::Uuid;

const PROOF_PATH: &str = "/run/academic-current-preparation/proof.json";
const CHECKS: [&str; 8] = [
    "CURRENT_CATALOG_PINS_PRESERVED",
    "CURRENT_DELIVERY_GRAPH_RECONCILED",
    "CURRENT_FACTS_PRESERVED",
    "CURRENT_LEGACY_SCHEMA_REMOVED",
    "CURRENT_SCOPE_GUARDS_ENABLED",
    "CURRENT_SUPERVISION_PRESERVED",
    "CURRENT_TEACHERS_RECONCILED",
    "CURRENT_TIMETABLE_IDS_RECONCILED",
];
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Check {
    code: String,
    passed: bool,
    mismatch_count: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TenantProof {
    school_id: Uuid,
    subdomain: String,
    database_name: String,
    cutover_date: chrono::NaiveDate,
    selection_fingerprint: String,
    source_fingerprint: String,
    archive_sha256: String,
    restored_source_fingerprint: String,
    decryption_verified: bool,
    history_version: i64,
    checks: Vec<Check>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proof {
    schema_version: u32,
    operation: String,
    release_sha: String,
    image_digest: String,
    inventory_fingerprint: String,
    tenants: Vec<TenantProof>,
}
pub(crate) struct ValidatedPreparation {
    proof: Option<Proof>,
}
fn invalid() -> String {
    "current_preparation_invalid".into()
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn decode(value: &str) -> Result<[u8; 32], String> {
    if !hash(value) {
        return Err(invalid());
    }
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| invalid())?;
    }
    Ok(result)
}
impl ValidatedPreparation {
    pub(crate) fn load(tenants: &[OperationalTenant]) -> Result<Self, String> {
        let Some(path) = env::var_os("CURRENT_ACADEMIC_PREPARATION_FILE") else {
            return Ok(Self { proof: None });
        };
        if Path::new(&path) != Path::new(PROOF_PATH) {
            return Err(invalid());
        }
        let meta = fs::symlink_metadata(&path).map_err(|_| invalid())?;
        if !meta.is_file()
            || meta.permissions().mode() & 0o777 != 0o600
            || meta.len() > 1024 * 1024
            || meta.uid() != fs::metadata("/proc/self").map_err(|_| invalid())?.uid()
        {
            return Err(invalid());
        }
        let proof: Proof = serde_json::from_slice(&fs::read(path).map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
        Self::validate(proof, tenants)
    }
    fn validate(proof: Proof, tenants: &[OperationalTenant]) -> Result<Self, String> {
        if proof.schema_version != 1
            || proof.operation != "academic-current-156157"
            || proof.release_sha
                != env::var("CURRENT_ACADEMIC_RELEASE_SHA").map_err(|_| invalid())?
            || proof.image_digest
                != env::var("CURRENT_ACADEMIC_IMAGE_DIGEST").map_err(|_| invalid())?
            || proof.release_sha.len() != 40
            || !proof.release_sha.bytes().all(|b| b.is_ascii_hexdigit())
            || !proof.image_digest.starts_with("sha256:")
            || !hash(&proof.image_digest[7..])
            || proof.inventory_fingerprint
                != super::tenant_rollout::inventory_digest(tenants).map_err(|_| invalid())?
            || proof.tenants.len() > tenants.len()
        {
            return Err(invalid());
        }
        let mut ids = BTreeSet::new();
        for item in &proof.tenants {
            let Some(tenant) = tenants
                .iter()
                .find(|t| t.id == item.school_id && t.subdomain == item.subdomain)
            else {
                return Err(invalid());
            };
            let route = url::Url::parse(&tenant.database_url).map_err(|_| invalid())?;
            if !ids.insert(item.school_id)
                || route.path().trim_start_matches('/') != item.database_name
                || !hash(&item.selection_fingerprint)
                || !hash(&item.source_fingerprint)
                || !hash(&item.archive_sha256)
                || item.restored_source_fingerprint != item.source_fingerprint
                || !item.decryption_verified
                || item.history_version != school_migrations::latest_migration_version()
                || item.checks.len() != 8
            {
                return Err(invalid());
            }
            let mut codes = item
                .checks
                .iter()
                .map(|c| c.code.as_str())
                .collect::<Vec<_>>();
            codes.sort_unstable();
            if codes.as_slice() != CHECKS.as_slice()
                || item
                    .checks
                    .iter()
                    .any(|c| !c.passed || c.mismatch_count != 0)
            {
                return Err(invalid());
            }
        }
        Ok(Self { proof: Some(proof) })
    }
    pub(crate) fn expectation(
        &self,
        id: Uuid,
        subdomain: &str,
    ) -> Result<Option<CurrentPreparationExpectation>, String> {
        let Some(proof) = &self.proof else {
            return Ok(None);
        };
        let Some(item) = proof
            .tenants
            .iter()
            .find(|t| t.school_id == id && t.subdomain == subdomain)
        else {
            return Ok(None);
        };
        Ok(Some(CurrentPreparationExpectation {
            cutover_date: item.cutover_date,
            selection_fingerprint: decode(&item.selection_fingerprint)?,
        }))
    }
}

async fn verify_connection_identity(
    connection: &mut sqlx::PgConnection,
    tenant: &OperationalTenant,
) -> Result<(), String> {
    let identity:(String,String,Option<String>)=DbSession::new(connection).query_as("school.current_preparation.identity","SELECT current_user::text,current_database(),shobj_description(r.oid,'pg_authid') FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database()")
        .fetch_one().await.map_err(|_|invalid())?;
    let route = url::Url::parse(&tenant.database_url).map_err(|_| invalid())?;
    if identity.0 != format!("so_{}_owner", tenant.id.simple())
        || identity.1 != route.path().trim_start_matches('/')
        || identity.2.as_deref() != Some(format!("schoolorbit:{}:migration", tenant.id).as_str())
    {
        return Err(invalid());
    }
    Ok(())
}

/// Both headless release and internal migrations resolve the same complete
/// operational inventory. Missing proof is represented by the required marker,
/// so migration157 itself refuses before destructive DML.
pub(crate) async fn run_for_school(
    pool: &sqlx::PgPool,
    admin: &AdminClient,
    subdomain: &str,
) -> Result<(), String> {
    let tenants = admin
        .list_operational_tenants()
        .await
        .map_err(|_| invalid())?;
    let tenant = tenants
        .iter()
        .find(|t| t.subdomain == subdomain)
        .ok_or_else(invalid)?;
    let proof = ValidatedPreparation::load(&tenants)?;
    let expected = proof.expectation(tenant.id, subdomain)?;
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "school.current_preparation.identity",
    )
    .await
    .map_err(|_| invalid())?;
    verify_connection_identity(&mut connection, tenant).await?;
    drop(connection);
    school_migrations::run_operational_tenant_migrations(pool, expected.as_ref())
        .await
        .map_err(|e| e.into_legacy_message())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::sync::Mutex;
    static ENV: Mutex<()> = Mutex::new(());
    struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                if let Some(value) = value {
                    env::set_var(name, value)
                } else {
                    env::remove_var(name)
                }
            }
        }
    }
    fn context() -> Restore {
        let saved = Restore(
            [
                "CURRENT_ACADEMIC_RELEASE_SHA",
                "CURRENT_ACADEMIC_IMAGE_DIGEST",
                "CURRENT_ACADEMIC_PREPARATION_FILE",
            ]
            .into_iter()
            .map(|key| (key, env::var_os(key)))
            .collect(),
        );
        env::set_var("CURRENT_ACADEMIC_RELEASE_SHA", "a".repeat(40));
        env::set_var(
            "CURRENT_ACADEMIC_IMAGE_DIGEST",
            format!("sha256:{}", "b".repeat(64)),
        );
        env::remove_var("CURRENT_ACADEMIC_PREPARATION_FILE");
        saved
    }
    fn tenants() -> Vec<OperationalTenant> {
        vec![
            OperationalTenant {
                id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
                subdomain: "one".into(),
                status: "active".into(),
                database_url:
                    "postgresql://runtime:private@db.example:5432/tenant_one?sslmode=verify-full"
                        .into(),
            },
            OperationalTenant {
                id: Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap(),
                subdomain: "two".into(),
                status: "inactive".into(),
                database_url:
                    "postgresql://runtime:private@db.example:5432/tenant_two?sslmode=verify-full"
                        .into(),
            },
        ]
    }
    fn proof(tenants: &[OperationalTenant]) -> Value {
        json!({"schemaVersion":1,"operation":"academic-current-156157","releaseSha":"a".repeat(40),"imageDigest":format!("sha256:{}","b".repeat(64)),"inventoryFingerprint":super::super::tenant_rollout::inventory_digest(tenants).unwrap(),"tenants":[{"schoolId":tenants[0].id,"subdomain":"one","databaseName":"tenant_one","cutoverDate":"2026-11-14","selectionFingerprint":"c".repeat(64),"sourceFingerprint":"d".repeat(64),"restoredSourceFingerprint":"d".repeat(64),"archiveSha256":"e".repeat(64),"decryptionVerified":true,"historyVersion":school_migrations::latest_migration_version(),"checks":CHECKS.map(|code|json!({"code":code,"passed":true,"mismatchCount":0}))}]})
    }
    fn validated(
        value: Value,
        tenants: &[OperationalTenant],
    ) -> Result<ValidatedPreparation, String> {
        ValidatedPreparation::validate(
            serde_json::from_value(value).map_err(|_| invalid())?,
            tenants,
        )
    }
    #[test]
    fn private_preparation_binds_release_full_inventory_and_affected_subset() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        let value = proof(&rows);
        let valid = validated(value.clone(), &rows).unwrap();
        assert!(valid.expectation(rows[0].id, "one").unwrap().is_some());
        // The canonical operational marker, not an absent bundle entry, decides
        // whether the second database has already applied157 or must refuse.
        assert!(valid.expectation(rows[1].id, "two").unwrap().is_none());
        let mut changed = tenants();
        changed[1].status = "active".into();
        assert!(validated(value.clone(), &changed).is_err());
        changed = tenants();
        changed[1].database_url = "postgresql://runtime:private@other.example/tenant_two".into();
        assert!(validated(value.clone(), &changed).is_err());
        for key in ["releaseSha", "imageDigest", "inventoryFingerprint"] {
            let mut bad = value.clone();
            bad[key] = json!("unknown");
            assert!(validated(bad, &rows).is_err());
        }
    }
    #[test]
    fn malformed_or_incomplete_tenant_proof_never_becomes_an_expectation() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        let original = proof(&rows);
        for key in ["sourceFingerprint", "selectionFingerprint", "archiveSha256"] {
            let mut bad = original.clone();
            bad["tenants"][0][key] = json!("not-a-hash");
            assert!(validated(bad, &rows).is_err());
        }
        for (key, value) in [
            ("databaseName", json!("foreign")),
            ("decryptionVerified", json!(false)),
            ("historyVersion", json!(156)),
            ("restoredSourceFingerprint", json!("f".repeat(64))),
        ] {
            let mut bad = original.clone();
            bad["tenants"][0][key] = value;
            assert!(validated(bad, &rows).is_err());
        }
        let mut bad = original.clone();
        bad["tenants"][0]["checks"].as_array_mut().unwrap().pop();
        assert!(validated(bad, &rows).is_err());
        let mut bad = original.clone();
        bad["tenants"][0]["checks"][0]["mismatchCount"] = json!(1);
        assert!(validated(bad, &rows).is_err());
        let mut bad = original.clone();
        bad["tenants"]
            .as_array_mut()
            .unwrap()
            .push(original["tenants"][0].clone());
        assert!(validated(bad, &rows).is_err());
        let mut bad = original;
        bad["skipPreparation"] = json!(true);
        assert!(validated(bad, &rows).is_err());
    }
    #[test]
    fn transport_has_no_arbitrary_path_or_skip_flag() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        assert!(ValidatedPreparation::load(&rows)
            .unwrap()
            .expectation(rows[0].id, "one")
            .unwrap()
            .is_none());
        env::set_var(
            "CURRENT_ACADEMIC_PREPARATION_FILE",
            "/tmp/client-supplied.json",
        );
        assert!(ValidatedPreparation::load(&rows).is_err());
    }
}
