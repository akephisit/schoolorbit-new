use crate::{models::*, money::{self, invalid, text}, repository};
use chrono::{DateTime, NaiveDate, Utc};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::{process, process_models::{ProcessStepKind, ProcessTemplateStep}};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
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
#[derive(sqlx::FromRow)]
struct YearRow {
    id: Uuid,
    revision: i64,
    name: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    is_closed: bool,
    closed_at: Option<DateTime<Utc>>,
    funding_configured: bool,
}
const YEAR_COLUMNS: &str = "id,revision,name,start_date,end_date,is_closed,closed_at,funding_configured";
const USED: &str = "EXISTS(SELECT 1 FROM budget_projects WHERE fiscal_year_id=$1) OR EXISTS(SELECT 1 FROM budget_project_revisions WHERE document->'project'->>'fiscalYearId'=$1::text) OR EXISTS(SELECT 1 FROM budget_plan_snapshots WHERE document->'project'->>'fiscalYearId'=$1::text)";
pub(crate) fn require_catalogs(a: &ActorContext) -> Result<(), AppError> {
    a.require_any_permission(&[codes::BUDGET_READ_OWN,codes::BUDGET_READ_ASSIGNED,codes::BUDGET_READ_ORGANIZATION_UNIT,codes::BUDGET_READ_ORGANIZATION_TREE,codes::BUDGET_READ_SCHOOL,codes::BUDGET_CREATE_OWN,codes::BUDGET_MANAGE_SCHOOL,codes::BUDGET_PAYMENT_EXECUTE_SCHOOL,codes::BUDGET_ADJUSTMENT_CREATE_SCHOOL,codes::BUDGET_DOWNLOAD_OWN,codes::BUDGET_DOWNLOAD_ORGANIZATION_UNIT,codes::BUDGET_DOWNLOAD_ORGANIZATION_TREE,codes::BUDGET_DOWNLOAD_SCHOOL])
}
fn require_settings(a: &ActorContext) -> Result<(), AppError> {
    a.require_any_permission(&[codes::BUDGET_MANAGE_SCHOOL,codes::BUDGET_PAYMENT_EXECUTE_SCHOOL])
}
async fn row(tx: &mut Transaction<'_,Postgres>, id:Uuid) -> Result<YearRow,AppError> {
    DbSession::new(&mut **tx).query_as("budget.settings.year",sqlx::AssertSqlSafe(format!("SELECT {YEAR_COLUMNS} FROM budget_fiscal_years WHERE id=$1"))).bind(id).fetch_optional().await?.ok_or_else(||AppError::NotFound("ไม่พบปีงบประมาณ".into()))
}
async fn used(tx: &mut Transaction<'_,Postgres>, id:Uuid)->Result<bool,AppError>{
    Ok(DbSession::new(&mut **tx).scalar("budget.settings.used",sqlx::AssertSqlSafe(format!("SELECT {USED}"))).bind(id).fetch_one().await?)
}
async fn close_blocked(tx: &mut Transaction<'_,Postgres>, id:Uuid)->Result<bool,AppError>{
    Ok(DbSession::new(&mut **tx).scalar("budget.settings.close_blocked", "SELECT EXISTS(SELECT 1 FROM budget_projects WHERE fiscal_year_id=$1 AND status='submitted') OR EXISTS(SELECT 1 FROM budget_requests r JOIN budget_projects p ON p.id=r.project_id WHERE p.fiscal_year_id=$1 AND r.status='submitted') OR EXISTS(SELECT 1 FROM budget_adjustments WHERE fiscal_year_id=$1 AND status='submitted') OR EXISTS(SELECT 1 FROM budget_allocations a JOIN budget_projects p ON p.id=a.project_id WHERE p.fiscal_year_id=$1 AND a.reserved>0) OR EXISTS(SELECT 1 FROM budget_loans l JOIN budget_adjustments a ON a.id=l.id WHERE a.fiscal_year_id=$1 AND l.amount>l.repaid)").bind(id).fetch_one().await?)
}
async fn year_deletable(tx: &mut Transaction<'_,Postgres>,id:Uuid)->Result<bool,AppError>{
    let blocked:bool=DbSession::new(&mut **tx).scalar("budget.settings.year_deletable", "SELECT EXISTS(SELECT 1 FROM budget_projects p WHERE p.fiscal_year_id=$1 AND NOT budget_draft_project_deletable(p.id)) OR EXISTS(SELECT 1 FROM budget_project_revisions r JOIN budget_projects p ON p.id=r.project_id WHERE p.fiscal_year_id<>$1 AND r.document->'project'->>'fiscalYearId'=$1::text) OR EXISTS(SELECT 1 FROM budget_plan_snapshots WHERE document->'project'->>'fiscalYearId'=$1::text) OR EXISTS(SELECT 1 FROM budget_adjustments WHERE fiscal_year_id=$1) OR EXISTS(SELECT 1 FROM budget_year_funds WHERE fiscal_year_id=$1 AND received>0)").bind(id).fetch_one().await?;
    Ok(!blocked)
}
async fn project_year(r:YearRow,tx:&mut Transaction<'_,Postgres>,manage:bool)->Result<BudgetFiscalYear,AppError>{
    let used=used(tx,r.id).await?;
    let can_delete=manage && !r.is_closed && year_deletable(tx,r.id).await?;
    let can_close=manage && !r.is_closed && !close_blocked(tx,r.id).await?;
    let deletion_impact=if manage{crate::deletion::year_impact(tx,r.id).await?}else{BudgetDeletionImpact::default()};
    Ok(BudgetFiscalYear{id:r.id,revision:r.revision,name:r.name,start_date:r.start_date,end_date:r.end_date,is_closed:r.is_closed,closed_at:r.closed_at,used,date_locked:used,can_close,can_delete,deletion_impact})
}
async fn years(tx:&mut Transaction<'_,Postgres>,manage:bool)->Result<Vec<BudgetFiscalYear>,AppError>{
    let rows:Vec<YearRow>=DbSession::new(&mut **tx).query_as("budget.settings.years",sqlx::AssertSqlSafe(format!("SELECT {YEAR_COLUMNS} FROM budget_fiscal_years ORDER BY start_date DESC,id"))).fetch_all().await?;
    let mut result=Vec::with_capacity(rows.len());
    for r in rows { result.push(project_year(r,tx,manage).await?); }
    Ok(result)
}
async fn catalogs_tx(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<(Vec<BudgetCatalogItem>,Vec<BudgetCatalogItem>),AppError>{
    let funds=DbSession::new(&mut **tx).query_as::<CatalogRow>("budget.settings.funds", "SELECT fund_id AS id,name,active FROM budget_year_funds WHERE fiscal_year_id=$1 ORDER BY name,fund_id").bind(id).fetch_all().await?.into_iter().map(Into::into).collect();
    let cats=DbSession::new(&mut **tx).query_as::<CatalogRow>("budget.settings.categories", "SELECT expense_category_id AS id,name,active FROM budget_year_expense_categories WHERE fiscal_year_id=$1 ORDER BY name,expense_category_id").bind(id).fetch_all().await?.into_iter().map(Into::into).collect();
    Ok((funds,cats))
}
#[derive(sqlx::FromRow)]
struct CatalogRow{id:Uuid,name:String,active:bool}
impl From<CatalogRow> for BudgetCatalogItem {fn from(r:CatalogRow)->Self{Self{id:r.id,name:r.name,active:r.active}}}
pub async fn catalogs(pool:&PgPool,a:&ActorContext,q:BudgetCatalogQuery)->Result<BudgetCatalogs,AppError>{
    require_catalogs(a)?;
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.catalogs").await?;
    read_only(&mut tx).await?;
    let fiscal_years=years(&mut tx,a.has_permission(codes::BUDGET_MANAGE_SCHOOL)).await?;
    let (funds,expense_categories)=if let Some(id)=q.fiscal_year_id{row(&mut tx,id).await?;catalogs_tx(&mut tx,id).await?}else{(vec![],vec![])};
    tx.commit().await?;
    Ok(BudgetCatalogs{fiscal_years,funds,expense_categories})
}
pub async fn get(pool:&PgPool,a:&ActorContext)->Result<BudgetSettings,AppError>{
    require_settings(a)?;
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.index").await?;
    read_only(&mut tx).await?;
    let fiscal_years=years(&mut tx,a.has_permission(codes::BUDGET_MANAGE_SCHOOL)).await?;
    tx.commit().await?;
    Ok(BudgetSettings{fiscal_years})
}
async fn read_only(tx:&mut Transaction<'_,Postgres>)->Result<(),AppError>{
    DbSession::new(&mut **tx).query("budget.settings.read_only","SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute().await?;Ok(())
}
pub(crate) async fn lock_years(tx:&mut Transaction<'_,Postgres>,ids:&[Uuid],allow_closed:bool)->Result<Vec<BudgetFiscalYear>,AppError>{
    let mut ids=ids.to_vec();ids.sort();ids.dedup();
    let rows:Vec<YearRow>=DbSession::new(&mut **tx).query_as("budget.settings.lock_years",sqlx::AssertSqlSafe(format!("SELECT {YEAR_COLUMNS} FROM budget_fiscal_years WHERE id=ANY($1) ORDER BY id FOR UPDATE"))).bind(&ids).fetch_all().await?;
    if rows.len()!=ids.len(){return Err(AppError::NotFound("ไม่พบปีงบประมาณ".into()));}
    if !allow_closed && rows.iter().any(|r|r.is_closed){return Err(AppError::Conflict("ปีงบประมาณปิดแล้ว ไม่สามารถเปลี่ยนแปลงข้อมูล".into()));}
    Ok(rows.into_iter().map(|r|BudgetFiscalYear{id:r.id,revision:r.revision,name:r.name,start_date:r.start_date,end_date:r.end_date,is_closed:r.is_closed,closed_at:r.closed_at,used:false,date_locked:false,can_close:false,can_delete:false,deletion_impact:BudgetDeletionImpact::default()}).collect())
}
pub(crate) async fn lock_project_years(tx:&mut Transaction<'_,Postgres>,projects:&[Uuid],additional_years:&[Uuid])->Result<(),AppError>{
    let mut ids:Vec<Uuid>=DbSession::new(&mut **tx).scalar("budget.settings.project_years","SELECT DISTINCT fiscal_year_id FROM budget_projects WHERE id=ANY($1)").bind(projects).fetch_all().await?;
    ids.extend_from_slice(additional_years);lock_years(tx,&ids,false).await?;Ok(())
}
pub(crate) async fn routing(tx:&mut Transaction<'_,Postgres>,year:Uuid)->Result<SettingsRow,AppError>{
    Ok(DbSession::new(&mut **tx).query_as("budget.settings.routing","SELECT revision,routing,plan_template_id,request_template_id,adjustment_template_id FROM budget_fiscal_years WHERE id=$1").bind(year).fetch_one().await?)
}
fn validate_dates(name:&str,start:NaiveDate,end:NaiveDate)->Result<(),AppError>{
    text(name,120)?;if end<start || (end-start).num_days()>730{return Err(invalid("ช่วงปีงบประมาณไม่ถูกต้อง"));}Ok(())
}
pub fn validate_steps(steps:&[ProcessTemplateStep])->Result<(),AppError>{
    if steps.is_empty(){return Ok(());}process::validate_steps(steps)?;
    if steps.len()>10 || steps.iter().any(|s|s.recipients.is_empty()||s.requires_signature||s.requires_evidence||s.requires_delegated_authority||!matches!(s.kind,ProcessStepKind::Review|ProcessStepKind::Approve)) || steps.last().is_none_or(|s|s.kind!=ProcessStepKind::Approve){return Err(invalid("เส้นทางต้องเป็นตรวจสอบ/อนุมัติและจบด้วยอนุมัติ โดยไม่ใช้ลายเซ็น หลักฐาน หรือมอบอำนาจรายขั้นตอน"));}Ok(())
}
async fn templates(tx:&mut Transaction<'_,Postgres>,a:&ActorContext,r:&Routing)->Result<Vec<Option<Uuid>>,AppError>{
    let mut ids=vec![];for (title,steps) in [("อนุมัติแผนงบประมาณ",&r.plan_steps),("อนุมัติคำขอใช้งบประมาณ",&r.request_steps),("อนุมัติปรับงบประมาณ",&r.adjustment_steps)]{
        validate_steps(steps)?;if steps.is_empty(){ids.push(None);}else{let id=Uuid::now_v7();process::create_source_template_in_transaction(tx,a.user_id,id,"budget",title,steps.clone()).await?;ids.push(Some(id));}
    }Ok(ids)
}
async fn audit<T:Serialize>(tx:&mut Transaction<'_,Postgres>,a:&ActorContext,id:Uuid,revision:i64,action:&str,input:&T)->Result<(),AppError>{
    DbSession::new(&mut **tx).query("budget.settings.audit","INSERT INTO budget_history(id,resource_kind,resource_id,revision,actor_id,document) VALUES($1,'settings',$2,$3,$4,$5)").bind(Uuid::now_v7()).bind(id).bind(revision).bind(a.user_id).bind(Json(serde_json::json!({"action":action,"revision":revision,"settings":input}))).execute().await?;Ok(())
}
pub async fn create_year(pool:&PgPool,a:&ActorContext,input:CreateBudgetFiscalYearRequest)->Result<BudgetYearSettings,AppError>{
    a.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;validate_dates(&input.name,input.start_date,input.end_date)?;
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.create_year").await?;
    if let Some(source)=input.copy_from_fiscal_year_id{lock_years(&mut tx,&[source],true).await?;}
    let id=Uuid::now_v7();if let Some(old)=repository::claim(&mut tx,a.user_id,input.request_id,"fiscal_year_create",id,&input).await?{tx.commit().await?;return get_year(pool,a,old).await;}
    let count:i64=DbSession::new(&mut *tx).scalar("budget.settings.year_count","SELECT count(*) FROM budget_fiscal_years").fetch_one().await?;if count>=100{return Err(invalid("รองรับไม่เกิน 100 ปีงบประมาณ"));}
    DbSession::new(&mut *tx).query("budget.settings.create_year","INSERT INTO budget_fiscal_years(id,name,start_date,end_date) VALUES($1,$2,$3,$4)").bind(id).bind(input.name.trim()).bind(input.start_date).bind(input.end_date).execute().await?;
    if let Some(source)=input.copy_from_fiscal_year_id{
        DbSession::new(&mut *tx).query("budget.settings.copy_funds","INSERT INTO budget_year_funds(fiscal_year_id,fund_id,name,active) SELECT $1,fund_id,name,active FROM budget_year_funds WHERE fiscal_year_id=$2").bind(id).bind(source).execute().await?;
        DbSession::new(&mut *tx).query("budget.settings.copy_categories","INSERT INTO budget_year_expense_categories(fiscal_year_id,expense_category_id,name,active) SELECT $1,expense_category_id,name,active FROM budget_year_expense_categories WHERE fiscal_year_id=$2").bind(id).bind(source).execute().await?;
        let r=routing(&mut tx,source).await?;let ids=templates(&mut tx,a,&r.routing).await?;
        DbSession::new(&mut *tx).query("budget.settings.copy_routing","UPDATE budget_fiscal_years SET routing=$2,plan_template_id=$3,request_template_id=$4,adjustment_template_id=$5 WHERE id=$1").bind(id).bind(r.routing).bind(ids[0]).bind(ids[1]).bind(ids[2]).execute().await?;
    }
    audit(&mut tx,a,id,1,"year_created",&input).await?;tx.commit().await?;get_year(pool,a,id).await
}
#[derive(sqlx::FromRow)]
struct FundRow{fund_id:Uuid,carryover:Option<i64>,annual_funding:Option<i64>,reserve:Option<i64>,received:Option<i64>}
async fn year_detail(tx:&mut Transaction<'_,Postgres>,a:&ActorContext,id:Uuid)->Result<BudgetYearSettings,AppError>{
    let y=row(tx,id).await?;let configured=y.funding_configured;let fiscal_year=project_year(y,tx,a.has_permission(codes::BUDGET_MANAGE_SCHOOL)).await?;
    let (funds,expense_categories)=catalogs_tx(tx,id).await?;let r=routing(tx,id).await?;
    let sources:Vec<FundRow>=DbSession::new(&mut **tx).query_as("budget.settings.funding","SELECT fund_id,carryover,annual_funding,reserve,received FROM budget_year_funds WHERE fiscal_year_id=$1 ORDER BY fund_id").bind(id).fetch_all().await?;
    let allocations:Vec<(Uuid,Uuid,i64)>=DbSession::new(&mut **tx).query_as("budget.settings.allocations","SELECT organization_unit_id,fund_id,allocated FROM budget_unit_allocations WHERE fiscal_year_id=$1 ORDER BY organization_unit_id,fund_id").bind(id).fetch_all().await?;
    let commitments:Vec<(Uuid,Uuid,i64)>=DbSession::new(&mut **tx).query_as("budget.settings.commitments","SELECT p.organization_unit_id,b.fund_id,sum(b.planned+b.adjusted)::bigint FROM budget_allocations b JOIN budget_projects p ON p.id=b.project_id WHERE p.fiscal_year_id=$1 GROUP BY p.organization_unit_id,b.fund_id ORDER BY 1,2").bind(id).fetch_all().await?;
    let authorized_funding=if configured{Some(sources.iter().filter_map(|s|Some(BudgetAuthorizedFundingInput{fund_id:s.fund_id,carryover_satang:s.carryover?.to_string(),annual_funding_satang:s.annual_funding?.to_string(),reserve_satang:s.reserve?.to_string()})).collect())}else{None};
    let unit_allocations=allocations.iter().map(|(unit,fund,n)|BudgetUnitAllocationInput{organization_unit_id:*unit,fund_id:*fund,allocated_satang:n.to_string()}).collect();
    let source_balances=sources.iter().map(|s|{
        let allocated=allocations.iter().filter(|(_,f,_)|*f==s.fund_id).map(|(_,_,n)|*n).sum::<i64>();let committed=commitments.iter().filter(|(_,f,_)|*f==s.fund_id).map(|(_,_,n)|*n).sum::<i64>();let authorized=s.carryover.zip(s.annual_funding).map(|(c,a)|c+a);
        BudgetSourceBalance{fund_id:s.fund_id,carryover_satang:s.carryover.map(|n|n.to_string()),annual_funding_satang:s.annual_funding.map(|n|n.to_string()),authorized_satang:authorized.map(|n|n.to_string()),allocated_satang:allocated.to_string(),reserve_satang:s.reserve.map(|n|n.to_string()),unallocated_satang:authorized.zip(s.reserve).map(|(n,r)|(n-r-allocated).to_string()),committed_satang:committed.to_string(),remaining_satang:authorized.map(|n|(n-committed).to_string()),received_satang:s.received.map(|n|n.to_string())}
    }).collect();
    let mut units=BTreeMap::<(Uuid,Uuid),(i64,i64)>::new();for (u,f,n) in allocations{units.entry((u,f)).or_default().0=n;}for (u,f,n) in commitments{units.entry((u,f)).or_default().1=n;}
    let unit_balances=units.into_iter().map(|((u,f),(allocated,committed))|BudgetUnitBalance{organization_unit_id:u,fund_id:f,allocated_satang:allocated.to_string(),committed_satang:committed.to_string(),remaining_satang:(allocated-committed).to_string()}).collect();
    Ok(BudgetYearSettings{fiscal_year,funds,expense_categories,plan_steps:r.routing.plan_steps.clone(),request_steps:r.routing.request_steps.clone(),adjustment_steps:r.routing.adjustment_steps.clone(),authorized_funding,unit_allocations,source_balances,unit_balances,can_manage:a.has_permission(codes::BUDGET_MANAGE_SCHOOL),can_record_funding:a.has_permission(codes::BUDGET_PAYMENT_EXECUTE_SCHOOL)})
}
pub async fn get_year(pool:&PgPool,a:&ActorContext,id:Uuid)->Result<BudgetYearSettings,AppError>{
    require_settings(a)?;let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.get_year").await?;read_only(&mut tx).await?;let result=year_detail(&mut tx,a,id).await?;tx.commit().await?;Ok(result)
}
fn validate_items(items:&[BudgetCatalogItemInput])->Result<(),AppError>{
    if items.len()>100{return Err(invalid("ตั้งค่าแต่ละประเภทได้ไม่เกิน 100 รายการ"));}let mut seen=BTreeSet::new();for item in items{text(&item.name,120)?;if item.id.is_some_and(|id|!seen.insert(id)){return Err(invalid("รหัสรายการตั้งค่าซ้ำ"));}}Ok(())
}
async fn save_items(tx:&mut Transaction<'_,Postgres>,year:Uuid,items:&[BudgetCatalogItemInput],funds:bool)->Result<(),AppError>{
    let (identity,table,column)=if funds{("budget_funds","budget_year_funds","fund_id")}else{("budget_expense_categories","budget_year_expense_categories","expense_category_id")};
    let existing:Vec<Uuid>=DbSession::new(&mut **tx).scalar("budget.settings.existing_items",sqlx::AssertSqlSafe(format!("SELECT {column} FROM {table} WHERE fiscal_year_id=$1"))).bind(year).fetch_all().await?;
    let ids=items.iter().filter_map(|i|i.id).collect::<Vec<_>>();if ids.iter().any(|id|!existing.contains(id)){return Err(invalid("รายการตั้งค่าไม่ได้อยู่ในปีงบประมาณนี้"));}
    if existing.len()+items.iter().filter(|i|i.id.is_none()).count()>500{return Err(invalid("ตั้งค่ารวมประวัติได้ไม่เกิน 500 รายการ"));}
    DbSession::new(&mut **tx).query("budget.settings.deactivate_items",sqlx::AssertSqlSafe(format!("UPDATE {table} SET active=false WHERE fiscal_year_id=$1"))).bind(year).execute().await?;
    for item in items{let id=item.id.unwrap_or_else(Uuid::now_v7);if item.id.is_none(){DbSession::new(&mut **tx).query("budget.settings.create_identity",sqlx::AssertSqlSafe(format!("INSERT INTO {identity}(id) VALUES($1)"))).bind(id).execute().await?;}
        DbSession::new(&mut **tx).query("budget.settings.save_item",sqlx::AssertSqlSafe(format!("INSERT INTO {table}(fiscal_year_id,{column},name,active) VALUES($1,$2,$3,$4) ON CONFLICT(fiscal_year_id,{column}) DO UPDATE SET name=excluded.name,active=excluded.active"))).bind(year).bind(id).bind(item.name.trim()).bind(item.active).execute().await?;
    }Ok(())
}
pub async fn save_year(pool:&PgPool,a:&ActorContext,id:Uuid,input:SaveBudgetYearSettingsRequest)->Result<BudgetYearSettings,AppError>{
    a.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;validate_dates(&input.name,input.start_date,input.end_date)?;validate_items(&input.funds)?;validate_items(&input.expense_categories)?;
    for s in [&input.plan_steps,&input.request_steps,&input.adjustment_steps]{validate_steps(s)?;}
    if input.unit_allocations.len()>1000{return Err(invalid("จัดสรรได้ไม่เกิน 1,000 รายการ"));}
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.save_year").await?;lock_years(&mut tx,&[id],false).await?;
    if repository::claim(&mut tx,a.user_id,input.request_id,"year_settings_save",id,&(id,&input)).await?.is_some(){tx.commit().await?;return get_year(pool,a,id).await;}
    let old=row(&mut tx,id).await?;repository::revision(old.revision,input.expected_revision)?;
    if (old.start_date!=input.start_date||old.end_date!=input.end_date)&&used(&mut tx,id).await?{return Err(invalid("ปีงบประมาณถูกใช้แล้ว ไม่สามารถเปลี่ยนวันเริ่มหรือสิ้นสุด"));}
    save_items(&mut tx,id,&input.funds,true).await?;save_items(&mut tx,id,&input.expense_categories,false).await?;
    let funds:Vec<Uuid>=DbSession::new(&mut *tx).scalar("budget.settings.year_fund_ids","SELECT fund_id FROM budget_year_funds WHERE fiscal_year_id=$1").bind(id).fetch_all().await?;
    let mut seen=BTreeSet::new();let mut school_total=vec![];
    if let Some(authorized)=&input.authorized_funding{
        if authorized.len()!=funds.len(){return Err(invalid("กรุณาระบุวงเงินทุกแหล่งเงินของปีงบประมาณ รวมรายการปิดใช้งาน"));}
        for f in authorized{if !funds.contains(&f.fund_id)||!seen.insert(f.fund_id){return Err(invalid("แหล่งเงินซ้ำหรือไม่ได้อยู่ในปีงบประมาณ"));}let carry=money::parse_money(&f.carryover_satang)?;let annual=money::parse_money(&f.annual_funding_satang)?;let total=money::sum([carry,annual])?;let reserve=money::parse_money(&f.reserve_satang)?;if reserve>total{return Err(invalid("เงินสำรองเกินวงเงินที่ได้รับอนุมัติ"));}school_total.push(total);
            DbSession::new(&mut *tx).query("budget.settings.save_authorization","UPDATE budget_year_funds SET carryover=$3,annual_funding=$4,reserve=$5 WHERE fiscal_year_id=$1 AND fund_id=$2").bind(id).bind(f.fund_id).bind(carry).bind(annual).bind(reserve).execute().await?;
        }money::sum(school_total)?;
    }else{
        if old.funding_configured{return Err(invalid("ไม่สามารถล้างวงเงินที่ยืนยันแล้วเป็นยังไม่ตั้งค่า"));}
        DbSession::new(&mut *tx).query("budget.settings.unconfigured","UPDATE budget_year_funds SET carryover=NULL,annual_funding=NULL,reserve=NULL WHERE fiscal_year_id=$1").bind(id).execute().await?;
    }
    let mut seen=BTreeSet::new();DbSession::new(&mut *tx).query("budget.settings.replace_allocations","DELETE FROM budget_unit_allocations WHERE fiscal_year_id=$1").bind(id).execute().await?;
    for unit in &input.unit_allocations{if !funds.contains(&unit.fund_id)||!seen.insert((unit.organization_unit_id,unit.fund_id)){return Err(invalid("หน่วยงานหรือแหล่งเงินจัดสรรซ้ำหรือไม่ถูกต้อง"));}let amount=money::parse_money(&unit.allocated_satang)?;let valid:bool=DbSession::new(&mut *tx).scalar("budget.settings.unit_valid","SELECT EXISTS(SELECT 1 FROM organization_units WHERE id=$1 AND is_active) OR EXISTS(SELECT 1 FROM budget_projects WHERE fiscal_year_id=$2 AND organization_unit_id=$1)").bind(unit.organization_unit_id).bind(id).fetch_one().await?;if !valid{return Err(invalid("ไม่พบหน่วยงานที่จัดสรร"));}
        DbSession::new(&mut *tx).query("budget.settings.unit_save","INSERT INTO budget_unit_allocations(fiscal_year_id,organization_unit_id,fund_id,allocated) VALUES($1,$2,$3,$4)").bind(id).bind(unit.organization_unit_id).bind(unit.fund_id).bind(amount).execute().await?;
    }
    let routing=Routing{plan_steps:input.plan_steps.clone(),request_steps:input.request_steps.clone(),adjustment_steps:input.adjustment_steps.clone()};let ids=templates(&mut tx,a,&routing).await?;
    DbSession::new(&mut *tx).query("budget.settings.store_year","UPDATE budget_fiscal_years SET revision=revision+1,name=$2,start_date=$3,end_date=$4,funding_configured=$5,routing=$6,plan_template_id=$7,request_template_id=$8,adjustment_template_id=$9 WHERE id=$1").bind(id).bind(input.name.trim()).bind(input.start_date).bind(input.end_date).bind(input.authorized_funding.is_some()).bind(Json(&routing)).bind(ids[0]).bind(ids[1]).bind(ids[2]).execute().await?;
    validate_limits(&mut tx,id,false).await?;audit(&mut tx,a,id,old.revision+1,"year_settings_saved",&input).await?;tx.commit().await?;get_year(pool,a,id).await
}
/// Call after the complete signed movement, while the year lock is held.
pub(crate) async fn validate_limits(tx:&mut Transaction<'_,Postgres>,year:Uuid,require_configured:bool)->Result<(),AppError>{
    let configured:bool=DbSession::new(&mut **tx).scalar("budget.settings.configured","SELECT funding_configured FROM budget_fiscal_years WHERE id=$1").bind(year).fetch_one().await?;
    if !configured{if require_configured{return Err(AppError::Conflict("กรุณายืนยันวงเงินประจำปีและจัดสรรหน่วยงานก่อนอนุมัติงบประมาณ".into()));}let nonzero:bool=DbSession::new(&mut **tx).scalar("budget.settings.unconfigured_allocations","SELECT EXISTS(SELECT 1 FROM budget_unit_allocations WHERE fiscal_year_id=$1 AND allocated<>0)").bind(year).fetch_one().await?;if nonzero{return Err(invalid("ต้องยืนยันวงเงินประจำปีก่อนจัดสรรเงิน"));}return Ok(());}
    let invalid:bool=DbSession::new(&mut **tx).scalar("budget.settings.validate_limits",r#"
      WITH commitments AS (SELECT p.organization_unit_id,a.fund_id,sum(a.planned+a.adjusted) amount FROM budget_allocations a JOIN budget_projects p ON p.id=a.project_id WHERE p.fiscal_year_id=$1 GROUP BY 1,2),
      allocated AS (SELECT fund_id,sum(allocated) amount FROM budget_unit_allocations WHERE fiscal_year_id=$1 GROUP BY fund_id)
      SELECT EXISTS(SELECT 1 FROM budget_year_funds f LEFT JOIN allocated a ON a.fund_id=f.fund_id WHERE f.fiscal_year_id=$1 AND (f.carryover IS NULL OR f.annual_funding IS NULL OR f.reserve IS NULL OR coalesce(a.amount,0)+f.reserve>f.carryover+f.annual_funding))
       OR EXISTS(SELECT 1 FROM commitments c LEFT JOIN budget_unit_allocations u ON u.fiscal_year_id=$1 AND u.organization_unit_id=c.organization_unit_id AND u.fund_id=c.fund_id WHERE c.amount>coalesce(u.allocated,0))
       OR EXISTS(SELECT 1 FROM commitments c LEFT JOIN budget_year_funds f ON f.fiscal_year_id=$1 AND f.fund_id=c.fund_id WHERE f.fund_id IS NULL)
       OR (SELECT coalesce(sum(amount),0) FROM commitments)>(SELECT coalesce(sum(carryover+annual_funding),0) FROM budget_year_funds WHERE fiscal_year_id=$1)
    "#).bind(year).fetch_one().await?;
    if invalid{return Err(AppError::Conflict("วงเงินโรงเรียน แหล่งเงิน เงินสำรอง หรือหน่วยงานไม่เพียงพอสำหรับยอดอนุมัติ".into()));}Ok(())
}
pub async fn save_received(pool:&PgPool,a:&ActorContext,id:Uuid,input:SaveBudgetReceivedFundingRequest)->Result<BudgetYearSettings,AppError>{
    a.require_permission(codes::BUDGET_PAYMENT_EXECUTE_SCHOOL)?;if input.funds.len()>500{return Err(invalid("แหล่งเงินมากเกินกำหนด"));}
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.save_received").await?;lock_years(&mut tx,&[id],false).await?;
    if repository::claim(&mut tx,a.user_id,input.request_id,"year_funding_save",id,&(id,&input)).await?.is_some(){tx.commit().await?;return get_year(pool,a,id).await;}
    let old=row(&mut tx,id).await?;repository::revision(old.revision,input.expected_revision)?;let mut seen=BTreeSet::new();for f in &input.funds{if !seen.insert(f.fund_id){return Err(invalid("แหล่งเงินซ้ำ"));}let n=f.received_satang.as_deref().map(money::parse_money).transpose()?;
        let changed=DbSession::new(&mut *tx).query("budget.settings.received_save","UPDATE budget_year_funds SET received=$3 WHERE fiscal_year_id=$1 AND fund_id=$2").bind(id).bind(f.fund_id).bind(n).execute().await?;if changed.rows_affected()!=1{return Err(invalid("แหล่งเงินไม่ได้อยู่ในปีงบประมาณ"));}
    }
    DbSession::new(&mut *tx).query("budget.settings.received_revision","UPDATE budget_fiscal_years SET revision=revision+1 WHERE id=$1").bind(id).execute().await?;audit(&mut tx,a,id,old.revision+1,"funding_received_saved",&input).await?;tx.commit().await?;get_year(pool,a,id).await
}
pub async fn close_year(pool:&PgPool,a:&ActorContext,id:Uuid,input:BudgetMutationRequest)->Result<BudgetYearSettings,AppError>{
    a.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.close_year").await?;lock_years(&mut tx,&[id],true).await?;
    if repository::claim(&mut tx,a.user_id,input.request_id,"year_close",id,&(id,&input)).await?.is_some(){tx.commit().await?;return get_year(pool,a,id).await;}
    let old=row(&mut tx,id).await?;repository::revision(old.revision,input.expected_revision)?;if old.is_closed||close_blocked(&mut tx,id).await?{return Err(AppError::Conflict("ยังมีกระบวนงาน ยอดผูกพัน หรือเงินยืมค้าง ไม่สามารถปิดปีงบประมาณ".into()));}
    DbSession::new(&mut *tx).query("budget.settings.close_year","UPDATE budget_fiscal_years SET revision=revision+1,is_closed=true,closed_at=clock_timestamp() WHERE id=$1").bind(id).execute().await?;audit(&mut tx,a,id,old.revision+1,"year_closed",&input).await?;tx.commit().await?;get_year(pool,a,id).await
}
pub async fn delete_year(pool:&PgPool,a:&ActorContext,id:Uuid,input:BudgetMutationRequest)->Result<BudgetDeletedResource,AppError>{
    a.require_permission(codes::BUDGET_MANAGE_SCHOOL)?;let mut tx=school_tenancy::TenantPool::begin_observed(pool,"budget.settings.delete_year").await?;lock_years(&mut tx,&[id],false).await?;let old=row(&mut tx,id).await?;repository::revision(old.revision,input.expected_revision)?;
    if !year_deletable(&mut tx,id).await?{return Err(AppError::Conflict("ปีงบประมาณมีแผนที่อนุมัติ ข้อมูลการเงิน หรือประวัติที่ยังต้องเก็บ".into()));}
    let projects:Vec<Uuid>=DbSession::new(&mut *tx).scalar("budget.settings.delete_year_projects","SELECT id FROM budget_projects WHERE fiscal_year_id=$1 ORDER BY id FOR UPDATE").bind(id).fetch_all().await?;
    for project in projects{crate::deletion::delete_project_for_year_in_transaction(&mut tx,a,project,id).await?;}
    DbSession::new(&mut *tx).query("budget.settings.finalize_year_delete","SELECT budget_finalize_draft_year_removal($1)").bind(id).execute().await?;tx.commit().await?;Ok(BudgetDeletedResource{deleted_id:id})
}
pub(crate) async fn validate_project(tx:&mut Transaction<'_,Postgres>,p:&BudgetProjectInput,_existing:bool)->Result<(),AppError>{
    text(&p.name,250)?;let valid:bool=DbSession::new(&mut **tx).scalar("budget.settings.validate_project","SELECT EXISTS(SELECT 1 FROM budget_fiscal_years y JOIN organization_units u ON u.id=$2 AND u.is_active WHERE y.id=$1 AND NOT y.is_closed AND y.start_date<=$3 AND y.end_date>=$4 AND $4>=$3)").bind(p.fiscal_year_id).bind(p.organization_unit_id).bind(p.start_date).bind(p.end_date).fetch_one().await?;if !valid{return Err(invalid("ปีงบประมาณ หน่วยงาน หรือช่วงวันที่โครงการไม่ถูกต้อง"));}Ok(())
}
pub(crate) fn validate_activity_dates(p:&BudgetProjectInput,activities:&[BudgetActivity])->Result<(),AppError>{
    for activity in activities{let (Some(start),Some(end))=(activity.start_date,activity.end_date)else{return Err(invalid("กรุณาระบุวันเริ่มและสิ้นสุดของทุกกิจกรรมก่อนบันทึกหรืออนุมัติแผน"));};if start>end||start<p.start_date||end>p.end_date{return Err(invalid("วันที่กิจกรรมต้องอยู่ภายในช่วงโครงการและปีงบประมาณ"));}}Ok(())
}
