use chrono::{Duration, Utc};
use school_authorization::ActorContext;
use school_behavior::{
    access::BehaviorAccess,
    models::*,
    services::{catalog, cleanup, integration, manual, report, schedules},
};
use school_errors::AppError;
use school_permissions::registry::codes;
use school_test_db::{create_named_test_pool, run_test_migrations};
use sqlx::PgPool;
use uuid::Uuid;

struct Fixture {
    pool: PgPool,
    admin: BehaviorAccess,
    teacher: BehaviorAccess,
    other: BehaviorAccess,
    term: Uuid,
    grade: Uuid,
    room_a: Uuid,
    room_b: Uuid,
    a: Uuid,
    b: Uuid,
    unassigned: Uuid,
}
async fn user(pool: &PgPool, name: &str, kind: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users(username,email,password_hash,first_name,last_name,user_type,status)VALUES($1,$2,'not-a-real-login',$1,'Fixture',$3,'active')RETURNING id").bind(name).bind(format!("{name}@example.invalid")).bind(kind).fetch_one(pool).await.unwrap()
}
fn access(id: Uuid, permissions: &[&str]) -> BehaviorAccess {
    BehaviorAccess::for_actor(&ActorContext {
        user_id: id,
        permissions: permissions.iter().map(|s| s.to_string()).collect(),
    })
}
async fn fixture(name: &str) -> Fixture {
    let pool = create_named_test_pool(name).await;
    run_test_migrations(&pool).await;
    let admin = user(&pool, "admin-behavior", "staff").await;
    let teacher = user(&pool, "teacher-behavior", "staff").await;
    let other = user(&pool, "other-behavior", "staff").await;
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let start = today - Duration::days(40);
    let year:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,status)VALUES(2999,'Behavior Fixture','active')RETURNING id").fetch_one(&pool).await.unwrap();
    let bell:Uuid=sqlx::query_scalar("INSERT INTO bell_schedules(id,academic_year_id,code,name,is_default)VALUES(uuidv7(),$1,'BEHAVIOR','Behavior fixture',true)RETURNING id").bind(year).fetch_one(&pool).await.unwrap();
    let term:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,bell_schedule_id,status,included_in_year_result,blocks_year_closure)VALUES($1,1,'B1','Behavior Term','regular',$2,'active',true,true)RETURNING id").bind(year).bind(bell).fetch_one(&pool).await.unwrap();
    let grade:Uuid=sqlx::query_scalar("INSERT INTO grade_levels(level_type,year,is_active)VALUES('secondary',1,true)ON CONFLICT(level_type,year)DO UPDATE SET is_active=true RETURNING id").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_year_grade_levels(academic_year_id,grade_level_id)VALUES($1,$2)ON CONFLICT DO NOTHING").bind(year).bind(grade).execute(&pool).await.unwrap();
    let edition:Uuid=sqlx::query_scalar("INSERT INTO curriculum_editions(id,name,revision_year)VALUES(uuidv7(),'Behavior Curriculum',2999)RETURNING id").fetch_one(&pool).await.unwrap();
    let level:Uuid=sqlx::query_scalar("INSERT INTO curriculum_levels(id,edition_id,code,name_th,grade_level_ids,is_active)VALUES(uuidv7(),$1,'BEHAVIOR','Behavior level',$2,true)RETURNING id").bind(edition).bind(sqlx::types::Json(vec![grade])).fetch_one(&pool).await.unwrap();
    let program:Uuid=sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status)VALUES(uuidv7(),$1,'BEHAVIOR','Behavior program','draft')RETURNING id").bind(level).fetch_one(&pool).await.unwrap();
    let mut rooms = Vec::new();
    for n in 1..=2 {
        let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active)VALUES($1,$1,$2,$3,$4,$5,50,true)RETURNING id").bind(format!("Behavior Room {n}")).bind(year).bind(grade).bind(program).bind(n.to_string()).fetch_one(&pool).await.unwrap();
        rooms.push(room);
    }
    sqlx::query("INSERT INTO homeroom_advisors(id,homeroom_id,user_id,role)VALUES(uuidv7(),$1,$2,'primary')").bind(rooms[0]).bind(teacher).execute(&pool).await.unwrap();
    let mut students = Vec::new();
    for n in 0..3 {
        let id = user(&pool, &format!("behavior-student-{n}"), "student").await;
        sqlx::query("INSERT INTO student_info(user_id,student_id)VALUES($1,$2)")
            .bind(id)
            .bind(format!("B-{n}"))
            .execute(&pool)
            .await
            .unwrap();
        let sy:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status)VALUES(uuidv7(),$1,$2,$3,$4,'active')RETURNING id").bind(id).bind(year).bind(grade).bind(program).fetch_one(&pool).await.unwrap();
        if n < 2 {
            sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number)VALUES(uuidv7(),$1,$2,$3,$4,'current','regular',1)").bind(sy).bind(year).bind(rooms[n]).bind(start).execute(&pool).await.unwrap();
        }
        students.push(id);
    }
    Fixture {
        pool,
        admin: access(
            admin,
            &[
                codes::BEHAVIOR_READ_SCHOOL,
                codes::BEHAVIOR_CREATE_SCHOOL,
                codes::BEHAVIOR_DELETE_SCHOOL,
            ],
        ),
        teacher: access(
            teacher,
            &[
                codes::BEHAVIOR_READ_OWN,
                codes::BEHAVIOR_READ_ASSIGNED,
                codes::BEHAVIOR_CREATE_ASSIGNED,
                codes::BEHAVIOR_DELETE_OWN,
            ],
        ),
        other: access(
            other,
            &[
                codes::BEHAVIOR_READ_OWN,
                codes::BEHAVIOR_CREATE_SCHOOL,
                codes::BEHAVIOR_DELETE_OWN,
            ],
        ),
        term,
        grade,
        room_a: rooms[0],
        room_b: rooms[1],
        a: students[0],
        b: students[1],
        unassigned: students[2],
    }
}
fn today() -> chrono::NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
}

