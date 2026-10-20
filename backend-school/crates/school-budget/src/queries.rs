//! Bounded, resource-authorized projections; dashboard and CSV share the same accounts.
use crate::{
    models::*,
    money::{self, invalid},
    policy, process,
    repository::{self, ProjectRecord, RequestRecord},
};
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;
const PROJECT_SCOPE:&str="($1 OR ($2 AND p.owner_id=$3) OR p.organization_unit_id=ANY($4) OR ($5 AND EXISTS(SELECT 1 FROM process_participants pp JOIN process_instances i ON i.id=pp.instance_id WHERE pp.user_id=$3 AND i.source_module='budget' AND ((i.source_type='budget_project_plan' AND EXISTS(SELECT 1 FROM budget_project_attempts attempt WHERE attempt.project_id=p.id AND attempt.id=i.source_id)) OR (i.source_type='budget_request' AND EXISTS(SELECT 1 FROM budget_requests r WHERE r.project_id=p.id AND r.id=i.source_id)) OR (i.source_type='budget_adjustment' AND EXISTS(SELECT 1 FROM budget_adjustments a WHERE (a.from_project_id=p.id OR a.to_project_id=p.id) AND a.id=i.source_id))))))";
pub(crate) async fn summary(
    tx: &mut Transaction<'_, Postgres>,
    r: &ProjectRecord,
) -> Result<BudgetProjectSummary, AppError> {
    let unit: String = sqlx::query_scalar("SELECT name FROM organization_units WHERE id=$1")
        .bind(r.document.project.organization_unit_id)
        .fetch_one(&mut **tx)
        .await?;
    let (planned,reserved,paid,available):(i64,i64,i64,i64)=sqlx::query_as("SELECT coalesce(sum(planned),0)::bigint,coalesce(sum(reserved),0)::bigint,coalesce(sum(paid),0)::bigint,coalesce(sum(planned+adjusted-reserved-paid),0)::bigint FROM budget_allocations WHERE project_id=$1").bind(r.id).fetch_one(&mut **tx).await?;
    project_summary_from(r, unit, (planned, reserved, paid, available))
}
fn project_summary_from(
    r: &ProjectRecord,
    unit: String,
    totals: (i64, i64, i64, i64),
) -> Result<BudgetProjectSummary, AppError> {
    let (planned, reserved, paid, available) = totals;
    let total = if r.accepted_version.is_some() {
        planned
    } else {
        money::sum(
            r.document
                .activities
                .iter()
                .map(|x| money::parse_money(&x.total_satang))
                .collect::<Result<Vec<_>, _>>()?,
        )?
    };
    let p = &r.document.project;
    Ok(BudgetProjectSummary {
        id: r.id,
        revision: r.revision,
        name: p.name.clone(),
        fiscal_year_id: p.fiscal_year_id,
        organization_unit_id: p.organization_unit_id,
        organization_unit_name: unit,
        start_date: p.start_date,
        end_date: p.end_date,
        owner_id: r.owner_id,
        status: r.status()?,
        accepted_version: r.accepted_version,
        total_satang: total.to_string(),
        reserved_satang: reserved.to_string(),
        paid_satang: paid.to_string(),
        available_satang: available.to_string(),
    })
}

