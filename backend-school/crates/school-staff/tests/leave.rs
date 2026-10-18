use chrono::NaiveDate;
use school_authorization::ActorContext;
use school_permissions::registry::codes;
use school_staff::{
    career::StaffPersonnelType,
    leave::{files, models::*, service},
};
use school_workflow::{process, process_models::*};
use sqlx::PgPool;
use uuid::Uuid;

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}
async fn fixture(name: &str) -> (PgPool, ActorContext, ActorContext, ActorContext) {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 5).await;
    school_test_db::run_test_migrations(&pool).await;
    let mut actors = vec![];
    for name in ["Requester", "Reviewer", "Other"] {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic',$3,'Fixture','staff','active')").bind(id).bind(id.to_string()).bind(name).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO staff_career_history(user_id,kind,personnel_type,effective_date,source) VALUES($1,'personnel_type','civil_servant','2020-01-01','staff_entry')").bind(id).execute(&pool).await.unwrap();
        actors.push(ActorContext {
            user_id: id,
            permissions: vec![
                codes::STAFF_LEAVE_READ_OWN.into(),
                codes::STAFF_LEAVE_READ_ASSIGNED.into(),
                codes::STAFF_LEAVE_CREATE_OWN.into(),
                codes::STAFF_LEAVE_UPDATE_ASSIGNED.into(),
                codes::STAFF_LEAVE_DOWNLOAD_OWN.into(),
            ],
        });
    }
    let mut reviewer = actors.remove(1);
    reviewer.permissions.extend([
        codes::STAFF_LEAVE_MANAGE_SCHOOL.into(),
        codes::STAFF_LEAVE_READ_SCHOOL.into(),
        codes::PROCESS_MANAGE_SCHOOL.into(),
    ]);
    (pool, actors.remove(0), reviewer, actors.remove(0))
}
fn step(reviewer: Uuid) -> ProcessTemplateStep {
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
fn policy(
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
        unit,
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
async fn typ(
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
            code: code.into(),
            name: "ทดสอบลา".into(),
            kind: LeaveKind::Leave,
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
fn period(t: Uuid, day: u32) -> LeavePeriodInput {
    LeavePeriodInput {
        type_id: t,
        start_date: date(day),
        end_date: date(day),
        start_time: None,
        end_time: None,
        day_portion: LeaveDayPortion::FullDay,
        reason: "กิจธุระส่วนตัว".into(),
        evidence_file_ids: vec![],
    }
}
async fn create(pool: &PgPool, a: &ActorContext, t: Uuid, day: u32) -> LeaveRequestDetail {
    service::create(
        pool,
        a,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: period(t, day),
        },
    )
    .await
    .unwrap()
}
async fn submit(pool: &PgPool, a: &ActorContext, d: &LeaveRequestDetail) -> LeaveRequestDetail {
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
fn action(d: &LeaveRequestDetail, a: ProcessAction) -> LeaveActionRequest {
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
#[tokio::test]
async fn leave_policy_versions_are_category_specific_and_submission_pins_exact_snapshot() {
    let (pool, owner, admin, _) = fixture("leave_versions").await;
    let (t, p1) = typ(&pool, &admin, "versioned", LeaveUnit::HalfDay, None).await;
    let d = create(&pool, &owner, t.id, 1).await;
    let d = submit(&pool, &owner, &d).await;
    let mut newer = policy(
        p1.type_revision,
        admin.user_id,
        LeaveUnit::HalfDay,
        Some(20),
    );
    newer.source_reference = "แก้ไขเกณฑ์วันเดิม".into();
    let p2 = service::save_policy(&pool, &admin, t.id, newer)
        .await
        .unwrap();
    assert_eq!(
        service::effective_policy(&pool, &owner, t.id, date(1))
            .await
            .unwrap()
            .id,
        p2.id
    );
    assert_eq!(
        service::detail(&pool, &owner, d.request.id)
            .await
            .unwrap()
            .policy
            .unwrap()
            .id,
        p1.id
    );
    let mut other = policy(
        p2.type_revision,
        admin.user_id,
        LeaveUnit::HalfDay,
        Some(10),
    );
    other.personnel_types = vec![StaffPersonnelType::GovernmentEmployee];
    service::save_policy(&pool, &admin, t.id, other)
        .await
        .unwrap();
    assert_eq!(
        service::effective_policy(&pool, &owner, t.id, date(1))
            .await
            .unwrap()
            .id,
        p2.id
    );
    let doc: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM staff_leave_policies WHERE id=$1")
            .bind(p1.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(doc["sourceReference"], p1.source_reference);
    assert!(
        sqlx::query("UPDATE staff_leave_policies SET version=99 WHERE id=$1")
            .bind(p1.id)
            .execute(&pool)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn leave_replay_stale_and_denied_actions_preserve_exact_audit_and_work() {
    let (pool, owner, admin, other) = fixture("leave_replay_access").await;
    let (t, _) = typ(&pool, &admin, "replay", LeaveUnit::Day, None).await;
    let input = CreateLeaveRequest {
        request_id: Uuid::now_v7(),
        period: period(t.id, 1),
    };
    let d = service::create(&pool, &owner, input.clone()).await.unwrap();
    assert_eq!(
        service::create(&pool, &owner, input.clone())
            .await
            .unwrap()
            .request
            .id,
        d.request.id
    );
    let mut conflict = input;
    conflict.period.reason = "เปลี่ยนข้อมูล".into();
    assert!(service::create(&pool, &owner, conflict).await.is_err());
    assert!(service::detail(&pool, &other, d.request.id).await.is_err());
    let sent = submit(&pool, &owner, &d).await;
    assert!(service::act(
        &pool,
        &owner,
        sent.request.id,
        action(&sent, ProcessAction::Approve)
    )
    .await
    .is_err());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM staff_leave_history WHERE request_id=$1")
            .bind(d.request.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    let a = action(&sent, ProcessAction::Approve);
    let approved = service::act(&pool, &admin, sent.request.id, a.clone())
        .await
        .unwrap();
    assert_eq!(approved.request.status, LeaveStatus::Approved);
    assert_eq!(
        service::act(&pool, &admin, sent.request.id, a)
            .await
            .unwrap()
            .request
            .revision,
        approved.request.revision
    );
    assert!(service::act(
        &pool,
        &admin,
        sent.request.id,
        action(&sent, ProcessAction::Reject)
    )
    .await
    .is_err());
    let tasks:i64=sqlx::query_scalar("SELECT count(*) FROM work_item_assignees a JOIN work_items w ON w.id=a.work_item_id WHERE w.source_resource_id=$1").bind(d.request.id).fetch_one(&pool).await.unwrap();
    assert_eq!(tasks, 1);
    let history: i64 =
        sqlx::query_scalar("SELECT count(*) FROM staff_leave_history WHERE request_id=$1")
            .bind(d.request.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(history, 3);
}
#[tokio::test]
async fn leave_concurrent_quota_reservations_allow_only_one_and_rollback_failed_receipt() {
    let (pool, owner, admin, _) = fixture("leave_quota_concurrency").await;
    let (t, _) = typ(&pool, &admin, "limited", LeaveUnit::Day, Some(1)).await;
    let a = create(&pool, &owner, t.id, 1).await;
    let b = create(&pool, &owner, t.id, 2).await;
    let ai = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: 1,
    };
    let bi = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: 1,
    };
    let (ar, br) = tokio::join!(
        service::submit(&pool, &owner, a.request.id, ai),
        service::submit(&pool, &owner, b.request.id, bi)
    );
    assert_ne!(ar.is_ok(), br.is_ok());
    let used: i64 = sqlx::query_scalar(
        "SELECT sum(amount)::bigint FROM staff_leave_requests WHERE status='submitted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(used, 1);
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_leave_receipts WHERE operation='request_submit'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, 1);
}
#[tokio::test]
async fn leave_overlaps_union_types_and_allows_distinct_half_sessions() {
    let (pool, owner, admin, _) = fixture("leave_overlap_sessions").await;
    let (t, _) = typ(&pool, &admin, "half", LeaveUnit::HalfDay, None).await;
    let (other, _) = typ(&pool, &admin, "other", LeaveUnit::Day, None).await;
    let mut am = period(t.id, 1);
    am.day_portion = LeaveDayPortion::Morning;
    let d = service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: am.clone(),
        },
    )
    .await
    .unwrap();
    submit(&pool, &owner, &d).await;
    am.day_portion = LeaveDayPortion::Afternoon;
    let d = service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: am,
        },
    )
    .await
    .unwrap();
    submit(&pool, &owner, &d).await;
    let d = create(&pool, &owner, other.id, 1).await;
    assert!(service::submit(
        &pool,
        &owner,
        d.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1
        }
    )
    .await
    .is_err());
    let mut third = period(t.id, 2);
    third.day_portion = LeaveDayPortion::Morning;
    let a = service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: third.clone(),
        },
    )
    .await
    .unwrap();
    let b = service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: third,
        },
    )
    .await
    .unwrap();
    let (ar, br) = tokio::join!(
        service::submit(
            &pool,
            &owner,
            a.request.id,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: 1
            }
        ),
        service::submit(
            &pool,
            &owner,
            b.request.id,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: 1
            }
        )
    );
    assert_ne!(ar.is_ok(), br.is_ok());
}
#[tokio::test]
async fn leave_approved_cancellation_uses_audited_second_process_and_releases_usage_only_after_approval(
) {
    let (pool, owner, admin, _) = fixture("leave_cancel_flow").await;
    let (t, _) = typ(&pool, &admin, "cancel", LeaveUnit::Day, Some(2)).await;
    let d = create(&pool, &owner, t.id, 1).await;
    let sent = submit(&pool, &owner, &d).await;
    let approved = service::act(
        &pool,
        &admin,
        d.request.id,
        action(&sent, ProcessAction::Approve),
    )
    .await
    .unwrap();
    let original = approved.instance.as_ref().unwrap().id;
    let ci = CancelLeaveRequest {
        request_id: Uuid::now_v7(),
        expected_revision: approved.request.revision,
        reason: "เปลี่ยนแผน".into(),
    };
    let cancel = service::cancel(&pool, &owner, d.request.id, ci.clone())
        .await
        .unwrap();
    assert_eq!(cancel.request.status, LeaveStatus::CancellationPending);
    assert_ne!(cancel.instance.as_ref().unwrap().id, original);
    assert_eq!(
        service::cancel(&pool, &owner, d.request.id, ci)
            .await
            .unwrap()
            .request
            .revision,
        cancel.request.revision
    );
    let restored = service::act(
        &pool,
        &admin,
        d.request.id,
        action(&cancel, ProcessAction::Reject),
    )
    .await
    .unwrap();
    assert_eq!(restored.request.status, LeaveStatus::Approved);
    assert_eq!(restored.instance.as_ref().unwrap().id, original);
    let cancel = service::cancel(
        &pool,
        &owner,
        d.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: restored.request.revision,
            reason: "ขอใหม่พร้อมข้อมูล".into(),
        },
    )
    .await
    .unwrap();
    let done = service::act(
        &pool,
        &admin,
        d.request.id,
        action(&cancel, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(done.request.status, LeaveStatus::Cancelled);
    let report = service::usage(
        &pool,
        &owner,
        LeaveUsageQuery {
            from: date(1),
            to: date(30),
            kind: None,
        },
    )
    .await
    .unwrap();
    assert!(report.items.is_empty());
    assert_eq!(
        done.history
            .iter()
            .filter(|h| h.action == "cancellation_requested")
            .count(),
        2
    );
}
async fn staged(pool: &PgPool, owner: Uuid, purpose: &str) -> (Uuid, Uuid) {
    let f = Uuid::now_v7();
    let v = Uuid::now_v7();
    sqlx::query("INSERT INTO files(id,owner_user_id,created_by,display_filename,purpose_code,visibility,lifecycle_status,retention_class,expires_at) VALUES($1,$2,$2,'test.png',$3,'private','ready','temporary',now()+interval '1 hour')").bind(f).bind(owner).bind(purpose).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,1,'r2','private','stored',$3,'image/png','png',100,repeat('a',64),'clean')").bind(v).bind(f).bind(format!("synthetic/{f}")).execute(pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(f)
        .bind(v)
        .execute(pool)
        .await
        .unwrap();
    (f, v)
}
#[tokio::test]
async fn leave_evidence_is_private_resource_bound_and_immutable_after_submission() {
    let (pool, owner, admin, other) = fixture("leave_evidence_acl").await;
    let (t, _) = typ(&pool, &admin, "evidence", LeaveUnit::Day, None).await;
    let d = create(&pool, &owner, t.id, 1).await;
    let other_d = create(&pool, &owner, t.id, 2).await;
    let (f, v) = staged(&pool, owner.user_id, "staff_leave_evidence").await;
    files::record_upload(&pool, owner.user_id, d.request.id, f)
        .await
        .unwrap();
    let mut p = period(t.id, 2);
    p.evidence_file_ids = vec![f];
    assert!(service::update(
        &pool,
        &owner,
        other_d.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            period: p
        }
    )
    .await
    .is_err());
    let mut p = period(t.id, 1);
    p.evidence_file_ids = vec![f];
    let updated = service::update(
        &pool,
        &owner,
        d.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            period: p,
        },
    )
    .await
    .unwrap();
    let accepted = submit(&pool, &owner, &updated).await;
    let snap: Uuid = sqlx::query_scalar(
        "SELECT version_id FROM staff_leave_file_links WHERE request_id=$1 AND file_id=$2",
    )
    .bind(d.request.id)
    .bind(f)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(snap, v);
    assert!(sqlx::query("UPDATE files SET deleted_at=now() WHERE id=$1")
        .bind(f)
        .execute(&pool)
        .await
        .is_err());
    use school_file_platform::{
        access::FilePolicyAction,
        platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
        repository::PlatformFile,
    };
    let file = PlatformFile {
        id: f,
        owner_user_id: Some(owner.user_id),
        purpose: FilePurpose::StaffLeaveEvidence,
        visibility: FileVisibility::Private,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: Some(1),
        display_filename: "test.png".into(),
        detected_mime_type: "image/png".into(),
        byte_size: 100,
    };
    assert!(files::authorize_existing(
        &pool,
        &admin,
        &file,
        FilePolicyAction::Read,
        Some(d.request.id)
    )
    .await
    .is_ok());
    assert!(files::authorize_existing(
        &pool,
        &other,
        &file,
        FilePolicyAction::Read,
        Some(d.request.id)
    )
    .await
    .is_err());
    assert!(
        files::delete_guard(&pool, &owner, &file, Some(d.request.id))
            .await
            .is_err()
    );
    assert_eq!(accepted.evidence_file_ids, vec![f]);
}
#[tokio::test]
async fn leave_source_templates_cannot_be_edited_by_generic_template_manager_and_signatures_need_consent(
) {
    let (pool, owner, admin, _) = fixture("leave_provider_signature").await;
    let (t, p) = typ(&pool, &admin, "signed", LeaveUnit::Day, None).await;
    assert!(process::save_template(
        &pool,
        &admin,
        Some(p.template_id),
        SaveProcessTemplateRequest {
            request_id: Uuid::now_v7(),
            expected_revision: Some(1),
            code: "hijack".into(),
            title: "changed".into(),
            steps: vec![step(owner.user_id)],
            active: true
        }
    )
    .await
    .is_err());
    assert!(!process::list_templates(&pool)
        .await
        .unwrap()
        .iter()
        .any(|t| t.id == p.template_id));
    let mut next = policy(p.type_revision, admin.user_id, LeaveUnit::Day, None);
    next.steps[0].requires_signature = true;
    service::save_policy(&pool, &admin, t.id, next)
        .await
        .unwrap();
    let d = create(&pool, &owner, t.id, 1).await;
    let d = submit(&pool, &owner, &d).await;
    assert!(service::act(
        &pool,
        &admin,
        d.request.id,
        action(&d, ProcessAction::Approve)
    )
    .await
    .is_err());
    let (f, _) = staged(&pool, admin.user_id, "staff_leave_signature").await;
    let mut a = action(&d, ProcessAction::Approve);
    a.signature = Some(LeaveSignatureConsent {
        consent: false,
        image_file_id: Some(f),
        input: SignatureInput {
            method: SignatureMethod::Image,
            asset_id: None,
            strokes: vec![],
            page: 1,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            document_revision: d.request.revision,
            content_hash: d.content_hash.clone(),
        },
    });
    assert!(service::act(&pool, &admin, d.request.id, a.clone())
        .await
        .is_err());
    a.signature.as_mut().unwrap().consent = true;
    let done = service::act(&pool, &admin, d.request.id, a).await.unwrap();
    assert_eq!(done.request.status, LeaveStatus::Approved);
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_leave_file_links WHERE request_id=$1 AND file_id=$2",
    )
    .bind(d.request.id)
    .bind(f)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn leave_disabled_or_missing_profiles_never_hide_stored_drafts_or_change_pinned_work() {
    let (pool, owner, admin, _) = fixture("leave_disabled_profiles").await;
    let (t, p) = typ(&pool, &admin, "disable", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 2).await;
    let active = create(&pool, &owner, t.id, 1).await;
    let active = submit(&pool, &owner, &active).await;
    let mut disabled = policy(p.type_revision, admin.user_id, LeaveUnit::Day, None);
    disabled.enabled = false;
    service::save_policy(&pool, &admin, t.id, disabled)
        .await
        .unwrap();
    let read = service::detail(&pool, &owner, draft.request.id)
        .await
        .unwrap();
    assert!(!read.can_submit);
    assert!(!read.policy.unwrap().enabled);
    assert!(read.can_cancel);
    let mut input = period(t.id, 2);
    input.reason = "แก้ไขเหตุผลระหว่างปิดรับ".into();
    let edited = service::update(
        &pool,
        &owner,
        draft.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            period: input,
        },
    )
    .await
    .unwrap();
    assert!(!edited.can_submit);
    assert!(service::submit(
        &pool,
        &owner,
        draft.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: edited.request.revision
        }
    )
    .await
    .is_err());
    assert!(service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: period(t.id, 3)
        }
    )
    .await
    .is_err());
    let approved = service::act(
        &pool,
        &admin,
        active.request.id,
        action(&active, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(approved.request.status, LeaveStatus::Approved);
    assert_eq!(approved.policy.unwrap().id, p.id);
    sqlx::query("INSERT INTO staff_career_history(user_id,kind,personnel_type,effective_date,source) VALUES($1,'personnel_type','government_employee','2026-11-01','staff_entry')").bind(owner.user_id).execute(&pool).await.unwrap();
    let missing = service::detail(&pool, &owner, draft.request.id)
        .await
        .unwrap();
    assert!(missing.policy.is_none());
    assert!(!missing.can_submit);
    assert!(missing.can_cancel);
    let cancelled = service::cancel(
        &pool,
        &owner,
        draft.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: missing.request.revision,
            reason: "ถอนร่างระหว่างแก้ไขประเภทบุคลากร".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(cancelled.request.status, LeaveStatus::Cancelled);
}

#[tokio::test]
async fn leave_type_crud_archive_restore_preserves_request_history_and_replays_unused_delete() {
    let (pool, owner, admin, _) = fixture("leave_type_lifecycle").await;
    let input = SaveLeaveTypeRequest {
        request_id: Uuid::now_v7(),
        expected_revision: None,
        code: "unused".into(),
        name: "ประเภทที่ยังไม่ใช้".into(),
        kind: LeaveKind::Leave,
        archived: false,
    };
    let unused = service::save_type(&pool, &admin, None, input.clone())
        .await
        .unwrap();
    let mut duplicate = input;
    duplicate.request_id = Uuid::now_v7();
    assert!(matches!(
        service::save_type(&pool, &admin, None, duplicate).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let deletion = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: unused.revision,
    };
    service::delete_type(&pool, &admin, unused.id, deletion.clone())
        .await
        .unwrap();
    service::delete_type(&pool, &admin, unused.id, deletion)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_leave_types WHERE id=$1")
        .bind(unused.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    let (t, p) = typ(&pool, &admin, "used_type", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 1).await;
    let archived = service::save_type(
        &pool,
        &admin,
        Some(t.id),
        SaveLeaveTypeRequest {
            request_id: Uuid::now_v7(),
            expected_revision: Some(p.type_revision),
            code: t.code.clone(),
            name: "ชื่อประเภทที่แก้ไข".into(),
            kind: t.kind,
            archived: true,
        },
    )
    .await
    .unwrap();
    assert!(service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: period(t.id, 2)
        }
    )
    .await
    .is_err());
    assert!(service::submit(
        &pool,
        &owner,
        draft.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1
        }
    )
    .await
    .is_err());
    let original = service::detail(&pool, &owner, draft.request.id)
        .await
        .unwrap();
    assert_eq!(original.request.type_name, t.name);
    assert_eq!(original.request.revision, 1);
    let restored = service::save_type(
        &pool,
        &admin,
        Some(t.id),
        SaveLeaveTypeRequest {
            request_id: Uuid::now_v7(),
            expected_revision: Some(archived.revision),
            code: t.code.clone(),
            name: archived.name.clone(),
            kind: t.kind,
            archived: false,
        },
    )
    .await
    .unwrap();
    let active = submit(&pool, &owner, &draft).await;
    assert_eq!(active.request.type_name, t.name);
    let later = create(&pool, &owner, t.id, 2).await;
    assert_eq!(later.request.type_name, restored.name);
    assert!(service::delete_type(
        &pool,
        &admin,
        t.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: restored.revision
        }
    )
    .await
    .is_err());
    let revision_name:String=sqlx::query_scalar("SELECT document->>'typeName' FROM staff_leave_revisions WHERE request_id=$1 AND revision=1").bind(draft.request.id).fetch_one(&pool).await.unwrap();
    assert_eq!(revision_name, t.name);
}

