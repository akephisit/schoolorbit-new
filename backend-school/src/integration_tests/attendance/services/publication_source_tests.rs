use super::*;
use chrono::Datelike;
use school_academic_timetable::{
    collaboration::TimetableMutationContext,
    models::{
        timetable_block::UpdateTimetableBlockRequest,
        timetable_publication::{
            PreviewTimetablePublicationRequest, PublishTimetableVersionRequest,
        },
        timetable_version::{CloneTimetableVersionRequest, TimetableVersion},
    },
    services::{
        timetable_block_service as blocks, timetable_lifecycle as lifecycle,
        timetable_version_service as tables,
    },
};

struct Prepared {
    pool: PgPool,
    actor: ActorContext,
    source: AttendanceSeed,
    session: AttendanceSession,
    mutation: TimetableMutationContext,
    draft: TimetableVersion,
    request: PublishTimetableVersionRequest,
    current_key: String,
}

async fn moved_draft(
    pool: &PgPool,
    mutation: &TimetableMutationContext,
    source: Uuid,
    group: Uuid,
    date: NaiveDate,
) -> TimetableVersion {
    let original = tables::get_version(pool, source, date).await.unwrap();
    let draft = tables::clone_draft(
        pool,
        mutation,
        source,
        CloneTimetableVersionRequest {
            source_row_version: original.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    crate::integration_tests::academic::delivery::fill_change_set_target_deficits(
        pool, TEACHER, TERM, draft.id,
    )
    .await;
    let id:Uuid=sqlx::query_scalar("SELECT b.id FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id AND g.is_active WHERE b.timetable_version_id=$1 AND b.is_active AND g.learning_group_id=$2 ORDER BY b.id LIMIT 1")
        .bind(draft.id).bind(group).fetch_one(pool).await.unwrap();
    let block = blocks::get_block(pool, id).await.unwrap();
    let day = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
        [date.weekday().num_days_from_monday() as usize];
    let period:Uuid=sqlx::query_scalar("SELECT p.id FROM bell_schedule_periods p WHERE p.bell_schedule_id=$1 AND p.is_active AND p.id<>$2 AND (p.applicable_days IS NULL OR $3=ANY(string_to_array(p.applicable_days,','))) AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks b WHERE b.timetable_version_id=$4 AND b.is_active AND b.id<>$5 AND b.day_of_week=$3 AND b.bell_schedule_period_id=p.id) ORDER BY p.order_index,p.id LIMIT 1")
        .bind(draft.bell_schedule_id).bind(block.bell_schedule_period_id).bind(day).bind(draft.id).bind(id).fetch_one(pool).await.unwrap();
    blocks::update_block(
        pool,
        mutation,
        id,
        UpdateTimetableBlockRequest {
            timetable_version_id: draft.id,
            row_version: block.row_version,
            day_of_week: Some(day.into()),
            bell_schedule_period_id: Some(period),
            title: None,
            clear_title: false,
            note: None,
            clear_note: false,
            room_id: None,
            clear_room: false,
            instructor_ids: None,
            teacher_ids: None,
        },
    )
    .await
    .unwrap();
    tables::get_version(pool, draft.id, date).await.unwrap()
}

async fn publication_request(
    pool: &PgPool,
    draft: &TimetableVersion,
    date: NaiveDate,
) -> PublishTimetableVersionRequest {
    let preview = lifecycle::preview(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        pool,
        draft.id,
        PreviewTimetablePublicationRequest {
            row_version: draft.row_version,
            effective_from: date,
        },
    )
    .await
    .unwrap();
    assert!(preview.can_publish, "{:?}", preview.findings);
    PublishTimetableVersionRequest {
        row_version: draft.row_version,
        effective_from: date,
        preview_hash: preview.preview_hash,
        idempotency_key: Uuid::now_v7(),
    }
}

async fn prepare(name: &str) -> Prepared {
    let f = fixture(name).await;
    let date = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    // A school may teach any configured weekday. This fixture needs a real lesson
    // on the actual scan day, including weekends; no injected clock or skipped case.
    sqlx::query("UPDATE academic_years SET school_days='MON,TUE,WED,THU,FRI,SAT,SUN' WHERE id=(SELECT academic_year_id FROM academic_terms WHERE id=$1)")
        .bind(TERM).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE bell_schedule_periods SET applicable_days=NULL WHERE bell_schedule_id=(SELECT bell_schedule_id FROM academic_terms WHERE id=$1)")
        .bind(TERM).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days,is_active) SELECT uuidv7(),base.bell_schedule_id,'Publication race fixture',base.start_time+step*interval '1 hour',base.end_time+step*interval '1 hour',base.order_index+step,NULL,true FROM (SELECT * FROM bell_schedule_periods WHERE bell_schedule_id=(SELECT bell_schedule_id FROM academic_terms WHERE id=$1) ORDER BY order_index DESC LIMIT 1) base CROSS JOIN generate_series(1,8) step")
        .bind(TERM).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted) VALUES($1,$2,true) ON CONFLICT(academic_term_id,date) DO UPDATE SET counted=true")
        .bind(TERM).bind(date).execute(&f.pool).await.unwrap();
    let (source,group):(Uuid,Uuid)=sqlx::query_as("SELECT v.id,g.learning_group_id FROM academic_timetable_versions v JOIN academic_timetable_blocks b ON b.timetable_version_id=v.id AND b.is_active JOIN academic_timetable_block_groups g ON g.block_id=b.id AND g.is_active JOIN learning_offerings o ON o.id=b.learning_offering_id AND o.kind='course' WHERE v.id=academic_timetable_applicable_publication($1,$2) AND EXISTS(SELECT 1 FROM learning_group_students s WHERE s.learning_group_id=g.learning_group_id AND s.student_id=$3 AND s.membership_status='active') ORDER BY b.id,g.id LIMIT 1")
        .bind(TERM).bind(date).bind(STUDENT).fetch_one(&f.pool).await.unwrap();
    let mutation = TimetableMutationContext::for_tests(&f.pool, TEACHER);
    let first = moved_draft(&f.pool, &mutation, source, group, date).await;
    let request = publication_request(&f.pool, &first, date).await;
    let baseline = lifecycle::publish(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &f.pool,
        &mutation,
        first.id,
        request,
    )
    .await
    .unwrap();
    let source = seeds(&f.pool, TERM, date)
        .await
        .unwrap()
        .into_iter()
        .find(|seed| {
            seed.session.kind == AttendanceKind::Lesson
                && seed.session.learning_group_id == Some(group)
                && seed
                    .students
                    .iter()
                    .any(|student| student.student_id == STUDENT)
        })
        .unwrap();
    let mut session = source.session.clone();
    session.roster = source.students.clone();
    let session = sessions::with_token(&f.pool, session).await.unwrap();
    let draft = moved_draft(&f.pool, &mutation, baseline.id, group, date).await;
    let request = publication_request(&f.pool, &draft, date).await;
    let period:Uuid=sqlx::query_scalar("SELECT b.bell_schedule_period_id FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id AND g.is_active WHERE b.timetable_version_id=$1 AND b.is_active AND g.learning_group_id=$2 AND b.day_of_week=$3 ORDER BY b.id LIMIT 1")
        .bind(draft.id).bind(group).bind(["MON","TUE","WED","THU","FRI","SAT","SUN"][date.weekday().num_days_from_monday() as usize]).fetch_one(&f.pool).await.unwrap();
    Prepared {
        pool: f.pool,
        actor: ActorContext {
            user_id: TEACHER,
            permissions: vec![
                codes::ATTENDANCE_READ_SCHOOL.into(),
                codes::ATTENDANCE_UPDATE_SCHOOL.into(),
                codes::ATTENDANCE_VERIFY_ASSIGNED.into(),
            ],
        },
        source,
        session,
        mutation,
        draft,
        request,
        current_key: format!("{group}:{period}"),
    }
}

