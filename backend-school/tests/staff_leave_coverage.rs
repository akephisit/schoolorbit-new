#[path = "../crates/school-staff-leave/tests/support/leave_fixture.rs"]
mod support;
use chrono::NaiveDate;
use school_authorization::ActorContext;
use school_staff_leave::{models::*, service};
use school_workflow::process_models::*;
use sqlx::PgPool;
use support::*;
use uuid::Uuid;

struct AcademicCoverageFixture {
    pool: PgPool,
    owner: ActorContext,
    admin: ActorContext,
    teacher: ActorContext,
    co_teacher: ActorContext,
    year: Uuid,
    term: Uuid,
    math: Uuid,
    english: Uuid,
    math_group: Uuid,
    english_group: Uuid,
    other_group: Uuid,
    allocation: Uuid,
    return_allocation: Uuid,
    pupils: Vec<Uuid>,
    from: NaiveDate,
    back: NaiveDate,
}
async fn coverage_actor(pool: &PgPool, name: &str, manager: bool) -> ActorContext {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic',$3,'Coverage','staff','active')").bind(id).bind(id.to_string()).bind(name).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO staff_career_history(user_id,kind,personnel_type,effective_date,source) VALUES($1,'personnel_type','civil_servant','2020-01-01','staff_entry')").bind(id).execute(pool).await.unwrap();
    let mut permissions = vec![
        codes::STAFF_LEAVE_READ_OWN.into(),
        codes::STAFF_LEAVE_READ_ASSIGNED.into(),
        codes::STAFF_LEAVE_CREATE_OWN.into(),
        codes::STAFF_LEAVE_UPDATE_ASSIGNED.into(),
        codes::ATTENDANCE_READ_ASSIGNED.into(),
        codes::ATTENDANCE_UPDATE_ASSIGNED.into(),
    ];
    if manager {
        permissions.extend([
            codes::STAFF_LEAVE_MANAGE_SCHOOL.into(),
            codes::STAFF_LEAVE_READ_SCHOOL.into(),
            codes::PROCESS_MANAGE_SCHOOL.into(),
        ]);
    }
    ActorContext {
        user_id: id,
        permissions,
    }
}
async fn academic_coverage_fixture(
    name: &str,
    other_room: bool,
    future_days: i64,
) -> AcademicCoverageFixture {
    use school_test_db::cutover_test_support::{
        apply_migrations_through, seed_release_two_predecessor,
    };
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 6).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 161).await.unwrap();
    let owner = coverage_actor(&pool, "Math owner", false).await;
    let admin = coverage_actor(&pool, "Manager", true).await;
    let teacher = coverage_actor(&pool, "English substitute", false).await;
    let co_teacher = coverage_actor(&pool, "Co teacher", false).await;
    let year = Uuid::from_u128(0x10000000000000000000000000000025);
    let term = Uuid::now_v7();
    let bell = Uuid::now_v7();
    sqlx::query("INSERT INTO bell_schedules(id,academic_year_id,code,name) VALUES($1,$2,'COVERAGE-ACADEMIC','Coverage academic')").bind(bell).bind(year).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) VALUES($1,$2,'Dated coverage',1000,'COVERAGE-ACADEMIC','custom',false,false,'planning',$3,'MON,TUE,WED,THU,FRI,SAT,SUN')").bind(term).bind(year).bind(bell).execute(&pool).await.unwrap();
    let timetable:Uuid=sqlx::query_scalar("INSERT INTO academic_timetables(academic_term_id,academic_year_id,bell_schedule_id) VALUES($1,$2,$3) RETURNING id").bind(term).bind(year).bind(bell).fetch_one(&pool).await.unwrap();
    let from = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
        + chrono::Duration::days(future_days);
    let back = from + chrono::Duration::days(7);
    let day = school_academic_timetable::services::daily_teaching::day_code_from_date(from);
    let mut pupils = vec![];
    for n in 0..3 {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic',$3,'Pupil','student','active')").bind(id).bind(id.to_string()).bind(format!("Coverage{n}")).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT uuidv7(),$1,academic_year_id,grade_level_id,study_program_id,'active' FROM student_academic_years WHERE academic_year_id=$2 ORDER BY id LIMIT 1").bind(id).bind(year).execute(&pool).await.unwrap();
        pupils.push(id);
    }
    let grade: Uuid = sqlx::query_scalar(
        "SELECT grade_level_id FROM student_academic_years WHERE academic_year_id=$1 LIMIT 1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    let english_subject = school_academic_core::services::catalog::create_subject(
        &pool,
        school_academic_core::models::CreateCatalogSubjectRequest {
            code: "ENG-COVERAGE".into(),
            subject_group_id: Uuid::from_u128(0x783a4a9d9ff14eacb37006b58daa1eb7),
        },
    )
    .await
    .unwrap();
    let english_version = school_academic_core::services::catalog::create_subject_version(
        &pool,
        english_subject.id,
        school_academic_core::models::CreateSubjectVersionRequest {
            name_th: "English coverage".into(),
            name_en: Some("English coverage".into()),
            credit: "1.00".into(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".into(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            term_code: None,
            periods_per_week: Some(1),
            grade_level_ids: vec![grade],
        },
    )
    .await
    .unwrap();
    school_academic_core::services::catalog::publish_subject_version(
        &pool,
        english_version.id,
        school_academic_core::models::PublishVersionRequest {
            row_version: english_version.row_version,
        },
    )
    .await
    .unwrap();
    let source:Uuid=sqlx::query_scalar("SELECT learning_offering_id FROM course_offering_details WHERE subject_version_id='20000000-0000-0000-0000-000000000025' LIMIT 1").fetch_one(&pool).await.unwrap();
    let (math, english, math_group, english_group, other_group) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    for (id, name) in [(math, "Math coverage"), (english, "English coverage")] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,weekly_period_target,owning_organization_unit_id) SELECT $1,$2,$3,'course',$4,$4,1,owning_organization_unit_id FROM learning_offerings WHERE id=$5").bind(id).bind(term).bind(year).bind(name).bind(source).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT $1,$2,$3,CASE WHEN $1=$4 THEN $5 ELSE subject_version_id END,CASE WHEN $1=$4 THEN $6 ELSE subject_id END,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$7").bind(id).bind(term).bind(year).bind(english).bind(english_version.id).bind(english_subject.id).bind(source).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO learning_offering_targets(id,learning_offering_id,academic_term_id,academic_year_id,target_kind,homeroom_id,grade_level_id,study_program_id) SELECT uuidv7(),$1,$2,$3,'homeroom',id,grade_level_id,study_program_id FROM homerooms WHERE academic_year_id=$3 ORDER BY id LIMIT 2").bind(id).bind(term).bind(year).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
    }
    for (group, offering, teacher_id, members) in [
        (math_group, math, owner.user_id, pupils[..2].to_vec()),
        (
            english_group,
            english,
            if other_room {
                co_teacher.user_id
            } else {
                teacher.user_id
            },
            pupils[..2].to_vec(),
        ),
        (other_group, english, teacher.user_id, vec![pupils[2]]),
    ] {
        sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name) VALUES($1,$2,$3,$4,$5,$5)").bind(group).bind(offering).bind(term).bind(year).bind(group.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) VALUES(uuidv7(),$1,$2,$3,$4,'primary')").bind(group).bind(term).bind(year).bind(teacher_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,published_at) SELECT uuidv7(),$1,$2,$3,id,student_id,'active','operational_change','2020-01-01',now() FROM student_academic_years WHERE academic_year_id=$3 AND student_id=ANY($4)").bind(group).bind(term).bind(year).bind(members).execute(&pool).await.unwrap();
    }
    let mut allocations = vec![];
    for (n, group, offering, instructors) in [
        (0, math_group, math, vec![owner.user_id]),
        (
            1,
            english_group,
            english,
            vec![if other_room {
                co_teacher.user_id
            } else {
                teacher.user_id
            }],
        ),
        (2, other_group, english, vec![teacher.user_id]),
    ] {
        let period = Uuid::now_v7();
        let block = Uuid::now_v7();
        let allocation = Uuid::now_v7();
        sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,$3,$4,$5,$6,NULL)").bind(period).bind(bell).bind(format!("Coverage period{n}")).bind(chrono::NaiveTime::from_hms_opt(9+n,0,0).unwrap()).bind(chrono::NaiveTime::from_hms_opt(10+n,0,0).unwrap()).bind(n as i32+1).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,scheduling_mode,learning_offering_id) VALUES($1,$2,$3,$4,$5,$6,$7,'COURSE','independent',$8)").bind(block).bind(timetable).bind(term).bind(year).bind(bell).bind(period).bind(day).bind(offering).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_groups(id,block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,homeroom_ids) VALUES($1,$2,$3,$4,$5,$6,ARRAY(SELECT id FROM homerooms WHERE academic_year_id=$6 ORDER BY id LIMIT 1))").bind(allocation).bind(block).bind(group).bind(offering).bind(term).bind(year).execute(&pool).await.unwrap();
        for (index, instructor) in instructors.iter().enumerate() {
            sqlx::query("INSERT INTO academic_timetable_block_group_instructors(id,block_group_id,instructor_id,role,display_order) VALUES(uuidv7(),$1,$2,'primary',$3)").bind(allocation).bind(instructor).bind(index as i32+1).execute(&pool).await.unwrap();
        }
        allocations.push(allocation);
    }
    AcademicCoverageFixture {
        pool,
        owner,
        admin,
        teacher,
        co_teacher,
        year,
        term,
        math,
        english,
        math_group,
        english_group,
        other_group,
        allocation: allocations[0],
        return_allocation: allocations[1],
        pupils,
        from,
        back,
    }
}
async fn academic_coverage_plan(
    f: &AcademicCoverageFixture,
    mode: school_staff_leave::coverage_models::LeaveCoverageMode,
) -> LeaveRequestDetail {
    use school_staff_leave::{coverage, coverage_models::*};
    let (t, _) = typ(&f.pool, &f.admin, "academic_coverage", LeaveUnit::Day, None).await;
    let mut period = period(t.id, 17);
    period.academic_year_id = Some(f.year);
    period.academic_term_id = Some(f.term);
    period.start_date = f.from;
    period.end_date = f.from;

    let draft = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period,
        },
    )
    .await
    .unwrap();
    let original = &draft.coverage.days[0].lessons[0].lesson;
    assert_eq!(original.allocation_id, f.allocation);
    let return_key = if mode == LeaveCoverageMode::Reciprocal {
        let choices = coverage::return_lessons(
            &f.pool,
            &f.owner,
            draft.request.id,
            LeaveCoverageReturnLessonsRequest {
                expected_revision: draft.request.revision,
                source_digest: draft.coverage.source_digest.clone().unwrap(),
                lesson_key: original.lesson_key.clone(),
                replacement_teacher_id: f.teacher.user_id,
                date: f.back,
            },
        )
        .await
        .unwrap();
        assert_eq!(choices.lessons.len(), 1);
        assert_eq!(choices.lessons[0].allocation_id, f.return_allocation);
        Some(choices.lessons[0].lesson_key.clone())
    } else {
        None
    };
    coverage::save(
        &f.pool,
        &f.owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: vec![LeaveCoverageSelection {
                lesson_key: original.lesson_key.clone(),
                replacement_teacher_id: f.teacher.user_id,
                mode,
                taught_offering_id: Some(f.english),
                taught_activity: None,
                return_lesson_key: return_key,
            }],
        },
    )
    .await
    .unwrap()
}
async fn confirm_and_approve_academic(
    f: &AcademicCoverageFixture,
    planned: &LeaveRequestDetail,
) -> LeaveRequestDetail {
    use school_staff_leave::{coverage, coverage_models::*};
    let sent = submit(&f.pool, &f.owner, planned).await;
    let task = coverage::assigned(&f.pool, &f.teacher, LeaveCoverageTaskQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    coverage::confirm(
        &f.pool,
        &f.teacher,
        sent.request.id,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: task.plan_revision,
            assignment_ids: task
                .assignments
                .iter()
                .map(|l| l.assignment.as_ref().unwrap().id)
                .collect(),
            decision: LeaveCoverageConsent::Confirmed,
        },
    )
    .await
    .unwrap();
    let ready = service::detail(&f.pool, &f.admin, sent.request.id)
        .await
        .unwrap();
    service::act(
        &f.pool,
        &f.admin,
        ready.request.id,
        action(&ready, ProcessAction::Approve),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn coverage_cross_offering_reciprocal_preserves_actual_subject_on_both_dates() {
    use school_academic_timetable::services::{attendance, daily_teaching};
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    let f = academic_coverage_fixture("coverage_cross_subject_pair", false, 7).await;
    assert_ne!(f.math_group, f.english_group);
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Reciprocal).await;
    let before = attendance::blocks(&f.pool, f.term, f.from).await.unwrap();
    assert_eq!(
        before
            .iter()
            .flat_map(|b| &b.groups)
            .find(|g| g.id == f.allocation)
            .unwrap()
            .learning_group_id,
        f.math_group
    );
    let approved = confirm_and_approve_academic(&f, &planned).await;
    let first = attendance::blocks(&f.pool, f.term, f.from).await.unwrap();
    let block = first
        .iter()
        .find(|b| b.groups.iter().any(|g| g.id == f.allocation))
        .unwrap();
    let group = &block.groups[0];
    assert_eq!(block.learning_offering_id, Some(f.english));
    assert_eq!(group.learning_group_id, f.english_group);
    let teachers = group
        .instructors
        .iter()
        .map(|t| t.teacher_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(teachers, [f.teacher.user_id].into_iter().collect());
    let returned = attendance::blocks(&f.pool, f.term, f.back).await.unwrap();
    let back = returned
        .iter()
        .find(|b| b.groups.iter().any(|g| g.id == f.return_allocation))
        .unwrap();
    assert_eq!(back.learning_offering_id, Some(f.math));
    assert_eq!(back.groups[0].learning_group_id, f.math_group);
    assert_eq!(back.groups[0].instructors[0].teacher_id, f.owner.user_id);
    let daily = daily_teaching::get_daily_teaching_overview(
        &f.pool,
        daily_teaching::DailyTeachingQuery {
            academic_term_id: f.term,
            date: Some(f.from),
            include_empty_teachers: Some(false),
        },
    )
    .await
    .unwrap();
    let entry = daily
        .teachers
        .iter()
        .find(|t| t.id == f.teacher.user_id)
        .unwrap()
        .periods
        .iter()
        .flat_map(|p| &p.entries)
        .find(|e| e.entry_id == f.allocation)
        .unwrap();
    assert_eq!(entry.offering_id, Some(f.english));
    assert_eq!(entry.learning_group_id, Some(f.english_group));
    let current:(Uuid,Uuid)=sqlx::query_as("SELECT g.learning_group_id,b.learning_offering_id FROM academic_timetable_block_groups g JOIN academic_timetable_blocks b ON b.id=g.block_id WHERE g.id=$1").bind(f.allocation).fetch_one(&f.pool).await.unwrap();
    assert_eq!(current, (f.math_group, f.math));
    let pair:i64=sqlx::query_scalar("SELECT count(*) FROM academic_teaching_coverage_duties WHERE request_id=$1 AND kind='reciprocal' AND state='approved'").bind(approved.request.id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(pair, 2);
}
#[tokio::test]
async fn coverage_own_subject_other_room_gets_exact_dated_attendance_without_other_class_access() {
    use school_attendance::{application, models::AttendanceKind, services::sessions};
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    let f = academic_coverage_fixture("coverage_own_subject_dated_access", true, 0).await;
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
    assert_eq!(
        planned.coverage.days[0].lessons[0]
            .assignment
            .as_ref()
            .unwrap()
            .taught_learning_group_id,
        Some(f.english_group)
    );
    confirm_and_approve_academic(&f, &planned).await;
    let seeds = application::seeds(&f.pool, f.term, f.from).await.unwrap();
    let seed = seeds
        .into_iter()
        .find(|s| {
            s.session.kind == AttendanceKind::Lesson
                && s.session.source_key == format!("placement:{}", f.allocation)
        })
        .unwrap();
    assert_eq!(seed.session.learning_group_id, Some(f.english_group));
    assert_eq!(seed.session.offering_id, Some(f.english));
    assert_ne!(seed.session.learning_group_id, Some(f.other_group));
    assert_eq!(
        seed.students
            .iter()
            .map(|s| s.student_id)
            .collect::<std::collections::BTreeSet<_>>(),
        f.pupils[..2].iter().copied().collect()
    );
    assert!(!seed.students.iter().any(|s| s.student_id == f.pupils[2]));
    let saved = sessions::ensure(&f.pool, &f.teacher, seed).await.unwrap();
    assert!(saved.teacher_ids.contains(&f.teacher.user_id));
    let another = application::seeds(&f.pool, f.term, f.back)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.session.source_key == format!("placement:{}", f.return_allocation))
        .unwrap();
    assert!(sessions::ensure(&f.pool, &f.teacher, another)
        .await
        .is_err());
    let frozen: String =
        sqlx::query_scalar("SELECT row_to_json(s)::text FROM attendance_sessions s WHERE id=$1")
            .bind(saved.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let owner_view = service::detail(&f.pool, &f.owner, planned.request.id)
        .await
        .unwrap();
    let cancellation = service::cancel(
        &f.pool,
        &f.owner,
        planned.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: owner_view.request.revision,
            reason: "Return to school".into(),
        },
    )
    .await
    .unwrap();
    service::act(
        &f.pool,
        &f.admin,
        cancellation.request.id,
        action(&cancellation, ProcessAction::Approve),
    )
    .await
    .unwrap();
    let unchanged: String =
        sqlx::query_scalar("SELECT row_to_json(s)::text FROM attendance_sessions s WHERE id=$1")
            .bind(saved.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(frozen, unchanged);
}
#[tokio::test]
async fn coverage_draft_audience_change_is_refused_before_any_reservation() {
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    let f = academic_coverage_fixture("coverage_stale_actual_pupils", false, 7).await;
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Reciprocal).await;
    sqlx::query("UPDATE learning_group_students SET membership_status='removed' WHERE learning_group_id=$1 AND student_id=$2").bind(f.english_group).bind(f.pupils[1]).execute(&f.pool).await.unwrap();
    assert!(service::submit(
        &f.pool,
        &f.owner,
        planned.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: planned.request.revision
        }
    )
    .await
    .is_err());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_teaching_coverage_duties WHERE request_id=$1",
    )
    .bind(planned.request.id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn coverage_reciprocal_second_leg_conflict_rolls_back_both_legs_and_submit_receipt() {
    use school_staff_leave::coverage_models::*;
    let f = academic_coverage_fixture("coverage_second_leg_atomic", false, 7).await;
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Reciprocal).await;
    let (t, _) = typ(
        &f.pool,
        &f.admin,
        "second_leg_absence",
        LeaveUnit::Day,
        None,
    )
    .await;
    let mut p = period(t.id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.back;
    p.end_date = f.back;

    let blocker = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    let blocker = service::submit(
        &f.pool,
        &f.owner,
        blocker.request.id,
        SubmitLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: blocker.request.revision,
            emergency_reason: Some("Manager must complete this separate absence".into()),
        },
    )
    .await
    .unwrap();
    let attempt = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: planned.request.revision,
    };
    assert!(
        service::submit(&f.pool, &f.owner, planned.request.id, attempt.clone())
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_teaching_coverage_duties WHERE request_id=$1",
    )
    .bind(planned.request.id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "the free first leg must not survive the unavailable second leg"
    );
    let current = service::detail(&f.pool, &f.owner, planned.request.id)
        .await
        .unwrap();
    assert_eq!(current.request.status, LeaveStatus::Draft);
    assert_eq!(current.request.revision, planned.request.revision);
    assert_eq!(current.coverage.plan_hash, planned.coverage.plan_hash);
    service::cancel(
        &f.pool,
        &f.owner,
        blocker.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: blocker.request.revision,
            reason: "Free the return date".into(),
        },
    )
    .await
    .unwrap();
    // An aborted transaction does not consume the idempotency key.
    let sent = service::submit(&f.pool, &f.owner, planned.request.id, attempt)
        .await
        .unwrap();
    assert_eq!(sent.request.status, LeaveStatus::Submitted);
    let pair: (i64,i64) = sqlx::query_as("SELECT count(*),count(DISTINCT pair_id) FROM academic_teaching_coverage_duties WHERE request_id=$1 AND pair_id IS NOT NULL AND state='reserved'").bind(sent.request.id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(pair, (2, 1));
}

