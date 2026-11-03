use school_academic_core::{
    models::*,
    services::{bell_schedules, term_school_schedule, years_terms},
};
use sqlx::PgPool;
use uuid::Uuid;

fn weekdays(saturday: bool) -> Vec<String> {
    let mut days = ["MON", "TUE", "WED", "THU", "FRI"]
        .map(str::to_owned)
        .to_vec();
    if saturday {
        days.push("SAT".into());
    }
    days
}

async fn fixture(name: &str) -> (PgPool, Uuid, AcademicTerm, BellSchedule) {
    let pool = school_test_db::create_named_test_pool(name).await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type) VALUES($1,'term-school-days','!','Term','Schedule','staff')")
        .bind(actor).execute(&pool).await.unwrap();
    let year = years_terms::create_year(
        &pool,
        actor,
        CreateAcademicYearRequest {
            year: 2910,
            custom_name: None,
            default_school_days: weekdays(false),
        },
    )
    .await
    .unwrap();
    let schedule = bell_schedules::create(
        &pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id: year.id,
            name: "Source periods".into(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let saved = bell_schedules::replace_periods(
        &pool,
        actor,
        schedule.id,
        ReplaceBellSchedulePeriodsRequest {
            row_version: schedule.row_version,
            periods: vec![BellSchedulePeriodInput {
                name: Some("Period one".into()),
                start_time: "08:00:00".parse().unwrap(),
                end_time: "09:00:00".parse().unwrap(),
                order_index: 1,
                applicable_days: None,
                is_active: true,
            }],
        },
    )
    .await
    .unwrap();
    let schedule = bell_schedules::get(&pool, saved.snapshot.bell_schedule_id)
        .await
        .unwrap();
    let term = years_terms::create_term(
        &pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: year.id,
            term_type: AcademicTermType::Regular,
            custom_name: None,
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: schedule.id,
            school_days: weekdays(false),
        },
    )
    .await
    .unwrap();
    (pool, actor, term, schedule)
}

fn create_request(
    source: &AcademicTerm,
    schedule: &BellSchedule,
) -> CreateAcademicTermCopySchoolScheduleRequest {
    CreateAcademicTermCopySchoolScheduleRequest {
        academic_year_id: source.academic_year_id,
        term_type: AcademicTermType::Regular,
        custom_name: None,
        included_in_year_result: true,
        blocks_year_closure: true,
        school_days: weekdays(true),
        source_term_id: source.id,
        source_term_row_version: source.row_version,
        source_bell_schedule_row_version: schedule.row_version,
    }
}

