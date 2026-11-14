use super::*;
use chrono::Datelike;
use school_academic_timetable::{
    collaboration::TimetableMutationContext,
    models::timetable_block::{
        CreateOrdinaryTimetableBlockRequest, TimetableBlock, UpdateTimetableBlockRequest,
    },
    services::timetable_block_service as blocks,
};

struct Prepared {
    pool: PgPool,
    actor: ActorContext,
    source: AttendanceSeed,
    session: AttendanceSession,
    mutation: TimetableMutationContext,
    block_id: Uuid,
    request: UpdateTimetableBlockRequest,
    current_key: String,
    current_start: NaiveTime,
}
fn move_request(block: &TimetableBlock, day: &str, period: Uuid) -> UpdateTimetableBlockRequest {
    UpdateTimetableBlockRequest {
        timetable_id: block.timetable_id,
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
    }
}
async fn prepare(name: &str) -> Prepared {
    let f = fixture(name).await;
    let date = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    sqlx::query("UPDATE academic_terms SET school_days='MON,TUE,WED,THU,FRI,SAT,SUN' WHERE id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE bell_schedule_periods SET applicable_days=NULL WHERE bell_schedule_id=(SELECT bell_schedule_id FROM academic_terms WHERE id=$1)").bind(TERM).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days,is_active) SELECT uuidv7(),base.bell_schedule_id,'Current edit race',base.start_time+step*interval '1 hour',base.end_time+step*interval '1 hour',base.order_index+step,NULL,true FROM (SELECT * FROM bell_schedule_periods WHERE bell_schedule_id=(SELECT bell_schedule_id FROM academic_terms WHERE id=$1) ORDER BY order_index DESC LIMIT 1) base CROSS JOIN generate_series(1,8) step").bind(TERM).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted) VALUES($1,$2,true) ON CONFLICT(academic_term_id,date) DO UPDATE SET counted=true").bind(TERM).bind(date).execute(&f.pool).await.unwrap();
    let (id,group):(Uuid,Uuid)=sqlx::query_as("SELECT block.id,placed.learning_group_id FROM academic_timetable_blocks block JOIN academic_timetable_block_groups placed ON placed.block_id=block.id AND placed.is_active JOIN learning_offerings offering ON offering.id=block.learning_offering_id AND offering.kind='course' WHERE block.academic_term_id=$1 AND block.is_active AND EXISTS(SELECT 1 FROM learning_group_students member WHERE member.learning_group_id=placed.learning_group_id AND member.student_id=$2 AND member.membership_status<>'removed') ORDER BY block.id,placed.id LIMIT 1").bind(TERM).bind(STUDENT).fetch_one(&f.pool).await.unwrap();
    let original = blocks::get_block(&f.pool, id).await.unwrap();
    let day = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
        [date.weekday().num_days_from_monday() as usize];
    let periods:Vec<(Uuid,NaiveTime)>=sqlx::query_as("SELECT period.id,period.start_time FROM bell_schedule_periods period WHERE period.bell_schedule_id=$1 AND period.is_active AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_id=$2 AND block.day_of_week=$3 AND block.bell_schedule_period_id=period.id AND block.is_active) ORDER BY period.order_index,period.id LIMIT 2").bind(original.bell_schedule_id).bind(original.timetable_id).bind(day).fetch_all(&f.pool).await.unwrap();
    assert_eq!(periods.len(), 2);
    let mutation = TimetableMutationContext::for_tests(&f.pool, TEACHER);
    let baseline = blocks::update_block(
        &f.pool,
        &mutation,
        id,
        move_request(&original, day, periods[0].0),
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
                && seed.session.start_time == periods[0].1
                && seed
                    .students
                    .iter()
                    .any(|student| student.student_id == STUDENT)
        })
        .unwrap();
    let mut session = source.session.clone();
    session.roster = source.students.clone();
    let session = sessions::with_token(&f.pool, session).await.unwrap();
    let current_key = source.session.source_key.clone();
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
        block_id: id,
        request: move_request(&baseline, day, periods[1].0),
        current_key,
        current_start: periods[1].1,
    }
}
async fn wait_for_blocked(
    connection: &mut sqlx::PgConnection,
    blocker: i32,
    fragment: &str,
) -> i32 {
    tokio::time::timeout(std::time::Duration::from_secs(15),async {loop {
        sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *connection).await.unwrap();
        let pid:Option<i32>=sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE $2 ORDER BY pid LIMIT 1").bind(blocker).bind(format!("%{fragment}%")).fetch_optional(&mut *connection).await.unwrap();
        if let Some(pid)=pid{return pid;}tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }}).await.unwrap_or_else(|_|panic!("expected {fragment} behind backend {blocker}"))
}
async fn block_current_edit(
    prepared: &Prepared,
) -> (
    sqlx::Transaction<'static, sqlx::Postgres>,
    i32,
    tokio::task::JoinHandle<Result<TimetableBlock, AppError>>,
) {
    let key = (prepared.block_id.as_u128() as u64) as i64;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION fixture_pause_current_edit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='{}'::uuid AND NEW.bell_schedule_period_id='{}'::uuid THEN PERFORM pg_advisory_xact_lock({key}); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_pause_current_edit BEFORE UPDATE ON academic_timetable_blocks FOR EACH ROW EXECUTE FUNCTION fixture_pause_current_edit();",prepared.block_id,prepared.request.bell_schedule_period_id.unwrap()))).execute(&prepared.pool).await.unwrap();
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
    let id = prepared.block_id;
    let request = prepared.request.clone();
    let mut edit =
        tokio::spawn(async move { blocks::update_block(&pool, &mutation, id, request).await });
    let writer = tokio::select! {outcome=&mut edit=>panic!("edit finished before fixture gate: {outcome:?}"),writer=wait_for_blocked(&mut guard,pid,"academic_timetable_blocks")=>writer};
    (guard, writer, edit)
}

