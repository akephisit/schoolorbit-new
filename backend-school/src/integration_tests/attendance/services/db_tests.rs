use super::*;
use crate::integration_tests::academic::{
    core::services_tests::prepare_core_fixture_through,
    cutover_test_support::apply_migrations_through,
};
use school_attendance::{
    rules::FACE_MODEL,
    services::{faces, notifications, reports},
};
pub(super) const TEACHER: Uuid = Uuid::from_u128(0x50000000000000000000000000000002);
pub(super) const STUDENT: Uuid = Uuid::from_u128(0x50000000000000000000000000000001);
pub(super) const TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000251);
pub(super) struct Fixture {
    pub(super) pool: PgPool,
    actor: ActorContext,
    pub(super) students: Vec<AttendanceRosterStudent>,
}
pub(super) async fn fixture(name: &str) -> Fixture {
    // Ordinary service tests use current provider contracts. Historical migration
    // tests opt into their exact predecessor through fixture_at instead.
    let latest = sqlx::migrate!("./migrations")
        .iter()
        .map(|migration| migration.version)
        .max()
        .expect("the active migration timeline must not be empty");
    fixture_at(name, latest).await
}
async fn fixture_at(name: &str, version: i64) -> Fixture {
    let pool = prepare_core_fixture_through(name, version).await;
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
        }],
    }
}
async fn legacy_present(f: &Fixture, session: &AttendanceSession) {
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin,updated_by) VALUES($1,$2,$3,'present','teacher',$4)")
        .bind(session.id).bind(STUDENT).bind(f.students[0].student_academic_year_id).bind(TEACHER).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE attendance_sessions SET saved_at=now(),saved_by=$2,row_version=row_version+1 WHERE id=$1")
        .bind(session.id).bind(TEACHER).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO audit_logs(user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,'update','attendance',$2,'[]',$3,'historical fixture')")
        .bind(TEACHER).bind(session.id).bind(sqlx::types::Json(serde_json::json!([[STUDENT,"present","",1]]))).execute(&f.pool).await.unwrap();
}
async fn drain_score_changes(pool: &PgPool) {
    loop {
        let mut tx = pool.begin().await.unwrap();
        let changes =
            school_attendance::services::score_changes::pending_on_connection(&mut tx, 100)
                .await
                .unwrap();
        if changes.is_empty() {
            tx.commit().await.unwrap();
            break;
        }
        for change in changes {
            assert!(
                school_attendance::services::score_changes::acknowledge(&mut tx, &change)
                    .await
                    .unwrap()
            );
        }
        tx.commit().await.unwrap();
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
async fn attendance_note_migration_preserves_results_and_audits_without_note_text() {
    let f = fixture_at("attendance_note_preservation", 105).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let saved_session = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    legacy_present(&f, &saved_session).await;
    sqlx::query("UPDATE attendance_records SET note='old person note' WHERE session_id=$1")
        .bind(saved_session.id)
        .execute(&f.pool)
        .await
        .unwrap();
    let old_audit = Uuid::new_v4();
    let missing_session = Uuid::new_v4();
    sqlx::query("INSERT INTO audit_logs(id,user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,$2,'update','attendance',$3,$4,$5,'correction reason')")
        .bind(old_audit).bind(TEACHER).bind(missing_session)
        .bind(sqlx::types::Json(serde_json::json!([[STUDENT,"absent","old note"]])))
        .bind(sqlx::types::Json(serde_json::json!([[STUDENT,"present","new note",12]])))
        .execute(&f.pool).await.unwrap();
    let unrelated = Uuid::new_v4();
    sqlx::query("INSERT INTO audit_logs(id,user_id,action,entity_type,old_values,new_values,description) VALUES($1,$2,'update','calendar',$3,$3,'unrelated')")
        .bind(unrelated).bind(TEACHER).bind(sqlx::types::Json(serde_json::json!({"note":"calendar note"})))
        .execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted,note) VALUES($1,$2,true,'calendar day note')")
        .bind(TERM).bind(date).execute(&f.pool).await.unwrap();
    let facts: sqlx::types::Json<serde_json::Value> = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(r)-'note' ORDER BY r.student_id) FROM attendance_records r",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let identity: sqlx::types::Json<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(a)-'old_values'-'new_values' FROM audit_logs a WHERE id=$1",
    )
    .bind(old_audit)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let unrelated_before: sqlx::types::Json<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM audit_logs a WHERE id=$1")
            .bind(unrelated)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    apply_migrations_through(&f.pool, 106).await.unwrap();
    apply_migrations_through(&f.pool, 106).await.unwrap();
    let after: sqlx::types::Json<serde_json::Value> = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(r) ORDER BY r.student_id) FROM attendance_records r",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(facts.0, after.0);
    let retained: sqlx::types::Json<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(a)-'old_values'-'new_values' FROM audit_logs a WHERE id=$1",
    )
    .bind(old_audit)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(identity.0, retained.0);
    let (old, new): (
        sqlx::types::Json<serde_json::Value>,
        sqlx::types::Json<serde_json::Value>,
    ) = sqlx::query_as("SELECT old_values,new_values FROM audit_logs WHERE id=$1")
        .bind(old_audit)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        old.0,
        serde_json::json!([{"studentId":STUDENT,"result":"absent"}])
    );
    assert_eq!(
        new.0,
        serde_json::json!([{"studentId":STUDENT,"result":"present","rowVersion":12}])
    );
    let unrelated_after: sqlx::types::Json<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM audit_logs a WHERE id=$1")
            .bind(unrelated)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(unrelated_before.0, unrelated_after.0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT note FROM attendance_days WHERE academic_term_id=$1 AND date=$2"
        )
        .bind(TERM)
        .bind(date)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        "calendar day note"
    );
    apply_migrations_through(&f.pool, 108).await.unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, saved_session.id)
        .await
        .unwrap();
    assert_eq!(detail.session.id, saved_session.id);
    assert!(sqlx::query("SELECT note FROM attendance_records")
        .execute(&f.pool)
        .await
        .is_err());
    let mut correction = save_payload(&detail, STUDENT, AttendanceResult::Late);
    correction.reason = "verified correction".into();
    let corrected = sessions::save(&f.pool, &f.actor, detail.session.id, correction)
        .await
        .unwrap();
    assert_eq!(
        corrected
            .students
            .iter()
            .find(|r| r.student_id == STUDENT)
            .unwrap()
            .result,
        AttendanceResult::Late
    );
    let latest:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT new_values FROM audit_logs WHERE entity_type='attendance' AND entity_id=$1 AND action='update' ORDER BY created_at DESC,id DESC LIMIT 1")
        .bind(detail.session.id).fetch_one(&f.pool).await.unwrap();
    assert!(latest
        .0
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item.get("note").is_none() && item.get("rowVersion").is_some()));
}

