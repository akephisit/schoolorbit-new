use crate::{models::*,money::invalid};
use chrono::{DateTime,Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use serde::{Deserialize,Serialize};
use sqlx::{types::Json,PgPool,Postgres,Transaction};
use uuid::Uuid;
#[derive(Debug,Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct DraftPlan { pub project:BudgetProjectInput,pub activities:Vec<BudgetActivity> }
#[derive(Debug,Clone,sqlx::FromRow)]
pub(crate) struct ProjectRecord {pub id:Uuid,pub owner_id:Uuid,pub revision:i64,pub status:String,pub attempt_id:Uuid,pub accepted_version:Option<i64>,pub accepted_snapshot_id:Option<Uuid>,pub document:Json<DraftPlan>,pub instance_id:Option<Uuid>}
impl ProjectRecord { pub fn status(&self)->Result<BudgetStatus,AppError>{parse_status(&self.status)} }
pub(crate) fn parse_status(status:&str)->Result<BudgetStatus,AppError>{match status {"draft"=>Ok(BudgetStatus::Draft),"submitted"=>Ok(BudgetStatus::Submitted),"returned"=>Ok(BudgetStatus::Returned),"approved"=>Ok(BudgetStatus::Approved),"rejected"=>Ok(BudgetStatus::Rejected),"closed"=>Ok(BudgetStatus::Closed),_=>Err(AppError::InternalServerError("budget_invalid_persisted_status".into()))}}
pub(crate) fn stale()->AppError {AppError::Conflict("ข้อมูลงบประมาณเปลี่ยนแล้ว กรุณาโหลดรายการล่าสุด".into())}
pub(crate) fn denied()->AppError {AppError::Forbidden("ไม่มีสิทธิ์ดำเนินการงบประมาณนี้".into())}
pub(crate) fn revision(actual:i64,expected:i64)->Result<(),AppError>{if actual>0&&actual==expected {Ok(())}else{Err(stale())}}
pub(crate) async fn project(pool:&PgPool,id:Uuid)->Result<ProjectRecord,AppError>{sqlx::query_as("SELECT id,owner_id,revision,status,attempt_id,accepted_version,accepted_snapshot_id,document,instance_id FROM budget_projects WHERE id=$1").bind(id).fetch_optional(pool).await?.ok_or_else(||AppError::NotFound("ไม่พบโครงการ".into()))}
/// The aggregate row also serializes absent allocations and plan-amendment insertions.
pub(crate) async fn lock_projects(tx:&mut Transaction<'_,Postgres>,ids:&[Uuid])->Result<Vec<ProjectRecord>,AppError>{
 let mut ids=ids.to_vec();ids.sort();ids.dedup();
 let rows=sqlx::query_as::<_,ProjectRecord>("SELECT id,owner_id,revision,status,attempt_id,accepted_version,accepted_snapshot_id,document,instance_id FROM budget_projects WHERE id=ANY($1) ORDER BY id FOR UPDATE").bind(&ids).fetch_all(&mut **tx).await?;
 if rows.len()!=ids.len(){return Err(AppError::NotFound("ไม่พบโครงการ".into()));} Ok(rows)
}
pub(crate) async fn lock_project(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<ProjectRecord,AppError>{lock_projects(tx,&[id]).await?.pop().ok_or_else(||AppError::NotFound("ไม่พบโครงการ".into()))}
pub(crate) async fn store_project(tx:&mut Transaction<'_,Postgres>,r:&ProjectRecord)->Result<(),AppError>{
 sqlx::query("UPDATE budget_projects SET revision=$2,status=$3,attempt_id=$4,accepted_version=$5,accepted_snapshot_id=$6,document=$7,instance_id=$8,fiscal_year_id=$9,organization_unit_id=$10 WHERE id=$1").bind(r.id).bind(r.revision).bind(&r.status).bind(r.attempt_id).bind(r.accepted_version).bind(r.accepted_snapshot_id).bind(&r.document).bind(r.instance_id).bind(r.document.project.fiscal_year_id).bind(r.document.project.organization_unit_id).execute(&mut **tx).await?;
 sqlx::query("INSERT INTO budget_project_revisions(project_id,revision,document) VALUES($1,$2,$3)").bind(r.id).bind(r.revision).bind(&r.document).execute(&mut **tx).await?;Ok(())
}
pub(crate) async fn claim<T:Serialize>(tx:&mut Transaction<'_,Postgres>,actor:Uuid,key:Uuid,operation:&str,id:Uuid,payload:&T)->Result<Option<Uuid>,AppError>{
 let hash=school_workflow::process::payload_hash(&(operation,payload))?;
 let inserted:Option<Uuid>=sqlx::query_scalar("INSERT INTO budget_receipts(actor_id,request_id,operation,payload_hash,resource_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING RETURNING resource_id").bind(actor).bind(key).bind(operation).bind(&hash).bind(id).fetch_optional(&mut **tx).await?;
 if inserted.is_some(){return Ok(None);}
 let old:(String,String,Uuid)=sqlx::query_as("SELECT operation,payload_hash,resource_id FROM budget_receipts WHERE actor_id=$1 AND request_id=$2").bind(actor).bind(key).fetch_one(&mut **tx).await?;
 if old.0!=operation||old.1!=hash{return Err(AppError::Conflict("รหัสคำขอถูกใช้กับข้อมูลอื่นแล้ว".into()));}Ok(Some(old.2))
}
pub(crate) async fn history(tx:&mut Transaction<'_,Postgres>,actor:&ActorContext,kind:BudgetResourceKind,id:Uuid,rev:i64,action:&str,comment:Option<String>)->Result<(),AppError>{
 if kind==BudgetResourceKind::Adjustment{let written=sqlx::query("INSERT INTO budget_adjustment_revisions(adjustment_id,revision,status,document) SELECT id,revision,status,document FROM budget_adjustments WHERE id=$1 AND revision=$2").bind(id).bind(rev).execute(&mut **tx).await?;if written.rows_affected()!=1{return Err(stale());}}
 let entry=BudgetHistoryEntry{id:Uuid::now_v7(),resource_kind:kind,resource_id:id,revision:rev,action:action.into(),actor_id:actor.user_id,comment,created_at:Utc::now()};
 sqlx::query("INSERT INTO budget_history(id,resource_kind,resource_id,revision,actor_id,document) VALUES($1,$2,$3,$4,$5,$6)").bind(entry.id).bind(kind.as_str()).bind(id).bind(rev).bind(actor.user_id).bind(Json(entry)).execute(&mut **tx).await?;Ok(())
}
#[derive(Debug,Clone,sqlx::FromRow)]
pub(crate) struct Account {pub project_id:Uuid,pub activity_id:Uuid,pub fund_id:Uuid,pub expense_category_id:Uuid,pub planned:i64,pub adjusted:i64,pub reserved:i64,pub paid:i64}
impl Account {pub fn reference(&self)->BudgetAllocationRef{BudgetAllocationRef{project_id:self.project_id,activity_id:self.activity_id,fund_id:self.fund_id,expense_category_id:self.expense_category_id}}pub fn available(&self)->i64{self.planned+self.adjusted-self.reserved-self.paid}}
pub(crate) async fn account(tx:&mut Transaction<'_,Postgres>,r:&BudgetAllocationRef)->Result<Account,AppError>{sqlx::query_as("SELECT project_id,activity_id,fund_id,expense_category_id,planned,adjusted,reserved,paid FROM budget_allocations WHERE project_id=$1 AND activity_id=$2 AND fund_id=$3 AND expense_category_id=$4 FOR UPDATE").bind(r.project_id).bind(r.activity_id).bind(r.fund_id).bind(r.expense_category_id).fetch_optional(&mut **tx).await?.ok_or_else(||invalid("ไม่พบหมวดงบประมาณที่อนุมัติแล้ว"))}
pub(crate) async fn accounts(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<Vec<Account>,AppError>{Ok(sqlx::query_as("SELECT project_id,activity_id,fund_id,expense_category_id,planned,adjusted,reserved,paid FROM budget_allocations WHERE project_id=$1 ORDER BY activity_id,fund_id,expense_category_id").bind(id).fetch_all(&mut **tx).await?)}
#[derive(Clone,Copy,Default)]pub(crate) struct Delta{pub planned:i64,pub adjusted:i64,pub reserved:i64,pub paid:i64}
pub(crate) async fn post(tx:&mut Transaction<'_,Postgres>,actor:Uuid,r:&BudgetAllocationRef,d:Delta,kind:&str,source:Uuid)->Result<(),AppError>{
 if d.planned==0&&d.adjusted==0&&d.reserved==0&&d.paid==0{return Ok(());}
 let changed=sqlx::query("UPDATE budget_allocations SET planned=planned+$5,adjusted=adjusted+$6,reserved=reserved+$7,paid=paid+$8 WHERE project_id=$1 AND activity_id=$2 AND fund_id=$3 AND expense_category_id=$4 AND planned+$5 BETWEEN 0 AND 999999999999 AND adjusted+$6 BETWEEN -999999999999 AND 999999999999 AND reserved+$7 BETWEEN 0 AND 999999999999 AND paid+$8 BETWEEN 0 AND 999999999999 AND planned+$5+adjusted+$6 BETWEEN 0 AND 999999999999 AND planned+$5+adjusted+$6>=reserved+$7+paid+$8").bind(r.project_id).bind(r.activity_id).bind(r.fund_id).bind(r.expense_category_id).bind(d.planned).bind(d.adjusted).bind(d.reserved).bind(d.paid).execute(&mut **tx).await?;
 if changed.rows_affected()!=1{return Err(AppError::Conflict("งบประมาณคงเหลือไม่พอหรือรายการผูกพันเปลี่ยนแล้ว".into()));}
 sqlx::query("INSERT INTO budget_ledger(id,project_id,activity_id,fund_id,expense_category_id,planned_delta,adjusted_delta,reserved_delta,paid_delta,source_kind,source_id,actor_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)").bind(Uuid::now_v7()).bind(r.project_id).bind(r.activity_id).bind(r.fund_id).bind(r.expense_category_id).bind(d.planned).bind(d.adjusted).bind(d.reserved).bind(d.paid).bind(kind).bind(source).bind(actor).execute(&mut **tx).await?;Ok(())
}
pub(crate) async fn accepted(tx:&mut Transaction<'_,Postgres>,r:&ProjectRecord)->Result<BudgetPlanSnapshot,AppError>{let id=r.accepted_snapshot_id.ok_or_else(||invalid("โครงการยังไม่มีแผนที่อนุมัติ"))?;Ok(sqlx::query_scalar::<_,Json<BudgetPlanSnapshot>>("SELECT document FROM budget_plan_snapshots WHERE id=$1").bind(id).fetch_one(&mut **tx).await?.0)}
#[derive(Debug,Clone,sqlx::FromRow)]pub(crate) struct RequestRecord{pub id:Uuid,pub project_id:Uuid,pub activity_id:Uuid,pub owner_id:Uuid,pub revision:i64,pub status:String,pub document:Json<BudgetSpendingRequestInput>,pub instance_id:Option<Uuid>,pub created_at:DateTime<Utc>}
pub(crate) async fn request(pool:&PgPool,id:Uuid)->Result<RequestRecord,AppError>{sqlx::query_as("SELECT id,project_id,activity_id,owner_id,revision,status,document,instance_id,created_at FROM budget_requests WHERE id=$1").bind(id).fetch_optional(pool).await?.ok_or_else(||AppError::NotFound("ไม่พบคำขอใช้งบประมาณ".into()))}
pub(crate) async fn lock_request(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<RequestRecord,AppError>{sqlx::query_as("SELECT id,project_id,activity_id,owner_id,revision,status,document,instance_id,created_at FROM budget_requests WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::NotFound("ไม่พบคำขอใช้งบประมาณ".into()))}
pub(crate) async fn store_request(tx:&mut Transaction<'_,Postgres>,r:&RequestRecord)->Result<(),AppError>{sqlx::query("UPDATE budget_requests SET revision=$2,status=$3,document=$4,instance_id=$5,activity_id=$6 WHERE id=$1").bind(r.id).bind(r.revision).bind(&r.status).bind(&r.document).bind(r.instance_id).bind(r.activity_id).execute(&mut **tx).await?;sqlx::query("INSERT INTO budget_request_revisions(request_id,revision,document) VALUES($1,$2,$3)").bind(r.id).bind(r.revision).bind(&r.document).execute(&mut **tx).await?;Ok(())}
#[derive(Debug,Clone,sqlx::FromRow)]pub(crate) struct RequestLine{pub plan_line_id:Uuid,pub fund_id:Uuid,pub expense_category_id:Uuid,pub amount:i64,pub reserved:i64,pub paid:i64,pub description:String,pub unit:String,pub fund_name:String,pub expense_category_name:String}
pub(crate) async fn request_lines(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<Vec<RequestLine>,AppError>{Ok(sqlx::query_as("SELECT plan_line_id,fund_id,expense_category_id,amount,reserved,paid,description,unit,fund_name,expense_category_name FROM budget_request_lines WHERE request_id=$1 ORDER BY plan_line_id").bind(id).fetch_all(&mut **tx).await?)}
#[cfg(test)]mod tests{use super::*;#[test]fn statuses_and_revisions_are_closed(){assert!(parse_status("unknown").is_err());assert!(revision(1,0).is_err());assert!(!BudgetStatus::Approved.editable());assert!(BudgetStatus::Returned.editable());}}
