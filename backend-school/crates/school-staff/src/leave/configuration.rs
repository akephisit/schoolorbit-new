use super::{models::*, policy::*, repository::*};
use chrono::{NaiveDate, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::process;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;
pub async fn list_types(
    pool: &PgPool,
    actor: &ActorContext,
    q: LeaveListQuery,
) -> Result<Vec<LeaveType>, AppError> {
    actor.require_any_permission(&[
        codes::STAFF_LEAVE_READ_OWN,
        codes::STAFF_LEAVE_READ_ASSIGNED,
        codes::STAFF_LEAVE_READ_SCHOOL,
        codes::STAFF_LEAVE_CREATE_OWN,
        codes::STAFF_LEAVE_MANAGE_SCHOOL,
    ])?;
    if q.include_archived == Some(true) {
        actor.require_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)?;
    }
    let kind = q.kind.map(|k| {
        if k == LeaveKind::Leave {
            "leave"
        } else {
            "exit"
        }
    });
    Ok(sqlx::query_scalar::<_,Json<LeaveType>>("SELECT document FROM staff_leave_types WHERE ($1::text IS NULL OR kind=$1) AND ($2 OR document->>'archived'='false') ORDER BY code,id LIMIT 200").bind(kind).bind(q.include_archived.unwrap_or(false)).fetch_all(pool).await?.into_iter().map(|r|r.0).collect())
}
pub async fn save_type(
    pool: &PgPool,
    actor: &ActorContext,
    id: Option<Uuid>,
    input: SaveLeaveTypeRequest,
) -> Result<LeaveType, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)?;
    if input.code.as_ref().is_some_and(|code| {
        code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    }) {
        return Err(invalid("รหัสประเภทคำขอไม่ถูกต้อง"));
    }
    if input.name.trim().is_empty() || input.name.chars().count() > 200 {
        return Err(invalid("กรุณาระบุชื่อประเภทคำขอไม่เกิน 200 ตัวอักษร"));
    }
    let mut tx = pool.begin().await?;
    let rid = id.unwrap_or_else(Uuid::now_v7);
    if let Some(replay) = claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        if id.is_some() {
            "type_update"
        } else {
            "type_create"
        },
        rid,
        &(id, &input),
    )
    .await?
    {
        let old: Json<LeaveType> =
            sqlx::query_scalar("SELECT document FROM staff_leave_types WHERE id=$1")
                .bind(replay)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        return Ok(old.0);
    }
    let (revision, code) = if id.is_some() {
        let old: Json<LeaveType> =
            sqlx::query_scalar("SELECT document FROM staff_leave_types WHERE id=$1 FOR UPDATE")
                .bind(rid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| invalid("ไม่พบประเภทคำขอ"))?;
        if input.expected_revision != Some(old.revision) {
            return Err(stale());
        }
        if input.code.as_ref().is_some_and(|code| code != &old.code) || old.kind != input.kind {
            return Err(invalid("ไม่สามารถเปลี่ยนรหัสหรือกลุ่มคำขอ"));
        }
        (old.revision + 1, old.code.clone())
    } else {
        (
            1,
            input
                .code
                .unwrap_or_else(|| format!("type_{}", rid.simple())),
        )
    };
    let item = LeaveType {
        id: rid,
        code,
        name: input.name,
        kind: input.kind,
        archived: input.archived,
        revision,
    };
    sqlx::query("INSERT INTO staff_leave_types(id,code,kind,revision,document) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,document=excluded.document").bind(rid).bind(&item.code).bind(if item.kind==LeaveKind::Leave{"leave"}else{"exit"}).bind(revision).bind(Json(&item)).execute(&mut *tx).await.map_err(|error|{
        if error.as_database_error().is_some_and(|database|database.constraint()==Some("staff_leave_types_code_key")){
            AppError::Conflict("รหัสประเภทคำขอนี้มีอยู่แล้ว กรุณาใช้รหัสอื่น".into())
        }else{error.into()}
    })?;
    tx.commit().await?;
    Ok(item)
}
pub async fn delete_type(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: LeaveMutationRequest,
) -> Result<(), AppError> {
    actor.require_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)?;
    let mut tx = pool.begin().await?;
    if claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "type_delete",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return Ok(());
    }
    let rev: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM staff_leave_types WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if rev != Some(input.expected_revision) {
        return Err(stale());
    }
    let used:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_leave_policies WHERE type_id=$1) OR EXISTS(SELECT 1 FROM staff_leave_requests WHERE type_id=$1)").bind(id).fetch_one(&mut *tx).await?;
    if used {
        return Err(invalid("ประเภทนี้มีการใช้งานแล้ว กรุณาจัดเก็บแทนการลบ"));
    }
    sqlx::query("DELETE FROM staff_leave_types WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn policies(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<Vec<LeavePolicyVersion>, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)?;
    Ok(sqlx::query_scalar::<_,Json<LeavePolicyVersion>>("SELECT document FROM staff_leave_policies WHERE type_id=$1 ORDER BY version DESC LIMIT 200").bind(id).fetch_all(pool).await?.into_iter().map(|r|r.0).collect())
}
pub async fn effective_policy(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    date: NaiveDate,
) -> Result<LeavePolicyVersion, AppError> {
    actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN, codes::STAFF_LEAVE_CREATE_OWN])?;
    let mut tx = pool.begin().await?;
    let (_, p) = resolve_policy(&mut tx, actor.user_id, id, date).await?;
    tx.commit().await?;
    Ok(p)
}
pub async fn save_policy(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: SaveLeavePolicyRequest,
) -> Result<LeavePolicyVersion, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)?;
    validate_policy(&input)?;
    let mut tx = pool.begin().await?;
    let pid = Uuid::now_v7();
    if let Some(replay) = claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "policy_create",
        pid,
        &(id, &input),
    )
    .await?
    {
        let p = pinned_policy(&mut tx, Some(replay))
            .await?
            .ok_or_else(|| invalid("ไม่พบเกณฑ์"))?;
        tx.commit().await?;
        return Ok(p);
    }
    let old: Json<LeaveType> =
        sqlx::query_scalar("SELECT document FROM staff_leave_types WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| invalid("ไม่พบประเภทคำขอ"))?;
    if old.revision != input.expected_revision {
        return Err(stale());
    }
    let incompatible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_leave_policies WHERE type_id=$1 AND (document->>'unit'<>$2 OR (document->>'quotaYearStartMonth')::int<>$3))").bind(id).bind(serde_json::to_value(input.unit).map_err(|_|invalid("หน่วยไม่ถูกต้อง"))?.as_str()).bind(input.quota_year_start_month as i32).fetch_one(&mut *tx).await?;
    if incompatible {
        return Err(invalid(
            "ไม่สามารถเปลี่ยนหน่วยหรือเดือนเริ่มปีของประเภทที่มีเกณฑ์แล้ว กรุณาสร้างประเภทใหม่",
        ));
    }
    let version: i64 = sqlx::query_scalar(
        "SELECT coalesce(max(version),0)+1 FROM staff_leave_policies WHERE type_id=$1",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let p = LeavePolicyVersion {
        id: pid,
        type_id: id,
        version,
        type_revision: old.revision + 1,
        effective_from: input.effective_from,
        source_reference: input.source_reference,
        enabled: input.enabled,
        personnel_types: input.personnel_types,
        unit: input.unit,
        counting_mode: input.counting_mode,
        working_weekdays: input.working_weekdays,
        excluded_dates: input.excluded_dates,
        quota_year_start_month: input.quota_year_start_month,
        max_per_request: input.max_per_request,
        annual_limit: input.annual_limit,
        evidence_required: input.evidence_required,
        evidence_from_amount: input.evidence_from_amount,
        minimum_notice_days: input.minimum_notice_days,
        steps: input.steps,
        template_id: Uuid::now_v7(),
        created_at: Utc::now(),
    };
    process::create_source_template_in_transaction(
        &mut tx,
        actor.user_id,
        p.template_id,
        "staff_leave",
        &old.name,
        p.steps.clone(),
    )
    .await?;
    sqlx::query("INSERT INTO staff_leave_policies(id,type_id,version,effective_from,document,created_by) VALUES($1,$2,$3,$4,$5,$6)").bind(pid).bind(id).bind(version).bind(p.effective_from).bind(Json(&p)).bind(actor.user_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO staff_leave_policy_categories(policy_id,type_id,personnel_type,effective_from) SELECT $1,$2,unnest($3::text[]),$4").bind(pid).bind(id).bind(p.personnel_types.iter().map(|t|t.as_str()).collect::<Vec<_>>()).bind(p.effective_from).execute(&mut *tx).await?;
    let mut typ = old.0;
    typ.revision += 1;
    sqlx::query("UPDATE staff_leave_types SET revision=$2,document=$3 WHERE id=$1")
        .bind(id)
        .bind(typ.revision)
        .bind(Json(typ))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(p)
}
