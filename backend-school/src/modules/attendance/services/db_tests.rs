use super::*;
use crate::modules::academic::{
    core::services_tests::prepare_current_core_fixture,
    cutover_test_support::apply_migrations_through,
};
use school_attendance::{
    rules::FACE_MODEL,
    services::{faces, notifications, reports},
};
const TEACHER: Uuid = Uuid::from_u128(0x50000000000000000000000000000002);
const STUDENT: Uuid = Uuid::from_u128(0x50000000000000000000000000000001);
const TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000251);
struct Fixture {
    pool: PgPool,
    actor: ActorContext,
    students: Vec<AttendanceRosterStudent>,
}
async fn fixture(name: &str) -> Fixture {
    fixture_at(name, 105).await
}
async fn fixture_at(name: &str, version: i64) -> Fixture {
    let pool = prepare_current_core_fixture(name).await;
    apply_migrations_through(&pool, version).await.unwrap();
    let actor = ActorContext {
        user_id: TEACHER,
        permissions: vec![
            codes::ATTENDANCE_UPDATE_ASSIGNED.into(),
            codes::ATTENDANCE_READ_ASSIGNED.into(),
            codes::ATTENDANCE_VERIFY_ASSIGNED.into(),
        ],
    };
    let first:AttendanceRosterStudent=sqlx::query_as("SELECT student_id,id student_academic_year_id,1::integer class_number FROM student_academic_years WHERE student_id=$1 AND academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=$2)").bind(STUDENT).bind(TERM).fetch_one(&pool).await.unwrap();
    let second = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$2,'fixture-not-a-login','คนที่สอง','ทดสอบ','student','active')").bind(second).bind(format!("{}@example.invalid",second.simple())).execute(&pool).await.unwrap();
    let year:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT gen_random_uuid(),$1,academic_year_id,grade_level_id,study_program_id,'active' FROM student_academic_years WHERE id=$2 RETURNING id").bind(second).bind(first.student_academic_year_id).fetch_one(&pool).await.unwrap();
    let students = vec![
        first,
        AttendanceRosterStudent {
            student_id: second,
            student_academic_year_id: year,
            class_number: Some(2),
        },
    ];
    Fixture {
        pool,
        actor,
        students,
    }
}
fn seed(f: &Fixture, kind: AttendanceKind, date: NaiveDate) -> AttendanceSeed {
    AttendanceSeed {
        session: seed_session(
            TERM,
            date,
            kind,
            "fixture-room".into(),
            "เช็คชื่อทดสอบ".into(),
            vec![TEACHER],
            None,
            None,
            None,
            NaiveTime::MIN,
            NaiveTime::MIN,
        ),
        students: f.students.clone(),
    }
}
fn save_payload(
    detail: &AttendanceDetail,
    student: Uuid,
    result: AttendanceResult,
) -> SaveAttendanceResults {
    SaveAttendanceResults {
        session: session_context(&detail.session),
        roster_token: detail.session.roster_token.clone(),
        row_version: detail.session.row_version,
        reason: if detail.session.saved_at.is_some() {
            "ปรับผลทดสอบ".into()
        } else {
            String::new()
        },
        students: vec![AttendanceResultInput {
            student_id: student,
            result,
            note: String::new(),
        }],
    }
}
async fn parents(f: &Fixture) {
    for number in 0..2 {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$2,'fixture-not-a-login','ผู้ปกครอง','ทดสอบ','parent','active')").bind(id).bind(format!("parent-{number}@example.invalid")).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO student_parents(student_user_id,parent_user_id,relationship) VALUES($1,$2,'parent')").bind(STUDENT).bind(id).execute(&f.pool).await.unwrap();
    }
}
#[tokio::test]
async fn attendance_calendar_can_be_first_save_and_stale_settings_cannot_overwrite_it() {
    let f = fixture("attendance_first_calendar").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let before = settings::get(&f.pool, TERM).await.unwrap();
    assert_eq!(before.row_version, 0);
    let saved = settings::save_days(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceDays {
            row_version: 0,
            days: vec![AttendanceDay {
                date,
                counted: false,
                note: "วันหยุด".into(),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.row_version, 1);
    assert_eq!(
        saved.configuration.late_after,
        before.configuration.late_after
    );
    assert!(
        !settings::is_counted(&f.pool, TERM, date, &saved.configuration)
            .await
            .unwrap()
    );
    assert!(settings::save(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            row_version: 0,
            configuration: before.configuration,
        }
    )
    .await
    .is_err());
    assert_eq!(settings::get(&f.pool, TERM).await.unwrap().row_version, 1);
}
#[tokio::test]
async fn attendance_partial_save_preserves_unchecked_and_deduplicates_every_guardian() {
    let f = fixture("attendance_save_results").await;
    parents(&f).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let d = sessions::detail(&f.pool, &f.actor, s.id).await.unwrap();
    assert!(d
        .students
        .iter()
        .all(|r| r.result == AttendanceResult::Unchecked));
    assert!(sessions::save(
        &f.pool,
        &f.actor,
        s.id,
        SaveAttendanceResults {
            session: session_context(&d.session),
            roster_token: d.session.roster_token.clone(),
            row_version: d.session.row_version,
            reason: String::new(),
            students: vec![]
        }
    )
    .await
    .is_err());
    let payload = save_payload(&d, STUDENT, AttendanceResult::Present);
    let saved = sessions::save(&f.pool, &f.actor, s.id, payload)
        .await
        .unwrap();
    assert_eq!(
        saved
            .students
            .iter()
            .filter(|r| r.result == AttendanceResult::Unchecked)
            .count(),
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM attendance_notifications")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
    let payload = save_payload(&saved, STUDENT, AttendanceResult::Present);
    let saved = sessions::save(&f.pool, &f.actor, s.id, payload)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        3
    );
    let payload = save_payload(&saved, STUDENT, AttendanceResult::Late);
    sessions::save(&f.pool, &f.actor, s.id, payload)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        6
    );
    let unrelated = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: f.actor.permissions.clone(),
    };
    assert!(sessions::detail(&f.pool, &unrelated, s.id).await.is_err());
    assert!(
        reports::summaries(&f.pool, TERM, None, Some(unrelated.user_id))
            .await
            .unwrap()
            .summaries
            .is_empty()
    );
    for n in notifications::pending(&f.pool).await.unwrap() {
        notifications::store(&f.pool, &n).await.unwrap();
        notifications::store(&f.pool, &n).await.unwrap();
        notifications::delivered(&f.pool, n.id).await.unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notifications WHERE title IN('เข้าโรงเรียนแล้ว','ผลเช็คชื่อ','แก้ไขผลเช็คชื่อ')"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        6
    );
    notifications::queue_digest(
        &f.pool,
        TERM,
        date,
        NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    notifications::queue_digest(
        &f.pool,
        TERM,
        date,
        NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_notifications WHERE dedup_key LIKE 'digest:%'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
}
#[tokio::test]
async fn attendance_calendar_cancelled_and_unchecked_results_survive_verified_purge() {
    let f = fixture("attendance_verified_purge").await;
    let config = settings::save(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            configuration: AttendanceConfiguration::default(),
            row_version: 0,
        },
    )
    .await
    .unwrap();
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let d = sessions::detail(&f.pool, &f.actor, s.id).await.unwrap();
    let p = save_payload(&d, STUDENT, AttendanceResult::Activity);
    let saved = sessions::save(&f.pool, &f.actor, s.id, p).await.unwrap();
    let excluded = NaiveDate::from_ymd_opt(2025, 10, 10).unwrap();
    settings::save_days(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceDays {
            row_version: config.row_version,
            days: vec![AttendanceDay {
                date: excluded,
                counted: false,
                note: "วันหยุด".into(),
            }],
        },
    )
    .await
    .unwrap();
    let holiday = sessions::ensure(
        &f.pool,
        &f.actor,
        seed(&f, AttendanceKind::Arrival, excluded),
    )
    .await
    .unwrap();
    let d = sessions::detail(&f.pool, &f.actor, holiday.id)
        .await
        .unwrap();
    assert!(!d.counted);
    let p = save_payload(&d, STUDENT, AttendanceResult::Present);
    sessions::save(&f.pool, &f.actor, holiday.id, p)
        .await
        .unwrap();
    let cancelled = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Lesson, date))
        .await
        .unwrap();
    let d = sessions::detail(&f.pool, &f.actor, cancelled.id)
        .await
        .unwrap();
    sessions::cancel(
        &f.pool,
        &f.actor,
        cancelled.id,
        AttendanceCancellation {
            session: session_context(&d.session),
            roster_token: d.session.roster_token.clone(),
            row_version: d.session.row_version,
            cancelled: true,
            reason: "กิจกรรมแทนเรียน".into(),
        },
    )
    .await
    .unwrap();
    let unchecked = sessions::ensure(
        &f.pool,
        &f.actor,
        seed(
            &f,
            AttendanceKind::Arrival,
            NaiveDate::from_ymd_opt(2025, 10, 8).unwrap(),
        ),
    )
    .await
    .unwrap();
    assert!(sessions::detail(&f.pool, &f.actor, unchecked.id)
        .await
        .unwrap()
        .students
        .iter()
        .all(|r| r.result == AttendanceResult::Unchecked));
    let before = reports::summaries(&f.pool, TERM, None, None).await.unwrap();
    assert!(before.summaries.iter().any(|s| s.student_id == STUDENT
        && s.category == "school"
        && s.activity == 1
        && s.unchecked == 1));
    assert!(before.summaries.iter().all(|s| s.category != "lesson"));
    let scoped = reports::summaries(&f.pool, TERM, None, Some(TEACHER))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&scoped.summaries).unwrap(),
        serde_json::to_value(&before.summaries).unwrap()
    );
    let impact = reports::impact(&f.pool, TERM).await.unwrap();
    assert!(reports::purge(
        &f.pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: impact.records,
            reason: "ทดสอบ".into()
        }
    )
    .await
    .is_err());
    for n in notifications::pending(&f.pool).await.unwrap() {
        notifications::store(&f.pool, &n).await.unwrap();
        notifications::delivered(&f.pool, n.id).await.unwrap();
    }
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on='2025-10-31' WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(reports::purge(
        &f.pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: impact.records + 1,
            reason: "ทดสอบ".into()
        }
    )
    .await
    .is_err());
    reports::purge(
        &f.pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: impact.records,
            reason: "ทดสอบเก็บสรุป".into(),
        },
    )
    .await
    .unwrap();
    let after = reports::summaries(&f.pool, TERM, None, None).await.unwrap();
    let archived_first = serde_json::to_value(&after.summaries[0]).unwrap();
    assert!(after.archived);
    assert_eq!(
        serde_json::to_value(before.summaries).unwrap(),
        serde_json::to_value(after.summaries).unwrap()
    );
    assert_eq!(
        serde_json::to_value(scoped.summaries).unwrap(),
        serde_json::to_value(
            reports::summaries(&f.pool, TERM, None, Some(TEACHER))
                .await
                .unwrap()
                .summaries
        )
        .unwrap()
    );
    let archived_page = report(&f.pool, &f.actor, report_query(None, 1, 1))
        .await
        .unwrap();
    assert!(archived_page.archived);
    assert_eq!(archived_page.total, scoped.total);
    assert_eq!(archived_page.summaries.len(), 1);
    assert_eq!(
        serde_json::to_value(&archived_page.summaries[0]).unwrap(),
        archived_first
    );
    assert_eq!(reports::impact(&f.pool, TERM).await.unwrap().records, 0);
    assert!(sessions::save(
        &f.pool,
        &f.actor,
        s.id,
        save_payload(&saved, STUDENT, AttendanceResult::Present)
    )
    .await
    .is_err());
}
#[tokio::test]
async fn attendance_special_round_refuses_overlapping_students_and_distinct_rounds_are_valid() {
    let f = fixture("attendance_special_groups").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let definition = SpecialAttendanceDefinition {
        title: "ชุมนุม".into(),
        dates: vec![date],
        start_time: NaiveTime::MIN,
        end_time: NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
        counted: true,
        notify: true,
        groups: vec![SpecialAttendanceGroup {
            name: "กลุ่ม 1".into(),
            teacher_ids: vec![TEACHER],
            homeroom_ids: vec![],
            audience_group_ids: vec![],
            student_ids: vec![STUDENT],
            use_homeroom_advisors: false,
        }],
    };
    let s = seed(&f, AttendanceKind::Special, date);
    let overlapping = vec![s.clone(), s.clone()];
    assert!(specials::create_template(
        &f.pool,
        TEACHER,
        TERM,
        definition.clone(),
        vec![overlapping]
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_special_templates")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    specials::create_template(
        &f.pool,
        TEACHER,
        TERM,
        definition.clone(),
        vec![vec![s.clone()]],
    )
    .await
    .unwrap();
    specials::create_template(&f.pool, TEACHER, TERM, definition, vec![vec![s]])
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn attendance_scans_commit_evidence_once_and_do_not_mark_other_students_absent() {
    let f = fixture("attendance_scan_evidence").await;
    parents(&f).await;
    let date = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=(SELECT academic_year_id FROM academic_terms WHERE id=$1)").bind(TERM).bind(date+chrono::Duration::days(2)).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE academic_terms SET start_date=$2,planned_end_date=$3 WHERE id=$1")
        .bind(TERM)
        .bind(date - chrono::Duration::days(1))
        .bind(date + chrono::Duration::days(1))
        .execute(&f.pool)
        .await
        .unwrap();
    let configuration = AttendanceConfiguration {
        late_after: NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
        ..Default::default()
    };
    settings::save(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            configuration,
            row_version: 0,
        },
    )
    .await
    .unwrap();
    let descriptor = FaceDescriptor {
        values: vec![0.01; 128],
    };
    faces::enroll(
        &f.pool,
        TEACHER,
        STUDENT,
        EnrollAttendanceFace {
            model: FACE_MODEL.into(),
            descriptors: vec![descriptor.clone(), descriptor.clone()],
            consent_confirmed: true,
        },
    )
    .await
    .unwrap();
    let encrypted: String = sqlx::query_scalar(
        "SELECT encrypted_descriptors FROM attendance_face_enrollments WHERE student_id=$1",
    )
    .bind(STUDENT)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert!(!encrypted.contains("values"));
    let device = specials::save_device(
        &f.pool,
        TEACHER,
        Uuid::new_v4(),
        SaveAttendanceDevice {
            name: "เว็บแคม".into(),
            enabled: true,
            operator_id: TEACHER,
        },
    )
    .await
    .unwrap();
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let file = evidence_file(&f.pool, STUDENT).await;
    let scan = AttendanceScan {
        event_id: Uuid::new_v4(),
        device_id: device.id,
        session_id: s.id,
        session: session_context(&s),
        roster_token: s.roster_token.clone(),
        student_id: STUDENT,
        descriptor,
        evidence_file_id: file,
        captured_at: Utc::now(),
    };
    let old_gallery = faces::gallery(&f.pool, TERM).await.unwrap();
    faces::remove(&f.pool, TEACHER, STUDENT).await.unwrap();
    faces::enroll(
        &f.pool,
        TEACHER,
        STUDENT,
        EnrollAttendanceFace {
            model: FACE_MODEL.into(),
            descriptors: vec![
                FaceDescriptor {
                    values: vec![0.8; 128]
                };
                2
            ],
            consent_confirmed: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        old_gallery[0].row_version,
        faces::gallery(&f.pool, TERM).await.unwrap()[0].row_version
    );
    assert!(matches!(
        faces::scan_with_gallery(&f.pool, &f.actor, scan.clone(), &old_gallery).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_scan_events")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert!(sessions::detail(&f.pool, &f.actor, s.id)
        .await
        .unwrap()
        .students
        .iter()
        .all(|r| r.result == AttendanceResult::Unchecked));
    faces::enroll(
        &f.pool,
        TEACHER,
        STUDENT,
        EnrollAttendanceFace {
            model: FACE_MODEL.into(),
            descriptors: vec![scan.descriptor.clone(); 2],
            consent_confirmed: true,
        },
    )
    .await
    .unwrap();
    let result = faces::scan(&f.pool, &f.actor, scan.clone()).await.unwrap();
    assert!(!result.duplicate);
    assert_eq!(result.result, AttendanceResult::Present);
    assert!(
        faces::scan(&f.pool, &f.actor, scan.clone())
            .await
            .unwrap()
            .duplicate
    );
    let mut expired_replay = scan.clone();
    expired_replay.captured_at -= chrono::Duration::minutes(6);
    sqlx::query("UPDATE attendance_scan_events SET captured_at=$2 WHERE id=$1")
        .bind(scan.event_id)
        .bind(expired_replay.captured_at)
        .execute(&f.pool)
        .await
        .unwrap();
    faces::remove(&f.pool, TEACHER, STUDENT).await.unwrap();
    assert!(
        faces::scan(&f.pool, &f.actor, expired_replay.clone())
            .await
            .unwrap()
            .duplicate
    );
    let mut altered = expired_replay.clone();
    altered.evidence_file_id = Uuid::new_v4();
    assert!(matches!(
        faces::scan(&f.pool, &f.actor, altered).await,
        Err(AppError::Conflict(_))
    ));
    expired_replay.event_id = Uuid::new_v4();
    assert!(faces::scan(&f.pool, &f.actor, expired_replay)
        .await
        .is_err());
    let d = sessions::detail(&f.pool, &f.actor, s.id).await.unwrap();
    assert_eq!(
        d.students
            .iter()
            .filter(|r| r.result == AttendanceResult::Unchecked)
            .count(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        3
    );
    let flag = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Lesson, date))
        .await
        .unwrap();
    let d = sessions::detail(&f.pool, &f.actor, flag.id).await.unwrap();
    let payload = save_payload(&d, STUDENT, AttendanceResult::Present);
    sessions::save(&f.pool, &f.actor, flag.id, payload)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        6
    );
    let impact = reports::impact(&f.pool, TERM).await.unwrap();
    assert_eq!(impact.evidence_bytes, 50000);
    sqlx::query("UPDATE files SET expires_at=now()-interval '1 minute' WHERE id=$1")
        .bind(file)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(faces::expire_evidence(&f.pool).await.unwrap(), 1);
    let status: String = sqlx::query_scalar("SELECT lifecycle_status FROM files WHERE id=$1")
        .bind(file)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(status, "delete_requested");
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM file_operations WHERE file_id=$1 AND operation_type='delete_object')").bind(file).fetch_one(&f.pool).await.unwrap());
    // An explicit lesson absence remains independent of an accepted arrival scan.
    let second = f.students[1].student_id;
    let lesson = sessions::detail(&f.pool, &f.actor, flag.id).await.unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        flag.id,
        save_payload(&lesson, second, AttendanceResult::Absent),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO student_parents(student_user_id,parent_user_id,relationship) SELECT $1,parent_user_id,'parent' FROM student_parents WHERE student_user_id=$2")
        .bind(second).bind(STUDENT).execute(&f.pool).await.unwrap();
    let descriptor = FaceDescriptor {
        values: vec![0.2; 128],
    };
    faces::enroll(
        &f.pool,
        TEACHER,
        second,
        EnrollAttendanceFace {
            model: FACE_MODEL.into(),
            descriptors: vec![descriptor.clone(), descriptor.clone()],
            consent_confirmed: true,
        },
    )
    .await
    .unwrap();
    let mut config = settings::get(&f.pool, TERM).await.unwrap();
    config.configuration.late_after = NaiveTime::MIN;
    config.configuration.weekdays = vec![1, 2, 3, 4, 5, 6, 7];
    settings::save(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            configuration: config.configuration,
            row_version: config.row_version,
        },
    )
    .await
    .unwrap();
    let late = AttendanceScan {
        event_id: Uuid::new_v4(),
        device_id: device.id,
        session_id: s.id,
        session: session_context(&s),
        roster_token: s.roster_token.clone(),
        student_id: second,
        descriptor,
        evidence_file_id: evidence_file(&f.pool, second).await,
        captured_at: Utc::now(),
    };
    assert_eq!(
        faces::scan(&f.pool, &f.actor, late.clone())
            .await
            .unwrap()
            .result,
        AttendanceResult::Late
    );
    assert!(
        faces::scan(&f.pool, &f.actor, late)
            .await
            .unwrap()
            .duplicate
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM attendance_notifications WHERE student_id=$1 AND dedup_key LIKE 'arrival:%'").bind(second).fetch_one(&f.pool).await.unwrap(), 3);
    let live = reports::summaries(&f.pool, TERM, Some(second), None)
        .await
        .unwrap();
    let school = live
        .summaries
        .iter()
        .find(|r| r.category == "school")
        .unwrap();
    assert_eq!((school.late, school.absent, school.expected), (1, 0, 1));
    assert_eq!(
        live.summaries
            .iter()
            .find(|r| r.category == "lesson")
            .unwrap()
            .absent,
        1
    );
    for n in notifications::pending(&f.pool).await.unwrap() {
        notifications::store(&f.pool, &n).await.unwrap();
        notifications::delivered(&f.pool, n.id).await.unwrap();
    }
    let impact = reports::impact(&f.pool, TERM).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=$2 WHERE id=$1")
        .bind(TERM)
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    reports::purge(
        &f.pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: impact.records,
            reason: "เก็บสรุปการมาหลังหน้าเสาธง".into(),
        },
    )
    .await
    .unwrap();
    let archived = reports::summaries(&f.pool, TERM, Some(second), None)
        .await
        .unwrap();
    assert!(archived.archived);
    let school = archived
        .summaries
        .iter()
        .find(|r| r.category == "school")
        .unwrap();
    assert_eq!((school.late, school.absent, school.expected), (1, 0, 1));
    assert_eq!(
        archived
            .summaries
            .iter()
            .find(|r| r.category == "lesson")
            .unwrap()
            .absent,
        1
    );
}