#[tokio::test]
async fn attendance_note_migration_refuses_ambiguous_snapshots_and_can_retry() {
    let f = fixture_at("attendance_note_refusal", 105).await;
    let source = seed(
        &f,
        AttendanceKind::Arrival,
        NaiveDate::from_ymd_opt(2025, 10, 9).unwrap(),
    );
    let s = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    legacy_present(&f, &s).await;
    sqlx::query(
        "UPDATE attendance_records SET note='must survive refused cleanup' WHERE session_id=$1",
    )
    .bind(s.id)
    .execute(&f.pool)
    .await
    .unwrap();
    let audit = Uuid::new_v4();
    sqlx::query("INSERT INTO audit_logs(id,user_id,action,entity_type,entity_id,old_values,new_values) VALUES($1,$2,'update','attendance',$3,'[]','[]')")
        .bind(audit).bind(TEACHER).bind(s.id).execute(&f.pool).await.unwrap();
    let cases = [
        serde_json::json!({"unexpected":"shape"}),
        serde_json::json!([[STUDENT, "present"]]),
        serde_json::json!([[STUDENT, "present", 42, 1]]),
        serde_json::json!([["invalid-id", "present", "note", 1]]),
        serde_json::json!([[STUDENT, "invalid-result", "note", 1]]),
        serde_json::json!([[STUDENT, "present", "note", 0]]),
        serde_json::json!([[STUDENT, "present", "note", 1, "extra"]]),
        serde_json::json!([{"studentId":STUDENT,"result":"present","rowVersion":1,"note":"unknown named shape"}]),
        serde_json::json!([{"studentId":STUDENT,"result":"present"}]),
    ];
    for bad in cases {
        sqlx::query("UPDATE audit_logs SET new_values=$2 WHERE id=$1")
            .bind(audit)
            .bind(sqlx::types::Json(&bad))
            .execute(&f.pool)
            .await
            .unwrap();
        let failure = apply_migrations_through(&f.pool, 106).await.unwrap_err();
        assert!(failure
            .to_string()
            .contains("ATTENDANCE_NOTE_AUDIT_UNMAPPABLE"));
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT note FROM attendance_records WHERE session_id=$1 AND student_id=$2"
            )
            .bind(s.id)
            .bind(STUDENT)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            "must survive refused cleanup"
        );
        let untouched: sqlx::types::Json<serde_json::Value> =
            sqlx::query_scalar("SELECT new_values FROM audit_logs WHERE id=$1")
                .bind(audit)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        assert_eq!(untouched.0, bad);
    }
    sqlx::query("UPDATE audit_logs SET new_values=NULL WHERE id=$1")
        .bind(audit)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(apply_migrations_through(&f.pool, 106)
        .await
        .unwrap_err()
        .to_string()
        .contains("ATTENDANCE_NOTE_AUDIT_UNMAPPABLE"));
    sqlx::query("UPDATE audit_logs SET new_values='[]' WHERE id=$1")
        .bind(audit)
        .execute(&f.pool)
        .await
        .unwrap();
    apply_migrations_through(&f.pool, 106).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_records WHERE result='present'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_logs WHERE id=$1")
            .bind(audit)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn attendance_note_migration_blocks_cleanup_when_audit_preservation_fails() {
    let f = fixture_at("attendance_note_reconcile", 105).await;
    let source = seed(
        &f,
        AttendanceKind::Arrival,
        NaiveDate::from_ymd_opt(2025, 10, 9).unwrap(),
    );
    let s = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    legacy_present(&f, &s).await;
    sqlx::query(
        "UPDATE attendance_records SET note='retained until reconciliation' WHERE session_id=$1",
    )
    .bind(s.id)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::raw_sql("CREATE FUNCTION corrupt_note_audit_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.description='corrupted'; RETURN NEW; END $$; CREATE TRIGGER corrupt_note_audit_fixture BEFORE UPDATE ON audit_logs FOR EACH ROW EXECUTE FUNCTION corrupt_note_audit_fixture();")
        .execute(&f.pool).await.unwrap();
    let failure = apply_migrations_through(&f.pool, 106).await.unwrap_err();
    assert!(failure
        .to_string()
        .contains("ATTENDANCE_NOTE_PRESERVATION_FAILED"));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT note FROM attendance_records WHERE session_id=$1")
            .bind(s.id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "retained until reconciliation"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_logs WHERE description='corrupted'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    sqlx::raw_sql("DROP TRIGGER corrupt_note_audit_fixture ON audit_logs; DROP FUNCTION corrupt_note_audit_fixture();").execute(&f.pool).await.unwrap();
    apply_migrations_through(&f.pool, 106).await.unwrap();
    assert!(sqlx::query("SELECT note FROM attendance_records")
        .execute(&f.pool)
        .await
        .is_err());
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
            auto_finalize_from: None,
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
            auto_finalize_from: None,
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
    drain_score_changes(&f.pool).await;
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
        scoring: None,
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
            auto_finalize_from: None,
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
            auto_finalize_from: None,
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
    drain_score_changes(&f.pool).await;
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
async fn attendance_scoped_arrival_timing_and_query_plan() {
    let f = fixture("attendance_arrival_timing").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let actor = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_READ_SCHOOL.into()],
    };
    let original = seeds(&f.pool, TERM, date)
        .await
        .unwrap()
        .into_iter()
        .find(|s| {
            s.session.kind == AttendanceKind::Arrival
                && s.students.iter().any(|r| r.student_id == STUDENT)
        })
        .unwrap();
    let original_room = original.session.homeroom_id.unwrap();
    let mut selected_room = original_room;
    for index in 0..40 {
        let room = Uuid::new_v4();
        sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) SELECT $1,$2,$2,academic_year_id,grade_level_id,$2,study_program_id,40,true FROM homerooms WHERE id=$3")
            .bind(room).bind(format!("PERF-{index}")).bind(original_room).execute(&f.pool).await.unwrap();
        sqlx::query(r#"WITH source AS MATERIALIZED (
            SELECT gen_random_uuid() student_id,gen_random_uuid() year_id,n number FROM generate_series(1,30) n
        ), added_users AS (
            INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status)
            SELECT student_id,student_id::text||'@example.invalid',student_id::text,'fixture-not-a-login','นักเรียนทดสอบ','ระบบ','student','active' FROM source RETURNING id
        ), added_years AS (
            INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status)
            SELECT s.year_id,s.student_id,y.academic_year_id,y.grade_level_id,y.study_program_id,'active'
            FROM source s JOIN added_users u ON u.id=s.student_id CROSS JOIN student_academic_years y WHERE y.id=$2
            RETURNING id,student_id,academic_year_id
        ) INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number)
          SELECT gen_random_uuid(),y.id,y.academic_year_id,$1,'2025-05-01','current','regular',s.number
          FROM added_years y JOIN source s ON s.student_id=y.student_id"#)
            .bind(room).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
        selected_room = room;
    }
    sqlx::query("ANALYZE homeroom_placements")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE student_academic_years")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE users").execute(&f.pool).await.unwrap();
    let context = OpenAttendanceSession {
        academic_term_id: TERM,
        date,
        kind: AttendanceKind::Arrival,
        source_key: selected_room.to_string(),
    };
    let mut baseline_times = vec![];
    let mut scoped_times = vec![];
    for sample in 0..6 {
        let started = std::time::Instant::now();
        let mut all = seeds(&f.pool, TERM, date).await.unwrap();
        all.extend(specials::seeds(&f.pool, TERM, date).await.unwrap());
        let source = all
            .into_iter()
            .find(|s| s.session.kind == context.kind && s.session.source_key == context.source_key)
            .unwrap();
        let baseline = preview_seed(&f.pool, &actor, source).await.unwrap();
        let baseline_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = std::time::Instant::now();
        let scoped = open(&f.pool, &actor, context.clone()).await.unwrap();
        let scoped_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            serde_json::to_value(&baseline).unwrap(),
            serde_json::to_value(&scoped).unwrap()
        );
        assert_eq!(scoped.students.len(), 30);
        if sample > 0 {
            baseline_times.push(baseline_ms);
            scoped_times.push(scoped_ms);
        }
    }
    baseline_times.sort_by(f64::total_cmp);
    scoped_times.sort_by(f64::total_cmp);
    println!(
        "attendance_preview_timing synthetic_rooms=40 synthetic_students=1200 samples=5 baseline_median_ms={:.3} scoped_median_ms={:.3}",
        baseline_times[2], scoped_times[2]
    );
    for (label, sql) in [
        ("all_students", core::ALL_STUDENTS_SQL),
        ("selected_room", core::ROOM_ROSTER_SQL),
    ] {
        let statement = format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {sql}");
        let query = sqlx::query_scalar::<_, sqlx::types::Json<serde_json::Value>>(
            sqlx::AssertSqlSafe(statement),
        )
        .bind(TERM)
        .bind(date);
        let plan = if label == "selected_room" {
            query.bind(selected_room).fetch_one(&f.pool).await.unwrap()
        } else {
            query.fetch_one(&f.pool).await.unwrap()
        };
        let item = &plan.0[0];
        println!(
            "attendance_roster_plan scope={label} execution_ms={} rows={} shared_hit_blocks={} shared_read_blocks={}",
            item["Execution Time"],
            item["Plan"]["Actual Rows"],
            item["Plan"]["Shared Hit Blocks"],
            item["Plan"]["Shared Read Blocks"]
        );
    }
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
}