#[tokio::test]
async fn configured_grade_rules_do_not_require_enrolled_students() {
    let f = fixture("behavior_empty_grade_configuration").await;
    let grade: Uuid = sqlx::query_scalar("INSERT INTO grade_levels(level_type,year,is_active) VALUES('secondary',2,true) ON CONFLICT(level_type,year) DO UPDATE SET is_active=true RETURNING id")
        .fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO academic_year_grade_levels(academic_year_id,grade_level_id) SELECT academic_year_id,$2 FROM academic_terms WHERE id=$1")
        .bind(f.term).bind(grade).execute(&f.pool).await.unwrap();
    let settings = manual::selector_options(&f.pool, f.admin, query(&f), true)
        .await
        .unwrap();
    assert!(settings.grade_levels.iter().any(|g| g.id == grade));
    let picker = manual::selector_options(&f.pool, f.teacher, query(&f), false)
        .await
        .unwrap();
    assert_eq!(picker.grade_levels.len(), 1);
    assert_eq!(picker.grade_levels[0].id, f.grade);
    assert!(!picker.grade_levels.iter().any(|g| g.id == grade));
}
fn query(f: &Fixture) -> BehaviorStudentsQuery {
    BehaviorStudentsQuery {
        academic_term_id: f.term,
        date: today(),
        grade_level_id: None,
        homeroom_id: None,
        search: None,
        offset: None,
        limit: Some(10000),
    }
}
fn logs() -> BehaviorReportQuery {
    BehaviorReportQuery {
        start: today(),
        end: today(),
        academic_term_id: None,
        student_id: None,
        homeroom_id: None,
        grade_level_id: None,
        side: None,
        source: None,
        search: None,
        batch_id: None,
        created_by: None,
        mine: None,
        cancelled: None,
        offset: None,
        limit: Some(10000),
    }
}
async fn item(f: &Fixture, side: BehaviorSide, points: i32) -> BehaviorItem {
    catalog::save_item(
        &f.pool,
        f.admin.actor_id,
        SaveBehaviorItem {
            request_id: Uuid::new_v4(),
            id: None,
            row_version: None,
            name: format!("Behavior {}", side.code()),
            side,
            enabled: true,
            effective_from: today(),
            points,
        },
    )
    .await
    .unwrap()
}
async fn payload(
    f: &Fixture,
    access: BehaviorAccess,
    item: &BehaviorItem,
    students: Vec<Uuid>,
) -> CreateBehaviorBatch {
    let page = manual::students(&f.pool, access, query(f)).await.unwrap();
    CreateBehaviorBatch {
        request_id: Uuid::new_v4(),
        academic_term_id: f.term,
        item_id: item.id,
        item_row_version: item.row_version,
        item_version_id: item.version_id.unwrap(),
        occurred_at: Utc::now(),
        detail: Some("รายละเอียดที่ต้องลบจริง".into()),
        student_ids: students,
        roster_token: page.roster_token,
    }
}
async fn summary(f: &Fixture, student: Uuid) -> BehaviorStudentSummary {
    report::student_summary(
        &f.pool,
        f.admin,
        student,
        BehaviorSummaryQuery {
            start: None,
            end: None,
            academic_term_id: None,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn manual_group_scope_union_locked_points_and_creator_cancellation() {
    let f = fixture("behavior_group_union_cancel").await;
    let item = item(&f, BehaviorSide::Deduction, 20).await;
    let page = manual::students(&f.pool, f.admin, query(&f)).await.unwrap();
    let mut room_query = query(&f);
    room_query.homeroom_id = Some(f.room_a);
    let room_a = manual::students(&f.pool, f.admin, room_query)
        .await
        .unwrap();
    let mut room_query = query(&f);
    room_query.homeroom_id = Some(f.room_b);
    let room_b = manual::students(&f.pool, f.admin, room_query)
        .await
        .unwrap();
    assert_eq!(page.roster_token, room_a.roster_token);
    assert_eq!(room_a.roster_token, room_b.roster_token);
    let p = payload(&f, f.admin, &item, vec![f.a, f.b, f.a, f.unassigned]).await;
    let receipt = manual::create_batch(&f.pool, f.admin, p.clone())
        .await
        .unwrap();
    assert_eq!(receipt.affected_students, 3);
    assert!(
        manual::create_batch(&f.pool, f.admin, p.clone())
            .await
            .unwrap()
            .replayed
    );
    let teacher_wrong = payload(&f, f.teacher, &item, vec![f.a, f.b]).await;
    assert!(matches!(
        manual::create_batch(&f.pool, f.teacher, teacher_wrong).await,
        Err(AppError::Forbidden(_))
    ));
    let self_b = payload(&f, f.other, &item, vec![f.b]).await;
    manual::create_batch(&f.pool, f.other, self_b)
        .await
        .unwrap();
    let mut creator = f.teacher;
    creator.create_school = true;
    let own_b = payload(&f, creator, &item, vec![f.b]).await;
    manual::create_batch(&f.pool, creator, own_b).await.unwrap();
    let list = report::report(&f.pool, f.teacher, logs()).await.unwrap();
    assert_eq!(
        list.total, 2,
        "own-created B and assigned A are a union; other-created B is denied"
    );
    let all = report::report(&f.pool, f.admin, logs()).await.unwrap();
    let own = list.items.iter().find(|e| e.student_id == f.b).unwrap();
    let others = all
        .items
        .iter()
        .find(|e| e.student_id == f.b && e.created_by == Some(f.admin.actor_id))
        .unwrap();
    assert!(manual::cancel_entries(
        &f.pool,
        f.teacher,
        CancelBehaviorEntries {
            request_id: Uuid::new_v4(),
            entry_ids: vec![others.id],
            reason: "wrong selection".into()
        }
    )
    .await
    .is_err());
    let p = CancelBehaviorEntries {
        request_id: Uuid::new_v4(),
        entry_ids: vec![own.id],
        reason: "wrong selection".into(),
    };
    assert_eq!(
        manual::cancel_entries(&f.pool, f.teacher, p.clone())
            .await
            .unwrap()
            .cancelled,
        1
    );
    assert_eq!(
        manual::cancel_entries(&f.pool, f.teacher, p)
            .await
            .unwrap()
            .cancelled,
        1
    );
    assert_eq!(summary(&f, f.b).await.current.deduction, 40);
    assert!(
        report::student_summary(
            &f.pool,
            f.teacher,
            f.b,
            BehaviorSummaryQuery {
                start: None,
                end: None,
                academic_term_id: None
            }
        )
        .await
        .is_err(),
        "creator log rights do not reveal another room's full score"
    );
}
#[tokio::test]
async fn partial_side_archive_never_claims_matching_current_or_history_windows() {
    let f = fixture("behavior_partial_archive_windows").await;
    for (side, points) in [(BehaviorSide::Merit, 20), (BehaviorSide::Deduction, 5)] {
        let item = item(&f, side, points).await;
        let p = payload(&f, f.admin, &item, vec![f.a]).await;
        manual::create_batch(&f.pool, f.admin, p).await.unwrap();
    }
    assert_eq!(summary(&f, f.a).await.current.net, Some(15));
    let scope = BehaviorCleanupScope {
        start: Some(today()),
        end: Some(today()),
        academic_year_id: None,
        academic_term_id: Some(f.term),
        sides: vec![BehaviorSide::Merit],
        sources: vec![BehaviorSource::Manual],
    };
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::Archive,
        },
    )
    .await
    .unwrap();
    cleanup::execute(
        &f.pool,
        f.admin.actor_id,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope,
            mode: BehaviorCleanupMode::Archive,
            reason: "archive merit only".into(),
        },
    )
    .await
    .unwrap();
    let s = summary(&f, f.a).await;
    assert_eq!((s.current.merit, s.current.deduction), (0, 5));
    assert_eq!((s.history.merit, s.history.deduction), (20, 0));
    assert_eq!(s.current.net, None);
    assert_eq!(s.history.net, None);
    assert_eq!(s.comparable_current.net, None);
    assert!(!s.comparison_exact);
    assert_eq!(s.lifetime.net, 15);
}