#[tokio::test]
async fn attendance_dated_providers_and_workspace_read_do_not_create_facts() {
    let f = fixture("attendance_dated_providers").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let mut actor = f.actor.clone();
    actor.permissions.push(codes::ATTENDANCE_READ_SCHOOL.into());
    let w = workspace(
        &f.pool,
        &actor,
        AttendanceQuery {
            academic_term_id: TERM,
            date,
        },
    )
    .await
    .unwrap();
    assert!(w.sessions.iter().any(|s| s.kind == AttendanceKind::Arrival));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let preview = w
        .sessions
        .iter()
        .find(|s| s.kind == AttendanceKind::Arrival)
        .unwrap();
    let detail = open(&f.pool, &actor, session_context(preview))
        .await
        .unwrap();
    assert_eq!(detail.session.row_version, 0);
    let _again = open(&f.pool, &actor, session_context(preview))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let rows = core::students(&f.pool, TERM, date).await.unwrap();
    assert!(rows.iter().any(|s| s.student_id == STUDENT));
    let opts = options(
        &f.pool,
        &ActorContext {
            user_id: TEACHER,
            permissions: vec![codes::ATTENDANCE_ENROLL_SCHOOL.into()],
        },
        TERM,
    )
    .await
    .unwrap();
    assert!(opts.audiences.is_empty());
}