#[tokio::test]
async fn term_school_copy_create_in_active_year_keeps_days_and_periods_independent() {
    let (pool, actor, source, schedule) = fixture("term_school_copy_active").await;
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(source.academic_year_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
        .bind(source.id)
        .execute(&pool)
        .await
        .unwrap();
    let original_periods = bell_schedules::list_periods(&pool, schedule.id)
        .await
        .unwrap();
    let copied = term_school_schedule::create(&pool, actor, create_request(&source, &schedule))
        .await
        .unwrap();
    assert_eq!(copied.term.school_days, weekdays(true));
    assert_eq!(
        years_terms::get_term(&pool, source.id)
            .await
            .unwrap()
            .school_days,
        weekdays(false)
    );
    assert_ne!(copied.term.bell_schedule_id, source.bell_schedule_id);
    assert!(copied.can_edit_school_days && copied.can_assign_bell_schedule);
    assert!(copied.bell_schedule.edit_blocked_reason.is_none());
    let snapshot = bell_schedules::list_periods(&pool, copied.bell_schedule.id)
        .await
        .unwrap();
    assert_ne!(snapshot.periods[0].id, original_periods.periods[0].id);
    assert_eq!(snapshot.periods[0].applicable_days, None);
    bell_schedules::replace_periods(
        &pool,
        actor,
        copied.bell_schedule.id,
        ReplaceBellSchedulePeriodsRequest {
            row_version: snapshot.row_version,
            periods: vec![BellSchedulePeriodInput {
                name: Some("Copied edit".into()),
                start_time: "08:30:00".parse().unwrap(),
                end_time: "09:30:00".parse().unwrap(),
                order_index: 1,
                applicable_days: Some(weekdays(true)),
                is_active: true,
            }],
        },
    )
    .await
    .unwrap();
    let after = bell_schedules::list_periods(&pool, schedule.id)
        .await
        .unwrap();
    assert_eq!(after.periods[0].id, original_periods.periods[0].id);
    assert_eq!(
        after.periods[0].start_time,
        original_periods.periods[0].start_time
    );
    assert!(years_terms::update_term(
        &pool,
        actor,
        source.id,
        UpdateAcademicTermRequest {
            term_type: source.term_type,
            custom_name: Some(source.name.clone()),
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: source.bell_schedule_id,
            school_days: weekdays(true),
            row_version: source.row_version
        }
    )
    .await
    .is_err());
}

#[tokio::test]
async fn term_school_copy_refuses_stale_sources_and_rolls_back_late_failure() {
    let (pool, actor, source, schedule) = fixture("term_school_copy_atomic").await;
    let counts = || {
        sqlx::query_as::<_,(i64,i64,i64)>("SELECT (SELECT count(*) FROM academic_terms),(SELECT count(*) FROM bell_schedules),(SELECT count(*) FROM bell_schedule_periods)")
    };
    let before = counts().fetch_one(&pool).await.unwrap();
    for request in [
        CreateAcademicTermCopySchoolScheduleRequest {
            source_term_row_version: source.row_version + 1,
            ..create_request(&source, &schedule)
        },
        CreateAcademicTermCopySchoolScheduleRequest {
            source_bell_schedule_row_version: schedule.row_version + 1,
            ..create_request(&source, &schedule)
        },
        CreateAcademicTermCopySchoolScheduleRequest {
            blocks_year_closure: false,
            ..create_request(&source, &schedule)
        },
        CreateAcademicTermCopySchoolScheduleRequest {
            school_days: vec![],
            ..create_request(&source, &schedule)
        },
    ] {
        assert!(term_school_schedule::create(&pool, actor, request)
            .await
            .is_err());
        assert_eq!(counts().fetch_one(&pool).await.unwrap(), before);
    }
    let copied = term_school_schedule::create(&pool, actor, create_request(&source, &schedule))
        .await
        .unwrap();
    let request = CopyAcademicTermSchoolScheduleRequest {
        source_term_id: source.id,
        source_term_row_version: source.row_version,
        source_bell_schedule_row_version: schedule.row_version,
        school_days: weekdays(false),
        row_version: copied.term.row_version,
    };
    let (left, right) = tokio::join!(
        term_school_schedule::copy_into(&pool, actor, copied.term.id, request.clone()),
        term_school_schedule::copy_into(&pool, actor, copied.term.id, request)
    );
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "one captured target revision accepts exactly one copy"
    );
    let after = counts().fetch_one(&pool).await.unwrap();
    assert_eq!(after, (before.0 + 1, before.1 + 2, before.2 + 2));
}

#[tokio::test]
async fn term_school_copy_and_standalone_clone_refuse_malformed_stored_period_days() {
    let (pool, actor, source, schedule) = fixture("term_school_copy_stored_days").await;
    let counts = || {
        sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT (SELECT count(*) FROM academic_terms),(SELECT count(*) FROM bell_schedules),(SELECT count(*) FROM bell_schedule_periods),(SELECT count(*) FROM academic_audit_events)",
    )
    };
    let clone_request = || CloneBellScheduleRequest {
        academic_year_id: source.academic_year_id,
        name: "Strict stored source clone".into(),
        owning_organization_unit_id: None,
        row_version: schedule.row_version,
    };
    for malformed in [" MON ", "mon", ""] {
        sqlx::query(
            "UPDATE bell_schedule_periods SET applicable_days=$2 WHERE bell_schedule_id=$1",
        )
        .bind(schedule.id)
        .bind(malformed)
        .execute(&pool)
        .await
        .unwrap();
        let before = counts().fetch_one(&pool).await.unwrap();
        let source_before = serde_json::to_value(
            bell_schedules::list_periods(&pool, schedule.id)
                .await
                .unwrap(),
        )
        .unwrap();
        for result in [
            bell_schedules::clone_schedule(&pool, actor, schedule.id, clone_request())
                .await
                .map(|_| ()),
            term_school_schedule::create(&pool, actor, create_request(&source, &schedule))
                .await
                .map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(school_errors::AppError::ValidationError(_))),
                "{malformed:?}: {result:?}"
            );
        }
        assert_eq!(counts().fetch_one(&pool).await.unwrap(), before);
        assert_eq!(
            serde_json::to_value(
                bell_schedules::list_periods(&pool, schedule.id)
                    .await
                    .unwrap()
            )
            .unwrap(),
            source_before
        );
    }
    for valid in [None, Some("MON,TUE,WED,THU,FRI")] {
        sqlx::query(
            "UPDATE bell_schedule_periods SET applicable_days=$2 WHERE bell_schedule_id=$1",
        )
        .bind(schedule.id)
        .bind(valid)
        .execute(&pool)
        .await
        .unwrap();
        let cloned = bell_schedules::clone_schedule(&pool, actor, schedule.id, clone_request())
            .await
            .unwrap();
        let periods = bell_schedules::list_periods(&pool, cloned.id)
            .await
            .unwrap();
        assert_eq!(periods.periods[0].applicable_days.as_deref(), valid);
        assert_ne!(periods.periods[0].bell_schedule_id, schedule.id);
    }
}