#[tokio::test]
async fn reset_twenty_zero_then_five_same_day_and_actual_detail_deletion() {
    let f = fixture("behavior_reset_same_day").await;
    let deduction = item(&f, BehaviorSide::Deduction, 20).await;
    let p = payload(&f, f.admin, &deduction, vec![f.a]).await;
    manual::create_batch(&f.pool, f.admin, p.clone())
        .await
        .unwrap();
    let preview = schedules::preview_reset(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorReset {
            sides: vec![BehaviorSide::Deduction],
            through: today(),
        },
    )
    .await
    .unwrap();
    let reset = ResetBehaviorScores {
        request_id: Uuid::new_v4(),
        token: preview.token,
        revision: preview.revision,
        sides: vec![BehaviorSide::Deduction],
        through: today(),
        reason: "monthly policy".into(),
    };
    let ready = BehaviorResetReadiness {
        finalized_through: today(),
        delivered_through: today(),
    };
    schedules::reset(&f.pool, f.admin.actor_id, reset.clone(), ready.clone())
        .await
        .unwrap();
    schedules::reset(&f.pool, f.admin.actor_id, reset, ready)
        .await
        .unwrap();
    let s = summary(&f, f.a).await;
    assert_eq!(
        (
            s.current.deduction,
            s.history.deduction,
            s.lifetime.deduction
        ),
        (0, 20, 20)
    );
    assert_eq!(s.current.net, None);
    assert!(!s.comparison_exact);
    let details: i64 = sqlx::query_scalar("SELECT count(*) FROM behavior_batches")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(details, 0);
    let entries: i64 = sqlx::query_scalar("SELECT count(*) FROM behavior_entries")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(entries, 0);
    assert!(
        !manual::create_batch(&f.pool, f.admin, p)
            .await
            .unwrap()
            .details_available,
        "old receipt cannot resurrect purged details"
    );
    let five = item(&f, BehaviorSide::Deduction, 5).await;
    let p = payload(&f, f.admin, &five, vec![f.a]).await;
    manual::create_batch(&f.pool, f.admin, p).await.unwrap();
    let s = summary(&f, f.a).await;
    assert_eq!(
        (
            s.current.deduction,
            s.history.deduction,
            s.lifetime.deduction
        ),
        (5, 20, 25)
    );
}
#[tokio::test]
async fn purge_modes_stale_preview_and_history_adjustment() {
    let f = fixture("behavior_cleanup_adjust").await;
    let merit = item(&f, BehaviorSide::Merit, 7).await;
    let p = payload(&f, f.admin, &merit, vec![f.a]).await;
    manual::create_batch(&f.pool, f.admin, p).await.unwrap();
    let scope = BehaviorCleanupScope {
        start: Some(today()),
        end: Some(today()),
        academic_year_id: None,
        academic_term_id: Some(f.term),
        sides: vec![BehaviorSide::Merit],
        sources: vec![BehaviorSource::Manual],
    };
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DetailsOnly,
        },
    )
    .await
    .unwrap();
    let p = payload(&f, f.admin, &merit, vec![f.b]).await;
    manual::create_batch(&f.pool, f.admin, p).await.unwrap();
    assert!(cleanup::execute(
        &f.pool,
        f.admin.actor_id,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DetailsOnly,
            reason: "retention".into()
        }
    )
    .await
    .is_err());
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DetailsOnly,
        },
    )
    .await
    .unwrap();
    cleanup::execute(
        &f.pool,
        f.admin.actor_id,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DetailsOnly,
            reason: "retention".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(summary(&f, f.a).await.current.merit, 7);
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::Archive,
        },
    )
    .await
    .unwrap();
    cleanup::execute(
        &f.pool,
        f.admin.actor_id,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope: scope.clone(),
            mode: BehaviorCleanupMode::Archive,
            reason: "retention".into(),
        },
    )
    .await
    .unwrap();
    let s = summary(&f, f.a).await;
    assert_eq!((s.current.merit, s.history.merit), (0, 7));
    let t = &s.history_totals[0];
    cleanup::adjust_history(
        &f.pool,
        f.admin.actor_id,
        AdjustBehaviorHistory {
            request_id: Uuid::new_v4(),
            total_id: t.id,
            row_version: t.row_version,
            delta: -2,
            reason: "review historical total".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(summary(&f, f.a).await.history.merit, 5);
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DeleteEffects,
        },
    )
    .await
    .unwrap();
    cleanup::execute(
        &f.pool,
        f.admin.actor_id,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope,
            mode: BehaviorCleanupMode::DeleteEffects,
            reason: "delete all effects".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(summary(&f, f.a).await.lifetime.merit, 0);
}
#[tokio::test]
async fn attendance_replay_correction_cap_suppression_and_null_room() {
    let f = fixture("behavior_attendance_replay").await;
    let settings = catalog::settings(&f.pool).await.unwrap();
    catalog::save_settings(
        &f.pool,
        SaveBehaviorSettings {
            settings: BehaviorSettings {
                enabled: true,
                daily_deduction_cap: Some(5),
                ..settings
            },
        },
    )
    .await
    .unwrap();
    for status in ["absent", "skipped"] {
        catalog::save_rule(
            &f.pool,
            f.admin.actor_id,
            SaveBehaviorAttendanceRule {
                kind: BehaviorSource::Lesson,
                status: status.into(),
                grade_level_id: None,
                effective_from: today(),
                action: BehaviorScoreAction::Deduction,
                points: 4,
            },
        )
        .await
        .unwrap();
    }
    let generation = Uuid::new_v4();
    let fact = |key: &str| BehaviorAttendanceFact {
        source_key: key.into(),
        source_session_id: None,
        kind: BehaviorSource::Lesson,
        student_id: f.unassigned,
        status: "absent".into(),
        counted: true,
        grade_level_id: f.grade,
        homeroom_id: None,
        overrides: None,
    };
    let mut day = BehaviorAttendanceDay {
        generation,
        academic_term_id: f.term,
        date: today(),
        revision: 1,
        facts: vec![fact("b"), fact("a")],
    };
    let mut tx = f.pool.begin().await.unwrap();
    assert_eq!(
        integration::replace_attendance_day(&mut tx, day.clone())
            .await
            .unwrap(),
        BehaviorDeliveryOutcome::Applied
    );
    tx.commit().await.unwrap();
    assert_eq!(summary(&f, f.unassigned).await.current.deduction, 5);
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM behavior_entries WHERE cancelled_at IS NULL ORDER BY id",
    )
    .fetch_all(&f.pool)
    .await
    .unwrap();
    day.revision = 2;
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day.clone())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let after: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM behavior_entries WHERE cancelled_at IS NULL ORDER BY id",
    )
    .fetch_all(&f.pool)
    .await
    .unwrap();
    assert_eq!(ids, after, "unchanged facts preserve identity");
    day.revision = 3;
    day.facts.retain(|r| r.source_key == "b");
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day.clone())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(summary(&f, f.unassigned).await.current.deduction, 4);
    let scope = BehaviorCleanupScope {
        start: Some(today()),
        end: Some(today()),
        academic_year_id: None,
        academic_term_id: None,
        sides: vec![BehaviorSide::Deduction],
        sources: vec![BehaviorSource::Lesson],
    };
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DeleteEffects,
        },
    )
    .await
    .unwrap();
    cleanup::execute(
        &f.pool,
        f.admin.actor_id,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope,
            mode: BehaviorCleanupMode::DeleteEffects,
            reason: "clear wrong import".into(),
        },
    )
    .await
    .unwrap();
    day.revision = 4;
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(summary(&f, f.unassigned).await.current.deduction, 0);
}

