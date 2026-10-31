//! Disposable predecessor/refusal and readiness proofs; never production repairs.
use school_test_db::academic_context_receipt_fixture::seed_legacy_curriculum_receipt;
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

const RECOVERY_CHECK: &str = "ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE";
const ORIGINAL: &str = "SELECT jsonb_build_object('receipts',(SELECT COALESCE(jsonb_agg(to_jsonb(receipt) ORDER BY idempotency_key),'[]'::jsonb) FROM learning_delivery_apply_runs receipt),'history',(SELECT jsonb_agg(to_jsonb(history) ORDER BY version) FROM _sqlx_migrations history),'offerings',(SELECT jsonb_agg(to_jsonb(offering) ORDER BY id) FROM learning_offerings offering),'years',(SELECT jsonb_agg(to_jsonb(year) ORDER BY id) FROM academic_years year),'terms',(SELECT jsonb_agg(to_jsonb(term) ORDER BY id) FROM academic_terms term),'column',(SELECT jsonb_build_object('number',attribute.attnum,'type',attribute.atttypid,'notNull',attribute.attnotnull,'defaultOid',definition.oid,'default',definition.adbin::text,'constraints',(SELECT jsonb_agg(jsonb_build_object('oid',rule.oid,'expression',rule.conbin::text) ORDER BY rule.oid) FROM pg_constraint rule WHERE rule.conrelid=attribute.attrelid AND attribute.attnum=ANY(rule.conkey))) FROM pg_attribute attribute LEFT JOIN pg_attrdef definition ON definition.adrelid=attribute.attrelid AND definition.adnum=attribute.attnum WHERE attribute.attrelid='learning_delivery_apply_runs'::regclass AND attribute.attname='created_offering_count' AND NOT attribute.attisdropped))";

async fn predecessor(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 1).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 147).await.unwrap();
    pool
}

async fn execute(pool: &PgPool, statement: &'static str) {
    // Statements come exclusively from the closed synthetic fixture cases below.
    sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
        .execute(pool)
        .await
        .unwrap();
}

async fn canonical(pool: &PgPool) -> Result<(), school_migrations::MigrationFailure> {
    tokio::time::timeout(
        Duration::from_secs(60),
        school_migrations::run_tenant_migrations_detailed(pool),
    )
    .await
    .expect("migration must release the sole owner connection")
}

async fn original(pool: &PgPool) -> Value {
    sqlx::query_scalar(ORIGINAL).fetch_one(pool).await.unwrap()
}

async fn no_staging(pool: &PgPool) {
    let absent:bool=sqlx::query_scalar("SELECT NOT(EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid='learning_delivery_apply_runs'::regclass AND NOT attisdropped AND starts_with(attname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_class WHERE relnamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(relname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(proname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(typname,'academic149_')))")
        .fetch_one(pool).await.unwrap();
    assert!(absent);
}

async fn refusal_preserves(pool: &PgPool, code: &str) {
    let before = original(pool).await;
    let failure = canonical(pool)
        .await
        .expect_err("unsupported state must refuse");
    assert_eq!(failure.diagnostic.code, code, "{failure:?}");
    assert_eq!(failure.diagnostic.version, Some(149));
    assert_eq!(original(pool).await, before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .unwrap(),
        147
    );
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('academic_context_receipt_recovery_audit') IS NOT NULL"
    )
    .fetch_one(pool)
    .await
    .unwrap());
    no_staging(pool).await;
}

async fn readiness(pool: &PgPool, recovery_passed: bool) {
    let status =
        school_academic_core::context_reconciliation::read_academic_context_cutover_audit(pool)
            .await
            .unwrap();
    assert_eq!(status.checks.len(), 6);
    assert_eq!(status.completed, recovery_passed, "{:?}", status.checks);
    let mut recovery_checks = 0;
    for check in status.checks {
        if check.code == RECOVERY_CHECK {
            recovery_checks += 1;
            assert_eq!(check.passed, recovery_passed);
        } else {
            assert!(
                check.passed,
                "other context checks must remain valid: {check:?}"
            );
        }
    }
    assert_eq!(recovery_checks, 1);
}

