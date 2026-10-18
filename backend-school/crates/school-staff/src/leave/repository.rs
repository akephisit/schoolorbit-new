use super::{models::*, policy::*};
use crate::career::StaffPersonnelType;
use chrono::{NaiveDate, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::{process, process_models::*};
use serde::Serialize;
use sqlx::{types::Json, Postgres, Transaction};
use uuid::Uuid;
#[derive(sqlx::FromRow)]
pub(super) struct Record {
    pub(super) document: Json<LeaveRequestSummary>,
    pub(super) policy_id: Option<Uuid>,
    pub(super) instance_id: Option<Uuid>,
    pub(super) cancellation_id: Option<Uuid>,
    pub(super) content_hash: String,
    pub(super) evidence_file_ids: Vec<Uuid>,
}
pub(super) async fn load(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Record, AppError> {
    sqlx::query_as("SELECT document,policy_id,instance_id,cancellation_id,content_hash,evidence_file_ids FROM staff_leave_requests WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::NotFound("ไม่พบคำขอ".into()))
}
pub(super) async fn claim<T: Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    key: Uuid,
    op: &str,
    id: Uuid,
    payload: &T,
) -> Result<Option<Uuid>, AppError> {
    let hash = process::payload_hash(&(op, payload))?;
    let inserted:Option<Uuid>=sqlx::query_scalar("INSERT INTO staff_leave_receipts(actor_id,request_id,operation,payload_hash,resource_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING RETURNING resource_id").bind(actor).bind(key).bind(op).bind(&hash).bind(id).fetch_optional(&mut **tx).await?;
    if inserted.is_some() {
        return Ok(None);
    }
    let old:(String,String,Uuid)=sqlx::query_as("SELECT operation,payload_hash,resource_id FROM staff_leave_receipts WHERE actor_id=$1 AND request_id=$2").bind(actor).bind(key).fetch_one(&mut **tx).await?;
    if old.0 != op || old.1 != hash {
        return Err(AppError::Conflict("รหัสคำขอถูกใช้กับข้อมูลอื่นแล้ว".into()));
    }
    Ok(Some(old.2))
}
pub(super) async fn staff_name(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
) -> Result<String, AppError> {
    sqlx::query_scalar("SELECT display_name FROM user_display_names WHERE id=$1 AND user_type='staff' AND status='active'").bind(actor).fetch_optional(&mut **tx).await?.ok_or_else(denied)
}
pub(super) async fn category(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    date: NaiveDate,
) -> Result<Option<StaffPersonnelType>, AppError> {
    let row:Option<Option<String>>=sqlx::query_scalar("SELECT personnel_type FROM staff_career_history WHERE user_id=$1 AND kind='personnel_type' AND (effective_date<=$2 OR effective_date IS NULL) ORDER BY effective_date DESC NULLS LAST,created_at DESC,id DESC LIMIT 1").bind(actor).bind(date).fetch_optional(&mut **tx).await?;
    row.flatten()
        .map(|v| v.parse().map_err(|_| invalid("ประเภทบุคลากรไม่ถูกต้อง")))
        .transpose()
}
pub(super) async fn preview_policy(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    type_id: Uuid,
    date: NaiveDate,
) -> Result<Option<(StaffPersonnelType, LeavePolicyVersion)>, AppError> {
    let Some(kind) = category(tx, owner, date).await? else {
        return Ok(None);
    };
    let row:Option<Json<LeavePolicyVersion>>=sqlx::query_scalar("SELECT p.document FROM staff_leave_policies p JOIN staff_leave_policy_categories c ON c.policy_id=p.id WHERE c.type_id=$1 AND c.personnel_type=$2 AND c.effective_from<=$3 ORDER BY p.effective_from DESC,p.version DESC LIMIT 1").bind(type_id).bind(kind.as_str()).bind(date).fetch_optional(&mut **tx).await?;
    Ok(row.map(|p| (kind, p.0)))
}
pub(super) async fn resolve_policy(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    type_id: Uuid,
    date: NaiveDate,
) -> Result<(StaffPersonnelType, LeavePolicyVersion), AppError> {
    preview_policy(tx, owner, type_id, date)
        .await?
        .ok_or_else(|| invalid("กรุณาตรวจประเภทบุคลากรและเกณฑ์ที่มีผลสำหรับวันที่นี้ก่อนยื่นคำขอ"))
}
pub(super) async fn pinned_policy(
    tx: &mut Transaction<'_, Postgres>,
    id: Option<Uuid>,
) -> Result<Option<LeavePolicyVersion>, AppError> {
    let Some(id) = id else {
        return Ok(None);
    };
    Ok(sqlx::query_scalar::<_, Json<LeavePolicyVersion>>(
        "SELECT document FROM staff_leave_policies WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|p| p.0))
}
pub(super) async fn instance(
    tx: &mut Transaction<'_, Postgres>,
    id: Option<Uuid>,
) -> Result<Option<ProcessInstance>, AppError> {
    let Some(id) = id else {
        return Ok(None);
    };
    Ok(sqlx::query_scalar::<_, Json<ProcessInstance>>(
        "SELECT definition FROM process_instances WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|p| p.0))
}
pub(super) async fn require_read(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    r: &Record,
) -> Result<(), AppError> {
    let assigned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM process_participants p JOIN process_instances i ON i.id=p.instance_id WHERE p.user_id=$1 AND i.source_module='staff_leave' AND (i.source_id=$2 OR EXISTS(SELECT 1 FROM staff_leave_history h WHERE h.request_id=$2 AND h.entry->>'action'='cancellation_requested' AND i.source_id=(h.entry->>'id')::uuid)))").bind(actor.user_id).bind(r.document.id).fetch_one(&mut **tx).await?;
    if !can_read(actor, r.document.owner_id, assigned) {
        return Err(denied());
    }
    Ok(())
}
pub(super) fn owner_edit(actor: &ActorContext, r: &Record) -> Result<(), AppError> {
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN, codes::STAFF_LEAVE_READ_SCHOOL])?;
    if r.document.owner_id != actor.user_id
        || !matches!(
            r.document.status,
            LeaveStatus::Draft | LeaveStatus::Returned
        )
    {
        return Err(denied());
    }
    Ok(())
}
pub(super) async fn store(tx: &mut Transaction<'_, Postgres>, r: &Record) -> Result<(), AppError> {
    let status = serde_json::to_value(r.document.status).map_err(|_| invalid("สถานะไม่ถูกต้อง"))?;
    sqlx::query("UPDATE staff_leave_requests SET document=$2,status=$3,revision=$4,policy_id=$5,instance_id=$6,cancellation_id=$7,amount=$8,content_hash=$9,evidence_file_ids=$10 WHERE id=$1").bind(r.document.id).bind(&r.document).bind(status.as_str()).bind(r.document.revision).bind(r.policy_id).bind(r.instance_id).bind(r.cancellation_id).bind(r.document.amount).bind(&r.content_hash).bind(&r.evidence_file_ids).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO staff_leave_revisions(request_id,revision,document,content_hash) VALUES($1,$2,$3,$4)").bind(r.document.id).bind(r.document.revision).bind(&r.document).bind(&r.content_hash).execute(&mut **tx).await?;
    Ok(())
}
pub(super) async fn history(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    r: &Record,
    action: &str,
    comment: Option<String>,
    signature: Option<LeaveSignatureConsent>,
    event_id: Option<Uuid>,
) -> Result<(), AppError> {
    let e = LeaveHistoryEntry {
        id: event_id.unwrap_or_else(Uuid::now_v7),
        actor_id: actor.user_id,
        actor_name: staff_name(tx, actor.user_id).await?,
        action: action.into(),
        revision: r.document.revision,
        comment,
        signature,
        content_hash: r.content_hash.clone(),
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO staff_leave_history(id,request_id,actor_id,entry) VALUES($1,$2,$3,$4)",
    )
    .bind(e.id)
    .bind(r.document.id)
    .bind(actor.user_id)
    .bind(Json(e))
    .execute(&mut **tx)
    .await?;
    Ok(())
}
pub(super) fn period_from(r: &Record) -> LeavePeriodInput {
    LeavePeriodInput {
        type_id: r.document.type_id,
        start_date: r.document.start_date,
        end_date: r.document.end_date,
        start_time: r.document.start_time,
        end_time: r.document.end_time,
        day_portion: r.document.day_portion,
        reason: r.document.reason.clone(),
        evidence_file_ids: r.evidence_file_ids.clone(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leave_draft_owner_cannot_edit_other_or_approved() {
        let id = Uuid::new_v4();
        let a = ActorContext {
            user_id: id,
            permissions: vec![
                codes::STAFF_LEAVE_CREATE_OWN.into(),
                codes::STAFF_LEAVE_READ_OWN.into(),
            ],
        };
        let mut r = Record {
            document: Json(LeaveRequestSummary {
                id: Uuid::new_v4(),
                type_id: Uuid::new_v4(),
                type_name: "test".into(),
                kind: LeaveKind::Leave,
                owner_id: id,
                owner_name: "test".into(),
                status: LeaveStatus::Draft,
                revision: 1,
                policy_version_id: None,
                personnel_type: None,
                start_date: Utc::now().date_naive(),
                end_date: Utc::now().date_naive(),
                start_time: None,
                end_time: None,
                day_portion: LeaveDayPortion::FullDay,
                amount: 1,
                unit: LeaveUnit::Day,
                reason: "test".into(),
                created_at: Utc::now(),
            }),
            policy_id: None,
            instance_id: None,
            cancellation_id: None,
            content_hash: String::new(),
            evidence_file_ids: vec![],
        };
        assert!(owner_edit(&a, &r).is_ok());
        r.document.status = LeaveStatus::Approved;
        assert!(owner_edit(&a, &r).is_err());
        r.document.status = LeaveStatus::Draft;
        r.document.owner_id = Uuid::new_v4();
        assert!(owner_edit(&a, &r).is_err());
    }
}