#[tokio::test]
async fn queued_reset_with_long_reason_finishes_and_preserves_full_audit() {
    let f = fixture("behavior_reset_long_reason").await;
    let merit = item(&f, BehaviorSide::Merit, 7).await;
    let p = payload(&f, f.admin, &merit, vec![f.a]).await;
    manual::create_batch(&f.pool, f.admin, p).await.unwrap();
    sqlx::query("INSERT INTO behavior_batches(academic_term_id,item_version_id,occurred_at,date,detail,created_by)SELECT $1,$2,now(),$3,'รายละเอียดที่ต้องลบจริง',$4 FROM generate_series(1,20000)").bind(f.term).bind(merit.version_id.unwrap()).bind(today()).bind(f.admin.actor_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_entries(total_id,batch_id,item_name,points,created_by)SELECT t.id,b.id,'Behavior merit',7,$1 FROM behavior_batches b JOIN behavior_daily_totals t ON t.student_id=$2 AND t.side='merit' WHERE NOT EXISTS(SELECT 1 FROM behavior_entries e WHERE e.batch_id=b.id)").bind(f.admin.actor_id).bind(f.a).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE behavior_daily_totals t SET points=(SELECT sum(e.points) FROM behavior_entries e WHERE e.total_id=t.id),row_version=row_version+1 WHERE student_id=$1 AND side='merit'").bind(f.a).execute(&f.pool).await.unwrap();
    let before = summary(&f, f.a).await;
    assert_eq!(before.current.merit, 140007);
    let old_cycle: Uuid =
        sqlx::query_scalar("SELECT id FROM behavior_cycles WHERE side='merit' AND state='current'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let reason = "รีเซ็ตตามนโยบายโรงเรียนและเก็บเหตุผลสำหรับตรวจสอบย้อนหลัง ".repeat(10);
    assert!((201..=1000).contains(&reason.trim().chars().count()));
    let preview = schedules::preview_reset(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorReset {
            sides: vec![BehaviorSide::Merit],
            through: today(),
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.entries, 20001);
    let result = schedules::reset(
        &f.pool,
        f.admin.actor_id,
        ResetBehaviorScores {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            sides: vec![BehaviorSide::Merit],
            through: today(),
            reason: reason.clone(),
        },
        BehaviorResetReadiness {
            finalized_through: today(),
            delivered_through: today(),
        },
    )
    .await
    .unwrap();
    assert_eq!(result.status, "pending");
    assert!(cleanup::run_cleanup_jobs(&f.pool, 1).await.unwrap());
    let partial = cleanup::job(&f.pool, result.id).await.unwrap();
    assert_eq!(
        (partial.status.as_str(), partial.entries_deleted),
        ("pending", 5000)
    );
    assert_eq!(summary(&f, f.a).await.current.merit, before.current.merit);
    assert!(!cleanup::run_cleanup_jobs(&f.pool, 32).await.unwrap());
    let finished = cleanup::job(&f.pool, result.id).await.unwrap();
    assert_eq!(
        (finished.status.as_str(), finished.entries_deleted),
        ("completed", 20001)
    );
    let after = summary(&f, f.a).await;
    assert_eq!(
        (
            after.current.merit,
            after.history.merit,
            after.lifetime.merit
        ),
        (0, before.current.merit, before.lifetime.merit)
    );
    let (state, label, ends_on): (String, String, Option<chrono::NaiveDate>) =
        sqlx::query_as("SELECT state,label,ends_on FROM behavior_cycles WHERE id=$1")
            .bind(old_cycle)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(
        (state.as_str(), label.as_str(), ends_on),
        ("history", "ประวัติรอบคะแนน", Some(today()))
    );
    let (new_cycle, starts_on): (Uuid, chrono::NaiveDate) = sqlx::query_as(
        "SELECT id,starts_on FROM behavior_cycles WHERE side='merit' AND state='current'",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_ne!(new_cycle, old_cycle);
    assert_eq!(starts_on, today());
    let (job_reason, audit_reason, audit_mode): (String, String, String) = sqlx::query_as("SELECT j.reason,a.reason,a.mode FROM behavior_cleanup_jobs j JOIN behavior_cleanup_audits a ON a.id=j.id WHERE j.id=$1")
        .bind(result.id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(job_reason, reason.trim());
    assert_eq!(audit_reason, reason.trim());
    assert_eq!(audit_mode, "reset");
    let details: i64 = sqlx::query_scalar("SELECT count(*) FROM behavior_batches")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(details, 0);
    assert!(!cleanup::run_cleanup_jobs(&f.pool, 1).await.unwrap());
}

#[tokio::test]
async fn durable_cleanup_chunks_keep_totals_until_completed_and_resume_after_restart() {
    let f = fixture("behavior_durable_cleanup").await;
    let merit = item(&f, BehaviorSide::Merit, 7).await;
    let p = payload(&f, f.admin, &merit, vec![f.a]).await;
    manual::create_batch(&f.pool, f.admin, p).await.unwrap();
    sqlx::query("INSERT INTO behavior_batches(academic_term_id,item_version_id,occurred_at,date,detail,created_by)SELECT $1,$2,now(),$3,'รายละเอียดที่ต้องลบจริง',$4 FROM generate_series(1,20000)").bind(f.term).bind(merit.version_id.unwrap()).bind(today()).bind(f.admin.actor_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_entries(total_id,batch_id,item_name,points,created_by)SELECT t.id,b.id,'Behavior merit',7,$1 FROM behavior_batches b JOIN behavior_daily_totals t ON t.student_id=$2 AND t.side='merit' WHERE NOT EXISTS(SELECT 1 FROM behavior_entries e WHERE e.batch_id=b.id)").bind(f.admin.actor_id).bind(f.a).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE behavior_daily_totals SET points=140007,row_version=row_version+1 WHERE student_id=$1 AND side='merit'").bind(f.a).execute(&f.pool).await.unwrap();
    let scope = BehaviorCleanupScope {
        start: Some(today()),
        end: Some(today()),
        academic_year_id: None,
        academic_term_id: None,
        sides: vec![BehaviorSide::Merit],
        sources: vec![BehaviorSource::Manual],
    };
    let preview = cleanup::preview(
        &f.pool,
        f.admin.actor_id,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::Archive,
        },
    )
    .await
    .unwrap();
    let request = ExecuteBehaviorCleanup {
        request_id: Uuid::new_v4(),
        token: preview.token,
        revision: preview.revision,
        scope,
        mode: BehaviorCleanupMode::Archive,
        reason: "large monthly retention".into(),
    };
    let result = cleanup::execute(&f.pool, f.admin.actor_id, request.clone())
        .await
        .unwrap();
    assert_eq!(result.status, "pending");
    assert_eq!(summary(&f, f.a).await.current.merit, 140007);
    assert_eq!(
        cleanup::execute(&f.pool, f.admin.actor_id, request)
            .await
            .unwrap()
            .id,
        result.id,
        "queued creation retries return same job"
    );
    assert!(cleanup::run_cleanup_jobs(&f.pool, 1).await.unwrap());
    let progress = cleanup::job(&f.pool, result.id).await.unwrap();
    assert_eq!(progress.entries_deleted, 5000);
    assert_eq!(progress.status, "pending");
    assert_eq!(summary(&f, f.a).await.current.merit, 140007);
    let request = payload(&f, f.admin, &merit, vec![f.a]).await;
    assert!(
        manual::create_batch(&f.pool, f.admin, request)
            .await
            .is_err(),
        "job owns mutation scope until completion"
    );
    assert!(!cleanup::run_cleanup_jobs(&f.pool, 32).await.unwrap());
    let finished = cleanup::job(&f.pool, result.id).await.unwrap();
    assert_eq!(finished.status, "completed");
    assert_eq!(finished.entries_deleted, 20001);
    let s = summary(&f, f.a).await;
    assert_eq!(
        (s.current.merit, s.history.merit, s.lifetime.merit),
        (0, 140007, 140007)
    );
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM behavior_batches")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    assert!(
        !cleanup::run_cleanup_jobs(&f.pool, 1).await.unwrap(),
        "restart does not repeat archive"
    );
}

#[tokio::test]
async fn unchanged_no_score_snapshot_stays_zero_when_a_rule_is_added() {
    let f = fixture("behavior_zero_snapshot").await;
    let settings = catalog::settings(&f.pool).await.unwrap();
    catalog::save_settings(
        &f.pool,
        SaveBehaviorSettings {
            settings: BehaviorSettings {
                enabled: true,
                ..settings
            },
        },
    )
    .await
    .unwrap();
    let fact = BehaviorAttendanceFact {
        source_key: "lesson-zero".into(),
        source_session_id: None,
        kind: BehaviorSource::Lesson,
        student_id: f.a,
        status: "present".into(),
        counted: true,
        grade_level_id: f.grade,
        homeroom_id: Some(f.room_a),
        overrides: None,
    };
    let mut day = BehaviorAttendanceDay {
        generation: Uuid::new_v4(),
        academic_term_id: f.term,
        date: today(),
        revision: 1,
        facts: vec![fact],
    };
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day.clone())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(summary(&f, f.a).await.current.merit, 0);
    catalog::save_rule(
        &f.pool,
        f.admin.actor_id,
        SaveBehaviorAttendanceRule {
            kind: BehaviorSource::Lesson,
            status: "present".into(),
            grade_level_id: None,
            effective_from: today(),
            action: BehaviorScoreAction::Merit,
            points: 9,
        },
    )
    .await
    .unwrap();
    day.revision = 2;
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day.clone())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        summary(&f, f.a).await.current.merit,
        0,
        "adding criteria must not change an already captured no-score result"
    );
    catalog::save_rule(
        &f.pool,
        f.admin.actor_id,
        SaveBehaviorAttendanceRule {
            kind: BehaviorSource::Lesson,
            status: "late".into(),
            grade_level_id: None,
            effective_from: today(),
            action: BehaviorScoreAction::Deduction,
            points: 2,
        },
    )
    .await
    .unwrap();
    day.revision = 3;
    day.facts[0].status = "late".into();
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(summary(&f, f.a).await.current.deduction, 2);
}