#[tokio::test]
async fn coverage_shared_allocation_keeps_co_teacher_original_subject() {
    use school_staff_leave::{coverage, coverage_models::*};
    let f = academic_coverage_fixture("coverage_team_original_content", false, 7).await;
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) VALUES(uuidv7(),$1,$2,$3,$4,'secondary')").bind(f.math_group).bind(f.term).bind(f.year).bind(f.co_teacher.user_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_group_instructors(id,block_group_id,instructor_id,role,display_order) VALUES(uuidv7(),$1,$2,'secondary',2)").bind(f.allocation).bind(f.co_teacher.user_id).execute(&f.pool).await.unwrap();
    let (t, _) = typ(&f.pool, &f.admin, "team_content", LeaveUnit::Day, None).await;
    let mut p = period(t.id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;

    let draft = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    let mut input = SaveLeaveCoverageRequest {
        request_id: Uuid::now_v7(),
        expected_revision: draft.request.revision,
        source_digest: draft.coverage.source_digest.clone().unwrap(),
        assignments: vec![LeaveCoverageSelection {
            lesson_key: draft.coverage.days[0].lessons[0].lesson.lesson_key.clone(),
            replacement_teacher_id: f.teacher.user_id,
            mode: LeaveCoverageMode::Substitution,
            taught_offering_id: Some(f.english),
            taught_activity: None,
            return_lesson_key: None,
        }],
    };
    assert!(
        coverage::save(&f.pool, &f.owner, draft.request.id, input.clone())
            .await
            .is_err()
    );
    input.request_id = Uuid::now_v7();
    input.assignments[0].taught_offering_id = Some(f.math);
    let planned = coverage::save(&f.pool, &f.owner, draft.request.id, input)
        .await
        .unwrap();
    confirm_and_approve_academic(&f, &planned).await;
    let daily = school_academic_timetable::services::daily_teaching::get_daily_teaching_overview(
        &f.pool,
        school_academic_timetable::services::daily_teaching::DailyTeachingQuery {
            academic_term_id: f.term,
            date: Some(f.from),
            include_empty_teachers: None,
        },
    )
    .await
    .unwrap();
    for teacher in [f.co_teacher.user_id, f.teacher.user_id] {
        let entry = daily
            .teachers
            .iter()
            .find(|t| t.id == teacher)
            .unwrap()
            .periods
            .iter()
            .flat_map(|p| &p.entries)
            .find(|e| e.entry_id == f.allocation)
            .unwrap();
        assert_eq!(entry.offering_id, Some(f.math));
        assert_eq!(entry.learning_group_id, Some(f.math_group));
    }
}