#[tokio::test]
async fn attendance_concurrent_saves_keep_one_revision_and_do_not_grant_own_access_to_others() {
    let f = fixture("attendance_concurrent_save").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, s.id).await.unwrap();
    let a = save_payload(&detail, STUDENT, AttendanceResult::Present);
    let b = save_payload(&detail, STUDENT, AttendanceResult::Leave);
    let (a, b) = tokio::join!(
        sessions::save(&f.pool, &f.actor, s.id, a),
        sessions::save(&f.pool, &f.actor, s.id, b)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let failure = if let Err(error) = a {
        error
    } else {
        b.unwrap_err()
    };
    assert!(matches!(failure, AppError::Conflict(_)));
    let final_detail = sessions::detail(&f.pool, &f.actor, s.id).await.unwrap();
    assert_eq!(
        final_detail.session.row_version,
        detail.session.row_version + 1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    let student = ActorContext {
        user_id: STUDENT,
        permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
    };
    let own = report(&f.pool, &student, report_query(Some(STUDENT), 1, 50))
        .await
        .unwrap();
    assert!(own.summaries.iter().all(|s| s.student_id == STUDENT));
    assert!(!own.summaries.is_empty());
    assert!(report(
        &f.pool,
        &student,
        report_query(Some(f.students[1].student_id), 1, 50)
    )
    .await
    .is_err());
    assert!(report(&f.pool, &student, report_query(None, 1, 50))
        .await
        .is_err());
}

async fn evidence_file(pool: &PgPool, student: Uuid) -> Uuid {
    let file:Uuid=sqlx::query_scalar("INSERT INTO files(display_filename,purpose_code,visibility,lifecycle_status,retention_class,inspection_metadata,created_by,owner_user_id) VALUES('scan.jpg','attendance_evidence','private','ready','temporary','{\"kind\":\"image\",\"width_px\":640,\"height_px\":480}',$1,$2) RETURNING id").bind(TEACHER).bind(student).fetch_one(pool).await.unwrap();
    let version:Uuid=sqlx::query_scalar("INSERT INTO file_versions(file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status,created_by) VALUES($1,1,'r2','private','stored',$2,'image/jpeg','jpg',50000,repeat('a',64),'clean',$3) RETURNING id").bind(file).bind(format!("tenants/{}/attendance/evidence/{file}/v1/original.jpg", Uuid::new_v4())).bind(TEACHER).fetch_one(pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(file)
        .bind(version)
        .execute(pool)
        .await
        .unwrap();
    file
}

#[tokio::test]
async fn attendance_concurrent_arrival_notices_do_not_notify_twice() {
    let f = fixture("attendance_concurrent_daily_notice").await;
    parents(&f).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let arrival = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let mut other = seed(&f, AttendanceKind::Arrival, date);
    other.session.source_key = "another-room".into();
    other.session.id = Uuid::new_v4();
    let flag = sessions::ensure(&f.pool, &f.actor, other).await.unwrap();
    let mut arrival_tx = f.pool.begin().await.unwrap();
    notifications::queue_result(
        &mut arrival_tx,
        &arrival,
        STUDENT,
        AttendanceResult::Present,
        2,
        false,
    )
    .await
    .unwrap();
    let mut flag_tx = f.pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *flag_tx)
        .await
        .unwrap();
    let flag_task = tokio::spawn(async move {
        notifications::queue_result(
            &mut flag_tx,
            &flag,
            STUDENT,
            AttendanceResult::Present,
            2,
            false,
        )
        .await
        .unwrap();
        flag_tx.commit().await.unwrap();
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND NOT granted)").bind(pid).fetch_one(&f.pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("flag notice must wait for the uncommitted arrival decision");
    arrival_tx.commit().await.unwrap();
    flag_task.await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        3
    );
}

#[tokio::test]
async fn attendance_mixed_own_and_assigned_permissions_cannot_expand_student_history() {
    let f = fixture("attendance_mixed_scope").await;
    parents(&f).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let own = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let another_teacher = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$2,'fixture-not-a-login','ครู','อื่น','staff','active')").bind(another_teacher).bind(format!("{}@example.invalid",another_teacher)).execute(&f.pool).await.unwrap();
    let other_actor = ActorContext {
        user_id: another_teacher,
        permissions: vec![codes::ATTENDANCE_UPDATE_ASSIGNED.into()],
    };
    let mut other_seed = seed(&f, AttendanceKind::Lesson, date);
    other_seed.session.teacher_ids = vec![another_teacher];
    let other = sessions::ensure(&f.pool, &other_actor, other_seed)
        .await
        .unwrap();
    let mut mixed = f.actor.clone();
    mixed.permissions.push(codes::ATTENDANCE_READ_OWN.into());
    let query = || AttendanceHistoryQuery {
        academic_term_id: TERM,
        student_id: STUDENT,
        start: date,
        end: date,
    };
    let items = history(&f.pool, &mixed, query()).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].session_id, own.id);
    assert_ne!(items[0].session_id, other.id);
    let parent: Uuid = sqlx::query_scalar(
        "SELECT parent_user_id FROM student_parents WHERE student_user_id=$1 LIMIT 1",
    )
    .bind(STUDENT)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    for user_id in [STUDENT, parent] {
        let actor = ActorContext {
            user_id,
            permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
        };
        assert_eq!(history(&f.pool, &actor, query()).await.unwrap().len(), 2);
    }
    let school = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_READ_SCHOOL.into()],
    };
    assert_eq!(history(&f.pool, &school, query()).await.unwrap().len(), 2);
    let unrelated = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
    };
    assert!(history(&f.pool, &unrelated, query()).await.is_err());
    let file = evidence_file(&f.pool, STUDENT).await;
    let detail = sessions::detail(&f.pool, &other_actor, other.id)
        .await
        .unwrap();
    sessions::save(
        &f.pool,
        &other_actor,
        other.id,
        save_payload(&detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES(gen_random_uuid(),$1,$2,$4,$3,now())").bind(other.id).bind(STUDENT).bind(file).bind(specials::save_device(&f.pool, TEACHER, Uuid::new_v4(), SaveAttendanceDevice { name: "fixture".into(), enabled: true, operator_id: TEACHER }).await.unwrap().id).execute(&f.pool).await.unwrap();
    use school_file_platform::repository::FileRepository;
    let repository = school_file_platform::repository::SqlFileRepository::new(f.pool.clone());
    let platform_file = repository.load_delivery(file).await.unwrap().unwrap().file;
    assert!(require_evidence_file(
        &f.pool,
        &mixed,
        &platform_file,
        crate::policies::file_access_policy::FilePolicyAction::Read,
        Some(STUDENT)
    )
    .await
    .is_err());
    let linked = ActorContext {
        user_id: parent,
        permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
    };
    require_evidence_file(
        &f.pool,
        &linked,
        &platform_file,
        crate::policies::file_access_policy::FilePolicyAction::Read,
        Some(STUDENT),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn attendance_retention_drains_more_than_one_batch_after_term_closes() {
    let f = fixture("attendance_retention_batches").await;
    for _ in 0..105 {
        evidence_file(&f.pool, STUDENT).await;
    }
    sqlx::query("UPDATE files SET expires_at=now()-interval '1 minute' WHERE purpose_code='attendance_evidence'").execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on='2025-10-31' WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        faces::expire_evidence(&f.pool).await.unwrap(),
        faces::EVIDENCE_EXPIRY_BATCH
    );
    assert_eq!(faces::expire_evidence(&f.pool).await.unwrap(), 5);
    assert_eq!(faces::expire_evidence(&f.pool).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM file_operations WHERE operation_type='delete_object'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        105
    );
}

mod retention_tests {
    use super::*;
    use crate::services::cleaner::FileCleaner;
    use async_trait::async_trait;
    use bytes::Bytes;
    use school_file_platform::platform_service::FilePlatform;
    use school_file_platform::{
        malware_scanner::{MalwareScanner, ScanOutcome},
        platform_types::DownloadGrant,
        storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::{sync::Arc, time::Duration};
    use url::Url;

    #[derive(Default)]
    struct RetentionStorage(AtomicUsize);
    #[async_trait]
    impl StorageProvider for RetentionStorage {
        async fn check_readiness(&self) -> Result<(), StorageError> {
            panic!("retention must not probe readiness")
        }
        async fn put(&self, _: &StoredObject, _: Bytes) -> Result<(), StorageError> {
            panic!("retention must not upload")
        }
        async fn get(&self, _: &StoredObject, _: u64) -> Result<Bytes, StorageError> {
            panic!("retention must not read bytes")
        }
        async fn head(&self, _: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
            panic!("retention must not inspect objects")
        }
        async fn delete(&self, _: &StoredObject) -> Result<(), StorageError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn private_download_grant(
            &self,
            _: &StoredObject,
            _: &str,
            _: Duration,
        ) -> Result<DownloadGrant, StorageError> {
            panic!("retention must not grant downloads")
        }
        fn public_location(&self, _: &StoredObject) -> Result<Url, StorageError> {
            panic!("private evidence must not become public")
        }
    }
    struct RetentionScanner;
    #[async_trait]
    impl MalwareScanner for RetentionScanner {
        async fn scan(&self, _: &[u8]) -> ScanOutcome {
            panic!("retention must not scan content")
        }
    }

    #[tokio::test]
    async fn attendance_expiry_and_queued_deletions_drain_past_first_hourly_batch() {
        let f = fixture("attendance_file_cleaner_capacity").await;
        let pool = f.pool;
        let actor = f.actor.user_id;
        for _ in 0..105 {
            let file = Uuid::new_v4();
            sqlx::query("INSERT INTO files(id,owner_user_id,purpose_code,display_filename,visibility,lifecycle_status,retention_class,created_by,expires_at,inspection_metadata) VALUES($1,$2,'attendance_evidence','fixture.jpg','private','ready','temporary',$2,now()-interval '1 minute','{\"kind\":\"image\",\"width_px\":640,\"height_px\":480}')").bind(file).bind(actor).execute(&pool).await.unwrap();
            let version:Uuid=sqlx::query_scalar("INSERT INTO file_versions(file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status,created_by) VALUES($1,1,'r2','private','stored',$2,'image/jpeg','jpg',50000,repeat('a',64),'clean',$3) RETURNING id").bind(file).bind(format!("tenants/{}/attendance/evidence/{file}/v1/original.jpg", Uuid::new_v4())).bind(actor).fetch_one(&pool).await.unwrap();
            sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
                .bind(file)
                .bind(version)
                .execute(&pool)
                .await
                .unwrap();
        }
        assert_eq!(
            school_attendance::services::faces::expire_evidence(&pool)
                .await
                .unwrap(),
            100
        );
        let storage = Arc::new(RetentionStorage::default());
        let platform = Arc::new(FilePlatform::new(
            storage.clone(),
            Arc::new(RetentionScanner),
        ));
        FileCleaner::new(pool.clone(), platform)
            .reconcile_file_operations()
            .await;
        assert_eq!(storage.0.load(Ordering::SeqCst), 105);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM files WHERE lifecycle_status='deleted' AND purpose_code='attendance_evidence'").fetch_one(&pool).await.unwrap(), 105);
    }
}

fn report_query(student: Option<Uuid>, page: i64, page_size: i64) -> AttendanceReportQuery {
    AttendanceReportQuery {
        academic_term_id: TERM,
        student_id: student,
        page,
        page_size,
        search: String::new(),
        category: None,
    }
}
#[tokio::test]
async fn attendance_report_pages_preserve_counts_filters_and_teacher_scope() {
    let f = fixture("attendance_report_pages").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let own = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, own.id).await.unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        own.id,
        save_payload(&detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let mut foreign = seed(
        &f,
        AttendanceKind::Arrival,
        date + chrono::Duration::days(1),
    );
    foreign.session.source_key = "other-room".into();
    foreign.session.id = Uuid::new_v4();
    foreign.session.teacher_ids = vec![Uuid::new_v4()];
    let admin = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_UPDATE_SCHOOL.into()],
    };
    sessions::ensure(&f.pool, &admin, foreign).await.unwrap();
    let full = reports::summaries(&f.pool, TERM, None, Some(TEACHER))
        .await
        .unwrap();
    let mut rows = vec![];
    for page_number in 1..=full.total {
        let page = report(&f.pool, &f.actor, report_query(None, page_number, 1))
            .await
            .unwrap();
        assert_eq!(page.total, full.total);
        assert_eq!(page.summaries.len(), 1);
        rows.extend(page.summaries);
    }
    assert_eq!(
        serde_json::to_value(rows).unwrap(),
        serde_json::to_value(full.summaries).unwrap()
    );
    let empty = report(&f.pool, &f.actor, report_query(None, 100, 1))
        .await
        .unwrap();
    assert!(empty.summaries.is_empty());
    assert_eq!(empty.total, full.total);
    let mut filtered = report_query(None, 1, 100);
    filtered.category = Some("school".into());
    let school = report(&f.pool, &f.actor, filtered.clone()).await.unwrap();
    assert_eq!(school.total, 2);
    assert!(school.summaries.iter().all(|row| row.expected == 1));
    filtered.search = "%".into();
    assert_eq!(report(&f.pool, &f.actor, filtered).await.unwrap().total, 0);
    assert!(report(&f.pool, &f.actor, report_query(None, 0, 1))
        .await
        .is_err());
    assert!(report(&f.pool, &f.actor, report_query(None, 1, 101))
        .await
        .is_err());
    let outsider = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ATTENDANCE_READ_ASSIGNED.into()],
    };
    assert_eq!(
        report(&f.pool, &outsider, report_query(None, 1, 50))
            .await
            .unwrap()
            .total,
        0
    );
    assert!(matches!(
        report(&f.pool, &outsider, report_query(Some(STUDENT), 1, 50)).await,
        Err(AppError::Forbidden(_))
    ));
}
#[tokio::test]
async fn attendance_parent_migration_repairs_missing_roles_and_preserves_role_history() {
    let f = fixture_at("attendance_parent_role", 101).await;
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE code='PARENT')"
    )
    .fetch_one(&f.pool)
    .await
    .unwrap());
    let mut parents = vec![];
    for index in 0..3 {
        let parent = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'not-a-login','Parent','Fixture','parent',$3)")
            .bind(parent).bind(format!("parent-{parent}" )).bind(if index == 2 { "inactive" } else { "active" }).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO student_parents(student_user_id,parent_user_id,relationship) VALUES($1,$2,'parent')").bind(STUDENT).bind(parent).execute(&f.pool).await.unwrap();
        parents.push(parent);
    }
    let custom = Uuid::new_v4();
    sqlx::query("INSERT INTO roles(id,code,name,user_type) VALUES($1,'CUSTOM_PARENT','Custom parent','parent')").bind(custom).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id,started_at,ended_at) VALUES($1,$2,CURRENT_DATE-2,CURRENT_DATE-1)").bind(parents[1]).bind(custom).execute(&f.pool).await.unwrap();
    apply_migrations_through(&f.pool, 102).await.unwrap();
    apply_migrations_through(&f.pool, 105).await.unwrap();
    let cache = school_authorization::PermissionCache::new();
    let actor = school_authorization::load_actor_context(parents[0], "fixture", &f.pool, &cache)
        .await
        .unwrap();
    assert!(actor.has_permission(codes::ATTENDANCE_READ_OWN));
    assert!(report(&f.pool, &actor, report_query(Some(STUDENT), 1, 50))
        .await
        .is_ok());
    assert!(matches!(
        report(
            &f.pool,
            &actor,
            report_query(Some(f.students[1].student_id), 1, 50)
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    for parent in &parents[1..] {
        assert!(!sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=$1 AND r.code='PARENT')").bind(parent).fetch_one(&f.pool).await.unwrap());
    }
}

#[tokio::test]
async fn attendance_enrollment_metadata_is_scoped_and_withdrawal_reconciles_it() {
    let f = fixture("attendance_enrollment_metadata").await;
    sqlx::query("INSERT INTO homeroom_advisors (homeroom_id,user_id,role) SELECT p.homeroom_id,$1,'primary' FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id JOIN academic_terms t ON t.academic_year_id=y.academic_year_id WHERE y.student_id=$2 AND t.id=$3 AND p.status='current'")
        .bind(TEACHER)
        .bind(STUDENT)
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    faces::enroll(
        &f.pool,
        TEACHER,
        STUDENT,
        EnrollAttendanceFace {
            model: FACE_MODEL.into(),
            descriptors: vec![
                FaceDescriptor {
                    values: vec![0.01; 128]
                };
                3
            ],
            consent_confirmed: true,
        },
    )
    .await
    .unwrap();
    let scoped = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_ENROLL_ASSIGNED.into()],
    };
    let rows = face_enrollments(&f.pool, &scoped, TERM).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].student_id, STUDENT);
    let wire = serde_json::to_value(&rows).unwrap();
    assert!(wire[0].get("descriptors").is_none());
    assert!(wire[0].get("model").is_none());
    let outsider = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: scoped.permissions.clone(),
    };
    assert!(face_enrollments(&f.pool, &outsider, TERM)
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        face_enrollments(&f.pool, &f.actor, TERM).await,
        Err(AppError::Forbidden(_))
    ));
    let school = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_ENROLL_SCHOOL.into()],
    };
    assert_eq!(
        face_enrollments(&f.pool, &school, TERM)
            .await
            .unwrap()
            .len(),
        1
    );
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(STUDENT)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(face_enrollments(&f.pool, &school, TERM)
        .await
        .unwrap()
        .is_empty());
    sqlx::query("UPDATE users SET status='active' WHERE id=$1")
        .bind(STUDENT)
        .execute(&f.pool)
        .await
        .unwrap();
    faces::remove(&f.pool, TEACHER, STUDENT).await.unwrap();
    assert!(face_enrollments(&f.pool, &scoped, TERM)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn attendance_navigation_migration_repairs_only_generated_default_names() {
    let f = fixture_at("attendance_navigation_names", 102).await;
    sqlx::query("INSERT INTO menu_groups(code,name) VALUES('academic_delivery','academic_delivery') ON CONFLICT(code) DO UPDATE SET name='academic_delivery'").execute(&f.pool).await.unwrap();
    apply_migrations_through(&f.pool, 103).await.unwrap();
    let name: String =
        sqlx::query_scalar("SELECT name FROM menu_groups WHERE code='academic_delivery'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(name, "งานจัดการเรียนการสอน");
    sqlx::query("UPDATE menu_groups SET name='งานสอนของโรงเรียน' WHERE code='academic_delivery'")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../migrations/103_attendance_navigation_labels.sql"
    ))
    .execute(&f.pool)
    .await
    .unwrap();
    let name: String =
        sqlx::query_scalar("SELECT name FROM menu_groups WHERE code='academic_delivery'")
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(name, "งานสอนของโรงเรียน");
}