pub async fn workspace(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
) -> Result<BudgetProjectWorkspace, AppError> {
    let read_scope = policy::read_scope(pool, a, false).await?;
    let scope = policy::update_scope(pool, a).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let r:ProjectRecord=sqlx::query_as("SELECT id,owner_id,revision,status,attempt_id,accepted_version,accepted_snapshot_id,document,instance_id FROM budget_projects WHERE id=$1").bind(id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("ไม่พบโครงการ".into()))?;
    policy::require_read_in_transaction(&mut tx, a, &r, &read_scope).await?;
    let i = process::load(&mut tx, r.instance_id).await?;
    let mut capabilities = policy::capabilities(
        a,
        r.status()?.editable()
            && policy::direct(
                &scope,
                a.user_id,
                r.owner_id,
                r.document.project.organization_unit_id,
            ),
        policy::can_act(a, i.as_ref(), r.owner_id),
        r.accepted_version.is_some(),
    );
    capabilities.can_amend = r.accepted_version.is_some()
        && matches!(r.status()?, BudgetStatus::Approved | BudgetStatus::Rejected)
        && policy::direct(
            &scope,
            a.user_id,
            r.owner_id,
            r.document.project.organization_unit_id,
        );
    let project = summary(&mut tx, &r).await?;
    let accepted_plan = if r.accepted_version.is_some() {
        Some(repository::accepted(&mut tx, &r).await?)
    } else {
        None
    };
    let balances = balances(&mut tx, id).await?;
    tx.commit().await?;
    Ok(BudgetProjectWorkspace {
        project,
        activities: r.document.0.activities,
        accepted_plan,
        balances,
        capabilities,
    })
}
pub async fn projects(
    pool: &PgPool,
    a: &ActorContext,
    q: BudgetListQuery,
) -> Result<BudgetProjectPage, AppError> {
    let s = policy::read_scope(pool, a, false).await?;
    let (limit, offset) = money::paging(&q)?;
    let search = q
        .search
        .as_deref()
        .map(|v| money::text(v, 200))
        .transpose()?;
    let filter=format!("{PROJECT_SCOPE} AND ($6::uuid IS NULL OR p.fiscal_year_id=$6) AND ($7::uuid IS NULL OR p.organization_unit_id=$7) AND ($8::text IS NULL OR p.status=$8) AND ($9::text IS NULL OR strpos(lower(p.document->'project'->>'name'),lower($9))>0)");
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM budget_projects p WHERE {filter}"
    )))
    .bind(s.school)
    .bind(s.own)
    .bind(a.user_id)
    .bind(&s.units)
    .bind(s.assigned)
    .bind(q.fiscal_year_id)
    .bind(q.organization_unit_id)
    .bind(q.status.map(|x| x.as_str()))
    .bind(&search)
    .fetch_one(&mut *tx)
    .await?;
    let rows:Vec<ProjectRecord>=sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT p.id,p.owner_id,p.revision,p.status,p.attempt_id,p.accepted_version,p.accepted_snapshot_id,p.document,p.instance_id FROM budget_projects p WHERE {filter} ORDER BY p.created_at DESC,p.id DESC LIMIT $10 OFFSET $11"))).bind(s.school).bind(s.own).bind(a.user_id).bind(&s.units).bind(s.assigned).bind(q.fiscal_year_id).bind(q.organization_unit_id).bind(q.status.map(|x|x.as_str())).bind(search).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    let items = project_summaries(&mut tx, &rows).await?;
    tx.commit().await?;
    Ok(BudgetProjectPage {
        items,
        total,
        limit,
        offset,
    })
}
pub(crate) async fn balances(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Vec<BudgetBalance>, AppError> {
    let accounts = repository::accounts(tx, id).await?;
    use std::collections::HashMap;
    let loans:Vec<(Uuid,Uuid,Uuid,i64)>=sqlx::query_as("SELECT (l.document->'from'->>'activityId')::uuid,(l.document->'from'->>'fundId')::uuid,(l.document->'from'->>'expenseCategoryId')::uuid,sum(l.amount-l.repaid)::bigint FROM budget_loans l JOIN budget_adjustments a ON a.id=l.id WHERE a.from_project_id=$1 AND l.repaid<l.amount GROUP BY 1,2,3").bind(id).fetch_all(&mut **tx).await?;
    let loans = loans
        .into_iter()
        .map(|(a, f, c, n)| ((a, f, c), n))
        .collect::<HashMap<_, _>>();
    Ok(accounts
        .into_iter()
        .map(|r| {
            let outstanding = loans
                .get(&(r.activity_id, r.fund_id, r.expense_category_id))
                .copied()
                .unwrap_or(0);
            BudgetBalance {
                project_id: r.project_id,
                activity_id: r.activity_id,
                fund_id: r.fund_id,
                expense_category_id: r.expense_category_id,
                planned_satang: r.planned.to_string(),
                adjusted_satang: r.adjusted.to_string(),
                current_satang: (r.planned + r.adjusted).to_string(),
                reserved_satang: r.reserved.to_string(),
                paid_satang: r.paid.to_string(),
                available_satang: r.available().to_string(),
                loan_outstanding_satang: outstanding.to_string(),
            }
        })
        .collect())
}
pub(crate) async fn require_request_read(
    pool: &PgPool,
    a: &ActorContext,
    r: &RequestRecord,
) -> Result<(), AppError> {
    let p = repository::project(pool, r.project_id).await?;
    let s = policy::read_scope(pool, a, false).await?;
    let assigned = if s.assigned {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM process_participants WHERE instance_id=$1 AND user_id=$2)",
        )
        .bind(r.instance_id)
        .bind(a.user_id)
        .fetch_one(pool)
        .await?
    } else {
        false
    };
    if policy::direct(
        &s,
        a.user_id,
        r.owner_id,
        p.document.project.organization_unit_id,
    ) || assigned
    {
        Ok(())
    } else {
        Err(repository::denied())
    }
}