#[tokio::test]
async fn coverage_joint_source_block_reserves_each_allocation_as_one_teacher_interval() {
    use school_staff_leave::{coverage, coverage_models::*};
    let f = academic_coverage_fixture("coverage_joint_source_allocations", true, 7).await;
    let group = Uuid::now_v7();
    let allocation = Uuid::now_v7();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name) VALUES($1,$2,$3,$4,'JOINT','Joint second audience')").bind(group).bind(f.math).bind(f.term).bind(f.year).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) VALUES(uuidv7(),$1,$2,$3,$4,'primary')").bind(group).bind(f.term).bind(f.year).bind(f.owner.user_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,published_at) SELECT uuidv7(),$1,$2,$3,id,student_id,'active','operational_change','2020-01-01',now() FROM student_academic_years WHERE academic_year_id=$3 AND student_id=$4").bind(group).bind(f.term).bind(f.year).bind(f.pupils[2]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_groups(id,block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,homeroom_ids) SELECT $1,block_id,$2,learning_offering_id,academic_term_id,academic_year_id,homeroom_ids FROM academic_timetable_block_groups WHERE id=$3").bind(allocation).bind(group).bind(f.allocation).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_group_instructors(id,block_group_id,instructor_id,role,display_order) VALUES(uuidv7(),$1,$2,'primary',1)").bind(allocation).bind(f.owner.user_id).execute(&f.pool).await.unwrap();
    let (t, _) = typ(&f.pool, &f.admin, "joint_source", LeaveUnit::Day, None).await;
    let mut p = period(t.id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;

    let draft = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    let lessons = &draft.coverage.days[0].lessons;
    assert_eq!(lessons.len(), 2);
    assert_eq!(lessons[0].lesson.block_id, lessons[1].lesson.block_id);
    assert_ne!(
        lessons[0].lesson.allocation_id,
        lessons[1].lesson.allocation_id
    );
    let planned = coverage::save(
        &f.pool,
        &f.owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: lessons
                .iter()
                .map(|l| LeaveCoverageSelection {
                    lesson_key: l.lesson.lesson_key.clone(),
                    replacement_teacher_id: f.teacher.user_id,
                    mode: LeaveCoverageMode::Substitution,
                    taught_offering_id: Some(f.math),
                    taught_activity: None,
                    return_lesson_key: None,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    let approved = confirm_and_approve_academic(&f, &planned).await;
    let rows:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT source_allocation_id,teacher_id FROM academic_teaching_coverage_duties WHERE request_id=$1 AND kind='substitution' AND state='approved' ORDER BY source_allocation_id").bind(approved.request.id).fetch_all(&f.pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.1 == f.teacher.user_id));
    assert_eq!(
        rows.iter()
            .map(|r| r.0)
            .collect::<std::collections::BTreeSet<_>>(),
        [f.allocation, allocation].into_iter().collect()
    );
}
#[tokio::test]
async fn coverage_materialized_attendance_accepts_exact_pupil_grant_without_changing_snapshot() {
    use school_attendance::{
        application,
        models::{AttendanceKind, AttendanceQuery},
        services::sessions,
    };
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    let f = academic_coverage_fixture("coverage_saved_attendance_overlay", true, 0).await;
    let before = application::seeds(&f.pool, f.term, f.from)
        .await
        .unwrap()
        .into_iter()
        .find(|s| {
            s.session.kind == AttendanceKind::Lesson
                && s.session.source_key == format!("placement:{}", f.allocation)
        })
        .unwrap();
    let saved = sessions::ensure(&f.pool, &f.owner, before).await.unwrap();
    let initial = sessions::get(&f.pool, saved.id).await.unwrap();
    let reference = school_attendance::models::OpenAttendanceSession {
        academic_term_id: f.term,
        date: f.from,
        kind: AttendanceKind::Lesson,
        source_key: initial.source_key.clone(),
    };
    sessions::save_with_seed(
        &f.pool,
        &f.owner,
        initial.id,
        school_attendance::models::SaveAttendanceResults {
            session: reference.clone(),
            roster_token: initial.roster_token.clone(),
            row_version: initial.row_version,
            reason: "Initial owner mark".into(),
            students: vec![school_attendance::models::AttendanceResultInput {
                student_id: f.pupils[0],
                result: school_attendance::models::AttendanceResult::Present,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let frozen: String =
        sqlx::query_scalar("SELECT row_to_json(s)::text FROM attendance_sessions s WHERE id=$1")
            .bind(saved.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
    confirm_and_approve_academic(&f, &planned).await;
    let listed = sessions::list(
        &f.pool,
        &f.teacher,
        &AttendanceQuery {
            academic_term_id: f.term,
            date: f.from,
        },
    )
    .await
    .unwrap();
    assert!(listed.iter().any(|s| s.id == saved.id));
    let workspace = application::workspace(
        &f.pool,
        &f.teacher,
        AttendanceQuery {
            academic_term_id: f.term,
            date: f.from,
        },
    )
    .await
    .unwrap();
    let found = workspace
        .sessions
        .iter()
        .find(|s| s.id == saved.id)
        .unwrap();
    assert_eq!(found.teacher_ids, saved.teacher_ids);
    assert!(!found.teacher_ids.contains(&f.teacher.user_id));
    school_attendance::policy::require_session_on_connection(
        &mut *f.pool.acquire().await.unwrap(),
        &f.teacher,
        found,
        true,
    )
    .await
    .unwrap();
    let unchanged: String =
        sqlx::query_scalar("SELECT row_to_json(s)::text FROM attendance_sessions s WHERE id=$1")
            .bind(saved.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(frozen, unchanged);
    let mut forged = found.clone();
    forged.date = f.back;
    assert!(school_attendance::policy::require_session_on_connection(
        &mut *f.pool.acquire().await.unwrap(),
        &f.teacher,
        &forged,
        true
    )
    .await
    .is_err());
    let mut wrong_pupils = found.clone();
    wrong_pupils.roster[0].student_id = f.pupils[2];
    assert!(school_attendance::policy::require_session_on_connection(
        &mut *f.pool.acquire().await.unwrap(),
        &f.teacher,
        &wrong_pupils,
        true
    )
    .await
    .is_err());
    let current = sessions::get(&f.pool, saved.id).await.unwrap();
    let owner_result:String=sqlx::query_scalar("SELECT row_to_json(r)::text FROM attendance_records r WHERE session_id=$1 AND student_id=$2").bind(saved.id).bind(f.pupils[0]).fetch_one(&f.pool).await.unwrap();
    let marked = sessions::save_with_seed(
        &f.pool,
        &f.teacher,
        saved.id,
        school_attendance::models::SaveAttendanceResults {
            session: reference,
            roster_token: current.roster_token,
            row_version: current.row_version,
            reason: "Substitute records actual attendance".into(),
            students: vec![school_attendance::models::AttendanceResultInput {
                student_id: f.pupils[1],
                result: school_attendance::models::AttendanceResult::Late,
            }],
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(marked.session.teacher_ids, saved.teacher_ids);
    assert_eq!(
        marked
            .students
            .iter()
            .find(|p| p.student_id == f.pupils[1])
            .unwrap()
            .result,
        school_attendance::models::AttendanceResult::Late
    );
    let before_cancel:String=sqlx::query_scalar("SELECT jsonb_build_array((SELECT to_jsonb(s) FROM attendance_sessions s WHERE id=$1),(SELECT jsonb_agg(to_jsonb(r) ORDER BY student_id) FROM attendance_records r WHERE session_id=$1))::text").bind(saved.id).fetch_one(&f.pool).await.unwrap();
    let owner_view = service::detail(&f.pool, &f.owner, planned.request.id)
        .await
        .unwrap();
    let cancellation = service::cancel(
        &f.pool,
        &f.owner,
        planned.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: owner_view.request.revision,
            reason: "Cancel covered absence".into(),
        },
    )
    .await
    .unwrap();
    service::act(
        &f.pool,
        &f.admin,
        cancellation.request.id,
        action(&cancellation, ProcessAction::Approve),
    )
    .await
    .unwrap();
    let after_cancel:String=sqlx::query_scalar("SELECT jsonb_build_array((SELECT to_jsonb(s) FROM attendance_sessions s WHERE id=$1),(SELECT jsonb_agg(to_jsonb(r) ORDER BY student_id) FROM attendance_records r WHERE session_id=$1))::text").bind(saved.id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(before_cancel, after_cancel);
    let original_result:String=sqlx::query_scalar("SELECT row_to_json(r)::text FROM attendance_records r WHERE session_id=$1 AND student_id=$2").bind(saved.id).bind(f.pupils[0]).fetch_one(&f.pool).await.unwrap();
    assert_eq!(owner_result, original_result);
}

#[tokio::test]
async fn coverage_balanced_cancellation_releases_future_pair_and_retains_owed_started_pair() {
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    for (name, offset, expected) in [
        ("coverage_cancel_future_pair", 7, "released"),
        ("coverage_cancel_started_pair", -7, "approved"),
    ] {
        let f = academic_coverage_fixture(name, false, offset).await;
        let planned = academic_coverage_plan(&f, LeaveCoverageMode::Reciprocal).await;
        let approved = confirm_and_approve_academic(&f, &planned).await;
        let cancellation = service::cancel(
            &f.pool,
            &f.owner,
            approved.request.id,
            CancelLeaveRequest {
                request_id: Uuid::now_v7(),
                expected_revision: approved.request.revision,
                reason: "Cancel absence".into(),
            },
        )
        .await
        .unwrap();
        let cancelled = service::act(
            &f.pool,
            &f.admin,
            cancellation.request.id,
            action(&cancellation, ProcessAction::Approve),
        )
        .await
        .unwrap();
        assert_eq!(cancelled.request.status, LeaveStatus::Cancelled);
        let states:Vec<String>=sqlx::query_scalar("SELECT state FROM academic_teaching_coverage_duties WHERE request_id=$1 AND kind='reciprocal' ORDER BY pair_leg").bind(approved.request.id).fetch_all(&f.pool).await.unwrap();
        assert_eq!(states, vec![expected.to_string(), expected.to_string()]);
    }
}
#[tokio::test]
async fn coverage_global_dated_reservation_has_one_concurrent_winner_and_failed_receipt_rolls_back()
{
    use school_staff_leave::{coverage, coverage_models::*};
    let f = academic_coverage_fixture("coverage_global_concurrent_winner", true, 7).await;
    let first = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
    let second_owner = coverage_actor(&f.pool, "Concurrent owner", false).await;
    let another_block = Uuid::now_v7();
    let another_allocation = Uuid::now_v7();
    let mut tx = f.pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) SELECT $1,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,'STRUCTURAL','OTHER','Concurrent duty' FROM academic_timetable_blocks WHERE id=(SELECT block_id FROM academic_timetable_block_groups WHERE id=$2)").bind(another_block).bind(f.allocation).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_term_id,academic_year_id) VALUES($1,$2,$3,$4,$5)").bind(another_allocation).bind(another_block).bind(second_owner.user_id).bind(f.term).bind(f.year).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let mut p = period(first.request.type_id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;

    let second = service::create(
        &f.pool,
        &second_owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    let second = coverage::save(
        &f.pool,
        &second_owner,
        second.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: second.request.revision,
            source_digest: second.coverage.source_digest.clone().unwrap(),
            assignments: vec![LeaveCoverageSelection {
                lesson_key: second.coverage.days[0].lessons[0].lesson.lesson_key.clone(),
                replacement_teacher_id: f.teacher.user_id,
                mode: LeaveCoverageMode::Substitution,
                taught_offering_id: None,
                taught_activity: Some("Concurrent activity".into()),
                return_lesson_key: None,
            }],
        },
    )
    .await
    .unwrap();
    let a = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: first.request.revision,
    };
    let b = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: second.request.revision,
    };
    let (x, y) = tokio::join!(
        service::submit(&f.pool, &f.owner, first.request.id, a.clone()),
        service::submit(&f.pool, &second_owner, second.request.id, b.clone())
    );
    assert_ne!(x.is_ok(), y.is_ok());
    let (winner, winner_actor, loser, loser_actor, loser_input) = if let Ok(winner) = x {
        (winner, &f.owner, &second, &second_owner, b)
    } else {
        (y.unwrap(), &second_owner, &first, &f.owner, a)
    };
    let live:i64=sqlx::query_scalar("SELECT count(DISTINCT request_id) FROM academic_teaching_coverage_duties WHERE teacher_id=$1 AND state='reserved'").bind(f.teacher.user_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(live, 1);
    let receipt: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM staff_leave_receipts WHERE actor_id=$1 AND request_id=$2)",
    )
    .bind(loser_actor.user_id)
    .bind(loser_input.request_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert!(!receipt);
    service::cancel(
        &f.pool,
        winner_actor,
        winner.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: winner.request.revision,
            reason: "Release concurrent promise".into(),
        },
    )
    .await
    .unwrap();
    let retried = service::submit(&f.pool, loser_actor, loser.request.id, loser_input)
        .await
        .unwrap();
    assert_eq!(retried.request.status, LeaveStatus::Submitted);
}

#[tokio::test]
async fn coverage_weekly_guard_scopes_selected_term_but_dated_occupancy_is_global() {
    use school_academic_timetable::services::dated_teaching as duties;
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    let f = academic_coverage_fixture("coverage_weekly_selected_context", true, 7).await;
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
    let sent = submit(&f.pool, &f.owner, &planned).await;
    let other_term = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) SELECT $1,academic_year_id,'Other date-free term',1001,'OTHER-DATE-FREE','custom',false,false,'planning',bell_schedule_id,school_days FROM academic_terms WHERE id=$2").bind(other_term).bind(f.term).execute(&f.pool).await.unwrap();
    let other_table:Uuid=sqlx::query_scalar("INSERT INTO academic_timetables(academic_term_id,academic_year_id,bell_schedule_id) SELECT id,academic_year_id,bell_schedule_id FROM academic_terms WHERE id=$1 RETURNING id").bind(other_term).fetch_one(&f.pool).await.unwrap();
    for (term, table, allowed) in [
        (other_term, other_table, true),
        (f.term, Uuid::nil(), false),
    ] {
        let mut tx = f.pool.begin().await.unwrap();
        let block = Uuid::now_v7();
        sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) SELECT $1,CASE WHEN $2='00000000-0000-0000-0000-000000000000'::uuid THEN b.timetable_id ELSE $2 END,$3,b.academic_year_id,b.bell_schedule_id,b.bell_schedule_period_id,b.day_of_week,'STRUCTURAL','OTHER','New weekly obligation' FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$4").bind(block).bind(table).bind(term).bind(f.allocation).execute(&mut *tx).await.unwrap();
        let outcome=sqlx::query("INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_term_id,academic_year_id) VALUES(uuidv7(),$1,$2,$3,$4)").bind(block).bind(f.teacher.user_id).bind(term).bind(f.year).execute(&mut *tx).await;
        if allowed {
            outcome.unwrap();
            tx.commit().await.unwrap();
        } else {
            assert!(outcome.is_err());
            tx.rollback().await.unwrap();
        }
    }
    let mut connection = f.pool.acquire().await.unwrap();
    let start = chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let end = chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap();
    assert!(duties::teacher_free_on_connection(
        &mut connection,
        f.term,
        f.teacher.user_id,
        f.from,
        start,
        end,
        sent.request.id,
        &[]
    )
    .await
    .unwrap());
    assert!(!duties::teacher_free_on_connection(
        &mut connection,
        f.term,
        f.teacher.user_id,
        f.from,
        start,
        end,
        Uuid::nil(),
        &[]
    )
    .await
    .unwrap());
}
#[tokio::test]
async fn coverage_migration_preserves_legacy_documents_revisions_and_hashes_without_backfill() {
    use school_test_db::cutover_test_support::apply_migrations_through;
    let pool = school_test_db::create_named_test_pool("coverage160_legacy_preservation").await;
    apply_migrations_through(&pool, 159).await.unwrap();
    let owner = coverage_actor(&pool, "Legacy migration owner", false).await;
    let approver = coverage_actor(&pool, "Legacy approver", false).await;
    let typ: Uuid = sqlx::query_scalar("SELECT id FROM staff_leave_types ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = Uuid::now_v7();
    let document = serde_json::json!({"id":id,"typeId":typ,"typeName":"Legacy","kind":"leave","ownerId":owner.user_id,"ownerName":"Legacy owner","status":"approved","revision":1,"policyVersionId":null,"personnelType":"civil_servant","startDate":"2026-11-17","endDate":"2026-11-17","startTime":null,"endTime":null,"dayPortion":"full_day","amount":1,"unit":"day","reason":"private original reason","createdAt":"2026-11-01T00:00:00Z"});
    let legacy_hash = school_workflow::process::payload_hash(&document).unwrap();
    sqlx::query("INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,amount,content_hash) VALUES($1,$2,$3,'approved',1,$4,1,$5)").bind(id).bind(owner.user_id).bind(typ).bind(&document).bind(&legacy_hash).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO staff_leave_revisions(request_id,revision,document,content_hash) VALUES($1,1,$2,$3)").bind(id).bind(&document).bind(&legacy_hash).execute(&pool).await.unwrap();
    let mut links = vec![];
    let mut signature_file = None;
    for (purpose, user) in [
        ("staff_leave_evidence", owner.user_id),
        ("staff_leave_signature", approver.user_id),
    ] {
        let file = Uuid::now_v7();
        let version = Uuid::now_v7();
        sqlx::query("INSERT INTO files(id,owner_user_id,created_by,display_filename,purpose_code,visibility,lifecycle_status,retention_class,expires_at) VALUES($1,$2,$2,'legacy.png',$3,'private','ready','temporary',now()+interval '1 hour')").bind(file).bind(user).bind(purpose).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,1,'r2','private','stored',$3,'image/png','png',100,repeat('a',64),'clean')").bind(version).bind(file).bind(format!("synthetic/legacy/{file}")).execute(&pool).await.unwrap();
        sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
            .bind(file)
            .bind(version)
            .execute(&pool)
            .await
            .unwrap();
        links.push((file, version));
        if purpose == "staff_leave_signature" {
            signature_file = Some(file);
        }
    }
    let signature = LeaveSignatureConsent {
        consent: true,
        image_file_id: signature_file,
        input: SignatureInput {
            method: SignatureMethod::Image,
            asset_id: None,
            strokes: vec![],
            page: 1,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            document_revision: 1,
            content_hash: legacy_hash.clone(),
        },
    };
    school_workflow::signatures::validate_content_bound_signature(
        &signature.input,
        true,
        1,
        &legacy_hash,
        true,
    )
    .unwrap();
    let signed = LeaveHistoryEntry {
        id: Uuid::now_v7(),
        actor_id: approver.user_id,
        actor_name: "Legacy approver".into(),
        action: "approve".into(),
        revision: 1,
        comment: Some("Original approval".into()),
        signature: Some(signature),
        content_hash: legacy_hash,
        created_at: chrono::Utc::now(),
    };
    sqlx::query(
        "INSERT INTO staff_leave_history(id,request_id,actor_id,entry) VALUES($1,$2,$3,$4)",
    )
    .bind(signed.id)
    .bind(id)
    .bind(approver.user_id)
    .bind(sqlx::types::Json(&signed))
    .execute(&pool)
    .await
    .unwrap();
    for (file, version) in links {
        sqlx::query("INSERT INTO staff_leave_file_links(request_id,revision,file_id,version_id,checksum) VALUES($1,1,$2,$3,repeat('a',64))").bind(id).bind(file).bind(version).execute(&pool).await.unwrap();
    }
    let query="SELECT jsonb_build_array((SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM staff_leave_requests r),(SELECT jsonb_agg(to_jsonb(r) ORDER BY request_id,revision) FROM staff_leave_revisions r),(SELECT jsonb_agg(to_jsonb(h) ORDER BY id) FROM staff_leave_history h),(SELECT jsonb_agg(to_jsonb(l) ORDER BY request_id,revision,file_id) FROM staff_leave_file_links l),(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM file_versions v),(SELECT jsonb_agg(to_jsonb(f) ORDER BY id) FROM files f))::text";
    let before: String = sqlx::query_scalar(query).fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 161).await.unwrap();
    let after: String = sqlx::query_scalar(query).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    school_migrations::run_tenant_migrations(&pool)
        .await
        .unwrap();
    let replay: String = sqlx::query_scalar(query).fetch_one(&pool).await.unwrap();
    assert_eq!(before, replay);
    let count:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM staff_leave_coverage_plans)+(SELECT count(*) FROM academic_teaching_coverage_duties)+(SELECT count(*) FROM staff_leave_coverage_confirmations)").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0);
}

fn coverage_http_router(pool: &PgPool, actor: &ActorContext, tenant_name: &str) -> axum::Router {
    use school_auth::{
        config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
        session_service::AuthenticatedSession,
    };
    use school_authorization::PermissionCache;
    use school_tenancy::{AdminClient, AdminClientConfig, PoolManager, TenantContext};
    use std::sync::Arc;
    let cache = Arc::new(PermissionCache::new());
    let revision = cache.snapshot_revision(tenant_name, actor.user_id);
    assert!(cache.fill_if_current(
        tenant_name,
        actor.user_id,
        revision,
        actor.permissions.clone()
    ));
    let identity = Arc::new(school_auth::session_cache::SessionCache::new());
    let runtime = AuthRuntime {
        admin_client: Arc::new(AdminClient::new(
            "http://127.0.0.1:9".into(),
            "fixture".into(),
            AdminClientConfig::from_env().unwrap(),
        )),
        pool_manager: Arc::new(PoolManager::new(school_test_db::verify_tenant_schema)),
        permission_cache: cache,
        identity_cache: identity.clone(),
        config: Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
            [31; 32],
        ))),
        session_events: tokio::sync::broadcast::channel(8).0,
        permission_events: tokio::sync::broadcast::channel(8).0,
    };
    let session = AuthenticatedSession::for_tests(
        identity,
        TenantContext {
            tenant_id: Uuid::now_v7(),
            subdomain: tenant_name.into(),
            pool: pool.clone().into(),
        },
        Uuid::now_v7(),
        actor.user_id,
        "staff",
    );
    axum::Router::new()
        .nest("/api", school_staff_http::leave::routes())
        .layer(axum::Extension(session))
        .with_state(school_staff_http::leave::LeaveHttpState {
            auth_runtime: runtime,
            work_event_channel: tokio::sync::broadcast::channel(8).0,
        })
}
#[tokio::test]
async fn coverage_http_assigned_tasks_are_redacted_recipient_scoped_and_tenant_bound() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use school_staff_leave::{coverage, coverage_models::*};
    use tower::ServiceExt;
    let f = academic_coverage_fixture("coverage_http_tenant_one", true, 7).await;
    let base:(Uuid,Uuid,String)=sqlx::query_as("SELECT b.timetable_id,b.bell_schedule_id,b.day_of_week FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$1").bind(f.allocation).fetch_one(&f.pool).await.unwrap();
    for hour in [13, 14] {
        let period = Uuid::now_v7();
        let block = Uuid::now_v7();
        let allocation = Uuid::now_v7();
        sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,'Private recipient scope',$3,$4,$5,NULL)").bind(period).bind(base.1).bind(chrono::NaiveTime::from_hms_opt(hour,0,0).unwrap()).bind(chrono::NaiveTime::from_hms_opt(hour+1,0,0).unwrap()).bind(hour as i32).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,scheduling_mode,learning_offering_id) VALUES($1,$2,$3,$4,$5,$6,$7,'COURSE','independent',$8)").bind(block).bind(base.0).bind(f.term).bind(f.year).bind(base.1).bind(period).bind(&base.2).bind(f.math).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_groups(id,block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,homeroom_ids) SELECT $1,$2,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,homeroom_ids FROM academic_timetable_block_groups WHERE id=$3").bind(allocation).bind(block).bind(f.allocation).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_group_instructors(id,block_group_id,instructor_id,role,display_order) VALUES(uuidv7(),$1,$2,'primary',1)").bind(allocation).bind(f.owner.user_id).execute(&f.pool).await.unwrap();
    }
    let (t, _) = typ(&f.pool, &f.admin, "private_tasks", LeaveUnit::Day, None).await;
    let mut p = period(t.id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;

    p.reason = "PRIVATE-ABSENCE-SENTINEL-DO-NOT-EXPOSE".into();
    let draft = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    assert_eq!(draft.coverage.days[0].lessons.len(), 3);
    let planned = coverage::save(
        &f.pool,
        &f.owner,
        draft.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.request.revision,
            source_digest: draft.coverage.source_digest.clone().unwrap(),
            assignments: draft.coverage.days[0]
                .lessons
                .iter()
                .enumerate()
                .map(|(i, l)| LeaveCoverageSelection {
                    lesson_key: l.lesson.lesson_key.clone(),
                    replacement_teacher_id: if i == 2 {
                        f.co_teacher.user_id
                    } else {
                        f.teacher.user_id
                    },
                    mode: LeaveCoverageMode::Substitution,
                    taught_offering_id: Some(f.math),
                    taught_activity: None,
                    return_lesson_key: None,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    let sent = submit(&f.pool, &f.owner, &planned).await;
    let router = coverage_http_router(&f.pool, &f.teacher, "coverage-tenant-one");
    let response = router
        .clone()
        .oneshot(
            Request::get("/api/staff-leave/coverage/assigned")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let wire = std::str::from_utf8(&bytes).unwrap();
    assert!(!wire.contains("reason"));
    assert!(!wire.contains(&sent.request.reason));
    let page: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    fn private_fields_absent(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields {
                    assert!(
                        ![
                            "reason",
                            "emergencyReason",
                            "evidenceFileIds",
                            "evidence",
                            "policy",
                            "personnelType",
                            "contentHash",
                            "ownerId"
                        ]
                        .contains(&key.as_str()),
                        "private field {key}"
                    );
                    private_fields_absent(child);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    private_fields_absent(child)
                }
            }
            serde_json::Value::String(text) => assert!(!text.contains("PRIVATE-ABSENCE-SENTINEL")),
            _ => {}
        }
    }
    private_fields_absent(&page);
    assert_eq!(page["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        page["data"]["items"][0]["assignments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let detail = router
        .clone()
        .oneshot(
            Request::get(format!("/api/staff-leave/requests/{}", sent.request.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::FORBIDDEN);
    let mut denied = f.teacher.clone();
    denied.permissions.clear();
    let denied_router = coverage_http_router(&f.pool, &denied, "coverage-tenant-denied");
    let denied = denied_router
        .oneshot(
            Request::get("/api/staff-leave/coverage/assigned")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let pool_two = school_test_db::create_named_test_pool("coverage_http_tenant_two").await;
    school_test_db::run_test_migrations(&pool_two).await;
    let user_two = coverage_actor(&pool_two, "Other tenant teacher", false).await;
    let other_router = coverage_http_router(&pool_two, &user_two, "coverage-tenant-two");
    let other = other_router
        .oneshot(
            Request::get(format!("/api/staff-leave/requests/{}", sent.request.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(other.status(), StatusCode::NOT_FOUND);
    let task = coverage::assigned(&f.pool, &f.teacher, Default::default())
        .await
        .unwrap()
        .items
        .remove(0);
    let foreign = coverage::assigned(&f.pool, &f.co_teacher, Default::default())
        .await
        .unwrap()
        .items
        .remove(0)
        .assignments[0]
        .assignment
        .as_ref()
        .unwrap()
        .id;
    let own = task.assignments[0].assignment.as_ref().unwrap().id;
    let mixed = serde_json::json!({"requestId":Uuid::now_v7(),"expectedRevision":task.plan_revision,"assignmentIds":[own,foreign],"decision":"confirmed"});
    let mixed_response = router
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/staff-leave/requests/{}/coverage/confirm",
                sent.request.id
            ))
            .header("content-type", "application/json")
            .body(Body::from(mixed.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mixed_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_leave_coverage_confirmations WHERE request_id=$1",
    )
    .bind(sent.request.id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "mixed recipient batch must roll back every row");
    let confirm = serde_json::json!({"requestId":Uuid::now_v7(),"expectedRevision":task.plan_revision,"assignmentIds":[own],"decision":"confirmed"});
    for _ in 0..2 {
        let result = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/staff-leave/requests/{}/coverage/confirm",
                    sent.request.id
                ))
                .header("content-type", "application/json")
                .body(Body::from(confirm.to_string()))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::OK);
        let result: serde_json::Value =
            serde_json::from_slice(&to_bytes(result.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        private_fields_absent(&result);
        let rows = result["data"]["assignments"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .filter(|r| r["assignment"]["consent"] == "confirmed")
                .count(),
            1
        );
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_leave_coverage_confirmations WHERE request_id=$1",
    )
    .bind(sent.request.id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let forged = serde_json::json!({"requestId":Uuid::now_v7(),"expectedRevision":task.plan_revision,"assignmentIds":[task.assignments[0].assignment.as_ref().unwrap().id],"decision":"confirmed","teacherId":f.owner.user_id});
    let invalid = router
        .oneshot(
            Request::post(format!(
                "/api/staff-leave/requests/{}/coverage/confirm",
                sent.request.id
            ))
            .header("content-type", "application/json")
            .body(Body::from(forged.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn coverage_actual_grade_filters_do_not_inherit_other_offering_target_grades() {
    use school_staff_leave::{coverage, coverage_models::*};
    let f = academic_coverage_fixture("coverage_actual_teaching_grades", true, 7).await;
    let original_grade:Uuid=sqlx::query_scalar("SELECT grade_level_id FROM student_academic_years WHERE student_id=$1 AND academic_year_id=$2").bind(f.pupils[0]).bind(f.year).fetch_one(&f.pool).await.unwrap();
    let other_grade:Uuid=sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type=(SELECT level_type FROM grade_levels WHERE id=$1) AND year=2").bind(original_grade).fetch_one(&f.pool).await.unwrap();
    assert_ne!(original_grade, other_grade);
    sqlx::query("INSERT INTO academic_year_grade_levels(academic_year_id,grade_level_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(f.year).bind(other_grade).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE student_academic_years SET grade_level_id=$1 WHERE student_id=$2 AND academic_year_id=$3").bind(other_grade).bind(f.pupils[2]).bind(f.year).execute(&f.pool).await.unwrap();
    for grade in [original_grade, other_grade] {
        sqlx::query("INSERT INTO learning_offering_targets(id,learning_offering_id,academic_term_id,academic_year_id,target_kind,homeroom_id,grade_level_id,study_program_id) SELECT uuidv7(),$1,$2,$3,'grade_program',NULL,$4,study_program_id FROM student_academic_years WHERE academic_year_id=$3 LIMIT 1").bind(f.english).bind(f.term).bind(f.year).bind(grade).execute(&f.pool).await.unwrap();
    }
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
    let basis = LeaveCoverageCandidatesRequest {
        expected_revision: planned.request.revision,
        source_digest: planned.coverage.source_digest.clone().unwrap(),
        lesson_keys: vec![planned.coverage.days[0].lessons[0]
            .lesson
            .lesson_key
            .clone()],
        search: None,
        subject_group_id: None,
        grade_level_id: Some(original_grade),
        same_subject_group_only: false,
        same_student_group_only: false,
        same_grade_only: false,
    };
    let original = coverage::candidates(&f.pool, &f.owner, planned.request.id, basis.clone())
        .await
        .unwrap();
    assert!(!original
        .candidates
        .iter()
        .any(|c| c.teacher_id == f.teacher.user_id));
    let actual = coverage::candidates(
        &f.pool,
        &f.owner,
        planned.request.id,
        LeaveCoverageCandidatesRequest {
            grade_level_id: Some(other_grade),
            ..basis
        },
    )
    .await
    .unwrap();
    assert!(actual
        .candidates
        .iter()
        .any(|c| c.teacher_id == f.teacher.user_id));
}
async fn returned_reciprocal_fixture(
    name: &str,
) -> (
    AcademicCoverageFixture,
    LeaveRequestDetail,
    Vec<school_staff_leave::coverage_models::LeaveCoverageSelection>,
    Uuid,
) {
    use school_staff_leave::{coverage, coverage_models::*};
    let f = academic_coverage_fixture(name, false, 7).await;
    let period_id = Uuid::now_v7();
    let extra_block = Uuid::now_v7();
    let extra_allocation = Uuid::now_v7();
    let mut tx = f.pool.begin().await.unwrap();
    sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) SELECT $1,bell_schedule_id,'Extra consent','12:00','13:00',4,NULL FROM academic_terms WHERE id=$2").bind(period_id).bind(f.term).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) SELECT $1,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,$2,day_of_week,'STRUCTURAL','OTHER','Extra duty' FROM academic_timetable_blocks WHERE id=(SELECT block_id FROM academic_timetable_block_groups WHERE id=$3)").bind(extra_block).bind(period_id).bind(f.allocation).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_term_id,academic_year_id) VALUES($1,$2,$3,$4,$5)").bind(extra_allocation).bind(extra_block).bind(f.owner.user_id).bind(f.term).bind(f.year).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let partial = academic_coverage_plan(&f, LeaveCoverageMode::Reciprocal).await;
    let original = &partial.coverage.days[0].lessons[0];
    let a = original.assignment.as_ref().unwrap();
    let extra = partial.coverage.days[0]
        .lessons
        .iter()
        .find(|l| l.lesson.allocation_id == extra_allocation)
        .unwrap();
    let selections = vec![
        LeaveCoverageSelection {
            lesson_key: original.lesson.lesson_key.clone(),
            replacement_teacher_id: a.replacement_teacher_id,
            mode: a.mode,
            taught_offering_id: a.taught_offering_id,
            taught_activity: None,
            return_lesson_key: a.return_lesson_key.clone(),
        },
        LeaveCoverageSelection {
            lesson_key: extra.lesson.lesson_key.clone(),
            replacement_teacher_id: f.co_teacher.user_id,
            mode: LeaveCoverageMode::Substitution,
            taught_offering_id: None,
            taught_activity: Some("Extra activity".into()),
            return_lesson_key: None,
        },
    ];
    let complete = coverage::save(
        &f.pool,
        &f.owner,
        partial.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: partial.request.revision,
            source_digest: partial.coverage.source_digest.clone().unwrap(),
            assignments: selections.clone(),
        },
    )
    .await
    .unwrap();
    let sent = submit(&f.pool, &f.owner, &complete).await;
    let task = coverage::assigned(&f.pool, &f.teacher, Default::default())
        .await
        .unwrap()
        .items
        .remove(0);
    let stable_id = task.assignments[0].assignment.as_ref().unwrap().id;
    coverage::confirm(
        &f.pool,
        &f.teacher,
        sent.request.id,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: task.plan_revision,
            assignment_ids: vec![stable_id],
            decision: LeaveCoverageConsent::Confirmed,
        },
    )
    .await
    .unwrap();
    let other = coverage::assigned(&f.pool, &f.co_teacher, Default::default())
        .await
        .unwrap()
        .items
        .remove(0);
    coverage::confirm(
        &f.pool,
        &f.co_teacher,
        sent.request.id,
        ConfirmLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: other.plan_revision,
            assignment_ids: vec![other.assignments[0].assignment.as_ref().unwrap().id],
            decision: LeaveCoverageConsent::Declined,
        },
    )
    .await
    .unwrap();
    let returned = coverage::replan(
        &f.pool,
        &f.owner,
        sent.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: sent.request.revision,
        },
    )
    .await
    .unwrap();
    (f, returned, selections, stable_id)
}

#[tokio::test]
async fn coverage_reciprocal_replan_retains_consent_and_creates_a_distinct_reservation_pair() {
    use school_staff_leave::{coverage, coverage_models::*};
    let (f, returned, mut selections, stable_id) =
        returned_reciprocal_fixture("coverage_pair_replan_consent_cycle").await;
    let sent = returned.clone();
    let receipts_before:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(c) ORDER BY id)::text FROM staff_leave_coverage_confirmations c WHERE request_id=$1").bind(returned.request.id).fetch_one(&f.pool).await.unwrap();
    let returned = coverage::refresh(
        &f.pool,
        &f.owner,
        returned.request.id,
        LeaveMutationRequest {
            request_id: Uuid::now_v7(),
            expected_revision: returned.request.revision,
        },
    )
    .await
    .unwrap();
    let a = returned.coverage.days[0].lessons[0]
        .assignment
        .as_ref()
        .unwrap();
    assert_eq!(a.id, stable_id);
    assert_eq!(a.consent, LeaveCoverageConsent::Confirmed);
    selections[1].replacement_teacher_id = f.admin.user_id;
    let saved = coverage::save(
        &f.pool,
        &f.owner,
        sent.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: returned.request.revision,
            source_digest: returned.coverage.source_digest.clone().unwrap(),
            assignments: selections,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        saved.coverage.days[0].lessons[0]
            .assignment
            .as_ref()
            .unwrap()
            .id,
        stable_id
    );
    assert_eq!(
        saved.coverage.days[0].lessons[0]
            .assignment
            .as_ref()
            .unwrap()
            .consent,
        LeaveCoverageConsent::Confirmed
    );
    let input = LeaveMutationRequest {
        request_id: Uuid::now_v7(),
        expected_revision: saved.request.revision,
    };
    service::submit(&f.pool, &f.owner, saved.request.id, input.clone())
        .await
        .unwrap();
    service::submit(&f.pool, &f.owner, saved.request.id, input)
        .await
        .unwrap();
    let(count,cycles):(i64,i64)=sqlx::query_as("SELECT count(*),count(DISTINCT pair_id) FROM academic_teaching_coverage_duties WHERE request_id=$1 AND kind='reciprocal'").bind(sent.request.id).fetch_one(&f.pool).await.unwrap();
    assert_eq!((count, cycles), (4, 2));
    let receipts:i64=sqlx::query_scalar("SELECT count(*) FROM staff_leave_coverage_confirmations WHERE request_id=$1 AND assignment_id=$2").bind(sent.request.id).bind(stable_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(receipts, 1);
    let receipts_after:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(c) ORDER BY id)::text FROM staff_leave_coverage_confirmations c WHERE request_id=$1").bind(sent.request.id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(receipts_before, receipts_after);
}

#[tokio::test]
async fn coverage_candidates_batch_all_slots_compare_real_intervals_audiences_and_bilingual_names()
{
    use school_staff_leave::{coverage, coverage_models::*};
    let f = academic_coverage_fixture("coverage_candidate_all_slots", true, 7).await;
    let mut tx = f.pool.begin().await.unwrap();
    let base:(Uuid,Uuid,String)=sqlx::query_as("SELECT b.timetable_id,b.bell_schedule_id,b.day_of_week FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$1").bind(f.allocation).fetch_one(&mut *tx).await.unwrap();
    for (n, teacher, start, end) in [
        (4, f.owner.user_id, "10:00", "11:00"),
        (5, f.teacher.user_id, "10:30", "11:30"),
    ] {
        let period = Uuid::now_v7();
        let block = Uuid::now_v7();
        sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,'Different overlapping bell ID',$3::text::time,$4::text::time,$5,NULL)").bind(period).bind(base.1).bind(start).bind(end).bind(n).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_blocks(id,timetable_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) VALUES($1,$2,$3,$4,$5,$6,$7,'STRUCTURAL','OTHER','Availability proof')").bind(block).bind(base.0).bind(f.term).bind(f.year).bind(base.1).bind(period).bind(&base.2).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_term_id,academic_year_id) VALUES(uuidv7(),$1,$2,$3,$4)").bind(block).bind(teacher).bind(f.term).bind(f.year).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    sqlx::query("UPDATE users SET title='นาย',first_name='สมชาย',last_name='ทดลอง',title_en='Mr.',first_name_en='DistinctEnglish',last_name_en='Lookup' WHERE id=$1").bind(f.teacher.user_id).execute(&f.pool).await.unwrap();
    let (t, _) = typ(&f.pool, &f.admin, "batch_candidates", LeaveUnit::Day, None).await;
    let mut p = period(t.id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;

    let draft = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    let first = &draft.coverage.days[0].lessons[0].lesson;
    let basis = LeaveCoverageCandidatesRequest {
        expected_revision: draft.request.revision,
        source_digest: draft.coverage.source_digest.clone().unwrap(),
        lesson_keys: vec![first.lesson_key.clone()],
        search: Some("DistinctEnglish".into()),
        subject_group_id: None,
        grade_level_id: None,
        same_subject_group_only: false,
        same_student_group_only: false,
        same_grade_only: false,
    };
    let free = coverage::candidates(&f.pool, &f.owner, draft.request.id, basis.clone())
        .await
        .unwrap();
    assert!(free
        .candidates
        .iter()
        .any(|c| c.teacher_id == f.teacher.user_id && !c.display_name.contains("DistinctEnglish")));
    let all = coverage::candidates(
        &f.pool,
        &f.owner,
        draft.request.id,
        LeaveCoverageCandidatesRequest {
            lesson_keys: draft.coverage.days[0]
                .lessons
                .iter()
                .map(|l| l.lesson.lesson_key.clone())
                .collect(),
            ..basis.clone()
        },
    )
    .await
    .unwrap();
    assert!(!all
        .candidates
        .iter()
        .any(|c| c.teacher_id == f.teacher.user_id));
    let audience = coverage::candidates(
        &f.pool,
        &f.owner,
        draft.request.id,
        LeaveCoverageCandidatesRequest {
            search: None,
            same_student_group_only: true,
            ..basis.clone()
        },
    )
    .await
    .unwrap();
    assert!(audience
        .candidates
        .iter()
        .any(|c| c.teacher_id == f.co_teacher.user_id));
    assert!(!audience
        .candidates
        .iter()
        .any(|c| c.teacher_id == f.teacher.user_id));
    let grade = coverage::candidates(
        &f.pool,
        &f.owner,
        draft.request.id,
        LeaveCoverageCandidatesRequest {
            search: None,
            same_grade_only: true,
            ..basis
        },
    )
    .await
    .unwrap();
    assert!(
        grade
            .candidates
            .iter()
            .any(|c| c.teacher_id == f.teacher.user_id),
        "source grades {:?}; free candidate grades {:?}",
        first.grade_level_ids,
        free.candidates
            .iter()
            .find(|c| c.teacher_id == f.teacher.user_id)
            .map(|c| &c.grade_levels)
    );
}

#[tokio::test]
async fn coverage_source_and_taught_scope_deletion_return_typed_conflicts_and_keep_past_snapshots()
{
    use school_academic_delivery::models::current::DeliveryDeletionScope;
    use school_academic_timetable::services::{deletion, timetable_block_service};
    use school_errors::AppError;
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    for (name, offset, future) in [
        ("coverage_future_deletion_guard", 7, true),
        ("coverage_past_deletion_evidence", -14, false),
    ] {
        let f = academic_coverage_fixture(name, true, offset).await;
        let planned = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
        let approved = confirm_and_approve_academic(&f, &planned).await;
        let scope = DeliveryDeletionScope {
            delete_offering: false,
            learning_offering_id: f.english,
            academic_year_id: f.year,
            academic_term_id: f.term,
            learning_group_ids: vec![f.english_group],
        };
        let mut connection = f.pool.acquire().await.unwrap();
        let preview = deletion::deletion_impact_on_connection(&mut connection, &scope).await;
        drop(connection);
        if future {
            assert!(matches!(preview, Err(AppError::Conflict(_))));
            let mut tx = f.pool.begin().await.unwrap();
            assert!(
                matches!(deletion::delete_in_transaction(&mut tx,&scope,None).await,Err(AppError::Conflict(message)) if message.contains("หน้าที่สอน"))
            );
            tx.rollback().await.unwrap();
            let (group_revision, name): (i64, String) =
                sqlx::query_as("SELECT row_version,name FROM learning_groups WHERE id=$1")
                    .bind(f.english_group)
                    .fetch_one(&f.pool)
                    .await
                    .unwrap();
            let edit = school_academic_delivery::services::groups::update(
                &f.pool,
                f.admin.user_id,
                f.english_group,
                school_academic_delivery::models::UpdateLearningGroupRequest {
                    row_version: group_revision,
                    name: format!("{name} edit"),
                    description: None,
                    capacity: None,
                    registration_open: false,
                    preferred_room_ids: vec![],
                },
            )
            .await;
            assert!(
                matches!(edit,Err(AppError::Conflict(message)) if message.contains("หน้าที่สอน")),
                "Delivery must map its own coverage guard to an actionable conflict"
            );
            let (membership,membership_revision):(Uuid,i64)=sqlx::query_as("SELECT id,row_version FROM learning_group_students WHERE learning_group_id=$1 AND student_id=$2").bind(f.english_group).bind(f.pupils[0]).fetch_one(&f.pool).await.unwrap();
            let remove = school_academic_delivery::services::roster_memberships::remove_membership(
                &f.pool,
                f.admin.user_id,
                f.english_group,
                membership,
                school_academic_delivery::models::RemoveDatedRosterMembershipRequest {
                    group_row_version: group_revision,
                    membership_row_version: membership_revision,
                    left_at: f.from - chrono::Duration::days(1),
                },
            )
            .await;
            assert!(
                matches!(remove,Err(AppError::Conflict(message)) if message.contains("หน้าที่สอน"))
            );
            let after: (i64, String) =
                sqlx::query_as("SELECT row_version,name FROM learning_groups WHERE id=$1")
                    .bind(f.english_group)
                    .fetch_one(&f.pool)
                    .await
                    .unwrap();
            assert_eq!(after, (group_revision, name));
        } else {
            preview.unwrap();
        }
        let(block,table,revision):(Uuid,Uuid,i64)=sqlx::query_as("SELECT b.id,b.timetable_id,b.row_version FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$1").bind(f.allocation).fetch_one(&f.pool).await.unwrap();
        let snapshot:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(d) ORDER BY id)::text FROM academic_teaching_coverage_duties d WHERE request_id=$1").bind(approved.request.id).fetch_one(&f.pool).await.unwrap();
        let mutation =
            school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
                &f.pool,
                f.admin.user_id,
            );
        let removed =
            timetable_block_service::deactivate_block(&f.pool, &mutation, block, table, revision)
                .await;
        if future {
            assert!(matches!(removed, Err(AppError::Conflict(_))));
        } else {
            assert!(!removed.unwrap().is_active);
        }
        let retained:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(d) ORDER BY id)::text FROM academic_teaching_coverage_duties d WHERE request_id=$1").bind(approved.request.id).fetch_one(&f.pool).await.unwrap();
        assert_eq!(snapshot, retained);
    }
}
#[tokio::test]
async fn coverage_same_day_started_exchange_keeps_owed_return_and_unrelated_edit_remains_possible()
{
    use school_staff_leave::coverage_models::LeaveCoverageMode;
    let f = academic_coverage_fixture("coverage_today_begun_pair", false, 0).await;
    sqlx::query("UPDATE bell_schedule_periods SET start_time='00:00',end_time='00:01' WHERE id=(SELECT b.bell_schedule_period_id FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$1)").bind(f.allocation).execute(&f.pool).await.unwrap();
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Reciprocal).await;
    let approved = confirm_and_approve_academic(&f, &planned).await;
    // An unrelated current slot remains editable in the same selected term.
    sqlx::query("UPDATE academic_timetable_blocks SET note='Unrelated saved note' WHERE id=(SELECT block_id FROM academic_timetable_block_groups WHERE learning_group_id=$1)").bind(f.other_group).execute(&f.pool).await.unwrap();
    let cancellation = service::cancel(
        &f.pool,
        &f.owner,
        approved.request.id,
        CancelLeaveRequest {
            request_id: Uuid::now_v7(),
            expected_revision: approved.request.revision,
            reason: "Return after first exchange lesson".into(),
        },
    )
    .await
    .unwrap();
    service::act(
        &f.pool,
        &f.admin,
        cancellation.request.id,
        action(&cancellation, ProcessAction::Approve),
    )
    .await
    .unwrap();
    let states:Vec<String>=sqlx::query_scalar("SELECT state FROM academic_teaching_coverage_duties WHERE request_id=$1 AND kind='reciprocal' ORDER BY pair_leg").bind(approved.request.id).fetch_all(&f.pool).await.unwrap();
    assert_eq!(states, vec!["approved".to_string(), "approved".to_string()]);
}

