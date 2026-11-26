use super::queries::detail;
use super::{coverage, coverage_models::SubmitLeaveRequest};
use super::{models::*, policy::*, repository::*};
use crate::permission_codes_generated as codes;
use chrono::{NaiveDate, Utc};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_workflow::process::{ProcessSourcePort, StartProcessInput};
use school_workflow::{process, process_models::*};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;
pub async fn create(
    pool: &PgPool,
    actor: &ActorContext,
    input: CreateLeaveRequest,
) -> Result<LeaveRequestDetail, AppError> {
    let period: StoredLeavePeriod = input.period.clone().into();
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN, codes::STAFF_LEAVE_READ_SCHOOL])?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.lifecycle.create").await?;
    let (year, term) = coverage::explicit_context(&period)?;
    school_academic_timetable::services::dated_teaching::lock_context(&mut tx, year, term).await?;
    let id = Uuid::now_v7();
    if let Some(replay) = claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "request_create",
        id,
        &input,
    )
    .await?
    {
        tx.commit().await?;
        return detail(pool, actor, replay).await;
    }
    if !period.evidence_file_ids.is_empty() {
        return Err(invalid("กรุณาสร้างร่างก่อนแนบหลักฐาน"));
    }
    let typ: Json<LeaveType> = DbSession::new(&mut *tx)
        .scalar(
            "staff.leave.lifecycle.create",
            "SELECT document FROM staff_leave_types WHERE id=$1 FOR SHARE",
        )
        .bind(period.type_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| invalid("ไม่พบประเภทคำขอ"))?;
    if typ.archived {
        return Err(invalid("ประเภทคำขอถูกจัดเก็บแล้ว"));
    }
    let (cat, p) = resolve_policy(&mut tx, actor.user_id, typ.id, period.start_date).await?;
    if !p.enabled {
        return Err(invalid("เกณฑ์โรงเรียนปิดการยื่นคำขอสำหรับประเภทบุคลากรและวันที่นี้"));
    }
    let quantity = amount(&period, &p, typ.kind)?;
    let companions = super::companions::resolve_on_connection(
        &mut tx,
        actor.user_id,
        typ.kind,
        &period.companion_user_ids,
    )
    .await?;
    coverage::require_supported_journey_coverage(&mut tx, actor.user_id, typ.kind, &period).await?;
    let s = LeaveRequestSummary {
        destination: period.destination,
        companions,
        companion_revision: None,
        academic_year_id: period.academic_year_id,
        academic_term_id: period.academic_term_id,
        half_day_boundary: period.half_day_boundary,
        id,
        type_id: typ.id,
        type_name: typ.name.clone(),
        kind: typ.kind,
        owner_id: actor.user_id,
        owner_name: staff_name(&mut tx, actor.user_id).await?,
        status: LeaveStatus::Draft,
        revision: 1,
        policy_version_id: None,
        personnel_type: Some(cat),
        start_date: period.start_date,
        end_date: period.end_date,
        start_time: period.start_time,
        end_time: period.end_time,
        day_portion: period.day_portion,
        amount: quantity,
        unit: p.unit,
        reason: period.reason,
        created_at: Utc::now(),
    };
    let hash = process::payload_hash(&s)?;
    DbSession::new(&mut *tx).query("staff.leave.lifecycle.create", "INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,amount,content_hash) VALUES($1,$2,$3,'draft',1,$4,$5,$6)").bind(id).bind(actor.user_id).bind(s.type_id).bind(Json(&s)).bind(quantity).bind(&hash).execute().await?;
    let r = Record {
        document: Json(s),
        policy_id: None,
        instance_id: None,
        cancellation_id: None,
        content_hash: hash,
        evidence_file_ids: vec![],
    };
    store(&mut tx, &r).await?;
    history(&mut tx, actor, &r, "created", None, None, None).await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
