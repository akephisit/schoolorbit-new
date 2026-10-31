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
use sqlx::{Connection, PgConnection, PgPool, SqlSafeStr};
use std::{borrow::Cow, time::Duration};
use uuid::Uuid;

async fn predecessor(name: &str, version: i64) -> PgPool {
    predecessor_with_connections(name, version, 1).await
}

async fn predecessor_with_connections(name: &str, version: i64, connections: u32) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(
        &name.to_ascii_lowercase(),
        connections,
    )
    .await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, version).await.unwrap();
    pool
}

const PHASE_BOUNDARY: &str = "-- ACADEMIC_CONTEXT_149_FINALIZE\n";
const ORIGINAL_FACTS:&str="SELECT jsonb_build_object('receipts',(SELECT COALESCE(jsonb_agg(to_jsonb(receipt) ORDER BY idempotency_key),'[]'::jsonb) FROM learning_delivery_apply_runs receipt),'history',(SELECT jsonb_agg(to_jsonb(history) ORDER BY version) FROM _sqlx_migrations history),'offerings',(SELECT jsonb_agg(to_jsonb(offering) ORDER BY id) FROM learning_offerings offering),'column',(SELECT jsonb_build_object('number',attribute.attnum,'type',attribute.atttypid,'notNull',attribute.attnotnull,'defaultOid',definition.oid,'default',definition.adbin::text,'constraints',(SELECT jsonb_agg(jsonb_build_object('oid',rule.oid,'expression',rule.conbin::text) ORDER BY rule.oid) FROM pg_constraint rule WHERE rule.conrelid=attribute.attrelid AND attribute.attnum=ANY(rule.conkey))) FROM pg_attribute attribute LEFT JOIN pg_attrdef definition ON definition.adrelid=attribute.attrelid AND definition.adnum=attribute.attnum WHERE attribute.attrelid='learning_delivery_apply_runs'::regclass AND attribute.attname='created_offering_count' AND NOT attribute.attisdropped))";

struct Original {
    schema: String,
    relation: i64,
    facts: Value,
}

async fn original(pool: &PgPool) -> Original {
    let (schema, relation): (String, i64) = sqlx::query_as(
        "SELECT current_schema(),'learning_delivery_apply_runs'::regclass::oid::bigint",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    Original {
        schema,
        relation,
        facts: sqlx::query_scalar(ORIGINAL_FACTS)
            .fetch_one(pool)
            .await
            .unwrap(),
    }
}

async fn fresh(pool: &PgPool, schema: &str) -> PgConnection {
    let mut connection = PgConnection::connect_with(pool.connect_options().as_ref())
        .await
        .unwrap();
    sqlx::query("SELECT set_config('search_path',$1,false)")
        .bind(format!("\"{}\",public", schema.replace('"', "\"\"")))
        .execute(&mut connection)
        .await
        .unwrap();
    connection
}

async fn restored_on_fresh_connection(pool: &PgPool, before: &Original) {
    let mut connection = fresh(pool, &before.schema).await;
    let actual: Value = tokio::time::timeout(
        Duration::from_secs(10),
        sqlx::query_scalar(ORIGINAL_FACTS).fetch_one(&mut connection),
    )
    .await
    .expect("abandoned transaction must release its locks")
    .unwrap();
    assert_eq!(
        actual, before.facts,
        "failed transition changed original committed facts"
    );
    let residue:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM pg_attribute WHERE attrelid=$1::bigint::oid AND NOT attisdropped AND starts_with(attname,'academic149_'))+(SELECT count(*) FROM pg_class WHERE relnamespace=current_schema()::regnamespace AND starts_with(relname,'academic149_'))+(SELECT count(*) FROM pg_proc WHERE pronamespace=current_schema()::regnamespace AND starts_with(proname,'academic149_'))+(SELECT count(*) FROM pg_type WHERE typnamespace=current_schema()::regnamespace AND starts_with(typname,'academic149_'))").bind(before.relation).fetch_one(&mut connection).await.unwrap();
    assert_eq!(residue, 0);
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('academic_context_receipt_recovery_audit') IS NOT NULL"
    )
    .fetch_one(&mut connection)
    .await
    .unwrap());
}

#[derive(Clone, Copy, Debug)]
enum FaultPhase {
    AfterProjection,
    After148,
    Inside149,
}

