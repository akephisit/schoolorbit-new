use chrono::NaiveDate;
use school_authorization::ActorContext;
use school_staff::career::StaffPersonnelType;
use school_staff_leave::{models::*, service};
use school_workflow::process_models::*;
use sqlx::PgPool;
use uuid::Uuid;
pub(crate) fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

pub(crate) fn step(reviewer: Uuid) -> ProcessTemplateStep {
    ProcessTemplateStep {
        title: "พิจารณาอนุมัติ".into(),
        kind: ProcessStepKind::Approve,
        completion_policy: ProcessCompletionPolicy::Any,
        recipients: vec![ProcessRecipient {
            kind: ProcessRecipientKind::User,
            user_id: Some(reviewer),
            organization_unit_id: None,
            position_code: None,
        }],
        due_after_days: None,
        requires_signature: false,
        requires_comment: false,
        requires_evidence: false,
        requires_delegated_authority: false,
    }
}

pub(crate) fn policy(
    revision: i64,
    reviewer: Uuid,
    unit: LeaveUnit,
    cap: Option<i64>,
) -> SaveLeavePolicyRequest {
    SaveLeavePolicyRequest {
        request_id: Uuid::now_v7(),
        expected_revision: revision,
        effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        source_reference: "เกณฑ์โรงเรียนสำหรับทดสอบ ไม่ใช่สิทธิ์ตามกฎหมาย".into(),
        enabled: true,
        personnel_types: vec![StaffPersonnelType::CivilServant],
        unit: match unit {
            LeaveUnit::Day => LeavePolicyUnit::Day,
            LeaveUnit::Minute => LeavePolicyUnit::Minute,
            LeaveUnit::HalfDay => {
                panic!("historical HalfDay policies must be seeded as evidence, not active writes")
            }
        },
        counting_mode: LeaveCountingMode::CalendarDays,
        working_weekdays: vec![],
        excluded_dates: vec![],
        quota_year_start_month: 10,
        max_per_request: None,
        annual_limit: cap,
        evidence_required: false,
        evidence_from_amount: None,
        minimum_notice_days: None,
        steps: vec![step(reviewer)],
    }
}

pub(crate) async fn typ(
    pool: &PgPool,
    admin: &ActorContext,
    code: &str,
    unit: LeaveUnit,
    cap: Option<i64>,
) -> (LeaveType, LeavePolicyVersion) {
    let t = service::save_type(
        pool,
        admin,
        None,
        SaveLeaveTypeRequest {
            request_id: Uuid::now_v7(),
            expected_revision: None,
            code: Some(code.into()),
            name: "ทดสอบลา".into(),
            kind: if unit == LeaveUnit::Minute {
                LeaveKind::Exit
            } else {
                LeaveKind::Leave
            },
            archived: false,
        },
    )
    .await
    .unwrap();
    let p = service::save_policy(
        pool,
        admin,
        t.id,
        policy(t.revision, admin.user_id, unit, cap),
    )
    .await
    .unwrap();
    (t, p)
}

pub(crate) fn period(t: Uuid, day: u32) -> LeavePeriodInput {
    LeavePeriodInput {
        destination: None,
        companion_user_ids: vec![],
        academic_year_id: Some(Uuid::from_u128(0x16000000000000000000000000000001)),
        academic_term_id: Some(Uuid::from_u128(0x16000000000000000000000000000003)),
        type_id: t,
        start_date: date(day),
        end_date: date(day),
        start_time: None,
        end_time: None,
        reason: "กิจธุระส่วนตัว".into(),
        evidence_file_ids: vec![],
    }
}

pub(crate) async fn submit(
    pool: &PgPool,
    a: &ActorContext,
    d: &LeaveRequestDetail,
) -> LeaveRequestDetail {
    service::submit(
        pool,
        a,
        d.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: d.request.revision,
        },
    )
    .await
    .unwrap()
}

pub(crate) fn action(d: &LeaveRequestDetail, a: ProcessAction) -> LeaveActionRequest {
    LeaveActionRequest {
        request_id: Uuid::now_v7(),
        expected_revision: d.request.revision,
        expected_process_revision: d.instance.as_ref().unwrap().revision,
        purpose: d.process_purpose,
        action: a,
        comment: Some("พิจารณาตามเกณฑ์".into()),
        signature: None,
    }
}
