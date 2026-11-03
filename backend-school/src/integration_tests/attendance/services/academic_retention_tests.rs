use super::*;
use chrono::Duration;
use school_academic_lifecycle::{
    models::{AcademicRetentionBoundary, UpdateAcademicRetentionPolicy},
    services::retention,
};

const RETENTION_PERMISSIONS: [&str; 4] = [
    codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
    codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
    codes::ATTENDANCE_DELETE_SCHOOL,
    codes::BEHAVIOR_CLEANUP_EXECUTE_SCHOOL,
];

async fn authority(f: &Fixture) -> (ActorContext, Uuid) {
    let role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type,is_active) VALUES('RETENTION_TEST','Retention test','staff',true) RETURNING id").fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=ANY($2)")
        .bind(role).bind(RETENTION_PERMISSIONS.to_vec()).execute(&f.pool).await.unwrap();
    // Remove fixture role assignments so the live authority test cannot inherit a wildcard.
    sqlx::query("DELETE FROM user_roles WHERE user_id=$1")
        .bind(TEACHER)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(TEACHER)
        .bind(role)
        .execute(&f.pool)
        .await
        .unwrap();
    (
        ActorContext {
            user_id: TEACHER,
            permissions: RETENTION_PERMISSIONS
                .iter()
                .map(|p| p.to_string())
                .collect(),
        },
        role,
    )
}
async fn close_for_retention(f: &Fixture) {
    sqlx::query("UPDATE academic_retention_policy SET enabled=true,effective_from=now()-interval '200 days',updated_by=$1")
        .bind(TEACHER).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,canonical_request,created_at) SELECT uuidv7(),academic_year_id,id,$2,'close',repeat('a',64),'{}','{}','{}',now()-interval '100 days' FROM academic_terms WHERE id=$1")
        .bind(TERM).bind(TEACHER).execute(&f.pool).await.unwrap();
    // This fixture's annual membership is secondary. Explicitly retain that source classification.
    sqlx::query(
        "UPDATE academic_retention_policy SET primary_boundary='term',secondary_boundary='term'",
    )
    .execute(&f.pool)
    .await
    .unwrap();
}
async fn advance(f: &Fixture) {
    sqlx::query("UPDATE academic_retention_jobs SET next_attempt_at=now()-interval '1 second'")
        .execute(&f.pool)
        .await
        .unwrap();
    crate::adapters::academic_retention::run(&f.pool)
        .await
        .unwrap();
}
async fn history_fingerprint(f: &Fixture) -> serde_json::Value {
    sqlx::query_scalar::<_,sqlx::types::Json<serde_json::Value>>("SELECT jsonb_build_object('users',(SELECT jsonb_agg(to_jsonb(u) ORDER BY u.id) FROM users u),'annual',(SELECT jsonb_agg(to_jsonb(a) ORDER BY a.id) FROM student_academic_years a),'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p))")
        .fetch_one(&f.pool).await.unwrap().0
}