pub async fn update(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: UpdateLeaveRequest,
) -> Result<LeaveRequestDetail, AppError> {
    let period: StoredLeavePeriod = input.period.clone().into();
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN, codes::STAFF_LEAVE_READ_SCHOOL])?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.lifecycle.update").await?;
    coverage::lock_updated_contexts(&mut tx, id, &period).await?;
    if claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "request_update",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return detail(pool, actor, id).await;
    }
    let mut r = load(&mut tx, id).await?;
    owner_edit(actor, &r)?;
    if r.document.revision != input.expected_revision {
        return Err(stale());
    }
    if period.type_id != r.document.type_id {
        return Err(invalid("ไม่สามารถเปลี่ยนประเภทของร่าง กรุณาสร้างใหม่"));
    }
    let (cat, p) = resolve_policy(
        &mut tx,
        actor.user_id,
        r.document.type_id,
        period.start_date,
    )
    .await?;
    super::files::validate_evidence(&mut tx, actor.user_id, id, &period.evidence_file_ids).await?;
    let n = amount(&period, &p, r.document.kind)?;
    let companions = super::companions::resolve_on_connection(
        &mut tx,
        actor.user_id,
        r.document.kind,
        &period.companion_user_ids,
    )
    .await?;
    coverage::require_supported_journey_coverage(&mut tx, actor.user_id, r.document.kind, &period)
        .await?;
    r.document.companions = companions;
    r.document.destination = period.destination;
    r.document.companion_revision = None;
    r.document.academic_year_id = period.academic_year_id;
    r.document.academic_term_id = period.academic_term_id;
    // Retain originally captured context for exact old assignment evidence.
    // Current inputs cannot set or replace this value; FullDay controls scope.
    r.document.start_date = period.start_date;
    r.document.end_date = period.end_date;
    r.document.start_time = period.start_time;
    r.document.end_time = period.end_time;
    r.document.day_portion = period.day_portion;
    r.document.reason = period.reason;
    r.document.amount = n;
    r.document.unit = p.unit;
    r.document.personnel_type = Some(cat);
    r.evidence_file_ids = period.evidence_file_ids;
    r.document.revision += 1;
    coverage::reconcile_updated_plan(&mut tx, &r).await?;
    let plan = coverage::stored_plan(&mut tx, id).await?;
    r.content_hash = process::payload_hash(&(
        &r.document,
        &r.evidence_file_ids,
        plan.as_ref().map(|p| &p.plan_hash),
    ))?;
    store(&mut tx, &r).await?;
    history(&mut tx, actor, &r, "updated", None, None, None).await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
struct LeaveSource {
    request_id: Uuid,
    owner_id: Uuid,
    source_id: Uuid,
    purpose: LeaveProcessPurpose,
}
#[async_trait::async_trait]
impl ProcessSourcePort for LeaveSource {
    fn process_owner(&self, _actor: &ActorContext) -> Uuid {
        self.owner_id
    }

