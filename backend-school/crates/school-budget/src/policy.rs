//! Scope predicates are unions. Organization relationships are resolved by Authorization.
use crate::{models::*,repository::{self,ProjectRecord}};
use school_authorization::{ActorContext,resource_access_policy::{accessible_organization_unit_ids,UserResourceListAccess}};
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;
#[derive(Debug,Clone,Default)]pub(crate) struct Scope {pub school:bool,pub own:bool,pub assigned:bool,pub units:Vec<Uuid>}
impl Scope{pub fn any(&self)->bool{self.school||self.own||self.assigned||!self.units.is_empty()}}
pub(crate) async fn read_scope(pool:&PgPool,a:&ActorContext,export:bool)->Result<Scope,AppError>{
 let (school,own,assigned,unit,tree)=if export {(codes::BUDGET_DOWNLOAD_SCHOOL,codes::BUDGET_DOWNLOAD_OWN,None,codes::BUDGET_DOWNLOAD_ORGANIZATION_UNIT,codes::BUDGET_DOWNLOAD_ORGANIZATION_TREE)}else{(codes::BUDGET_READ_SCHOOL,codes::BUDGET_READ_OWN,Some(codes::BUDGET_READ_ASSIGNED),codes::BUDGET_READ_ORGANIZATION_UNIT,codes::BUDGET_READ_ORGANIZATION_TREE)};
 let mut s=Scope{school:a.has_permission(school),own:a.has_permission(own),assigned:assigned.is_some_and(|p|a.has_permission(p)),units:vec![]};
 if !s.school{if a.has_permission(unit){s.units.extend(accessible_organization_unit_ids(pool,UserResourceListAccess::OrganizationUnit(a.user_id)).await?.unwrap_or_default());}if a.has_permission(tree){s.units.extend(accessible_organization_unit_ids(pool,UserResourceListAccess::OrganizationTree(a.user_id)).await?.unwrap_or_default());}s.units.sort();s.units.dedup();}if !s.any(){return Err(repository::denied());}Ok(s)
}
pub(crate) async fn update_scope(pool:&PgPool,a:&ActorContext)->Result<Scope,AppError>{
 let mut s=Scope{school:a.has_permission(codes::BUDGET_UPDATE_SCHOOL),own:a.has_permission(codes::BUDGET_UPDATE_OWN),assigned:false,units:vec![]};
 if !s.school{if a.has_permission(codes::BUDGET_UPDATE_ORGANIZATION_UNIT){s.units.extend(accessible_organization_unit_ids(pool,UserResourceListAccess::OrganizationUnit(a.user_id)).await?.unwrap_or_default());}if a.has_permission(codes::BUDGET_UPDATE_ORGANIZATION_TREE){s.units.extend(accessible_organization_unit_ids(pool,UserResourceListAccess::OrganizationTree(a.user_id)).await?.unwrap_or_default());}s.units.sort();s.units.dedup();}Ok(s)
}
pub(crate) fn direct(s:&Scope,actor:Uuid,owner:Uuid,unit:Uuid)->bool{s.school||(s.own&&actor==owner)||s.units.contains(&unit)}
pub(crate) async fn assigned_project(pool:&PgPool,actor:Uuid,id:Uuid)->Result<bool,AppError>{Ok(sqlx::query_scalar(ASSIGNED_PROJECT_SQL).bind(actor).bind(id).fetch_one(pool).await?)}
pub(crate) async fn require_read(pool:&PgPool,a:&ActorContext,r:&ProjectRecord)->Result<(),AppError>{let s=read_scope(pool,a,false).await?;if direct(&s,a.user_id,r.owner_id,r.document.project.organization_unit_id)||(s.assigned&&assigned_project(pool,a.user_id,r.id).await?){Ok(())}else{Err(repository::denied())}}
pub(crate) async fn require_update(pool:&PgPool,a:&ActorContext,r:&ProjectRecord)->Result<(),AppError>{let s=update_scope(pool,a).await?;if direct(&s,a.user_id,r.owner_id,r.document.project.organization_unit_id){Ok(())}else{Err(repository::denied())}}
pub(crate) async fn require_create_unit(pool:&PgPool,a:&ActorContext,unit:Uuid)->Result<(),AppError>{a.require_permission(codes::BUDGET_CREATE_OWN)?;let s=update_scope(pool,a).await?;let member=accessible_organization_unit_ids(pool,UserResourceListAccess::OrganizationUnit(a.user_id)).await?.unwrap_or_default();if s.school||s.units.contains(&unit)||member.contains(&unit){Ok(())}else{Err(repository::denied())}}
pub(crate) fn can_act(a:&ActorContext,instance:Option<&school_workflow::process_models::ProcessInstance>,owner:Uuid)->bool{a.user_id!=owner&&a.has_permission(codes::BUDGET_APPROVE_ASSIGNED)&&instance.is_some_and(|i|i.status==school_workflow::process_models::ProcessStatus::Active&&i.steps.get(i.current_step).is_some_and(|s|s.recipients.iter().any(|r|r.user_id==a.user_id&&r.completed_at.is_none())))}
pub(crate) fn capabilities(a:&ActorContext,editable:bool,act:bool,accepted:bool)->BudgetCapabilities{BudgetCapabilities{can_edit:editable,can_submit:editable,can_act:act,can_amend:editable&&accepted,can_create_request:accepted&&a.has_permission(codes::BUDGET_CREATE_OWN),can_pay:a.has_permission(codes::BUDGET_PAYMENT_EXECUTE_SCHOOL),can_close:a.has_permission(codes::BUDGET_PAYMENT_EXECUTE_SCHOOL),can_adjust:a.has_permission(codes::BUDGET_ADJUSTMENT_CREATE_SCHOOL),can_manage_settings:a.has_permission(codes::BUDGET_MANAGE_SCHOOL),can_export:a.has_any_permission(&[codes::BUDGET_DOWNLOAD_OWN,codes::BUDGET_DOWNLOAD_ORGANIZATION_UNIT,codes::BUDGET_DOWNLOAD_ORGANIZATION_TREE,codes::BUDGET_DOWNLOAD_SCHOOL])}}
#[cfg(test)]mod tests{use super::*;#[test]fn independent_scopes_form_a_union(){let actor=Uuid::now_v7();let unit=Uuid::now_v7();let s=Scope{own:true,units:vec![unit],..Default::default()};assert!(direct(&s,actor,actor,Uuid::now_v7()));assert!(direct(&s,actor,Uuid::now_v7(),unit));assert!(!direct(&s,actor,Uuid::now_v7(),Uuid::now_v7()));}}

