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
    let pool = prepare_current_core_fixture(name).await;
    apply_migrations_through(&pool, 101).await.unwrap();
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
async fn attendance_save_infers_only_on_nonempty_save_and_deduplicates_every_guardian() {
    let f = fixture("attendance_save_results").await;
    parents(&f).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Flag, date))
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
            .filter(|r| r.result == AttendanceResult::Absent)
            .count(),
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM attendance_notifications")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 4);
    let payload = save_payload(&saved, STUDENT, AttendanceResult::Present);
    let saved = sessions::save(&f.pool, &f.actor, s.id, payload)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_notifications")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        4
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
        7
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
            "SELECT count(*) FROM notifications WHERE title IN('ผลเช็คชื่อ','แก้ไขผลเช็คชื่อ')"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        7
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
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Flag, date))
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
    let holiday = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Flag, excluded))
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
            AttendanceKind::Flag,
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
        2
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
        student_id: STUDENT,
        descriptor,
        evidence_file_id: file,
        captured_at: Utc::now(),
    };
    let result = faces::scan(&f.pool, &f.actor, scan.clone()).await.unwrap();
    assert!(!result.duplicate);
    assert_eq!(result.result, AttendanceResult::Present);
    assert!(
        faces::scan(&f.pool, &f.actor, scan)
            .await
            .unwrap()
            .duplicate
    );
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
    let flag = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Flag, date))
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
        4
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
    // The second pupil was marked absent at flag ceremony, then arrived late.
    // Arrival still informs every guardian and proves school presence independently.
    let second = f.students[1].student_id;
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
            .find(|r| r.category == "flag")
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
            .find(|r| r.category == "flag")
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
    assert!(w.sessions.iter().any(|s| s.kind == AttendanceKind::Flag));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    super::super::runtime::snapshot_day(&f.pool, TERM, date)
        .await
        .unwrap();
    super::super::runtime::snapshot_day(&f.pool, TERM, date)
        .await
        .unwrap();
    let results: Vec<String> = sqlx::query_scalar("SELECT result FROM attendance_records")
        .fetch_all(&f.pool)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().all(|result| result == "unchecked"));
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
    let s = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Flag, date))
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
        2
    );
    let student = ActorContext {
        user_id: STUDENT,
        permissions: vec![codes::ATTENDANCE_READ_OWN.into()],
    };
    let own = report(
        &f.pool,
        &student,
        AttendanceReportQuery {
            academic_term_id: TERM,
            student_id: Some(STUDENT),
        },
    )
    .await
    .unwrap();
    assert!(own.summaries.iter().all(|s| s.student_id == STUDENT));
    assert!(!own.summaries.is_empty());
    assert!(report(
        &f.pool,
        &student,
        AttendanceReportQuery {
            academic_term_id: TERM,
            student_id: Some(f.students[1].student_id)
        }
    )
    .await
    .is_err());
    assert!(report(
        &f.pool,
        &student,
        AttendanceReportQuery {
            academic_term_id: TERM,
            student_id: None
        }
    )
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
async fn attendance_concurrent_arrival_and_flag_notices_do_not_notify_twice() {
    let f = fixture("attendance_concurrent_daily_notice").await;
    parents(&f).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let arrival = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Arrival, date))
        .await
        .unwrap();
    let flag = sessions::ensure(&f.pool, &f.actor, seed(&f, AttendanceKind::Flag, date))
        .await
        .unwrap();
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
