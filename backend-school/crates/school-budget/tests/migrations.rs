use sqlx::PgPool;
use uuid::Uuid;
const HISTORY: &str = include_str!("../../../migrations/130_budget_history_ownership.sql");
async fn shaped(name: &str) -> PgPool {
    let p = school_test_db::create_named_test_pool(name).await;
    sqlx::raw_sql("CREATE TABLE budget_projects(id uuid PRIMARY KEY,attempt_id uuid NOT NULL);CREATE TABLE budget_plan_snapshots(id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES budget_projects(id));CREATE TABLE process_instances(id uuid PRIMARY KEY,source_module text NOT NULL,source_type text NOT NULL,source_id uuid NOT NULL);CREATE TABLE budget_adjustments(id uuid PRIMARY KEY,revision bigint NOT NULL,status text NOT NULL,document jsonb NOT NULL);CREATE FUNCTION preserve_budget_evidence() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'immutable';END$$;").execute(&p).await.unwrap();
    p
}
#[tokio::test]
async fn budget_history_migration_preserves_known_current_and_accepted_identity() {
    let p = shaped("budget_history_migration_valid").await;
    let project = Uuid::now_v7();
    let current = Uuid::now_v7();
    let accepted = Uuid::now_v7();
    let adjustment = Uuid::now_v7();
    sqlx::query("INSERT INTO budget_projects VALUES($1,$2)")
        .bind(project)
        .bind(current)
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO budget_plan_snapshots VALUES($1,$2)")
        .bind(accepted)
        .bind(project)
        .execute(&p)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO budget_adjustments VALUES($1,4,'returned','{\"amountSatang\":\"100\"}')",
    )
    .bind(adjustment)
    .execute(&p)
    .await
    .unwrap();
    sqlx::raw_sql(HISTORY).execute(&p).await.unwrap();
    let attempts: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM budget_project_attempts WHERE project_id=$1")
            .bind(project)
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(attempts.len(), 2);
    assert!(attempts.contains(&current) && attempts.contains(&accepted));
    let revisions: Vec<i64> = sqlx::query_scalar(
        "SELECT revision FROM budget_adjustment_revisions WHERE adjustment_id=$1",
    )
    .bind(adjustment)
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(
        revisions,
        vec![4],
        "do not fabricate unrecoverable historical drafts"
    );
    assert!(
        sqlx::query("UPDATE budget_project_attempts SET project_id=$2 WHERE id=$1")
            .bind(current)
            .bind(project)
            .execute(&p)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn budget_history_migration_refuses_unmappable_attempt_and_rolls_back_for_retry() {
    let p = shaped("budget_history_migration_refuse").await;
    let project = Uuid::now_v7();
    let current = Uuid::now_v7();
    let unknown = Uuid::now_v7();
    sqlx::query("INSERT INTO budget_projects VALUES($1,$2)")
        .bind(project)
        .bind(current)
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO process_instances VALUES($1,'budget','budget_project_plan',$2)")
        .bind(Uuid::now_v7())
        .bind(unknown)
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    let error = sqlx::raw_sql(HISTORY).execute(&mut *tx).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("BUDGET_ATTEMPT_HISTORY_UNMAPPABLE"));
    tx.rollback().await.unwrap();
    let exists: bool = sqlx::query_scalar(
        "SELECT to_regclass(current_schema()||'.budget_project_attempts') IS NOT NULL",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert!(!exists);
    sqlx::query("UPDATE budget_projects SET attempt_id=$2 WHERE id=$1")
        .bind(project)
        .bind(unknown)
        .execute(&p)
        .await
        .unwrap();
    sqlx::raw_sql(HISTORY).execute(&p).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT project_id FROM budget_project_attempts WHERE id=$1")
            .bind(unknown)
            .fetch_one(&p)
            .await
            .unwrap(),
        project
    );
}