fn fixture_migrator(phase: FaultPhase, statement: &str) -> sqlx::migrate::Migrator {
    let mut migrations = sqlx::migrate!("./migrations");
    let entries=migrations.iter().map(|migration| {
        if migration.version!=149 { return migration.clone(); }
        let script=migration.sql.as_str();
        let (prepare,finalize)=script.split_once(PHASE_BOUNDARY).unwrap();
        let changed=match phase {
            FaultPhase::AfterProjection=>format!("{prepare}\n{statement}\n{PHASE_BOUNDARY}{finalize}"),
            // The first prefix execution is at147; the native149 execution is at148.
            FaultPhase::After148=>format!("DO $fixture$ BEGIN IF (SELECT max(version) FROM _sqlx_migrations)=148 THEN {statement} END IF; END $fixture$;\n{script}"),
            FaultPhase::Inside149=>format!("{script}\n{statement}"),
        };
        sqlx::migrate::Migration::new(149,Cow::Borrowed("private fault fixture"),migration.migration_type,sqlx::AssertSqlSafe(changed).into_sql_str(),false)
    }).collect::<Vec<_>>();
    migrations.migrations = Cow::Owned(entries);
    migrations.locking = false;
    migrations
}

struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn atomic_errors_after_projection_after148_and_inside149_restore_original_state() {
    for (phase, statement) in [
        (FaultPhase::AfterProjection, "SELECT 1/0;"),
        (FaultPhase::After148, "PERFORM 1/0;"),
        (FaultPhase::Inside149, "SELECT 1/0;"),
    ] {
        let pool = predecessor(&format!("receipt149_error_{phase:?}"), 147).await;
        lower_match(&pool).await;
        let before = original(&pool).await;
        let migrations = fixture_migrator(phase, statement);
        let error = {
            let mut connection = pool.acquire().await.unwrap();
            academic149::run_transition(&mut connection, &migrations)
                .await
                .unwrap_err()
        };
        match error {
            sqlx::migrate::MigrateError::ExecuteMigration(error, 149) => assert_eq!(
                error.as_database_error().unwrap().code().as_deref(),
                Some("22012")
            ),
            other => panic!("unexpected private fixture failure: {other:?}"),
        }
        restored_on_fresh_connection(&pool, &before).await;
        canonical(&pool).await;
        ready(&pool).await;
    }
}

#[tokio::test]
async fn atomic_cancellation_at_each_phase_never_exposes_committed148_or_projection() {
    for phase in [
        FaultPhase::AfterProjection,
        FaultPhase::After148,
        FaultPhase::Inside149,
    ] {
        let pool =
            predecessor_with_connections(&format!("receipt149_cancel_{phase:?}"), 147, 3).await;
        lower_match(&pool).await;
        let before = original(&pool).await;
        let mut observer = fresh(&pool, &before.schema).await;
        let key = Uuid::new_v4().as_u128() as i64;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(key)
            .execute(&mut observer)
            .await
            .unwrap();
        let statement = match phase {
            FaultPhase::After148 => format!("PERFORM pg_advisory_xact_lock({key});"),
            _ => format!("SELECT pg_advisory_xact_lock({key});"),
        };
        let migrations = fixture_migrator(phase, &statement);
        let task_pool = pool.clone();
        let (send_pid, receive_pid) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connection = task_pool.acquire().await.unwrap();
            let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            send_pid.send(pid).unwrap();
            academic149::run_transition(&mut connection, &migrations).await
        });
        let _abort = AbortOnDrop(task.abort_handle());
        let pid = receive_pid.await.unwrap();
        tokio::time::timeout(Duration::from_secs(10),async {
            loop {
                if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND NOT granted)").bind(pid).fetch_one(&mut observer).await.unwrap() {break;}
                assert!(!task.is_finished(),"fixture finished before its phase barrier");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("migration must reach the requested private phase");
        // System-catalog MVCC shows the original physical column. A normal data
        // reader blocks rather than observing the intermediate148/projection.
        let columns:i64=sqlx::query_scalar("SELECT count(*) FROM pg_attribute WHERE attrelid=$1::bigint::oid AND NOT attisdropped AND starts_with(attname,'academic149_')").bind(before.relation).fetch_one(&mut observer).await.unwrap();
        assert_eq!(columns, 0);
        let mut read = observer.begin().await.unwrap();
        let blocked = sqlx::query("LOCK TABLE _sqlx_migrations IN ACCESS SHARE MODE NOWAIT")
            .execute(&mut *read)
            .await
            .unwrap_err();
        assert_eq!(
            blocked.as_database_error().unwrap().code().as_deref(),
            Some("55P03")
        );
        read.rollback().await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(key)
            .execute(&mut observer)
            .await
            .unwrap();
        restored_on_fresh_connection(&pool, &before).await;
        canonical(&pool).await;
        ready(&pool).await;
    }
}

