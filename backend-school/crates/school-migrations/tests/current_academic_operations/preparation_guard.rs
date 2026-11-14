use super::*;
use school_migrations::{run_operational_tenant_migrations, CurrentPreparationExpectation};
use sqlx::{Connection, Executor};

async fn expected(pool: &PgPool) -> CurrentPreparationExpectation {
    let (date,hash):(chrono::NaiveDate,Vec<u8>)=sqlx::query_as("WITH source AS(SELECT id academic_term_id,academic_year_id,COALESCE(academic_delivery_applicable_publication(id,(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date),(SELECT (array_agg(v.id ORDER BY v.id))[1] FROM academic_delivery_versions v WHERE v.academic_term_id=t.id AND v.status IN('draft','published') HAVING count(*)=1)) delivery_id,COALESCE(academic_timetable_applicable_publication(id,(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date),(SELECT (array_agg(v.id ORDER BY v.id))[1] FROM academic_timetable_versions v WHERE v.academic_term_id=t.id AND v.status IN('draft','published') HAVING count(*)=1)) timetable_id FROM academic_terms t) SELECT (clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date,sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(source) ORDER BY academic_term_id)::text,'[]'),'UTF8')) FROM source").fetch_one(pool).await.unwrap();
    CurrentPreparationExpectation {
        cutover_date: date,
        selection_fingerprint: hash.try_into().unwrap(),
    }
}

#[tokio::test]
async fn preparation_required_date_and_selection_fail_atomically_then_exact_proof_retries() {
    let pool = predecessor("current_preparation_guard").await;
    let correct = expected(&pool).await;
    let before:Value=sqlx::query_scalar("SELECT jsonb_build_object('offerings',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offerings r),'versions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_delivery_versions r),'blocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetable_blocks r))").fetch_one(&pool).await.unwrap();
    let mut wrong_date = correct.clone();
    wrong_date.cutover_date -= chrono::Duration::days(1);
    let mut wrong_hash = correct.clone();
    wrong_hash.selection_fingerprint[0] ^= 1;
    for (proof, code) in [
        (None, "ACADEMIC_CURRENT_PREPARATION_REQUIRED"),
        (Some(wrong_date), "ACADEMIC_CURRENT_PREPARATION_MISMATCH"),
        (Some(wrong_hash), "ACADEMIC_CURRENT_PREPARATION_MISMATCH"),
    ] {
        let error = run_operational_tenant_migrations(&pool, proof.as_ref())
            .await
            .unwrap_err();
        assert_eq!(
            (
                error.diagnostic.version,
                error.diagnostic.sqlstate.as_deref(),
                error.diagnostic.code
            ),
            (Some(157), Some("23514"), code)
        );
        assert!(sqlx::query_scalar::<_,bool>("SELECT to_regclass('academic_timetables') IS NULL AND to_regclass('pg_temp.academic_current_preparation_expected') IS NULL").fetch_one(&pool).await.unwrap());
        let after:Value=sqlx::query_scalar("SELECT jsonb_build_object('offerings',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offerings r),'versions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_delivery_versions r),'blocks',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetable_blocks r))").fetch_one(&pool).await.unwrap();
        assert_eq!(before, after);
    }
    run_operational_tenant_migrations(&pool, Some(&correct))
        .await
        .unwrap();
    assert_eq!(
        school_migrations::verify_tenant_schema(&pool)
            .await
            .unwrap(),
        157
    );
    let receipt:Value=sqlx::query_scalar("SELECT to_jsonb(r) FROM academic_current_operations_migration_audit r WHERE migration_version=157").fetch_one(&pool).await.unwrap();
    run_operational_tenant_migrations(&pool, None)
        .await
        .unwrap();
    assert_eq!(receipt,sqlx::query_scalar::<_,Value>("SELECT to_jsonb(r) FROM academic_current_operations_migration_audit r WHERE migration_version=157").fetch_one(&pool).await.unwrap());
}