#[tokio::test]
async fn coverage_http_refresh_recovers_external_source_change_without_rewriting_old_evidence() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use school_staff_leave::{coverage, coverage_models::*};
    use tower::ServiceExt;
    let f = academic_coverage_fixture("coverage_external_source_refresh", true, 7).await;
    let planned = academic_coverage_plan(&f, LeaveCoverageMode::Substitution).await;
    let old_plan:String=sqlx::query_scalar("SELECT document::text FROM staff_leave_coverage_plan_revisions WHERE request_id=$1 AND revision=$2").bind(planned.request.id).bind(planned.coverage.revision).fetch_one(&f.pool).await.unwrap();
    let (block,bell):(Uuid,Uuid)=sqlx::query_as("SELECT b.id,b.bell_schedule_id FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$1").bind(f.allocation).fetch_one(&f.pool).await.unwrap();
    let slot = Uuid::now_v7();
    sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,'Changed source',TIME '13:00',TIME '14:00',13,NULL)").bind(slot).bind(bell).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_blocks SET bell_schedule_period_id=$1,row_version=row_version+1 WHERE id=$2").bind(slot).bind(block).execute(&f.pool).await.unwrap();
    let old = service::detail(&f.pool, &f.owner, planned.request.id)
        .await
        .unwrap();
    assert_eq!(
        old.coverage.source_digest, planned.coverage.source_digest,
        "GET remains a read of the accepted draft plan"
    );
    let query = LeaveCoverageCandidatesRequest {
        expected_revision: old.request.revision,
        source_digest: old.coverage.source_digest.clone().unwrap(),
        lesson_keys: vec![old.coverage.days[0].lessons[0].lesson.lesson_key.clone()],
        search: None,
        subject_group_id: None,
        grade_level_id: None,
        same_subject_group_only: false,
        same_student_group_only: false,
        same_grade_only: false,
    };
    assert!(
        coverage::candidates(&f.pool, &f.owner, old.request.id, query.clone())
            .await
            .is_err(),
        "stale guards stay strict before explicit reconciliation"
    );
    let router = coverage_http_router(&f.pool, &f.owner, "coverage-source-refresh");
    let payload =
        serde_json::json!({"requestId":Uuid::now_v7(),"expectedRevision":old.request.revision});
    let response = router
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/staff-leave/requests/{}/coverage/refresh",
                old.request.id
            ))
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "explicit coverage refresh must recover a stale stored plan"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    let refreshed: LeaveRequestDetail = serde_json::from_value(body["data"].clone()).unwrap();
    assert_eq!(refreshed.request.revision, old.request.revision + 1);
    assert_eq!(refreshed.coverage.revision, old.coverage.revision + 1);
    assert_ne!(refreshed.coverage.source_digest, old.coverage.source_digest);
    assert_eq!(
        refreshed.coverage.days[0].lessons[0]
            .lesson
            .bell_schedule_period_id,
        slot
    );
    assert!(refreshed.coverage.days[0].lessons[0].assignment.is_none());
    let retained:String=sqlx::query_scalar("SELECT document::text FROM staff_leave_coverage_plan_revisions WHERE request_id=$1 AND revision=$2").bind(planned.request.id).bind(planned.coverage.revision).fetch_one(&f.pool).await.unwrap();
    assert_eq!(retained, old_plan);
    let replay = router
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/staff-leave/requests/{}/coverage/refresh",
                old.request.id
            ))
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let replay: serde_json::Value =
        serde_json::from_slice(&to_bytes(replay.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(
        replay["data"]["request"]["revision"],
        refreshed.request.revision
    );
    coverage::candidates(
        &f.pool,
        &f.owner,
        old.request.id,
        LeaveCoverageCandidatesRequest {
            expected_revision: refreshed.request.revision,
            source_digest: refreshed.coverage.source_digest.clone().unwrap(),
            lesson_keys: vec![refreshed.coverage.days[0].lessons[0]
                .lesson
                .lesson_key
                .clone()],
            ..query
        },
    )
    .await
    .unwrap();
    let saved = coverage::save(
        &f.pool,
        &f.owner,
        old.request.id,
        SaveLeaveCoverageRequest {
            request_id: Uuid::now_v7(),
            expected_revision: refreshed.request.revision,
            source_digest: refreshed.coverage.source_digest.clone().unwrap(),
            assignments: vec![LeaveCoverageSelection {
                lesson_key: refreshed.coverage.days[0].lessons[0]
                    .lesson
                    .lesson_key
                    .clone(),
                replacement_teacher_id: f.teacher.user_id,
                mode: LeaveCoverageMode::Substitution,
                taught_offering_id: Some(f.english),
                taught_activity: None,
                return_lesson_key: None,
            }],
        },
    )
    .await
    .unwrap();
    let sent = submit(&f.pool, &f.owner, &saved).await;
    assert_eq!(sent.request.status, LeaveStatus::Submitted);
    let denied=router.oneshot(Request::post(format!("/api/staff-leave/requests/{}/coverage/refresh",sent.request.id)).header("content-type","application/json").body(Body::from(serde_json::json!({"requestId":Uuid::now_v7(),"expectedRevision":sent.request.revision}).to_string())).unwrap()).await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        service::detail(&f.pool, &f.owner, sent.request.id)
            .await
            .unwrap()
            .content_hash,
        sent.content_hash
    );
}

