//! Synthetic, read-only observations of real 147/148 schemas; no tenant remediation.
use chrono::{DateTime, Duration, Utc};
use school_test_db::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, seed_release_two_predecessor, CutoverFixture,
};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use std::borrow::Cow;
use uuid::Uuid;

const OBSERVATIONS: &str = include_str!("../../scripts/lib/pipeline-academic148-observations.sql");
const TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000251);
const OTHER_TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000252);

fn manifest() -> Value {
    Value::Array(
        sqlx::migrate!("./migrations")
            .iter()
            .map(|migration| {
                json!({"version":migration.version,"checksum":hex::encode(&migration.checksum)})
            })
            .collect(),
    )
}

async fn fixture(name: &str, version: i64) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, version).await.unwrap();
    pool
}

fn instant(day: u32) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&format!("2001-01-{day:02}T00:00:00Z"))
        .unwrap()
        .with_timezone(&Utc)
}

async fn clone_offering_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    created_at: Option<DateTime<Utc>>,
) -> Uuid {
    let id = Uuid::new_v4();
    let inserted = sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'academic_term_id',$2::uuid,'academic_year_id',term.academic_year_id,'status','draft','published_at',NULL,'publish_idempotency_key',NULL,'name_snapshot','private-observation-sentinel','created_at',COALESCE($3::timestamptz,now()),'updated_at',COALESCE($3::timestamptz,now())))).* FROM learning_offerings source JOIN activity_offering_details detail ON detail.learning_offering_id=source.id CROSS JOIN academic_terms term WHERE source.academic_term_id=$4 AND term.id=$2 ORDER BY source.id LIMIT 1")
        .bind(id).bind(term).bind(created_at).bind(TERM).execute(&mut **tx).await.unwrap();
    assert_eq!(inserted.rows_affected(), 1);
    let inserted = sqlx::query("INSERT INTO activity_offering_details SELECT (jsonb_populate_record(NULL::activity_offering_details,to_jsonb(detail)||jsonb_build_object('learning_offering_id',$1::uuid,'academic_term_id',$2::uuid,'academic_year_id',term.academic_year_id))).* FROM activity_offering_details detail CROSS JOIN academic_terms term WHERE detail.academic_term_id=$3 AND term.id=$2 ORDER BY detail.learning_offering_id LIMIT 1")
        .bind(id).bind(term).bind(TERM).execute(&mut **tx).await.unwrap();
    assert_eq!(inserted.rows_affected(), 1);
    id
}

async fn clone_offering(pool: &PgPool, term: Uuid, created_at: DateTime<Utc>) -> Uuid {
    let mut tx = pool.begin().await.unwrap();
    let id = clone_offering_in_transaction(&mut tx, term, Some(created_at)).await;
    tx.commit().await.unwrap();
    id
}

async fn receipt(
    pool: &PgPool,
    ids: Vec<Option<Uuid>>,
    recorded_created: i32,
    created_at: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_delivery_apply_runs(idempotency_key,academic_term_id,request_hash,source_hash,offering_ids,created_offering_count,created_at) VALUES($1,$2,repeat('c',64),repeat('d',64),$3,$4,$5)")
        .bind(id).bind(TERM).bind(ids).bind(recorded_created).bind(created_at).execute(pool).await.unwrap();
    id
}

async fn facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('receipts',(SELECT jsonb_build_array(count(*),md5(COALESCE(string_agg(md5(to_jsonb(r)::text),'' ORDER BY idempotency_key),''))) FROM learning_delivery_apply_runs r),'offerings',(SELECT jsonb_build_array(count(*),md5(COALESCE(string_agg(md5(to_jsonb(o)::text),'' ORDER BY id),''))) FROM learning_offerings o),'course_details',(SELECT jsonb_build_array(count(*),md5(COALESCE(string_agg(md5(to_jsonb(d)::text),'' ORDER BY learning_offering_id),''))) FROM course_offering_details d),'activity_details',(SELECT jsonb_build_array(count(*),md5(COALESCE(string_agg(md5(to_jsonb(d)::text),'' ORDER BY learning_offering_id),''))) FROM activity_offering_details d),'audit',(SELECT jsonb_build_array(count(*),md5(COALESCE(string_agg(md5(to_jsonb(a)::text),'' ORDER BY id),''))) FROM academic_audit_events a),'history',(SELECT jsonb_build_array(count(*),md5(COALESCE(string_agg(md5(to_jsonb(h)::text),'' ORDER BY version),''))) FROM _sqlx_migrations h))")
        .fetch_one(pool).await.unwrap()
}