#[tokio::test]
async fn attendance_scoped_arrival_preserves_roster_identity_and_read_only_scope() {
    let f = fixture("attendance_scoped_arrival").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let source = seeds(&f.pool, TERM, date)
        .await
        .unwrap()
        .into_iter()
        .find(|s| {
            s.session.kind == AttendanceKind::Arrival
                && s.students.iter().any(|s| s.student_id == STUDENT)
        })
        .unwrap();
    let actor = ActorContext {
        user_id: TEACHER,
        permissions: vec![
            codes::ATTENDANCE_READ_SCHOOL.into(),
            codes::ATTENDANCE_UPDATE_SCHOOL.into(),
        ],
    };
    let context = session_context(&source.session);
    let baseline = preview_seed(&f.pool, &actor, source.clone()).await.unwrap();
    let current = open(&f.pool, &actor, context.clone()).await.unwrap();
    assert_eq!(
        serde_json::to_value(&current).unwrap(),
        serde_json::to_value(&baseline).unwrap()
    );
    let unrelated = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ATTENDANCE_READ_ASSIGNED.into()],
    };
    assert!(matches!(
        open(&f.pool, &unrelated, context.clone()).await,
        Err(AppError::Forbidden(_))
    ));
    let mut missing = context.clone();
    missing.source_key = Uuid::new_v4().to_string();
    assert!(matches!(
        open(&f.pool, &actor, missing).await,
        Err(AppError::NotFound(_))
    ));
    let mut malformed = context.clone();
    malformed.source_key = "invalid-room".into();
    assert!(matches!(
        open(&f.pool, &actor, malformed).await,
        Err(AppError::NotFound(_))
    ));
    let (other_term,other_date):(Uuid,NaiveDate)=sqlx::query_as("SELECT id,start_date FROM academic_terms WHERE academic_year_id<>(SELECT academic_year_id FROM academic_terms WHERE id=$1) ORDER BY start_date,id LIMIT 1")
        .bind(TERM).fetch_one(&f.pool).await.unwrap();
    let mut cross_year = context.clone();
    cross_year.academic_term_id = other_term;
    cross_year.date = other_date;
    assert!(matches!(
        open(&f.pool, &actor, cross_year).await,
        Err(AppError::NotFound(_))
    ));
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
    // Opening arrival is independent of unrelated timetable availability.
    sqlx::query(
        "ALTER TABLE academic_timetable_blocks RENAME TO attendance_test_unavailable_blocks",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    let independent = open(&f.pool, &actor, context.clone()).await.unwrap();
    assert_eq!(
        serde_json::to_value(&independent).unwrap(),
        serde_json::to_value(&baseline).unwrap()
    );
    let saved = save(
        &f.pool,
        &actor,
        current.session.id,
        save_payload(&current, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    assert_eq!(
        saved
            .students
            .iter()
            .filter(|r| r.result == AttendanceResult::Present)
            .count(),
        1
    );
    assert!(saved
        .students
        .iter()
        .filter(|r| r.student_id != STUDENT)
        .all(|r| r.result == AttendanceResult::Unchecked));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn attendance_scoped_arrival_uses_latest_dated_placement_and_keeps_saved_roster() {
    let f = fixture("attendance_scoped_arrival_transfer").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let actor = ActorContext {
        user_id: TEACHER,
        permissions: vec![
            codes::ATTENDANCE_READ_SCHOOL.into(),
            codes::ATTENDANCE_UPDATE_SCHOOL.into(),
        ],
    };
    let original = seeds(&f.pool, TERM, date)
        .await
        .unwrap()
        .into_iter()
        .find(|s| {
            s.session.kind == AttendanceKind::Arrival
                && s.students.iter().any(|s| s.student_id == STUDENT)
        })
        .unwrap();
    let context = session_context(&original.session);
    let old_room = original.session.homeroom_id.unwrap();
    let preview = open(&f.pool, &actor, context.clone()).await.unwrap();
    let saved = save(
        &f.pool,
        &actor,
        preview.session.id,
        save_payload(&preview, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let original_number = saved
        .students
        .iter()
        .find(|r| r.student_id == STUDENT)
        .unwrap()
        .class_number;
    let new_room = Uuid::new_v4();
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) SELECT $1,'SCOPED-NEW','ห้องทดสอบย้าย',academic_year_id,grade_level_id,'SCOPED-NEW',study_program_id,40,true FROM homerooms WHERE id=$2")
        .bind(new_room).bind(old_room).execute(&f.pool).await.unwrap();
    let year = f.students[0].student_academic_year_id;
    sqlx::query("UPDATE homeroom_placements SET status='ended',end_date=$2 WHERE student_academic_year_id=$1 AND status='current'")
        .bind(year).bind(date).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number) SELECT gen_random_uuid(),id,academic_year_id,$1,$2,'current','regular',42 FROM student_academic_years WHERE id=$3")
        .bind(new_room).bind(date).bind(year).execute(&f.pool).await.unwrap();
    let before = core::students_in_room(&f.pool, TERM, date.pred_opt().unwrap(), old_room)
        .await
        .unwrap();
    assert!(before.iter().any(|r| r.student_id == STUDENT));
    let old = core::students_in_room(&f.pool, TERM, date, old_room)
        .await
        .unwrap();
    assert!(!old.iter().any(|r| r.student_id == STUDENT));
    let new = core::students_in_room(&f.pool, TERM, date, new_room)
        .await
        .unwrap();
    assert_eq!(
        new.iter()
            .find(|r| r.student_id == STUDENT)
            .unwrap()
            .class_number,
        Some(42)
    );
    let all = core::students(&f.pool, TERM, date).await.unwrap();
    assert_eq!(
        all.iter()
            .find(|r| r.student_id == STUDENT)
            .unwrap()
            .homeroom_id,
        Some(new_room)
    );
    let preserved = open(&f.pool, &actor, context).await.unwrap();
    let retained = preserved
        .students
        .iter()
        .find(|r| r.student_id == STUDENT)
        .unwrap();
    assert_eq!(retained.result, AttendanceResult::Present);
    assert_eq!(retained.class_number, original_number);
    assert_eq!(preserved.session.roster_token, saved.session.roster_token);
}

#[tokio::test]
async fn attendance_lesson_choices_use_published_rooms_and_respect_assigned_scope() {
    let f = fixture("attendance_lesson_choices").await;
    let (term, start): (Uuid, NaiveDate) = sqlx::query_as("SELECT v.academic_term_id,v.effective_from FROM academic_timetable_versions v JOIN academic_terms t ON t.id=v.academic_term_id WHERE v.status='published' AND t.status='active' AND EXISTS(SELECT 1 FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id AND g.is_active WHERE b.timetable_version_id=v.id AND b.is_active AND b.block_kind<>'structural') ORDER BY v.effective_from DESC,v.id LIMIT 1")
        .fetch_one(&f.pool).await.unwrap();
    let school = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_READ_SCHOOL.into()],
    };
    let mut found = None;
    for offset in 0..7 {
        let date = start + chrono::Duration::days(offset);
        let w = workspace(
            &f.pool,
            &school,
            AttendanceQuery {
                academic_term_id: term,
                date,
            },
        )
        .await
        .unwrap();
        if !w.lesson_choices.is_empty() {
            found = Some(w);
            break;
        }
    }
    let w = found.expect("published fixture must contain a teaching day");
    let rooms = core::rooms(&f.pool, term).await.unwrap();
    assert!(w.lesson_choices.iter().any(|c| !c.homerooms.is_empty()));
    for choice in &w.lesson_choices {
        assert!(w
            .sessions
            .iter()
            .any(|s| s.id == choice.session_id && s.kind == AttendanceKind::Lesson));
        assert!(!choice.period_name.is_empty());
        assert!(!choice.offering_name.is_empty());
        for room in &choice.homerooms {
            assert!(rooms.iter().any(|r| r.id == room.id && r.name == room.name));
        }
    }
    let unrelated = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ATTENDANCE_READ_ASSIGNED.into()],
    };
    let denied = workspace(
        &f.pool,
        &unrelated,
        AttendanceQuery {
            academic_term_id: term,
            date: w.date,
        },
    )
    .await
    .unwrap();
    assert!(denied.sessions.is_empty());
    assert!(denied.lesson_choices.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
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
    let b = save_payload(&detail, STUDENT, AttendanceResult::SickLeave);
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
        crate::integration_tests::policies::file_access_policy::FilePolicyAction::Read,
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
        crate::integration_tests::policies::file_access_policy::FilePolicyAction::Read,
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
    apply_migrations_through(&f.pool, 108).await.unwrap();
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
    sqlx::query("UPDATE users SET title='เด็กชาย',first_name='ทดสอบ',last_name='ระบบ' WHERE id=$1")
        .bind(STUDENT)
        .execute(&f.pool)
        .await
        .unwrap();
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let mut source = seed(&f, AttendanceKind::Arrival, date);
    source.session.roster = source.students.clone();
    source.session = sessions::with_token(&f.pool, source.session).await.unwrap();
    let preview = sessions::detail_unchecked(&f.pool, source.session.clone())
        .await
        .unwrap();
    assert_eq!(
        preview
            .students
            .iter()
            .find(|s| s.student_id == STUDENT)
            .unwrap()
            .display_name,
        "เด็กชายทดสอบ ระบบ"
    );
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
    let mut addition = save_payload(&first, second, AttendanceResult::SickLeave);
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
    sqlx::query("UPDATE attendance_records SET result='late' WHERE session_id=$1")
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
    let pool = prepare_core_fixture_through("attendance_legacy_sparse_migration", 103).await;
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,start_time,end_time,saved_by) VALUES($1,$2,'2025-10-09','flag','legacy','ceremony',$3,'08:00','08:15',$4)").bind(session).bind(TERM).bind(vec![TEACHER]).bind(TEACHER).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,class_number,result,origin,note,row_version) SELECT $1,$2,id,11,'present','teacher','preserved',7 FROM student_academic_years WHERE student_id=$2 AND academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=$3)").bind(session).bind(STUDENT).bind(TERM).execute(&pool).await.unwrap();
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
        ("present".into(), "teacher".into(), "preserved".into(), 7)
    );
    apply_migrations_through(&pool, 108).await.unwrap();
    let report = reports::summaries(&pool, TERM, Some(STUDENT), None)
        .await
        .unwrap();
    assert_eq!(report.summaries.len(), 1);
    assert_eq!(report.summaries[0].category, "special");
    assert_eq!(report.summaries[0].present, 1);
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
    drain_score_changes(&f.pool).await;
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

