use crate::{
    models::*,
    money::{self, invalid, text},
    policy,
    ports::BudgetCatalogPort,
    process::{self, Source},
    queries,
    repository::{self, Delta, DraftPlan, ProjectRecord},
    settings,
};
use chrono::Utc;
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
pub async fn create(
    pool: &PgPool,
    a: &ActorContext,
    input: CreateBudgetProjectRequest,
) -> Result<BudgetProjectWorkspace, AppError> {
    policy::require_create_permission(a)?;
    let mut preflight = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "budget.projects.create.preflight",
    )
    .await?;
    policy::require_create_unit(&mut preflight, a, input.organization_unit_id).await?;
    let read = policy::read_scope_on_connection(&mut preflight, a, false).await?;
    if !policy::direct(&read, a.user_id, a.user_id, input.organization_unit_id) {
        return Err(repository::denied());
    }
    drop(preflight);
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "budget.projects.create").await?;
    let id = Uuid::now_v7();
    if let Some(old) = repository::claim(
        &mut tx,
        a.user_id,
        input.request_id,
        "project_create",
        id,
        &input,
    )
    .await?
    {
        tx.commit().await?;
        return queries::workspace(pool, a, old).await;
    }
    let p = BudgetProjectInput {
        name: text(&input.name, 250)?,
        fiscal_year_id: input.fiscal_year_id,
        organization_unit_id: input.organization_unit_id,
        start_date: input.start_date,
        end_date: input.end_date,
    };
    settings::validate_project(&mut tx, &p, false).await?;
    let r = ProjectRecord {
        id,
        owner_id: a.user_id,
        revision: 1,
        status: "draft".into(),
        attempt_id: Uuid::now_v7(),
        accepted_version: None,
        accepted_snapshot_id: None,
        document: Json(DraftPlan {
            project: p,
            activities: vec![],
        }),
        instance_id: None,
    };
    DbSession::new(&mut *tx).query("budget.src.projects.create", "INSERT INTO budget_projects(id,owner_id,fiscal_year_id,organization_unit_id,revision,status,attempt_id,document) VALUES($1,$2,$3,$4,1,'draft',$5,$6)").bind(id).bind(a.user_id).bind(r.document.project.fiscal_year_id).bind(r.document.project.organization_unit_id).bind(r.attempt_id).bind(&r.document).execute().await?;
    DbSession::new(&mut *tx)
        .query(
            "budget.src.projects.create",
            "INSERT INTO budget_project_attempts(id,project_id) VALUES($1,$2)",
        )
        .bind(r.attempt_id)
        .bind(id)
        .execute()
        .await?;
    repository::store_project(&mut tx, &r).await?;
    repository::history(
        &mut tx,
        a,
        BudgetResourceKind::Project,
        id,
        1,
        "created",
        None,
    )
    .await?;
    tx.commit().await?;
    queries::workspace(pool, a, id).await
}
fn preserved_snapshot(old: Option<&BudgetPlanLine>, line: &BudgetPlanLineInput) -> bool {
    old.is_some_and(|o| {
        o.supply_item_id == line.supply_item_id
            && o.supply_version_id == line.supply_version_id
            && o.unit_price_satang == line.unit_price_satang
            && o.unit == line.unit
            && o.description == line.description
    })
}
async fn activities(
    tx: &mut Transaction<'_, Postgres>,
    a: &ActorContext,
    r: &ProjectRecord,
    input: Vec<BudgetActivityInput>,
    catalog: &dyn BudgetCatalogPort,
) -> Result<Vec<BudgetActivity>, AppError> {
    if input.len() > 100 || input.iter().map(|x| x.lines.len()).sum::<usize>() > 1000 {
        return Err(invalid("โครงการรองรับไม่เกิน 100 กิจกรรมและ 1,000 รายการ"));
    }
    let existing_activities: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "budget.src.projects.activities",
            "SELECT id FROM budget_activities WHERE project_id=$1",
        )
        .bind(r.id)
        .fetch_all()
        .await?;
    let existing_lines: Vec<(Uuid, Uuid)> = DbSession::new(&mut **tx)
        .query_as(
            "budget.src.projects.activities",
            "SELECT id,activity_id FROM budget_plan_lines WHERE project_id=$1",
        )
        .bind(r.id)
        .fetch_all()
        .await?;
    let mut seen = BTreeSet::new();
    let mut seen_lines = BTreeSet::new();
    let mut result = vec![];
    let funds: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "budget.src.projects.activities",
            "SELECT id FROM budget_funds WHERE active",
        )
        .fetch_all()
        .await?;
    let cats: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "budget.src.projects.activities",
            "SELECT id FROM budget_expense_categories WHERE active",
        )
        .fetch_all()
        .await?;
    for item in input {
        let id = item.id.unwrap_or_else(Uuid::now_v7);
        if !seen.insert(id) || item.id.is_some() && !existing_activities.contains(&id) {
            return Err(invalid("กิจกรรมซ้ำหรือไม่อยู่ในโครงการนี้"));
        }
        let title = text(&item.title, 250)?;
        let old_activity = r.document.activities.iter().find(|x| x.id == id);
        if item.default_fund_id.is_some_and(|id| !funds.contains(&id))
            && !old_activity.is_some_and(|o| o.default_fund_id == item.default_fund_id)
            || item
                .default_expense_category_id
                .is_some_and(|id| !cats.contains(&id))
                && !old_activity.is_some_and(|o| {
                    o.default_expense_category_id == item.default_expense_category_id
                })
        {
            return Err(invalid("หมวดเริ่มต้นของกิจกรรมถูกปิดใช้งาน"));
        }
        DbSession::new(&mut **tx).query("budget.src.projects.activities",
            "INSERT INTO budget_activities(id,project_id) VALUES($1,$2) ON CONFLICT(id) DO NOTHING",
        )
        .bind(id)
        .bind(r.id)
        .execute()
        .await?;
        let mut lines = vec![];
        for l in item.lines {
            let line_id = l.id.unwrap_or_else(Uuid::now_v7);
            if !seen_lines.insert(line_id)
                || l.id.is_some() && !existing_lines.contains(&(line_id, id))
            {
                return Err(invalid("รายการซ้ำหรือไม่อยู่ในกิจกรรมนี้"));
            }
            let old = r
                .document
                .activities
                .iter()
                .flat_map(|x| &x.lines)
                .find(|x| x.id == line_id);
            let unchanged = preserved_snapshot(old, &l);
            if (!funds.contains(&l.fund_id) || !cats.contains(&l.expense_category_id))
                && !old.is_some_and(|o| {
                    o.fund_id == l.fund_id && o.expense_category_id == l.expense_category_id
                })
            {
                return Err(invalid("แหล่งเงินหรือหมวดรายจ่ายถูกปิดใช้งาน"));
            }
            let mut line = BudgetPlanLine {
                id: line_id,
                description: text(&l.description, 500)?,
                quantity: l.quantity.clone(),
                unit: text(&l.unit, 80)?,
                unit_price_satang: money::parse_money(&l.unit_price_satang)?.to_string(),
                total_satang: String::new(),
                fund_id: l.fund_id,
                expense_category_id: l.expense_category_id,
                supply_item_id: l.supply_item_id,
                supply_version_id: l.supply_version_id,
                supply_version_revision: None,
                supply_name_snapshot: None,
            };
            match (l.supply_item_id, l.supply_version_id) {
                (Some(item), Some(version)) => {
                    if unchanged {
                        if let Some(o) = old {
                            line.supply_version_revision = o.supply_version_revision;
                            line.supply_name_snapshot = o.supply_name_snapshot.clone();
                        }
                    } else {
                        let snap = catalog.snapshot(tx, a, item, version).await?;
                        line.description = snap.name.clone();
                        line.unit = snap.unit;
                        line.unit_price_satang = snap.price_satang.to_string();
                        line.supply_version_revision = Some(snap.revision);
                        line.supply_name_snapshot = Some(snap.name);
                    }
                }
                (None, None) => {}
                _ => return Err(invalid("ต้องระบุวัสดุและรุ่นราคาคู่กัน")),
            }
            line.total_satang =
                money::line_total(&line.quantity, &line.unit_price_satang)?.to_string();
            DbSession::new(&mut **tx).query("budget.src.projects.activities", "INSERT INTO budget_plan_lines(id,project_id,activity_id) VALUES($1,$2,$3) ON CONFLICT(id) DO NOTHING").bind(line_id).bind(r.id).bind(id).execute().await?;
            lines.push(line);
        }
        let total = money::sum(
            lines
                .iter()
                .map(|x| money::parse_money(&x.total_satang))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        result.push(BudgetActivity {
            id,
            title,
            default_fund_id: item.default_fund_id,
            default_expense_category_id: item.default_expense_category_id,
            total_satang: total.to_string(),
            lines,
        });
    }
    money::sum(
        result
            .iter()
            .map(|x| money::parse_money(&x.total_satang))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    Ok(result)
}
pub async fn save(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
    input: SaveBudgetProjectRequest,
    catalog: &dyn BudgetCatalogPort,
) -> Result<BudgetProjectWorkspace, AppError> {
    let mut preflight =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "budget.projects.save.preflight")
            .await?;
    let old = repository::project_on_connection(&mut preflight, id).await?;
    policy::require_read(&mut preflight, a, &old).await?;
    policy::require_update(&mut preflight, a, &old).await?;
    let read_scope = policy::read_scope_on_connection(&mut preflight, a, false).await?;
    let update_scope = policy::update_scope_on_connection(&mut preflight, a).await?;
    if input.organization_unit_id != old.document.project.organization_unit_id {
        policy::require_create_unit(&mut preflight, a, input.organization_unit_id).await?;
    }
    drop(preflight);
    let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "budget.projects.save").await?;
    let mut r = repository::lock_project(&mut tx, id).await?;
    policy::require_read_in_transaction(&mut tx, a, &r, &read_scope).await?;
    policy::require_update_with_scope(a, &r, &update_scope)?;
    if repository::claim(
        &mut tx,
        a.user_id,
        input.request_id,
        "project_save",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return queries::workspace(pool, a, id).await;
    }
    repository::revision(r.revision, input.expected_revision)?;
    if !r.status()?.editable() {
        return Err(repository::stale());
    }
    let p = BudgetProjectInput {
        name: text(&input.name, 250)?,
        fiscal_year_id: input.fiscal_year_id,
        organization_unit_id: input.organization_unit_id,
        start_date: input.start_date,
        end_date: input.end_date,
    };
    let mut destination = r.clone();
    destination.document.project.organization_unit_id = p.organization_unit_id;
    policy::require_read_in_transaction(&mut tx, a, &destination, &read_scope).await?;
    if r.accepted_version.is_some()
        && (p.fiscal_year_id != r.document.project.fiscal_year_id
            || p.organization_unit_id != r.document.project.organization_unit_id)
    {
        return Err(invalid("แผนที่อนุมัติแล้วไม่สามารถเปลี่ยนปีงบประมาณหรือหน่วยงาน"));
    }
    settings::validate_project(
        &mut tx,
        &p,
        p.fiscal_year_id == r.document.project.fiscal_year_id,
    )
    .await?;
    let items = activities(&mut tx, a, &r, input.activities, catalog).await?;
    r.document = Json(DraftPlan {
        project: p,
        activities: items,
    });
    r.revision += 1;
    repository::store_project(&mut tx, &r).await?;
    repository::history(
        &mut tx,
        a,
        BudgetResourceKind::Project,
        id,
        r.revision,
        "saved",
        None,
    )
    .await?;
    tx.commit().await?;
    queries::workspace(pool, a, id).await
}
pub async fn submit(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
    input: BudgetMutationRequest,
) -> Result<BudgetProjectWorkspace, AppError> {
    let mut preflight = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "budget.projects.submit.preflight",
    )
    .await?;
    let old = repository::project_on_connection(&mut preflight, id).await?;
    policy::require_read(&mut preflight, a, &old).await?;
    policy::require_update(&mut preflight, a, &old).await?;
    let read_scope = policy::read_scope_on_connection(&mut preflight, a, false).await?;
    let update_scope = policy::update_scope_on_connection(&mut preflight, a).await?;
    drop(preflight);
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "budget.projects.submit").await?;
    let mut r = repository::lock_project(&mut tx, id).await?;
    policy::require_read_in_transaction(&mut tx, a, &r, &read_scope).await?;
    policy::require_update_with_scope(a, &r, &update_scope)?;
    if repository::claim(
        &mut tx,
        a.user_id,
        input.request_id,
        "project_submit",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return queries::workspace(pool, a, id).await;
    }
    repository::revision(r.revision, input.expected_revision)?;
    if !r.status()?.editable()
        || r.document.activities.is_empty()
        || r.document.activities.iter().any(|x| x.lines.is_empty())
    {
        return Err(invalid("กรุณาบันทึกกิจกรรมและรายการงบประมาณก่อนส่งแผน"));
    }
    let route = settings::routing(&mut tx).await?;
    settings::validate_project(&mut tx, &r.document.project, true).await?;
    r.status = "submitted".into();
    r.revision += 1;
    let source = Source {
        resource_type: "budget_project_plan",
        resource_id: r.attempt_id,
        owner_id: r.owner_id,
        actor_id: a.user_id,
    };
    let i = process::start(
        &mut tx,
        a,
        &source,
        input.request_id,
        route.plan_template_id,
        r.document.project.name.clone(),
        format!("/staff/budget/projects/{id}"),
    )
    .await?;
    r.instance_id = Some(i.id);
    repository::store_project(&mut tx, &r).await?;
    repository::history(
        &mut tx,
        a,
        BudgetResourceKind::Project,
        id,
        r.revision,
        "submitted",
        None,
    )
    .await?;
    tx.commit().await?;
    queries::workspace(pool, a, id).await
}
async fn accept(
    tx: &mut Transaction<'_, Postgres>,
    a: &ActorContext,
    r: &mut ProjectRecord,
) -> Result<(), AppError> {
    let mut planned = BTreeMap::<(Uuid, Uuid, Uuid), i64>::new();
    let mut line_dimensions = BTreeMap::new();
    for activity in &r.document.activities {
        for l in &activity.lines {
            let key = (activity.id, l.fund_id, l.expense_category_id);
            let v = planned.entry(key).or_default();
            *v = money::sum([*v, money::parse_money(&l.total_satang)?])?;
            line_dimensions.insert(l.id, key);
        }
    }
    // Every financially referenced line keeps its activity/fund/category, including closed and refunded history.
    let refs:Vec<(Uuid,Uuid,Uuid,Uuid)>=DbSession::new(&mut **tx).query_as("budget.src.projects.accept", "SELECT DISTINCT l.plan_line_id,r.activity_id,l.fund_id,l.expense_category_id FROM budget_request_lines l JOIN budget_requests r ON r.id=l.request_id WHERE r.project_id=$1 AND (r.status IN('approved','closed') OR EXISTS(SELECT 1 FROM budget_payment_lines p WHERE p.request_id=r.id))").bind(r.id).fetch_all().await?;
    for (line, activity, fund, cat) in refs {
        if line_dimensions.get(&line) != Some(&(activity, fund, cat)) {
            return Err(invalid(
                "แผนแก้ไขไม่สามารถลบหรือเปลี่ยนหมวดรายการที่มีการผูกพันหรือจ่ายเงินแล้ว",
            ));
        }
    }
    let existing = repository::accounts(tx, r.id).await?;
    for x in &existing {
        let value = planned
            .remove(&(x.activity_id, x.fund_id, x.expense_category_id))
            .unwrap_or(0);
        if value + x.adjusted < x.reserved + x.paid {
            return Err(invalid("แผนแก้ไขต่ำกว่ายอดผูกพันหรือจ่ายจริง"));
        }
        repository::post(
            tx,
            a.user_id,
            &x.reference(),
            Delta {
                planned: value - x.planned,
                ..Default::default()
            },
            "plan",
            r.attempt_id,
        )
        .await?;
    }
    for ((activity, fund, cat), value) in planned {
        DbSession::new(&mut **tx).query("budget.src.projects.accept", "INSERT INTO budget_allocations(project_id,activity_id,fund_id,expense_category_id) VALUES($1,$2,$3,$4)").bind(r.id).bind(activity).bind(fund).bind(cat).execute().await?;
        repository::post(
            tx,
            a.user_id,
            &BudgetAllocationRef {
                project_id: r.id,
                activity_id: activity,
                fund_id: fund,
                expense_category_id: cat,
            },
            Delta {
                planned: value,
                ..Default::default()
            },
            "plan",
            r.attempt_id,
        )
        .await?;
    }
    let total = money::sum(
        r.document
            .activities
            .iter()
            .map(|x| money::parse_money(&x.total_satang))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let snapshot = BudgetPlanSnapshot {
        id: r.attempt_id,
        version: r.accepted_version.unwrap_or(0) + 1,
        project: r.document.project.clone(),
        activities: r.document.activities.clone(),
        total_satang: total.to_string(),
        accepted_at: Utc::now(),
        accepted_by: a.user_id,
    };
    DbSession::new(&mut **tx).query("budget.src.projects.accept", "INSERT INTO budget_plan_snapshots(id,project_id,version,document,accepted_by,accepted_at) VALUES($1,$2,$3,$4,$5,$6)").bind(snapshot.id).bind(r.id).bind(snapshot.version).bind(Json(&snapshot)).bind(a.user_id).bind(snapshot.accepted_at).execute().await?;
    r.accepted_version = Some(snapshot.version);
    r.accepted_snapshot_id = Some(snapshot.id);
    Ok(())
}
pub async fn act(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
    input: BudgetActionRequest,
) -> Result<BudgetProjectWorkspace, AppError> {
    let mut preflight =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "budget.projects.act.preflight")
            .await?;
    let old = repository::project_on_connection(&mut preflight, id).await?;
    policy::require_read(&mut preflight, a, &old).await?;
    let read_scope = policy::read_scope_on_connection(&mut preflight, a, false).await?;
    drop(preflight);
    let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "budget.projects.act").await?;
    let mut r = repository::lock_project(&mut tx, id).await?;
    policy::require_read_in_transaction(&mut tx, a, &r, &read_scope).await?;
    if repository::claim(
        &mut tx,
        a.user_id,
        input.request_id,
        "project_action",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return queries::workspace(pool, a, id).await;
    }
    repository::revision(r.revision, input.expected_revision)?;
    if r.status()? != BudgetStatus::Submitted {
        return Err(repository::stale());
    }
    let source = Source {
        resource_type: "budget_project_plan",
        resource_id: r.attempt_id,
        owner_id: r.owner_id,
        actor_id: a.user_id,
    };
    let i = process::act(&mut tx, a, &source, r.instance_id, &input).await?;
    let status = process::status(&i);
    if status == BudgetStatus::Approved {
        accept(&mut tx, a, &mut r).await?;
    }
    r.status = status.as_str().into();
    r.revision += 1;
    repository::store_project(&mut tx, &r).await?;
    repository::history(
        &mut tx,
        a,
        BudgetResourceKind::Project,
        id,
        r.revision,
        process::history_action(input.action, status),
        input.comment,
    )
    .await?;
    tx.commit().await?;
    queries::workspace(pool, a, id).await
}
pub async fn amend(
    pool: &PgPool,
    a: &ActorContext,
    id: Uuid,
    input: StartBudgetAmendmentRequest,
) -> Result<BudgetProjectWorkspace, AppError> {
    let mut preflight =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "budget.projects.amend.preflight")
            .await?;
    let old = repository::project_on_connection(&mut preflight, id).await?;
    policy::require_read(&mut preflight, a, &old).await?;
    policy::require_update(&mut preflight, a, &old).await?;
    let reason = text(&input.reason, 4000)?;
    let read_scope = policy::read_scope_on_connection(&mut preflight, a, false).await?;
    let update_scope = policy::update_scope_on_connection(&mut preflight, a).await?;
    drop(preflight);
    let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "budget.projects.amend").await?;
    let mut r = repository::lock_project(&mut tx, id).await?;
    policy::require_read_in_transaction(&mut tx, a, &r, &read_scope).await?;
    policy::require_update_with_scope(a, &r, &update_scope)?;
    if repository::claim(
        &mut tx,
        a.user_id,
        input.request_id,
        "project_amend",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return queries::workspace(pool, a, id).await;
    }
    repository::revision(r.revision, input.expected_revision)?;
    if r.accepted_version.is_none()
        || !matches!(r.status()?, BudgetStatus::Approved | BudgetStatus::Rejected)
    {
        return Err(repository::stale());
    }
    let plan = repository::accepted(&mut tx, &r).await?;
    r.document = Json(DraftPlan {
        project: plan.project,
        activities: plan.activities,
    });
    r.status = "draft".into();
    r.attempt_id = Uuid::now_v7();
    DbSession::new(&mut *tx)
        .query(
            "budget.src.projects.amend",
            "INSERT INTO budget_project_attempts(id,project_id) VALUES($1,$2)",
        )
        .bind(r.attempt_id)
        .bind(id)
        .execute()
        .await?;
    r.instance_id = None;
    r.revision += 1;
    repository::store_project(&mut tx, &r).await?;
    repository::history(
        &mut tx,
        a,
        BudgetResourceKind::Project,
        id,
        r.revision,
        "amendment_started",
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    queries::workspace(pool, a, id).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_catalog_line_preserves_snapshot() {
        let l = BudgetPlanLineInput {
            id: None,
            description: "paper".into(),
            quantity: "2".into(),
            unit: "pack".into(),
            unit_price_satang: "123".into(),
            fund_id: Uuid::now_v7(),
            expense_category_id: Uuid::now_v7(),
            supply_item_id: Some(Uuid::now_v7()),
            supply_version_id: Some(Uuid::now_v7()),
        };
        let old = BudgetPlanLine {
            id: Uuid::now_v7(),
            description: l.description.clone(),
            quantity: "1".into(),
            unit: l.unit.clone(),
            unit_price_satang: l.unit_price_satang.clone(),
            total_satang: "123".into(),
            fund_id: l.fund_id,
            expense_category_id: l.expense_category_id,
            supply_item_id: l.supply_item_id,
            supply_version_id: l.supply_version_id,
            supply_version_revision: Some(2),
            supply_name_snapshot: Some("paper".into()),
        };
        assert!(preserved_snapshot(Some(&old), &l));
    }
}
