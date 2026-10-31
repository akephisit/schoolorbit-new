//! Native owner-runner proofs for the finite receipt transition, never tenant data.
#[path = "../crates/school-migrations/src/academic149.rs"]
mod academic149;

use school_test_db::academic_context_receipt_fixture::{
    seed_legacy_curriculum_receipt, LegacyCurriculumReceipt,
};
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn predecessor(name: &str, version: i64) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 1).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, version).await.unwrap();
    pool
}

async fn receipt_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(receipt) ORDER BY idempotency_key),'[]'::jsonb) FROM learning_delivery_apply_runs receipt")
        .fetch_one(pool).await.unwrap()
}

async fn column_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('number',attribute.attnum,'type',attribute.atttypid,'notNull',attribute.attnotnull,'defaultOid',definition.oid,'default',definition.adbin::text,'constraints',(SELECT jsonb_agg(jsonb_build_object('oid',rule.oid,'expression',rule.conbin::text) ORDER BY rule.oid) FROM pg_constraint rule WHERE rule.conrelid=attribute.attrelid AND attribute.attnum=ANY(rule.conkey))) FROM pg_attribute attribute LEFT JOIN pg_attrdef definition ON definition.adrelid=attribute.attrelid AND definition.adnum=attribute.attnum WHERE attribute.attrelid='learning_delivery_apply_runs'::regclass AND attribute.attname='created_offering_count' AND NOT attribute.attisdropped")
        .fetch_one(pool).await.unwrap()
}

async fn lower_match(pool: &PgPool) -> LegacyCurriculumReceipt {
    let receipt = seed_legacy_curriculum_receipt(pool, 1).await;
    let (id,offering,detail):(Uuid,Value,Value)=sqlx::query_as("SELECT offering.id,to_jsonb(offering),to_jsonb(detail) FROM learning_delivery_apply_runs receipt JOIN learning_offerings offering ON offering.id=ANY(receipt.offering_ids) AND offering.created_at=receipt.created_at JOIN course_offering_details detail ON detail.learning_offering_id=offering.id WHERE receipt.idempotency_key=$1")
        .bind(receipt.request_id).fetch_one(pool).await.unwrap();
    let before = receipt_facts(pool).await;
    // Same-ID deletion/recreation is a source-confirmed historical lifecycle;
    // the separate observer regression exercises its full 089 producer/deleter.
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        sqlx::query("DELETE FROM learning_offerings WHERE id=$1 AND status='draft'")
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,$1::jsonb||jsonb_build_object('created_at',now(),'updated_at',now()))).*")
        .bind(offering).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,$1::jsonb)).*")
        .bind(detail).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(receipt_facts(pool).await, before);
    receipt
}

async fn canonical(pool: &PgPool) {
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        school_migrations::run_tenant_migrations_detailed(pool),
    )
    .await
    .expect("canonical runner must release a pool-size-one lease")
    .unwrap();
    assert_eq!(
        school_migrations::verify_tenant_schema(pool).await.unwrap(),
        149
    );
}

async fn ready(pool: &PgPool) {
    let status =
        school_academic_core::context_reconciliation::read_academic_context_cutover_audit(pool)
            .await
            .unwrap();
    assert!(status.completed, "{:?}", status.checks);
    assert_eq!(status.checks.len(), 6);
    assert!(status
        .checks
        .iter()
        .any(|check| check.code == "ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE" && check.passed));
}

