use crate::{
    models::*,
    money::{invalid, text},
    repository,
};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::{
    process,
    process_models::{ProcessStepKind, ProcessTemplateStep},
};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Routing {
    pub plan_steps: Vec<ProcessTemplateStep>,
    pub request_steps: Vec<ProcessTemplateStep>,
    pub adjustment_steps: Vec<ProcessTemplateStep>,
}
#[derive(sqlx::FromRow)]
pub(crate) struct SettingsRow {
    pub revision: i64,
    pub routing: Json<Routing>,
    pub plan_template_id: Option<Uuid>,
    pub request_template_id: Option<Uuid>,
    pub adjustment_template_id: Option<Uuid>,
}
pub(crate) fn require_catalogs(a: &ActorContext) -> Result<(), AppError> {
    a.require_any_permission(&[
        codes::BUDGET_READ_OWN,
        codes::BUDGET_READ_ASSIGNED,
        codes::BUDGET_READ_ORGANIZATION_UNIT,
        codes::BUDGET_READ_ORGANIZATION_TREE,
        codes::BUDGET_READ_SCHOOL,
        codes::BUDGET_CREATE_OWN,
        codes::BUDGET_MANAGE_SCHOOL,
        codes::BUDGET_PAYMENT_EXECUTE_SCHOOL,
        codes::BUDGET_ADJUSTMENT_CREATE_SCHOOL,
        codes::BUDGET_DOWNLOAD_OWN,
        codes::BUDGET_DOWNLOAD_ORGANIZATION_UNIT,
        codes::BUDGET_DOWNLOAD_ORGANIZATION_TREE,
        codes::BUDGET_DOWNLOAD_SCHOOL,
    ])
}
pub async fn catalogs(pool: &PgPool, a: &ActorContext) -> Result<BudgetCatalogs, AppError> {
    require_catalogs(a)?;
    let mut preflight = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "budget.settings.catalogs.preflight",
    )
    .await?;
    catalogs_read_on_connection(&mut preflight, true).await
}