async fn evidence(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        "SELECT evidence FROM academic_context_receipt_recovery_audit WHERE singleton",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn ready_fixture(name: &str) -> PgPool {
    let pool = predecessor(name).await;
    seed_legacy_curriculum_receipt(&pool, 1).await;
    canonical(&pool).await.unwrap();
    readiness(&pool, true).await;
    pool
}

#[tokio::test]
async fn ordinary_mask_zero_and_pure_lower_receipts_are_preserved_together() {
    let pool = predecessor("receipt149_mixed_admission").await;
    let lower = seed_legacy_curriculum_receipt(&pool, 1).await;
    // Synthetic current shape only; the separate historical producer regression
    // establishes the possible lifecycle, without attributing a production cause.
    sqlx::query("UPDATE learning_offerings offering SET created_at=offering.created_at+interval '1 second' FROM learning_delivery_apply_runs receipt WHERE receipt.idempotency_key=$1 AND offering.id=ANY(receipt.offering_ids) AND offering.created_at=receipt.created_at")
        .bind(lower.request_id).execute(&pool).await.unwrap();
    // A later accepted request retains the same complete output cohort. Reusing
    // that cohort avoids duplicating the term/subject registry owned by the seed.
    let retained_request = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_delivery_apply_runs SELECT (jsonb_populate_record(NULL::learning_delivery_apply_runs,to_jsonb(receipt)||jsonb_build_object('idempotency_key',$1::uuid,'created_offering_count',0,'retained_offering_count',cardinality(receipt.offering_ids),'created_at',receipt.created_at+interval '2 seconds'))).* FROM learning_delivery_apply_runs receipt WHERE idempotency_key=$2")
        .bind(retained_request).bind(lower.request_id).execute(&pool).await.unwrap();
    let shapes:Vec<(Uuid,i32,i64,i64,bool,i32)>=sqlx::query_as("SELECT receipt.idempotency_key,receipt.created_offering_count,(SELECT count(*) FROM unnest(receipt.offering_ids) output(id) JOIN learning_offerings offering ON offering.id=output.id AND offering.academic_term_id=receipt.academic_term_id AND offering.created_at=receipt.created_at),(SELECT count(*) FROM learning_offerings offering WHERE offering.academic_term_id=receipt.academic_term_id AND offering.created_at=receipt.created_at),NOT EXISTS(SELECT 1 FROM unnest(receipt.offering_ids) output(id) LEFT JOIN learning_offerings offering ON offering.id=output.id AND offering.academic_term_id=receipt.academic_term_id WHERE offering.id IS NULL) AND cardinality(receipt.offering_ids)=(SELECT count(DISTINCT output.id) FROM unnest(receipt.offering_ids) output(id)),cardinality(receipt.offering_ids) FROM learning_delivery_apply_runs receipt")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(shapes.len(), 2);
    for (id, recorded, matched, all_created, valid_outputs, cardinality) in shapes {
        assert_eq!(
            (matched, all_created, valid_outputs, cardinality),
            (0, 0, true, 2)
        );
        if id == lower.request_id {
            assert_eq!(
                recorded, 1,
                "original receipt must have only the lower-count violation"
            );
        } else {
            assert_eq!(id, retained_request);
            assert_eq!(recorded, 0, "retained-only receipt must satisfy mask0");
        }
    }
    let before = original(&pool).await;
    canonical(&pool).await.unwrap();
    readiness(&pool, true).await;
    let receipts:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(receipt)-'starts_on' ORDER BY idempotency_key) FROM learning_delivery_apply_runs receipt").fetch_one(&pool).await.unwrap();
    assert_eq!(receipts, before["receipts"]);
    let after = original(&pool).await;
    assert_eq!(after["column"], before["column"]);
    assert_eq!(after["offerings"], before["offerings"]);
    let proof = evidence(&pool).await;
    assert_eq!(proof["receiptCount"], 2);
    assert_eq!(proof["recoveredReceiptCount"], 1);
    assert_eq!(proof["originalSha256"], proof["finalSha256"]);
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT bool_and(starts_on IS NULL) FROM learning_delivery_apply_runs"
    )
    .fetch_one(&pool)
    .await
    .unwrap());
    no_staging(&pool).await;
}