    async fn validate_existing(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        i: &ProcessInstance,
    ) -> Result<(), AppError> {
        if i.source.module_code != "staff_leave"
            || i.source.resource_id != self.source_id
            || i.created_by != self.owner_id
        {
            return Err(denied());
        }
        let assigned = i
            .steps
            .iter()
            .any(|s| s.recipients.iter().any(|r| r.user_id == actor.user_id));
        if !can_read(actor, self.owner_id, assigned) {
            return Err(denied());
        }
        Ok(())
    }
    async fn validate_start(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        source: &ProcessSource,
    ) -> Result<(), AppError> {
        let r = load(tx, self.request_id).await?;
        let manager_start = actor.has_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)
            && self.purpose == LeaveProcessPurpose::Request
            && coverage::stored_plan(&mut **tx, self.request_id)
                .await?
                .is_some_and(|p| p.emergency_reason.is_some());
        if (actor.user_id != self.owner_id && !manager_start)
            || source.module_code != "staff_leave"
            || source.resource_id != self.source_id
            || source.resource_type
                != if self.purpose == LeaveProcessPurpose::Request {
                    "staff_leave_request"
                } else {
                    "staff_leave_cancellation"
                }
            || !matches!(
                r.document.status,
                LeaveStatus::Submitted | LeaveStatus::CancellationPending
            )
        {
            return Err(denied());
        }
        Ok(())
    }
    async fn validate_action(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        i: &ProcessInstance,
        r: &ProcessActionRequest,
    ) -> Result<(), AppError> {
        self.validate_existing(tx, actor, i).await?;
        actor.require_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED)?;
        if self.purpose == LeaveProcessPurpose::Request
            && matches!(r.action, ProcessAction::Review | ProcessAction::Approve)
        {
            let record = load(tx, self.request_id).await?;
            super::companions::require_ready(&mut **tx, &record.document).await?;
            if !coverage::approval_ready(&mut **tx, &record).await? {
                return Err(invalid("รอผู้สอนยืนยันแผนครบก่อนพิจารณาอนุมัติ"));
            }
        }
        if actor.user_id == self.owner_id
            || !matches!(
                r.action,
                ProcessAction::Review
                    | ProcessAction::Approve
                    | ProcessAction::Reject
                    | ProcessAction::ReturnForRevision
            )
            || !r.annotations.is_empty()
            || !r.evidence_file_ids.is_empty()
        {
            return Err(denied());
        }
        Ok(())
    }
    async fn validate_delegated_authority(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        _actor: &ActorContext,
        _i: &ProcessInstance,
    ) -> Result<(), AppError> {
        Err(invalid("ยังไม่รองรับขั้นตอนมอบอำนาจ"))
    }
    async fn completed(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        _i: &ProcessInstance,
    ) -> Result<(), AppError> {
        Ok(())
    }
}
pub(super) async fn start(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    r: &mut Record,
    p: &LeavePolicyVersion,
    key: Uuid,
    purpose: LeaveProcessPurpose,
) -> Result<(), AppError> {
    let source_id = if purpose == LeaveProcessPurpose::Request {
        r.document.id
    } else {
        r.cancellation_id.ok_or_else(|| invalid("ไม่พบคำขอยกเลิก"))?
    };
    let source = LeaveSource {
        request_id: r.document.id,
        owner_id: r.document.owner_id,
        source_id,
        purpose,
    };
    let i = process::start_in_transaction(
        tx,
        actor,
        &source,
        StartProcessInput {
            request_id: key,
            source: ProcessSource {
                module_code: "staff_leave".into(),
                resource_type: if purpose == LeaveProcessPurpose::Request {
                    "staff_leave_request"
                } else {
                    "staff_leave_cancellation"
                }
                .into(),
                resource_id: source_id,
            },
            template_id: p.template_id,
            title: if purpose == LeaveProcessPurpose::Request {
                format!("{} · {}", r.document.type_name, r.document.owner_name)
            } else {
                format!(
                    "ขอยกเลิก{} · {}",
                    r.document.type_name, r.document.owner_name
                )
            },
            action_path: format!("/staff/leave/requests/{}", r.document.id),
            step_recipients: vec![],
            due_at: None,
        },
    )
    .await
    .map_err(|error| match error {
        AppError::ValidationError(message) if purpose == LeaveProcessPurpose::Cancellation => {
            invalid(&format!(
                "เส้นทางยกเลิกไม่พร้อมใช้งาน กรุณาให้ผู้ดูแลปรับเกณฑ์ปัจจุบัน: {message}"
            ))
        }
        other => other,
    })?;
    if i.steps.iter().any(|s| {
        s.recipients
            .iter()
            .any(|a| a.user_id == r.document.owner_id)
    }) {
        return Err(invalid(
            "ผู้ขอไม่สามารถเป็นผู้ตรวจหรืออนุมัติคำขอของตนเอง กรุณาปรับเส้นทาง",
        ));
    }
    if i.steps.iter().any(|step| {
        step.recipients.iter().any(|recipient| {
            r.document
                .companions
                .iter()
                .any(|person| person.id == recipient.user_id)
        })
    }) {
        return Err(invalid(
            "ผู้ร่วมเดินทางไม่สามารถตรวจหรืออนุมัติคำขอเดินทางของตนเอง กรุณาให้ผู้ดูแลปรับเส้นทาง",
        ));
    }
    r.instance_id = Some(i.id);
    Ok(())
}
pub async fn submit(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: impl Into<SubmitLeaveRequest>,
) -> Result<LeaveRequestDetail, AppError> {
    let input = input.into();
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.lifecycle.submit").await?;
    coverage::lock_retirement_context(&mut tx, id).await?;
    if claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "request_submit",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return detail(pool, actor, id).await;
    }
    coverage::lock_request_context(&mut tx, id).await?;
    let mut r = load(&mut tx, id).await?;
    owner_edit(actor, &r)?;
    if r.document.revision != input.expected_revision {
        return Err(stale());
    }
    let typ: Json<LeaveType> = DbSession::new(&mut *tx)
        .scalar(
            "staff.leave.lifecycle.submit",
            "SELECT document FROM staff_leave_types WHERE id=$1 FOR SHARE",
        )
        .bind(r.document.type_id)
        .fetch_one()
        .await?;
    if typ.archived {
        return Err(invalid("ประเภทคำขอถูกจัดเก็บแล้ว"));
    }
    let (cat, p) = resolve_policy(
        &mut tx,
        r.document.owner_id,
        r.document.type_id,
        r.document.start_date,
    )
    .await?;
    if !p.enabled {
        return Err(invalid("เกณฑ์โรงเรียนปิดการยื่นคำขอสำหรับประเภทบุคลากรและวันที่นี้"));
    }
    require_current_record(&r.document)?;
    let quantity = amount(&period_from(&r), &p, typ.kind)?;
    if (p.evidence_required || p.evidence_from_amount.is_some_and(|n| quantity >= n))
        && r.evidence_file_ids.is_empty()
    {
        return Err(invalid("ประเภทนี้ต้องแนบหลักฐานก่อนส่ง"));
    }
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    if p.minimum_notice_days
        .is_some_and(|n| (r.document.start_date - today).num_days() < i64::from(n))
    {
        return Err(invalid("วันที่ขอไม่ครบระยะเวลาแจ้งล่วงหน้าตามเกณฑ์โรงเรียน"));
    }
    let period = period_from(&r);
    super::companions::resolve_on_connection(
        &mut tx,
        r.document.owner_id,
        typ.kind,
        &period.companion_user_ids,
    )
    .await?;
    let year = if typ.kind == LeaveKind::OfficialTravel {
        None
    } else {
        let year = quota_year(r.document.start_date, p.quota_year_start_month)?;
        check_quota(&mut tx, &r, &p, quantity, year).await?;
        Some(year)
    };
    let plan = coverage::prepare_submission(&mut tx, &r, input.emergency_reason.as_deref()).await?;
    r.document.status = LeaveStatus::Submitted;
    r.document.revision += 1;
    r.document.companion_revision =
        (!r.document.companions.is_empty()).then_some(r.document.revision);
    r.document.personnel_type = Some(cat);
    r.document.policy_version_id = Some(p.id);
    r.document.amount = quantity;
    r.document.unit = p.unit;
    r.policy_id = Some(p.id);
    let snapshots =
        super::files::validate_evidence(&mut tx, actor.user_id, id, &r.evidence_file_ids).await?;
    r.content_hash =
        process::payload_hash(&(period_from(&r), &p, cat, &snapshots, &plan.plan_hash))?;
    // Persist source state before Workflow validates it; all writes roll back together.
    DbSession::new(&mut *tx).query("staff.leave.lifecycle.submit", "UPDATE staff_leave_requests SET status='submitted',document=$2,quota_year_start=$3 WHERE id=$1").bind(id).bind(&r.document).bind(year).execute().await?;
    if input.emergency_reason.is_some() {
        r.instance_id = None;
    }
    if input.emergency_reason.is_none() {
        start(
            &mut tx,
            actor,
            &mut r,
            &p,
            input.request_id,
            LeaveProcessPurpose::Request,
        )
        .await?;
    }
    store(&mut tx, &r).await?;
    super::files::retain_snapshots(&mut tx, id, r.document.revision, &snapshots).await?;
    history(&mut tx, actor, &r, "submitted", None, None, None).await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