async fn wait_for_blocked(
    connection: &mut sqlx::PgConnection,
    blocker: i32,
    fragment: &str,
) -> i32 {
    tokio::time::timeout(std::time::Duration::from_secs(15),async {
        loop {
            let pid:Option<i32>=sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE $2 ORDER BY pid LIMIT 1")
                .bind(blocker).bind(format!("%{fragment}%")).fetch_optional(&mut *connection).await.unwrap();
            if let Some(pid)=pid {return pid;}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("expected a real queued database operation")
}

async fn block_publication(
    prepared: &Prepared,
) -> (
    sqlx::Transaction<'static, sqlx::Postgres>,
    i32,
    tokio::task::JoinHandle<Result<TimetableVersion, AppError>>,
) {
    let key = (prepared.draft.id.as_u128() as u64) as i64;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION fixture_pause_publication() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.entity_id='{}'::uuid AND NEW.event_code='academic_timetable_version.published' THEN PERFORM pg_advisory_xact_lock({key}); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_pause_publication BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fixture_pause_publication();",prepared.draft.id)))
        .execute(&prepared.pool).await.unwrap();
    let mut guard = prepared.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *guard)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    let pool = prepared.pool.clone();
    let mutation = prepared.mutation.clone();
    let id = prepared.draft.id;
    let request = prepared.request.clone();
    let publication = tokio::spawn(async move {
        lifecycle::publish(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &pool,
            &mutation,
            id,
            request,
        )
        .await
    });
    let publisher = wait_for_blocked(&mut guard, pid, "academic_audit_events").await;
    (guard, publisher, publication)
}

#[tokio::test]
async fn attendance_queued_first_lesson_materialization_refuses_the_plan_replaced_while_waiting() {
    for mode in ["ensure", "save", "cancel", "scan"] {
        let prepared = prepare(&format!("attendance_publication_queue_{mode}")).await;
        let scan = if mode == "scan" {
            let descriptor = FaceDescriptor {
                values: vec![0.01; 128],
            };
            faces::enroll(
                &prepared.pool,
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
                &prepared.pool,
                TEACHER,
                Uuid::now_v7(),
                SaveAttendanceDevice {
                    name: "Publication source fixture".into(),
                    enabled: true,
                    operator_id: TEACHER,
                },
            )
            .await
            .unwrap();
            Some(AttendanceScan {
                event_id: Uuid::now_v7(),
                device_id: device.id,
                session_id: prepared.session.id,
                session: session_context(&prepared.session),
                roster_token: prepared.session.roster_token.clone(),
                student_id: STUDENT,
                descriptor,
                evidence_file_id: evidence_file(&prepared.pool, STUDENT).await,
                captured_at: Utc::now(),
            })
        } else {
            None
        };
        let (mut guard, publisher, publication) = block_publication(&prepared).await;
        let pool = prepared.pool.clone();
        let actor = prepared.actor.clone();
        let source = prepared.source.clone();
        let session = prepared.session.clone();
        let pending = tokio::spawn(async move {
            match mode {
                "ensure" => sessions::ensure(&pool, &actor, source).await.map(|_| ()),
                "save" => save(
                    &pool,
                    &actor,
                    session.id,
                    SaveAttendanceResults {
                        session: session_context(&session),
                        roster_token: session.roster_token,
                        row_version: 0,
                        reason: String::new(),
                        students: vec![AttendanceResultInput {
                            student_id: STUDENT,
                            result: AttendanceResult::Present,
                        }],
                    },
                )
                .await
                .map(|_| ()),
                "cancel" => cancel(
                    &pool,
                    &actor,
                    session.id,
                    AttendanceCancellation {
                        session: session_context(&session),
                        roster_token: session.roster_token,
                        row_version: 0,
                        cancelled: true,
                        reason: "Source race fixture".into(),
                    },
                )
                .await
                .map(|_| ()),
                "scan" => faces::scan_with_seed_and_cached_gallery(
                    &pool,
                    &Uuid::now_v7().to_string(),
                    &actor,
                    scan.unwrap(),
                    Some(source),
                )
                .await
                .map(|_| ()),
                _ => unreachable!(),
            }
        });
        wait_for_blocked(&mut guard, publisher, "academic_terms").await;
        guard.commit().await.unwrap();
        publication.await.unwrap().unwrap();
        assert!(
            matches!(pending.await.unwrap(), Err(AppError::Conflict(_))),
            "{mode} accepted a stale first lesson"
        );
        let artifacts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM attendance_sessions WHERE id=$1),(SELECT count(*) FROM attendance_records WHERE session_id=$1),(SELECT count(*) FROM attendance_scan_events WHERE session_id=$1)")
            .bind(prepared.session.id).fetch_one(&prepared.pool).await.unwrap();
        assert_eq!(
            artifacts,
            (0, 0, 0),
            "{mode} left stale attendance artifacts"
        );
    }
}

#[tokio::test]
async fn attendance_finalization_resolves_one_fresh_batch_after_a_waiting_plan_publication() {
    let prepared = prepare("attendance_publication_finalization_queue").await;
    let date = prepared.session.date;
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1,coverage_start=$1")
        .bind(date)
        .execute(&prepared.pool)
        .await
        .unwrap();
    let (mut guard, publisher, publication) = block_publication(&prepared).await;
    let pool = prepared.pool.clone();
    let now = date
        .succ_opt()
        .unwrap()
        .and_hms_opt(5, 0, 0)
        .unwrap()
        .and_utc();
    let finalization = tokio::spawn(async move {
        school_attendance::application::finalization::catch_up(&pool, now, 1).await
    });
    wait_for_blocked(&mut guard, publisher, "academic_terms").await;
    guard.commit().await.unwrap();
    publication.await.unwrap().unwrap();
    assert_eq!(finalization.await.unwrap().unwrap(), 1);
    let keys:Vec<String>=sqlx::query_scalar("SELECT source_key FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind='lesson'").bind(TERM).bind(date).fetch_all(&prepared.pool).await.unwrap();
    assert!(keys.contains(&prepared.current_key));
    assert!(!keys.contains(&prepared.source.session.source_key));
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM attendance_finalized_dates d JOIN attendance_module_state m ON d.generation=m.generation WHERE m.singleton AND d.academic_term_id=$1 AND d.date=$2)").bind(TERM).bind(date).fetch_one(&prepared.pool).await.unwrap());
}