async fn observe_with_manifest(pool: &PgPool, manifest: Value) -> Option<Value> {
    let before = facts(pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    let modes: (String, String) = sqlx::query_as(
        "SELECT current_setting('transaction_read_only'),current_setting('transaction_isolation')",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(modes, ("on".into(), "repeatable read".into()));
    let result = sqlx::query_scalar(OBSERVATIONS)
        .bind(manifest)
        .fetch_optional(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        facts(pool).await,
        before,
        "observation changed source facts"
    );
    result
}

async fn observe(pool: &PgPool) -> Value {
    observe_with_manifest(pool, manifest()).await.unwrap()
}

fn assert_contract(report: &Value) {
    assert_eq!(report.as_object().unwrap().len(), 8);
    assert_eq!(report["operation"], "academic148_curriculum");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["history_verified"], true);
    assert!(matches!(
        report["history_version"].as_u64(),
        Some(147 | 148)
    ));
    let counts = report["counts"].as_object().unwrap();
    assert_eq!(counts.len(), 16);
    assert!(counts.values().all(|value| value.as_u64().is_some()));
    let masks = report["masks"].as_array().unwrap();
    assert_eq!(masks.len(), 16);
    for (mask, row) in masks.iter().enumerate() {
        assert_eq!(row.as_object().unwrap().len(), 2);
        assert_eq!(row["mask"], mask);
    }
    assert_eq!(
        masks
            .iter()
            .map(|row| row["count"].as_u64().unwrap())
            .sum::<u64>(),
        counts["receipts"].as_u64().unwrap()
    );
    assert_eq!(
        masks[1..]
            .iter()
            .map(|row| row["count"].as_u64().unwrap())
            .sum::<u64>(),
        counts["violating_receipts"].as_u64().unwrap()
    );
    let timing = report["timing"].as_array().unwrap();
    for (row, kind) in timing
        .iter()
        .zip(["before050", "equal050", "after050", "unknown050"])
    {
        assert_eq!(row.as_object().unwrap().len(), 4);
        assert_eq!(row["kind"], kind);
    }
    for (key, total) in [
        ("count", "receipts"),
        ("violating", "violating_receipts"),
        (
            "zero_recorded_positive_created",
            "zero_recorded_positive_created",
        ),
    ] {
        assert_eq!(
            timing
                .iter()
                .map(|row| row[key].as_u64().unwrap())
                .sum::<u64>(),
            counts[total].as_u64().unwrap()
        );
    }
    let checkpoints = report["checkpoints"].as_array().unwrap();
    assert_eq!(checkpoints.len(), 2);
    for (row, version) in checkpoints.iter().zip([138, 141]) {
        assert_eq!(row.as_object().unwrap().len(), 5);
        assert_eq!(row["version"], version);
        for key in ["present", "unique", "count_match", "hash_match"] {
            assert!(row[key].is_boolean());
        }
    }
    let text = report.to_string();
    assert!(!text.contains("private-observation-sentinel"));
    assert!(!text.contains(&"c".repeat(64)) && !text.contains(&"d".repeat(64)));
}

#[tokio::test]
async fn original_four_predicates_keep_multiplicity_and_report_only_aggregate_decompositions() {
    let pool = fixture("observations148_predicates", 147).await;
    let old_zero = clone_offering(&pool, TERM, instant(1)).await;
    let private_id = receipt(&pool, vec![Some(old_zero)], 0, instant(1)).await;
    receipt(&pool, vec![], 0, instant(2)).await;
    let retained = clone_offering(&pool, TERM, instant(3)).await;
    let created = clone_offering(&pool, TERM, instant(4)).await;
    receipt(&pool, vec![Some(created), Some(retained)], 1, instant(4)).await;
    receipt(&pool, vec![], 2, instant(5)).await;
    let selected = clone_offering(&pool, TERM, instant(6)).await;
    clone_offering(&pool, TERM, instant(6)).await;
    receipt(&pool, vec![Some(selected)], 1, instant(6)).await;
    let wrong_term = clone_offering(&pool, OTHER_TERM, instant(7)).await;
    receipt(
        &pool,
        vec![
            Some(retained),
            Some(retained),
            Some(Uuid::new_v4()),
            Some(wrong_term),
            None,
        ],
        0,
        instant(8),
    )
    .await;
    let duplicated = clone_offering(&pool, TERM, instant(9)).await;
    receipt(
        &pool,
        vec![Some(duplicated), Some(duplicated), Some(Uuid::new_v4())],
        0,
        instant(9),
    )
    .await;
    let first = clone_offering(&pool, TERM, instant(10)).await;
    let second = clone_offering(&pool, TERM, instant(10)).await;
    receipt(&pool, vec![Some(first)], 1, instant(10)).await;
    receipt(&pool, vec![Some(second)], 1, instant(10)).await;
    let report = observe(&pool).await;
    assert_contract(&report);
    assert!(!report.to_string().contains(&private_id.to_string()));
    assert_eq!(
        report["counts"],
        json!({"receipts":9,"violating_receipts":7,"created_count_mismatches":3,"matched_created_lower":1,"matched_created_higher":2,"zero_recorded_positive_created":2,"all_created_count_mismatches":4,"invalid_output_receipts":2,"nonunique_output_receipts":2,"missing_output_entries":2,"wrong_context_output_entries":1,"null_output_entries":1,"duplicate_nonnull_output_entries":2,"extra_timestamp_offerings":3,"shared_timestamp_cohorts":1,"shared_timestamp_receipts":2})
    );
    for (mask, count) in [(0, 2), (1, 2), (2, 3), (12, 1), (15, 1)] {
        assert_eq!(report["masks"][mask]["count"], count);
    }
    let refusal = apply_migrations_through(&pool, 148).await.unwrap_err();
    assert!(refusal
        .to_string()
        .contains("ACADEMIC_CONTEXT_148_CURRICULUM_CREATED_INTENT_UNMAPPABLE"));
    assert_eq!(observe(&pool).await, report, "failed 148 must roll back");
}

#[tokio::test]
async fn real050_default_zero_is_observed_without_rewriting_the_accepted_receipt() {
    let pool = school_test_db::create_named_test_pool("observations148_real050").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    // Finish the real 045 checksum gate before accepting runtime work; 050 is still pending.
    apply_migrations_through(&pool, 49).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let offering = clone_offering_in_transaction(&mut tx, TERM, None).await;
    let id = Uuid::new_v4();
    // Real pre050 shape: the producer omitted both created_at fields and stored no counts.
    sqlx::query("INSERT INTO learning_delivery_apply_runs(idempotency_key,academic_term_id,request_hash,source_hash,offering_ids) VALUES($1,$2,repeat('c',64),repeat('d',64),$3)")
        .bind(id).bind(TERM).bind(vec![offering]).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    apply_migrations_through(&pool, 147).await.unwrap();
    let report = observe(&pool).await;
    assert_contract(&report);
    assert_eq!(report["counts"]["zero_recorded_positive_created"], 1);
    assert_eq!(report["masks"][1]["count"], 1);
    assert_eq!(report["timing"][0]["zero_recorded_positive_created"], 1);
    assert_eq!(sqlx::query_scalar::<_, i32>("SELECT created_offering_count FROM learning_delivery_apply_runs WHERE idempotency_key=$1").bind(id).fetch_one(&pool).await.unwrap(), 0);
}

#[tokio::test]
async fn timing_boundaries_and_real138141_checkpoints_survive148_without_provenance_claims() {
    let pool = fixture("observations148_checkpoints", 137).await;
    let installed: DateTime<Utc> =
        sqlx::query_scalar("SELECT installed_on FROM _sqlx_migrations WHERE version=50")
            .fetch_one(&pool)
            .await
            .unwrap();
    for date in [
        installed - Duration::days(1),
        installed,
        installed + Duration::days(1),
    ] {
        receipt(&pool, vec![], 0, date).await;
    }
    assert!(observe_with_manifest(&pool, manifest()).await.is_none());
    apply_migrations_through(&pool, 147).await.unwrap();
    let predecessor = observe(&pool).await;
    assert_contract(&predecessor);
    for row in predecessor["checkpoints"].as_array().unwrap() {
        assert_eq!(
            row,
            &json!({"version":row["version"],"present":true,"unique":true,"count_match":true,"hash_match":true})
        );
    }
    assert_eq!(
        predecessor["timing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["count"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 0]
    );
    apply_migrations_through(&pool, 148).await.unwrap();
    let current = observe(&pool).await;
    assert_eq!(current["history_version"], 148);
    assert_eq!(current["checkpoints"], predecessor["checkpoints"]);
    // A changed ledger representation can alter a checkpoint hash without changing count.
    sqlx::query("UPDATE learning_delivery_apply_runs SET source_hash=repeat('e',64)")
        .execute(&pool)
        .await
        .unwrap();
    let changed = observe(&pool).await;
    for row in changed["checkpoints"].as_array().unwrap() {
        assert_eq!(row["count_match"], true);
        assert_eq!(row["hash_match"], false);
    }
    receipt(&pool, vec![], 0, instant(11)).await;
    for row in observe(&pool).await["checkpoints"].as_array().unwrap() {
        assert_eq!(row["count_match"], false);
        assert_eq!(row["hash_match"], false);
    }
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,payload) SELECT event_code,entity_type,payload FROM academic_audit_events WHERE event_code='catalog.start_only_reconciled'").execute(&pool).await.unwrap();
    sqlx::query(
        "DELETE FROM academic_audit_events WHERE event_code='catalog.grade_snapshots_reconciled'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let ambiguity = observe(&pool).await;
    assert_eq!(
        ambiguity["checkpoints"][0],
        json!({"version":138,"present":true,"unique":false,"count_match":false,"hash_match":false})
    );
    assert_eq!(
        ambiguity["checkpoints"][1],
        json!({"version":141,"present":false,"unique":false,"count_match":false,"hash_match":false})
    );
}

#[tokio::test]
async fn history_checksums_prefix_and_read_only_snapshot_fail_closed() {
    let pool = fixture("observations148_history", 146).await;
    assert!(observe_with_manifest(&pool, manifest()).await.is_none());
    apply_migrations_through(&pool, 147).await.unwrap();
    for altered in [
        {
            let mut value = manifest();
            value[0]["checksum"] = json!("0".repeat(96));
            value
        },
        {
            let mut value = manifest();
            value[0]["version"] = json!(2);
            value
        },
        {
            let mut value = manifest();
            value[0]["checksum"] = Value::Null;
            value
        },
        json!([]),
    ] {
        assert!(observe_with_manifest(&pool, altered).await.is_none());
    }
    assert!(sqlx::query_scalar::<_, Value>(OBSERVATIONS)
        .bind(manifest())
        .fetch_optional(&pool)
        .await
        .unwrap()
        .is_none());
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::query("UPDATE learning_delivery_apply_runs SET created_offering_count=0")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("25006")
    );
    tx.rollback().await.unwrap();
    let mut migrator = sqlx::migrate!("./migrations");
    let mut migrations = migrator.iter().cloned().collect::<Vec<_>>();
    migrations.push(sqlx::migrate::Migration::new(
        149,
        Cow::Borrowed("synthetic future observation test"),
        sqlx::migrate::MigrationType::Simple,
        sqlx::SqlStr::from_static("SELECT 1;"),
        false,
    ));
    migrator.migrations = Cow::Owned(migrations);
    migrator.locking = false;
    school_test_db::run_fixture_migrator(&pool, &migrator)
        .await
        .unwrap();
    assert!(observe_with_manifest(&pool, manifest()).await.is_none());
}

#[tokio::test]
async fn receipt_and_output_budgets_refuse_the_whole_report_without_truncation() {
    let pool = fixture("observations148_budgets", 147).await;
    let id = receipt(&pool, vec![Some(Uuid::new_v4()); 1_000_001], 0, instant(12)).await;
    assert!(observe_with_manifest(&pool, manifest()).await.is_none());
    sqlx::query("DELETE FROM learning_delivery_apply_runs WHERE idempotency_key=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO learning_delivery_apply_runs(idempotency_key,academic_term_id,request_hash,source_hash,offering_ids) SELECT uuid_generate_v5($1,number::text),$2,repeat('c',64),repeat('d',64),ARRAY[]::uuid[] FROM generate_series(1,100001) number")
        .bind(Uuid::new_v4()).bind(TERM).execute(&pool).await.unwrap();
    // The provider returns no partial count of the first 100000 receipts.
    assert!(observe_with_manifest(&pool, manifest()).await.is_none());
    sqlx::query("DELETE FROM learning_delivery_apply_runs")
        .execute(&pool)
        .await
        .unwrap();
    let source: Uuid = sqlx::query_scalar("SELECT learning_offering_id FROM activity_offering_details WHERE academic_term_id=$1 ORDER BY learning_offering_id LIMIT 1")
        .bind(TERM).fetch_one(&pool).await.unwrap();
    let namespace = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(source)||jsonb_build_object('id',uuid_generate_v5($1,number::text),'status','draft','published_at',NULL,'publish_idempotency_key',NULL))).* FROM learning_offerings source CROSS JOIN generate_series(1,100001) number WHERE source.id=$2")
        .bind(namespace).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO activity_offering_details SELECT (jsonb_populate_record(NULL::activity_offering_details,to_jsonb(source)||jsonb_build_object('learning_offering_id',uuid_generate_v5($1,number::text)))).* FROM activity_offering_details source CROSS JOIN generate_series(1,100001) number WHERE source.learning_offering_id=$2")
        .bind(namespace).bind(source).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert!(observe_with_manifest(&pool, manifest()).await.is_none());
}