pub async fn act(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: LeaveActionRequest,
) -> Result<LeaveRequestDetail, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.lifecycle.act").await?;
    coverage::lock_retirement_context(&mut tx, id).await?;
    if claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "request_action",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return detail(pool, actor, id).await;
    }
    coverage::lock_action_context(&mut tx, id, input.action).await?;
    let mut r = load(&mut tx, id).await?;
    require_read(&mut tx, actor, &r).await?;
    if r.document.revision != input.expected_revision {
        return Err(stale());
    }
    let purpose = if r.document.status == LeaveStatus::CancellationPending {
        LeaveProcessPurpose::Cancellation
    } else {
        LeaveProcessPurpose::Request
    };
    if purpose != input.purpose
        || !matches!(
            r.document.status,
            LeaveStatus::Submitted | LeaveStatus::CancellationPending
        )
    {
        return Err(stale());
    }
    if input
        .comment
        .as_ref()
        .is_some_and(|c| c.chars().count() > 4000)
    {
        return Err(invalid("ความเห็นยาวเกินไป"));
    }
    let source = LeaveSource {
        request_id: id,
        owner_id: r.document.owner_id,
        source_id: if purpose == LeaveProcessPurpose::Request {
            id
        } else {
            r.cancellation_id.ok_or_else(|| invalid("ไม่พบคำขอยกเลิก"))?
        },
        purpose,
    };
    if purpose == LeaveProcessPurpose::Request
        && matches!(input.action, ProcessAction::Review | ProcessAction::Approve)
    {
        if let Some(plan) = coverage::stored_plan(&mut tx, id).await? {
            coverage::revalidate(&mut tx, &r, &plan).await?;
        }
    }
    let mut signature_snapshot = vec![];
    if let Some(s) = &input.signature {
        school_workflow::signatures::validate_content_bound_signature(
            &s.input,
            s.consent,
            r.document.revision,
            &r.content_hash,
            s.image_file_id.is_some(),
        )?;
        if let Some(file) = s.image_file_id {
            signature_snapshot
                .push(super::files::signature_snapshot(&mut tx, actor.user_id, file).await?);
        }
    }
    let i = process::act_source_in_transaction(
        &mut tx,
        actor,
        &source,
        r.instance_id.ok_or_else(|| invalid("ไม่พบกระบวนงาน"))?,
        &ProcessActionRequest {
            request_id: input.request_id,
            expected_revision: input.expected_process_revision,
            action: input.action,
            annotations: vec![],
            comment: input.comment.clone(),
            evidence_file_ids: vec![],
            signature: input.signature.as_ref().map(|s| s.input.clone()),
            reassign_user_id: None,
            reassign_from_user_id: None,
            assignment_id: None,
            routing: None,
        },
    )
    .await?;
    r.document.status = match (purpose, i.status) {
        (LeaveProcessPurpose::Request, ProcessStatus::Completed) => LeaveStatus::Approved,
        (LeaveProcessPurpose::Request, ProcessStatus::Rejected) => LeaveStatus::Rejected,
        (LeaveProcessPurpose::Request, ProcessStatus::RevisionRequested) => LeaveStatus::Returned,
        (LeaveProcessPurpose::Cancellation, ProcessStatus::Completed) => LeaveStatus::Cancelled,
        (
            LeaveProcessPurpose::Cancellation,
            ProcessStatus::Rejected | ProcessStatus::RevisionRequested,
        ) => LeaveStatus::Approved,
        _ => r.document.status,
    };
    match r.document.status {
        LeaveStatus::Approved if purpose == LeaveProcessPurpose::Request => {
            school_academic_timetable::services::dated_teaching::approve_on_connection(&mut tx, id)
                .await?
        }
        LeaveStatus::Returned | LeaveStatus::Rejected => {
            school_academic_timetable::services::dated_teaching::release_on_connection(
                &mut tx, id, false,
            )
            .await?
        }
        LeaveStatus::Cancelled => {
            school_academic_timetable::services::dated_teaching::release_on_connection(
                &mut tx, id, true,
            )
            .await?
        }
        _ => {}
    }
    r.document.revision += 1;
    store(&mut tx, &r).await?;
    super::files::retain_snapshots(&mut tx, id, r.document.revision, &signature_snapshot).await?;
    let action = serde_json::to_value(input.action).map_err(|_| invalid("การดำเนินการไม่ถูกต้อง"))?;
    history(
        &mut tx,
        actor,
        &r,
        &format!(
            "{}{}",
            if purpose == LeaveProcessPurpose::Cancellation {
                "cancellation_"
            } else {
                ""
            },
            action.as_str().unwrap_or("action")
        ),
        input.comment,
        input.signature,
        None,
    )
    .await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