pub(crate) async fn request_summary(
    tx: &mut Transaction<'_, Postgres>,
    r: &RequestRecord,
) -> Result<BudgetRequestSummary, AppError> {
    let (total,reserved,paid):(i64,i64,i64)=sqlx::query_as("SELECT coalesce(sum(amount),0)::bigint,coalesce(sum(reserved),0)::bigint,coalesce(sum(paid),0)::bigint FROM budget_request_lines WHERE request_id=$1").bind(r.id).fetch_one(&mut **tx).await?;
    request_summary_from(r, (total, reserved, paid))
}
fn request_summary_from(
    r: &RequestRecord,
    totals: (i64, i64, i64),
) -> Result<BudgetRequestSummary, AppError> {
    let (total, reserved, paid) = totals;
    Ok(BudgetRequestSummary {
        id: r.id,
        project_id: r.project_id,
        activity_id: r.activity_id,
        owner_id: r.owner_id,
        title: r.document.title.clone(),
        revision: r.revision,
        status: repository::parse_status(&r.status)?,
        total_satang: total.to_string(),
        reserved_satang: reserved.to_string(),
        paid_satang: paid.to_string(),
        remaining_satang: if r.status == "closed" {
            0
        } else {
            total - paid
        }
        .to_string(),
        created_at: r.created_at,
    })
}