#[tokio::test]
async fn term_school_copy_refuses_source_schedule_outside_term_year_without_partial_writes() {
    let (pool, actor, source, _) = fixture("term_school_copy_source_year").await;
    let other_year = years_terms::create_year(
        &pool,
        actor,
        CreateAcademicYearRequest {
            year: 2911,
            custom_name: None,
            default_school_days: weekdays(false),
        },
    )
    .await
    .unwrap();
    let other_schedule = bell_schedules::create(
        &pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id: other_year.id,
            name: "Other year source".into(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let saved = bell_schedules::replace_periods(
        &pool,
        actor,
        other_schedule.id,
        ReplaceBellSchedulePeriodsRequest {
            row_version: other_schedule.row_version,
            periods: vec![BellSchedulePeriodInput {
                name: Some("Other year wildcard".into()),
                start_time: "08:00:00".parse().unwrap(),
                end_time: "09:00:00".parse().unwrap(),
                order_index: 1,
                applicable_days: None,
                is_active: true,
            }],
        },
    )
    .await
    .unwrap();
    let other_schedule = bell_schedules::get(&pool, saved.snapshot.bell_schedule_id)
        .await
        .unwrap();
    // The database's ID-only schedule FK permits this inconsistent legacy shape.
    sqlx::query(
        "UPDATE academic_terms SET bell_schedule_id=$2,row_version=row_version+1 WHERE id=$1",
    )
    .bind(source.id)
    .bind(other_schedule.id)
    .execute(&pool)
    .await
    .unwrap();
    let source = years_terms::get_term(&pool, source.id).await.unwrap();
    let facts = "SELECT jsonb_build_object('terms',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM academic_terms row),'schedules',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM bell_schedules row),'periods',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM bell_schedule_periods row),'audit',(SELECT jsonb_agg(to_jsonb(row) ORDER BY id) FROM academic_audit_events row))";
    let before: serde_json::Value = sqlx::query_scalar(facts).fetch_one(&pool).await.unwrap();
    let error =
        term_school_schedule::create(&pool, actor, create_request(&source, &other_schedule))
            .await
            .unwrap_err();
    assert!(
        matches!(error, school_errors::AppError::Conflict(ref message)
        if message.contains("ไม่อยู่ในปีการศึกษาที่ตรวจไว้")),
        "{error:?}"
    );
    let after: serde_json::Value = sqlx::query_scalar(facts).fetch_one(&pool).await.unwrap();
    assert_eq!(
        after, before,
        "refusal preserves exact source rows and audits without partial copies"
    );
}

#[tokio::test]
async fn term_school_copy_refuses_empty_inactive_and_oversized_period_sources() {
    let (pool, actor, source, schedule) = fixture("term_school_copy_unusable").await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM bell_schedules")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE bell_schedule_periods SET is_active=false WHERE bell_schedule_id=$1")
        .bind(schedule.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        term_school_schedule::create(&pool, actor, create_request(&source, &schedule))
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM bell_schedule_periods WHERE bell_schedule_id=$1")
        .bind(schedule.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        term_school_schedule::create(&pool, actor, create_request(&source, &schedule))
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,start_time,end_time,order_index,is_active) SELECT uuidv7(),$1,'08:00','09:00',step,false FROM generate_series(1,1001) step").bind(schedule.id).execute(&pool).await.unwrap();
    assert!(
        term_school_schedule::create(&pool, actor, create_request(&source, &schedule))
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM bell_schedules")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
}