#[tokio::test]
async fn attendance_midnight_finalizer_preserves_checked_results_and_skips_excluded_rounds() {
    use chrono::TimeZone;
    use school_attendance::services::finalization as closing;
    let f = fixture("attendance_midnight_finalization").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1,coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let arrival = seed(&f, AttendanceKind::Arrival, date);
    let arrival_session = sessions::ensure(&f.pool, &f.actor, arrival.clone())
        .await
        .unwrap();
    let arrival_detail = sessions::detail(&f.pool, &f.actor, arrival_session.id)
        .await
        .unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        arrival_session.id,
        save_payload(&arrival_detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let lesson = seed(&f, AttendanceKind::Lesson, date);
    let mut excluded = seed(&f, AttendanceKind::Lesson, date);
    excluded.session.id = Uuid::now_v7();
    excluded.session.source_key = "excluded".into();
    excluded.session.count_override = Some(false);
    let mut cancelled = seed(&f, AttendanceKind::Lesson, date);
    cancelled.session.id = Uuid::now_v7();
    cancelled.session.source_key = "cancelled".into();
    let saved = sessions::ensure(&f.pool, &f.actor, cancelled.clone())
        .await
        .unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, saved.id).await.unwrap();
    sessions::cancel(
        &f.pool,
        &f.actor,
        saved.id,
        AttendanceCancellation {
            session: session_context(&detail.session),
            roster_token: detail.session.roster_token.clone(),
            row_version: detail.session.row_version,
            cancelled: true,
            reason: "round cancelled".into(),
        },
    )
    .await
    .unwrap();
    let seeds = vec![arrival, lesson, excluded, cancelled];
    let generation = sessions::module_generation(&f.pool).await.unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    assert!(closing::finalize_date_in_transaction(
        &mut tx,
        TERM,
        date,
        generation,
        &seeds,
        Utc.with_ymd_and_hms(2025, 10, 9, 16, 59, 59).unwrap()
    )
    .await
    .is_err());
    tx.rollback().await.unwrap();
    let now = Utc.with_ymd_and_hms(2025, 10, 9, 17, 0, 0).unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    let result =
        closing::finalize_date_in_transaction(&mut tx, TERM, date, generation, &seeds, now)
            .await
            .unwrap();
    assert_eq!(result.inserted_results, 3);
    tx.commit().await.unwrap();
    let facts: Vec<(String, String, Option<chrono::DateTime<Utc>>)> =
        sqlx::query_as("SELECT result,origin,processed_at FROM attendance_records ORDER BY result")
            .fetch_all(&f.pool)
            .await
            .unwrap();
    assert_eq!(facts.len(), 4);
    assert_eq!(
        facts
            .iter()
            .filter(|r| r.0 == "present" && r.1 == "teacher" && r.2.is_none())
            .count(),
        1
    );
    assert_eq!(
        facts
            .iter()
            .filter(|r| r.0 == "absent" && r.1 == "automatic" && r.2 == Some(now))
            .count(),
        3
    );
    let queued = school_attendance::services::score_changes::pending_on_connection(
        &mut *f.pool.acquire().await.unwrap(),
        100,
    )
    .await
    .unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    assert!(
        closing::finalize_date_in_transaction(
            &mut tx,
            TERM,
            date,
            generation,
            &seeds,
            now + chrono::Duration::days(2)
        )
        .await
        .unwrap()
        .already_completed
    );
    tx.commit().await.unwrap();
    let after = school_attendance::services::score_changes::pending_on_connection(
        &mut *f.pool.acquire().await.unwrap(),
        100,
    )
    .await
    .unwrap();
    assert_eq!(queued[0].row_version, after[0].row_version);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        4
    );
}

#[tokio::test]
async fn attendance_score_queue_revisions_survive_acknowledgement_and_corrections() {
    use school_attendance::services::score_changes as changes;
    let f = fixture("attendance_score_queue_revisions").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let saved = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, saved.id).await.unwrap();
    let detail = sessions::save(
        &f.pool,
        &f.actor,
        saved.id,
        save_payload(&detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let first = changes::pending_on_connection(&mut *f.pool.acquire().await.unwrap(), 100)
        .await
        .unwrap()
        .remove(0);
    let mut tx = f.pool.begin().await.unwrap();
    assert!(changes::acknowledge(&mut tx, &first).await.unwrap());
    tx.commit().await.unwrap();
    let detail = sessions::save(
        &f.pool,
        &f.actor,
        saved.id,
        save_payload(&detail, STUDENT, AttendanceResult::SickLeave),
    )
    .await
    .unwrap();
    let second = changes::pending_on_connection(&mut *f.pool.acquire().await.unwrap(), 100)
        .await
        .unwrap()
        .remove(0);
    assert!(second.row_version > first.row_version);
    let facts = changes::day_facts_on_connection(&mut *f.pool.acquire().await.unwrap(), TERM, date)
        .await
        .unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].result, AttendanceResult::SickLeave);
    assert_eq!(facts[0].source_key, "arrival");
    sessions::save(
        &f.pool,
        &f.actor,
        saved.id,
        save_payload(&detail, STUDENT, AttendanceResult::PersonalLeave),
    )
    .await
    .unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    assert!(!changes::acknowledge(&mut tx, &second).await.unwrap());
    tx.commit().await.unwrap();
    let third = changes::pending_on_connection(&mut *f.pool.acquire().await.unwrap(), 100)
        .await
        .unwrap()
        .remove(0);
    assert!(third.row_version > second.row_version);
    let facts = changes::day_facts_on_connection(&mut *f.pool.acquire().await.unwrap(), TERM, date)
        .await
        .unwrap();
    assert_eq!(facts[0].result, AttendanceResult::PersonalLeave);
}