#[tokio::test]
async fn empty_future_round_does_not_freeze_policy_before_its_first_fact() {
    let f = fixture("behavior_empty_future_policy").await;
    let future = today() + Duration::days(1);
    let settings = catalog::settings(&f.pool).await.unwrap();
    catalog::save_settings(
        &f.pool,
        SaveBehaviorSettings {
            settings: BehaviorSettings {
                enabled: false,
                count_special: false,
                ..settings
            },
        },
    )
    .await
    .unwrap();
    let mut day = BehaviorAttendanceDay {
        generation: Uuid::new_v4(),
        academic_term_id: f.term,
        date: future,
        revision: 1,
        facts: vec![],
    };
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day.clone())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let policies: i64 =
        sqlx::query_scalar("SELECT count(*) FROM behavior_source_days WHERE date=$1")
            .bind(future)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(policies, 0);
    let settings = catalog::settings(&f.pool).await.unwrap();
    catalog::save_settings(
        &f.pool,
        SaveBehaviorSettings {
            settings: BehaviorSettings {
                enabled: true,
                count_special: true,
                ..settings
            },
        },
    )
    .await
    .unwrap();
    catalog::save_rule(
        &f.pool,
        f.admin.actor_id,
        SaveBehaviorAttendanceRule {
            kind: BehaviorSource::Special,
            status: "skipped".into(),
            grade_level_id: None,
            effective_from: future,
            action: BehaviorScoreAction::Deduction,
            points: 6,
        },
    )
    .await
    .unwrap();
    day.revision = 2;
    day.facts.push(BehaviorAttendanceFact {
        source_key: "future-special".into(),
        source_session_id: None,
        kind: BehaviorSource::Special,
        student_id: f.a,
        status: "skipped".into(),
        counted: true,
        grade_level_id: f.grade,
        homeroom_id: Some(f.room_a),
        overrides: None,
    });
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(&mut tx, day)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let points:i64=sqlx::query_scalar("SELECT COALESCE(sum(points),0)::bigint FROM behavior_daily_totals WHERE student_id=$1 AND date=$2 AND side='deduction'").bind(f.a).bind(future).fetch_one(&f.pool).await.unwrap();
    assert_eq!(
        points, 6,
        "first real special/skipped fact must use the enabled policy"
    );
}