const ASSIGNED_PROJECT_SQL:&str="SELECT EXISTS(SELECT 1 FROM process_participants pp JOIN process_instances i ON i.id=pp.instance_id JOIN budget_projects p ON p.id=$2 WHERE pp.user_id=$1 AND i.source_module='budget' AND ((i.source_type='budget_project_plan' AND EXISTS(SELECT 1 FROM budget_project_attempts attempt WHERE attempt.project_id=p.id AND attempt.id=i.source_id)) OR (i.source_type='budget_request' AND EXISTS(SELECT 1 FROM budget_requests r WHERE r.project_id=p.id AND r.id=i.source_id)) OR (i.source_type='budget_adjustment' AND EXISTS(SELECT 1 FROM budget_adjustments a WHERE (a.from_project_id=p.id OR a.to_project_id=p.id) AND a.id=i.source_id))))";

pub(crate) async fn require_read_in_transaction(tx:&mut sqlx::Transaction<'_,sqlx::Postgres>,a:&ActorContext,r:&ProjectRecord,s:&Scope)->Result<(),AppError>{
 if direct(s,a.user_id,r.owner_id,r.document.project.organization_unit_id){return Ok(());}
 if s.assigned&&sqlx::query_scalar::<_,bool>(ASSIGNED_PROJECT_SQL).bind(a.user_id).bind(r.id).fetch_one(&mut **tx).await?{return Ok(());}Err(repository::denied())
}
pub(crate) fn require_update_with_scope(a:&ActorContext,r:&ProjectRecord,s:&Scope)->Result<(),AppError>{if direct(s,a.user_id,r.owner_id,r.document.project.organization_unit_id){Ok(())}else{Err(repository::denied())}}