fn session_context(s: &AttendanceSession) -> OpenAttendanceSession {
    OpenAttendanceSession {
        academic_term_id: s.academic_term_id,
        date: s.date,
        kind: s.kind,
        source_key: s.source_key.clone(),
    }
}

#[tokio::test]
async fn attendance_sparse_first_save_additions_corrections_and_stale_roster_are_atomic() {
    let f = fixture("attendance_sparse_first_save").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let mut source = seed(&f, AttendanceKind::Arrival, date);
    source.session.roster = source.students.clone();
    source.session = sessions::with_token(&f.pool, source.session).await.unwrap();
    let preview = sessions::detail_unchecked(&f.pool, source.session.clone())
        .await
        .unwrap();
    let mut stale_source = source.clone();
    stale_source.students.pop();
    stale_source.session.roster = stale_source.students.clone();
    assert!(matches!(
        sessions::save_with_seed(
            &f.pool,
            &f.actor,
            source.session.id,
            save_payload(&preview, STUDENT, AttendanceResult::Present),
            Some(stale_source)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let first = sessions::save_with_seed(
        &f.pool,
        &f.actor,
        source.session.id,
        save_payload(&preview, STUDENT, AttendanceResult::Present),
        Some(source.clone()),
    )
    .await
    .unwrap();
    assert_eq!(
        first
            .students
            .iter()
            .filter(|s| s.result == AttendanceResult::Unchecked && s.row_version == 0)
            .count(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert!(matches!(
        sessions::save_with_seed(
            &f.pool,
            &f.actor,
            source.session.id,
            save_payload(&preview, STUDENT, AttendanceResult::Present),
            Some(source)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let unchanged = sessions::save(
        &f.pool,
        &f.actor,
        first.session.id,
        save_payload(&first, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    assert_eq!(unchanged.session.row_version, first.session.row_version);
    let second = f.students[1].student_id;
    let mut addition = save_payload(&first, second, AttendanceResult::Leave);
    addition.reason.clear();
    let added = sessions::save(&f.pool, &f.actor, first.session.id, addition)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        2
    );
    let mut correction = save_payload(&added, STUDENT, AttendanceResult::Absent);
    correction.reason.clear();
    assert!(
        sessions::save(&f.pool, &f.actor, first.session.id, correction.clone())
            .await
            .is_err()
    );
    assert_eq!(
        sessions::get(&f.pool, first.session.id)
            .await
            .unwrap()
            .row_version,
        added.session.row_version
    );
    correction.reason = "ตรวจสอบกับครูผู้รับผิดชอบ".into();
    let corrected = sessions::save(&f.pool, &f.actor, first.session.id, correction)
        .await
        .unwrap();
    assert_eq!(
        corrected
            .students
            .iter()
            .find(|s| s.student_id == STUDENT)
            .unwrap()
            .result,
        AttendanceResult::Absent
    );
    assert!(sessions::save(
        &f.pool,
        &f.actor,
        first.session.id,
        save_payload(&corrected, STUDENT, AttendanceResult::Unchecked)
    )
    .await
    .is_err());
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM audit_logs WHERE entity_type='attendance' AND old_values IS NOT NULL AND new_values IS NOT NULL").fetch_one(&f.pool).await.unwrap(),3);
}

#[tokio::test]
async fn attendance_reset_is_scoped_reconciled_and_invalidates_old_requests() {
    let f = fixture("attendance_scoped_reset").await;
    let other = fixture("attendance_reset_other_tenant").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let other_round = sessions::ensure(
        &other.pool,
        &other.actor,
        seed(&other, AttendanceKind::Arrival, date),
    )
    .await
    .unwrap();
    let other_detail = sessions::detail(&other.pool, &other.actor, other_round.id)
        .await
        .unwrap();
    sessions::save(
        &other.pool,
        &other.actor,
        other_round.id,
        save_payload(&other_detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let seed = seed(&f, AttendanceKind::Arrival, date);
    let mut virtual_session = seed.session.clone();
    virtual_session.roster = seed.students.clone();
    let preview = sessions::detail_unchecked(
        &f.pool,
        sessions::with_token(&f.pool, virtual_session)
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let saved = sessions::save_with_seed(
        &f.pool,
        &f.actor,
        preview.session.id,
        save_payload(&preview, STUDENT, AttendanceResult::Present),
        Some(seed.clone()),
    )
    .await
    .unwrap();
    let pending = notifications::pending(&f.pool).await.unwrap();
    assert!(notifications::store(&f.pool, &pending[0]).await.unwrap());
    let owned_notice = pending[0].notification_id;
    let evidence = evidence_file(&f.pool, STUDENT).await;
    let unrelated_file = evidence_file(&f.pool, STUDENT).await;
    sqlx::query("UPDATE files SET purpose_code='profile_avatar' WHERE id=$1")
        .bind(unrelated_file)
        .execute(&f.pool)
        .await
        .unwrap();
    let unrelated_notice:Uuid=sqlx::query_scalar("INSERT INTO notifications(user_id,title,message,type) VALUES($1,'อื่น','fixture','info') RETURNING id").bind(TEACHER).fetch_one(&f.pool).await.unwrap();
    let before_users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    let mut request = ResetAttendanceModule {
        generation: impact.generation,
        token: impact.token.clone(),
        reason: "เริ่มใช้โมดูลใหม่".into(),
    };
    sqlx::query("UPDATE attendance_records SET note='changed' WHERE session_id=$1")
        .bind(saved.session.id)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        super::reset(&f.pool, TEACHER, request.clone()).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(reports::impact(&f.pool, TERM).await.unwrap().records, 1);
    let current = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    request.token = current.token;
    // A provider failure must roll back both the notice deletion and the reset.
    sqlx::raw_sql("CREATE FUNCTION reject_reset_file() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture_delete_failure'; END $$; CREATE TRIGGER reject_reset_file BEFORE UPDATE ON file_versions FOR EACH ROW EXECUTE FUNCTION reject_reset_file();")
        .execute(&f.pool).await.unwrap();
    assert!(matches!(
        super::reset(&f.pool, TEACHER, request.clone()).await,
        Err(AppError::ServiceUnavailable(_))
    ));
    assert_eq!(reports::impact(&f.pool, TERM).await.unwrap().records, 1);
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(owned_notice)
    .fetch_one(&f.pool)
    .await
    .unwrap());
    assert_eq!(
        sessions::module_generation(&f.pool).await.unwrap(),
        impact.generation
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id=$1")
            .bind(evidence)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "ready"
    );
    sqlx::raw_sql(
        "DROP TRIGGER reject_reset_file ON file_versions; DROP FUNCTION reject_reset_file();",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    let reset = super::reset(&f.pool, TEACHER, request.clone())
        .await
        .unwrap();
    assert_eq!(reset.prepared_deletes.len(), 1);
    let reset = reset.impact;
    assert!(reset.counts.iter().all(|r| r.count == 0));
    assert_eq!(reports::impact(&other.pool, TERM).await.unwrap().records, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id=$1")
            .bind(unrelated_file)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "ready"
    );
    assert_ne!(reset.generation, impact.generation);
    assert_eq!(
        reset.coverage_start,
        Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        before_users
    );
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(unrelated_notice)
    .fetch_one(&f.pool)
    .await
    .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT lifecycle_status FROM files WHERE id=$1")
            .bind(evidence)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "delete_requested"
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM file_operations WHERE file_id=$1 AND operation_type='delete_object')").bind(evidence).fetch_one(&f.pool).await.unwrap());
    assert!(!sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)"
    )
    .bind(owned_notice)
    .fetch_one(&f.pool)
    .await
    .unwrap());
    for notice in pending {
        assert!(!notifications::store(&f.pool, &notice).await.unwrap());
    }
    assert!(matches!(
        sessions::save_with_seed(
            &f.pool,
            &f.actor,
            preview.session.id,
            save_payload(&preview, STUDENT, AttendanceResult::Present),
            Some(seed)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        super::reset(&f.pool, TEACHER, request).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn attendance_migration_preserves_explicit_legacy_ceremony_facts_and_retries() {
    let pool = prepare_current_core_fixture("attendance_legacy_sparse_migration").await;
    apply_migrations_through(&pool, 103).await.unwrap();
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time,saved_by) VALUES($1,$2,'2025-10-09','flag','legacy','ceremony',$3,'08:00','08:15',$4)").bind(session).bind(TERM).bind(vec![TEACHER]).bind(TEACHER).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,class_number,result,origin,note,row_version) SELECT $1,$2,id,11,'leave','teacher','preserved',7 FROM student_academic_years WHERE student_id=$2 AND academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=$3)").bind(session).bind(STUDENT).bind(TERM).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 105).await.unwrap();
    apply_migrations_through(&pool, 105).await.unwrap();
    let migrated = sessions::get(&pool, session).await.unwrap();
    assert_eq!(migrated.kind, AttendanceKind::Special);
    assert_eq!(migrated.roster.len(), 1);
    assert_eq!(migrated.roster[0].class_number, Some(11));
    let fact: (String, String, String, i64) = sqlx::query_as(
        "SELECT result,origin,note,row_version FROM attendance_records WHERE session_id=$1",
    )
    .bind(session)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        fact,
        ("leave".into(), "teacher".into(), "preserved".into(), 7)
    );
    let report = reports::summaries(&pool, TERM, Some(STUDENT), None)
        .await
        .unwrap();
    assert_eq!(report.summaries.len(), 1);
    assert_eq!(report.summaries[0].category, "special");
    assert_eq!(report.summaries[0].leave, 1);
}

#[tokio::test]
async fn attendance_first_scan_and_virtual_cancellation_create_no_placeholder_results() {
    let f = fixture("attendance_virtual_scan_cancel").await;
    let date = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$1)")
        .bind(date + chrono::Duration::days(2))
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET start_date=$2,planned_end_date=$3 WHERE id=$1")
        .bind(TERM)
        .bind(date - chrono::Duration::days(1))
        .bind(date + chrono::Duration::days(1))
        .execute(&f.pool)
        .await
        .unwrap();
    let descriptor = FaceDescriptor {
        values: vec![0.01; 128],
    };
    faces::enroll(
        &f.pool,
        TEACHER,
        STUDENT,
        EnrollAttendanceFace {
            model: FACE_MODEL.into(),
            descriptors: vec![descriptor.clone(); 2],
            consent_confirmed: true,
        },
    )
    .await
    .unwrap();
    let device = specials::save_device(
        &f.pool,
        TEACHER,
        Uuid::new_v4(),
        SaveAttendanceDevice {
            name: "fixture".into(),
            enabled: true,
            operator_id: TEACHER,
        },
    )
    .await
    .unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let mut virtual_session = source.session.clone();
    virtual_session.roster = source.students.clone();
    let virtual_session = sessions::with_token(&f.pool, virtual_session)
        .await
        .unwrap();
    let scan = AttendanceScan {
        event_id: Uuid::new_v4(),
        device_id: device.id,
        session_id: virtual_session.id,
        session: session_context(&virtual_session),
        roster_token: virtual_session.roster_token.clone(),
        student_id: STUDENT,
        descriptor,
        evidence_file_id: evidence_file(&f.pool, STUDENT).await,
        captured_at: Utc::now(),
    };
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let cache_key = Uuid::new_v4().to_string();
    assert!(
        !faces::scan_with_seed_and_cached_gallery(
            &f.pool,
            &cache_key,
            &f.actor,
            scan.clone(),
            Some(source)
        )
        .await
        .unwrap()
        .duplicate
    );
    assert!(
        faces::scan_with_seed_and_cached_gallery(&f.pool, &cache_key, &f.actor, scan, None)
            .await
            .unwrap()
            .duplicate
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    let detail = sessions::detail(&f.pool, &f.actor, virtual_session.id)
        .await
        .unwrap();
    assert_eq!(
        detail
            .students
            .iter()
            .filter(|s| s.result == AttendanceResult::Unchecked && s.row_version == 0)
            .count(),
        1
    );
    let mut cancelled_seed = seed(&f, AttendanceKind::Lesson, date);
    cancelled_seed.session.roster = cancelled_seed.students.clone();
    let preview = sessions::with_token(&f.pool, cancelled_seed.session.clone())
        .await
        .unwrap();
    let cancelled = sessions::cancel_with_seed(
        &f.pool,
        &f.actor,
        preview.id,
        AttendanceCancellation {
            session: session_context(&preview),
            roster_token: preview.roster_token,
            row_version: 0,
            cancelled: true,
            reason: "งดรอบ".into(),
        },
        Some(cancelled_seed),
    )
    .await
    .unwrap();
    assert!(cancelled.session.cancelled);
    let restored = sessions::cancel(
        &f.pool,
        &f.actor,
        cancelled.session.id,
        AttendanceCancellation {
            session: session_context(&cancelled.session),
            roster_token: cancelled.session.roster_token,
            row_version: cancelled.session.row_version,
            cancelled: false,
            reason: "คืนรอบ".into(),
        },
    )
    .await
    .unwrap();
    assert!(!restored.session.cancelled);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn attendance_migration_refuses_roster_drift_and_fact_corruption() {
    for corrupt_fact in [false, true] {
        let f = fixture_at(
            if corrupt_fact {
                "attendance_fact_corruption"
            } else {
                "attendance_roster_drift"
            },
            103,
        )
        .await;
        let session = Uuid::new_v4();
        sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time) VALUES($1,$2,'2025-10-09','flag','legacy','ceremony',$3,'08:00','08:15')").bind(session).bind(TERM).bind(vec![TEACHER]).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,note) VALUES($1,$2,$3,'present','preserved')").bind(session).bind(STUDENT).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
        apply_migrations_through(&f.pool, 104).await.unwrap();
        let trigger = if corrupt_fact {
            "CREATE FUNCTION corrupt_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.note='corrupted'; RETURN NEW; END $$; CREATE TRIGGER corrupt_fixture BEFORE UPDATE ON attendance_records FOR EACH ROW EXECUTE FUNCTION corrupt_fixture();"
        } else {
            "CREATE FUNCTION corrupt_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.roster='[]'::jsonb; RETURN NEW; END $$; CREATE TRIGGER corrupt_fixture BEFORE UPDATE ON attendance_sessions FOR EACH ROW EXECUTE FUNCTION corrupt_fixture();"
        };
        sqlx::raw_sql(trigger).execute(&f.pool).await.unwrap();
        let error = apply_migrations_through(&f.pool, 105)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if corrupt_fact {
                "ATTENDANCE_FACT_PRESERVATION_FAILED"
            } else {
                "ATTENDANCE_ROSTER_RECONCILIATION_FAILED"
            }),
            "{error}"
        );
        assert_eq!(sqlx::query_as::<_,(String,String)>("SELECT s.kind,r.note FROM attendance_sessions s JOIN attendance_records r ON r.session_id=s.id WHERE s.id=$1").bind(session).fetch_one(&f.pool).await.unwrap(),("flag".into(),"preserved".into()));
        assert!(sqlx::query_scalar::<_, bool>(
            "SELECT to_regclass('attendance_calendar_snapshots') IS NOT NULL"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap());
    }
}

#[tokio::test]
async fn attendance_expand_refuses_oversized_legacy_roster_without_partial_schema() {
    let f = fixture_at("attendance_oversized_legacy_roster", 103).await;
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time) VALUES($1,$2,'2025-10-09','arrival','legacy','fixture',$3,'08:00','08:00')").bind(session).bind(TERM).bind(vec![TEACHER]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) SELECT uuid_generate_v5($1,'student-'||g),$1::text||'-'||g||'@example.invalid',$1::text||'-'||g,'fixture-not-a-login','fixture','student','student','active' FROM generate_series(1,2001) g")
        .bind(session).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT uuid_generate_v5($1,'year-'||g),uuid_generate_v5($1,'student-'||g),y.academic_year_id,y.grade_level_id,y.study_program_id,'active' FROM student_academic_years y CROSS JOIN generate_series(1,2001) g WHERE y.id=$2")
        .bind(session).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result) SELECT $1,uuid_generate_v5($1,'student-'||g),uuid_generate_v5($1,'year-'||g),'unchecked' FROM generate_series(1,2001) g")
        .bind(session).execute(&f.pool).await.unwrap();
    let error = apply_migrations_through(&f.pool, 104)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("ATTENDANCE_ROSTER_TOO_LARGE"), "{error}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        2001
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT to_regclass('attendance_module_state') IS NULL")
            .fetch_one(&f.pool)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn attendance_virtual_coverage_and_history_remain_read_only_and_archive_exactly() {
    let f = fixture("attendance_virtual_coverage").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let school = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_UPDATE_SCHOOL.into()],
    };
    let before = report(&f.pool, &school, report_query(Some(STUDENT), 1, 100))
        .await
        .unwrap();
    assert!(before
        .summaries
        .iter()
        .any(|r| r.category == "school" && r.unchecked > 0));
    assert!(before
        .summaries
        .iter()
        .all(|r| r.absent == 0 && r.present == 0 && r.expected == r.unchecked));
    let own = ActorContext {
        user_id: STUDENT,
        permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
    };
    let history = history(
        &f.pool,
        &own,
        AttendanceHistoryQuery {
            academic_term_id: TERM,
            student_id: STUDENT,
            start: date,
            end: date,
        },
    )
    .await
    .unwrap();
    assert!(!history.is_empty());
    assert!(history
        .iter()
        .all(|r| r.result == AttendanceResult::Unchecked));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let workspace = workspace(
        &f.pool,
        &school,
        AttendanceQuery {
            academic_term_id: TERM,
            date,
        },
    )
    .await
    .unwrap();
    let round = workspace
        .sessions
        .iter()
        .find(|r| r.kind == AttendanceKind::Arrival)
        .unwrap();
    let preview = open(&f.pool, &school, session_context(round))
        .await
        .unwrap();
    let checked = preview.students.first().unwrap().student_id;
    let saved = save(
        &f.pool,
        &school,
        preview.session.id,
        save_payload(&preview, checked, AttendanceResult::Present),
    )
    .await
    .unwrap();
    assert!(saved
        .students
        .iter()
        .any(|r| r.result == AttendanceResult::Present));
    let after = report(&f.pool, &school, report_query(Some(STUDENT), 1, 100))
        .await
        .unwrap();
    assert_eq!(
        before.summaries.iter().map(|r| r.expected).sum::<i64>(),
        after.summaries.iter().map(|r| r.expected).sum::<i64>()
    );
    // Moving the pupil after today's first check cannot count another school day.
    let new_room = Uuid::new_v4();
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) SELECT $1,'ATT-NEW','ห้องใหม่',academic_year_id,grade_level_id,'ATT-NEW',study_program_id,40,true FROM homerooms WHERE id=$2")
        .bind(new_room).bind(saved.session.homeroom_id.unwrap()).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE homeroom_placements SET status='ended',end_date=$1 WHERE student_academic_year_id=$2 AND status='current'")
        .bind(date).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number) SELECT gen_random_uuid(),id,academic_year_id,$1,$2,'current','regular',11 FROM student_academic_years WHERE id=$3")
        .bind(new_room).bind(date).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
    let moved = report(&f.pool, &school, report_query(Some(STUDENT), 1, 100))
        .await
        .unwrap();
    assert_eq!(
        after
            .summaries
            .iter()
            .filter(|r| r.category == "school")
            .map(|r| r.expected)
            .sum::<i64>(),
        moved
            .summaries
            .iter()
            .filter(|r| r.category == "school")
            .map(|r| r.expected)
            .sum::<i64>()
    );
    let epoch = sessions::module_generation(&f.pool).await.unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    school_attendance::services::lock_term(&mut tx, TERM, false, true)
        .await
        .unwrap();
    let expected = projection::expected(&mut *tx, TERM, Some((date, date)))
        .await
        .unwrap();
    notifications::queue_digest_in_transaction(
        tx,
        TERM,
        date,
        NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
        epoch,
        &expected,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    for n in notifications::pending(&f.pool).await.unwrap() {
        notifications::store(&f.pool, &n).await.unwrap();
        notifications::delivered(&f.pool, n.id).await.unwrap();
    }
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=planned_end_date WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    let before_archive = report(&f.pool, &school, report_query(Some(STUDENT), 1, 100))
        .await
        .unwrap();
    let impact = reports::impact(&f.pool, TERM).await.unwrap();
    purge(
        &f.pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: impact.records,
            reason: "archive coverage".into(),
        },
    )
    .await
    .unwrap();
    let archived = report(&f.pool, &school, report_query(Some(STUDENT), 1, 100))
        .await
        .unwrap();
    assert!(archived.archived);
    assert_eq!(before_archive.summaries, archived.summaries);
}

#[tokio::test]
async fn attendance_guarded_reads_digest_and_archive_work_with_one_connection() {
    let f = fixture("attendance_one_connection").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let options = (*f.pool.connect_options())
        .clone()
        .options([("search_path", format!("{schema},public"))]);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_with(options)
        .await
        .unwrap();
    let actor = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_READ_SCHOOL.into()],
    };
    // More callers than pool slots must queue without pinning another connection.
    let reads = (0..8).map(|_| report(&pool, &actor, report_query(Some(STUDENT), 1, 100)));
    for result in tokio::time::timeout(
        std::time::Duration::from_secs(15),
        futures::future::join_all(reads),
    )
    .await
    .unwrap()
    {
        assert!(!result.unwrap().summaries.is_empty());
    }
    let own = ActorContext {
        user_id: STUDENT,
        permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
    };
    assert!(!history(
        &pool,
        &own,
        AttendanceHistoryQuery {
            academic_term_id: TERM,
            student_id: STUDENT,
            start: date,
            end: date
        }
    )
    .await
    .unwrap()
    .is_empty());
    let mut tx = pool.begin().await.unwrap();
    school_attendance::services::lock_term(&mut tx, TERM, false, true)
        .await
        .unwrap();
    let epoch = sessions::module_generation_in_tx(&mut tx).await.unwrap();
    let expected = projection::expected(&mut *tx, TERM, Some((date, date)))
        .await
        .unwrap();
    notifications::queue_digest_in_transaction(
        tx,
        TERM,
        date,
        NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
        epoch,
        &expected,
    )
    .await
    .unwrap();
    for notice in notifications::pending(&pool).await.unwrap() {
        notifications::store(&pool, &notice).await.unwrap();
        notifications::delivered(&pool, notice.id).await.unwrap();
    }
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=planned_end_date WHERE id=$1")
        .bind(TERM)
        .execute(&pool)
        .await
        .unwrap();
    let result = purge(
        &pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: 0,
            reason: "archive fixture".into(),
        },
    )
    .await
    .unwrap();
    assert!(result.archived);
    assert!(
        report(&pool, &actor, report_query(Some(STUDENT), 1, 100))
            .await
            .unwrap()
            .archived
    );
}