#[tokio::test]
async fn attendance_status_migration_refuses_unclassified_leave_without_modifying_facts() {
    let f = fixture_at("attendance_status_migration_guard", 107).await;
    let source = seed(
        &f,
        AttendanceKind::Arrival,
        NaiveDate::from_ymd_opt(2025, 10, 9).unwrap(),
    );
    let saved = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'leave','teacher')")
        .bind(saved.id).bind(STUDENT).bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
    let failure = apply_migrations_through(&f.pool, 108)
        .await
        .unwrap_err()
        .to_string();
    assert!(failure.contains("ATTENDANCE_UNCLASSIFIED_LEAVE"));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT result FROM attendance_records")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "leave"
    );
    sqlx::query("DELETE FROM attendance_records")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_term_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,leave,activity,unchecked,expected) VALUES($1,$2,'school','','school',0,0,0,1,0,0,1)")
        .bind(TERM).bind(STUDENT).execute(&f.pool).await.unwrap();
    assert!(apply_migrations_through(&f.pool, 108)
        .await
        .unwrap_err()
        .to_string()
        .contains("ATTENDANCE_UNCLASSIFIED_LEAVE"));
    sqlx::query("DELETE FROM attendance_term_summaries")
        .execute(&f.pool)
        .await
        .unwrap();
    apply_migrations_through(&f.pool, 108).await.unwrap();
    assert!(!sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='attendance_term_summaries' AND column_name='leave')")
        .fetch_one(&f.pool).await.unwrap());
}

