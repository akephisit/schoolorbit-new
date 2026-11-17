//! Real migration/provider coverage for authorized never-approved source removal.
use school_notifications::{dispatch, publisher::NotificationType};
use school_workflow::{process_models::ProcessSource, process_purge};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

struct Fixture {
    pool: PgPool,
    actor: Uuid,
    unit: Uuid,
    year: Uuid,
}

async fn fixture(name: &str) -> Fixture {
    let pool = school_test_db::create_named_test_pool(name).await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Purge','Fixture','staff','active')")
        .bind(actor).bind(actor.to_string()).execute(&pool).await.unwrap();
    let unit = sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let year = Uuid::now_v7();
    sqlx::query("INSERT INTO budget_fiscal_years(id,name,start_date,end_date) VALUES($1,'Purge fixture','2026-01-01','2026-12-31')")
        .bind(year).execute(&pool).await.unwrap();
    Fixture { pool, actor, unit, year }
}

async fn project(f: &Fixture, status: &str) -> (Uuid, Vec<Uuid>) {
    let id = Uuid::now_v7();
    let attempts = vec![Uuid::now_v7(), Uuid::now_v7()];
    let document = json!({"project":{"name":"Purge fixture","fiscalYearId":f.year,
        "organizationUnitId":f.unit,"startDate":"2026-01-01","endDate":"2026-12-31"},"activities":[]});
    let mut tx = f.pool.begin().await.unwrap();
    sqlx::query("INSERT INTO budget_projects(id,owner_id,fiscal_year_id,organization_unit_id,revision,status,attempt_id,document) VALUES($1,$2,$3,$4,2,$5,$6,$7)")
        .bind(id).bind(f.actor).bind(f.year).bind(f.unit).bind(status).bind(attempts[1]).bind(&document)
        .execute(&mut *tx).await.unwrap();
    for attempt in &attempts {
        sqlx::query("INSERT INTO budget_project_attempts(id,project_id) VALUES($1,$2)")
            .bind(attempt).bind(id).execute(&mut *tx).await.unwrap();
    }
    for revision in [1_i64, 2] {
        sqlx::query("INSERT INTO budget_project_revisions(project_id,revision,document) VALUES($1,$2,$3)")
            .bind(id).bind(revision).bind(&document).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO budget_history(id,resource_kind,resource_id,revision,actor_id,document) VALUES($1,'project',$2,2,$3,'{}')")
        .bind(Uuid::now_v7()).bind(id).bind(f.actor).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO budget_receipts(actor_id,request_id,operation,payload_hash,resource_id) VALUES($1,$2,'project_create','synthetic',$3)")
        .bind(f.actor).bind(Uuid::now_v7()).bind(id).execute(&mut *tx).await.unwrap();
    for attempt in &attempts {
        workflow(&mut tx, f.actor, *attempt).await;
    }
    tx.commit().await.unwrap();
    (id, attempts)
}

async fn workflow(tx: &mut Transaction<'_, Postgres>, actor: Uuid, attempt: Uuid) {
    let instance = Uuid::now_v7();
    sqlx::query("INSERT INTO process_instances(id,source_module,source_type,source_id,revision,definition,created_by) VALUES($1,'budget','budget_project_plan',$2,2,'{\"status\":\"rejected\"}',$3)")
        .bind(instance).bind(attempt).bind(actor).execute(&mut **tx).await.unwrap();
    sqlx::query("INSERT INTO process_participants(instance_id,user_id) VALUES($1,$2)")
        .bind(instance).bind(actor).execute(&mut **tx).await.unwrap();
    for revision in [1_i64, 2] {
        sqlx::query("INSERT INTO process_instance_revisions(instance_id,revision,definition) VALUES($1,$2,'{}')")
            .bind(instance).bind(revision).execute(&mut **tx).await.unwrap();
    }
    sqlx::query("INSERT INTO process_action_events(id,instance_id,actor_id,request_id,revision,event) VALUES($1,$2,$3,$4,2,'{}')")
        .bind(Uuid::now_v7()).bind(instance).bind(actor).bind(Uuid::now_v7()).execute(&mut **tx).await.unwrap();
    sqlx::query("INSERT INTO process_action_history(id,instance_id,actor_id,entry) VALUES($1,$2,$3,'{}')")
        .bind(Uuid::now_v7()).bind(instance).bind(actor).execute(&mut **tx).await.unwrap();
    for (operation, resource) in [("process_start",attempt),("process_action",instance)] {
        sqlx::query("INSERT INTO workflow_mutation_receipts(actor_id,request_id,operation,payload_hash,resource_id) VALUES($1,$2,$3,'synthetic',$4)")
            .bind(actor).bind(Uuid::now_v7()).bind(operation).bind(resource).execute(&mut **tx).await.unwrap();
    }
    // Two rounds prove old work is removed even when absent from current JSON steps.
    for _ in 0..2 {
        let window = Uuid::now_v7();
        let item = Uuid::now_v7();
        let source = Uuid::now_v7();
        sqlx::query("INSERT INTO workflow_windows(id,module_code,workflow_code,title,managed_by_permission,status,created_by) VALUES($1,'budget','process_step','Purge fixture','process.manage.school','open',$2)")
            .bind(window).bind(actor).execute(&mut **tx).await.unwrap();
        sqlx::query("INSERT INTO work_items(id,workflow_window_id,module_code,source_resource_type,source_resource_id,title,action_path,created_by) VALUES($1,$2,'budget','budget_project_plan',$3,'Purge fixture','/staff/budget',$4)")
            .bind(item).bind(window).bind(attempt).bind(actor).execute(&mut **tx).await.unwrap();
        sqlx::query("INSERT INTO work_item_assignees(id,work_item_id,assignee_type,user_id) VALUES($1,$2,'user',$3)")
            .bind(Uuid::now_v7()).bind(item).bind(actor).execute(&mut **tx).await.unwrap();
        sqlx::query("INSERT INTO process_notification_sources(id,instance_id,recipient_id,work_item_id) VALUES($1,$2,$3,$4)")
            .bind(source).bind(instance).bind(actor).bind(item).execute(&mut **tx).await.unwrap();
        dispatch::create_in_transaction(tx,&[dispatch::NewNotification {
            id: Uuid::now_v7(),notification_id: Uuid::now_v7(),recipient_id: actor,
            source: dispatch::NotificationSource::Process { source_id: source },
            idempotency_key: format!("purge:{source}"),title:"Purge fixture".into(),message:"Review".into(),
            type_:NotificationType::Info,presentation:dispatch::Presentation::AlertOnce,link:Some("/staff/budget".into()),
        }]).await.unwrap();
    }
}

async fn purge(f: &Fixture, id: Uuid) {
    let mut tx = f.pool.begin().await.unwrap();
    let attempts: Vec<Uuid> = sqlx::query_scalar("SELECT budget_prepare_draft_removal($1,2)")
        .bind(id).fetch_all(&mut *tx).await.unwrap();
    for attempt in attempts {
        process_purge::purge_source_in_transaction(&mut tx,&ProcessSource {
            module_code:"budget".into(),resource_type:"budget_project_plan".into(),resource_id:attempt,
        }).await.unwrap();
    }
    sqlx::query("SELECT budget_finalize_draft_removal($1)").bind(id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn never_approved_purge_removes_all_attempts_work_notifications_and_evidence_only_for_owned_project() {
    let f = fixture("budget_purge_all_attempts").await;
    let (retained, _) = project(&f,"submitted").await;
    let retained_before: serde_json::Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM budget_projects p WHERE id=$1")
        .bind(retained).fetch_one(&f.pool).await.unwrap();
    for status in ["draft","submitted","returned","rejected"] {
        let (id, attempts) = project(&f,status).await;
        purge(&f,id).await;
        let remaining: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM budget_projects WHERE id=$1)+(SELECT count(*) FROM budget_project_attempts WHERE project_id=$1)+(SELECT count(*) FROM budget_project_revisions WHERE project_id=$1)+(SELECT count(*) FROM budget_history WHERE resource_id=$1)+(SELECT count(*) FROM budget_receipts WHERE resource_id=$1)+(SELECT count(*) FROM process_instances WHERE source_module='budget' AND source_id=ANY($2))+(SELECT count(*) FROM work_items WHERE module_code='budget' AND source_resource_id=ANY($2))+(SELECT count(*) FROM workflow_mutation_receipts WHERE resource_id=ANY($2))")
            .bind(id).bind(attempts).fetch_one(&f.pool).await.unwrap();
        assert_eq!(remaining,0,"purge status {status}");
    }
    let retained_after: serde_json::Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM budget_projects p WHERE id=$1")
        .bind(retained).fetch_one(&f.pool).await.unwrap();
    assert_eq!(retained_before,retained_after);
    for (table, expected) in [("process_instances",2_i64),("process_instance_revisions",4),("process_action_events",2),("process_action_history",2),("process_participants",2),("workflow_windows",4),("work_items",4),("work_item_assignees",4),("process_notification_sources",4),("notification_receipts",4),("notifications",4),("notification_presentations",4),("workflow_mutation_receipts",4),("budget_draft_removal_scope",0),("workflow_source_removal_scope",0),("workflow_evidence_removal_scope",0)] {
        let count:i64=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}"))).fetch_one(&f.pool).await.unwrap();
        assert_eq!(count,expected,"remaining {table}");
    }
}

#[tokio::test]
async fn source_purge_requires_sealed_authority_and_accepted_or_financial_projects_cannot_acquire_it() {
    let f = fixture("budget_purge_guards").await;
    let (id, attempts) = project(&f,"draft").await;
    let mut tx = f.pool.begin().await.unwrap();
    let error=process_purge::purge_source_in_transaction(&mut tx,&ProcessSource {module_code:"budget".into(),resource_type:"budget_project_plan".into(),resource_id:attempts[0]}).await.unwrap_err();
    assert!(error.to_string().contains("WORKFLOW_SOURCE_REMOVAL_NOT_AUTHORIZED"));
    tx.rollback().await.unwrap();
    sqlx::query("INSERT INTO budget_plan_snapshots(id,project_id,version,document,accepted_by) VALUES($1,$2,1,'{}',$3)")
        .bind(attempts[0]).bind(id).bind(f.actor).execute(&f.pool).await.unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM process_instances r").fetch_one(&f.pool).await.unwrap();
    let error=sqlx::query("SELECT budget_prepare_draft_removal($1,2)").bind(id).execute(&f.pool).await.unwrap_err();
    assert_eq!(error.as_database_error().unwrap().message(),"BUDGET_PROJECT_NOT_DELETABLE");
    assert_eq!(before,sqlx::query_scalar::<_,serde_json::Value>("SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM process_instances r").fetch_one(&f.pool).await.unwrap());
    for statement in ["DELETE FROM budget_plan_snapshots WHERE project_id=$1","DELETE FROM budget_project_revisions WHERE project_id=$1","DELETE FROM budget_project_attempts WHERE project_id=$1","DELETE FROM budget_projects WHERE id=$1"] {
        assert!(sqlx::query(statement).bind(id).execute(&f.pool).await.is_err());
    }
}

#[tokio::test]
async fn annual_removal_preserves_retained_cross_year_history_and_refuses_cash_receipts() {
    let f=fixture("budget_purge_year_history").await;
    let (id,_)=project(&f,"returned").await;
    let other_year=Uuid::now_v7();
    sqlx::query("INSERT INTO budget_fiscal_years(id,name,start_date,end_date) VALUES($1,'Other year','2026-01-01','2026-12-31')").bind(other_year).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE budget_projects SET fiscal_year_id=$2 WHERE id=$1").bind(id).bind(other_year).execute(&f.pool).await.unwrap();
    let error=sqlx::query("SELECT budget_finalize_draft_year_removal($1)").bind(f.year).execute(&f.pool).await.unwrap_err();
    assert_eq!(error.as_database_error().unwrap().message(),"BUDGET_YEAR_NOT_DELETABLE");
    purge(&f,id).await;
    let fund=Uuid::now_v7();
    sqlx::query("INSERT INTO budget_funds(id) VALUES($1)").bind(fund).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO budget_year_funds(fiscal_year_id,fund_id,name,active,received) VALUES($1,$2,'Cash evidence',true,1)").bind(f.year).bind(fund).execute(&f.pool).await.unwrap();
    assert!(sqlx::query("SELECT budget_finalize_draft_year_removal($1)").bind(f.year).execute(&f.pool).await.is_err());
    sqlx::query("UPDATE budget_year_funds SET received=NULL WHERE fiscal_year_id=$1").bind(f.year).execute(&f.pool).await.unwrap();
    sqlx::query("SELECT budget_finalize_draft_year_removal($1)").bind(f.year).execute(&f.pool).await.unwrap();
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM budget_funds WHERE id=$1)").bind(fund).fetch_one(&f.pool).await.unwrap());
}