#[tokio::test]
async fn attendance_migration_refuses_unchecked_capture_and_rolls_back_before_cleanup() {
    let f = fixture_at("attendance_unmappable_capture", 103).await;
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time) VALUES($1,$2,'2025-10-09','flag','legacy','ceremony',$3,'08:00','08:15')").bind(session).bind(TERM).bind(vec![TEACHER]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result) VALUES($1,$2,$3,'unchecked')").bind(session).bind(STUDENT).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
    let file = evidence_file(&f.pool, STUDENT).await;
    let device = specials::save_device(
        &f.pool,
        TEACHER,
        Uuid::new_v4(),
        SaveAttendanceDevice {
            name: "fixture".into(),
            enabled: true,
            operator_id: TEACHER,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES(gen_random_uuid(),$1,$2,$3,$4,now())").bind(session).bind(STUDENT).bind(device.id).bind(file).execute(&f.pool).await.unwrap();
    let failure = apply_migrations_through(&f.pool, 105)
        .await
        .unwrap_err()
        .to_string();
    assert!(failure.contains("ATTENDANCE_UNCHECKED_CAPTURE_UNMAPPABLE"));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT kind FROM attendance_sessions WHERE id=$1")
            .bind(session)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "flag"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_records WHERE result='unchecked'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_scan_events")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('attendance_calendar_snapshots') IS NOT NULL"
    )
    .fetch_one(&f.pool)
    .await
    .unwrap());
}

