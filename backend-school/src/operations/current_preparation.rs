//! Private release evidence. No HTTP request can supply a cutover date or skip.
use school_database::DbSession;
use school_migrations::CurrentPreparationExpectation;
use school_tenancy::{AdminClient, OperationalTenant};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};
use uuid::Uuid;

const PROOF_PATH: &str = "/run/academic-current-preparation/proof.json";
const RESTORE_POLICY: &str = "pg18.6-exact-expression-replay-nn-v1";
const RESTORE_EVIDENCE_DOMAIN: &str = "schoolorbit.academic-current.restore-evidence.v2";
const MAX_RESTORE_COUNT: u32 = 20_000;
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
struct RestoreFidelity {
    policy: String,
    source_canonical_fingerprint: String,
    restored_canonical_fingerprint: String,
    metadata_sha256: String,
    nn_identity_map_sha256: String,
    evidence_sha256: String,
    check_count: u32,
    partial_index_predicate_count: u32,
    nn_rename_count: u32,
    unclassified_difference_count: u32,
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
    restored_reconciled_fingerprint: String,
    restore_fidelity: RestoreFidelity,
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
fn evidence_field(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}
fn evidence_digest(proof: &Proof, item: &TenantProof) -> String {
    let fidelity = &item.restore_fidelity;
    let mut digest = Sha256::new();
    for value in [
        RESTORE_EVIDENCE_DOMAIN.to_owned(),
        fidelity.policy.clone(),
        proof.release_sha.clone(),
        proof.image_digest.clone(),
        proof.inventory_fingerprint.clone(),
        item.school_id.to_string(),
        item.subdomain.clone(),
        item.database_name.clone(),
        item.archive_sha256.clone(),
        item.source_fingerprint.clone(),
        item.restored_source_fingerprint.clone(),
        item.restored_reconciled_fingerprint.clone(),
        fidelity.source_canonical_fingerprint.clone(),
        fidelity.restored_canonical_fingerprint.clone(),
        fidelity.metadata_sha256.clone(),
        fidelity.nn_identity_map_sha256.clone(),
        item.cutover_date.to_string(),
        item.selection_fingerprint.clone(),
        item.history_version.to_string(),
        item.decryption_verified.to_string(),
        fidelity.check_count.to_string(),
        fidelity.partial_index_predicate_count.to_string(),
        fidelity.nn_rename_count.to_string(),
        fidelity.unclassified_difference_count.to_string(),
    ] {
        evidence_field(&mut digest, &value);
    }
    let mut checks = item.checks.iter().collect::<Vec<_>>();
    checks.sort_unstable_by(|a, b| a.code.cmp(&b.code));
    for check in checks {
        evidence_field(&mut digest, &check.code);
        evidence_field(&mut digest, &check.passed.to_string());
        evidence_field(&mut digest, &check.mismatch_count.to_string());
    }
    hex::encode(digest.finalize())
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
        if proof.schema_version != 2
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
            let fidelity = &item.restore_fidelity;
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
                || !hash(&item.restored_source_fingerprint)
                || !hash(&item.restored_reconciled_fingerprint)
                || fidelity.policy != RESTORE_POLICY
                || !hash(&fidelity.source_canonical_fingerprint)
                || !hash(&fidelity.restored_canonical_fingerprint)
                || fidelity.source_canonical_fingerprint != fidelity.restored_canonical_fingerprint
                || !hash(&fidelity.metadata_sha256)
                || !hash(&fidelity.nn_identity_map_sha256)
                || !hash(&fidelity.evidence_sha256)
                || fidelity.check_count > MAX_RESTORE_COUNT
                || fidelity.partial_index_predicate_count > MAX_RESTORE_COUNT
                || fidelity.nn_rename_count > MAX_RESTORE_COUNT
                || fidelity.unclassified_difference_count != 0
                || !item.decryption_verified
                || item.history_version != school_migrations::latest_migration_version()
                || item.checks.len() != 8
            {
                return Err(invalid());
            }
            if fidelity.evidence_sha256 != evidence_digest(&proof, item) {
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
        let mut value = json!({
            "schemaVersion": 2,
            "operation": "academic-current-156157",
            "releaseSha": "a".repeat(40),
            "imageDigest": format!("sha256:{}", "b".repeat(64)),
            "inventoryFingerprint": super::super::tenant_rollout::inventory_digest(tenants).unwrap(),
            "tenants": [{
                "schoolId": tenants[0].id,
                "subdomain": "one",
                "databaseName": "tenant_one",
                "cutoverDate": "2026-11-14",
                "selectionFingerprint": "c".repeat(64),
                "sourceFingerprint": "d".repeat(64),
                "restoredSourceFingerprint": "f".repeat(64),
                "restoredReconciledFingerprint": "0".repeat(64),
                "archiveSha256": "e".repeat(64),
                "decryptionVerified": true,
                "historyVersion": school_migrations::latest_migration_version(),
                "restoreFidelity": {
                    "policy": RESTORE_POLICY,
                    "sourceCanonicalFingerprint": "1".repeat(64),
                    "restoredCanonicalFingerprint": "1".repeat(64),
                    "metadataSha256": "2".repeat(64),
                    "nnIdentityMapSha256": "3".repeat(64),
                    "evidenceSha256": "4".repeat(64),
                    "checkCount": 43,
                    "partialIndexPredicateCount": 3,
                    "nnRenameCount": 16,
                    "unclassifiedDifferenceCount": 0
                },
                "checks": CHECKS.map(|code| json!({"code": code, "passed": true, "mismatchCount": 0}))
            }]
        });
        seal(&mut value);
        value
    }
    fn seal(value: &mut Value) {
        let parsed: Proof = serde_json::from_value(value.clone()).unwrap();
        for (index, item) in parsed.tenants.iter().enumerate() {
            value["tenants"][index]["restoreFidelity"]["evidenceSha256"] =
                json!(evidence_digest(&parsed, item));
        }
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
        for key in [
            "sourceFingerprint",
            "restoredSourceFingerprint",
            "restoredReconciledFingerprint",
            "selectionFingerprint",
            "archiveSha256",
        ] {
            let mut bad = original.clone();
            bad["tenants"][0][key] = json!("not-a-hash");
            seal(&mut bad);
            assert!(validated(bad, &rows).is_err());
        }
        for (key, value) in [
            ("databaseName", json!("foreign")),
            ("decryptionVerified", json!(false)),
            ("historyVersion", json!(156)),
        ] {
            let mut bad = original.clone();
            bad["tenants"][0][key] = value;
            seal(&mut bad);
            assert!(validated(bad, &rows).is_err());
        }
        let mut bad = original.clone();
        bad["tenants"][0]["checks"].as_array_mut().unwrap().pop();
        seal(&mut bad);
        assert!(validated(bad, &rows).is_err());
        let mut bad = original.clone();
        bad["tenants"][0]["checks"][0]["mismatchCount"] = json!(1);
        seal(&mut bad);
        assert!(validated(bad, &rows).is_err());
        let mut bad = original.clone();
        bad["tenants"]
            .as_array_mut()
            .unwrap()
            .push(original["tenants"][0].clone());
        seal(&mut bad);
        assert!(validated(bad, &rows).is_err());
        let mut bad = original;
        bad["skipPreparation"] = json!(true);
        assert!(validated(bad, &rows).is_err());
    }
    #[test]
    fn restore_fidelity_accepts_distinct_raw_hashes_and_sorted_checks() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        // Actual PG18.6 logical restoration changes CHECK deparsing and NOT NULL
        // names. Schema2 retains those raw hashes and binds the exact PG oracle
        // and name reconciliation instead of asserting synthetic raw equality.
        let mut value = proof(&rows);
        assert_ne!(
            value["tenants"][0]["sourceFingerprint"],
            value["tenants"][0]["restoredSourceFingerprint"]
        );
        assert_ne!(
            value["tenants"][0]["restoredSourceFingerprint"],
            value["tenants"][0]["restoredReconciledFingerprint"]
        );
        let expected_digest = value["tenants"][0]["restoreFidelity"]["evidenceSha256"].clone();
        value["tenants"][0]["checks"]
            .as_array_mut()
            .unwrap()
            .reverse();
        seal(&mut value);
        assert_eq!(
            value["tenants"][0]["restoreFidelity"]["evidenceSha256"],
            expected_digest
        );
        let valid = validated(value, &rows).unwrap();
        let expected = valid.expectation(rows[0].id, "one").unwrap().unwrap();
        assert_eq!(expected.cutover_date.to_string(), "2026-11-14");
        assert_eq!(expected.selection_fingerprint, [0xcc; 32]);
    }
    #[test]
    fn restore_evidence_binds_every_identity_hash_date_count_and_check() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        let original = proof(&rows);
        for key in [
            "sourceFingerprint",
            "restoredSourceFingerprint",
            "restoredReconciledFingerprint",
            "archiveSha256",
            "selectionFingerprint",
        ] {
            let mut changed = original.clone();
            changed["tenants"][0][key] = json!("5".repeat(64));
            assert!(validated(changed, &rows).is_err(), "unbound {key}");
        }
        for key in ["metadataSha256", "nnIdentityMapSha256", "evidenceSha256"] {
            let mut changed = original.clone();
            changed["tenants"][0]["restoreFidelity"][key] = json!("5".repeat(64));
            assert!(validated(changed, &rows).is_err(), "unbound {key}");
        }
        let mut changed = original.clone();
        changed["tenants"][0]["restoreFidelity"]["sourceCanonicalFingerprint"] =
            json!("5".repeat(64));
        changed["tenants"][0]["restoreFidelity"]["restoredCanonicalFingerprint"] =
            json!("5".repeat(64));
        assert!(validated(changed, &rows).is_err());
        for key in ["checkCount", "partialIndexPredicateCount", "nnRenameCount"] {
            let mut changed = original.clone();
            changed["tenants"][0]["restoreFidelity"][key] = json!(42);
            assert!(validated(changed, &rows).is_err(), "unbound {key}");
        }
        let mut changed = original.clone();
        changed["tenants"][0]["cutoverDate"] = json!("2026-11-15");
        assert!(validated(changed, &rows).is_err());
        for (key, value) in [
            ("code", json!("UNKNOWN")),
            ("passed", json!(false)),
            ("mismatchCount", json!(1)),
        ] {
            let mut changed = original.clone();
            changed["tenants"][0]["checks"][0][key] = value;
            seal(&mut changed);
            assert!(validated(changed, &rows).is_err(), "invalid check {key}");
        }
    }
    #[test]
    fn restore_fidelity_rejects_unknown_schema_fields_policy_and_types() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        let original = proof(&rows);
        let mut bad = original.clone();
        bad["schemaVersion"] = json!(1);
        assert!(validated(bad, &rows).is_err());
        for pointer in [
            "",
            "/tenants/0",
            "/tenants/0/restoreFidelity",
            "/tenants/0/checks/0",
        ] {
            let mut bad = original.clone();
            bad.pointer_mut(pointer).unwrap()["unknown"] = json!(true);
            assert!(validated(bad, &rows).is_err(), "unknown field at {pointer}");
        }
        let mut bad = original.clone();
        bad["tenants"][0]["restoreFidelity"]["policy"] = json!("equivalent-expression-v2");
        seal(&mut bad);
        assert!(validated(bad, &rows).is_err());
        for key in [
            "sourceCanonicalFingerprint",
            "restoredCanonicalFingerprint",
            "metadataSha256",
            "nnIdentityMapSha256",
            "evidenceSha256",
        ] {
            for value in [json!("A".repeat(64)), json!(12), json!(null)] {
                let mut bad = original.clone();
                bad["tenants"][0]["restoreFidelity"][key] = value;
                if key != "evidenceSha256" && bad["tenants"][0]["restoreFidelity"][key].is_string()
                {
                    seal(&mut bad);
                }
                assert!(validated(bad, &rows).is_err(), "invalid hash {key}");
            }
        }
        let mut bad = original.clone();
        bad["tenants"][0]["restoreFidelity"]["sourceCanonicalFingerprint"] = json!("A".repeat(64));
        bad["tenants"][0]["restoreFidelity"]["restoredCanonicalFingerprint"] =
            json!("A".repeat(64));
        seal(&mut bad);
        assert!(validated(bad, &rows).is_err());
        let mut bad = original;
        bad["tenants"][0]["restoreFidelity"]["restoredCanonicalFingerprint"] =
            json!("5".repeat(64));
        seal(&mut bad);
        assert!(validated(bad, &rows).is_err());
    }
    #[test]
    fn restore_fidelity_count_bounds_and_zero_unclassified_are_required() {
        let _lock = ENV.lock().unwrap();
        let _saved = context();
        let rows = tenants();
        let original = proof(&rows);
        for key in [
            "checkCount",
            "partialIndexPredicateCount",
            "nnRenameCount",
            "unclassifiedDifferenceCount",
        ] {
            for value in [json!(-1), json!(u64::MAX), json!("0"), json!(0.5)] {
                let mut bad = original.clone();
                bad["tenants"][0]["restoreFidelity"][key] = value;
                assert!(validated(bad, &rows).is_err(), "invalid count {key}");
            }
            let mut bad = original.clone();
            bad["tenants"][0]["restoreFidelity"][key] = json!(MAX_RESTORE_COUNT + 1);
            seal(&mut bad);
            assert!(validated(bad, &rows).is_err(), "excessive count {key}");
            let mut bad = original.clone();
            bad["tenants"][0]["restoreFidelity"][key] = json!(1);
            seal(&mut bad);
            if key == "unclassifiedDifferenceCount" {
                assert!(validated(bad, &rows).is_err());
            } else {
                assert!(validated(bad, &rows).is_ok());
                for bound in [0, MAX_RESTORE_COUNT] {
                    let mut valid = original.clone();
                    valid["tenants"][0]["restoreFidelity"][key] = json!(bound);
                    seal(&mut valid);
                    assert!(validated(valid, &rows).is_ok());
                }
            }
        }
    }
    #[test]
    fn restore_evidence_length_frames_utf8_fields_without_ambiguity() {
        fn framed(fields: &[&str]) -> Vec<u8> {
            let mut digest = Sha256::new();
            for field in fields {
                evidence_field(&mut digest, field);
            }
            digest.finalize().to_vec()
        }
        assert_ne!(framed(&["ab", "c"]), framed(&["a", "bc"]));
        assert_ne!(framed(&["", "abc"]), framed(&["abc", ""]));
        assert_ne!(framed(&["ท", "ย"]), framed(&["ทย"]));
        let mut independently_encoded = Sha256::new();
        independently_encoded.update(3_u64.to_be_bytes());
        independently_encoded.update("ท".as_bytes());
        assert_eq!(framed(&["ท"]), independently_encoded.finalize().to_vec());
    }
    #[test]
    fn restore_evidence_matches_controller_schema_two_vector() {
        let mut value = proof(&tenants());
        value["inventoryFingerprint"] = json!("c".repeat(64));
        let item = &mut value["tenants"][0];
        item["subdomain"] = json!("tenant-1");
        item["databaseName"] = json!("tenant_11111111111141118111111111111111");
        item["archiveSha256"] = json!("d".repeat(64));
        item["sourceFingerprint"] = json!("e".repeat(64));
        item["selectionFingerprint"] = json!("1".repeat(64));
        item["historyVersion"] = json!(157);
        item["restoreFidelity"]["sourceCanonicalFingerprint"] = json!("2".repeat(64));
        item["restoreFidelity"]["restoredCanonicalFingerprint"] = json!("2".repeat(64));
        item["restoreFidelity"]["metadataSha256"] = json!("3".repeat(64));
        item["restoreFidelity"]["nnIdentityMapSha256"] = json!("4".repeat(64));
        let parsed: Proof = serde_json::from_value(value).unwrap();
        // Independently generated by the controller's pure Python helper using
        // the schema2 field order, u64-BE lengths and canonical check order.
        assert_eq!(
            evidence_digest(&parsed, &parsed.tenants[0]),
            "049caf36905ac15c1581194ab09f00763892167c1ac3d01c9a22f83705cbe48a"
        );
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