#[tokio::test]
async fn attendance_prefetched_score_change_reloads_reset_before_new_generation_facts() {
    use school_attendance::services::score_changes as changes;
    let f = fixture("attendance_prefetched_reset_change").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let old_session = sessions::ensure(&f.pool, &f.actor, source.clone())
        .await
        .unwrap();
    let old_detail = sessions::detail(&f.pool, &f.actor, old_session.id)
        .await
        .unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        old_session.id,
        save_payload(&old_detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let prefetched = changes::pending_on_connection(&mut *f.pool.acquire().await.unwrap(), 100)
        .await
        .unwrap()
        .remove(0);
    assert!(!prefetched.reset);
    let impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    super::reset(
        &f.pool,
        TEACHER,
        ResetAttendanceModule {
            generation: impact.generation,
            token: impact.token,
            reason: "reset between queue prefetch and locking".into(),
        },
    )
    .await
    .unwrap();
    let new_session = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    assert_eq!(old_session.id, new_session.id);
    let new_detail = sessions::detail(&f.pool, &f.actor, new_session.id)
        .await
        .unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        new_session.id,
        save_payload(&new_detail, STUDENT, AttendanceResult::Late),
    )
    .await
    .unwrap();
    let generation = sessions::module_generation(&f.pool).await.unwrap();
    assert_ne!(generation, prefetched.generation);
    let mut tx = f.pool.begin().await.unwrap();
    school_attendance::services::lock_term(&mut tx, TERM, false, false)
        .await
        .unwrap();
    let current = changes::current_on_connection(&mut tx, &prefetched)
        .await
        .unwrap()
        .unwrap();
    assert!(current.reset);
    assert_eq!(current.generation, prefetched.generation);
    assert!(current.row_version > prefetched.row_version);
    // A fresh source snapshot belongs only to the new generation. The old queue
    // record's reset flag instructs the adapter to deliver an empty old day.
    let new_facts = changes::day_facts_on_connection(&mut tx, TERM, date)
        .await
        .unwrap();
    assert_eq!(new_facts.len(), 1);
    assert_eq!(new_facts[0].result, AttendanceResult::Late);
    let queued = changes::pending_on_connection(&mut tx, 100).await.unwrap();
    assert_eq!(queued.len(), 2);
    let next = queued.iter().find(|c| c.generation == generation).unwrap();
    assert!(!next.reset);
    assert!(next.row_version > current.row_version);
    assert!(changes::acknowledge(&mut tx, &current).await.unwrap());
    assert!(changes::current_on_connection(&mut tx, &prefetched)
        .await
        .unwrap()
        .is_none());
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn attendance_score_queue_rejects_non_reset_stale_generation_without_reading_new_facts() {
    use school_attendance::services::score_changes as changes;
    let f = fixture("attendance_stale_generation_queue").await;
    let mut tx = f.pool.begin().await.unwrap();
    school_attendance::services::lock_term(&mut tx, TERM, false, false)
        .await
        .unwrap();
    let generation = Uuid::new_v4();
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    changes::enqueue_with_generation(&mut tx, generation, TERM, date, false)
        .await
        .unwrap();
    let prefetched = changes::pending_on_connection(&mut tx, 100)
        .await
        .unwrap()
        .remove(0);
    assert!(matches!(
        changes::current_on_connection(&mut tx, &prefetched).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        changes::pending_on_connection(&mut tx, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn attendance_behavior_adapter_reconciles_corrections_and_recreated_generation_once() {
    let f = fixture_at("attendance_behavior_adapter", 109).await;
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
    sqlx::query("UPDATE behavior_settings SET enabled=true")
        .execute(&f.pool)
        .await
        .unwrap();
    for (status, action, points) in [("present", "merit", 3), ("skipped", "deduction", 2)] {
        sqlx::query("INSERT INTO behavior_attendance_rules(kind,status,effective_from,action,points,created_by)VALUES('arrival',$1,$2,$3,$4,$5)").bind(status).bind(date).bind(action).bind(points).bind(TEACHER).execute(&f.pool).await.unwrap();
    }
    let mut source = seed(&f, AttendanceKind::Arrival, date);
    source.session.count_override = Some(true);
    let session = sessions::ensure(&f.pool, &f.actor, source.clone())
        .await
        .unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, session.id)
        .await
        .unwrap();
    let saved = sessions::save(
        &f.pool,
        &f.actor,
        session.id,
        save_payload(&detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    crate::adapters::behavior::run(&f.pool).await.unwrap();
    crate::adapters::behavior::run(&f.pool).await.unwrap();
    let totals: (i64,i64)=sqlx::query_as("SELECT COALESCE(sum(points)FILTER(WHERE side='merit'),0)::bigint,COALESCE(sum(points)FILTER(WHERE side='deduction'),0)::bigint FROM behavior_daily_totals WHERE student_id=$1").bind(STUDENT).fetch_one(&f.pool).await.unwrap();
    assert_eq!(totals, (3, 0));
    sessions::save(
        &f.pool,
        &f.actor,
        session.id,
        save_payload(&saved, STUDENT, AttendanceResult::Skipped),
    )
    .await
    .unwrap();
    crate::adapters::behavior::run(&f.pool).await.unwrap();
    let totals: (i64,i64)=sqlx::query_as("SELECT COALESCE(sum(points)FILTER(WHERE side='merit'),0)::bigint,COALESCE(sum(points)FILTER(WHERE side='deduction'),0)::bigint FROM behavior_daily_totals WHERE student_id=$1").bind(STUDENT).fetch_one(&f.pool).await.unwrap();
    assert_eq!(totals, (0, 2));
    let impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    let tx = f.pool.begin().await.unwrap();
    school_attendance::services::reset::reset_in_transaction(
        &f.pool,
        tx,
        TEACHER,
        ResetAttendanceModule {
            generation: impact.generation,
            token: impact.token,
            reason: "synthetic generation reset".into(),
        },
    )
    .await
    .unwrap();
    let session = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    let detail = sessions::detail(&f.pool, &f.actor, session.id)
        .await
        .unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        session.id,
        save_payload(&detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    crate::adapters::behavior::run(&f.pool).await.unwrap();
    crate::adapters::behavior::run(&f.pool).await.unwrap();
    let totals: (i64,i64)=sqlx::query_as("SELECT COALESCE(sum(points)FILTER(WHERE side='merit'),0)::bigint,COALESCE(sum(points)FILTER(WHERE side='deduction'),0)::bigint FROM behavior_daily_totals WHERE student_id=$1").bind(STUDENT).fetch_one(&f.pool).await.unwrap();
    assert_eq!(totals, (3, 0));
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id WHERE t.student_id=$1 AND e.cancelled_at IS NULL").bind(STUDENT).fetch_one(&f.pool).await.unwrap(),1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_score_changes")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn attendance_settings_preserve_unchanged_past_activation_and_reject_past_changes() {
    let f = fixture("attendance_settings_activation_noop").await;
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let yesterday = today.pred_opt().unwrap();
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1 WHERE singleton")
        .bind(yesterday)
        .execute(&f.pool)
        .await
        .unwrap();
    let before = settings::get(&f.pool, TERM).await.unwrap();
    let mut configuration = before.configuration;
    configuration.weekdays = vec![1, 3, 5];
    let saved = settings::save(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            configuration: configuration.clone(),
            row_version: before.row_version,
            auto_finalize_from: Some(yesterday),
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.auto_finalize_from, yesterday);
    assert_eq!(saved.configuration.weekdays, vec![1, 3, 5]);
    assert_eq!(saved.row_version, 1);
    let two_days_ago = yesterday.pred_opt().unwrap();
    assert!(matches!(
        settings::save(
            &f.pool,
            TEACHER,
            TERM,
            SaveAttendanceSettings {
                configuration: configuration.clone(),
                row_version: saved.row_version,
                auto_finalize_from: Some(two_days_ago),
            }
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    assert_eq!(
        settings::get(&f.pool, TERM)
            .await
            .unwrap()
            .auto_finalize_from,
        yesterday
    );
    // Even a forward move must not enable historical backfill: the unchanged
    // value is the only accepted activation date before today's boundary.
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1 WHERE singleton")
        .bind(two_days_ago)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        settings::save(
            &f.pool,
            TEACHER,
            TERM,
            SaveAttendanceSettings {
                configuration: configuration.clone(),
                row_version: saved.row_version,
                auto_finalize_from: Some(yesterday),
            }
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    let next = settings::save(
        &f.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            configuration,
            row_version: saved.row_version,
            auto_finalize_from: Some(today),
        },
    )
    .await
    .unwrap();
    assert_eq!(next.auto_finalize_from, today);
    assert_eq!(next.row_version, 2);
}

#[tokio::test]
async fn attendance_lifecycle_delivery_gate_waits_for_finalization_scores_and_cleanup() {
    use chrono::TimeZone;
    use school_academic_lifecycle::ports::ExternalLifecyclePort;
    use school_behavior::models::{BehaviorCleanupScope, BehaviorSide, BehaviorSource};
    let f = fixture_at("attendance_lifecycle_delivery_gate", 109).await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET coverage_start=$1,auto_finalize_from=$1 WHERE singleton").bind(date).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration,updated_by)VALUES($1,$2,$3)")
        .bind(TERM)
        .bind(sqlx::types::Json(AttendanceConfiguration::default()))
        .bind(TEACHER)
        .execute(&f.pool)
        .await
        .unwrap();
    let provider = crate::adapters::academic_lifecycle::ApplicationLifecycleProviders;
    let mut tx = f.pool.begin().await.unwrap();
    assert!(matches!(
        provider
            .require_attendance_delivery_complete(&mut tx, TERM, date)
            .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let generation = school_attendance::services::sessions::module_generation(&f.pool)
        .await
        .unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let mut tx = f.pool.begin().await.unwrap();
    school_attendance::services::finalization::finalize_date_in_transaction(
        &mut tx,
        TERM,
        date,
        generation,
        &[source],
        Utc.with_ymd_and_hms(2025, 10, 9, 17, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    assert!(matches!(
        provider
            .require_attendance_delivery_complete(&mut tx, TERM, date)
            .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    crate::adapters::behavior::run(&f.pool).await.unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    provider
        .require_attendance_delivery_complete(&mut tx, TERM, date)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let scope = BehaviorCleanupScope {
        academic_year_id: None,
        academic_term_id: Some(TERM),
        start: Some(date),
        end: Some(date),
        sides: vec![BehaviorSide::Merit, BehaviorSide::Deduction],
        sources: vec![BehaviorSource::Manual],
    };
    sqlx::query("INSERT INTO behavior_cleanup_jobs(scope,mode,reset,reason,students,entries_expected,merit,deduction)VALUES($1,'details_only',false,'synthetic pending cleanup',0,0,0,0)").bind(sqlx::types::Json(scope)).execute(&f.pool).await.unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    assert!(matches!(
        provider
            .require_attendance_delivery_complete(&mut tx, TERM, date)
            .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    assert!(
        !school_behavior::services::cleanup::run_cleanup_jobs(&f.pool, 1)
            .await
            .unwrap()
    );
    let mut tx = f.pool.begin().await.unwrap();
    provider
        .require_attendance_delivery_complete(&mut tx, TERM, date)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn attendance_report_snapshot_allows_reports_and_first_save_without_mixed_coverage() {
    let f = fixture("attendance_report_save_overlap").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let mut preview_session = source.session.clone();
    preview_session.roster = source.students.clone();
    let preview = sessions::detail_unchecked(
        &f.pool,
        sessions::with_token(&f.pool, preview_session)
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let actor = ActorContext {
        user_id: TEACHER,
        permissions: vec![codes::ATTENDANCE_READ_SCHOOL.into()],
    };
    let query = report_query(Some(STUDENT), 1, 100);
    let before = report(&f.pool, &actor, query.clone()).await.unwrap();
    let mut snapshot = school_attendance::services::begin_report_snapshot(&f.pool, TERM)
        .await
        .unwrap();
    let mode: (String, String) = sqlx::query_as(
        "SELECT current_setting('transaction_isolation'),current_setting('transaction_read_only')",
    )
    .fetch_one(&mut *snapshot)
    .await
    .unwrap();
    assert_eq!(mode, ("repeatable read".into(), "on".into()));
    let expected = projection::expected(&mut snapshot, TERM, None)
        .await
        .unwrap();
    let concurrent = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        report(&f.pool, &actor, query.clone()),
    )
    .await
    .expect("reports must share the term guard")
    .unwrap();
    assert_eq!(concurrent.summaries, before.summaries);
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        sessions::save_with_seed(
            &f.pool,
            &f.actor,
            source.session.id,
            save_payload(&preview, STUDENT, AttendanceResult::Present),
            Some(source),
        ),
    )
    .await
    .expect("first save must commit while a report snapshot is open")
    .unwrap();
    let frozen = reports::page_with_expected_on_connection(&mut snapshot, &query, None, &expected)
        .await
        .unwrap();
    assert_eq!(
        frozen.summaries, before.summaries,
        "virtual coverage and saved facts must use one snapshot"
    );
    snapshot.rollback().await.unwrap();
    let after = report(&f.pool, &actor, query).await.unwrap();
    let present =
        |report: &AttendanceReport| report.summaries.iter().map(|row| row.present).sum::<i64>();
    assert_eq!(present(&after), present(&before) + 1);
    assert_eq!(
        after.summaries.iter().map(|row| row.expected).sum::<i64>(),
        before.summaries.iter().map(|row| row.expected).sum::<i64>()
    );
}

async fn attendance_wait_for_blocker(pool: &PgPool, blocker: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker)
            .fetch_one(pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("destructive operation must wait on report guard");
}

#[tokio::test]
async fn attendance_report_snapshot_blocks_reset_purge_and_lifecycle_until_release() {
    let f = fixture("attendance_report_destructive_guards").await;
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=planned_end_date WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    let reset_impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    let mut snapshot = school_attendance::services::begin_report_snapshot(&f.pool, TERM)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *snapshot)
        .await
        .unwrap();
    let pool = f.pool.clone();
    let lifecycle = tokio::spawn(async move {
        let mut tx = pool.begin().await.unwrap();
        school_academic_core::services::lifecycle_guard::lock_transition(&mut tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
    });
    attendance_wait_for_blocker(&f.pool, pid).await;
    snapshot.rollback().await.unwrap();
    lifecycle.await.unwrap();

    let mut snapshot = school_attendance::services::begin_report_snapshot(&f.pool, TERM)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *snapshot)
        .await
        .unwrap();
    let pool = f.pool.clone();
    let purge_worker = tokio::spawn(async move {
        purge(
            &pool,
            TEACHER,
            TERM,
            PurgeAttendanceTerm {
                expected_records: 0,
                reason: "concurrent archive".into(),
            },
        )
        .await
    });
    attendance_wait_for_blocker(&f.pool, pid).await;
    assert!(
        !settings::get_on_connection(&mut snapshot, TERM)
            .await
            .unwrap()
            .archived
    );
    snapshot.rollback().await.unwrap();
    assert!(purge_worker.await.unwrap().unwrap().archived);
    let archived = report(&f.pool, &f.actor, report_query(None, 1, 100))
        .await
        .unwrap();
    assert!(archived.archived);

    // Purge changed the reset preview: its stale token must still be rejected.
    assert!(matches!(
        super::reset(
            &f.pool,
            TEACHER,
            ResetAttendanceModule {
                generation: reset_impact.generation,
                token: reset_impact.token,
                reason: "stale report reset".into(),
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    let mut snapshot = school_attendance::services::begin_report_snapshot(&f.pool, TERM)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *snapshot)
        .await
        .unwrap();
    let pool = f.pool.clone();
    let reset_worker = tokio::spawn(async move {
        super::reset(
            &pool,
            TEACHER,
            ResetAttendanceModule {
                generation: impact.generation,
                token: impact.token,
                reason: "concurrent reset".into(),
            },
        )
        .await
    });
    attendance_wait_for_blocker(&f.pool, pid).await;
    assert!(
        settings::get_on_connection(&mut snapshot, TERM)
            .await
            .unwrap()
            .archived
    );
    snapshot.rollback().await.unwrap();
    reset_worker.await.unwrap().unwrap();
    let after = report(&f.pool, &f.actor, report_query(None, 1, 100))
        .await
        .unwrap();
    assert!(!after.archived);
    assert_eq!(after.total, 0);
}

#[tokio::test]
async fn attendance_report_snapshot_queued_behind_reset_keeps_transaction_start_state() {
    let f = fixture("attendance_report_queued_reset").await;
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=planned_end_date WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    purge(
        &f.pool,
        TEACHER,
        TERM,
        PurgeAttendanceTerm {
            expected_records: 0,
            reason: "archive before reset".into(),
        },
    )
    .await
    .unwrap();
    let impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    let mut reset_tx = f.pool.begin().await.unwrap();
    school_attendance::services::lock_module_exclusive(&mut reset_tx)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *reset_tx)
        .await
        .unwrap();
    let pool = f.pool.clone();
    let reader = tokio::spawn(async move {
        report(
            &pool,
            &ActorContext {
                user_id: TEACHER,
                permissions: vec![codes::ATTENDANCE_READ_SCHOOL.into()],
            },
            report_query(None, 1, 100),
        )
        .await
    });
    attendance_wait_for_blocker(&f.pool, pid).await;
    school_attendance::services::reset::reset_in_transaction(
        &f.pool,
        reset_tx,
        TEACHER,
        ResetAttendanceModule {
            generation: impact.generation,
            token: impact.token,
            reason: "reset queued report".into(),
        },
    )
    .await
    .unwrap();
    let before_reset = reader.await.unwrap().unwrap();
    assert!(
        before_reset.archived,
        "a waiting report must retain the snapshot established before its module-lock wait"
    );
    let after_reset = report(&f.pool, &f.actor, report_query(None, 1, 100))
        .await
        .unwrap();
    assert!(!after_reset.archived);
    assert_eq!(after_reset.total, 0);
}

#[tokio::test]
async fn attendance_history_snapshot_allows_parallel_readers_and_first_save_without_mixed_facts() {
    let mut f = fixture("attendance_history_save_overlap").await;
    // Keep a blocker, two readers and an independent observer/save connection.
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let options = (*f.pool.connect_options())
        .clone()
        .options([("search_path", format!("{schema},public"))]);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    f.pool.close().await;
    f.pool = pool;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let room = core::students(&f.pool, TERM, date)
        .await
        .unwrap()
        .into_iter()
        .find(|student| student.student_id == STUDENT)
        .unwrap()
        .homeroom_id
        .unwrap();
    let context = OpenAttendanceSession {
        academic_term_id: TERM,
        date,
        kind: AttendanceKind::Arrival,
        source_key: room.to_string(),
    };
    let preview = open(&f.pool, &f.actor, context.clone()).await.unwrap();
    let mut blocker = f.pool.begin().await.unwrap();
    // Arrival saves do not use the timetable provider. Pause history after it has
    // read the existing rounds, so a first save can commit between its queries.
    sqlx::query("LOCK TABLE academic_timetable_versions IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut readers = Vec::new();
    for _ in 0..2 {
        let pool = f.pool.clone();
        let actor = f.actor.clone();
        readers.push(tokio::spawn(async move {
            history(
                &pool,
                &actor,
                AttendanceHistoryQuery {
                    academic_term_id: TERM,
                    student_id: STUDENT,
                    start: date,
                    end: date,
                },
            )
            .await
        }));
    }
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid))",
            )
            .bind(blocker_pid)
            .fetch_one(&f.pool)
            .await
            .unwrap();
            if waiting == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both history readers must reach the provider concurrently");
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        save(
            &f.pool,
            &f.actor,
            preview.session.id,
            save_payload(&preview, STUDENT, AttendanceResult::Present),
        ),
    )
    .await
    .expect("first arrival save must not wait for history readers")
    .unwrap();
    blocker.rollback().await.unwrap();
    for reader in readers {
        let rows = reader.await.unwrap().unwrap();
        let selected: Vec<_> = rows
            .iter()
            .filter(|row| row.session_id == preview.session.id)
            .collect();
        assert_eq!(
            selected.len(),
            1,
            "one snapshot must not mix virtual and saved copies"
        );
        assert_eq!(selected[0].result, AttendanceResult::Unchecked);
    }
    let current = history(
        &f.pool,
        &f.actor,
        AttendanceHistoryQuery {
            academic_term_id: TERM,
            student_id: STUDENT,
            start: date,
            end: date,
        },
    )
    .await
    .unwrap();
    let selected: Vec<_> = current
        .iter()
        .filter(|row| row.session_id == preview.session.id)
        .collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].result, AttendanceResult::Present);
}

#[tokio::test]
async fn attendance_history_snapshot_queued_behind_reset_keeps_transaction_start_facts() {
    let f = fixture("attendance_history_queued_reset").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let source = seed(&f, AttendanceKind::Arrival, date);
    let session = sessions::ensure(&f.pool, &f.actor, source).await.unwrap();
    let preview = sessions::detail(&f.pool, &f.actor, session.id)
        .await
        .unwrap();
    sessions::save(
        &f.pool,
        &f.actor,
        session.id,
        save_payload(&preview, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let impact = school_attendance::services::reset::impact(&f.pool)
        .await
        .unwrap();
    let mut reset_tx = f.pool.begin().await.unwrap();
    school_attendance::services::lock_module_exclusive(&mut reset_tx)
        .await
        .unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *reset_tx)
        .await
        .unwrap();
    let pool = f.pool.clone();
    let actor = f.actor.clone();
    let reader = tokio::spawn(async move {
        history(
            &pool,
            &actor,
            AttendanceHistoryQuery {
                academic_term_id: TERM,
                student_id: STUDENT,
                start: date,
                end: date,
            },
        )
        .await
    });
    attendance_wait_for_blocker(&f.pool, blocker).await;
    school_attendance::services::reset::reset_in_transaction(
        &f.pool,
        reset_tx,
        TEACHER,
        ResetAttendanceModule {
            generation: impact.generation,
            token: impact.token,
            reason: "reset waiting history".into(),
        },
    )
    .await
    .unwrap();
    let before = reader.await.unwrap().unwrap();
    assert!(before
        .iter()
        .any(|row| row.session_id == session.id && row.result == AttendanceResult::Present));
    let after = history(
        &f.pool,
        &f.actor,
        AttendanceHistoryQuery {
            academic_term_id: TERM,
            student_id: STUDENT,
            start: date,
            end: date,
        },
    )
    .await
    .unwrap();
    assert!(after.is_empty());
}

async fn attendance_queue_rows(f: &Fixture, count: i64) {
    sqlx::query("INSERT INTO attendance_notifications(academic_term_id,recipient_id,student_id,dedup_key,title,message,link) SELECT $1,$2,$2,'queue-fixture-'||n,'fixture','fixture','/student/attendance' FROM generate_series(1,$3) n")
        .bind(TERM).bind(STUDENT).bind(count).execute(&f.pool).await.unwrap();
}

#[tokio::test]
async fn attendance_notification_claims_are_bounded_disjoint_and_future_leases_are_not_due() {
    let f = fixture("attendance_bounded_notice_claims").await;
    attendance_queue_rows(&f, notifications::DELIVERY_BATCH_SIZE + 3).await;
    let (first, second) = tokio::join!(
        notifications::pending(&f.pool),
        notifications::pending(&f.pool)
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert!(!first.is_empty() && !second.is_empty());
    assert!(first.len() <= 32 && second.len() <= 32);
    assert_eq!(first.len() + second.len(), 35);
    let ids: std::collections::HashSet<_> = first.iter().chain(&second).map(|n| n.id).collect();
    assert_eq!(ids.len(), 35);
    assert!(notifications::has_pending(&f.pool).await.unwrap());
    assert!(
        !notifications::has_due(&f.pool).await.unwrap(),
        "leased retries must not wake a tight loop"
    );
    assert!(notifications::pending(&f.pool).await.unwrap().is_empty());
    let notice = &first[0];
    assert!(notifications::store(&f.pool, notice).await.unwrap());
    assert!(notifications::store(&f.pool, notice).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications WHERE id=$1")
            .bind(notice.notification_id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query(
        "UPDATE attendance_notifications SET next_attempt_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(notice.id)
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(notifications::has_due(&f.pool).await.unwrap());
    let retry = notifications::pending(&f.pool).await.unwrap();
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].notification_id, notice.notification_id);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT attempts FROM attendance_notifications WHERE id=$1")
            .bind(notice.id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn attendance_notification_invalid_recipients_consume_only_one_due_batch() {
    let f = fixture("attendance_bounded_invalid_notices").await;
    attendance_queue_rows(&f, notifications::DELIVERY_BATCH_SIZE + 3).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(STUDENT)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(notifications::pending(&f.pool).await.unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_notifications WHERE published_at IS NOT NULL"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        32
    );
    assert!(notifications::has_due(&f.pool).await.unwrap());
    assert!(notifications::pending(&f.pool).await.unwrap().is_empty());
    assert!(!notifications::has_pending(&f.pool).await.unwrap());
}

#[tokio::test]
async fn attendance_notification_due_backlog_revisits_other_tenants_before_next_batch() {
    use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
    use std::{collections::HashMap, sync::Arc};
    struct AbortOnDrop(tokio::task::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    struct Behavior;
    #[async_trait::async_trait]
    impl school_attendance::runtime::AttendanceBehaviorPort for Behavior {
        async fn run(&self, _pool: &PgPool) -> Result<(), AppError> {
            Ok(())
        }
    }
    static BEHAVIOR: Behavior = Behavior;
    let first = fixture("attendance_fair_queue_first").await;
    let second = fixture("attendance_fair_queue_second").await;
    let lock_key = i64::from_le_bytes(*Uuid::new_v4().as_bytes().first_chunk::<8>().unwrap());
    let mut blocker = first.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut directory = HashMap::new();
    let mut active = Vec::new();
    for (f, name) in [(&first, "queue-first"), (&second, "queue-second")] {
        attendance_queue_rows(f, 97).await;
        // Two complete passes are permitted; the third must pause. A worker that
        // drains one tenant before visiting the other leaves that other at zero.
        sqlx::raw_sql(&format!("CREATE FUNCTION block_later_notice_batch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.published_at IS NOT NULL AND OLD.published_at IS NULL AND (SELECT count(*) FROM attendance_notifications WHERE published_at IS NOT NULL)>=64 THEN PERFORM pg_advisory_xact_lock({lock_key}); END IF; RETURN NEW; END $$; CREATE TRIGGER block_later_notice_batch BEFORE UPDATE ON attendance_notifications FOR EACH ROW EXECUTE FUNCTION block_later_notice_batch();"))
            .execute(&f.pool).await.unwrap();
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&f.pool)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let subdomain = format!("{name}-{}", Uuid::new_v4().simple());
        directory.insert(subdomain.clone(), serde_json::json!({"id":Uuid::new_v4(),"name":"fixture","db_connection_string":url.as_str()}));
        active.push(serde_json::json!({"subdomain":subdomain,"db_connection_string":url.as_str()}));
    }
    let directory = Arc::new(directory);
    let app = Router::new()
        .route(
            "/internal/schools",
            get(move || {
                let schools = active.clone();
                async move { Json(serde_json::json!({"schools":schools})) }
            }),
        )
        .route(
            "/internal/schools/{tenant}",
            get(move |Path(tenant): Path<String>| {
                let directory = directory.clone();
                async move {
                    directory
                        .get(&tenant)
                        .cloned()
                        .map(Json)
                        .ok_or(StatusCode::NOT_FOUND)
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut _server = AbortOnDrop(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let (channel, _) = tokio::sync::broadcast::channel(100);
    let state = school_attendance::runtime::AttendanceRuntimeState {
        admin_client: Arc::new(school_tenancy::AdminClient::new(
            format!("http://{address}"),
            "fixture-internal".into(),
            school_tenancy::AdminClientConfig::from_env().unwrap(),
        )),
        pool_manager: Arc::new(school_tenancy::PoolManager::new()),
        notification_channel: channel,
        behavior: &BEHAVIOR,
    };
    let mut _worker = AbortOnDrop(tokio::spawn(school_attendance::runtime::start(state)));
    // Startup contributes at most one Notify permit and the initial interval tick.
    // Reaching the third batch promptly also proves due-work self-waking.
    attendance_wait_for_blocker(&first.pool, blocker_pid).await;
    for f in [&first, &second] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM attendance_notifications WHERE published_at IS NOT NULL"
            )
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            64,
            "each tenant must receive one bounded batch per pass"
        );
    }
    blocker.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if !notifications::has_pending(&first.pool).await.unwrap()
                && !notifications::has_pending(&second.pool).await.unwrap()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("due backlog must drain without waiting for the 15-second timer");
    _worker.0.abort();
    let _ = (&mut _worker.0).await;
    _server.0.abort();
    let _ = (&mut _server.0).await;
    first.pool.close().await;
    second.pool.close().await;
}