#[tokio::test]
async fn attendance_queued_first_lesson_materialization_refuses_the_current_pattern_changed_while_waiting(
) {
    for mode in ["ensure", "save", "cancel", "scan"] {
        let prepared = prepare(&format!("attendance_edit_queue_{mode}")).await;
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
        let (mut guard, publisher, edit) = block_current_edit(&prepared).await;
        let pool = prepared.pool.clone();
        let actor = prepared.actor.clone();
        let source = prepared.source.clone();
        let session = prepared.session.clone();
        let mut pending = tokio::spawn(async move {
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
        tokio::select! {
            outcome=&mut pending => panic!("{mode} finished before the current edit term lock: {outcome:?}"),
            _=wait_for_blocked(&mut guard,publisher,"academic_terms") => {},
        }
        guard.commit().await.unwrap();
        edit.await.unwrap().unwrap();
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
async fn attendance_finalization_keeps_materialized_facts_after_a_waiting_current_edit() {
    let prepared = prepare("attendance_edit_finalization_queue").await;
    let date = prepared.session.date;
    sessions::ensure(&prepared.pool, &prepared.actor, prepared.source.clone())
        .await
        .unwrap();
    let configuration = settings::get(&prepared.pool, TERM).await.unwrap();
    settings::save(
        &prepared.pool,
        TEACHER,
        TERM,
        SaveAttendanceSettings {
            auto_finalize_from: Some(date),
            row_version: configuration.row_version,
            configuration: configuration.configuration,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1,coverage_start=$1")
        .bind(date)
        .execute(&prepared.pool)
        .await
        .unwrap();
    let (mut guard, publisher, edit) = block_current_edit(&prepared).await;
    let pool = prepared.pool.clone();
    let now = date
        .succ_opt()
        .unwrap()
        .and_hms_opt(5, 0, 0)
        .unwrap()
        .and_utc();
    let mut finalization = tokio::spawn(async move {
        school_attendance::application::finalization::catch_up(&pool, now, 1).await
    });
    tokio::select! {
        outcome=&mut finalization => panic!("finalization finished before the current edit term lock: {outcome:?}"),
        _=wait_for_blocked(&mut guard,publisher,"academic_terms") => {},
    }
    guard.commit().await.unwrap();
    edit.await.unwrap().unwrap();
    assert_eq!(finalization.await.unwrap().unwrap(), 1);
    let keys:Vec<String>=sqlx::query_scalar("SELECT source_key FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind='lesson'").bind(TERM).bind(date).fetch_all(&prepared.pool).await.unwrap();
    assert!(keys.contains(&prepared.current_key));
    assert_eq!(
        prepared.current_key, prepared.source.session.source_key,
        "a placement move keeps its occurrence identity"
    );
    let start:NaiveTime=sqlx::query_scalar("SELECT start_time FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind='lesson' AND source_key=$3").bind(TERM).bind(date).bind(&prepared.current_key).fetch_one(&prepared.pool).await.unwrap();
    assert_eq!(start, prepared.session.start_time);
    assert_ne!(start, prepared.current_start);
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM attendance_finalized_dates d JOIN attendance_module_state m ON d.generation=m.generation WHERE m.singleton AND d.academic_term_id=$1 AND d.date=$2)").bind(TERM).bind(date).fetch_one(&prepared.pool).await.unwrap());
}

async fn add_current_lesson(prepared: &Prepared, day: &str) -> TimetableBlock {
    let original = blocks::get_block(&prepared.pool, prepared.block_id)
        .await
        .unwrap();
    let placed = &original.groups[0];
    let period:Uuid=sqlx::query_scalar("SELECT period.id FROM bell_schedule_periods period WHERE period.bell_schedule_id=$1 AND period.is_active AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_id=$2 AND block.day_of_week=$3 AND block.bell_schedule_period_id=period.id AND block.is_active) ORDER BY period.order_index,period.id LIMIT 1").bind(original.bell_schedule_id).bind(original.timetable_id).bind(day).fetch_one(&prepared.pool).await.unwrap();
    let (revision, weekly): (i64, i32) = sqlx::query_as(
        "SELECT row_version,weekly_period_target FROM learning_offerings WHERE id=$1",
    )
    .bind(placed.learning_offering_id)
    .fetch_one(&prepared.pool)
    .await
    .unwrap();
    school_academic_delivery::services::offerings::update_weekly_period_target(
        &prepared.pool,
        placed.learning_offering_id,
        school_academic_delivery::models::current::UpdateWeeklyPeriodTargetRequest {
            row_version: revision,
            weekly_period_target: weekly + 1,
        },
    )
    .await
    .unwrap();
    blocks::create_ordinary_block(
        &prepared.pool,
        &prepared.mutation,
        CreateOrdinaryTimetableBlockRequest {
            timetable_id: original.timetable_id,
            academic_term_id: TERM,
            learning_group_id: placed.learning_group_id,
            day_of_week: day.into(),
            bell_schedule_period_id: period,
            room_id: None,
            instructor_ids: placed
                .instructors
                .iter()
                .map(|teacher| teacher.teacher_id)
                .collect(),
            note: None,
        },
    )
    .await
    .unwrap()
}

async fn lesson_history(pool: &PgPool, actor: &ActorContext, date: NaiveDate) -> serde_json::Value {
    let rows = history(
        pool,
        actor,
        AttendanceHistoryQuery {
            academic_term_id: TERM,
            student_id: STUDENT,
            start: date,
            end: date,
        },
    )
    .await
    .unwrap();
    serde_json::to_value(
        rows.into_iter()
            .filter(|row| row.kind == AttendanceKind::Lesson)
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
async fn historical_lesson_report(pool: &PgPool, date: NaiveDate) -> serde_json::Value {
    let expected = school_attendance::application::projection::expected(
        &mut *pool.acquire().await.unwrap(),
        TERM,
        Some((date, date)),
    )
    .await
    .unwrap();
    assert!(
        expected.iter().all(|row| row.category != "lesson"),
        "old unmaterialized lessons cannot become expected rows"
    );
    let report = reports::summaries_with_expected(pool, TERM, Some(STUDENT), None, &expected)
        .await
        .unwrap();
    serde_json::to_value(
        report
            .summaries
            .into_iter()
            .filter(|row| row.category == "lesson")
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[tokio::test]
async fn past_lesson_history_choices_and_finalization_ignore_new_current_monday_placements() {
    let prepared = prepare("attendance_past_materialized_boundary").await;
    let mut past = prepared.session.date.pred_opt().unwrap();
    while past.weekday() != chrono::Weekday::Mon {
        past = past.pred_opt().unwrap();
    }
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted) VALUES($1,$2,true) ON CONFLICT(academic_term_id,date) DO UPDATE SET counted=true").bind(TERM).bind(past).execute(&prepared.pool).await.unwrap();
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1,coverage_start=$1 WHERE singleton").bind(past).execute(&prepared.pool).await.unwrap();
    let mut actual = prepared.source.clone();
    actual.session.id = Uuid::new_v4();
    actual.session.date = past;
    let f = Fixture {
        pool: prepared.pool.clone(),
        actor: prepared.actor.clone(),
        students: actual.students.clone(),
    };
    let saved = accepted_historical_lesson(&f, actual.clone()).await;
    let mut fabricated = actual.clone();
    fabricated.session.id = Uuid::new_v4();
    fabricated.session.source_key = format!("placement:{}", Uuid::new_v4());
    let generation = sessions::module_generation(&prepared.pool).await.unwrap();
    let mut tx = prepared.pool.begin().await.unwrap();
    assert!(matches!(
        school_attendance::services::finalization::finalize_date_in_transaction(
            &mut tx,
            TERM,
            past,
            generation,
            &[fabricated],
            Utc::now()
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_records WHERE session_id=$1")
            .bind(saved.id)
            .fetch_one(&prepared.pool)
            .await
            .unwrap(),
        0
    );
    // The real worker can close the actual unchecked roster and advance past this date.
    assert!(
        school_attendance::application::finalization::catch_up(&prepared.pool, Utc::now(), 31)
            .await
            .unwrap()
            > 0
    );
    assert_eq!(
        school_attendance::application::finalization::catch_up(&prepared.pool, Utc::now(), 31)
            .await
            .unwrap(),
        0
    );
    let results:Vec<(Uuid,String,String)>=sqlx::query_as("SELECT student_id,result,origin FROM attendance_records WHERE session_id=$1 ORDER BY student_id").bind(saved.id).fetch_all(&prepared.pool).await.unwrap();
    assert_eq!(results.len(), actual.students.len());
    assert!(results
        .iter()
        .all(|row| row.1 == "absent" && row.2 == "automatic"));
    let before_history = lesson_history(&prepared.pool, &prepared.actor, past).await;
    let before_report = historical_lesson_report(&prepared.pool, past).await;
    let added = add_current_lesson(&prepared, "MON").await;
    let original = blocks::get_block(&prepared.pool, prepared.block_id)
        .await
        .unwrap();
    let free:Uuid=sqlx::query_scalar("SELECT period.id FROM bell_schedule_periods period WHERE period.bell_schedule_id=$1 AND period.is_active AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_id=$2 AND block.day_of_week='MON' AND block.bell_schedule_period_id=period.id AND block.is_active) ORDER BY period.order_index,period.id LIMIT 1").bind(original.bell_schedule_id).bind(original.timetable_id).fetch_one(&prepared.pool).await.unwrap();
    blocks::update_block(
        &prepared.pool,
        &prepared.mutation,
        original.id,
        move_request(&original, "MON", free),
    )
    .await
    .unwrap();
    assert_eq!(
        lesson_history(&prepared.pool, &prepared.actor, past).await,
        before_history
    );
    assert_eq!(
        historical_lesson_report(&prepared.pool, past).await,
        before_report
    );
    let workspace = workspace(
        &prepared.pool,
        &prepared.actor,
        AttendanceQuery {
            academic_term_id: TERM,
            date: past,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        workspace
            .sessions
            .iter()
            .filter(|session| session.kind == AttendanceKind::Lesson)
            .map(|session| session.id)
            .collect::<Vec<_>>(),
        vec![saved.id]
    );
    assert!(
        workspace.lesson_choices.is_empty(),
        "past labels come from actual saved title/time, not today's group/period metadata"
    );
    let open = OpenAttendanceSession {
        academic_term_id: TERM,
        date: past,
        kind: AttendanceKind::Lesson,
        source_key: format!("placement:{}", added.groups[0].id),
    };
    assert!(matches!(
        school_attendance::application::resolve_seed(&prepared.pool, &open).await,
        Err(AppError::ConflictWithCode {
            code: "ATTENDANCE_PAST_LESSON_NOT_MATERIALIZED",
            ..
        })
    ));
    assert_eq!(
        school_attendance::application::finalization::catch_up(&prepared.pool, Utc::now(), 31)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        lesson_history(&prepared.pool, &prepared.actor, past).await,
        before_history
    );
}

#[tokio::test]
async fn saved_current_occurrence_survives_move_without_hiding_a_second_unsaved_lesson() {
    let prepared = prepare("attendance_current_partial_day_move").await;
    let saved = sessions::ensure(&prepared.pool, &prepared.actor, prepared.source.clone())
        .await
        .unwrap();
    let detail = sessions::detail(&prepared.pool, &prepared.actor, saved.id)
        .await
        .unwrap();
    sessions::save(
        &prepared.pool,
        &prepared.actor,
        saved.id,
        save_payload(&detail, STUDENT, AttendanceResult::Present),
    )
    .await
    .unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('session',to_jsonb(session),'records',(SELECT jsonb_agg(to_jsonb(record) ORDER BY student_id) FROM attendance_records record WHERE record.session_id=session.id)) FROM attendance_sessions session WHERE id=$1").bind(saved.id).fetch_one(&prepared.pool).await.unwrap();
    let day = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
        [prepared.session.date.weekday().num_days_from_monday() as usize];
    let second = add_current_lesson(&prepared, day).await;
    let original = blocks::get_block(&prepared.pool, prepared.block_id)
        .await
        .unwrap();
    let free:Uuid=sqlx::query_scalar("SELECT period.id FROM bell_schedule_periods period WHERE period.bell_schedule_id=$1 AND period.is_active AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks block WHERE block.timetable_id=$2 AND block.day_of_week=$3 AND block.bell_schedule_period_id=period.id AND block.is_active) ORDER BY period.order_index,period.id LIMIT 1").bind(original.bell_schedule_id).bind(original.timetable_id).bind(day).fetch_one(&prepared.pool).await.unwrap();
    blocks::update_block(
        &prepared.pool,
        &prepared.mutation,
        original.id,
        move_request(&original, day, free),
    )
    .await
    .unwrap();
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('session',to_jsonb(session),'records',(SELECT jsonb_agg(to_jsonb(record) ORDER BY student_id) FROM attendance_records record WHERE record.session_id=session.id)) FROM attendance_sessions session WHERE id=$1").bind(saved.id).fetch_one(&prepared.pool).await.unwrap();
    assert_eq!(
        after, before,
        "ordinary schedule edits preserve the complete actual session and results"
    );
    let workspace = workspace(
        &prepared.pool,
        &prepared.actor,
        AttendanceQuery {
            academic_term_id: TERM,
            date: prepared.session.date,
        },
    )
    .await
    .unwrap();
    let actual = workspace
        .sessions
        .iter()
        .find(|session| session.id == saved.id)
        .unwrap();
    assert_eq!(actual.start_time, saved.start_time);
    assert!(workspace.sessions.iter().any(|session| session.source_key
        == format!("placement:{}", second.groups[0].id)
        && session.row_version == 0));
    assert_eq!(
        workspace
            .sessions
            .iter()
            .filter(|session| session.id == saved.id)
            .count(),
        1
    );
}
