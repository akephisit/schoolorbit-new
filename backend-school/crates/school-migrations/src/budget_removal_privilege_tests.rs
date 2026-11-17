use super::runtime_privilege_statement;
use sqlx::AssertSqlSafe;
use uuid::Uuid;

#[tokio::test]
async fn annual_budget_runtime_grants_cannot_forge_immutable_removal_scope() {
    let pool = school_test_db::create_named_test_pool("budget_runtime_scope_grants").await;
    school_test_db::run_test_migrations(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let role = format!("budget_scope_{}", Uuid::new_v4().simple());
    sqlx::query(AssertSqlSafe(format!("CREATE ROLE \"{role}\" NOLOGIN")))
        .execute(&mut *tx)
        .await
        .unwrap();
    let statement = runtime_privilege_statement(&role)
        .replace("SCHEMA public", &format!("SCHEMA \"{schema}\""));
    sqlx::raw_sql(AssertSqlSafe(statement.as_str()))
        .execute(&mut *tx)
        .await
        .unwrap();
    for table in [
        "budget_draft_removal_scope",
        "workflow_source_removal_scope",
        "workflow_evidence_removal_scope",
        "budget_erased_intents",
        "budget_year_template_ownership",
    ] {
        let allowed:bool=sqlx::query_scalar("SELECT has_table_privilege($1,$2,'INSERT') OR has_table_privilege($1,$2,'UPDATE') OR has_table_privilege($1,$2,'DELETE')").bind(&role).bind(format!("{schema}.{table}")).fetch_one(&mut *tx).await.unwrap();
        assert!(!allowed, "private {table}");
    }
    let guard_allowed: bool = sqlx::query_scalar("SELECT has_function_privilege($1,$2,'EXECUTE')")
        .bind(&role)
        .bind(format!("{schema}.budget_guard_draft_removal()"))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(!guard_allowed);
    let actor = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Privilege','Fixture','staff','active')").bind(actor).bind(actor.to_string()).execute(&mut *tx).await.unwrap();
    let unit: Uuid = sqlx::query_scalar("SELECT id FROM organization_units ORDER BY id LIMIT 1")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let year = Uuid::now_v7();
    let project = Uuid::now_v7();
    let attempt = Uuid::now_v7();
    sqlx::query("INSERT INTO budget_fiscal_years(id,name,start_date,end_date) VALUES($1,'Privilege fixture','2026-01-01','2026-12-31')").bind(year).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO budget_projects(id,owner_id,fiscal_year_id,organization_unit_id,revision,status,attempt_id,document) VALUES($1,$2,$3,$4,1,'draft',$5,'{}')").bind(project).bind(actor).bind(year).bind(unit).bind(attempt).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO budget_project_attempts(id,project_id) VALUES($1,$2)")
        .bind(attempt)
        .bind(project)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO budget_project_revisions(project_id,revision,document) VALUES($1,1,'{}')",
    )
    .bind(project)
    .execute(&mut *tx)
    .await
    .unwrap();
    // Even an owner-created temporary shadow containing a perfect-looking permit must be ignored.
    sqlx::raw_sql("CREATE TEMP TABLE budget_draft_removal_scope (LIKE budget_draft_removal_scope INCLUDING ALL)").execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO pg_temp.budget_draft_removal_scope SELECT pg_current_xact_id(),pg_backend_pid(),$1,'budget_project_revisions',encode(sha256(convert_to(to_jsonb(r)::text,'UTF8')),'hex') FROM budget_project_revisions r WHERE project_id=$1").bind(project).execute(&mut *tx).await.unwrap();
    sqlx::query(AssertSqlSafe(format!("SET LOCAL ROLE \"{role}\"")))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql("SET LOCAL schoolorbit.budget_draft_removal='true'; SET LOCAL schoolorbit.workflow_source_removal='true'").execute(&mut *tx).await.unwrap();
    for statement in [format!("INSERT INTO \"{schema}\".budget_draft_removal_scope SELECT * FROM pg_temp.budget_draft_removal_scope"),format!("DELETE FROM \"{schema}\".budget_project_revisions"),format!("UPDATE \"{schema}\".budget_project_revisions SET document='{{}}'")]{
        sqlx::raw_sql("SAVEPOINT refusal").execute(&mut *tx).await.unwrap();assert!(sqlx::raw_sql(AssertSqlSafe(statement.as_str())).execute(&mut *tx).await.is_err());sqlx::raw_sql("ROLLBACK TO SAVEPOINT refusal").execute(&mut *tx).await.unwrap();
    }
    sqlx::raw_sql("RESET ROLE").execute(&mut *tx).await.unwrap();
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM budget_project_revisions WHERE project_id=$1")
            .bind(project)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(rows, 1);
    tx.rollback().await.unwrap();
}