#[tokio::test]
async fn leave_list_unions_owned_and_assigned_records_before_pagination() {
    let (pool, owner, admin, other) = fixture("leave_list_union").await;
    let (t, p) = typ(&pool, &admin, "union", LeaveUnit::Day, None).await;
    let own = create(&pool, &owner, t.id, 1).await;
    service::save_policy(
        &pool,
        &admin,
        t.id,
        policy(p.type_revision, owner.user_id, LeaveUnit::Day, None),
    )
    .await
    .unwrap();
    let assigned = create(&pool, &other, t.id, 2).await;
    submit(&pool, &other, &assigned).await;
    let page = service::list(&pool, &owner, LeaveListQuery::default())
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page.items.iter().any(|r| r.id == own.request.id));
    assert!(page.items.iter().any(|r| r.id == assigned.request.id));
    let first = service::list(
        &pool,
        &owner,
        LeaveListQuery {
            limit: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(first.has_more);
    let second = service::list(
        &pool,
        &owner,
        LeaveListQuery {
            limit: Some(1),
            offset: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(!second.has_more);
    assert_ne!(first.items[0].id, second.items[0].id);
    let denied = ActorContext {
        user_id: admin.user_id,
        permissions: vec![
            codes::STAFF_LEAVE_READ_OWN.into(),
            codes::STAFF_LEAVE_READ_ASSIGNED.into(),
        ],
    };
    assert!(service::list(&pool, &denied, LeaveListQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
}

#[tokio::test]
async fn leave_cancellation_uses_current_routing_for_frozen_category_without_rewriting_approved_basis(
) {
    let (pool, owner, original, mut replacement) = fixture("leave_cancel_current_route").await;
    replacement.permissions.extend([
        codes::STAFF_LEAVE_MANAGE_SCHOOL.into(),
        codes::STAFF_LEAVE_READ_SCHOOL.into(),
    ]);
    let (t, original_policy) =
        typ(&pool, &original, "cancel_current", LeaveUnit::Day, Some(2)).await;
    let draft = create(&pool, &owner, t.id, 1).await;
    let submitted = submit(&pool, &owner, &draft).await;
    let approved = service::act(
        &pool,
        &original,
        draft.request.id,
        action(&submitted, ProcessAction::Approve),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(original.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let cancellation = CancelLeaveRequest {
        request_id: Uuid::now_v7(),
        expected_revision: approved.request.revision,
        reason: "ถอนวันลาที่อนุมัติแล้ว".into(),
    };
    assert!(
        matches!(service::cancel(&pool,&owner,draft.request.id,cancellation.clone()).await,Err(school_errors::AppError::ValidationError(message)) if message.contains("ผู้ดูแล"))
    );
    let mut current = policy(
        original_policy.type_revision,
        replacement.user_id,
        LeaveUnit::Day,
        Some(1),
    );
    current.effective_from = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    current.enabled = false;
    let current = service::save_policy(&pool, &replacement, t.id, current)
        .await
        .unwrap();
    service::save_type(
        &pool,
        &replacement,
        Some(t.id),
        SaveLeaveTypeRequest {
            request_id: Uuid::now_v7(),
            expected_revision: Some(current.type_revision),
            code: t.code.clone(),
            name: t.name.clone(),
            kind: t.kind,
            archived: true,
        },
    )
    .await
    .unwrap();
    // A later correction to current career must not erase the category accepted with this request.
    sqlx::query("INSERT INTO staff_career_history(user_id,kind,personnel_type,effective_date,source) VALUES($1,'personnel_type','government_employee',$2,'staff_entry')").bind(owner.user_id).bind(chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive()).execute(&pool).await.unwrap();
    let pending = service::cancel(&pool, &owner, draft.request.id, cancellation)
        .await
        .unwrap();
    assert_eq!(pending.request.status, LeaveStatus::CancellationPending);
    assert_eq!(pending.policy.as_ref().unwrap().id, original_policy.id);
    assert_eq!(
        pending.instance.as_ref().unwrap().template_id,
        Some(current.template_id)
    );
    assert_eq!(pending.request.amount, approved.request.amount);
    assert_eq!(
        pending.request.personnel_type,
        Some(StaffPersonnelType::CivilServant)
    );
    assert_eq!(
        pending.instance.as_ref().unwrap().steps[0].recipients[0].user_id,
        replacement.user_id
    );
    let report = service::usage(
        &pool,
        &owner,
        LeaveUsageQuery {
            from: date(1),
            to: date(30),
            kind: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(report.items[0].approved_amount, 1);
    assert_eq!(report.items[0].policy_version_id, original_policy.id);
    let finished = service::act(
        &pool,
        &replacement,
        draft.request.id,
        action(&pending, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(finished.request.status, LeaveStatus::Cancelled);
    assert!(service::usage(
        &pool,
        &owner,
        LeaveUsageQuery {
            from: date(1),
            to: date(30),
            kind: None
        }
    )
    .await
    .unwrap()
    .items
    .is_empty());
}

#[tokio::test]
async fn leave_future_approved_request_uses_pinned_cancellation_route_until_a_current_profile_exists(
) {
    let (pool, owner, admin, _) = fixture("leave_cancel_future_route").await;
    let t = service::save_type(
        &pool,
        &admin,
        None,
        SaveLeaveTypeRequest {
            request_id: Uuid::now_v7(),
            expected_revision: None,
            code: "future_only".into(),
            name: "เกณฑ์ในอนาคต".into(),
            kind: LeaveKind::Leave,
            archived: false,
        },
    )
    .await
    .unwrap();
    let future = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
        + chrono::Duration::days(30);
    let mut input = policy(t.revision, admin.user_id, LeaveUnit::Day, None);
    input.effective_from = future;
    let p = service::save_policy(&pool, &admin, t.id, input)
        .await
        .unwrap();
    let mut input = period(t.id, 1);
    input.start_date = future;
    input.end_date = future;
    let draft = service::create(
        &pool,
        &owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: input,
        },
    )
    .await
    .unwrap();
    let submitted = submit(&pool, &owner, &draft).await;
    let approved = service::act(
        &pool,
        &admin,
        draft.request.id,
        action(&submitted, ProcessAction::Approve),
    )
    .await
    .unwrap();
    let pending = service::cancel(
        &pool,
        &owner,
        draft.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: approved.request.revision,
            reason: "เปลี่ยนแผนในอนาคต".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(pending.request.status, LeaveStatus::CancellationPending);
    assert_eq!(pending.policy.as_ref().unwrap().id, p.id);
    assert_eq!(
        pending.instance.as_ref().unwrap().template_id,
        Some(p.template_id)
    );
}