#[tokio::test]
async fn coverage_refresh_rechecks_return_leg_and_pupils_even_when_original_digest_is_unchanged() {
    use school_staff_leave::coverage;
    for (name, change) in [
        ("coverage_refresh_return_clock", 0),
        ("coverage_refresh_return_pupils", 1),
        ("coverage_update_return_joint", 2),
    ] {
        let (f, returned, _, stable) = returned_reciprocal_fixture(name).await;
        let digest = returned.coverage.source_digest.clone();
        let original:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(c) ORDER BY id)::text FROM staff_leave_coverage_confirmations c WHERE request_id=$1").bind(returned.request.id).fetch_one(&f.pool).await.unwrap();
        let unaffected = returned.coverage.days[0].lessons[1]
            .assignment
            .as_ref()
            .unwrap()
            .id;
        match change {
            0 => {
                let (block,bell):(Uuid,Uuid)=sqlx::query_as("SELECT b.id,b.bell_schedule_id FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id WHERE g.id=$1").bind(f.return_allocation).fetch_one(&f.pool).await.unwrap();
                let slot = Uuid::now_v7();
                sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days) VALUES($1,$2,'Changed return',TIME '10:30',TIME '11:30',20,NULL)").bind(slot).bind(bell).execute(&f.pool).await.unwrap();
                sqlx::query("UPDATE academic_timetable_blocks SET bell_schedule_period_id=$1,row_version=row_version+1 WHERE id=$2").bind(slot).bind(block).execute(&f.pool).await.unwrap();
            }
            1 => {
                sqlx::query("UPDATE learning_group_students SET membership_status='removed' WHERE learning_group_id=$1 AND student_id=$2").bind(f.english_group).bind(f.pupils[1]).execute(&f.pool).await.unwrap();
            }
            _ => {
                sqlx::query("INSERT INTO academic_timetable_block_group_instructors(id,block_group_id,instructor_id,role,display_order) VALUES(uuidv7(),$1,$2,'secondary',2)").bind(f.return_allocation).bind(f.co_teacher.user_id).execute(&f.pool).await.unwrap();
            }
        }
        let refreshed = if change == 2 {
            let mut p = period(returned.request.type_id, 17);
            p.academic_year_id = Some(f.year);
            p.academic_term_id = Some(f.term);
            p.start_date = f.from;
            p.end_date = f.from;

            p.reason = "Clarified request details".into();
            service::update(
                &f.pool,
                &f.owner,
                returned.request.id,
                UpdateLeaveRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: returned.request.revision,
                    period: p,
                },
            )
            .await
            .unwrap()
        } else {
            coverage::refresh(
                &f.pool,
                &f.owner,
                returned.request.id,
                LeaveMutationRequest {
                    request_id: Uuid::now_v7(),
                    expected_revision: returned.request.revision,
                },
            )
            .await
            .unwrap()
        };
        assert_eq!(refreshed.coverage.source_digest, digest);
        let first = refreshed.coverage.days[0]
            .lessons
            .iter()
            .find(|l| l.lesson.allocation_id == f.allocation)
            .unwrap();
        assert!(
            first.assignment.is_none(),
            "changed return/pupil proof must invalidate assignment {stable}"
        );
        assert_eq!(
            refreshed.coverage.days[0].lessons[1]
                .assignment
                .as_ref()
                .unwrap()
                .id,
            unaffected
        );
        let preserved:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(c) ORDER BY id)::text FROM staff_leave_coverage_confirmations c WHERE request_id=$1").bind(returned.request.id).fetch_one(&f.pool).await.unwrap();
        assert_eq!(original, preserved);
    }
}