#[tokio::test]
async fn attendance_migration_refuses_unresolved_special_participants() {
    let f = fixture_at("attendance_unmappable_special", 103).await;
    let template = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_special_templates(id,academic_term_id,definition,created_by) VALUES($1,$2,'{}'::jsonb,$3)").bind(template).bind(TERM).bind(TEACHER).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_special_rounds(template_id,date) VALUES($1,'2025-10-09')")
        .bind(template)
        .execute(&f.pool)
        .await
        .unwrap();
    let failure = apply_migrations_through(&f.pool, 105)
        .await
        .unwrap_err()
        .to_string();
    assert!(failure.contains("ATTENDANCE_SPECIAL_PARTICIPANTS_UNMAPPABLE"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_special_rounds")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('attendance_calendar_snapshots') IS NOT NULL"
    )
    .fetch_one(&f.pool)
    .await
    .unwrap());
}

#[tokio::test]
async fn attendance_migration_refuses_ambiguous_archived_ceremony_scopes() {
    let f = fixture_at("attendance_unmappable_archive", 103).await;
    for scope in ["one", "two"] {
        sqlx::query("INSERT INTO attendance_term_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,leave,activity,unchecked,expected) VALUES($1,$2,'flag',$3,'legacy',1,0,0,0,0,0,1)").bind(TERM).bind(STUDENT).bind(scope).execute(&f.pool).await.unwrap();
    }
    let failure = apply_migrations_through(&f.pool, 105)
        .await
        .unwrap_err()
        .to_string();
    assert!(failure.contains("ATTENDANCE_ARCHIVED_CEREMONY_SCOPE_UNMAPPABLE"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_term_summaries WHERE category='flag'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        2
    );
}