#[tokio::test]
async fn item_create_and_update_are_replay_safe_after_ambiguous_transport() {
    let f = fixture("behavior_item_replay").await;
    let mut request = SaveBehaviorItem {
        request_id: Uuid::new_v4(),
        id: None,
        row_version: None,
        name: "Replay-safe item".into(),
        side: BehaviorSide::Merit,
        enabled: true,
        effective_from: today(),
        points: 3,
    };
    let first = catalog::save_item(&f.pool, f.admin.actor_id, request.clone())
        .await
        .unwrap();
    let second = catalog::save_item(&f.pool, f.admin.actor_id, request.clone())
        .await
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.version_id, second.version_id);
    assert_eq!(second.row_version, 1);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM behavior_items WHERE name='Replay-safe item'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    request.name = "Different payload".into();
    assert!(matches!(
        catalog::save_item(&f.pool, f.admin.actor_id, request.clone()).await,
        Err(AppError::Conflict(_))
    ));
    request.request_id = Uuid::new_v4();
    request.id = Some(first.id);
    request.row_version = Some(first.row_version);
    let updated = catalog::save_item(&f.pool, f.admin.actor_id, request.clone())
        .await
        .unwrap();
    assert_eq!(updated.row_version, 2);
    let replay = catalog::save_item(&f.pool, f.admin.actor_id, request)
        .await
        .unwrap();
    assert_eq!(replay.row_version, 2);
    assert_eq!(replay.id, first.id);
}
#[tokio::test]
async fn grade_rule_precedes_newer_global_and_future_rule_and_special_override() {
    let f = fixture("behavior_rule_precedence").await;
    let settings = catalog::settings(&f.pool).await.unwrap();
    catalog::save_settings(
        &f.pool,
        SaveBehaviorSettings {
            settings: BehaviorSettings {
                enabled: true,
                ..settings
            },
        },
    )
    .await
    .unwrap();
    for (offset, grade, points) in [
        (-2, None, 3),
        (0, None, 9),
        (1, None, 40),
        (-5, Some(f.grade), 5),
    ] {
        catalog::save_rule(
            &f.pool,
            f.admin.actor_id,
            SaveBehaviorAttendanceRule {
                kind: BehaviorSource::Arrival,
                status: "present".into(),
                grade_level_id: grade,
                effective_from: today() + Duration::days(offset),
                action: BehaviorScoreAction::Merit,
                points,
            },
        )
        .await
        .unwrap();
    }
    let basic = |key: &str, student| BehaviorAttendanceFact {
        source_key: key.into(),
        source_session_id: None,
        kind: BehaviorSource::Arrival,
        student_id: student,
        status: "present".into(),
        counted: true,
        grade_level_id: f.grade,
        homeroom_id: None,
        overrides: None,
    };
    let a = basic("arrival", f.a);
    let mut b = basic("special-override", f.b);
    b.kind = BehaviorSource::Special;
    b.overrides = Some(vec![BehaviorScoreOverride {
        status: "present".into(),
        action: BehaviorScoreAction::Merit,
        points: 2,
    }]);
    let mut c = basic("special-none", f.unassigned);
    c.kind = BehaviorSource::Special;
    c.overrides = Some(vec![]);
    let mut tx = f.pool.begin().await.unwrap();
    integration::replace_attendance_day(
        &mut tx,
        BehaviorAttendanceDay {
            generation: Uuid::new_v4(),
            academic_term_id: f.term,
            date: today(),
            revision: 1,
            facts: vec![a, b, c],
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(summary(&f, f.a).await.current.merit, 5);
    assert_eq!(summary(&f, f.b).await.current.merit, 2);
    assert_eq!(summary(&f, f.unassigned).await.current.merit, 0);
}

#[tokio::test]
async fn manual_selector_keeps_unassigned_students_without_optional_profile() {
    let f = fixture("behavior_missing_optional_profile").await;
    sqlx::query("DELETE FROM student_info WHERE user_id=$1")
        .bind(f.unassigned)
        .execute(&f.pool)
        .await
        .unwrap();
    let page = manual::students(&f.pool, f.admin, query(&f)).await.unwrap();
    let student = page.items.iter().find(|s| s.id == f.unassigned).unwrap();
    assert_eq!(student.student_code, None);
    assert_eq!(student.homeroom_id, None);
    assert!(manual::students(&f.pool, f.teacher, query(&f))
        .await
        .unwrap()
        .items
        .iter()
        .all(|s| s.id != f.unassigned));
}

#[tokio::test]
async fn summaries_retain_other_term_history_and_resets_require_explicit_inclusive_cutoffs() {
    let f = fixture("behavior_cross_term_history").await;
    let now = today();
    let closed_on = now - Duration::days(1);
    let old_term:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,bell_schedule_id,status,included_in_year_result,blocks_year_closure)SELECT academic_year_id,2,'OLD','Archived Term','custom',bell_schedule_id,'closed',false,false FROM academic_terms WHERE id=$1 RETURNING id").bind(f.term).fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,bell_schedule_id,status,included_in_year_result,blocks_year_closure)SELECT academic_year_id,3,'UNUSED','Never Activated','custom',bell_schedule_id,'planning',false,false FROM academic_terms WHERE id=$1").bind(f.term).execute(&f.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO academic_years(year,name,status)VALUES(2998,'Unactivated year','planning')",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE behavior_state SET activation_date=$1 WHERE singleton")
        .bind(now - Duration::days(3))
        .execute(&f.pool)
        .await
        .unwrap();
    let history:Uuid=sqlx::query_scalar("INSERT INTO behavior_cycles(side,starts_on,starts_at,ends_on,ends_at,state,label)VALUES('deduction',$1,$1::date::timestamp AT TIME ZONE 'Asia/Bangkok',$2,$3::date::timestamp AT TIME ZONE 'Asia/Bangkok','history','Earlier term')RETURNING id").bind(now-Duration::days(3)).bind(closed_on).bind(now).fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,homeroom_id,side,source,cycle_id,points,details_purged)VALUES($1,$2,$3,$4,$5,'deduction','manual',$6,20,true)").bind(f.a).bind(now-Duration::days(2)).bind(old_term).bind(f.grade).bind(f.room_a).bind(history).execute(&f.pool).await.unwrap();
    let current_item = item(&f, BehaviorSide::Deduction, 5).await;
    let request = payload(&f, f.admin, &current_item, vec![f.a]).await;
    manual::create_batch(&f.pool, f.admin, request)
        .await
        .unwrap();
    let all = report::student_summary(
        &f.pool,
        f.admin,
        f.a,
        BehaviorSummaryQuery {
            start: None,
            end: None,
            academic_term_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        (
            all.current.deduction,
            all.history.deduction,
            all.lifetime.deduction
        ),
        (5, 20, 25)
    );
    let selected = report::student_summary(
        &f.pool,
        f.admin,
        f.a,
        BehaviorSummaryQuery {
            start: None,
            end: None,
            academic_term_id: Some(f.term),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        (
            selected.current.deduction,
            selected.history.deduction,
            selected.lifetime.deduction
        ),
        (5, 0, 5)
    );
    sqlx::query("UPDATE behavior_cycles SET starts_on=$1,starts_at=$1::date::timestamp AT TIME ZONE 'Asia/Bangkok' WHERE side='deduction' AND state='current'")
        .bind(closed_on).execute(&f.pool).await.unwrap();
    let before_cutoff = closed_on - Duration::days(1);
    let ready = BehaviorResetReadiness {
        finalized_through: now,
        delivered_through: now,
    };
    assert_eq!(
        schedules::run_due_resets(&f.pool, now, ready.clone(), 8)
            .await
            .unwrap(),
        0,
        "academic lifecycle state never supplies an implicit Behavior cutoff"
    );
    let settings = catalog::settings(&f.pool).await.unwrap();
    catalog::save_settings(
        &f.pool,
        SaveBehaviorSettings {
            settings: BehaviorSettings {
                deduction_reset: BehaviorResetSchedule::Scheduled,
                deduction_reset_dates: vec![closed_on],
                ..settings
            },
        },
    )
    .await
    .unwrap();
    for (run_date, readiness) in [
        (closed_on, ready.clone()),
        (
            now,
            BehaviorResetReadiness {
                finalized_through: before_cutoff,
                delivered_through: now,
            },
        ),
        (
            now,
            BehaviorResetReadiness {
                finalized_through: now,
                delivered_through: before_cutoff,
            },
        ),
    ] {
        assert_eq!(
            schedules::run_due_resets(&f.pool, run_date, readiness, 8)
                .await
                .unwrap(),
            0,
            "inclusive cutoff must pass and both Attendance watermarks must be ready"
        );
    }
    assert_eq!(
        schedules::run_due_resets(&f.pool, now, ready.clone(), 8)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        schedules::run_due_resets(&f.pool, now, ready, 8)
            .await
            .unwrap(),
        0,
        "completed cutoff must not reset the next cycle again"
    );
    let cycles = catalog::cycles(&f.pool).await.unwrap();
    let current = cycles
        .iter()
        .find(|cycle| cycle.side == BehaviorSide::Deduction && cycle.state == "current")
        .unwrap();
    assert_eq!(current.starts_on, now);
    assert!(cycles
        .iter()
        .any(|cycle| cycle.side == BehaviorSide::Deduction
            && cycle.state == "history"
            && cycle.ends_on == Some(closed_on)));
    let after_reset = summary(&f, f.a).await;
    assert_eq!(
        (
            after_reset.current.deduction,
            after_reset.history.deduction,
            after_reset.lifetime.deduction
        ),
        (5, 20, 25),
        "reset carries later facts into the new cycle and preserves older term history once"
    );
}