#[tokio::test]
async fn fresh_initialization_allows_empty_retry_and_refuses_populated_adoption() {
    let fresh = school_test_db::create_named_test_pool("current_preparation_fresh").await;
    apply_migrations_through(&fresh, 155).await.unwrap();
    school_migrations::run_fresh_tenant_migrations(&fresh)
        .await
        .unwrap();
    school_migrations::run_fresh_tenant_migrations(&fresh)
        .await
        .unwrap();
    let populated = predecessor("current_preparation_adoption").await;
    let before: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(r) ORDER BY version) FROM _sqlx_migrations r",
    )
    .fetch_one(&populated)
    .await
    .unwrap();
    assert!(school_migrations::run_fresh_tenant_migrations(&populated)
        .await
        .is_err());
    assert_eq!(
        before,
        sqlx::query_scalar::<_, Value>(
            "SELECT jsonb_agg(to_jsonb(r) ORDER BY version) FROM _sqlx_migrations r"
        )
        .fetch_one(&populated)
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn cancellation_closes_the_session_before_size_one_pool_can_reuse_it() {
    let pool = predecessor("current_preparation_cancel").await;
    let proof = expected(&pool).await;
    let (pid, schema): (i32, String) =
        sqlx::query_as("SELECT pg_backend_pid(),current_schema()::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut blocker = sqlx::PgConnection::connect_with(&pool.connect_options())
        .await
        .unwrap();
    blocker
        .execute(sqlx::AssertSqlSafe(format!(
            "SET search_path TO \"{schema}\",public"
        )))
        .await
        .unwrap();
    let mut tx = blocker.begin().await.unwrap();
    sqlx::query("LOCK TABLE learning_offerings IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await
        .unwrap();
    let run_pool = pool.clone();
    let operation =
        tokio::spawn(
            async move { run_operational_tenant_migrations(&run_pool, Some(&proof)).await },
        );
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event_type='Lock')").bind(pid).fetch_one(&mut *tx).await.unwrap(){break;}
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    operation.abort();
    let _ = operation.await;
    tx.rollback().await.unwrap();
    let (new_pid,clean):(i32,bool)=tokio::time::timeout(std::time::Duration::from_secs(10),sqlx::query_as("SELECT pg_backend_pid(),to_regclass('pg_temp.academic_current_preparation_expected') IS NULL").fetch_one(&pool)).await.unwrap().unwrap();
    assert_ne!(pid, new_pid);
    assert!(clean);
    run_operational_tenant_migrations(&pool, Some(&expected(&pool).await))
        .await
        .unwrap();
}

#[tokio::test]
async fn concurrent_operational_jobs_finish_with_one_real_owner_connection_and_one_gate() {
    let admin = school_test_db::create_named_test_pool("current_preparation_real_owner").await;
    let id = Uuid::new_v4();
    let db = format!("preparation_{}", id.simple());
    let owner = format!("so_{}_owner", id.simple());
    let runtime = format!("so_{}_runtime", id.simple());
    let password = Uuid::new_v4().simple().to_string();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE ROLE \"{owner}\" LOGIN PASSWORD '{password}' NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS; CREATE ROLE \"{runtime}\" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS;"))).execute(&admin).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{db}\" OWNER \"{owner}\""
    )))
    .execute(&admin)
    .await
    .unwrap();
    let options = admin
        .connect_options()
        .as_ref()
        .clone()
        .database(&db)
        .username(&owner)
        .password(&password);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 155).await.unwrap();
    let proof = expected(&pool).await;
    let (a, b) = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        tokio::join!(
            run_operational_tenant_migrations(&pool, Some(&proof)),
            run_operational_tenant_migrations(&pool, Some(&proof))
        )
    })
    .await
    .unwrap();
    a.unwrap();
    b.unwrap();
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM academic_current_operations_migration_audit WHERE migration_version=157").fetch_one(&pool).await.unwrap(),1);
    assert!(!sqlx::query_scalar::<_,bool>("SELECT has_table_privilege($1,'academic_result_removal_scope','SELECT,INSERT,UPDATE,DELETE')").bind(&runtime).fetch_one(&pool).await.unwrap());
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{db}\"")))
        .execute(&admin)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP ROLE \"{runtime}\"; DROP ROLE \"{owner}\";"
    )))
    .execute(&admin)
    .await
    .unwrap();
}