#[tokio::test]
async fn academic_retention_preserves_attendance_behavior_and_original_file_manifest_after_disable()
{
    let f = fixture("academic_retention_real_providers").await;
    let (actor, role) = authority(&f).await;
    let date = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
        - Duration::days(120);
    sqlx::query("UPDATE behavior_state SET activation_date=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE behavior_cycles SET starts_on=$1,starts_at=$1::date::timestamp AT TIME ZONE 'Asia/Bangkok' WHERE state='current'").bind(date).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE behavior_settings SET enabled=true")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO behavior_attendance_rules(kind,status,effective_from,action,points,created_by) VALUES('arrival','present',$1,'merit',3,$2)")
        .bind(date).bind(TEACHER).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted) VALUES($1,$2,true)")
        .bind(TERM)
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let mut source = seed(&f, AttendanceKind::Arrival, date);
    source.session.count_override = Some(true);
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
    let before_totals:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_object('id',id,'points',points,'cycleId',cycle_id) ORDER BY id) FROM behavior_daily_totals")
        .fetch_one(&f.pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT sum(points)::bigint FROM behavior_daily_totals WHERE student_id=$1"
        )
        .bind(STUDENT)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        3
    );
    let file = evidence_file(&f.pool, STUDENT).await;
    let device = specials::save_device(
        &f.pool,
        TEACHER,
        Uuid::now_v7(),
        SaveAttendanceDevice {
            name: "Retention fixture".into(),
            enabled: true,
            operator_id: TEACHER,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES(uuidv7(),$1,$2,$3,$4,now())")
        .bind(session.id).bind(STUDENT).bind(device.id).bind(file).execute(&f.pool).await.unwrap();
    close_for_retention(&f).await;
    let before_school = reports::summaries(&f.pool, TERM, None, None)
        .await
        .unwrap()
        .summaries;
    let before_teacher = reports::summaries(&f.pool, TERM, None, Some(TEACHER))
        .await
        .unwrap()
        .summaries;

    let before_history = history_fingerprint(&f).await;
    advance(&f).await;
    let (job, stage, manifest): (Uuid, String, Vec<Uuid>) = sqlx::query_as(
        "SELECT id,status,evidence_file_ids FROM academic_retention_jobs WHERE academic_term_id=$1",
    )
    .bind(TERM)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(stage, "awaiting_behavior");
    assert_eq!(manifest, vec![file]);
    assert_eq!(
        serde_json::to_value(&before_school).unwrap(),
        serde_json::to_value(
            reports::summaries(&f.pool, TERM, None, None)
                .await
                .unwrap()
                .summaries
        )
        .unwrap()
    );
    assert_eq!(
        serde_json::to_value(&before_teacher).unwrap(),
        serde_json::to_value(
            reports::summaries(&f.pool, TERM, None, Some(TEACHER))
                .await
                .unwrap()
                .summaries
        )
        .unwrap()
    );
    assert_eq!(reports::impact(&f.pool, TERM).await.unwrap().records, 0);
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM attendance_term_summaries WHERE academic_term_id=$1 AND present>0)").bind(TERM).fetch_one(&f.pool).await.unwrap());
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM attendance_term_teacher_summaries WHERE academic_term_id=$1 AND teacher_id=$2 AND present>0)").bind(TERM).bind(TEACHER).fetch_one(&f.pool).await.unwrap());
    let version: i64 = sqlx::query_scalar("SELECT row_version FROM academic_retention_policy")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    retention::update_policy(
        &f.pool,
        &actor,
        UpdateAcademicRetentionPolicy {
            expected_row_version: version,
            enabled: false,
            primary_boundary: AcademicRetentionBoundary::Year,
            secondary_boundary: AcademicRetentionBoundary::Term,
            grace_days: 90,
        },
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM role_permissions WHERE role_id=$1")
        .bind(role)
        .execute(&f.pool)
        .await
        .unwrap();
    advance(&f).await; // Already accepted work resumes through trusted domain receipt authority.
    let after_totals:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_object('id',id,'points',points,'cycleId',cycle_id) ORDER BY id) FROM behavior_daily_totals")
        .fetch_one(&f.pool).await.unwrap();
    assert_eq!(before_totals.0, after_totals.0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM behavior_entries")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    advance(&f).await;
    let pending = retention::get_workspace(
        &crate::adapters::academic_retention::ACADEMIC_RETENTION,
        &f.pool,
        &actor,
        sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1")
            .bind(TERM)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let status = pending
        .terms
        .iter()
        .find(|t| t.academic_term_id == TERM)
        .unwrap()
        .job
        .as_ref()
        .unwrap();
    assert!(status.logical_cleanup_complete);
    assert!(!status.physical_cleanup_complete);
    assert_eq!(status.pending_files, 1);
    sqlx::query("UPDATE file_operations SET status='failed',last_error_code='synthetic_provider_failure' WHERE file_id=$1 AND operation_type='delete_object'").bind(file).execute(&f.pool).await.unwrap();
    advance(&f).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT failed_files FROM academic_retention_jobs WHERE id=$1"
        )
        .bind(job)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM academic_retention_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "awaiting_files"
    );
    assert_eq!(history_fingerprint(&f).await, before_history);
    let mut tx = f.pool.begin().await.unwrap();
    assert!(matches!(
        retention::require_reopen_allowed(&mut tx, pending.terms[0].academic_year_id, Some(TERM))
            .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn academic_retention_excludes_manual_archives_and_never_deletes_on_preview() {
    let f = fixture("academic_retention_manual_archive").await;
    let (actor, _) = authority(&f).await;
    close_for_retention(&f).await;
    sqlx::query("INSERT INTO attendance_term_archives(academic_term_id,archived_by,record_count,counted_record_count,settings_snapshot) VALUES($1,$2,0,0,'{}')")
        .bind(TERM).bind(TEACHER).execute(&f.pool).await.unwrap();
    let year: Uuid = sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1")
        .bind(TERM)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let workspace = retention::get_workspace(
        &crate::adapters::academic_retention::ACADEMIC_RETENTION,
        &f.pool,
        &actor,
        year,
    )
    .await
    .unwrap();
    let term = workspace
        .terms
        .iter()
        .find(|t| t.academic_term_id == TERM)
        .unwrap();
    assert!(!term.eligible);
    assert!(term
        .blocked_reasons
        .contains(&"attendance_already_archived".into()));
    advance(&f).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_retention_jobs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}