#[tokio::test]
async fn canonical_recovery_preserves_original_column_receipts_dates_and_old_key_replay() {
    let pool = predecessor("receipt149_recover", 147).await;
    let receipt = lower_match(&pool).await;
    let before = receipt_facts(&pool).await;
    let column = column_facts(&pool).await;
    let dates:Value=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(id,starts_on,ends_on,created_at) ORDER BY id) FROM learning_offerings").fetch_one(&pool).await.unwrap();
    canonical(&pool).await;
    ready(&pool).await;
    assert_eq!(column_facts(&pool).await, column);
    let normalized:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(receipt)-'starts_on' ORDER BY idempotency_key) FROM learning_delivery_apply_runs receipt").fetch_one(&pool).await.unwrap();
    assert_eq!(normalized, before);
    assert_eq!(sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(jsonb_build_array(id,starts_on,ends_on,created_at) ORDER BY id) FROM learning_offerings").fetch_one(&pool).await.unwrap(),dates);
    let evidence: Value =
        sqlx::query_scalar("SELECT evidence FROM academic_context_receipt_recovery_audit")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(evidence["sourceVersion"], 147);
    assert_eq!(evidence["recoveredReceiptCount"], 1);
    assert_eq!(evidence["originalSha256"], evidence["finalSha256"]);
    assert_ne!(evidence["projectedSha256"], evidence["originalSha256"]);
    let recovered_receipts = receipt_facts(&pool).await;
    let expected_ids: Vec<Uuid> =
        serde_json::from_value(before[0]["offering_ids"].clone()).unwrap();
    let expected_count = before[0]["created_offering_count"].as_u64().unwrap() as usize;
    let mut body = receipt.request_body;
    body["startsOn"] = json!("2026-05-01");
    let request: school_academic_delivery::models::ApplyCurriculumOfferingsRequest =
        serde_json::from_value(body).unwrap();
    let replay = school_academic_delivery::services::offerings::apply_from_curriculum(
        &pool,
        school_test_db::academic_context_receipt_fixture::ACTOR,
        request,
    )
    .await
    .unwrap();
    assert_eq!(replay.created_offering_count, expected_count);
    assert_eq!(replay.source_hash, "c".repeat(64));
    assert_eq!(replay.offering_ids, expected_ids);
    assert_eq!(receipt_facts(&pool).await, recovered_receipts);
    assert_eq!(sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(jsonb_build_array(id,starts_on,ends_on,created_at) ORDER BY id) FROM learning_offerings").fetch_one(&pool).await.unwrap(),dates);
    canonical(&pool).await;
    ready(&pool).await;
    assert_eq!(column_facts(&pool).await, column);
    assert_eq!(
        sqlx::query_scalar::<_, Value>(
            "SELECT evidence FROM academic_context_receipt_recovery_audit"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        evidence
    );
}

#[tokio::test]
async fn existing148_preserves_nonnull_dates_and_does_not_assert_legacy_null_intent() {
    let pool = predecessor("receipt149_existing148", 147).await;
    let receipt = seed_legacy_curriculum_receipt(&pool, 1).await;
    apply_migrations_through(&pool, 148).await.unwrap();
    // A separate post148 receipt accepts a nonnull date at insertion. The older
    // legacy receipt and its unbound date intent remain untouched.
    sqlx::query("INSERT INTO learning_delivery_apply_runs SELECT (jsonb_populate_record(NULL::learning_delivery_apply_runs,to_jsonb(receipt)||jsonb_build_object('idempotency_key',$1::uuid,'starts_on','2026-05-03'))).* FROM learning_delivery_apply_runs receipt WHERE idempotency_key=$2")
        .bind(Uuid::new_v4()).bind(receipt.request_id).execute(&pool).await.unwrap();
    let before = receipt_facts(&pool).await;
    canonical(&pool).await;
    assert_eq!(receipt_facts(&pool).await, before);
    ready(&pool).await;
    let evidence: Value =
        sqlx::query_scalar("SELECT evidence FROM academic_context_receipt_recovery_audit")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(evidence["sourceVersion"], 148);
    assert_eq!(evidence["recoveredReceiptCount"], 0);
    assert!(
        evidence["projectedSha256"].is_null()
            && evidence["nonreceiptSha256"].is_null()
            && evidence["legacyDatesUnbound"].is_null()
    );
}

#[tokio::test]
async fn future_no_tx_migration_remains_outside_the_finite149_envelope() {
    use std::borrow::Cow;
    let pool = predecessor("receipt149_future", 147).await;
    lower_match(&pool).await;
    let mut migrations = sqlx::migrate!("./migrations");
    let mut entries = migrations.iter().cloned().collect::<Vec<_>>();
    entries.push(sqlx::migrate::Migration::new(
        150,
        Cow::Borrowed("synthetic no_tx future"),
        sqlx::migrate::MigrationType::Simple,
        sqlx::SqlStr::from_static("VACUUM ANALYZE learning_delivery_apply_runs;"),
        true,
    ));
    migrations.migrations = Cow::Owned(entries);
    migrations.locking = false;
    {
        let mut connection = pool.acquire().await.unwrap();
        academic149::run_transition(&mut connection, &migrations)
            .await
            .unwrap();
        academic149::run_transition(&mut connection, &migrations)
            .await
            .unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap(),
        150
    );
    ready(&pool).await;
}

#[tokio::test]
async fn fresh_empty_schema_uses_same_canonical_transition_with_pool_size_one() {
    let pool =
        school_test_db::create_named_test_pool_with_max_connections("receipt149_fresh", 1).await;
    canonical(&pool).await;
    ready(&pool).await;
    let evidence: Value =
        sqlx::query_scalar("SELECT evidence FROM academic_context_receipt_recovery_audit")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(evidence["sourceVersion"], 147);
    assert_eq!(evidence["receiptCount"], 0);
}
