use chrono::NaiveDate;
use school_authorization::ActorContext;
use school_budget::{
    adjustments, files,
    models::*,
    ports::{BudgetCatalogPort, BudgetCatalogPriceSnapshot},
    projects, queries, requests, settings,
};
use school_errors::AppError;
use school_file_platform::repository::FileRepository;
use school_permissions::registry::codes;
use school_workflow::process_models::*;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::atomic::{AtomicI64, Ordering};
use uuid::Uuid;
struct Catalog {
    item: Uuid,
    version: Uuid,
    price: AtomicI64,
}
#[async_trait::async_trait]
impl BudgetCatalogPort for Catalog {
    async fn snapshot(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        item: Uuid,
        version: Uuid,
    ) -> Result<BudgetCatalogPriceSnapshot, AppError> {
        actor.require_permission(codes::SUPPLY_CATALOG_READ_SCHOOL)?;
        if item != self.item || version != self.version {
            return Err(AppError::NotFound("catalog".into()));
        }
        Ok(BudgetCatalogPriceSnapshot {
            item_id: item,
            version_id: version,
            revision: 1,
            name: "กระดาษ".into(),
            unit: "ห่อ".into(),
            price_satang: self.price.load(Ordering::SeqCst),
        })
    }
}
fn date(month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, month, day).unwrap()
}
fn step(user: Uuid, kind: ProcessStepKind) -> ProcessTemplateStep {
    ProcessTemplateStep {
        title: if kind == ProcessStepKind::Review {
            "ตรวจสอบ"
        } else {
            "อนุมัติ"
        }
        .into(),
        kind,
        completion_policy: ProcessCompletionPolicy::Any,
        recipients: vec![ProcessRecipient {
            kind: ProcessRecipientKind::User,
            user_id: Some(user),
            organization_unit_id: None,
            position_code: None,
        }],
        due_after_days: None,
        requires_signature: false,
        requires_comment: false,
        requires_evidence: false,
        requires_delegated_authority: false,
    }
}
struct Fixture {
    pool: PgPool,
    owner: ActorContext,
    reviewer: ActorContext,
    other: ActorContext,
    settings: BudgetSettings,
    unit: Uuid,
    catalog: Catalog,
}
async fn fixture(name: &str) -> Fixture {
    fixture_with_connections(name, 10).await
}
async fn fixture_with_connections(name: &str, max: u32) -> Fixture {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, max).await;
    school_test_db::run_test_migrations(&pool).await;
    let mut actors = vec![];
    for label in ["Budget Owner", "Budget Reviewer", "Denied"] {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic',$3,'Fixture','staff','active')").bind(id).bind(id.to_string()).bind(label).execute(&pool).await.unwrap();
        actors.push(ActorContext {
            user_id: id,
            permissions: vec![codes::WILDCARD.into()],
        });
    }
    let owner = actors.remove(0);
    let reviewer = actors.remove(0);
    for actor in [&owner, &reviewer] {
        assert_eq!(sqlx::query("INSERT INTO user_roles(id,user_id,role_id,started_at) SELECT $1,$2,id,'2020-01-01' FROM roles WHERE code='ADMIN'").bind(Uuid::now_v7()).bind(actor.user_id).execute(&pool).await.unwrap().rows_affected(),1);
    }
    let mut other = actors.remove(0);
    other.permissions.clear();
    let unit: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE code='BUD-PLA'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let route = vec![
        step(reviewer.user_id, ProcessStepKind::Review),
        step(reviewer.user_id, ProcessStepKind::Approve),
    ];
    let settings = settings::save(
        &pool,
        &reviewer,
        SaveBudgetSettingsRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            fiscal_years: vec![BudgetFiscalYearInput {
                id: None,
                name: "2570".into(),
                start_date: date(10, 1),
                end_date: NaiveDate::from_ymd_opt(2027, 9, 30).unwrap(),
                active: true,
            }],
            funds: vec![
                BudgetCatalogItemInput {
                    id: None,
                    name: "เงินอุดหนุน".into(),
                    active: true,
                },
                BudgetCatalogItemInput {
                    id: None,
                    name: "พัฒนาผู้เรียน".into(),
                    active: true,
                },
            ],
            expense_categories: vec![
                BudgetCatalogItemInput {
                    id: None,
                    name: "ค่าวัสดุ".into(),
                    active: true,
                },
                BudgetCatalogItemInput {
                    id: None,
                    name: "ค่าครุภัณฑ์".into(),
                    active: true,
                },
            ],
            plan_steps: route.clone(),
            request_steps: route.clone(),
            adjustment_steps: route,
        },
    )
    .await
    .unwrap();
    let catalog = Catalog {
        item: Uuid::now_v7(),
        version: Uuid::now_v7(),
        price: AtomicI64::new(179400),
    };
    Fixture {
        pool,
        owner,
        reviewer,
        other,
        settings,
        unit,
        catalog,
    }
}
fn line(f: &Fixture, amount: &str, fund: usize, cat: usize) -> BudgetPlanLineInput {
    BudgetPlanLineInput {
        id: None,
        description: "รายการ".into(),
        quantity: "1".into(),
        unit: "ชุด".into(),
        unit_price_satang: amount.into(),
        fund_id: f.settings.funds[fund].id,
        expense_category_id: f.settings.expense_categories[cat].id,
        supply_item_id: None,
        supply_version_id: None,
    }
}
fn activity(title: &str, line: BudgetPlanLineInput) -> BudgetActivityInput {
    BudgetActivityInput {
        id: None,
        title: title.into(),
        default_fund_id: Some(line.fund_id),
        default_expense_category_id: Some(line.expense_category_id),
        lines: vec![line],
    }
}
fn save_input(w: &BudgetProjectWorkspace) -> SaveBudgetProjectRequest {
    SaveBudgetProjectRequest {
        request_id: Uuid::now_v7(),
        expected_revision: w.project.revision,
        name: w.project.name.clone(),
        fiscal_year_id: w.project.fiscal_year_id,
        organization_unit_id: w.project.organization_unit_id,
        start_date: w.project.start_date,
        end_date: w.project.end_date,
        activities: w
            .activities
            .iter()
            .map(|a| BudgetActivityInput {
                id: Some(a.id),
                title: a.title.clone(),
                default_fund_id: a.default_fund_id,
                default_expense_category_id: a.default_expense_category_id,
                lines: a
                    .lines
                    .iter()
                    .map(|l| BudgetPlanLineInput {
                        id: Some(l.id),
                        description: l.description.clone(),
                        quantity: l.quantity.clone(),
                        unit: l.unit.clone(),
                        unit_price_satang: l.unit_price_satang.clone(),
                        fund_id: l.fund_id,
                        expense_category_id: l.expense_category_id,
                        supply_item_id: l.supply_item_id,
                        supply_version_id: l.supply_version_id,
                    })
                    .collect(),
            })
            .collect(),
    }
}
async fn draft(f: &Fixture, activities: Vec<BudgetActivityInput>) -> BudgetProjectWorkspace {
    let w = projects::create(
        &f.pool,
        &f.owner,
        CreateBudgetProjectRequest {
            request_id: Uuid::now_v7(),
            name: "โครงการคณิตศาสตร์".into(),
            fiscal_year_id: f.settings.fiscal_years[0].id,
            organization_unit_id: f.unit,
            start_date: date(11, 1),
            end_date: date(12, 30),
        },
    )
    .await
    .unwrap();
    let mut input = save_input(&w);
    input.activities = activities;
    projects::save(&f.pool, &f.owner, w.project.id, input, &f.catalog)
        .await
        .unwrap()
}
async fn action(
    f: &Fixture,
    kind: BudgetResourceKind,
    id: Uuid,
    revision: i64,
    action: ProcessAction,
) -> BudgetActionRequest {
    let p = queries::resource_process(&f.pool, &f.reviewer, kind, id)
        .await
        .unwrap()
        .process
        .unwrap();
    BudgetActionRequest {
        request_id: Uuid::now_v7(),
        expected_revision: revision,
        expected_process_revision: p.revision,
        action,
        comment: Some("ตรวจสอบแล้ว".into()),
    }
}
async fn approve_plan(f: &Fixture, w: BudgetProjectWorkspace) -> BudgetProjectWorkspace {
    let mut w = projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    for act in [ProcessAction::Review, ProcessAction::Approve] {
        let i = action(
            f,
            BudgetResourceKind::Project,
            w.project.id,
            w.project.revision,
            act,
        )
        .await;
        w = projects::act(&f.pool, &f.reviewer, w.project.id, i)
            .await
            .unwrap();
    }
    assert_eq!(w.project.status, BudgetStatus::Approved);
    assert_review_history(f, BudgetResourceKind::Project, w.project.id).await;
    w
}
async fn assert_review_history(f: &Fixture, kind: BudgetResourceKind, id: Uuid) {
    let history = queries::history(&f.pool, &f.reviewer, kind, id, BudgetListQuery::default())
        .await
        .unwrap();
    let actions = history
        .items
        .iter()
        .take(3)
        .map(|entry| entry.action.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actions, ["approved", "reviewed", "submitted"]);
}
async fn request(
    f: &Fixture,
    w: &BudgetProjectWorkspace,
    activity: usize,
    amount: &str,
) -> BudgetSpendingRequestDetail {
    let a = &w.activities[activity];
    requests::create(
        &f.pool,
        &f.owner,
        w.project.id,
        SaveBudgetSpendingRequestRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            activity_id: a.id,
            title: "ขอซื้อวัสดุ".into(),
            lines: vec![BudgetRequestLineInput {
                plan_line_id: a.lines[0].id,
                amount_satang: amount.into(),
            }],
            evidence_file_ids: vec![],
        },
    )
    .await
    .unwrap()
}
async fn submit_request(
    f: &Fixture,
    r: BudgetSpendingRequestDetail,
) -> BudgetSpendingRequestDetail {
    requests::submit(
        &f.pool,
        &f.owner,
        r.request.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: r.request.revision,
        },
    )
    .await
    .unwrap()
}
async fn approve_request(
    f: &Fixture,
    r: BudgetSpendingRequestDetail,
) -> BudgetSpendingRequestDetail {
    let mut r = submit_request(f, r).await;
    for act in [ProcessAction::Review, ProcessAction::Approve] {
        let i = action(
            f,
            BudgetResourceKind::Request,
            r.request.id,
            r.request.revision,
            act,
        )
        .await;
        r = requests::act(&f.pool, &f.reviewer, r.request.id, i)
            .await
            .unwrap();
    }
    assert_review_history(f, BudgetResourceKind::Request, r.request.id).await;
    r
}
async fn evidence(f: &Fixture, request: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    let version = Uuid::now_v7();
    sqlx::query("INSERT INTO files(id,owner_user_id,created_by,display_filename,purpose_code,visibility,lifecycle_status,retention_class,expires_at) VALUES($1,$2,$2,'fixture.png','budget_evidence','private','ready','temporary',now()+interval '1 hour')").bind(id).bind(f.reviewer.user_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,1,'r2','private','stored',$3,'image/png','png',100,repeat('a',64),'clean')").bind(version).bind(id).bind(format!("tenants/{}/budget/evidence/{id}/v1/original.png",f.owner.user_id)).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(id)
        .bind(version)
        .execute(&f.pool)
        .await
        .unwrap();
    files::record_upload(&f.pool, &f.reviewer, request, id)
        .await
        .unwrap();
    id
}
async fn reconciled(f: &Fixture) {
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM budget_allocations a WHERE (planned,adjusted,reserved,paid) IS DISTINCT FROM ((SELECT coalesce(sum(planned_delta),0) FROM budget_ledger l WHERE (l.project_id,l.activity_id,l.fund_id,l.expense_category_id)=(a.project_id,a.activity_id,a.fund_id,a.expense_category_id)),(SELECT coalesce(sum(adjusted_delta),0) FROM budget_ledger l WHERE (l.project_id,l.activity_id,l.fund_id,l.expense_category_id)=(a.project_id,a.activity_id,a.fund_id,a.expense_category_id)),(SELECT coalesce(sum(reserved_delta),0) FROM budget_ledger l WHERE (l.project_id,l.activity_id,l.fund_id,l.expense_category_id)=(a.project_id,a.activity_id,a.fund_id,a.expense_category_id)),(SELECT coalesce(sum(paid_delta),0) FROM budget_ledger l WHERE (l.project_id,l.activity_id,l.fund_id,l.expense_category_id)=(a.project_id,a.activity_id,a.fund_id,a.expense_category_id)))").fetch_one(&f.pool).await.unwrap();
    assert_eq!(n, 0);
}
#[tokio::test]
async fn budget_plan_exact_example_snapshot_catalog_and_scope() {
    let f = fixture("budget_plan_example").await;
    let w = draft(
        &f,
        vec![
            activity("วัสดุ", line(&f, "358800", 0, 0)),
            activity("อุปกรณ์", line(&f, "304000", 0, 1)),
            activity("พัฒนาผู้เรียน", line(&f, "330000", 1, 0)),
        ],
    )
    .await;
    assert_eq!(w.project.total_satang, "992800");
    assert!(queries::workspace(&f.pool, &f.other, w.project.id)
        .await
        .is_err());
    let w = approve_plan(&f, w).await;
    let dashboard = queries::dashboard(
        &f.pool,
        &f.owner,
        BudgetReportQuery {
            fiscal_year_id: Some(f.settings.fiscal_years[0].id),
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();
    assert_eq!(dashboard.totals.planned_satang, "992800");
    assert_eq!(dashboard.rows.len(), 3);
    let csv = queries::export_csv(&f.pool, &f.owner, BudgetReportQuery::default())
        .await
        .unwrap();
    assert!(csv.contains("9928.00"));
    assert!(csv.contains("เงินอุดหนุน /"));
    let snap = w.accepted_plan.clone().unwrap();
    assert!(
        sqlx::query("UPDATE budget_plan_snapshots SET document='{}' WHERE id=$1")
            .bind(snap.id)
            .execute(&f.pool)
            .await
            .is_err()
    );
    let w = projects::amend(
        &f.pool,
        &f.owner,
        w.project.id,
        StartBudgetAmendmentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            reason: "ปรับชื่อ".into(),
        },
    )
    .await
    .unwrap();
    let mut changed = save_input(&w);
    changed.fiscal_year_id = Uuid::now_v7();
    assert!(
        projects::save(&f.pool, &f.owner, w.project.id, changed, &f.catalog)
            .await
            .is_err()
    );
    let mut changed = save_input(&w);
    changed.name = "ชื่อร่างใหม่".into();
    let w = projects::save(&f.pool, &f.owner, w.project.id, changed, &f.catalog)
        .await
        .unwrap();
    assert_eq!(w.accepted_plan.unwrap().project.name, snap.project.name);
    let rows = queries::dashboard(&f.pool, &f.owner, BudgetReportQuery::default(), false)
        .await
        .unwrap()
        .rows;
    assert!(rows.iter().all(|r| r.project_name == snap.project.name));
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_request_reservation_payment_refund_close_replay_and_private_files() {
    let f = fixture("budget_payment_lifecycle").await;
    let w = approve_plan(
        &f,
        draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let r = approve_request(&f, request(&f, &w, 0, "8000").await).await;
    assert_eq!(r.request.reserved_satang, "8000");
    let file = evidence(&f, r.request.id).await;
    let pay = CreateBudgetPaymentRequest {
        request_id: Uuid::now_v7(),
        expected_revision: r.request.revision,
        paid_on: date(11, 2),
        voucher_number: "PV-1".into(),
        note: None,
        evidence_file_ids: vec![file],
        allocations: vec![BudgetPaymentAllocationInput {
            plan_line_id: r.lines[0].plan_line_id,
            amount_satang: "3000".into(),
        }],
    };
    let r = requests::pay(&f.pool, &f.reviewer, r.request.id, pay.clone())
        .await
        .unwrap();
    assert_eq!(r.request.paid_satang, "3000");
    assert_eq!(r.request.reserved_satang, "5000");
    let replay = requests::pay(&f.pool, &f.reviewer, r.request.id, pay.clone())
        .await
        .unwrap();
    assert_eq!(r.request.revision, replay.request.revision);
    let mut conflict = pay;
    conflict.voucher_number = "changed".into();
    assert!(requests::pay(&f.pool, &f.reviewer, r.request.id, conflict)
        .await
        .is_err());
    let pages = queries::payments(&f.pool, &f.owner, r.request.id, BudgetListQuery::default())
        .await
        .unwrap();
    assert_eq!(pages.total, 1);
    let p = &pages.items[0];
    assert!(sqlx::query("UPDATE files SET deleted_at=now() WHERE id=$1")
        .bind(file)
        .execute(&f.pool)
        .await
        .is_err());
    let platform = school_file_platform::repository::SqlFileRepository::new(f.pool.clone());
    let file_row = platform.load_delivery(file).await;
    assert!(file_row.is_ok());
    let file_row = file_row.unwrap().unwrap().file;
    assert!(
        files::delete_guard(&f.pool, &f.reviewer, &file_row, Some(r.request.id))
            .await
            .is_err()
    );
    assert!(files::authorize_existing(
        &f.pool,
        &f.other,
        &file_row,
        school_file_platform::access::FilePolicyAction::Read,
        Some(r.request.id)
    )
    .await
    .is_err());
    let refundfile = evidence(&f, r.request.id).await;
    let r = requests::refund(
        &f.pool,
        &f.reviewer,
        r.request.id,
        p.id,
        RefundBudgetPaymentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: r.request.revision,
            refunded_on: date(11, 3),
            reason: "คืนบางส่วน".into(),
            evidence_file_ids: vec![refundfile],
            allocations: vec![BudgetPaymentAllocationInput {
                plan_line_id: r.lines[0].plan_line_id,
                amount_satang: "1000".into(),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(r.request.paid_satang, "2000");
    assert_eq!(r.request.reserved_satang, "6000");
    let close = CloseBudgetRequestRequest {
        request_id: Uuid::now_v7(),
        expected_revision: r.request.revision,
        reason: "จบคำขอ".into(),
    };
    let r = requests::close(&f.pool, &f.reviewer, r.request.id, close.clone())
        .await
        .unwrap();
    assert_eq!(r.request.reserved_satang, "0");
    assert_eq!(
        requests::close(&f.pool, &f.reviewer, r.request.id, close)
            .await
            .unwrap()
            .request
            .revision,
        r.request.revision
    );
    let file = evidence(&f, r.request.id).await;
    let r = requests::refund(
        &f.pool,
        &f.reviewer,
        r.request.id,
        p.id,
        RefundBudgetPaymentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: r.request.revision,
            refunded_on: date(11, 4),
            reason: "คืนหลังปิด".into(),
            evidence_file_ids: vec![file],
            allocations: vec![BudgetPaymentAllocationInput {
                plan_line_id: r.lines[0].plan_line_id,
                amount_satang: "2000".into(),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(r.request.paid_satang, "0");
    assert_eq!(r.request.reserved_satang, "0");
    let w = queries::workspace(&f.pool, &f.owner, w.project.id)
        .await
        .unwrap();
    assert_eq!(w.project.available_satang, "10000");
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_concurrent_approvals_allocation_floor_and_atomic_rollback() {
    let f = fixture("budget_concurrent_requests").await;
    let mut a = activity("รายการร่วม", line(&f, "10000", 0, 0));
    a.lines.push(line(&f, "10000", 0, 0));
    let w = approve_plan(&f, draft(&f, vec![a]).await).await;
    let first = request(&f, &w, 0, "15000").await;
    let second = request(&f, &w, 0, "15000").await;
    let mut first = submit_request(&f, first).await;
    let mut second = submit_request(&f, second).await;
    for r in [&mut first, &mut second] {
        let i = action(
            &f,
            BudgetResourceKind::Request,
            r.request.id,
            r.request.revision,
            ProcessAction::Review,
        )
        .await;
        *r = requests::act(&f.pool, &f.reviewer, r.request.id, i)
            .await
            .unwrap();
    }
    let a = action(
        &f,
        BudgetResourceKind::Request,
        first.request.id,
        first.request.revision,
        ProcessAction::Approve,
    )
    .await;
    let b = action(
        &f,
        BudgetResourceKind::Request,
        second.request.id,
        second.request.revision,
        ProcessAction::Approve,
    )
    .await;
    let (a, b) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            requests::act(&f.pool, &f.reviewer, first.request.id, a),
            requests::act(&f.pool, &f.reviewer, second.request.id, b)
        )
    })
    .await
    .unwrap();
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let w = projects::amend(
        &f.pool,
        &f.owner,
        w.project.id,
        StartBudgetAmendmentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            reason: "ย้ายยอดภายในหมวด".into(),
        },
    )
    .await
    .unwrap();
    let mut input = save_input(&w);
    input.activities[0].lines[0].unit_price_satang = "1".into();
    input.activities[0].lines[1].unit_price_satang = "19999".into();
    let w = projects::save(&f.pool, &f.owner, w.project.id, input, &f.catalog)
        .await
        .unwrap();
    let mut w = projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let review = action(
        &f,
        BudgetResourceKind::Project,
        w.project.id,
        w.project.revision,
        ProcessAction::Review,
    )
    .await;
    w = projects::act(&f.pool, &f.reviewer, w.project.id, review)
        .await
        .unwrap();
    let approve = action(
        &f,
        BudgetResourceKind::Project,
        w.project.id,
        w.project.revision,
        ProcessAction::Approve,
    )
    .await; // Estimates may redistribute inside one bucket; approved additions must remain spendable without double counting.
    let w = projects::act(&f.pool, &f.reviewer, w.project.id, approve)
        .await
        .unwrap();
    assert_eq!(w.accepted_plan.as_ref().unwrap().version, 2);
    let w = projects::amend(
        &f.pool,
        &f.owner,
        w.project.id,
        StartBudgetAmendmentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            reason: "ทดสอบยอดผูกพัน".into(),
        },
    )
    .await
    .unwrap();
    let mut input = save_input(&w);
    input.activities[0].lines[1].unit_price_satang = "1".into();
    let w = projects::save(&f.pool, &f.owner, w.project.id, input, &f.catalog)
        .await
        .unwrap();
    let mut w = projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let review = action(
        &f,
        BudgetResourceKind::Project,
        w.project.id,
        w.project.revision,
        ProcessAction::Review,
    )
    .await;
    w = projects::act(&f.pool, &f.reviewer, w.project.id, review)
        .await
        .unwrap();
    let approve = action(
        &f,
        BudgetResourceKind::Project,
        w.project.id,
        w.project.revision,
        ProcessAction::Approve,
    )
    .await;
    assert!(projects::act(&f.pool, &f.reviewer, w.project.id, approve)
        .await
        .is_err());
    let unchanged = queries::workspace(&f.pool, &f.owner, w.project.id)
        .await
        .unwrap();
    assert_eq!(unchanged.accepted_plan.unwrap().version, 2);
    assert_eq!(unchanged.project.status, BudgetStatus::Submitted);
    reconciled(&f).await;
}
fn allocation(w: &BudgetProjectWorkspace) -> BudgetAllocationRef {
    let a = &w.activities[0];
    let l = &a.lines[0];
    BudgetAllocationRef {
        project_id: w.project.id,
        activity_id: a.id,
        fund_id: l.fund_id,
        expense_category_id: l.expense_category_id,
    }
}
async fn adjustment(
    f: &Fixture,
    kind: BudgetAdjustmentKind,
    from: Option<BudgetAllocationRef>,
    to: Option<BudgetAllocationRef>,
    amount: &str,
    loan: Option<Uuid>,
) -> BudgetAdjustmentDetail {
    let d = adjustments::create(
        &f.pool,
        &f.owner,
        SaveBudgetAdjustmentRequest {
            request_id: Uuid::now_v7(),
            kind,
            fiscal_year_id: f.settings.fiscal_years[0].id,
            from,
            to,
            amount_satang: amount.into(),
            reason: "รายการทดสอบ".into(),
            loan_id: loan,
        },
    )
    .await
    .unwrap();
    let mut d = adjustments::submit(
        &f.pool,
        &f.owner,
        d.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: d.revision,
        },
    )
    .await
    .unwrap();
    for a in [ProcessAction::Review, ProcessAction::Approve] {
        let i = action(f, BudgetResourceKind::Adjustment, d.id, d.revision, a).await;
        d = adjustments::act(&f.pool, &f.reviewer, d.id, i)
            .await
            .unwrap();
    }
    assert_review_history(f, BudgetResourceKind::Adjustment, d.id).await;
    d
}
#[tokio::test]
async fn budget_movements_loans_repayments_and_no_double_debit() {
    let f = fixture("budget_loans").await;
    let a = approve_plan(
        &f,
        draft(&f, vec![activity("ผู้ให้ยืม", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let b = approve_plan(
        &f,
        draft(&f, vec![activity("ผู้ยืม", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let af = allocation(&a);
    let bf = allocation(&b);
    adjustment(
        &f,
        BudgetAdjustmentKind::Add,
        None,
        Some(af.clone()),
        "1000",
        None,
    )
    .await;
    adjustment(
        &f,
        BudgetAdjustmentKind::Reduce,
        Some(af.clone()),
        None,
        "500",
        None,
    )
    .await;
    adjustment(
        &f,
        BudgetAdjustmentKind::Transfer,
        Some(af.clone()),
        Some(bf.clone()),
        "500",
        None,
    )
    .await;
    let loan = adjustment(
        &f,
        BudgetAdjustmentKind::Loan,
        Some(af.clone()),
        Some(bf.clone()),
        "3000",
        None,
    )
    .await;
    assert_eq!(loan.loan_outstanding_satang, "3000");
    let w = queries::workspace(&f.pool, &f.owner, a.project.id)
        .await
        .unwrap();
    assert_eq!(w.project.available_satang, "7000");
    let repay = adjustment(
        &f,
        BudgetAdjustmentKind::Repayment,
        Some(bf.clone()),
        Some(af.clone()),
        "1000",
        Some(loan.id),
    )
    .await;
    assert_eq!(repay.loan_outstanding_satang, "2000");
    let w = queries::workspace(&f.pool, &f.owner, a.project.id)
        .await
        .unwrap();
    assert_eq!(w.project.available_satang, "8000");
    let bad = adjustments::create(
        &f.pool,
        &f.owner,
        SaveBudgetAdjustmentRequest {
            request_id: Uuid::now_v7(),
            kind: BudgetAdjustmentKind::Repayment,
            fiscal_year_id: f.settings.fiscal_years[0].id,
            from: Some(bf),
            to: Some(af),
            amount_satang: "9999".into(),
            reason: "เกินยอด".into(),
            loan_id: Some(loan.id),
        },
    )
    .await
    .unwrap();
    let mut bad = adjustments::submit(
        &f.pool,
        &f.owner,
        bad.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: bad.revision,
        },
    )
    .await
    .unwrap();
    let review = action(
        &f,
        BudgetResourceKind::Adjustment,
        bad.id,
        bad.revision,
        ProcessAction::Review,
    )
    .await;
    bad = adjustments::act(&f.pool, &f.reviewer, bad.id, review)
        .await
        .unwrap();
    let approve = action(
        &f,
        BudgetResourceKind::Adjustment,
        bad.id,
        bad.revision,
        ProcessAction::Approve,
    )
    .await;
    assert!(adjustments::act(&f.pool, &f.reviewer, bad.id, approve)
        .await
        .is_err());
    assert_eq!(
        adjustments::detail(&f.pool, &f.owner, loan.id)
            .await
            .unwrap()
            .loan_outstanding_satang,
        "2000"
    );
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_catalog_snapshots_and_settings_preserve_archived_references() {
    let f = fixture("budget_catalog_snapshots").await;
    let mut l = line(&f, "0", 0, 0);
    l.quantity = "2".into();
    l.supply_item_id = Some(f.catalog.item);
    l.supply_version_id = Some(f.catalog.version);
    let w = draft(&f, vec![activity("วัสดุ", l)]).await;
    assert_eq!(w.activities[0].lines[0].total_satang, "358800");
    f.catalog.price.store(500000, Ordering::SeqCst);
    let mut input = save_input(&w);
    input.name = "ปรับชื่อเท่านั้น".into();
    let w = projects::save(&f.pool, &f.owner, w.project.id, input, &f.catalog)
        .await
        .unwrap();
    assert_eq!(w.activities[0].lines[0].unit_price_satang, "179400");
    let old = settings::get(&f.pool, &f.reviewer).await.unwrap();
    let mut update = SaveBudgetSettingsRequest {
        request_id: Uuid::now_v7(),
        expected_revision: old.revision,
        fiscal_years: old
            .fiscal_years
            .iter()
            .map(|x| BudgetFiscalYearInput {
                id: Some(x.id),
                name: x.name.clone(),
                start_date: x.start_date,
                end_date: x.end_date,
                active: x.active,
            })
            .collect(),
        funds: old
            .funds
            .iter()
            .map(|x| BudgetCatalogItemInput {
                id: Some(x.id),
                name: x.name.clone(),
                active: false,
            })
            .collect(),
        expense_categories: old
            .expense_categories
            .iter()
            .map(|x| BudgetCatalogItemInput {
                id: Some(x.id),
                name: x.name.clone(),
                active: false,
            })
            .collect(),
        plan_steps: old.plan_steps,
        request_steps: old.request_steps,
        adjustment_steps: old.adjustment_steps,
    };
    settings::save(&f.pool, &f.reviewer, update.clone())
        .await
        .unwrap();
    assert_eq!(
        settings::catalogs(&f.pool, &f.owner)
            .await
            .unwrap()
            .funds
            .len(),
        2
    );
    let saved = projects::save(&f.pool, &f.owner, w.project.id, save_input(&w), &f.catalog)
        .await
        .unwrap();
    assert_eq!(
        saved.activities[0].lines[0].fund_id,
        w.activities[0].lines[0].fund_id
    );
    update.request_id = Uuid::now_v7();
    update.expected_revision += 1;
    update.fiscal_years[0].end_date = date(10, 31);
    assert!(settings::save(&f.pool, &f.reviewer, update).await.is_err());
    reconciled(&f).await;
}
#[test]
fn budget_wire_payloads_are_frontend_shaped_and_money_remains_strings() {
    let id = Uuid::now_v7();
    let payload = serde_json::json!({"requestId":id,"expectedRevision":1,"fiscalYears":[{"name":"2570","startDate":"2026-10-01","endDate":"2027-09-30","active":true}],"funds":[{"name":"ทุน","active":true}],"expenseCategories":[],"planSteps":[],"requestSteps":[],"adjustmentSteps":[]});
    let x: SaveBudgetSettingsRequest = serde_json::from_value(payload).unwrap();
    assert!(x.funds[0].id.is_none());
    let payload = serde_json::json!({"requestId":id,"name":"โครงการ","fiscalYearId":id,"organizationUnitId":id,"startDate":"2026-11-01","endDate":"2026-12-01"});
    let _: CreateBudgetProjectRequest = serde_json::from_value(payload.clone()).unwrap();
    let mut payload = payload;
    payload["expectedRevision"] = 1.into();
    payload["activities"] = serde_json::json!([{"title":"กิจกรรม","defaultFundId":null,"defaultExpenseCategoryId":null,"lines":[{"description":"รายการ","quantity":"1.005","unit":"ชิ้น","unitPriceSatang":"100","fundId":id,"expenseCategoryId":id}]}]);
    let x: SaveBudgetProjectRequest = serde_json::from_value(payload).unwrap();
    assert_eq!(x.activities[0].lines[0].unit_price_satang, "100");
    let _:SaveBudgetSpendingRequestRequest=serde_json::from_value(serde_json::json!({"requestId":id,"expectedRevision":1,"activityId":id,"title":"คำขอ","lines":[{"planLineId":id,"amountSatang":"100"}],"evidenceFileIds":[]})).unwrap();
    let _:BudgetActionRequest=serde_json::from_value(serde_json::json!({"requestId":id,"expectedRevision":1,"expectedProcessRevision":1,"action":"return_for_revision","comment":"ปรับแก้"})).unwrap();
    let _:CreateBudgetPaymentRequest=serde_json::from_value(serde_json::json!({"requestId":id,"expectedRevision":1,"paidOn":"2026-11-02","voucherNumber":"PV-1","evidenceFileIds":[],"allocations":[{"planLineId":id,"amountSatang":"100"}]})).unwrap();
    let _:SaveBudgetAdjustmentRequest=serde_json::from_value(serde_json::json!({"requestId":id,"kind":"add","fiscalYearId":id,"to":{"projectId":id,"activityId":id,"fundId":id,"expenseCategoryId":id},"amountSatang":"100","reason":"เพิ่มเงิน"})).unwrap();
}
async fn wait_blocked(pool: &PgPool, blocker: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker)
            .fetch_one(pool)
            .await
            .unwrap();
            if blocked {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
fn settings_input(s: &BudgetSettings) -> SaveBudgetSettingsRequest {
    SaveBudgetSettingsRequest {
        request_id: Uuid::now_v7(),
        expected_revision: s.revision,
        fiscal_years: s
            .fiscal_years
            .iter()
            .map(|x| BudgetFiscalYearInput {
                id: Some(x.id),
                name: x.name.clone(),
                start_date: x.start_date,
                end_date: x.end_date,
                active: x.active,
            })
            .collect(),
        funds: s
            .funds
            .iter()
            .map(|x| BudgetCatalogItemInput {
                id: Some(x.id),
                name: x.name.clone(),
                active: x.active,
            })
            .collect(),
        expense_categories: s
            .expense_categories
            .iter()
            .map(|x| BudgetCatalogItemInput {
                id: Some(x.id),
                name: x.name.clone(),
                active: x.active,
            })
            .collect(),
        plan_steps: s.plan_steps.clone(),
        request_steps: s.request_steps.clone(),
        adjustment_steps: s.adjustment_steps.clone(),
    }
}
#[tokio::test]
async fn budget_settings_shared_guard_serializes_year_shrink_and_project_save() {
    let f = fixture("budget_settings_race").await;
    let w = draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await;
    let mut guard = f.pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    sqlx::query("SELECT singleton FROM budget_settings WHERE singleton FOR UPDATE")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    let mut input = save_input(&w);
    input.end_date = NaiveDate::from_ymd_opt(2027, 9, 1).unwrap();
    let pool = f.pool.clone();
    let owner = f.owner.clone();
    let catalog = Catalog {
        item: f.catalog.item,
        version: f.catalog.version,
        price: AtomicI64::new(1),
    };
    let id = w.project.id;
    let future =
        tokio::spawn(async move { projects::save(&pool, &owner, id, input, &catalog).await });
    wait_blocked(&f.pool, pid).await;
    sqlx::query("UPDATE budget_fiscal_years SET end_date='2026-12-31' WHERE id=$1")
        .bind(f.settings.fiscal_years[0].id)
        .execute(&mut *guard)
        .await
        .unwrap();
    guard.commit().await.unwrap();
    assert!(future.await.unwrap().is_err());
    assert_eq!(
        queries::workspace(&f.pool, &f.owner, id)
            .await
            .unwrap()
            .project
            .end_date,
        date(12, 30)
    );
    // The opposite ordering makes settings wait for a business save's shared guard.
    let mut guard = f.pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    sqlx::query("SELECT singleton FROM budget_settings WHERE singleton FOR SHARE")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    let pool = f.pool.clone();
    let reviewer = f.reviewer.clone();
    let mut input = settings_input(&f.settings);
    input.fiscal_years[0].end_date = date(10, 31);
    let future = tokio::spawn(async move { settings::save(&pool, &reviewer, input).await });
    wait_blocked(&f.pool, pid).await;
    guard.commit().await.unwrap();
    assert!(future.await.unwrap().is_err());
}
#[tokio::test]
async fn budget_locked_resource_authorization_is_rechecked_after_unit_move() {
    let f = fixture("budget_scope_race").await;
    let w = draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await;
    sqlx::query("INSERT INTO organization_members(id,organization_unit_id,user_id,position_code,started_at) VALUES($1,$2,$3,'member','2020-01-01')").bind(Uuid::now_v7()).bind(f.unit).bind(f.owner.user_id).execute(&f.pool).await.unwrap();
    let scoped = ActorContext {
        user_id: f.owner.user_id,
        permissions: vec![
            codes::BUDGET_READ_ORGANIZATION_UNIT.into(),
            codes::BUDGET_UPDATE_ORGANIZATION_UNIT.into(),
        ],
    };
    assert!(queries::workspace(&f.pool, &scoped, w.project.id)
        .await
        .is_ok());
    let destination: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE code='BUD-FIN'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let mut guard = f.pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM budget_projects WHERE id=$1 FOR UPDATE")
        .bind(w.project.id)
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    let mut input = save_input(&w);
    input.expected_revision += 1;
    input.name = "unauthorized after move".into();
    let key = input.request_id;
    let pool = f.pool.clone();
    let catalog = Catalog {
        item: f.catalog.item,
        version: f.catalog.version,
        price: AtomicI64::new(1),
    };
    let id = w.project.id;
    let future =
        tokio::spawn(async move { projects::save(&pool, &scoped, id, input, &catalog).await });
    wait_blocked(&f.pool, pid).await;
    sqlx::query("UPDATE budget_projects SET organization_unit_id=$2,revision=revision+1,document=jsonb_set(document,'{project,organizationUnitId}',to_jsonb($2::uuid)) WHERE id=$1").bind(id).bind(destination).execute(&mut *guard).await.unwrap();
    guard.commit().await.unwrap();
    assert!(future.await.unwrap().is_err());
    let receipts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM budget_receipts WHERE request_id=$1")
            .bind(key)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(receipts, 0);
    assert_ne!(
        queries::workspace(&f.pool, &f.owner, id)
            .await
            .unwrap()
            .project
            .name,
        "unauthorized after move"
    );
}
#[tokio::test]
async fn budget_permission_combinations_refuse_before_commit_and_return_routes_are_safe() {
    let f = fixture("budget_permission_combinations").await;
    let creator = ActorContext {
        user_id: f.owner.user_id,
        permissions: vec![
            codes::BUDGET_CREATE_OWN.into(),
            codes::BUDGET_UPDATE_SCHOOL.into(),
        ],
    };
    let key = Uuid::now_v7();
    assert!(projects::create(
        &f.pool,
        &creator,
        CreateBudgetProjectRequest {
            request_id: key,
            name: "denied".into(),
            fiscal_year_id: f.settings.fiscal_years[0].id,
            organization_unit_id: f.unit,
            start_date: date(11, 1),
            end_date: date(12, 1)
        }
    )
    .await
    .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM budget_receipts WHERE request_id=$1")
        .bind(key)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let mut update = settings_input(&f.settings);
    update.plan_steps = vec![step(f.owner.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let w = draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await;
    assert!(projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision
        }
    )
    .await
    .is_err());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM process_instances WHERE source_module='budget'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let saved = queries::workspace(&f.pool, &f.owner, w.project.id)
        .await
        .unwrap();
    assert_eq!(saved.project.status, BudgetStatus::Draft);
    let current = settings::get(&f.pool, &f.reviewer).await.unwrap();
    let mut update = settings_input(&current);
    update.plan_steps = vec![
        step(f.reviewer.user_id, ProcessStepKind::Review),
        step(f.reviewer.user_id, ProcessStepKind::Approve),
    ];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let mut w = projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let wrong = action(
        &f,
        BudgetResourceKind::Project,
        w.project.id,
        w.project.revision,
        ProcessAction::Approve,
    )
    .await;
    assert!(projects::act(&f.pool, &f.reviewer, w.project.id, wrong)
        .await
        .is_err());
    let returned = action(
        &f,
        BudgetResourceKind::Project,
        w.project.id,
        w.project.revision,
        ProcessAction::ReturnForRevision,
    )
    .await;
    w = projects::act(&f.pool, &f.reviewer, w.project.id, returned)
        .await
        .unwrap();
    assert!(w.balances.is_empty());
    let w = approve_plan(&f, w).await;
    let r = approve_request(&f, request(&f, &w, 0, "1000").await).await;
    let finance = ActorContext {
        user_id: f.reviewer.user_id,
        permissions: vec![codes::BUDGET_PAYMENT_EXECUTE_SCHOOL.into()],
    };
    let key = Uuid::now_v7();
    assert!(requests::pay(
        &f.pool,
        &finance,
        r.request.id,
        CreateBudgetPaymentRequest {
            request_id: key,
            expected_revision: r.request.revision,
            paid_on: date(11, 2),
            voucher_number: "denied".into(),
            note: None,
            evidence_file_ids: vec![],
            allocations: vec![BudgetPaymentAllocationInput {
                plan_line_id: r.lines[0].plan_line_id,
                amount_satang: "1000".into()
            }]
        }
    )
    .await
    .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM budget_receipts WHERE request_id=$1")
        .bind(key)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_credited_activity_allocation_is_spendable_above_line_estimate() {
    let f = fixture("budget_credited_spending").await;
    let a = approve_plan(
        &f,
        draft(&f, vec![activity("ต้นทาง", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let b = approve_plan(
        &f,
        draft(&f, vec![activity("ปลายทาง", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let af = allocation(&a);
    let bf = allocation(&b);
    adjustment(
        &f,
        BudgetAdjustmentKind::Add,
        None,
        Some(af.clone()),
        "5000",
        None,
    )
    .await;
    let r = approve_request(&f, request(&f, &a, 0, "15000").await).await;
    assert_eq!(r.request.reserved_satang, "15000");
    let extra = request(&f, &a, 0, "1").await;
    let mut extra = submit_request(&f, extra).await;
    let review = action(
        &f,
        BudgetResourceKind::Request,
        extra.request.id,
        extra.request.revision,
        ProcessAction::Review,
    )
    .await;
    extra = requests::act(&f.pool, &f.reviewer, extra.request.id, review)
        .await
        .unwrap();
    let approve = action(
        &f,
        BudgetResourceKind::Request,
        extra.request.id,
        extra.request.revision,
        ProcessAction::Approve,
    )
    .await;
    assert!(
        requests::act(&f.pool, &f.reviewer, extra.request.id, approve)
            .await
            .is_err()
    );
    requests::close(
        &f.pool,
        &f.reviewer,
        r.request.id,
        CloseBudgetRequestRequest {
            request_id: Uuid::now_v7(),
            expected_revision: r.request.revision,
            reason: "จบการทดสอบ".into(),
        },
    )
    .await
    .unwrap();
    adjustment(
        &f,
        BudgetAdjustmentKind::Transfer,
        Some(af.clone()),
        Some(bf.clone()),
        "1000",
        None,
    )
    .await;
    let r = approve_request(&f, request(&f, &b, 0, "11000").await).await;
    requests::close(
        &f.pool,
        &f.reviewer,
        r.request.id,
        CloseBudgetRequestRequest {
            request_id: Uuid::now_v7(),
            expected_revision: r.request.revision,
            reason: "จบการทดสอบ".into(),
        },
    )
    .await
    .unwrap();
    adjustment(
        &f,
        BudgetAdjustmentKind::Loan,
        Some(af),
        Some(bf),
        "2000",
        None,
    )
    .await;
    let r = approve_request(&f, request(&f, &b, 0, "13000").await).await;
    assert_eq!(r.request.reserved_satang, "13000");
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_single_connection_transactions_never_reacquire_the_pool() {
    let f = fixture_with_connections("budget_one_connection", 1).await;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let w = approve_plan(
            &f,
            draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await,
        )
        .await;
        let r = approve_request(&f, request(&f, &w, 0, "8000").await).await;
        let r = requests::pay(
            &f.pool,
            &f.reviewer,
            r.request.id,
            CreateBudgetPaymentRequest {
                request_id: Uuid::now_v7(),
                expected_revision: r.request.revision,
                paid_on: date(11, 2),
                voucher_number: "NO-ATTACHMENT".into(),
                note: None,
                evidence_file_ids: vec![],
                allocations: vec![BudgetPaymentAllocationInput {
                    plan_line_id: r.lines[0].plan_line_id,
                    amount_satang: "1000".into(),
                }],
            },
        )
        .await
        .unwrap();
        requests::close(
            &f.pool,
            &f.reviewer,
            r.request.id,
            CloseBudgetRequestRequest {
                request_id: Uuid::now_v7(),
                expected_revision: r.request.revision,
                reason: "complete".into(),
            },
        )
        .await
        .unwrap();
        adjustment(
            &f,
            BudgetAdjustmentKind::Add,
            None,
            Some(allocation(&w)),
            "1000",
            None,
        )
        .await;
        reconciled(&f).await;
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn budget_approvals_at_pool_capacity_finish_without_pool_starvation() {
    let f = fixture_with_connections("budget_pool_capacity", 3).await;
    let w = approve_plan(
        &f,
        draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let mut rs = vec![];
    for _ in 0..3 {
        let r = request(&f, &w, 0, "6000").await;
        let mut r = submit_request(&f, r).await;
        let review = action(
            &f,
            BudgetResourceKind::Request,
            r.request.id,
            r.request.revision,
            ProcessAction::Review,
        )
        .await;
        r = requests::act(&f.pool, &f.reviewer, r.request.id, review)
            .await
            .unwrap();
        rs.push(r);
    }
    let a = action(
        &f,
        BudgetResourceKind::Request,
        rs[0].request.id,
        rs[0].request.revision,
        ProcessAction::Approve,
    )
    .await;
    let b = action(
        &f,
        BudgetResourceKind::Request,
        rs[1].request.id,
        rs[1].request.revision,
        ProcessAction::Approve,
    )
    .await;
    let c = action(
        &f,
        BudgetResourceKind::Request,
        rs[2].request.id,
        rs[2].request.revision,
        ProcessAction::Approve,
    )
    .await;
    let results = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            requests::act(&f.pool, &f.reviewer, rs[0].request.id, a),
            requests::act(&f.pool, &f.reviewer, rs[1].request.id, b),
            requests::act(&f.pool, &f.reviewer, rs[2].request.id, c)
        )
    })
    .await
    .unwrap();
    assert_eq!(
        usize::from(results.0.is_ok())
            + usize::from(results.1.is_ok())
            + usize::from(results.2.is_ok()),
        1
    );
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_rejected_attempt_retains_assigned_reader_and_adjustment_snapshots() {
    let f = fixture("budget_attempt_history").await;
    let w = approve_plan(
        &f,
        draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    assert_eq!(sqlx::query("INSERT INTO user_roles(id,user_id,role_id,started_at) SELECT $1,$2,id,'2020-01-01' FROM roles WHERE code='ADMIN'").bind(Uuid::now_v7()).bind(f.other.user_id).execute(&f.pool).await.unwrap().rows_affected(),1);
    let assigned = ActorContext {
        user_id: f.other.user_id,
        permissions: vec![
            codes::BUDGET_READ_ASSIGNED.into(),
            codes::BUDGET_APPROVE_ASSIGNED.into(),
        ],
    };
    let mut update = settings_input(&f.settings);
    update.plan_steps = vec![step(assigned.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let w = projects::amend(
        &f.pool,
        &f.owner,
        w.project.id,
        StartBudgetAmendmentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            reason: "แก้ไข".into(),
        },
    )
    .await
    .unwrap();
    let w = projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let process = queries::resource_process(
        &f.pool,
        &assigned,
        BudgetResourceKind::Project,
        w.project.id,
    )
    .await
    .unwrap()
    .process
    .unwrap();
    let w = projects::act(
        &f.pool,
        &assigned,
        w.project.id,
        BudgetActionRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            expected_process_revision: process.revision,
            action: ProcessAction::Reject,
            comment: Some("ปฏิเสธฉบับแก้ไข".into()),
        },
    )
    .await
    .unwrap();
    let w = projects::amend(
        &f.pool,
        &f.owner,
        w.project.id,
        StartBudgetAmendmentRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            reason: "ร่างใหม่".into(),
        },
    )
    .await
    .unwrap();
    assert!(queries::workspace(&f.pool, &assigned, w.project.id)
        .await
        .is_ok());
    assert!(!queries::history(
        &f.pool,
        &assigned,
        BudgetResourceKind::Project,
        w.project.id,
        BudgetListQuery::default()
    )
    .await
    .unwrap()
    .items
    .is_empty());
    let d = adjustment(
        &f,
        BudgetAdjustmentKind::Add,
        None,
        Some(allocation(&w)),
        "1000",
        None,
    )
    .await;
    let snapshots: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM budget_adjustment_revisions WHERE adjustment_id=$1",
    )
    .bind(d.id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(snapshots, d.revision);
    assert!(sqlx::query(
        "UPDATE budget_adjustment_revisions SET document='{}' WHERE adjustment_id=$1"
    )
    .bind(d.id)
    .execute(&f.pool)
    .await
    .is_err());
}
#[tokio::test]
async fn budget_unactionable_recipient_is_refused_and_missing_ids_remain_not_found() {
    let f = fixture("budget_routing_read_denial").await;
    let mut update = settings_input(&f.settings);
    update.plan_steps = vec![step(f.other.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let w = draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await;
    let key = Uuid::now_v7();
    assert!(projects::submit(
        &f.pool,
        &f.owner,
        w.project.id,
        BudgetMutationRequest {
            request_id: key,
            expected_revision: w.project.revision
        }
    )
    .await
    .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM budget_receipts WHERE request_id=$1")
        .bind(key)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM process_instances WHERE source_module='budget'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert!(matches!(
        queries::workspace(&f.pool, &f.owner, Uuid::now_v7()).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        queries::request_detail(&f.pool, &f.owner, Uuid::now_v7()).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        adjustments::detail(&f.pool, &f.owner, Uuid::now_v7()).await,
        Err(AppError::NotFound(_))
    ));
}
#[tokio::test]
async fn budget_delegated_submitter_cannot_review_but_third_party_can() {
    let f = fixture("budget_delegated_self_approval").await;
    let w = draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await;
    // Owner A delegates submission to school updater B, who is also the configured
    // reviewer. Source ownership alone must not make B's self-approval actionable.
    let key = Uuid::now_v7();
    assert!(projects::submit(
        &f.pool,
        &f.reviewer,
        w.project.id,
        BudgetMutationRequest {
            request_id: key,
            expected_revision: w.project.revision,
        },
    )
    .await
    .is_err());
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM budget_receipts WHERE request_id=$1),(SELECT count(*) FROM process_instances WHERE source_module='budget')")
        .bind(key).fetch_one(&f.pool).await.unwrap();
    assert_eq!(counts, (0, 0));
    assert_eq!(
        queries::workspace(&f.pool, &f.owner, w.project.id)
            .await
            .unwrap()
            .project
            .status,
        BudgetStatus::Draft
    );

    // A separate active, genuinely granted reviewer C is actionable for B's submission.
    let third = ActorContext {
        user_id: f.other.user_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    assert_eq!(sqlx::query("INSERT INTO user_roles(id,user_id,role_id,started_at) SELECT $1,$2,id,'2020-01-01' FROM roles WHERE code='ADMIN'").bind(Uuid::now_v7()).bind(third.user_id).execute(&f.pool).await.unwrap().rows_affected(),1);
    let mut update = settings_input(&f.settings);
    update.plan_steps = vec![step(third.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let w = projects::submit(
        &f.pool,
        &f.reviewer,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let p = queries::resource_process(&f.pool, &third, BudgetResourceKind::Project, w.project.id)
        .await
        .unwrap()
        .process
        .unwrap();
    assert_eq!(p.created_by, f.reviewer.user_id);
    assert!(
        !queries::workspace(&f.pool, &f.reviewer, w.project.id)
            .await
            .unwrap()
            .capabilities
            .can_act
    );
    assert!(
        queries::workspace(&f.pool, &third, w.project.id)
            .await
            .unwrap()
            .capabilities
            .can_act
    );
    let w = projects::act(
        &f.pool,
        &third,
        w.project.id,
        BudgetActionRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            expected_process_revision: p.revision,
            action: ProcessAction::Approve,
            comment: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(w.project.status, BudgetStatus::Approved);
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_returned_routing_excludes_original_submitter_and_preserves_failed_resubmit() {
    let f = fixture("budget_returned_submitter_route").await;
    let third = ActorContext {
        user_id: f.other.user_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let resubmitter = ActorContext {
        user_id: Uuid::now_v7(),
        permissions: vec![codes::WILDCARD.into()],
    };
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Resubmitter','Fixture','staff','active')").bind(resubmitter.user_id).bind(resubmitter.user_id.to_string()).execute(&f.pool).await.unwrap();
    for actor in [&third, &resubmitter] {
        assert_eq!(sqlx::query("INSERT INTO user_roles(id,user_id,role_id,started_at) SELECT $1,$2,id,'2020-01-01' FROM roles WHERE code='ADMIN'").bind(Uuid::now_v7()).bind(actor.user_id).execute(&f.pool).await.unwrap().rows_affected(),1);
    }
    let mut update = settings_input(&f.settings);
    update.plan_steps = vec![step(third.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let w = draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await;
    // Manager A submits owner O's draft, and independent reviewer C returns it.
    let w = projects::submit(
        &f.pool,
        &f.reviewer,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let p = queries::resource_process(&f.pool, &third, BudgetResourceKind::Project, w.project.id)
        .await
        .unwrap()
        .process
        .unwrap();
    assert_eq!(p.created_by, f.reviewer.user_id);
    let w = projects::act(
        &f.pool,
        &third,
        w.project.id,
        BudgetActionRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            expected_process_revision: p.revision,
            action: ProcessAction::ReturnForRevision,
            comment: Some("แก้ไขก่อนส่งใหม่".into()),
        },
    )
    .await
    .unwrap();
    let mut update = settings_input(&settings::get(&f.pool, &f.reviewer).await.unwrap());
    update.plan_steps = vec![step(f.reviewer.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    // B must not resubmit to original submitter A: Workflow retains A as creator.
    // Compare persisted process/steps, work, events, project, and source history.
    let snapshot_sql = "SELECT jsonb_build_object('process',(SELECT to_jsonb(i) FROM process_instances i WHERE id=$1),'work',(SELECT jsonb_agg(to_jsonb(w) ORDER BY id) FROM work_items w),'assignees',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM work_item_assignees a),'events',(SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM process_action_events e),'history',(SELECT jsonb_agg(to_jsonb(h) ORDER BY id) FROM budget_history h WHERE resource_id=$2),'project',(SELECT to_jsonb(p) FROM budget_projects p WHERE id=$2))";
    let before: serde_json::Value = sqlx::query_scalar(snapshot_sql)
        .bind(p.id)
        .bind(w.project.id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let key = Uuid::now_v7();
    assert!(projects::submit(
        &f.pool,
        &resubmitter,
        w.project.id,
        BudgetMutationRequest {
            request_id: key,
            expected_revision: w.project.revision,
        }
    )
    .await
    .is_err());
    let after: serde_json::Value = sqlx::query_scalar(snapshot_sql)
        .bind(p.id)
        .bind(w.project.id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(after, before);
    let receipts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM budget_receipts WHERE request_id=$1")
            .bind(key)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(receipts, 0);

    // Restoring independent C remains actionable; the same process retains creator A.
    let mut update = settings_input(&settings::get(&f.pool, &f.reviewer).await.unwrap());
    update.plan_steps = vec![step(third.user_id, ProcessStepKind::Approve)];
    settings::save(&f.pool, &f.reviewer, update).await.unwrap();
    let w = projects::submit(
        &f.pool,
        &resubmitter,
        w.project.id,
        BudgetMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
        },
    )
    .await
    .unwrap();
    let current =
        queries::resource_process(&f.pool, &third, BudgetResourceKind::Project, w.project.id)
            .await
            .unwrap()
            .process
            .unwrap();
    assert_eq!(current.id, p.id);
    assert_eq!(current.created_by, f.reviewer.user_id);
    assert!(
        queries::workspace(&f.pool, &third, w.project.id)
            .await
            .unwrap()
            .capabilities
            .can_act
    );
    let w = projects::act(
        &f.pool,
        &third,
        w.project.id,
        BudgetActionRequest {
            request_id: Uuid::now_v7(),
            expected_revision: w.project.revision,
            expected_process_revision: current.revision,
            action: ProcessAction::Approve,
            comment: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(w.project.status, BudgetStatus::Approved);
    reconciled(&f).await;
}
#[tokio::test]
async fn budget_report_export_scope_is_full_read_and_download_intersection() {
    let f = fixture("budget_export_intersection").await;
    let first = approve_plan(
        &f,
        draft(&f, vec![activity("ของฉัน", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let other = Fixture {
        pool: f.pool.clone(),
        owner: ActorContext {
            user_id: f.other.user_id,
            permissions: vec![codes::WILDCARD.into()],
        },
        reviewer: f.reviewer.clone(),
        other: f.other.clone(),
        settings: f.settings.clone(),
        unit: f.unit,
        catalog: Catalog {
            item: f.catalog.item,
            version: f.catalog.version,
            price: AtomicI64::new(1),
        },
    };
    let second = approve_plan(
        &other,
        draft(&other, vec![activity("อีกคน", line(&other, "20000", 0, 0))]).await,
    )
    .await;
    let reader = ActorContext {
        user_id: f.owner.user_id,
        permissions: vec![
            codes::BUDGET_READ_SCHOOL.into(),
            codes::BUDGET_DOWNLOAD_OWN.into(),
        ],
    };
    assert_eq!(
        queries::dashboard(&f.pool, &reader, BudgetReportQuery::default(), false)
            .await
            .unwrap()
            .totals
            .planned_satang,
        "30000"
    );
    let q = BudgetReportQuery {
        export_scope: Some(true),
        ..Default::default()
    };
    let report = queries::dashboard(&f.pool, &reader, q.clone(), false)
        .await
        .unwrap();
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].project_id, first.project.id);
    assert_eq!(report.totals.planned_satang, "10000");
    let csv = queries::export_csv(&f.pool, &reader, q.clone())
        .await
        .unwrap();
    assert!(csv.contains("100.00"));
    assert!(!csv.contains("อีกคน"));
    let reverse = ActorContext {
        user_id: f.owner.user_id,
        permissions: vec![
            codes::BUDGET_READ_OWN.into(),
            codes::BUDGET_DOWNLOAD_SCHOOL.into(),
        ],
    };
    assert_eq!(
        queries::dashboard(&f.pool, &reverse, q.clone(), false)
            .await
            .unwrap()
            .rows
            .len(),
        1
    );
    let q = BudgetReportQuery {
        project_id: Some(second.project.id),
        ..q
    };
    assert!(queries::dashboard(&f.pool, &reverse, q, false)
        .await
        .unwrap()
        .rows
        .is_empty());
}
#[tokio::test]
async fn budget_concurrent_payments_cannot_exceed_remaining_reservation() {
    let f = fixture("budget_concurrent_payments").await;
    let w = approve_plan(
        &f,
        draft(&f, vec![activity("กิจกรรม", line(&f, "10000", 0, 0))]).await,
    )
    .await;
    let r = approve_request(&f, request(&f, &w, 0, "8000").await).await;
    let a = CreateBudgetPaymentRequest {
        request_id: Uuid::now_v7(),
        expected_revision: r.request.revision,
        paid_on: date(11, 2),
        voucher_number: "P-A".into(),
        note: None,
        evidence_file_ids: vec![],
        allocations: vec![BudgetPaymentAllocationInput {
            plan_line_id: r.lines[0].plan_line_id,
            amount_satang: "6000".into(),
        }],
    };
    let mut b = a.clone();
    b.request_id = Uuid::now_v7();
    b.expected_revision += 1;
    b.voucher_number = "P-B".into();
    let (a, b) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            requests::pay(&f.pool, &f.reviewer, r.request.id, a),
            requests::pay(&f.pool, &f.reviewer, r.request.id, b)
        )
    })
    .await
    .unwrap();
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let r = queries::request_detail(&f.pool, &f.owner, r.request.id)
        .await
        .unwrap();
    assert_eq!(r.request.paid_satang, "6000");
    assert_eq!(r.request.reserved_satang, "2000");
    assert_eq!(
        queries::payments(&f.pool, &f.owner, r.request.id, BudgetListQuery::default())
            .await
            .unwrap()
            .total,
        1
    );
    reconciled(&f).await;
}