#[tokio::test]
async fn actual_public_runner_preserves_native148_refusal_and_rolls_back_projection() {
    let pool = predecessor("receipt149_native148_failure", 147).await;
    lower_match(&pool).await;
    sqlx::query(
        "CREATE FUNCTION fixture_unknown_date_dependency() RETURNS date LANGUAGE plpgsql AS $$ BEGIN RETURN (SELECT END_DATE FROM ACADEMIC_YEARS LIMIT 1); END $$",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before = original(&pool).await;
    let failure = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(failure.diagnostic.version, Some(148));
    assert_eq!(
        failure.diagnostic.code,
        "ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY"
    );
    restored_on_fresh_connection(&pool, &before).await;
    sqlx::query("DROP FUNCTION fixture_unknown_date_dependency()")
        .execute(&pool)
        .await
        .unwrap();
    canonical(&pool).await;
    ready(&pool).await;
}

#[tokio::test]
async fn disconnect_inside149_preserves_primary_error_and_original_committed_state() {
    let pool = predecessor("receipt149_disconnect", 147).await;
    lower_match(&pool).await;
    let before = original(&pool).await;
    let migrations = fixture_migrator(
        FaultPhase::Inside149,
        "SELECT pg_terminate_backend(pg_backend_pid());",
    );
    let error = {
        let mut connection = pool.acquire().await.unwrap();
        academic149::run_transition(&mut connection, &migrations)
            .await
            .unwrap_err()
    };
    match error {
        sqlx::migrate::MigrateError::ExecuteMigration(error, 149) => {
            assert!(["connection", "database", "protocol"]
                .contains(&school_database::error_class(&error)))
        }
        other => panic!("cleanup replaced the original149 failure: {other:?}"),
    }
    // This disconnect occurred inside the script, before COMMIT was attempted.
    // COMMIT disconnects remain ambiguous and require fresh-history reconciliation.
    restored_on_fresh_connection(&pool, &before).await;
    canonical(&pool).await;
}

#[tokio::test]
async fn concurrent_canonical_owners_publish_one_final_phase_and_never_intermediate_state() {
    let pool = predecessor_with_connections("receipt149_concurrent", 147, 3).await;
    lower_match(&pool).await;
    let before = original(&pool).await;
    let mut observer = fresh(&pool, &before.schema).await;
    let key = Uuid::new_v4().as_u128() as i64;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut observer)
        .await
        .unwrap();
    let migrations = std::sync::Arc::new(fixture_migrator(
        FaultPhase::Inside149,
        &format!("SELECT pg_advisory_xact_lock({key});"),
    ));
    let first_pool = pool.clone();
    let first_source = migrations.clone();
    let (send_first, receive_first) = tokio::sync::oneshot::channel();
    let first = tokio::spawn(async move {
        let mut connection = first_pool.acquire().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        send_first.send(pid).unwrap();
        academic149::run_transition(&mut connection, &first_source).await
    });
    let _first_abort = AbortOnDrop(first.abort_handle());
    let first_pid = receive_first.await.unwrap();
    tokio::time::timeout(Duration::from_secs(10),async {
        while !sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND NOT granted)").bind(first_pid).fetch_one(&mut observer).await.unwrap() { assert!(!first.is_finished());tokio::time::sleep(Duration::from_millis(10)).await; }
    }).await.unwrap();
    let second_pool = pool.clone();
    let second_source = migrations.clone();
    let (send_second, receive_second) = tokio::sync::oneshot::channel();
    let second = tokio::spawn(async move {
        let mut connection = second_pool.acquire().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        send_second.send(pid).unwrap();
        academic149::run_transition(&mut connection, &second_source).await
    });
    let _second_abort = AbortOnDrop(second.abort_handle());
    let second_pid = receive_second.await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !sqlx::query_scalar::<_, bool>("SELECT $1=ANY(pg_blocking_pids($2))")
            .bind(first_pid)
            .bind(second_pid)
            .fetch_one(&mut observer)
            .await
            .unwrap()
        {
            assert!(!second.is_finished());
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM pg_attribute WHERE attrelid=$1::bigint::oid AND NOT attisdropped AND starts_with(attname,'academic149_')").bind(before.relation).fetch_one(&mut observer).await.unwrap(),0);
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .execute(&mut observer)
        .await
        .unwrap();
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        149
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_context_receipt_recovery_audit"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(receipt)-'starts_on' ORDER BY idempotency_key) FROM learning_delivery_apply_runs receipt").fetch_one(&pool).await.unwrap(),before.facts["receipts"]);
    ready(&pool).await;
}