#[derive(Clone, Copy)]
enum Output {
    Created,
    Retained,
    Missing,
    Null,
}

#[tokio::test]
async fn every_other_predicate_mask_and_both_count_boundaries_refuse_without_changes() {
    use Output::{Created as C, Missing as X, Null as Z, Retained as R};
    // Each row names its intended 148 mask. Mask1-higher and N>cardinality
    // exercise the two refused sides of the otherwise superseded count predicate.
    let cases: &[(&str, i32, &[Output])] = &[
        ("mask1_higher", 0, &[C, R]),
        ("mask2_extra_timestamp", 0, &[R]),
        ("mask3", 1, &[R]),
        ("mask4_missing", 1, &[C, X]),
        ("mask5", 2, &[C, X]),
        ("mask6", 0, &[R, X]),
        ("mask7", 1, &[R, X]),
        ("mask8_duplicate", 1, &[C, R, R]),
        ("mask9", 2, &[C, R, R]),
        ("mask10", 2, &[C, C]),
        ("mask11", 1, &[C, C]),
        ("mask12_null", 1, &[C, Z]),
        ("mask13", 2, &[C, Z]),
        ("mask14", 0, &[Z]),
        ("mask15", 1, &[Z]),
        ("above_distinct", 3, &[C, R]),
    ];
    for (name, recorded, outputs) in cases {
        let pool = predecessor(&format!("receipt149_{name}")).await;
        let receipt = seed_legacy_curriculum_receipt(&pool, 1).await;
        let (created,retained):(Uuid,Uuid)=sqlx::query_as("SELECT (SELECT offering.id FROM learning_offerings offering WHERE offering.id=ANY(receipt.offering_ids) AND offering.created_at=receipt.created_at),(SELECT offering.id FROM learning_offerings offering WHERE offering.id=ANY(receipt.offering_ids) AND offering.created_at<>receipt.created_at) FROM learning_delivery_apply_runs receipt WHERE idempotency_key=$1")
            .bind(receipt.request_id).fetch_one(&pool).await.unwrap();
        let missing = Uuid::new_v4();
        let ids: Vec<Option<Uuid>> = outputs
            .iter()
            .map(|output| match output {
                C => Some(created),
                R => Some(retained),
                X => Some(missing),
                Z => None,
            })
            .collect();
        sqlx::query("UPDATE learning_delivery_apply_runs SET offering_ids=$2,created_offering_count=$3 WHERE idempotency_key=$1")
            .bind(receipt.request_id).bind(ids).bind(recorded).execute(&pool).await.unwrap();
        refusal_preserves(&pool, "ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE").await;
    }
    let pool = predecessor("receipt149_wrong_context").await;
    let receipt = seed_legacy_curriculum_receipt(&pool, 1).await;
    let foreign_term: Uuid =
        sqlx::query_scalar("SELECT id FROM academic_terms WHERE id<>$1 ORDER BY id LIMIT 1")
            .bind(receipt.target_term_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(foreign_term, receipt.target_term_id);
    sqlx::query(
        "UPDATE learning_delivery_apply_runs SET academic_term_id=$2 WHERE idempotency_key=$1",
    )
    .bind(receipt.request_id)
    .bind(foreign_term)
    .execute(&pool)
    .await
    .unwrap();
    let foreign_outputs: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_delivery_apply_runs receipt CROSS JOIN LATERAL unnest(receipt.offering_ids) output(id) JOIN learning_offerings offering ON offering.id=output.id WHERE receipt.idempotency_key=$1 AND offering.academic_term_id<>receipt.academic_term_id")
        .bind(receipt.request_id).fetch_one(&pool).await.unwrap();
    assert!(
        foreign_outputs > 0,
        "fixture must contain real wrong-context outputs"
    );
    refusal_preserves(&pool, "ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE").await;
}

#[tokio::test]
async fn unknown_receipt_shape_default_check_index_view_foreign_key_and_trigger_refuse() {
    let cases = [
        ("shape","ALTER TABLE learning_delivery_apply_runs ADD COLUMN fixture_extra integer", "ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE"),
        ("default","ALTER TABLE learning_delivery_apply_runs ALTER COLUMN created_offering_count SET DEFAULT 7", "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"),
        ("check","ALTER TABLE learning_delivery_apply_runs ADD CONSTRAINT fixture_count_limit CHECK(created_offering_count<=50)", "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"),
        ("index","CREATE INDEX fixture_receipt_count ON learning_delivery_apply_runs(created_offering_count)", "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"),
        ("view","CREATE VIEW fixture_receipt_view AS SELECT created_offering_count FROM learning_delivery_apply_runs", "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"),
        ("incoming_fk","CREATE TABLE fixture_receipt_reference(receipt uuid REFERENCES learning_delivery_apply_runs(idempotency_key))", "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"),
        ("trigger","CREATE FUNCTION fixture_receipt_trigger() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$; CREATE TRIGGER fixture_receipt_update BEFORE UPDATE ON learning_delivery_apply_runs FOR EACH ROW EXECUTE FUNCTION fixture_receipt_trigger()", "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"),
    ];
    for (name, ddl, code) in cases {
        let pool = predecessor(&format!("receipt149_drift_{name}")).await;
        seed_legacy_curriculum_receipt(&pool, 1).await;
        execute(&pool, ddl).await;
        refusal_preserves(&pool, code).await;
    }
}

#[tokio::test]
async fn composite_and_array_rowtype_consumers_are_real_dependencies_and_refuse() {
    let cases = [
        ("row_function","CREATE FUNCTION fixture_row_consumer(value learning_delivery_apply_runs) RETURNS integer LANGUAGE sql AS 'SELECT ($1).created_offering_count'",false),
        ("array_function","CREATE FUNCTION fixture_row_consumer(value learning_delivery_apply_runs[]) RETURNS integer LANGUAGE sql AS 'SELECT cardinality($1)'",true),
        ("row_domain","CREATE DOMAIN fixture_receipt_domain AS learning_delivery_apply_runs",false),
        ("array_column","CREATE TABLE fixture_receipt_container(receipts learning_delivery_apply_runs[])",true),
    ];
    for (name, ddl, array) in cases {
        let pool = predecessor(&format!("receipt149_type_{name}")).await;
        seed_legacy_curriculum_receipt(&pool, 1).await;
        execute(&pool, ddl).await;
        let dependencies:i64=sqlx::query_scalar("SELECT count(*) FROM pg_depend dependency JOIN pg_class receipt ON receipt.oid='learning_delivery_apply_runs'::regclass JOIN pg_type row_type ON row_type.oid=receipt.reltype WHERE dependency.refclassid='pg_type'::regclass AND dependency.refobjid=CASE WHEN $1 THEN row_type.typarray ELSE row_type.oid END AND dependency.deptype='n'")
            .bind(array).fetch_one(&pool).await.unwrap();
        assert!(
            dependencies > 0,
            "fixture must establish the real pg_type dependency: {name}"
        );
        refusal_preserves(&pool, "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY").await;
    }
}

#[tokio::test]
async fn all_four_reserved_residue_categories_block_readiness_and_canonical_replay() {
    let pool = ready_fixture("receipt149_residue_categories").await;
    let proof = evidence(&pool).await;
    let history: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(history) ORDER BY version) FROM _sqlx_migrations history",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let cases = [
        (
            "ALTER TABLE learning_delivery_apply_runs ADD COLUMN academic149_fixture bigint",
            "ALTER TABLE learning_delivery_apply_runs DROP COLUMN academic149_fixture",
        ),
        (
            "CREATE SEQUENCE academic149_fixture",
            "DROP SEQUENCE academic149_fixture",
        ),
        (
            "CREATE FUNCTION academic149_fixture() RETURNS integer LANGUAGE sql AS 'SELECT 1'",
            "DROP FUNCTION academic149_fixture()",
        ),
        (
            "CREATE TYPE academic149_fixture AS ENUM('held')",
            "DROP TYPE academic149_fixture",
        ),
    ];
    for (create, drop) in cases {
        execute(&pool, create).await;
        readiness(&pool, false).await;
        let failure = canonical(&pool)
            .await
            .expect_err("replay must refuse residual staging");
        assert_eq!(
            failure.diagnostic.stage,
            school_migrations::MigrationStage::Execution
        );
        assert_eq!(
            failure.diagnostic.error_class,
            school_migrations::MigrationErrorClass::Source
        );
        assert_eq!(failure.diagnostic.version, None);
        assert_eq!(failure.diagnostic.sqlstate, None);
        assert_eq!(failure.into_legacy_message(), "Migration failed: source");
        assert_eq!(evidence(&pool).await, proof);
        assert_eq!(sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(history) ORDER BY version) FROM _sqlx_migrations history").fetch_one(&pool).await.unwrap(),history);
        execute(&pool, drop).await;
        readiness(&pool, true).await;
        canonical(&pool).await.unwrap();
        assert_eq!(evidence(&pool).await, proof);
    }
}