#[tokio::test]
async fn annual_budget_migration162_preserves_documents_identities_and_unknown_financial_authority()
{
    let pool = school_test_db::create_named_test_pool("annual_budget_migration162").await;
    school_test_db::cutover_test_support::apply_migrations_through(&pool, 161)
        .await
        .unwrap();
    let actor = Uuid::now_v7();
    let year = Uuid::now_v7();
    let inactive = Uuid::now_v7();
    let fund = Uuid::now_v7();
    let category = Uuid::now_v7();
    let project = Uuid::now_v7();
    let attempt = Uuid::now_v7();
    let activity = Uuid::now_v7();
    let line = Uuid::now_v7();
    let unit: Uuid = sqlx::query_scalar("SELECT id FROM organization_units ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let document = serde_json::json!({"project":{"name":"Preserved","fiscalYearId":year,"organizationUnitId":unit,"startDate":"2026-01-01","endDate":"2026-12-31"},"activities":[{"id":activity,"title":"Unknown historical dates","defaultFundId":fund,"defaultExpenseCategoryId":category,"lines":[{"id":line,"fundId":fund,"expenseCategoryId":category,"totalSatang":"900"}]}]});
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Migration','Fixture','staff','active')").bind(actor).bind(actor.to_string()).execute(&mut *tx).await.unwrap();
    for (id, active) in [(year, true), (inactive, false)] {
        sqlx::query("INSERT INTO budget_fiscal_years(id,name,start_date,end_date,active) VALUES($1,'Legacy','2026-01-01','2026-12-31',$2)").bind(id).bind(active).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO budget_funds(id,name,active) VALUES($1,'Legacy fund',false)")
        .bind(fund)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO budget_expense_categories(id,name,active) VALUES($1,'Legacy category',true)",
    )
    .bind(category)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO budget_projects(id,owner_id,fiscal_year_id,organization_unit_id,revision,status,attempt_id,document) VALUES($1,$2,$3,$4,1,'draft',$5,$6)").bind(project).bind(actor).bind(year).bind(unit).bind(attempt).bind(&document).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO budget_project_attempts(id,project_id) VALUES($1,$2)")
        .bind(attempt)
        .bind(project)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO budget_project_revisions(project_id,revision,document) VALUES($1,1,$2)",
    )
    .bind(project)
    .bind(&document)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO budget_activities(id,project_id) VALUES($1,$2)")
        .bind(activity)
        .bind(project)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO budget_plan_lines(id,project_id,activity_id) VALUES($1,$2,$3)")
        .bind(line)
        .bind(project)
        .bind(activity)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO budget_plan_snapshots(id,project_id,version,document,accepted_by) VALUES($1,$2,1,$3,$4)").bind(attempt).bind(project).bind(&document).bind(actor).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE budget_projects SET status='approved',accepted_version=1,accepted_snapshot_id=$2 WHERE id=$1").bind(project).bind(attempt).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO budget_allocations(project_id,activity_id,fund_id,expense_category_id,planned) VALUES($1,$2,$3,$4,900)").bind(project).bind(activity).bind(fund).bind(category).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO budget_ledger(id,project_id,activity_id,fund_id,expense_category_id,planned_delta,source_kind,source_id,actor_id) VALUES($1,$2,$3,$4,$5,900,'plan',$6,$7)").bind(Uuid::now_v7()).bind(project).bind(activity).bind(fund).bind(category).bind(attempt).bind(actor).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('project',(SELECT to_jsonb(p) FROM budget_projects p WHERE id=$1),'revisions',(SELECT jsonb_agg(to_jsonb(r)) FROM budget_project_revisions r WHERE project_id=$1),'attempts',(SELECT jsonb_agg(to_jsonb(a)) FROM budget_project_attempts a WHERE project_id=$1),'snapshots',(SELECT jsonb_agg(to_jsonb(s)) FROM budget_plan_snapshots s WHERE project_id=$1),'allocations',(SELECT jsonb_agg(to_jsonb(a)) FROM budget_allocations a WHERE project_id=$1),'ledger',(SELECT jsonb_agg(to_jsonb(l)) FROM budget_ledger l WHERE project_id=$1))").bind(project).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE budget_projects SET document=jsonb_set(document,'{activities,0,lines,0,fundId}',to_jsonb($2::uuid)) WHERE id=$1").bind(project).bind(Uuid::now_v7()).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let error = sqlx::raw_sql(include_str!(
        "../../../migrations/162_annual_budget_control.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("BUDGET_YEAR_REFERENCE_UNMAPPABLE"));
    tx.rollback().await.unwrap();
    let exists: bool = sqlx::query_scalar(
        "SELECT to_regclass(current_schema()||'.budget_year_funds') IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!exists);
    sqlx::query("UPDATE budget_projects SET document=$2 WHERE id=$1")
        .bind(project)
        .bind(&document)
        .execute(&pool)
        .await
        .unwrap();
    school_test_db::cutover_test_support::apply_migrations_through(&pool, 162)
        .await
        .unwrap();
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('project',(SELECT to_jsonb(p) FROM budget_projects p WHERE id=$1),'revisions',(SELECT jsonb_agg(to_jsonb(r)) FROM budget_project_revisions r WHERE project_id=$1),'attempts',(SELECT jsonb_agg(to_jsonb(a)) FROM budget_project_attempts a WHERE project_id=$1),'snapshots',(SELECT jsonb_agg(to_jsonb(s)) FROM budget_plan_snapshots s WHERE project_id=$1),'allocations',(SELECT jsonb_agg(to_jsonb(a)) FROM budget_allocations a WHERE project_id=$1),'ledger',(SELECT jsonb_agg(to_jsonb(l)) FROM budget_ledger l WHERE project_id=$1))").bind(project).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let years: Vec<(bool, Option<chrono::DateTime<chrono::Utc>>, bool)> = sqlx::query_as(
        "SELECT is_closed,closed_at,funding_configured FROM budget_fiscal_years ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(years.len(), 2);
    assert!(years
        .iter()
        .all(|(closed, at, funding)| !*closed && at.is_none() && !*funding));
    let overlay:Vec<(Uuid,String,bool,Option<i64>,Option<i64>,Option<i64>,Option<i64>)>=sqlx::query_as("SELECT fiscal_year_id,name,active,carryover,annual_funding,reserve,received FROM budget_year_funds WHERE fund_id=$1 ORDER BY fiscal_year_id").bind(fund).fetch_all(&pool).await.unwrap();
    assert_eq!(overlay.len(), 2);
    assert!(overlay
        .iter()
        .all(|(_, name, active, c, a, r, received)| name == "Legacy fund"
            && !*active
            && c.is_none()
            && a.is_none()
            && r.is_none()
            && received.is_none()));
    school_test_db::run_test_migrations(&pool).await;
    school_test_db::run_test_migrations(&pool).await;
    assert_eq!(after,sqlx::query_scalar::<_,serde_json::Value>("SELECT jsonb_build_object('project',(SELECT to_jsonb(p) FROM budget_projects p WHERE id=$1),'revisions',(SELECT jsonb_agg(to_jsonb(r)) FROM budget_project_revisions r WHERE project_id=$1),'attempts',(SELECT jsonb_agg(to_jsonb(a)) FROM budget_project_attempts a WHERE project_id=$1),'snapshots',(SELECT jsonb_agg(to_jsonb(s)) FROM budget_plan_snapshots s WHERE project_id=$1),'allocations',(SELECT jsonb_agg(to_jsonb(a)) FROM budget_allocations a WHERE project_id=$1),'ledger',(SELECT jsonb_agg(to_jsonb(l)) FROM budget_ledger l WHERE project_id=$1))").bind(project).fetch_one(&pool).await.unwrap());
}
