use super::queries::detail;
use super::{models::*, policy::*, repository::*};
use chrono::{NaiveDate, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::process::{ProcessSourcePort, StartProcessInput};
use school_workflow::{process, process_models::*};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;
pub async fn create(
    pool: &PgPool,
    actor: &ActorContext,
    input: CreateLeaveRequest,
) -> Result<LeaveRequestDetail, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN, codes::STAFF_LEAVE_READ_SCHOOL])?;
    let mut tx = pool.begin().await?;
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
    if !input.period.evidence_file_ids.is_empty() {
        return Err(invalid("กรุณาสร้างร่างก่อนแนบหลักฐาน"));
    }
    let typ: Json<LeaveType> =
        sqlx::query_scalar("SELECT document FROM staff_leave_types WHERE id=$1 FOR SHARE")
            .bind(input.period.type_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| invalid("ไม่พบประเภทคำขอ"))?;
    if typ.archived {
        return Err(invalid("ประเภทคำขอถูกจัดเก็บแล้ว"));
    }
    let (cat, p) = resolve_policy(&mut tx, actor.user_id, typ.id, input.period.start_date).await?;
    if !p.enabled {
        return Err(invalid("เกณฑ์โรงเรียนปิดการยื่นคำขอสำหรับประเภทบุคลากรและวันที่นี้"));
    }
    let quantity = amount(&input.period, &p)?;
    let s = LeaveRequestSummary {
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
        start_date: input.period.start_date,
        end_date: input.period.end_date,
        start_time: input.period.start_time,
        end_time: input.period.end_time,
        day_portion: input.period.day_portion,
        amount: quantity,
        unit: p.unit,
        reason: input.period.reason,
        created_at: Utc::now(),
    };
    let hash = process::payload_hash(&s)?;
    sqlx::query("INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,amount,content_hash) VALUES($1,$2,$3,'draft',1,$4,$5,$6)").bind(id).bind(actor.user_id).bind(s.type_id).bind(Json(&s)).bind(quantity).bind(&hash).execute(&mut *tx).await?;
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
    let mut tx = pool.begin().await?;
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
    if input.period.type_id != r.document.type_id {
        return Err(invalid("ไม่สามารถเปลี่ยนประเภทของร่าง กรุณาสร้างใหม่"));
    }
    let (cat, p) = resolve_policy(
        &mut tx,
        actor.user_id,
        r.document.type_id,
        input.period.start_date,
    )
    .await?;
    super::files::validate_evidence(&mut tx, actor.user_id, id, &input.period.evidence_file_ids)
        .await?;
    let n = amount(&input.period, &p)?;
    r.document.start_date = input.period.start_date;
    r.document.end_date = input.period.end_date;
    r.document.start_time = input.period.start_time;
    r.document.end_time = input.period.end_time;
    r.document.day_portion = input.period.day_portion;
    r.document.reason = input.period.reason;
    r.document.amount = n;
    r.document.unit = p.unit;
    r.document.personnel_type = Some(cat);
    r.evidence_file_ids = input.period.evidence_file_ids;
    r.document.revision += 1;
    r.content_hash = process::payload_hash(&(&r.document, &r.evidence_file_ids))?;
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
        if actor.user_id != self.owner_id
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
async fn start(
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
    .await?;
    if i.steps.iter().any(|s| {
        s.recipients
            .iter()
            .any(|a| a.user_id == r.document.owner_id)
    }) {
        return Err(invalid(
            "ผู้ขอไม่สามารถเป็นผู้ตรวจหรืออนุมัติคำขอของตนเอง กรุณาปรับเส้นทาง",
        ));
    }
    r.instance_id = Some(i.id);
    Ok(())
}
pub async fn submit(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: LeaveMutationRequest,
) -> Result<LeaveRequestDetail, AppError> {
    let mut tx = pool.begin().await?;
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
    let mut r = load(&mut tx, id).await?;
    owner_edit(actor, &r)?;
    if r.document.revision != input.expected_revision {
        return Err(stale());
    }
    let typ: Json<LeaveType> =
        sqlx::query_scalar("SELECT document FROM staff_leave_types WHERE id=$1 FOR SHARE")
            .bind(r.document.type_id)
            .fetch_one(&mut *tx)
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
    let quantity = amount(&period_from(&r), &p)?;
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
    let year = quota_year(r.document.start_date, p.quota_year_start_month)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("staff_leave_owner:{}", r.document.owner_id))
        .execute(&mut *tx)
        .await?;
    check_overlaps(&mut tx, &r, &p).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "staff_leave:{}:{}:{}",
            r.document.owner_id, r.document.type_id, year
        ))
        .execute(&mut *tx)
        .await?;
    let used:i64=sqlx::query_scalar("SELECT coalesce(sum(amount),0)::bigint FROM staff_leave_requests WHERE owner_id=$1 AND type_id=$2 AND quota_year_start=$3 AND status IN ('submitted','approved','cancellation_pending') AND id<>$4").bind(r.document.owner_id).bind(r.document.type_id).bind(year).bind(id).fetch_one(&mut *tx).await?;
    if p.annual_limit.is_some_and(|cap| used + quantity > cap) {
        return Err(invalid("ยอดอนุมัติและคำขอที่รอพิจารณารวมกันเกินสิทธิ์ตามเกณฑ์โรงเรียน"));
    }
    r.document.status = LeaveStatus::Submitted;
    r.document.revision += 1;
    r.document.personnel_type = Some(cat);
    r.document.policy_version_id = Some(p.id);
    r.document.amount = quantity;
    r.document.unit = p.unit;
    r.policy_id = Some(p.id);
    let snapshots =
        super::files::validate_evidence(&mut tx, actor.user_id, id, &r.evidence_file_ids).await?;
    r.content_hash = process::payload_hash(&(period_from(&r), &p, cat, &snapshots))?;
    // Persist source state before Workflow validates it; all writes roll back together.
    sqlx::query("UPDATE staff_leave_requests SET status='submitted',document=$2,quota_year_start=$3 WHERE id=$1").bind(id).bind(&r.document).bind(year).execute(&mut *tx).await?;
    start(
        &mut tx,
        actor,
        &mut r,
        &p,
        input.request_id,
        LeaveProcessPurpose::Request,
    )
    .await?;
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
    let mut tx = pool.begin().await?;
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
    let mut tx = pool.begin().await?;
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
        r.content_hash =
            process::payload_hash(&(&r.content_hash, &input.reason, attempt, r.document.revision))?;
        sqlx::query(
            "UPDATE staff_leave_requests SET status='cancellation_pending',document=$2 WHERE id=$1",
        )
        .bind(id)
        .bind(&r.document)
        .execute(&mut *tx)
        .await?;
        let p = pinned_policy(&mut tx, r.policy_id)
            .await?
            .ok_or_else(|| invalid("ไม่พบเกณฑ์เดิม"))?;
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
fn periods_overlap(
    a: &LeavePeriodInput,
    ap: &LeavePolicyVersion,
    b: &LeaveRequestSummary,
    bp: &LeavePolicyVersion,
) -> bool {
    use chrono::Datelike;
    let from = a.start_date.max(b.start_date);
    let to = a.end_date.min(b.end_date);
    if to < from {
        return false;
    }
    let counted = |d: NaiveDate, p: &LeavePolicyVersion| {
        p.counting_mode == LeaveCountingMode::CalendarDays
            || (p
                .working_weekdays
                .contains(&d.weekday().number_from_monday())
                && !p.excluded_dates.contains(&d))
    };
    for n in 0..=(to - from).num_days() {
        let d = from + chrono::Duration::days(n);
        if !counted(d, ap) || !counted(d, bp) {
            continue;
        }
        if ap.unit == LeaveUnit::Minute && bp.unit == LeaveUnit::Minute {
            if let (Some(ast), Some(aet), Some(bst), Some(bet)) =
                (a.start_time, a.end_time, b.start_time, b.end_time)
            {
                if ast < bet && bst < aet {
                    return true;
                }
                continue;
            }
        }
        // Half-day sessions are explicit. Minute/half-day overlap is conservative,
        // because this policy has no school-specific morning/afternoon time boundary.
        if ap.unit != LeaveUnit::Minute
            && bp.unit != LeaveUnit::Minute
            && a.day_portion != LeaveDayPortion::FullDay
            && b.day_portion != LeaveDayPortion::FullDay
            && a.day_portion != b.day_portion
        {
            continue;
        }
        return true;
    }
    false
}
async fn check_overlaps(
    tx: &mut Transaction<'_, Postgres>,
    r: &Record,
    p: &LeavePolicyVersion,
) -> Result<(), AppError> {
    let rows:Vec<(Json<LeaveRequestSummary>,Json<LeavePolicyVersion>)>=sqlx::query_as("SELECT r.document,p.document FROM staff_leave_requests r JOIN staff_leave_policies p ON p.id=r.policy_id WHERE r.owner_id=$1 AND r.id<>$2 AND r.status IN ('submitted','approved','cancellation_pending') AND (r.document->>'startDate')::date<=$3 AND (r.document->>'endDate')::date>=$4").bind(r.document.owner_id).bind(r.document.id).bind(r.document.end_date).bind(r.document.start_date).fetch_all(&mut **tx).await?;
    if rows
        .iter()
        .any(|(other, policy)| periods_overlap(&period_from(r), p, other, policy))
    {
        return Err(invalid(
            "ช่วงเวลาซ้อนกับคำขอที่รอพิจารณาหรืออนุมัติแล้ว กรุณาตรวจคำขอเดิม",
        ));
    }
    Ok(())
}