async fn catalogs_read_on_connection(
    connection: &mut sqlx::PgConnection,
    all: bool,
) -> Result<BudgetCatalogs, AppError> {
    let result:Json<BudgetCatalogs>=DbSession::new(&mut *connection).scalar("budget.src.settings.catalogs_read_on_connection", "SELECT jsonb_build_object('fiscalYears',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'startDate',start_date,'endDate',end_date,'active',active) ORDER BY start_date DESC,id),'[]') FROM budget_fiscal_years WHERE $1 OR active),'funds',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'active',active) ORDER BY name,id),'[]') FROM budget_funds WHERE $1 OR active),'expenseCategories',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'active',active) ORDER BY name,id),'[]') FROM budget_expense_categories WHERE $1 OR active))").bind(all).fetch_one().await?;
    Ok(result.0)
}
pub async fn get(pool: &PgPool, a: &ActorContext) -> Result<BudgetSettings, AppError> {
    a.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "budget.settings.get").await?;
    DbSession::new(&mut *tx)
        .query(
            "budget.src.settings.get",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let r:SettingsRow=DbSession::new(&mut *tx).query_as("budget.src.settings.get", "SELECT revision,routing,plan_template_id,request_template_id,adjustment_template_id FROM budget_settings WHERE singleton").fetch_one().await?;
    let c:Json<BudgetCatalogs>=DbSession::new(&mut *tx).scalar("budget.src.settings.get", "SELECT jsonb_build_object('fiscalYears',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'startDate',start_date,'endDate',end_date,'active',active) ORDER BY start_date DESC,id),'[]') FROM budget_fiscal_years),'funds',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'active',active) ORDER BY name,id),'[]') FROM budget_funds),'expenseCategories',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'active',active) ORDER BY name,id),'[]') FROM budget_expense_categories))").fetch_one().await?;
    tx.commit().await?;
    Ok(BudgetSettings {
        revision: r.revision,
        fiscal_years: c.fiscal_years.clone(),
        funds: c.funds.clone(),
        expense_categories: c.expense_categories.clone(),
        plan_steps: r.routing.plan_steps.clone(),
        request_steps: r.routing.request_steps.clone(),
        adjustment_steps: r.routing.adjustment_steps.clone(),
    })
}
pub fn validate_steps(steps: &[ProcessTemplateStep]) -> Result<(), AppError> {
    if steps.is_empty() {
        return Ok(());
    }
    process::validate_steps(steps)?;
    if steps.len() > 10
        || steps.iter().any(|s| {
            s.recipients.is_empty()
                || s.requires_signature
                || s.requires_evidence
                || s.requires_delegated_authority
                || !matches!(s.kind, ProcessStepKind::Review | ProcessStepKind::Approve)
        })
        || steps
            .last()
            .is_none_or(|s| s.kind != ProcessStepKind::Approve)
    {
        return Err(invalid(
            "เส้นทางต้องเป็นตรวจสอบ/อนุมัติและจบด้วยอนุมัติ โดยไม่ใช้ลายเซ็น หลักฐาน หรือมอบอำนาจรายขั้นตอน",
        ));
    }
    Ok(())
}
fn validate(input: &SaveBudgetSettingsRequest) -> Result<(), AppError> {
    if input.fiscal_years.len() > 100
        || input.funds.len() > 100
        || input.expense_categories.len() > 100
    {
        return Err(invalid("ตั้งค่าแต่ละประเภทได้ไม่เกิน 100 รายการ"));
    }
    for ids in [
        input
            .fiscal_years
            .iter()
            .filter_map(|r| r.id)
            .collect::<Vec<_>>(),
        input.funds.iter().filter_map(|r| r.id).collect(),
        input
            .expense_categories
            .iter()
            .filter_map(|r| r.id)
            .collect(),
    ] {
        if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(invalid("รหัสรายการตั้งค่าซ้ำ"));
        }
    }
    for y in &input.fiscal_years {
        text(&y.name, 120)?;
        if y.end_date < y.start_date || (y.end_date - y.start_date).num_days() > 730 {
            return Err(invalid("ช่วงปีงบประมาณไม่ถูกต้อง"));
        }
    }
    for c in input.funds.iter().chain(&input.expense_categories) {
        text(&c.name, 120)?;
    }
    for s in [
        &input.plan_steps,
        &input.request_steps,
        &input.adjustment_steps,
    ] {
        validate_steps(s)?;
    }
    Ok(())
}
pub async fn save(
    pool: &PgPool,
    a: &ActorContext,
    input: SaveBudgetSettingsRequest,
) -> Result<BudgetSettings, AppError> {
    a.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;
    validate(&input)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "budget.settings.save").await?;
    if repository::claim(
        &mut tx,
        a.user_id,
        input.request_id,
        "settings_save",
        Uuid::nil(),
        &input,
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return get(pool, a).await;
    }
    let old:SettingsRow=DbSession::new(&mut *tx).query_as("budget.src.settings.save", "SELECT revision,routing,plan_template_id,request_template_id,adjustment_template_id FROM budget_settings WHERE singleton FOR UPDATE").fetch_one().await?;
    repository::revision(old.revision, input.expected_revision)?;
    let fiscal_years = input
        .fiscal_years
        .iter()
        .map(|x| BudgetFiscalYear {
            id: x.id.unwrap_or_else(Uuid::now_v7),
            name: x.name.clone(),
            start_date: x.start_date,
            end_date: x.end_date,
            active: x.active,
        })
        .collect::<Vec<_>>();
    let funds = input
        .funds
        .iter()
        .map(|x| BudgetCatalogItem {
            id: x.id.unwrap_or_else(Uuid::now_v7),
            name: x.name.clone(),
            active: x.active,
        })
        .collect::<Vec<_>>();
    let expense_categories = input
        .expense_categories
        .iter()
        .map(|x| BudgetCatalogItem {
            id: x.id.unwrap_or_else(Uuid::now_v7),
            name: x.name.clone(),
            active: x.active,
        })
        .collect::<Vec<_>>();
    for (table, ids) in [
        (
            "budget_fiscal_years",
            fiscal_years.iter().map(|x| x.id).collect::<Vec<_>>(),
        ),
        ("budget_funds", funds.iter().map(|x| x.id).collect()),
        (
            "budget_expense_categories",
            expense_categories.iter().map(|x| x.id).collect(),
        ),
    ] {
        let existing: Vec<Uuid> = DbSession::new(&mut *tx)
            .scalar(
                "budget.src.settings.save",
                sqlx::AssertSqlSafe(format!("SELECT id FROM {table}")),
            )
            .fetch_all()
            .await?;
        if ids
            .iter()
            .any(|id| !existing.contains(id) && id.get_version_num() != 7)
        {
            return Err(invalid("รายการใหม่ต้องใช้รหัส UUIDv7 ของระบบ"));
        }
        if existing.iter().chain(&ids).collect::<BTreeSet<_>>().len() > 500 {
            return Err(invalid("ตั้งค่าแต่ละประเภทรวมประวัติได้ไม่เกิน 500 รายการ"));
        }
    }

    for y in &fiscal_years {
        let incompatible:bool=DbSession::new(&mut *tx).scalar("budget.src.settings.save", "SELECT EXISTS(SELECT 1 FROM budget_projects WHERE fiscal_year_id=$1 AND (((document->'project'->>'startDate')::date<$2 OR (document->'project'->>'endDate')::date>$3) OR EXISTS(SELECT 1 FROM budget_plan_snapshots s WHERE s.project_id=budget_projects.id AND ((s.document->'project'->>'startDate')::date<$2 OR (s.document->'project'->>'endDate')::date>$3))))").bind(y.id).bind(y.start_date).bind(y.end_date).fetch_one().await?;
        if incompatible {
            return Err(invalid("ช่วงปีงบประมาณตัดวันที่โครงการที่บันทึกไว้"));
        }
    }
    let ids = fiscal_years.iter().map(|x| x.id).collect::<Vec<_>>();
    let names = fiscal_years
        .iter()
        .map(|x| x.name.trim().to_owned())
        .collect::<Vec<_>>();
    let starts = fiscal_years
        .iter()
        .map(|x| x.start_date)
        .collect::<Vec<_>>();
    let ends = fiscal_years.iter().map(|x| x.end_date).collect::<Vec<_>>();
    let active = fiscal_years.iter().map(|x| x.active).collect::<Vec<_>>();
    DbSession::new(&mut *tx)
        .query(
            "budget.src.settings.save",
            "UPDATE budget_fiscal_years SET active=false WHERE NOT(id=ANY($1))",
        )
        .bind(&ids)
        .execute()
        .await?;
    DbSession::new(&mut *tx).query("budget.src.settings.save", "INSERT INTO budget_fiscal_years(id,name,start_date,end_date,active) SELECT * FROM unnest($1::uuid[],$2::text[],$3::date[],$4::date[],$5::bool[]) ON CONFLICT(id) DO UPDATE SET name=excluded.name,start_date=excluded.start_date,end_date=excluded.end_date,active=excluded.active").bind(ids).bind(names).bind(starts).bind(ends).bind(active).execute().await?;
    for (table, items) in [
        ("budget_funds", &funds),
        ("budget_expense_categories", &expense_categories),
    ] {
        let ids = items.iter().map(|x| x.id).collect::<Vec<_>>();
        let names = items
            .iter()
            .map(|x| x.name.trim().to_owned())
            .collect::<Vec<_>>();
        let active = items.iter().map(|x| x.active).collect::<Vec<_>>();
        DbSession::new(&mut *tx)
            .query(
                "budget.src.settings.save",
                sqlx::AssertSqlSafe(format!(
                    "UPDATE {table} SET active=false WHERE NOT(id=ANY($1))"
                )),
            )
            .bind(&ids)
            .execute()
            .await?;
        DbSession::new(&mut *tx).query("budget.src.settings.save", sqlx::AssertSqlSafe(format!("INSERT INTO {table}(id,name,active) SELECT * FROM unnest($1::uuid[],$2::text[],$3::bool[]) ON CONFLICT(id) DO UPDATE SET name=excluded.name,active=excluded.active"))).bind(ids).bind(names).bind(active).execute().await?;
    }
    let routing = Routing {
        plan_steps: input.plan_steps,
        request_steps: input.request_steps,
        adjustment_steps: input.adjustment_steps,
    };
    let mut templates = vec![];
    for (title, steps) in [
        ("อนุมัติแผนงบประมาณ", &routing.plan_steps),
        ("อนุมัติคำขอใช้งบประมาณ", &routing.request_steps),
        ("อนุมัติปรับงบประมาณ", &routing.adjustment_steps),
    ] {
        if steps.is_empty() {
            templates.push(None);
        } else {
            let id = Uuid::now_v7();
            process::create_source_template_in_transaction(
                &mut tx,
                a.user_id,
                id,
                "budget",
                title,
                steps.clone(),
            )
            .await?;
            templates.push(Some(id));
        }
    }
    DbSession::new(&mut *tx).query("budget.src.settings.save", "UPDATE budget_settings SET revision=revision+1,routing=$1,plan_template_id=$2,request_template_id=$3,adjustment_template_id=$4 WHERE singleton").bind(Json(&routing)).bind(templates[0]).bind(templates[1]).bind(templates[2]).execute().await?;
    #[derive(Serialize)]
    struct Audit<'a> {
        action: &'a str,
        revision: i64,
        routing: &'a Routing,
    }
    DbSession::new(&mut *tx).query("budget.src.settings.save", "INSERT INTO budget_history(id,resource_kind,resource_id,revision,actor_id,document) VALUES($1,'settings',$2,$3,$4,$5)").bind(Uuid::now_v7()).bind(Uuid::nil()).bind(old.revision+1).bind(a.user_id).bind(Json(Audit{action:"settings_saved",revision:old.revision+1,routing:&routing})).execute().await?;
    tx.commit().await?;
    get(pool, a).await
}
pub(crate) async fn routing(tx: &mut Transaction<'_, Postgres>) -> Result<SettingsRow, AppError> {
    Ok(DbSession::new(&mut **tx).query_as("budget.src.settings.routing", "SELECT revision,routing,plan_template_id,request_template_id,adjustment_template_id FROM budget_settings WHERE singleton FOR SHARE").fetch_one().await?)
}
pub(crate) async fn validate_project(
    tx: &mut Transaction<'_, Postgres>,
    p: &BudgetProjectInput,
    existing: bool,
) -> Result<(), AppError> {
    routing(tx).await?;
    text(&p.name, 250)?;
    let valid:bool=DbSession::new(&mut **tx).scalar("budget.src.settings.validate_project", "SELECT EXISTS(SELECT 1 FROM budget_fiscal_years y JOIN organization_units u ON u.id=$2 AND u.is_active WHERE y.id=$1 AND ($5 OR y.active) AND y.start_date<=$3 AND y.end_date>=$4 AND $4>=$3)").bind(p.fiscal_year_id).bind(p.organization_unit_id).bind(p.start_date).bind(p.end_date).bind(existing).fetch_one().await?;
    if !valid {
        return Err(invalid("ปีงบประมาณ หน่วยงาน หรือช่วงวันที่โครงการไม่ถูกต้อง"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_workflow_flags_refused() {
        let s = ProcessTemplateStep {
            title: "approve".into(),
            kind: ProcessStepKind::Approve,
            completion_policy: school_workflow::process_models::ProcessCompletionPolicy::Any,
            recipients: vec![school_workflow::process_models::ProcessRecipient {
                kind: school_workflow::process_models::ProcessRecipientKind::User,
                user_id: Some(Uuid::now_v7()),
                organization_unit_id: None,
                position_code: None,
            }],
            due_after_days: None,
            requires_signature: true,
            requires_comment: false,
            requires_evidence: false,
            requires_delegated_authority: false,
        };
        assert!(validate_steps(&[s.clone()]).is_err());
        let mut s = s;
        s.requires_signature = false;
        assert!(validate_steps(&[s.clone()]).is_ok());
        s.kind = ProcessStepKind::Review;
        assert!(validate_steps(&[s]).is_err());
    }
}