#[tokio::test]
async fn full_day_http_retires_partial_payloads_and_settings_even_for_existing_receipt_keys() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let f = academic_coverage_fixture("full_day_wire_guards", true, 7).await;
    let planned = academic_coverage_plan(
        &f,
        school_staff_leave::coverage_models::LeaveCoverageMode::Substitution,
    )
    .await;
    let owner = coverage_http_router(&f.pool, &f.owner, "full-day-owner");
    let manager = coverage_http_router(&f.pool, &f.admin, "full-day-manager");
    let key:Uuid=sqlx::query_scalar("SELECT request_id FROM staff_leave_receipts WHERE actor_id=$1 AND operation='request_create' AND resource_id=$2").bind(f.owner.user_id).bind(planned.request.id).fetch_one(&f.pool).await.unwrap();
    let mut p = period(planned.request.type_id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;
    let base = serde_json::json!({"requestId":key,"period":p});
    let before:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM staff_leave_requests),(SELECT count(*) FROM staff_leave_receipts)").fetch_one(&f.pool).await.unwrap();
    for (field, value) in [
        ("dayPortion", serde_json::json!("morning")),
        ("dayPortion", serde_json::json!("afternoon")),
        ("dayPortion", serde_json::json!("full_day")),
        ("halfDayBoundary", serde_json::json!("12:00:00")),
        ("halfDayBoundary", serde_json::Value::Null),
    ] {
        let mut forged = base.clone();
        forged["period"][field] = value;
        let response = owner
            .clone()
            .oneshot(
                Request::post("/api/staff-leave/requests")
                    .header("content-type", "application/json")
                    .body(Body::from(forged.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "retired field {field} must be rejected before receipt replay"
        );
    }
    let mut timed = base.clone();
    timed["requestId"] = serde_json::json!(Uuid::now_v7());
    timed["period"]["startTime"] = serde_json::json!("09:00:00");
    timed["period"]["endTime"] = serde_json::json!("10:00:00");
    let response = owner
        .clone()
        .oneshot(
            Request::post("/api/staff-leave/requests")
                .header("content-type", "application/json")
                .body(Body::from(timed.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let original_policy = service::policies(&f.pool, &f.admin, planned.request.type_id)
        .await
        .unwrap()
        .remove(0);
    let old_key:Uuid=sqlx::query_scalar("SELECT request_id FROM staff_leave_receipts WHERE actor_id=$1 AND operation='policy_create' AND resource_id=$2").bind(f.admin.user_id).bind(original_policy.id).fetch_one(&f.pool).await.unwrap();
    let mut forged = serde_json::to_value(policy(
        original_policy.type_revision - 1,
        f.admin.user_id,
        LeaveUnit::Day,
        None,
    ))
    .unwrap();
    forged["requestId"] = serde_json::json!(old_key);
    forged["unit"] = serde_json::json!("half_day");
    let response = manager
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/staff-leave/types/{}/policies",
                planned.request.type_id
            ))
            .header("content-type", "application/json")
            .body(Body::from(forged.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let mut wrong = policy(
        original_policy.type_revision,
        f.admin.user_id,
        LeaveUnit::Minute,
        None,
    );
    wrong.request_id = Uuid::now_v7();
    assert!(
        service::save_policy(&f.pool, &f.admin, planned.request.type_id, wrong)
            .await
            .is_err()
    );
    let after:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM staff_leave_requests),(SELECT count(*) FROM staff_leave_receipts)").fetch_one(&f.pool).await.unwrap();
    assert_eq!(before, after);
    for method in [axum::http::Method::GET, axum::http::Method::PUT] {
        let response = manager
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/api/staff-leave/coverage/settings")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = owner
        .clone()
        .oneshot(
            Request::get("/api/staff-leave/coverage/options")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let options: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert!(options["data"].get("halfDayBoundary").is_none());
    assert!(options["data"].get("revision").is_none());
    let (exit, _) = typ(&f.pool, &f.admin, "wire_exit", LeaveUnit::Minute, None).await;
    let mut p = period(exit.id, 17);
    p.academic_year_id = Some(f.year);
    p.academic_term_id = Some(f.term);
    p.start_date = f.from;
    p.end_date = f.from;
    for (start, end, other_day) in [
        (None, None, false),
        (Some("09:00:01"), Some("10:00:00"), false),
        (Some("09:15:00"), Some("10:00:00"), true),
    ] {
        let mut value = serde_json::to_value(&p).unwrap();
        value["startTime"] = serde_json::json!(start);
        value["endTime"] = serde_json::json!(end);
        if other_day {
            value["endDate"] = serde_json::json!(f.back);
        }
        let response = owner
            .clone()
            .oneshot(
                Request::post("/api/staff-leave/requests")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"requestId":Uuid::now_v7(),"period":value}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    p.start_time = chrono::NaiveTime::from_hms_opt(9, 15, 0);
    p.end_time = chrono::NaiveTime::from_hms_opt(10, 0, 0);
    let exact = service::create(
        &f.pool,
        &f.owner,
        CreateLeaveRequest {
            request_id: Uuid::now_v7(),
            period: p,
        },
    )
    .await
    .unwrap();
    assert_eq!(exact.request.amount, 45);
    assert_eq!(exact.request.unit, LeaveUnit::Minute);
    assert_eq!(exact.coverage.days[0].lessons.len(), 1);
}

mod codes {
    include!("staff_leave_coverage/permission_codes_generated.rs");
}
