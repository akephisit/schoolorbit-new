mod support;
use school_authorization::ActorContext;
use school_staff::career::StaffPersonnelType;
use school_staff_leave::{files, models::*, service};
use school_workflow::{process, process_models::*};
use sqlx::PgPool;
use support::*;
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, ActorContext, ActorContext, ActorContext) {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 5).await;
    school_test_db::run_test_migrations(&pool).await;
    sqlx::query("INSERT INTO academic_years(id,year,name,status) VALUES('16000000-0000-0000-0000-000000000001',2569,'Coverage fixture','planning')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO bell_schedules(id,academic_year_id,code,name) VALUES('16000000-0000-0000-0000-000000000002','16000000-0000-0000-0000-000000000001','COVERAGE','Coverage')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) VALUES('16000000-0000-0000-0000-000000000003','16000000-0000-0000-0000-000000000001','Coverage',1,'COVERAGE','regular',true,true,'planning','16000000-0000-0000-0000-000000000002','MON,TUE,WED,THU,FRI')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetables(academic_year_id,academic_term_id,bell_schedule_id) VALUES('16000000-0000-0000-0000-000000000001','16000000-0000-0000-0000-000000000003','16000000-0000-0000-0000-000000000002')").execute(&pool).await.unwrap();
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

#[tokio::test]
async fn leave_policy_versions_are_category_specific_and_submission_pins_exact_snapshot() {
    let (pool, owner, admin, _) = fixture("leave_versions").await;
    let (t, p1) = typ(&pool, &admin, "versioned", LeaveUnit::Day, None).await;
    let d = create(&pool, &owner, t.id, 1).await;
    let d = submit(&pool, &owner, &d).await;
    let mut newer = policy(p1.type_revision, admin.user_id, LeaveUnit::Day, Some(20));
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
    let mut other = policy(p2.type_revision, admin.user_id, LeaveUnit::Day, Some(10));
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
async fn leave_full_day_overlaps_union_types_and_exit_intervals_are_exact() {
    let (pool, owner, admin, _) = fixture("leave_current_overlap").await;
    let (first, _) = typ(&pool, &admin, "full", LeaveUnit::Day, None).await;
    let (second, _) = typ(&pool, &admin, "other", LeaveUnit::Day, None).await;
    let d = create(&pool, &owner, first.id, 1).await;
    submit(&pool, &owner, &d).await;
    let other = create(&pool, &owner, second.id, 1).await;
    assert!(service::submit(
        &pool,
        &owner,
        other.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: other.request.revision
        }
    )
    .await
    .is_err());
    let (exit, _) = typ(&pool, &admin, "exit_exact", LeaveUnit::Minute, None).await;
    for (start, end, ok) in [
        (9 * 60, 10 * 60, true),
        (10 * 60, 11 * 60, true),
        (9 * 60 + 30, 10 * 60 + 30, false),
    ] {
        let mut p = period(exit.id, 2);
        p.start_time = chrono::NaiveTime::from_hms_opt(start / 60, start % 60, 0);
        p.end_time = chrono::NaiveTime::from_hms_opt(end / 60, end % 60, 0);
        let d = service::create(
            &pool,
            &owner,
            CreateLeaveRequest {
                request_id: Uuid::now_v7(),
                period: p,
            },
        )
        .await
        .unwrap();
        let result = service::submit(
            &pool,
            &owner,
            d.request.id,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: d.request.revision,
            },
        )
        .await;
        assert_eq!(result.is_ok(), ok);
    }
    let a = create(&pool, &owner, first.id, 3).await;
    let b = create(&pool, &owner, second.id, 3).await;
    let (a, b) = tokio::join!(
        service::submit(
            &pool,
            &owner,
            a.request.id,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: a.request.revision
            }
        ),
        service::submit(
            &pool,
            &owner,
            b.request.id,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: b.request.revision
            }
        )
    );
    assert_ne!(a.is_ok(), b.is_ok());
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
    use school_file_contracts::{
        access::FilePolicyAction,
        file::PlatformFile,
        platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
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

#[test]
fn leave_type_request_accepts_omitted_null_and_explicit_internal_codes() {
    let payload = serde_json::json!({
        "requestId": Uuid::now_v7(),
        "name": "ลาส่วนตัว",
        "kind": "leave",
        "archived": false
    });
    let omitted: SaveLeaveTypeRequest = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(omitted.code, None);
    assert_eq!(omitted.expected_revision, None);
    let mut explicit = payload;
    explicit["code"] = serde_json::Value::Null;
    let null: SaveLeaveTypeRequest = serde_json::from_value(explicit.clone()).unwrap();
    assert_eq!(null.code, None);
    explicit["code"] = serde_json::json!("historical_code");
    let supplied: SaveLeaveTypeRequest = serde_json::from_value(explicit.clone()).unwrap();
    assert_eq!(supplied.code.as_deref(), Some("historical_code"));
    assert_eq!(
        serde_json::to_value(supplied).unwrap()["code"],
        explicit["code"]
    );
}

#[tokio::test]
async fn leave_type_omitted_codes_are_unique_replayable_and_immutable() {
    let (pool, owner, admin, _) = fixture("leave_type_auto_codes").await;
    let input = SaveLeaveTypeRequest {
        request_id: Uuid::now_v7(),
        expected_revision: None,
        code: None,
        name: "ประเภทจากชื่อเท่านั้น".into(),
        kind: LeaveKind::Leave,
        archived: false,
    };
    assert!(matches!(
        service::save_type(&pool, &owner, None, input.clone()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    let created = service::save_type(&pool, &admin, None, input.clone())
        .await
        .unwrap();
    let replay = service::save_type(&pool, &admin, None, input.clone())
        .await
        .unwrap();
    assert_eq!(replay.id, created.id);
    assert_eq!(replay.code, created.code);
    assert_eq!(replay.revision, created.revision);
    let mut second_input = input.clone();
    second_input.request_id = Uuid::now_v7();
    second_input.kind = LeaveKind::Exit;
    let second = service::save_type(&pool, &admin, None, second_input)
        .await
        .unwrap();
    assert_ne!(second.id, created.id);
    assert_ne!(second.code, created.code);
    for item in [&created, &second] {
        assert!(!item.code.is_empty() && item.code.len() <= 64);
        assert!(item
            .code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'));
    }
    let update = SaveLeaveTypeRequest {
        request_id: Uuid::now_v7(),
        expected_revision: Some(created.revision),
        name: "ชื่อที่แก้ไข".into(),
        ..input.clone()
    };
    for code in ["", "INVALID", "has space", &"a".repeat(65)] {
        let mut invalid = input.clone();
        invalid.request_id = Uuid::now_v7();
        invalid.code = Some(code.into());
        assert!(matches!(
            service::save_type(&pool, &admin, None, invalid).await,
            Err(school_errors::AppError::ValidationError(_))
        ));
    }
    for name in [" ".to_string(), "ก".repeat(201)] {
        let mut invalid = input.clone();
        invalid.name = name;
        let error = service::save_type(&pool, &admin, None, invalid)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            school_errors::AppError::ValidationError(ref message)
                if message == "กรุณาระบุชื่อประเภทคำขอไม่เกิน 200 ตัวอักษร"
        ));
    }
    let mut changed_code = update.clone();
    changed_code.code = Some(second.code.clone());
    assert!(matches!(
        service::save_type(&pool, &admin, Some(created.id), changed_code).await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let mut changed_kind = update.clone();
    changed_kind.kind = LeaveKind::Exit;
    assert!(matches!(
        service::save_type(&pool, &admin, Some(created.id), changed_kind).await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    assert!(matches!(
        service::save_type(&pool, &owner, Some(created.id), update.clone()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    let saved = service::save_type(&pool, &admin, Some(created.id), update.clone())
        .await
        .unwrap();
    assert_eq!(saved.code, created.code);
    assert_eq!(saved.name, update.name);
    assert_eq!(saved.revision, created.revision + 1);
    let replay = service::save_type(&pool, &admin, Some(created.id), update.clone())
        .await
        .unwrap();
    assert_eq!(replay.code, saved.code);
    assert_eq!(replay.revision, saved.revision);
    assert!(matches!(
        service::save_type(&pool, &admin, Some(second.id), update.clone()).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let mut stale = update;
    stale.request_id = Uuid::now_v7();
    assert!(matches!(
        service::save_type(&pool, &admin, Some(created.id), stale).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let stored: (String, i64) =
        sqlx::query_as("SELECT code,revision FROM staff_leave_types WHERE id=$1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, (created.code, saved.revision));
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_leave_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipts, 3);
}

#[tokio::test]
async fn leave_type_crud_archive_restore_preserves_request_history_and_replays_unused_delete() {
    let (pool, owner, admin, _) = fixture("leave_type_lifecycle").await;
    let input = SaveLeaveTypeRequest {
        request_id: Uuid::now_v7(),
        expected_revision: None,
        code: Some("unused".into()),
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
            code: None,
            name: "ชื่อประเภทที่แก้ไข".into(),
            kind: t.kind,
            archived: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(archived.code, t.code);
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
            code: None,
            name: archived.name.clone(),
            kind: t.kind,
            archived: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.code, t.code);
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
            code: Some(t.code.clone()),
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
    assert_eq!(report.items[0].approved_amount, "2");
    assert_eq!(report.items[0].amount_scale, 2);
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
            code: Some("future_only".into()),
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

#[tokio::test]
async fn coverage_missing_timetable_is_not_zero_and_partial_fields_are_rejected() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, _) = fixture("coverage_full_day_missing_vs_zero").await;
    let (t, _) = typ(&pool, &admin, "coverage_empty", LeaveUnit::Day, None).await;
    let input = period(t.id, 17);
    let preview = coverage::preview(
        &pool,
        &owner,
        PreviewLeaveCoverageRequest {
            period: input.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.timetable_state, LeaveCoverageTimetableState::Ready);
    assert_eq!(preview.days.len(), 1);
    assert!(preview.days[0].lessons.is_empty());
    for (field, value) in [
        ("dayPortion", serde_json::json!("morning")),
        ("dayPortion", serde_json::json!("full_day")),
        ("halfDayBoundary", serde_json::json!("11:30:00")),
        ("halfDayBoundary", serde_json::Value::Null),
    ] {
        let mut forged = serde_json::to_value(&input).unwrap();
        forged[field] = value;
        assert!(
            serde_json::from_value::<LeavePeriodInput>(forged).is_err(),
            "retired field {field} must never be an active input"
        );
    }
    sqlx::query("DELETE FROM academic_timetables")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        coverage::preview(&pool, &owner, PreviewLeaveCoverageRequest { period: input })
            .await
            .unwrap()
            .timetable_state,
        LeaveCoverageTimetableState::Missing
    );
}
#[tokio::test]
async fn coverage_options_keep_context_defaults_without_a_mutable_boundary() {
    use school_staff_leave::coverage;
    let (pool, owner, admin, mut other) = fixture("coverage_current_options").await;
    assert!(coverage::options(&pool, &owner)
        .await
        .unwrap()
        .contexts
        .is_empty());
    // Fixture contexts start hidden. Only Core-visible contexts belong in lookup.
    sqlx::query("UPDATE academic_years SET is_visible=true")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET is_visible=true")
        .execute(&pool)
        .await
        .unwrap();
    let options = coverage::options(&pool, &owner).await.unwrap();
    assert_eq!(options.contexts.len(), 1);
    assert!(options.default_academic_year_id.is_none());
    assert!(options.default_academic_term_id.is_none());
    let admin_options = coverage::options(&pool, &admin).await.unwrap();
    assert_eq!(
        admin_options.contexts[0].academic_term_id,
        options.contexts[0].academic_term_id
    );
    other
        .permissions
        .retain(|p| p != codes::STAFF_LEAVE_CREATE_OWN);
    assert!(coverage::options(&pool, &other).await.is_err());
    let value = serde_json::to_value(options).unwrap();
    assert!(value.get("halfDayBoundary").is_none());
    assert!(value.get("revision").is_none());
    let table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('staff_leave_coverage_settings')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(table.is_none());
}

async fn structural_coverage_slots(pool: &PgPool, teacher: Uuid, count: i32) {
    let (year, term, bell) = (
        Uuid::from_u128(0x16000000000000000000000000000001),
        Uuid::from_u128(0x16000000000000000000000000000003),
        Uuid::from_u128(0x16000000000000000000000000000002),
    );
    let timetable: Uuid =
        sqlx::query_scalar("SELECT id FROM academic_timetables WHERE academic_term_id=$1")
            .bind(term)
            .fetch_one(pool)
            .await
            .unwrap();
    for n in 0..count {
        let period = Uuid::now_v7();
        let block = Uuid::now_v7();
        sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,'Duty',$3,$4,$5,'TUE')").bind(period).bind(bell).bind(chrono::NaiveTime::from_hms_opt(9+n as u32,0,0).unwrap()).bind(chrono::NaiveTime::from_hms_opt(10+n as u32,0,0).unwrap()).bind(n+1).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_year_id,academic_term_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) VALUES($1,$2,$3,$4,$5,$6,'TUE','STRUCTURAL','OTHER','Coverage activity')").bind(block).bind(timetable).bind(year).bind(term).bind(bell).bind(period).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_year_id,academic_term_id) VALUES(uuidv7(),$1,$2,$3,$4)").bind(block).bind(teacher).bind(year).bind(term).execute(pool).await.unwrap();
    }
}
#[tokio::test]
async fn coverage_exact_partial_consents_gate_approval_and_keep_signed_revision_stable() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_exact_partial_consents").await;
    structural_coverage_slots(&pool, owner.user_id, 2).await;
    let (t, _) = typ(&pool, &admin, "covered", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 17).await;
    let lessons = draft
        .coverage
        .days
        .iter()
        .flat_map(|d| &d.lessons)
        .collect::<Vec<_>>();
    assert_eq!(lessons.len(), 2);
    let options = coverage::candidates(
        &pool,
        &owner,
        draft.request.id,
        LeaveCoverageCandidatesRequest {
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            lesson_keys: lessons
                .iter()
                .map(|l| l.lesson.lesson_key.clone())
                .collect(),
            search: None,
            subject_group_id: None,
            grade_level_id: None,
            same_subject_group_only: false,
            same_student_group_only: false,
            same_grade_only: false,
        },
    )
    .await
    .unwrap();
    assert!(options
        .candidates
        .iter()
        .any(|c| c.teacher_id == other.user_id));
    let planned = coverage::save(
        &pool,
        &owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: lessons
                .iter()
                .map(|l| LeaveCoverageSelection {
                    lesson_key: l.lesson.lesson_key.clone(),
                    replacement_teacher_id: other.user_id,
                    mode: LeaveCoverageMode::Substitution,
                    taught_offering_id: None,
                    taught_activity: Some("กิจกรรมเสริม".into()),
                    return_lesson_key: None,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    let submitted = submit(&pool, &owner, &planned).await;
    assert!(!submitted
        .available_actions
        .contains(&ProcessAction::Approve));
    assert!(service::act(
        &pool,
        &admin,
        submitted.request.id,
        action(&submitted, ProcessAction::Approve)
    )
    .await
    .is_err());
    let tasks = coverage::assigned(&pool, &other, LeaveCoverageTaskQuery::default())
        .await
        .unwrap();
    assert_eq!(tasks.items.len(), 1);
    let task = &tasks.items[0];
    assert_eq!(task.assignments.len(), 2);
    let ids = task
        .assignments
        .iter()
        .map(|l| l.assignment.as_ref().unwrap().id)
        .collect::<Vec<_>>();
    let input = ConfirmLeaveCoverageRequest {
        request_id: Uuid::now_v7(),
        expected_revision: task.plan_revision,
        assignment_ids: vec![ids[0]],
        decision: LeaveCoverageConsent::Confirmed,
    };
    let partial = coverage::confirm(&pool, &other, submitted.request.id, input.clone())
        .await
        .unwrap();
    assert_eq!(
        partial
            .assignments
            .iter()
            .filter(|l| l.assignment.as_ref().unwrap().consent == LeaveCoverageConsent::Confirmed)
            .count(),
        1
    );
    coverage::confirm(&pool, &other, submitted.request.id, input.clone())
        .await
        .unwrap();
    assert!(coverage::confirm(
        &pool,
        &other,
        submitted.request.id,
        ConfirmLeaveCoverageRequest {
            assignment_ids: vec![ids[1]],
            ..input.clone()
        }
    )
    .await
    .is_err());
    assert!(coverage::confirm(
        &pool,
        &other,
        submitted.request.id,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            assignment_ids: vec![ids[1], Uuid::now_v7()],
            ..input.clone()
        }
    )
    .await
    .is_err());
    let still = service::detail(&pool, &owner, submitted.request.id)
        .await
        .unwrap();
    assert_eq!(still.request.revision, submitted.request.revision);
    assert_eq!(still.content_hash, submitted.content_hash);
    coverage::confirm(
        &pool,
        &other,
        submitted.request.id,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            assignment_ids: vec![ids[1]],
            ..input
        },
    )
    .await
    .unwrap();
    let ready = service::detail(&pool, &admin, submitted.request.id)
        .await
        .unwrap();
    assert!(ready.available_actions.contains(&ProcessAction::Approve));
    let approved = service::act(
        &pool,
        &admin,
        ready.request.id,
        action(&ready, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(approved.request.status, LeaveStatus::Approved);
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT state FROM academic_teaching_coverage_duties WHERE request_id=$1",
    )
    .bind(approved.request.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(statuses.iter().all(|s| s == "approved"));
}
#[tokio::test]
async fn coverage_decline_returns_owner_to_revision_without_withdrawal() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_decline_replan").await;
    structural_coverage_slots(&pool, owner.user_id, 1).await;
    let (t, _) = typ(&pool, &admin, "replan", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 17).await;
    let lesson = &draft.coverage.days[0].lessons[0];
    let planned = coverage::save(
        &pool,
        &owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: vec![LeaveCoverageSelection {
                lesson_key: lesson.lesson.lesson_key.clone(),
                replacement_teacher_id: other.user_id,
                mode: LeaveCoverageMode::Substitution,
                taught_offering_id: None,
                taught_activity: Some("กิจกรรม".into()),
                return_lesson_key: None,
            }],
        },
    )
    .await
    .unwrap();
    let sent = submit(&pool, &owner, &planned).await;
    let task = coverage::assigned(&pool, &other, LeaveCoverageTaskQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    coverage::confirm(
        &pool,
        &other,
        sent.request.id,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: task.plan_revision,
            assignment_ids: vec![task.assignments[0].assignment.as_ref().unwrap().id],
            decision: LeaveCoverageConsent::Declined,
        },
    )
    .await
    .unwrap();
    let returned = coverage::replan(
        &pool,
        &owner,
        sent.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: sent.request.revision,
        },
    )
    .await
    .unwrap();
    assert_eq!(returned.request.status, LeaveStatus::Returned);
    assert_eq!(
        returned.instance.as_ref().unwrap().status,
        ProcessStatus::RevisionRequested
    );
    assert!(returned
        .history
        .iter()
        .any(|h| h.action == "coverage_revision_requested"));
    let live:i64=sqlx::query_scalar("SELECT count(*) FROM academic_teaching_coverage_duties WHERE request_id=$1 AND state<>'released'").bind(sent.request.id).fetch_one(&pool).await.unwrap();
    assert_eq!(live, 0);
}

#[tokio::test]
async fn coverage_replan_retains_only_unchanged_exact_assignment_receipt() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_replan_preserves_exact_consent").await;
    structural_coverage_slots(&pool, owner.user_id, 2).await;
    let (t, _) = typ(&pool, &admin, "retain_consent", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 17).await;
    let selections = draft
        .coverage
        .days
        .iter()
        .flat_map(|d| &d.lessons)
        .map(|l| LeaveCoverageSelection {
            lesson_key: l.lesson.lesson_key.clone(),
            replacement_teacher_id: other.user_id,
            mode: LeaveCoverageMode::Substitution,
            taught_offering_id: None,
            taught_activity: Some("Exact activity".into()),
            return_lesson_key: None,
        })
        .collect::<Vec<_>>();
    let planned = coverage::save(
        &pool,
        &owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: selections.clone(),
        },
    )
    .await
    .unwrap();
    let sent = submit(&pool, &owner, &planned).await;
    let task = coverage::assigned(&pool, &other, LeaveCoverageTaskQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    let first = task.assignments[0].assignment.as_ref().unwrap().id;
    let second = task.assignments[1].assignment.as_ref().unwrap().id;
    for (id, decision) in [
        (first, LeaveCoverageConsent::Confirmed),
        (second, LeaveCoverageConsent::Declined),
    ] {
        coverage::confirm(
            &pool,
            &other,
            sent.request.id,
            ConfirmLeaveCoverageRequest {
                request_id: Uuid::now_v7(),
                expected_revision: task.plan_revision,
                assignment_ids: vec![id],
                decision,
            },
        )
        .await
        .unwrap();
    }
    let original:Vec<(Uuid,String)>=sqlx::query_as("SELECT id,md5(row_to_json(c)::text) FROM staff_leave_coverage_confirmations c WHERE request_id=$1 ORDER BY id").bind(sent.request.id).fetch_all(&pool).await.unwrap();
    let returned = coverage::replan(
        &pool,
        &owner,
        sent.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: sent.request.revision,
        },
    )
    .await
    .unwrap();
    let mut revised = selections;
    revised[1].replacement_teacher_id = admin.user_id;
    let updated = coverage::save(
        &pool,
        &owner,
        sent.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: returned.request.revision,
            source_digest: returned.coverage.source_digest.clone().unwrap(),
            assignments: revised,
        },
    )
    .await
    .unwrap();
    let assignments = updated
        .coverage
        .days
        .iter()
        .flat_map(|d| &d.lessons)
        .map(|l| l.assignment.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(assignments[0].id, first);
    assert_eq!(assignments[0].consent, LeaveCoverageConsent::Confirmed);
    assert_eq!(
        assignments[0].confirmation_plan_revision,
        Some(task.plan_revision)
    );
    assert_ne!(assignments[1].id, second);
    assert_eq!(assignments[1].consent, LeaveCoverageConsent::Pending);
    let resubmitted = submit(&pool, &owner, &updated).await;
    assert_eq!(
        resubmitted.coverage.days[0].lessons[0]
            .assignment
            .as_ref()
            .unwrap()
            .consent,
        LeaveCoverageConsent::Confirmed
    );
    let preserved:Vec<(Uuid,String)>=sqlx::query_as("SELECT id,md5(row_to_json(c)::text) FROM staff_leave_coverage_confirmations c WHERE request_id=$1 ORDER BY id").bind(sent.request.id).fetch_all(&pool).await.unwrap();
    assert_eq!(original, preserved);
    assert!(sqlx::query(
        "UPDATE staff_leave_coverage_confirmations SET decision='declined' WHERE id=$1"
    )
    .bind(original[0].0)
    .execute(&pool)
    .await
    .is_err());
}
#[tokio::test]
async fn coverage_emergency_manager_completes_plan_then_starts_original_owner_workflow() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_emergency_authority").await;
    structural_coverage_slots(&pool, owner.user_id, 1).await;
    let (t, _) = typ(&pool, &admin, "emergency", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 17).await;
    let sent = service::submit(
        &pool,
        &owner,
        draft.request.id,
        SubmitLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            emergency_reason: Some("ฉุกเฉิน ให้ผู้จัดการจัดผู้สอน".into()),
        },
    )
    .await
    .unwrap();
    assert!(sent.instance.is_none());
    let admin_view = service::detail(&pool, &admin, sent.request.id)
        .await
        .unwrap();
    assert!(admin_view.coverage.can_manage);
    let reserved:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(d) ORDER BY id)::text FROM academic_teaching_coverage_duties d WHERE request_id=$1").bind(sent.request.id).fetch_one(&pool).await.unwrap();
    let refresh = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: sent.request.revision,
    };
    assert!(
        coverage::refresh(&pool, &other, sent.request.id, refresh.clone())
            .await
            .is_err()
    );
    let mut unreadable = admin.clone();
    unreadable
        .permissions
        .retain(|p| p != codes::STAFF_LEAVE_READ_SCHOOL);
    assert!(
        coverage::refresh(&pool, &unreadable, sent.request.id, refresh.clone())
            .await
            .is_err()
    );
    let sent = coverage::refresh(&pool, &admin, sent.request.id, refresh)
        .await
        .unwrap();
    assert!(sent.instance.is_none());
    assert!(sent.coverage.can_manage);
    assert_eq!(
        sent.coverage.emergency_reason,
        admin_view.coverage.emergency_reason
    );
    let unchanged:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(d) ORDER BY id)::text FROM academic_teaching_coverage_duties d WHERE request_id=$1").bind(sent.request.id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        reserved, unchanged,
        "refresh must not release, reserve or rebind emergency absence duties"
    );
    let input = SaveLeaveCoverageRequest {
        request_id: Uuid::now_v7(),
        expected_revision: sent.request.revision,
        source_digest: sent.coverage.source_digest.clone().unwrap(),
        assignments: vec![LeaveCoverageSelection {
            lesson_key: sent.coverage.days[0].lessons[0].lesson.lesson_key.clone(),
            replacement_teacher_id: other.user_id,
            mode: LeaveCoverageMode::Substitution,
            taught_offering_id: None,
            taught_activity: Some("Emergency activity".into()),
            return_lesson_key: None,
        }],
    };
    assert!(
        coverage::save(&pool, &other, sent.request.id, input.clone())
            .await
            .is_err()
    );
    let managed = coverage::save(&pool, &admin, sent.request.id, input)
        .await
        .unwrap();
    assert_eq!(managed.instance.as_ref().unwrap().created_by, owner.user_id);
    assert!(managed
        .history
        .iter()
        .any(|h| h.action == "coverage_planned" && h.actor_id == admin.user_id));
    assert!(!managed.available_actions.contains(&ProcessAction::Approve));
    let withdrawn = service::cancel(
        &pool,
        &owner,
        managed.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: managed.request.revision,
            reason: "Withdraw emergency".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(withdrawn.request.status, LeaveStatus::Cancelled);
}

#[tokio::test]
async fn coverage_edit_reconciles_stored_plan_dates_and_valid_context_atomically() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_update_context_reconcile").await;
    structural_coverage_slots(&pool, owner.user_id, 1).await;
    let (t, _) = typ(&pool, &admin, "update_plan", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 17).await;
    let planned = coverage::save(
        &pool,
        &owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: vec![LeaveCoverageSelection {
                lesson_key: draft.coverage.days[0].lessons[0].lesson.lesson_key.clone(),
                replacement_teacher_id: other.user_id,
                mode: LeaveCoverageMode::Substitution,
                taught_offering_id: None,
                taught_activity: Some("Edited activity".into()),
                return_lesson_key: None,
            }],
        },
    )
    .await
    .unwrap();
    let mut changed = period(t.id, 24);
    let updated = service::update(
        &pool,
        &owner,
        planned.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: planned.request.revision,
            period: changed.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.coverage.days[0].date, date(24));
    assert_ne!(
        updated.coverage.source_digest,
        planned.coverage.source_digest
    );
    assert!(updated.coverage.days[0].lessons[0].assignment.is_none());
    let current_hash = updated.content_hash.clone();
    changed.academic_term_id = Some(Uuid::now_v7());
    assert!(service::update(
        &pool,
        &owner,
        updated.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: updated.request.revision,
            period: changed.clone()
        }
    )
    .await
    .is_err());
    assert_eq!(
        service::detail(&pool, &owner, updated.request.id)
            .await
            .unwrap()
            .content_hash,
        current_hash
    );
    let next = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) SELECT $1,academic_year_id,'Second context',2,'SECOND-COVERAGE','custom',false,false,'planning',bell_schedule_id,school_days FROM academic_terms WHERE id=$2").bind(next).bind(updated.request.academic_term_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetables(academic_term_id,academic_year_id,bell_schedule_id) SELECT id,academic_year_id,bell_schedule_id FROM academic_terms WHERE id=$1").bind(next).execute(&pool).await.unwrap();
    changed.academic_term_id = Some(next);
    let rebound = service::update(
        &pool,
        &owner,
        updated.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: updated.request.revision,
            period: changed.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(rebound.coverage.academic_term_id, Some(next));
    assert!(rebound.coverage.days[0].lessons.is_empty());
    assert_eq!(
        rebound.coverage.timetable_state,
        LeaveCoverageTimetableState::Ready
    );
    let sent = submit(&pool, &owner, &rebound).await;
    assert_eq!(sent.request.status, LeaveStatus::Submitted);
}
#[tokio::test]
async fn coverage_existing_legacy_absence_denies_candidate_without_rewriting_evidence() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_legacy_absence_candidate").await;
    structural_coverage_slots(&pool, owner.user_id, 1).await;
    let (t, _) = typ(&pool, &admin, "legacy_absence", LeaveUnit::Day, None).await;
    let draft = create(&pool, &owner, t.id, 17).await;
    let mut legacy = draft.request.clone();
    legacy.id = Uuid::now_v7();
    legacy.owner_id = other.user_id;
    legacy.owner_name = "Legacy absent teacher".into();
    legacy.status = LeaveStatus::Approved;
    legacy.academic_year_id = None;
    legacy.academic_term_id = None;
    legacy.half_day_boundary = None;
    legacy.day_portion = LeaveDayPortion::Morning;
    let legacy_hash = process::payload_hash(&legacy).unwrap();
    sqlx::query("INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,amount,content_hash) VALUES($1,$2,$3,'approved',1,$4,1,$5)").bind(legacy.id).bind(other.user_id).bind(t.id).bind(sqlx::types::Json(&legacy)).bind(&legacy_hash).execute(&pool).await.unwrap();
    let before: String = sqlx::query_scalar(
        "SELECT document::text||content_hash FROM staff_leave_requests WHERE id=$1",
    )
    .bind(legacy.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let choices = coverage::candidates(
        &pool,
        &owner,
        draft.request.id,
        LeaveCoverageCandidatesRequest {
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            lesson_keys: vec![draft.coverage.days[0].lessons[0].lesson.lesson_key.clone()],
            search: None,
            subject_group_id: None,
            grade_level_id: None,
            same_subject_group_only: false,
            same_student_group_only: false,
            same_grade_only: false,
        },
    )
    .await
    .unwrap();
    assert!(!choices
        .candidates
        .iter()
        .any(|c| c.teacher_id == other.user_id));
    let old = service::detail(&pool, &other, legacy.id).await.unwrap();
    assert_eq!(old.coverage.state, LeaveCoverageState::LegacyUnverified);
    assert_eq!(old.content_hash, legacy_hash);
    let after: String = sqlx::query_scalar(
        "SELECT document::text||content_hash FROM staff_leave_requests WHERE id=$1",
    )
    .bind(legacy.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn coverage_legacy_draft_binding_rejects_unknown_closed_context_without_partial_write() {
    let (pool, owner, admin, _) = fixture("coverage_legacy_context_binding").await;
    let (t, _) = typ(&pool, &admin, "bind_legacy", LeaveUnit::Day, None).await;
    let original = create(&pool, &owner, t.id, 17).await;
    let mut legacy = original.request;
    legacy.id = Uuid::now_v7();
    legacy.academic_year_id = None;
    legacy.academic_term_id = None;
    legacy.half_day_boundary = None;
    let hash = process::payload_hash(&legacy).unwrap();
    sqlx::query("INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,amount,content_hash) VALUES($1,$2,$3,'draft',1,$4,1,$5)").bind(legacy.id).bind(owner.user_id).bind(t.id).bind(sqlx::types::Json(&legacy)).bind(&hash).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO staff_leave_revisions(request_id,revision,document,content_hash) VALUES($1,1,$2,$3)").bind(legacy.id).bind(sqlx::types::Json(&legacy)).bind(&hash).execute(&pool).await.unwrap();
    let mut p = period(t.id, 17);
    p.academic_term_id = Some(Uuid::now_v7());
    assert!(service::update(
        &pool,
        &owner,
        legacy.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            period: p
        }
    )
    .await
    .is_err());
    let after = service::detail(&pool, &owner, legacy.id).await.unwrap();
    assert_eq!(after.request.academic_term_id, None);
    assert_eq!(after.content_hash, hash);
    let closed = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) SELECT $1,academic_year_id,'Closed context',3,'CLOSED-COVERAGE','custom',false,false,'closed',bell_schedule_id,school_days FROM academic_terms WHERE id=$2").bind(closed).bind(period(t.id,17).academic_term_id).execute(&pool).await.unwrap();
    let mut p = period(t.id, 17);
    p.academic_term_id = Some(closed);
    assert!(matches!(
        service::update(
            &pool,
            &owner,
            legacy.id,
            UpdateLeaveRequest {
                request_id: Uuid::now_v7(),
                expected_revision: 1,
                period: p
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let bound = service::update(
        &pool,
        &owner,
        legacy.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            period: period(t.id, 17),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        bound.request.academic_term_id,
        period(t.id, 17).academic_term_id
    );
    let preserved: String = sqlx::query_scalar(
        "SELECT content_hash FROM staff_leave_revisions WHERE request_id=$1 AND revision=1",
    )
    .bind(legacy.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preserved, hash);
}

#[tokio::test]
async fn coverage_closed_context_allows_legal_retirement_and_correction_but_not_teaching_approval()
{
    let (pool, owner, admin, _) = fixture("coverage_closed_retirement").await;
    let (t, _) = typ(&pool, &admin, "closed_retirement", LeaveUnit::Day, None).await;
    let first = create(&pool, &owner, t.id, 17).await;
    let first = submit(&pool, &owner, &first).await;
    let approved = service::act(
        &pool,
        &admin,
        first.request.id,
        action(&first, ProcessAction::Approve),
    )
    .await
    .unwrap();
    let second = create(&pool, &owner, t.id, 18).await;
    let second = submit(&pool, &owner, &second).await;
    let withdrawn = create(&pool, &owner, t.id, 19).await;
    let correction = create(&pool, &owner, t.id, 20).await;
    let next = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) SELECT $1,academic_year_id,'Open correction target',2,'CORRECTION-TARGET','custom',false,false,'planning',bell_schedule_id,school_days FROM academic_terms WHERE id=$2").bind(next).bind(approved.request.academic_term_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetables(academic_year_id,academic_term_id,bell_schedule_id) SELECT academic_year_id,id,bell_schedule_id FROM academic_terms WHERE id=$1").bind(next).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(approved.request.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    // Administrative approval activates teaching and still requires an open target.
    assert!(service::act(
        &pool,
        &admin,
        second.request.id,
        action(&second, ProcessAction::Approve)
    )
    .await
    .is_err());
    let unchanged = service::detail(&pool, &owner, second.request.id)
        .await
        .unwrap();
    assert_eq!(unchanged.request.status, LeaveStatus::Submitted);
    assert_eq!(unchanged.request.revision, second.request.revision);
    assert_eq!(unchanged.content_hash, second.content_hash);
    // Rejection/withdrawal retire work and remain legal under the same ordered locks.
    let rejected = service::act(
        &pool,
        &admin,
        second.request.id,
        action(&second, ProcessAction::Reject),
    )
    .await
    .unwrap();
    assert_eq!(rejected.request.status, LeaveStatus::Rejected);
    let cancel = |d: &LeaveRequestDetail| CancelLeaveRequest {
        request_id: Uuid::now_v7(),
        expected_revision: d.request.revision,
        reason: "Retire closed-context request".into(),
    };
    let withdrawn = service::cancel(&pool, &owner, withdrawn.request.id, cancel(&withdrawn))
        .await
        .unwrap();
    assert_eq!(withdrawn.request.status, LeaveStatus::Cancelled);
    let pending = service::cancel(&pool, &owner, approved.request.id, cancel(&approved))
        .await
        .unwrap();
    assert_eq!(pending.request.status, LeaveStatus::CancellationPending);
    let cancelled = service::act(
        &pool,
        &admin,
        pending.request.id,
        action(&pending, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(cancelled.request.status, LeaveStatus::Cancelled);
    // The old closed context is only retired; the new editable target is validated.
    let mut target = period(t.id, 20);
    target.academic_term_id = Some(next);
    let corrected = service::update(
        &pool,
        &owner,
        correction.request.id,
        UpdateLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: correction.request.revision,
            period: target,
        },
    )
    .await
    .unwrap();
    assert_eq!(corrected.request.academic_term_id, Some(next));
    assert_eq!(corrected.coverage.academic_term_id, Some(next));
    assert!(corrected.coverage.days.iter().all(|d| d.lessons.is_empty()));
}

#[tokio::test]
async fn coverage_refresh_recovers_missing_to_ready_zero_without_dirty_selection() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (pool, owner, admin, other) = fixture("coverage_refresh_verified_zero").await;
    let (t, _) = typ(&pool, &admin, "refresh_zero", LeaveUnit::Day, None).await;
    let context = period(t.id, 17);
    sqlx::query("DELETE FROM academic_timetables WHERE academic_term_id=$1")
        .bind(context.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    let draft = create(&pool, &owner, t.id, 17).await;
    let missing = coverage::refresh(
        &pool,
        &owner,
        draft.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        missing.coverage.timetable_state,
        LeaveCoverageTimetableState::Missing
    );
    sqlx::query("INSERT INTO academic_timetables(academic_year_id,academic_term_id,bell_schedule_id) SELECT academic_year_id,id,bell_schedule_id FROM academic_terms WHERE id=$1").bind(context.academic_term_id).execute(&pool).await.unwrap();
    let read = service::detail(&pool, &owner, missing.request.id)
        .await
        .unwrap();
    assert_eq!(
        read.coverage.timetable_state,
        LeaveCoverageTimetableState::Missing
    );
    assert_eq!(read.coverage.revision, missing.coverage.revision);
    let attempt = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: missing.request.revision,
    };
    let fresh = coverage::refresh(&pool, &owner, missing.request.id, attempt.clone())
        .await
        .unwrap();
    assert_eq!(
        fresh.coverage.timetable_state,
        LeaveCoverageTimetableState::Ready
    );
    assert_eq!(fresh.coverage.state, LeaveCoverageState::Confirmed);
    assert!(fresh.coverage.days.iter().all(|d| d.lessons.is_empty()));
    assert_eq!(fresh.request.revision, missing.request.revision + 1);
    assert_eq!(fresh.coverage.revision, missing.coverage.revision + 1);
    let replay = coverage::refresh(&pool, &owner, fresh.request.id, attempt.clone())
        .await
        .unwrap();
    assert_eq!(replay.request.revision, fresh.request.revision);
    assert!(coverage::refresh(
        &pool,
        &owner,
        fresh.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            ..attempt.clone()
        }
    )
    .await
    .is_err());
    assert!(
        coverage::refresh(
            &pool,
            &owner,
            fresh.request.id,
            LeaveMutationRequest {
                expected_revision: fresh.request.revision,
                ..attempt
            }
        )
        .await
        .is_err(),
        "same key with changed revision is not a replay"
    );
    assert!(coverage::refresh(
        &pool,
        &other,
        fresh.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: fresh.request.revision
        }
    )
    .await
    .is_err());
    let unchanged = coverage::refresh(
        &pool,
        &owner,
        fresh.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: fresh.request.revision,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        unchanged.coverage.source_digest,
        fresh.coverage.source_digest
    );
    assert_eq!(unchanged.request.revision, fresh.request.revision + 1);
    let sent = submit(&pool, &owner, &unchanged).await;
    assert_eq!(sent.request.status, LeaveStatus::Submitted);
    let revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_leave_coverage_plan_revisions WHERE request_id=$1",
    )
    .bind(sent.request.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revisions, 4);
}

mod codes {
    include!("leave/permission_codes_generated.rs");
}