#[tokio::test]
async fn canonical_history_is_revalidated_after_waiting_for_the_owned_locks() {
    let pool = predecessor_with_connections("receipt149_history_drift", 147, 3).await;
    seed_legacy_curriculum_receipt(&pool, 1).await;
    let initial = original(&pool).await;
    let mut observer = fresh(&pool, &initial.schema).await;
    let mut gate = observer.begin().await.unwrap();
    sqlx::query("LOCK TABLE _sqlx_migrations IN SHARE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    let task_pool = pool.clone();
    let (send_pid, receive_pid) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut connection = task_pool.acquire().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        send_pid.send(pid).unwrap();
        let mut migrations = sqlx::migrate!("./migrations");
        migrations.locking = false;
        academic149::run_transition(&mut connection, &migrations).await
    });
    let _abort = AbortOnDrop(task.abort_handle());
    let pid = receive_pid.await.unwrap();
    tokio::time::timeout(Duration::from_secs(10),async {
        while !sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND relation='_sqlx_migrations'::regclass AND mode='AccessExclusiveLock' AND NOT granted)").bind(pid).fetch_one(&mut *gate).await.unwrap() {assert!(!task.is_finished());tokio::time::sleep(Duration::from_millis(10)).await;}
    }).await.unwrap();
    // A different owner advances through real native migrations while the waiter
    // has not acquired its lock. No SQLx history row is manually edited.
    let mut future = sqlx::migrate!("./migrations");
    let mut entries = future.iter().cloned().collect::<Vec<_>>();
    entries.push(sqlx::migrate::Migration::new(
        150,
        Cow::Borrowed("synthetic concurrent future"),
        sqlx::migrate::MigrationType::Simple,
        sqlx::SqlStr::from_static("SELECT 1;"),
        false,
    ));
    future.migrations = Cow::Owned(entries);
    future.locking = false;
    future.run_direct(None, &mut *gate, false).await.unwrap();
    gate.commit().await.unwrap();
    let error = task.await.unwrap().unwrap_err();
    assert!(
        matches!(error, sqlx::migrate::MigrateError::VersionMissing(150)),
        "post-lock history must refuse the future prefix before preparation: {error:?}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap(),
        150
    );
    assert_eq!(sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(receipt)-'starts_on' ORDER BY idempotency_key) FROM learning_delivery_apply_runs receipt").fetch_one(&pool).await.unwrap(),initial.facts["receipts"]);
}