async fn replace_fixture_evidence(pool: &PgPool, proof: Option<&Value>) {
    // Explicit fixture-only corruption; protection is restored before the provider
    // runs, so a disabled trigger cannot conceal a decoder/validator defect.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE academic_context_receipt_recovery_audit DISABLE TRIGGER academic_context_receipt_recovery_audit_immutable").execute(&mut *tx).await.unwrap();
    if let Some(proof) = proof {
        sqlx::query("INSERT INTO academic_context_receipt_recovery_audit(singleton,evidence) VALUES(true,$1) ON CONFLICT(singleton) DO UPDATE SET evidence=EXCLUDED.evidence")
            .bind(proof).execute(&mut *tx).await.unwrap();
    } else {
        sqlx::query("DELETE FROM academic_context_receipt_recovery_audit")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    sqlx::query("ALTER TABLE academic_context_receipt_recovery_audit ENABLE TRIGGER academic_context_receipt_recovery_audit_immutable").execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn missing_malformed_and_phase_inconsistent_evidence_fail_only_recovery_readiness() {
    let pool = ready_fixture("receipt149_bad_evidence").await;
    let baseline = evidence(&pool).await;
    replace_fixture_evidence(&pool, None).await;
    readiness(&pool, false).await;
    let mut malformed = vec![Value::Null, json!({"protocolVersion":1})];
    let mut absent_nullable = baseline.clone();
    absent_nullable
        .as_object_mut()
        .unwrap()
        .remove("projectedSha256");
    malformed.push(absent_nullable);
    let mut unknown_key = baseline.clone();
    unknown_key["unexpected"] = json!(true);
    malformed.push(unknown_key);
    for (field, value) in [
        ("finalSha256", json!("0".repeat(64))),
        ("native148LinkSha256", json!("0".repeat(64))),
        (
            "native148ReceiptCount",
            json!(baseline["native148ReceiptCount"].as_i64().unwrap() + 1),
        ),
        (
            "receiptCount",
            json!(baseline["receiptCount"].as_i64().unwrap() + 1),
        ),
        (
            "recoveredReceiptCount",
            json!(baseline["receiptCount"].as_i64().unwrap() + 1),
        ),
        ("projectedSha256", json!("0".repeat(64))),
        ("nonreceiptSha256", json!("0".repeat(64))),
        ("legacyDatesUnbound", json!(false)),
        ("phaseProofPassed", json!(false)),
    ] {
        let mut changed = baseline.clone();
        changed[field] = value;
        malformed.push(changed);
    }
    for proof in malformed {
        replace_fixture_evidence(&pool, Some(&proof)).await;
        readiness(&pool, false).await;
    }
    replace_fixture_evidence(&pool, Some(&baseline)).await;
    readiness(&pool, true).await;

    // A changed native immutable counter invalidates the phase link even when
    // every 149 field is the previously valid value and the first five checks pass.
    execute(&pool,"ALTER TABLE academic_context_cutover_audit DISABLE TRIGGER academic_context_cutover_audit_immutable; UPDATE academic_context_cutover_audit SET mapped_calendar_targets=mapped_calendar_targets+1; ALTER TABLE academic_context_cutover_audit ENABLE TRIGGER academic_context_cutover_audit_immutable").await;
    readiness(&pool, false).await;
    execute(&pool,"ALTER TABLE academic_context_cutover_audit DISABLE TRIGGER academic_context_cutover_audit_immutable; UPDATE academic_context_cutover_audit SET mapped_calendar_targets=mapped_calendar_targets-1; ALTER TABLE academic_context_cutover_audit ENABLE TRIGGER academic_context_cutover_audit_immutable").await;
    readiness(&pool, true).await;
}

#[tokio::test]
async fn immutable_proof_and_exact_enabled_trigger_are_required() {
    let pool = ready_fixture("receipt149_immutable_proof").await;
    let baseline = evidence(&pool).await;
    for statement in [
        "UPDATE academic_context_receipt_recovery_audit SET evidence=evidence",
        "DELETE FROM academic_context_receipt_recovery_audit",
    ] {
        let failure = sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            failure.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
        assert_eq!(evidence(&pool).await, baseline);
    }
    execute(&pool,"ALTER TABLE academic_context_receipt_recovery_audit DISABLE TRIGGER academic_context_receipt_recovery_audit_immutable").await;
    readiness(&pool, false).await;
    execute(&pool,"ALTER TABLE academic_context_receipt_recovery_audit ENABLE TRIGGER academic_context_receipt_recovery_audit_immutable").await;
    readiness(&pool, true).await;
    execute(&pool,"DROP TRIGGER academic_context_receipt_recovery_audit_immutable ON academic_context_receipt_recovery_audit").await;
    readiness(&pool, false).await;
    execute(&pool,"CREATE FUNCTION fixture_recovery_trigger() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END $$; CREATE TRIGGER academic_context_receipt_recovery_audit_immutable BEFORE UPDATE OR DELETE ON academic_context_receipt_recovery_audit FOR EACH ROW EXECUTE FUNCTION fixture_recovery_trigger()").await;
    readiness(&pool, false).await;
    execute(&pool,"DROP TRIGGER academic_context_receipt_recovery_audit_immutable ON academic_context_receipt_recovery_audit; CREATE TRIGGER academic_context_receipt_recovery_audit_immutable BEFORE UPDATE OR DELETE ON academic_context_receipt_recovery_audit FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation(); DROP FUNCTION fixture_recovery_trigger()").await;
    readiness(&pool, true).await;
    assert_eq!(evidence(&pool).await, baseline);
    execute(&pool,"ALTER TABLE academic_context_receipt_recovery_audit RENAME TO fixture_missing_recovery_audit").await;
    readiness(&pool, false).await;
    execute(&pool,"ALTER TABLE fixture_missing_recovery_audit RENAME TO academic_context_receipt_recovery_audit").await;
    readiness(&pool, true).await;
}