pub async fn request_detail(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
) -> Result<BudgetSpendingRequestDetail, AppError> {
    let read_scope = policy::read_scope(pool, a, false).await?;
    let update_scope = policy::update_scope(pool, a).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let r:RequestRecord=sqlx::query_as("SELECT id,project_id,activity_id,owner_id,revision,status,document,instance_id,created_at FROM budget_requests WHERE id=$1").bind(id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("ไม่พบคำขอใช้งบประมาณ".into()))?;
    let p:ProjectRecord=sqlx::query_as("SELECT id,owner_id,revision,status,attempt_id,accepted_version,accepted_snapshot_id,document,instance_id FROM budget_projects WHERE id=$1").bind(r.project_id).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("ไม่พบโครงการ".into()))?;
    require_request_read_in_transaction(&mut tx, a, &r, &p, &read_scope).await?;
    let update = require_request_update_with_scope(a, &r, &p, &update_scope).is_ok();
    let lines = repository::request_lines(&mut tx, id).await?;
    let i = process::load(&mut tx, r.instance_id).await?;
    let mut capabilities = policy::capabilities(
        a,
        update && repository::parse_status(&r.status)?.editable(),
        policy::can_act(a, i.as_ref(), r.owner_id),
        true,
    );
    capabilities.can_pay = r.status == "approved" && capabilities.can_pay;
    capabilities.can_close = r.status == "approved" && capabilities.can_close;
    let request = request_summary(&mut tx, &r).await?;
    tx.commit().await?;
    Ok(BudgetSpendingRequestDetail {
        request,
        lines: lines
            .into_iter()
            .map(|l| BudgetRequestLine {
                plan_line_id: l.plan_line_id,
                description: l.description,
                unit: l.unit,
                fund_id: l.fund_id,
                fund_name: l.fund_name,
                expense_category_id: l.expense_category_id,
                expense_category_name: l.expense_category_name,
                amount_satang: l.amount.to_string(),
                reserved_satang: l.reserved.to_string(),
                paid_satang: l.paid.to_string(),
            })
            .collect(),
        evidence_file_ids: r.document.evidence_file_ids.clone(),
        capabilities,
    })
}
pub async fn requests(
    pool: &PgPool,
    a: &ActorContext,
    q: BudgetListQuery,
) -> Result<BudgetRequestPage, AppError> {
    let s = policy::read_scope(pool, a, false).await?;
    let (limit, offset) = money::paging(&q)?;
    let filter="($1 OR ($2 AND r.owner_id=$3) OR p.organization_unit_id=ANY($4) OR ($5 AND EXISTS(SELECT 1 FROM process_participants pp WHERE pp.instance_id=r.instance_id AND pp.user_id=$3))) AND ($6::uuid IS NULL OR r.project_id=$6) AND ($7::uuid IS NULL OR r.activity_id=$7) AND ($8::text IS NULL OR r.status=$8) AND ($9::uuid IS NULL OR p.fiscal_year_id=$9)";
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let total:i64=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM budget_requests r JOIN budget_projects p ON p.id=r.project_id WHERE {filter}"))).bind(s.school).bind(s.own).bind(a.user_id).bind(&s.units).bind(s.assigned).bind(q.project_id).bind(q.activity_id).bind(q.status.map(|x|x.as_str())).bind(q.fiscal_year_id).fetch_one(&mut *tx).await?;
    let rows:Vec<RequestRecord>=sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT r.id,r.project_id,r.activity_id,r.owner_id,r.revision,r.status,r.document,r.instance_id,r.created_at FROM budget_requests r JOIN budget_projects p ON p.id=r.project_id WHERE {filter} ORDER BY r.created_at DESC,r.id DESC LIMIT $10 OFFSET $11"))).bind(s.school).bind(s.own).bind(a.user_id).bind(&s.units).bind(s.assigned).bind(q.project_id).bind(q.activity_id).bind(q.status.map(|x|x.as_str())).bind(q.fiscal_year_id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    let items = request_summaries(&mut tx, &rows).await?;
    tx.commit().await?;
    Ok(BudgetRequestPage {
        items,
        total,
        limit,
        offset,
    })
}
pub async fn payments(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
    q: BudgetListQuery,
) -> Result<BudgetPaymentPage, AppError> {
    let r = repository::request(pool, id).await?;
    require_request_read(pool, a, &r).await?;
    let (limit, offset) = money::paging(&q)?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM budget_payments WHERE request_id=$1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let mut rows:Vec<BudgetPayment>=sqlx::query_scalar::<_,Json<BudgetPayment>>("SELECT document FROM budget_payments WHERE request_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3").bind(id).bind(limit).bind(offset).fetch_all(pool).await?.into_iter().map(|x|x.0).collect();
    let ids = rows.iter().map(|x| x.id).collect::<Vec<_>>();
    let refunds: Vec<BudgetRefund> = sqlx::query_scalar::<_, Json<BudgetRefund>>(
        "SELECT document FROM budget_refunds WHERE payment_id=ANY($1) ORDER BY created_at,id",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|x| x.0)
    .collect();
    for p in &mut rows {
        p.refunds = refunds
            .iter()
            .filter(|r| r.payment_id == p.id)
            .cloned()
            .collect();
    }
    Ok(BudgetPaymentPage {
        items: rows,
        total,
        limit,
        offset,
    })
}
pub async fn resource_process(
    pool: &PgPool,
    a: &ActorContext,
    kind: BudgetResourceKind,
    id: Uuid,
) -> Result<BudgetProcessDetail, AppError> {
    let instance = match kind {
        BudgetResourceKind::Project => {
            let r = repository::project(pool, id).await?;
            policy::require_read(pool, a, &r).await?;
            r.instance_id
        }
        BudgetResourceKind::Request => {
            let r = repository::request(pool, id).await?;
            require_request_read(pool, a, &r).await?;
            r.instance_id
        }
        BudgetResourceKind::Adjustment => {
            let r = crate::adjustments::record(pool, id).await?;
            crate::adjustments::require_read(pool, a, &r).await?;
            r.instance_id
        }
    };
    let mut tx = pool.begin().await?;
    let process = process::load(&mut tx, instance).await?;
    tx.commit().await?;
    Ok(BudgetProcessDetail { process })
}
pub async fn history(
    pool: &PgPool,
    a: &ActorContext,
    kind: BudgetResourceKind,
    id: Uuid,
    q: BudgetListQuery,
) -> Result<BudgetHistoryPage, AppError> {
    resource_process(pool, a, kind, id).await?;
    let (limit, offset) = money::paging(&q)?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM budget_history WHERE resource_kind=$1 AND resource_id=$2",
    )
    .bind(kind.as_str())
    .bind(id)
    .fetch_one(pool)
    .await?;
    let items=sqlx::query_scalar::<_,Json<BudgetHistoryEntry>>("SELECT document FROM budget_history WHERE resource_kind=$1 AND resource_id=$2 ORDER BY revision DESC,id DESC LIMIT $3 OFFSET $4").bind(kind.as_str()).bind(id).bind(limit).bind(offset).fetch_all(pool).await?.into_iter().map(|x|x.0).collect();
    Ok(BudgetHistoryPage {
        items,
        total,
        limit,
        offset,
    })
}
pub async fn dashboard(
    pool: &PgPool,
    a: &ActorContext,
    q: BudgetReportQuery,
    export: bool,
) -> Result<BudgetDashboard, AppError> {
    let s = policy::read_scope(pool, a, false).await?;
    let download = if export || q.export_scope.unwrap_or(false) {
        policy::read_scope(pool, a, true).await?
    } else {
        policy::Scope {
            school: true,
            ..Default::default()
        }
    };
    let mut download_predicate = PROJECT_SCOPE.to_owned();
    for i in 1..=5 {
        download_predicate = download_predicate.replace(&format!("${i}"), &format!("@scope{i}@"));
    }
    for i in 1..=5 {
        download_predicate =
            download_predicate.replace(&format!("@scope{i}@"), &format!("${}", i + 8));
    }
    let filter=format!("{PROJECT_SCOPE} AND {download_predicate} AND ($6::uuid IS NULL OR p.fiscal_year_id=$6) AND ($7::uuid IS NULL OR p.id=$7) AND ($8::uuid IS NULL OR p.organization_unit_id=$8)");
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let rows:Vec<Json<BudgetDashboardRow>>=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("WITH loan_totals AS (SELECT a.from_project_id AS project_id,(l.document->'from'->>'activityId')::uuid AS activity_id,(l.document->'from'->>'fundId')::uuid AS fund_id,(l.document->'from'->>'expenseCategoryId')::uuid AS expense_category_id,sum(l.amount-l.repaid)::bigint AS outstanding FROM budget_loans l JOIN budget_adjustments a ON a.id=l.id WHERE l.repaid<l.amount GROUP BY 1,2,3,4) SELECT jsonb_build_object('projectId',b.project_id,'projectName',s.document->'project'->>'name','activityId',b.activity_id,'activityTitle',coalesce((SELECT x->>'title' FROM jsonb_array_elements(s.document->'activities') x WHERE (x->>'id')::uuid=b.activity_id),(SELECT x->>'title' FROM budget_plan_snapshots old CROSS JOIN LATERAL jsonb_array_elements(old.document->'activities') x WHERE old.project_id=p.id AND (x->>'id')::uuid=b.activity_id ORDER BY old.version DESC LIMIT 1),'กิจกรรมเดิม'),'fundId',b.fund_id,'fundName',f.name,'expenseCategoryId',b.expense_category_id,'expenseCategoryName',c.name,'plannedSatang',b.planned::text,'adjustedSatang',b.adjusted::text,'currentSatang',(b.planned+b.adjusted)::text,'reservedSatang',b.reserved::text,'paidSatang',b.paid::text,'availableSatang',(b.planned+b.adjusted-b.reserved-b.paid)::text,'loanOutstandingSatang',coalesce(lt.outstanding,0)::text) FROM budget_allocations b JOIN budget_projects p ON p.id=b.project_id JOIN budget_plan_snapshots s ON s.id=p.accepted_snapshot_id JOIN budget_funds f ON f.id=b.fund_id JOIN budget_expense_categories c ON c.id=b.expense_category_id LEFT JOIN loan_totals lt ON lt.project_id=b.project_id AND lt.activity_id=b.activity_id AND lt.fund_id=b.fund_id AND lt.expense_category_id=b.expense_category_id WHERE {filter} ORDER BY p.id,b.activity_id,b.fund_id,b.expense_category_id LIMIT 10001"))).bind(s.school).bind(s.own).bind(a.user_id).bind(s.units).bind(s.assigned).bind(q.fiscal_year_id).bind(q.project_id).bind(q.organization_unit_id).bind(download.school).bind(download.own).bind(a.user_id).bind(download.units).bind(download.assigned).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    if rows.len() > 10000 {
        return Err(invalid("รายงานเกิน 10,000 แถว กรุณากรองปีงบประมาณหรือโครงการ"));
    }
    let rows = rows.into_iter().map(|x| x.0).collect::<Vec<_>>();
    let get = |f: fn(&BudgetDashboardRow) -> &String| -> Result<String, AppError> {
        let vals = rows
            .iter()
            .map(|r| {
                f(r).parse::<i64>()
                    .map_err(|_| AppError::InternalServerError("budget_invalid_balance".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(vals
            .into_iter()
            .try_fold(0_i64, |a, b| {
                a.checked_add(b)
                    .ok_or_else(|| invalid("ยอดรวมรายงานเกินขอบเขต"))
            })?
            .to_string())
    };
    let totals = BudgetDashboardTotals {
        planned_satang: get(|r| &r.planned_satang)?,
        adjusted_satang: get(|r| &r.adjusted_satang)?,
        current_satang: get(|r| &r.current_satang)?,
        reserved_satang: get(|r| &r.reserved_satang)?,
        paid_satang: get(|r| &r.paid_satang)?,
        available_satang: get(|r| &r.available_satang)?,
        loan_outstanding_satang: get(|r| &r.loan_outstanding_satang)?,
    };
    Ok(BudgetDashboard { totals, rows })
}
fn csv_text(s: &str) -> String {
    let s = if s
        .trim_start_matches([' ', '\t', '\r', '\n'])
        .starts_with(['=', '+', '-', '@'])
        || s.starts_with(['\t', '\r', '\n'])
    {
        format!("'{s}")
    } else {
        s.to_owned()
    };
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn baht(s: &str) -> Result<String, AppError> {
    let n = s
        .parse::<i64>()
        .map_err(|_| AppError::InternalServerError("budget_invalid_balance".into()))?;
    let abs = n.unsigned_abs();
    Ok(format!(
        "{}{}.{:02}",
        if n < 0 { "-" } else { "" },
        abs / 100,
        abs % 100
    ))
}
fn pivot_csv(data: BudgetDashboard, metric: BudgetReportMetric) -> Result<String, AppError> {
    use std::collections::BTreeMap;
    let mut columns = BTreeMap::<(Uuid, Uuid), (String, String)>::new();
    let mut rows = BTreeMap::<(Uuid, Uuid), (String, String, BTreeMap<(Uuid, Uuid), i64>)>::new();
    for r in data.rows {
        let col = (r.fund_id, r.expense_category_id);
        columns.insert(col, (r.fund_name.clone(), r.expense_category_name.clone()));
        let value = match metric {
            BudgetReportMetric::Planned => r.planned_satang,
            BudgetReportMetric::Current => r.current_satang,
            BudgetReportMetric::Paid => r.paid_satang,
        }
        .parse::<i64>()
        .map_err(|_| invalid("ยอดรายงานไม่ถูกต้อง"))?;
        let row = rows
            .entry((r.project_id, r.activity_id))
            .or_insert_with(|| (r.project_name, r.activity_title, BTreeMap::new()));
        let cell = row.2.entry(col).or_default();
        *cell = cell
            .checked_add(value)
            .ok_or_else(|| invalid("ยอดรวมรายงานเกินขอบเขต"))?;
    }
    let mut header = vec![csv_text("โครงการ"), csv_text("กิจกรรม")];
    header.extend(
        columns
            .values()
            .map(|(fund, category)| csv_text(&format!("{fund} / {category}"))),
    );
    header.push(csv_text("รวม"));
    let mut csv = format!("\u{feff}{}\r\n", header.join(","));
    let mut totals = BTreeMap::<(Uuid, Uuid), i64>::new();
    let mut grand = 0_i64;
    for (_, (project, activity, cells)) in rows {
        let mut row = vec![csv_text(&project), csv_text(&activity)];
        let mut total = 0_i64;
        for col in columns.keys() {
            let value = cells.get(col).copied().unwrap_or(0);
            row.push(baht(&value.to_string())?);
            total = total
                .checked_add(value)
                .ok_or_else(|| invalid("ยอดรวมรายงานเกินขอบเขต"))?;
            let sum = totals.entry(*col).or_default();
            *sum = sum
                .checked_add(value)
                .ok_or_else(|| invalid("ยอดรวมรายงานเกินขอบเขต"))?;
        }
        grand = grand
            .checked_add(total)
            .ok_or_else(|| invalid("ยอดรวมรายงานเกินขอบเขต"))?;
        row.push(baht(&total.to_string())?);
        csv.push_str(&row.join(","));
        csv.push_str("\r\n");
    }
    let mut footer = vec![csv_text("รวม"), csv_text("")];
    for col in columns.keys() {
        footer.push(baht(&totals.get(col).copied().unwrap_or(0).to_string())?);
    }
    footer.push(baht(&grand.to_string())?);
    csv.push_str(&footer.join(","));
    csv.push_str("\r\n");
    Ok(csv)
}
pub async fn export_csv(
    pool: &PgPool,
    a: &ActorContext,
    q: BudgetReportQuery,
) -> Result<String, AppError> {
    let metric = q.metric.unwrap_or_default();
    pivot_csv(dashboard(pool, a, q, true).await?, metric)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_cells_are_quoted_formula_safe_and_money_exact() {
        assert_eq!(csv_text("=1+1"), "\"'=1+1\"");
        assert_eq!(csv_text("a,\"b\""), "\"a,\"\"b\"\"\"");
        assert_eq!(baht("358800").unwrap(), "3588.00");
        assert_eq!(baht("-1").unwrap(), "-0.01");
    }
}

pub(crate) async fn require_request_read_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    a: &ActorContext,
    r: &RequestRecord,
    p: &ProjectRecord,
    s: &policy::Scope,
) -> Result<(), AppError> {
    if policy::direct(
        s,
        a.user_id,
        r.owner_id,
        p.document.project.organization_unit_id,
    ) {
        return Ok(());
    }
    if s.assigned
        && sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM process_participants WHERE instance_id=$1 AND user_id=$2)",
        )
        .bind(r.instance_id)
        .bind(a.user_id)
        .fetch_one(&mut **tx)
        .await?
    {
        return Ok(());
    }
    Err(repository::denied())
}
pub(crate) fn require_request_update_with_scope(
    a: &ActorContext,
    r: &RequestRecord,
    p: &ProjectRecord,
    s: &policy::Scope,
) -> Result<(), AppError> {
    if policy::direct(
        s,
        a.user_id,
        r.owner_id,
        p.document.project.organization_unit_id,
    ) {
        Ok(())
    } else {
        Err(repository::denied())
    }
}

async fn project_summaries(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[ProjectRecord],
) -> Result<Vec<BudgetProjectSummary>, AppError> {
    use std::collections::HashMap;
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let ids = rows.iter().map(|r| r.id).collect::<Vec<_>>();
    let units = rows
        .iter()
        .map(|r| r.document.project.organization_unit_id)
        .collect::<Vec<_>>();
    let names: HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,name FROM organization_units WHERE id=ANY($1)",
    )
    .bind(units)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    let sums:Vec<(Uuid,i64,i64,i64,i64)>=sqlx::query_as("SELECT project_id,sum(planned)::bigint,sum(reserved)::bigint,sum(paid)::bigint,sum(planned+adjusted-reserved-paid)::bigint FROM budget_allocations WHERE project_id=ANY($1) GROUP BY project_id").bind(ids).fetch_all(&mut **tx).await?;
    let sums = sums
        .into_iter()
        .map(|(id, p, r, s, a)| (id, (p, r, s, a)))
        .collect::<HashMap<_, _>>();
    rows.iter()
        .map(|r| {
            project_summary_from(
                r,
                names
                    .get(&r.document.project.organization_unit_id)
                    .cloned()
                    .ok_or_else(repository::stale)?,
                sums.get(&r.id).copied().unwrap_or_default(),
            )
        })
        .collect()
}

async fn request_summaries(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[RequestRecord],
) -> Result<Vec<BudgetRequestSummary>, AppError> {
    use std::collections::HashMap;
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let ids = rows.iter().map(|r| r.id).collect::<Vec<_>>();
    let sums:Vec<(Uuid,i64,i64,i64)>=sqlx::query_as("SELECT request_id,sum(amount)::bigint,sum(reserved)::bigint,sum(paid)::bigint FROM budget_request_lines WHERE request_id=ANY($1) GROUP BY request_id").bind(ids).fetch_all(&mut **tx).await?;
    let sums = sums
        .into_iter()
        .map(|(id, a, r, p)| (id, (a, r, p)))
        .collect::<HashMap<_, _>>();
    rows.iter()
        .map(|r| request_summary_from(r, sums.get(&r.id).copied().unwrap_or_default()))
        .collect()
}