pub async fn cancel(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: CancelLeaveRequest,
) -> Result<LeaveRequestDetail, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN, codes::STAFF_LEAVE_READ_SCHOOL])?;
    if input.reason.trim().is_empty() || input.reason.chars().count() > 4000 {
        return Err(invalid("กรุณาระบุเหตุผลการยกเลิก"));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.lifecycle.cancel").await?;
    coverage::lock_retirement_context(&mut tx, id).await?;
    if claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "request_cancel",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return detail(pool, actor, id).await;
    }
    let mut r = load(&mut tx, id).await?;
    if r.document.owner_id != actor.user_id {
        return Err(denied());
    }
    if r.document.revision != input.expected_revision {
        return Err(stale());
    }
    if r.document.status == LeaveStatus::Approved {
        let attempt = Uuid::now_v7();
        r.cancellation_id = Some(attempt);
        r.document.status = LeaveStatus::CancellationPending;
        r.document.revision += 1;
        let p = cancellation_route(&mut tx, &r).await?;
        r.content_hash = process::payload_hash(&(
            &r.content_hash,
            &input.reason,
            attempt,
            r.document.revision,
            &p,
        ))?;
        DbSession::new(&mut *tx).query("staff.leave.lifecycle.cancel",
            "UPDATE staff_leave_requests SET status='cancellation_pending',document=$2 WHERE id=$1",
        )
        .bind(id)
        .bind(&r.document)
        .execute()
        .await?;
        start(
            &mut tx,
            actor,
            &mut r,
            &p,
            input.request_id,
            LeaveProcessPurpose::Cancellation,
        )
        .await?;
        store(&mut tx, &r).await?;
        history(
            &mut tx,
            actor,
            &r,
            "cancellation_requested",
            Some(input.reason),
            None,
            Some(attempt),
        )
        .await?;
    } else if matches!(
        r.document.status,
        LeaveStatus::Draft | LeaveStatus::Returned | LeaveStatus::Submitted
    ) {
        if let Some(i) = instance(&mut tx, r.instance_id).await? {
            let source = LeaveSource {
                request_id: id,
                owner_id: actor.user_id,
                source_id: id,
                purpose: LeaveProcessPurpose::Request,
            };
            process::withdraw_source_in_transaction(
                &mut tx,
                actor,
                &source,
                i.id,
                input.request_id,
                i.revision,
                &input.reason,
            )
            .await?;
        }
        school_academic_timetable::services::dated_teaching::release_on_connection(
            &mut tx, id, false,
        )
        .await?;
        r.document.status = LeaveStatus::Cancelled;
        r.document.revision += 1;
        store(&mut tx, &r).await?;
        history(
            &mut tx,
            actor,
            &r,
            "withdrawn",
            Some(input.reason),
            None,
            None,
        )
        .await?;
    } else {
        return Err(stale());
    }
    tx.commit().await?;
    detail(pool, actor, id).await
}
async fn check_quota(
    tx: &mut Transaction<'_, Postgres>,
    r: &Record,
    p: &LeavePolicyVersion,
    quantity: i64,
    year: NaiveDate,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx)
        .query(
            "staff.leave.lifecycle.submit",
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        )
        .bind(format!(
            "staff_leave:{}:{}:{}",
            r.document.owner_id, r.document.type_id, year
        ))
        .execute()
        .await?;
    let (used,compatible):(String,bool)=DbSession::new(&mut **tx).query_as("staff.leave.lifecycle.submit_quota", "SELECT coalesce(sum(amount::numeric * CASE document->>'unit' WHEN 'day' THEN 2 ELSE 1 END),0)::text,coalesce(bool_and(CASE WHEN $5 THEN document->>'unit' IN ('day','half_day') ELSE document->>'unit'='minute' END),true) FROM staff_leave_requests WHERE owner_id=$1 AND type_id=$2 AND quota_year_start=$3 AND status IN ('submitted','approved','cancellation_pending') AND id<>$4").bind(r.document.owner_id).bind(r.document.type_id).bind(year).bind(r.document.id).bind(p.unit==LeaveUnit::Day).fetch_one().await?;
    if !compatible {
        return Err(invalid(
            "หน่วยในบัญชีสิทธิ์เดิมไม่เข้ากัน กรุณาให้ผู้ดูแลตรวจสอบโดยไม่แปลงวันเป็นนาที",
        ));
    }
    let used = used
        .parse::<i128>()
        .map_err(|_| invalid("ยอดสิทธิ์เกินขอบเขตที่รองรับ"))?;
    let total = used
        .checked_add(i128::from(quantity) * quota_scale(p.unit))
        .ok_or_else(|| invalid("ยอดสิทธิ์เกินขอบเขตที่รองรับ"))?;
    if p.annual_limit
        .is_some_and(|cap| total > i128::from(cap) * quota_scale(p.unit))
    {
        return Err(invalid("ยอดอนุมัติและคำขอที่รอพิจารณารวมกันเกินสิทธิ์ตามเกณฑ์โรงเรียน"));
    }
    Ok(())
}

pub(super) async fn return_for_coverage_revision(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    r: &Record,
    key: Uuid,
) -> Result<(), AppError> {
    if let Some(id) = r.instance_id {
        let source = LeaveSource {
            request_id: r.document.id,
            owner_id: r.document.owner_id,
            source_id: r.document.id,
            purpose: LeaveProcessPurpose::Request,
        };
        process::return_source_for_revision_in_transaction(tx, actor, &source, id, key).await?;
    }
    Ok(())
}
