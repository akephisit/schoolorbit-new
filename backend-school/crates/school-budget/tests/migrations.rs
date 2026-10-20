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