#[tokio::test]
async fn postcommit_permission_failure_reports149_and_retries_without_receipt_reprojection() {
    let pool = predecessor("receipt149_permission_failure", 147).await;
    lower_match(&pool).await;
    let before = receipt_facts(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION fixture_reject_permission() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-permission-sentinel' USING ERRCODE='42501'; END $$; CREATE TRIGGER fixture_reject_permission BEFORE INSERT OR UPDATE ON permissions FOR EACH ROW EXECUTE FUNCTION fixture_reject_permission();").execute(&pool).await.unwrap();
    let failure = tokio::time::timeout(
        Duration::from_secs(60),
        school_migrations::run_tenant_migrations_detailed(&pool),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(
        failure.diagnostic.stage,
        school_migrations::MigrationStage::PermissionSync
    );
    assert_eq!(failure.diagnostic.sqlstate.as_deref(), Some("42501"));
    assert_eq!(failure.diagnostic.code, "unclassified");
    assert_eq!(
        failure.into_legacy_message(),
        "Permission sync failed after migrations: database"
    );
    assert_eq!(
        school_migrations::verify_tenant_schema(&pool)
            .await
            .unwrap(),
        149
    );
    ready(&pool).await;
    assert_eq!(sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(receipt)-'starts_on' ORDER BY idempotency_key) FROM learning_delivery_apply_runs receipt").fetch_one(&pool).await.unwrap(),before);
    let recovered = receipt_facts(&pool).await;
    let audit: Value =
        sqlx::query_scalar("SELECT evidence FROM academic_context_receipt_recovery_audit")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::raw_sql("DROP TRIGGER fixture_reject_permission ON permissions; DROP FUNCTION fixture_reject_permission();").execute(&pool).await.unwrap();
    canonical(&pool).await;
    assert_eq!(receipt_facts(&pool).await, recovered);
    assert_eq!(
        sqlx::query_scalar::<_, Value>(
            "SELECT evidence FROM academic_context_receipt_recovery_audit"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        audit
    );
}

#[tokio::test]
async fn postcommit_runtime_grant_failure_uses_owned_database_roles_and_retries_pool_one() {
    // A separate database gives the non-superuser test owner real authority over
    // public, matching tenant grants without changing another fixture's ownership.
    let admin =
        school_test_db::create_named_test_pool_with_max_connections("receipt149_grant_admin", 1)
            .await;
    let identity = Uuid::new_v4().simple().to_string();
    let owner = format!("so_{identity}_owner");
    let runtime = format!("so_{identity}_runtime");
    let database = format!("schoolorbit_grant_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE \"{owner}\" NOLOGIN"
    )))
    .execute(&admin)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{database}\" OWNER \"{owner}\""
    )))
    .execute(&admin)
    .await
    .unwrap();
    let options = admin.connect_options().as_ref().clone().database(&database);
    let setup_owner = owner.clone();
    let tenant = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let role = setup_owner.clone();
            Box::pin(async move {
                sqlx::query(sqlx::AssertSqlSafe(format!("SET ROLE \"{role}\"")))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .unwrap();
    let failure = tokio::time::timeout(
        Duration::from_secs(60),
        school_migrations::run_tenant_migrations_detailed(&tenant),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(
        failure.diagnostic.stage,
        school_migrations::MigrationStage::RuntimeGrants
    );
    assert_eq!(failure.diagnostic.sqlstate.as_deref(), Some("42704"));
    assert_eq!(failure.diagnostic.code, "unclassified");
    assert_eq!(
        failure.into_legacy_message(),
        "Runtime privilege reconciliation failed"
    );
    assert_eq!(
        school_migrations::verify_tenant_schema(&tenant)
            .await
            .unwrap(),
        149
    );
    ready(&tenant).await;
    let audit: Value =
        sqlx::query_scalar("SELECT evidence FROM academic_context_receipt_recovery_audit")
            .fetch_one(&tenant)
            .await
            .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE \"{runtime}\" NOLOGIN"
    )))
    .execute(&admin)
    .await
    .unwrap();
    canonical(&tenant).await;
    let privileges:bool=sqlx::query_scalar("SELECT has_schema_privilege($1,'public','USAGE') AND has_table_privilege($1,'learning_delivery_apply_runs','SELECT') AND has_table_privilege($1,'learning_delivery_apply_runs','INSERT') AND has_table_privilege($1,'learning_delivery_apply_runs','UPDATE') AND has_table_privilege($1,'learning_delivery_apply_runs','DELETE') AND NOT has_table_privilege($1,'_sqlx_migrations','INSERT,UPDATE,DELETE')").bind(&runtime).fetch_one(&tenant).await.unwrap();
    assert!(privileges);
    assert_eq!(
        sqlx::query_scalar::<_, Value>(
            "SELECT evidence FROM academic_context_receipt_recovery_audit"
        )
        .fetch_one(&tenant)
        .await
        .unwrap(),
        audit
    );
    tenant.close().await;
    // Drop only the owned test database and its two UUID-named roles. Its default
    // ACLs disappear with that database; no REASSIGN/DROP OWNED is used.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{database}\" WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE \"{runtime}\"")))
        .execute(&admin)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE \"{owner}\"")))
        .execute(&admin)
        .await
        .unwrap();
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
