//! Reconstruct valid migration160 evidence through owning acceptance primitives.
//! These fixtures are historical data, never a legacy public write adapter.
use crate::test_permission_codes as codes;
use crate::{configuration, coverage, coverage_models::*, models::*, repository, service};
use chrono::{NaiveDate, NaiveTime, Utc};
use school_authorization::ActorContext;
use school_staff::career::StaffPersonnelType;
use school_workflow::{process, process_models::*};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

struct Legacy {
    pool: PgPool,
    owner: ActorContext,
    manager: ActorContext,
    teacher: ActorContext,
    policy: LeavePolicyVersion,
    request: Uuid,
    replay: SubmitLeaveRequest,
    year: Uuid,
    term: Uuid,
}
fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 12, day).unwrap()
}
fn policy_input(revision: i64, manager: Uuid, cap: Option<i64>) -> SaveLeavePolicyRequest {
    SaveLeavePolicyRequest {
        request_id: Uuid::now_v7(),
        expected_revision: revision,
        effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        source_reference: "Explicit school policy fixture, not statutory allowances".into(),
        enabled: true,
        personnel_types: vec![StaffPersonnelType::CivilServant],
        unit: LeavePolicyUnit::Day,
        counting_mode: LeaveCountingMode::CalendarDays,
        working_weekdays: vec![],
        excluded_dates: vec![],
        quota_year_start_month: 10,
        max_per_request: Some(1),
        annual_limit: cap,
        evidence_required: false,
        evidence_from_amount: None,
        minimum_notice_days: None,
        steps: vec![ProcessTemplateStep {
            title: "Approve".into(),
            kind: ProcessStepKind::Approve,
            completion_policy: ProcessCompletionPolicy::Any,
            recipients: vec![ProcessRecipient {
                kind: ProcessRecipientKind::User,
                user_id: Some(manager),
                organization_unit_id: None,
                position_code: None,
            }],
            due_after_days: None,
            requires_signature: false,
            requires_comment: false,
            requires_evidence: false,
            requires_delegated_authority: false,
        }],
    }
}
async fn fixture(name: &str, portion: LeaveDayPortion) -> Legacy {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 5).await;
    school_test_db::cutover_test_support::apply_migrations_through(&pool, 160)
        .await
        .unwrap();
    let year = Uuid::now_v7();
    let term = Uuid::now_v7();
    let bell = Uuid::now_v7();
    let table = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_years(id,year,name,status,is_visible) VALUES($1,2569,'Legacy evidence','planning',true)").bind(year).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO bell_schedules(id,academic_year_id,code,name) VALUES($1,$2,'LEGACY-DAY','Legacy evidence')").bind(bell).bind(year).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days,is_visible) VALUES($1,$2,'Legacy context',1,'LEGACY-DAY','regular',true,true,'planning',$3,'MON,TUE,WED,THU,FRI',true)").bind(term).bind(year).bind(bell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetables(id,academic_year_id,academic_term_id,bell_schedule_id) VALUES($1,$2,$3,$4)").bind(table).bind(year).bind(term).bind(bell).execute(&pool).await.unwrap();
    let mut actors = vec![];
    for n in 0..3 {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'fixture-not-a-login',$2,'Legacy','staff','active')").bind(id).bind(format!("legacy-{n}-{}",id.simple())).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO staff_career_history(user_id,kind,personnel_type,effective_date,source) VALUES($1,'personnel_type','civil_servant','2020-01-01','staff_entry')").bind(id).execute(&pool).await.unwrap();
        actors.push(ActorContext {
            user_id: id,
            permissions: vec![
                codes::STAFF_LEAVE_CREATE_OWN.into(),
                codes::STAFF_LEAVE_READ_OWN.into(),
                codes::STAFF_LEAVE_READ_ASSIGNED.into(),
                codes::STAFF_LEAVE_UPDATE_ASSIGNED.into(),
                codes::STAFF_LEAVE_DOWNLOAD_OWN.into(),
            ],
        });
    }
    let owner = actors.remove(0);
    let mut manager = actors.remove(0);
    let teacher = actors.remove(0);
    manager.permissions.extend([
        codes::STAFF_LEAVE_MANAGE_SCHOOL.into(),
        codes::STAFF_LEAVE_READ_SCHOOL.into(),
        codes::PROCESS_MANAGE_SCHOOL.into(),
    ]);
    for (n, hour) in [(1, 9), (2, 13)] {
        let slot = Uuid::now_v7();
        let block = Uuid::now_v7();
        sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,$3,$4,$5,$6,NULL)").bind(slot).bind(bell).bind(format!("Period{n}")).bind(NaiveTime::from_hms_opt(hour,0,0).unwrap()).bind(NaiveTime::from_hms_opt(hour+1,0,0).unwrap()).bind(n).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) VALUES($1,$2,$3,$4,$5,$6,'TUE','STRUCTURAL','OTHER','Original lesson')").bind(block).bind(table).bind(term).bind(year).bind(bell).bind(slot).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_term_id,academic_year_id) VALUES(uuidv7(),$1,$2,$3,$4)").bind(block).bind(owner.user_id).bind(term).bind(year).execute(&pool).await.unwrap();
    }
    let typ = configuration::save_type(
        &pool,
        &manager,
        None,
        SaveLeaveTypeRequest {
            request_id: Uuid::now_v7(),
            expected_revision: None,
            code: Some("legacy_days".into()),
            name: "Original leave".into(),
            kind: LeaveKind::Leave,
            archived: false,
        },
    )
    .await
    .unwrap();
    let current = configuration::save_policy(
        &pool,
        &manager,
        typ.id,
        policy_input(typ.revision, manager.user_id, None),
    )
    .await
    .unwrap();
    let mut policy = current.clone();
    policy.id = Uuid::now_v7();
    policy.version += 1;
    policy.type_revision += 1;
    policy.unit = LeaveUnit::HalfDay;
    policy.max_per_request = Some(3);
    policy.annual_limit = Some(3);
    policy.source_reference = "Original immutable half-day policy, odd caps".into();
    sqlx::query("INSERT INTO staff_leave_policies(id,type_id,version,effective_from,document,created_by) VALUES($1,$2,$3,$4,$5,$6)").bind(policy.id).bind(typ.id).bind(policy.version).bind(policy.effective_from).bind(Json(&policy)).bind(manager.user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO staff_leave_policy_categories(policy_id,type_id,personnel_type,effective_from) VALUES($1,$2,'civil_servant',$3)").bind(policy.id).bind(typ.id).bind(policy.effective_from).execute(&pool).await.unwrap();
    sqlx::query("UPDATE staff_leave_types SET revision=$2,document=jsonb_set(document,'{revision}',to_jsonb($2::bigint)) WHERE id=$1").bind(typ.id).bind(policy.type_revision).execute(&pool).await.unwrap();
    sqlx::query("UPDATE staff_leave_coverage_settings SET half_day_boundary=TIME '11:30'")
        .execute(&pool)
        .await
        .unwrap();
    let request = Uuid::now_v7();
    let summary = LeaveRequestSummary {
        destination: None,
        companions: vec![],
        companion_revision: None,
        academic_year_id: Some(year),
        academic_term_id: Some(term),
        half_day_boundary: Some(NaiveTime::from_hms_opt(11, 30, 0).unwrap()),
        id: request,
        type_id: typ.id,
        type_name: typ.name,
        kind: LeaveKind::Leave,
        owner_id: owner.user_id,
        owner_name: "Original applicant".into(),
        status: LeaveStatus::Draft,
        revision: 1,
        policy_version_id: None,
        personnel_type: Some(StaffPersonnelType::CivilServant),
        start_date: date(1),
        end_date: date(1),
        start_time: None,
        end_time: None,
        day_portion: portion,
        amount: if portion == LeaveDayPortion::FullDay {
            2
        } else {
            1
        },
        unit: LeaveUnit::HalfDay,
        reason: "Original legal scope".into(),
        created_at: Utc::now(),
    };
    let mut r = repository::Record {
        content_hash: process::payload_hash(&summary).unwrap(),
        document: Json(summary),
        policy_id: None,
        instance_id: None,
        cancellation_id: None,
        evidence_file_ids: vec![],
    };
    let mut tx = pool.begin().await.unwrap();
    school_academic_timetable::services::dated_teaching::lock_context(&mut tx, year, term)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,amount,content_hash) VALUES($1,$2,$3,'draft',1,$4,$5,$6)").bind(request).bind(owner.user_id).bind(typ.id).bind(&r.document).bind(r.document.amount).bind(&r.content_hash).execute(&mut *tx).await.unwrap();
    repository::store(&mut tx, &r).await.unwrap();
    repository::history(&mut tx, &owner, &r, "created", None, None, None)
        .await
        .unwrap();
    let replay = SubmitLeaveRequest {
        request_id: Uuid::now_v7(),
        expected_revision: 1,
        emergency_reason: Some("Original accepted emergency".into()),
    };
    repository::claim(
        &mut tx,
        owner.user_id,
        replay.request_id,
        "request_submit",
        request,
        &(request, &replay),
    )
    .await
    .unwrap();
    r.document.status = LeaveStatus::Submitted;
    r.document.revision = 2;
    r.document.policy_version_id = Some(policy.id);
    r.policy_id = Some(policy.id);
    let plan = coverage::prepare_submission(&mut tx, &r, replay.emergency_reason.as_deref())
        .await
        .unwrap();
    let files: Vec<LeaveFileSnapshot> = vec![];
    r.content_hash = process::payload_hash(&(
        repository::period_from(&r),
        &policy,
        StaffPersonnelType::CivilServant,
        &files,
        &plan.plan_hash,
    ))
    .unwrap();
    sqlx::query("UPDATE staff_leave_requests SET quota_year_start='2026-10-01' WHERE id=$1")
        .bind(request)
        .execute(&mut *tx)
        .await
        .unwrap();
    repository::store(&mut tx, &r).await.unwrap();
    repository::history(&mut tx, &owner, &r, "submitted", None, None, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Legacy {
        pool,
        owner,
        manager,
        teacher,
        policy,
        request,
        replay,
        year,
        term,
    }
}
async fn detail(f: &Legacy) -> LeaveRequestDetail {
    service::detail(&f.pool, &f.owner, f.request).await.unwrap()
}
async fn migrate(f: &Legacy) {
    school_test_db::run_test_migrations(&f.pool).await;
}
async fn complete_and_confirm(f: &Legacy) -> LeaveRequestDetail {
    let d = detail(f).await;
    let managed = coverage::save(
        &f.pool,
        &f.manager,
        f.request,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: d.request.revision,
            source_digest: d.coverage.source_digest.clone().unwrap(),
            assignments: d
                .coverage
                .days
                .iter()
                .flat_map(|d| &d.lessons)
                .map(|l| LeaveCoverageSelection {
                    lesson_key: l.lesson.lesson_key.clone(),
                    replacement_teacher_id: f.teacher.user_id,
                    mode: LeaveCoverageMode::Substitution,
                    taught_offering_id: None,
                    taught_activity: Some("Original accepted teaching activity".into()),
                    return_lesson_key: None,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    coverage::confirm(
        &f.pool,
        &f.teacher,
        f.request,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: managed.coverage.revision,
            assignment_ids: managed
                .coverage
                .days
                .iter()
                .flat_map(|d| &d.lessons)
                .map(|l| l.assignment.as_ref().unwrap().id)
                .collect(),
            decision: LeaveCoverageConsent::Confirmed,
        },
    )
    .await
    .unwrap();
    detail(f).await
}
fn action(d: &LeaveRequestDetail, kind: ProcessAction) -> LeaveActionRequest {
    LeaveActionRequest {
        request_id: Uuid::now_v7(),
        expected_revision: d.request.revision,
        expected_process_revision: d.instance.as_ref().unwrap().revision,
        purpose: d.process_purpose,
        action: kind,
        comment: Some("Historical evidence continuation".into()),
        signature: None,
    }
}
async fn successor(f: &Legacy, cap: Option<i64>) -> LeavePolicyVersion {
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM staff_leave_types WHERE id=$1")
        .bind(f.policy.type_id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    configuration::save_policy(
        &f.pool,
        &f.manager,
        f.policy.type_id,
        policy_input(revision, f.manager.user_id, cap),
    )
    .await
    .unwrap()
}
async fn snapshot(f: &Legacy) -> String {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM staff_leave_policies p),(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM staff_leave_requests r),(SELECT jsonb_agg(to_jsonb(r) ORDER BY request_id,revision) FROM staff_leave_revisions r),(SELECT jsonb_agg(to_jsonb(h) ORDER BY id) FROM staff_leave_history h),(SELECT jsonb_agg(to_jsonb(p) ORDER BY request_id,revision) FROM staff_leave_coverage_plan_revisions p),(SELECT jsonb_agg(to_jsonb(a) ORDER BY request_id,plan_revision,assignment_id) FROM staff_leave_coverage_assignment_revisions a),(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM staff_leave_coverage_confirmations c),(SELECT jsonb_agg(to_jsonb(d) ORDER BY id) FROM academic_teaching_coverage_duties d),(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM process_instances i),(SELECT jsonb_agg(to_jsonb(l) ORDER BY request_id,revision,file_id) FROM staff_leave_file_links l))::text").fetch_one(&f.pool).await.unwrap()
}
#[tokio::test]
async fn full_day_legacy_signed_approval_and_id_replay_survive_161_without_rehash() {
    let f = fixture("full_day_legacy_signed", LeaveDayPortion::Morning).await;
    let d = complete_and_confirm(&f).await;
    let mut approve = action(&d, ProcessAction::Approve);
    approve.signature = Some(LeaveSignatureConsent {
        consent: true,
        image_file_id: None,
        input: SignatureInput {
            method: SignatureMethod::Drawn,
            asset_id: None,
            strokes: vec![InkStroke {
                points: vec![InkPoint { x: 0.1, y: 0.1 }, InkPoint { x: 0.2, y: 0.2 }],
                width: 0.005,
                color: "#000000".into(),
            }],
            page: 1,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            document_revision: d.request.revision,
            content_hash: d.content_hash.clone(),
        },
    });
    let approved = service::act(&f.pool, &f.manager, f.request, approve.clone())
        .await
        .unwrap();
    assert_eq!(approved.request.amount, 1);
    assert_eq!(approved.request.unit, LeaveUnit::HalfDay);
    let before = snapshot(&f).await;
    sqlx::query("CREATE VIEW full_day_unexpected_boundary_dependency AS SELECT half_day_boundary FROM staff_leave_coverage_settings").execute(&f.pool).await.unwrap();
    assert!(
        school_test_db::cutover_test_support::apply_migrations_through(&f.pool, 161)
            .await
            .is_err(),
        "migration must refuse unexpected dependencies instead of cascading into evidence"
    );
    assert_eq!(snapshot(&f).await, before);
    let boundary: Option<NaiveTime> =
        sqlx::query_scalar("SELECT half_day_boundary FROM staff_leave_coverage_settings")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(boundary, Some(NaiveTime::from_hms_opt(11, 30, 0).unwrap()));
    sqlx::query("DROP VIEW full_day_unexpected_boundary_dependency")
        .execute(&f.pool)
        .await
        .unwrap();
    migrate(&f).await;
    assert_eq!(snapshot(&f).await, before);
    let read = detail(&f).await;
    assert_eq!(read.content_hash, approved.content_hash);
    assert_eq!(read.request.day_portion, LeaveDayPortion::Morning);
    assert!(serde_json::to_value(&read.coverage)
        .unwrap()
        .get("halfDayBoundary")
        .is_none());
    service::submit(&f.pool, &f.owner, f.request, f.replay.clone())
        .await
        .unwrap();
    service::act(&f.pool, &f.manager, f.request, approve)
        .await
        .unwrap();
    assert_eq!(snapshot(&f).await, before);
    let pending = service::cancel(
        &f.pool,
        &f.owner,
        f.request,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: read.request.revision,
            reason: "Legal cancellation after retirement".into(),
        },
    )
    .await
    .unwrap();
    let cancelled = service::act(
        &f.pool,
        &f.manager,
        f.request,
        action(&pending, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(cancelled.request.unit, LeaveUnit::HalfDay);
    assert_eq!(cancelled.request.amount, 1);
    assert_eq!(cancelled.request.status, LeaveStatus::Cancelled);
}
#[tokio::test]
async fn full_day_legacy_boundary_refresh_and_explicit_revision_keep_exact_receipt_provenance() {
    for (name, portion) in [
        ("full_day_old_full_capture", LeaveDayPortion::FullDay),
        ("full_day_explicit_widen", LeaveDayPortion::Morning),
    ] {
        let f = fixture(name, portion).await;
        let d = complete_and_confirm(&f).await;
        let first = d.coverage.days[0].lessons[0]
            .assignment
            .as_ref()
            .unwrap()
            .clone();
        let returned = service::act(
            &f.pool,
            &f.manager,
            f.request,
            action(&d, ProcessAction::ReturnForRevision),
        )
        .await
        .unwrap();
        let before = snapshot(&f).await;
        migrate(&f).await;
        assert_eq!(snapshot(&f).await, before);
        let initial = detail(&f).await;
        assert!(!initial.can_submit);
        assert!(initial.submission_blocking_reason.is_some());
        assert!(
            configuration::effective_policy(&f.pool, &f.owner, f.policy.type_id, date(1))
                .await
                .is_err()
        );
        if portion == LeaveDayPortion::FullDay {
            let refreshed = coverage::refresh(
                &f.pool,
                &f.owner,
                f.request,
                LeaveMutationRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: returned.request.revision,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                refreshed.coverage.days[0].lessons[0]
                    .assignment
                    .as_ref()
                    .unwrap()
                    .confirmation_id,
                first.confirmation_id
            );
        } else {
            assert!(coverage::refresh(
                &f.pool,
                &f.owner,
                f.request,
                LeaveMutationRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: returned.request.revision
                }
            )
            .await
            .is_err());
        }
        let before_policy = detail(&f).await;
        let period = LeavePeriodInput {
            destination: None,
            companion_user_ids: vec![],
            academic_year_id: Some(f.year),
            academic_term_id: Some(f.term),
            type_id: f.policy.type_id,
            start_date: date(1),
            end_date: date(1),
            start_time: None,
            end_time: None,
            reason: "Explicit whole-day choice before policy configuration".into(),
            evidence_file_ids: vec![],
        };
        assert!(
            matches!(service::update(&f.pool,&f.owner,f.request,UpdateLeaveRequest{request_id:Uuid::now_v7(),expected_revision:before_policy.request.revision,period:period.clone()}).await,Err(school_errors::AppError::ValidationError(message)) if message.contains("เกณฑ์หน่วยวันฉบับใหม่"))
        );
        assert!(
            matches!(service::create(&f.pool,&f.owner,CreateLeaveRequest{request_id:Uuid::now_v7(),period}).await,Err(school_errors::AppError::ValidationError(message)) if message.contains("เกณฑ์หน่วยวันฉบับใหม่")),
            "must not fall back to the older Day policy"
        );
        successor(&f, Some(2)).await;
        let current = detail(&f).await;
        assert!(service::submit(
            &f.pool,
            &f.owner,
            f.request,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: current.request.revision
            }
        )
        .await
        .is_err());
        let revised = service::update(
            &f.pool,
            &f.owner,
            f.request,
            UpdateLeaveRequest {
                request_id: Uuid::now_v7(),
                expected_revision: current.request.revision,
                period: LeavePeriodInput {
                    destination: None,
                    companion_user_ids: vec![],
                    academic_year_id: Some(f.year),
                    academic_term_id: Some(f.term),
                    type_id: f.policy.type_id,
                    start_date: date(1),
                    end_date: date(1),
                    start_time: None,
                    end_time: None,
                    reason: "Explicit owner choice: whole day".into(),
                    evidence_file_ids: vec![],
                },
            },
        )
        .await
        .unwrap();
        assert_eq!(revised.request.unit, LeaveUnit::Day);
        assert_eq!(revised.request.amount, 1);
        assert_eq!(revised.coverage.days[0].lessons.len(), 2);
        let kept = revised.coverage.days[0].lessons[0]
            .assignment
            .as_ref()
            .unwrap();
        assert_eq!(kept.id, first.id);
        assert_eq!(kept.confirmation_id, first.confirmation_id);
        assert_eq!(kept.confirmed_at, first.confirmed_at);
        assert_eq!(
            kept.confirmation_plan_revision,
            first.confirmation_plan_revision
        );
        if portion == LeaveDayPortion::Morning {
            assert!(revised.coverage.days[0].lessons[1].assignment.is_none());
            assert!(service::submit(
                &f.pool,
                &f.owner,
                f.request,
                LeaveMutationRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: revised.request.revision
                }
            )
            .await
            .is_err());
        }
        let mut connection = f.pool.acquire().await.unwrap();
        let stored = coverage::stored_plan(&mut connection, f.request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.half_day_boundary,
            Some(NaiveTime::from_hms_opt(11, 30, 0).unwrap())
        );
        let captured = stored.plan_hash.clone();
        let mut unhashed = stored;
        unhashed.plan_hash.clear();
        assert_eq!(captured, process::payload_hash(&unhashed).unwrap());
        drop(connection);
        let ready = if portion == LeaveDayPortion::Morning {
            coverage::save(
                &f.pool,
                &f.owner,
                f.request,
                SaveLeaveCoverageRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: revised.request.revision,
                    source_digest: revised.coverage.source_digest.clone().unwrap(),
                    assignments: revised.coverage.days[0]
                        .lessons
                        .iter()
                        .map(|l| LeaveCoverageSelection {
                            lesson_key: l.lesson.lesson_key.clone(),
                            replacement_teacher_id: f.teacher.user_id,
                            mode: LeaveCoverageMode::Substitution,
                            taught_offering_id: None,
                            taught_activity: Some("Original accepted teaching activity".into()),
                            return_lesson_key: None,
                        })
                        .collect(),
                },
            )
            .await
            .unwrap()
        } else {
            revised
        };
        let sent = service::submit(
            &f.pool,
            &f.owner,
            f.request,
            LeaveMutationRequest {
                request_id: Uuid::now_v7(),
                expected_revision: ready.request.revision,
            },
        )
        .await
        .unwrap();
        let pending = sent.coverage.days[0]
            .lessons
            .iter()
            .filter_map(|l| l.assignment.as_ref())
            .filter(|a| a.consent == LeaveCoverageConsent::Pending)
            .map(|a| a.id)
            .collect::<Vec<_>>();
        assert_eq!(
            pending.len(),
            usize::from(portion == LeaveDayPortion::Morning)
        );
        if !pending.is_empty() {
            coverage::confirm(
                &f.pool,
                &f.teacher,
                f.request,
                ConfirmLeaveCoverageRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: sent.coverage.revision,
                    assignment_ids: pending,
                    decision: LeaveCoverageConsent::Confirmed,
                },
            )
            .await
            .unwrap();
        }
        let ready = detail(&f).await;
        let done = service::act(
            &f.pool,
            &f.manager,
            f.request,
            action(&ready, ProcessAction::Approve),
        )
        .await
        .unwrap();
        assert_eq!(done.request.unit, LeaveUnit::Day);
        assert_eq!(done.request.day_portion, LeaveDayPortion::FullDay);
        assert_eq!(done.request.amount, 1);
        assert_eq!(
            done.coverage.days[0].lessons[0]
                .assignment
                .as_ref()
                .unwrap()
                .confirmation_id,
            first.confirmation_id
        );
    }
}
#[tokio::test]
async fn full_day_mixed_quota_and_reports_use_exact_scale_and_one_concurrent_winner() {
    let f = fixture("full_day_mixed_quota", LeaveDayPortion::Morning).await;
    let d = complete_and_confirm(&f).await;
    service::act(
        &f.pool,
        &f.manager,
        f.request,
        action(&d, ProcessAction::Approve),
    )
    .await
    .unwrap();
    migrate(&f).await;
    let original: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM staff_leave_policies WHERE id=$1")
            .bind(f.policy.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let p = successor(&f, Some(2)).await;
    assert_eq!(p.type_id, f.policy.type_id);
    let mut requests = vec![];
    for day in [2, 3] {
        requests.push(
            service::create(
                &f.pool,
                &f.owner,
                CreateLeaveRequest {
                    request_id: Uuid::now_v7(),
                    period: LeavePeriodInput {
                        destination: None,
                        companion_user_ids: vec![],
                        academic_year_id: Some(f.year),
                        academic_term_id: Some(f.term),
                        type_id: p.type_id,
                        start_date: date(day),
                        end_date: date(day),
                        start_time: None,
                        end_time: None,
                        reason: "New whole-day request".into(),
                        evidence_file_ids: vec![],
                    },
                },
            )
            .await
            .unwrap(),
        );
    }
    let a = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: requests[0].request.revision,
    };
    let b = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: requests[1].request.revision,
    };
    let (ar, br) = tokio::join!(
        service::submit(&f.pool, &f.owner, requests[0].request.id, a.clone()),
        service::submit(&f.pool, &f.owner, requests[1].request.id, b.clone())
    );
    assert_ne!(ar.is_ok(), br.is_ok());
    let (winner, loser, retry) = if let Ok(winner) = ar {
        (winner, requests[1].request.id, b)
    } else {
        (br.unwrap(), requests[0].request.id, a)
    };
    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_leave_receipts WHERE actor_id=$1 AND request_id=$2",
    )
    .bind(f.owner.user_id)
    .bind(retry.request_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(failed, 0);
    let report = service::usage(
        &f.pool,
        &f.owner,
        LeaveUsageQuery {
            from: date(1),
            to: date(31),
            kind: Some(LeaveKind::Leave),
        },
    )
    .await
    .unwrap();
    assert_eq!(report.items.len(), 1);
    let row = &report.items[0];
    assert_eq!(row.unit, LeavePolicyUnit::Day);
    assert_eq!(row.amount_scale, 2);
    assert_eq!(row.approved_amount, "1");
    assert_eq!(row.reserved_amount, "2");
    assert_eq!(row.annual_limit.as_deref(), Some("4"));
    assert_eq!(row.policy_version_id, p.id);
    service::cancel(
        &f.pool,
        &f.owner,
        winner.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: winner.request.revision,
            reason: "Release only new reservation".into(),
        },
    )
    .await
    .unwrap();
    service::submit(&f.pool, &f.owner, loser, retry)
        .await
        .unwrap();
    let huge = successor(&f, Some(i64::MAX)).await;
    let d = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: LeavePeriodInput {
                destination: None,
                companion_user_ids: vec![],
                academic_year_id: Some(f.year),
                academic_term_id: Some(f.term),
                type_id: p.type_id,
                start_date: date(4),
                end_date: date(4),
                start_time: None,
                end_time: None,
                reason: "Exact large cap".into(),
                evidence_file_ids: vec![],
            },
        },
    )
    .await
    .unwrap();
    service::submit(
        &f.pool,
        &f.owner,
        d.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: d.request.revision,
        },
    )
    .await
    .unwrap();
    let report = service::usage(
        &f.pool,
        &f.owner,
        LeaveUsageQuery {
            from: date(1),
            to: date(31),
            kind: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        report.items[0].annual_limit.as_deref(),
        Some("18446744073709551614")
    );
    assert_eq!(report.items[0].policy_version_id, huge.id);
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM staff_leave_policies WHERE id=$1")
            .bind(f.policy.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(original, after);
    assert_eq!(after["annualLimit"], 3);
    assert_eq!(after["maxPerRequest"], 3);
    let cleared = successor(&f, None).await;
    assert!(cleared.annual_limit.is_none());
    let mut wrong = policy_input(cleared.type_revision, f.manager.user_id, Some(2));
    wrong.quota_year_start_month = 1;
    assert!(
        configuration::save_policy(&f.pool, &f.manager, f.policy.type_id, wrong)
            .await
            .is_err(),
        "same-type successor cannot reset the quota month"
    );
}

#[test]
fn full_day_codecs_preserve_original_optional_period_and_null_plan_shapes() {
    let period = r#"{"typeId":"00000000-0000-0000-0000-000000000001","startDate":"2026-12-01","endDate":"2026-12-01","startTime":null,"endTime":null,"dayPortion":"morning","reason":"Original evidence","evidenceFileIds":[]}"#;
    let stored: StoredLeavePeriod = serde_json::from_str(period).unwrap();
    assert_eq!(serde_json::to_string(&stored).unwrap(), period);
    let plan = r#"{"revision":1,"planHash":"original","state":"incomplete","sourceDigest":null,"academicYearId":null,"academicTermId":null,"halfDayBoundary":null,"timetableState":"legacy","days":[],"canManage":false,"blockingReason":null,"emergencyReason":null}"#;
    let stored: StoredLeaveCoveragePlan = serde_json::from_str(plan).unwrap();
    assert_eq!(serde_json::to_string(&stored).unwrap(), plan);
    let active: LeaveCoveragePlan = stored.into();
    assert!(serde_json::to_value(active)
        .unwrap()
        .get("halfDayBoundary")
        .is_none());
    for field in ["maxPerRequest", "annualLimit", "evidenceFromAmount"] {
        let mut fractional =
            serde_json::to_value(policy_input(1, Uuid::now_v7(), Some(2))).unwrap();
        fractional[field] = serde_json::json!(1.5);
        assert!(
            serde_json::from_value::<SaveLeavePolicyRequest>(fractional).is_err(),
            "fractional active Day limit must require an explicit whole-day choice"
        );
    }
}
