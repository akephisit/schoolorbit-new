use super::*;
use sqlx::{postgres::PgPoolOptions, types::Json};
use std::time::Duration;

#[tokio::test]
async fn attendance_read_snapshot_all_preview_kinds_use_one_connection_and_frozen_rosters() {
    let f = db_tests::fixture("attendance_read_snapshot_preview_kinds").await;
    let first: NaiveDate = sqlx::query_scalar("SELECT min(effective_from) FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published'")
        .bind(db_tests::TERM).fetch_one(&f.pool).await.unwrap();
    let mut teaching_date = None;
    for offset in 0..7 {
        let candidate = first + chrono::Duration::days(offset);
        if seeds(&f.pool, db_tests::TERM, candidate)
            .await
            .unwrap()
            .iter()
            .any(|seed| {
                seed.session.kind == AttendanceKind::Lesson
                    && seed
                        .students
                        .iter()
                        .any(|student| student.student_id == db_tests::STUDENT)
            })
        {
            teaching_date = Some(candidate);
            break;
        }
    }
    let date = teaching_date
        .expect("current fixture must include one published teaching day for the selected pupil");
    let actor = ActorContext {
        user_id: db_tests::TEACHER,
        permissions: vec![
            codes::ATTENDANCE_READ_SCHOOL.into(),
            codes::ATTENDANCE_UPDATE_SCHOOL.into(),
            codes::ATTENDANCE_MANAGE_SCHOOL.into(),
        ],
    };
    create_special(
        &f.pool,
        &actor,
        db_tests::TERM,
        SpecialAttendanceDefinition {
            scoring: None,
            title: "Local snapshot fixture".into(),
            dates: vec![date],
            start_time: NaiveTime::MIN,
            end_time: NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
            counted: true,
            notify: false,
            groups: vec![SpecialAttendanceGroup {
                name: "Fixture group".into(),
                teacher_ids: vec![db_tests::TEACHER],
                homeroom_ids: vec![],
                audience_group_ids: vec![],
                student_ids: vec![db_tests::STUDENT],
                use_homeroom_advisors: false,
            }],
        },
    )
    .await
    .unwrap();
    let mut available = seeds(&f.pool, db_tests::TERM, date).await.unwrap();
    available.extend(
        specials::seeds(&f.pool, db_tests::TERM, date)
            .await
            .unwrap(),
    );
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with(
            (*f.pool.connect_options())
                .clone()
                .options([("search_path", format!("{schema},public"))]),
        )
        .await
        .unwrap();
    let mut contexts = Vec::new();
    for kind in [
        AttendanceKind::Arrival,
        AttendanceKind::Lesson,
        AttendanceKind::Special,
    ] {
        let seed = available
            .iter()
            .find(|s| {
                s.session.kind == kind
                    && s.students.iter().any(|s| s.student_id == db_tests::STUDENT)
            })
            .unwrap()
            .clone();
        let context = OpenAttendanceSession {
            academic_term_id: db_tests::TERM,
            date,
            kind,
            source_key: seed.session.source_key.clone(),
        };
        let preview =
            tokio::time::timeout(Duration::from_secs(3), open(&pool, &actor, context.clone()))
                .await
                .unwrap()
                .unwrap();
        let explicit = preview_seed(&pool, &actor, seed.clone()).await.unwrap();
        assert_eq!(
            serde_json::to_value(&preview).unwrap(),
            serde_json::to_value(explicit).unwrap()
        );
        assert_eq!(preview.session.row_version, 0);
        assert!(!preview.session.roster_token.is_empty());
        let stranger = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ATTENDANCE_READ_ASSIGNED.into()],
        };
        assert!(matches!(
            open(&pool, &stranger, context.clone()).await,
            Err(AppError::Forbidden(_))
        ));
        contexts.push((context, seed, preview));
    }
    let (rounds, records): (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM attendance_sessions),(SELECT count(*) FROM attendance_records)").fetch_one(&pool).await.unwrap();
    assert_eq!((rounds, records), (0, 0));
    for (context, seed, original) in &mut contexts {
        // A persisted occurrence retains its own roster even if today's seed includes more pupils.
        seed.students.retain(|s| s.student_id == db_tests::STUDENT);
        let saved = sessions::ensure(&pool, &actor, seed.clone()).await.unwrap();
        let preview = open(&pool, &actor, context.clone()).await.unwrap();
        assert_eq!(preview.session.roster.len(), 1);
        assert_eq!(preview.session.roster[0].student_id, db_tests::STUDENT);
        assert_ne!(preview.session.row_version, 0);
        assert_eq!(preview.session.roster_token, saved.roster_token);
        assert_eq!(preview.students.len(), 1);
        assert_eq!(preview.session.id, original.session.id);
        sqlx::query("UPDATE attendance_sessions SET cancelled=true,cancellation_reason='local fixture' WHERE id=$1").bind(saved.id).execute(&pool).await.unwrap();
        let cancelled = open(&pool, &actor, context.clone()).await.unwrap();
        assert!(!cancelled.writable && !cancelled.counted && cancelled.session.cancelled);
    }
    // Generation changes affect tokens, without storing a new attendance occurrence.
    let before = open(&pool, &actor, contexts[0].0.clone()).await.unwrap();
    sqlx::query("UPDATE attendance_module_state SET generation=gen_random_uuid() WHERE singleton")
        .execute(&pool)
        .await
        .unwrap();
    let after = open(&pool, &actor, contexts[0].0.clone()).await.unwrap();
    assert_ne!(before.session.roster_token, after.session.roster_token);
    assert_eq!(before.session.id, after.session.id);
    // Both the current seed and persisted teachers remain independently authoritative.
    let assigned = ActorContext {
        user_id: db_tests::TEACHER,
        permissions: vec![codes::ATTENDANCE_READ_ASSIGNED.into()],
    };
    let mut forbidden_seed = contexts[0].1.clone();
    forbidden_seed.session.teacher_ids.clear();
    assert!(matches!(
        preview_seed(&pool, &assigned, forbidden_seed).await,
        Err(AppError::Forbidden(_))
    ));
    sqlx::query("UPDATE attendance_sessions SET teacher_ids='{}' WHERE id=$1")
        .bind(after.session.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        open(&pool, &assigned, contexts[0].0.clone()).await,
        Err(AppError::Forbidden(_))
    ));
    sqlx::query(
        "UPDATE attendance_sessions SET cancelled=false,cancellation_reason=NULL WHERE id=$1",
    )
    .bind(after.session.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO attendance_term_archives(academic_term_id,archived_by,record_count,counted_record_count,settings_snapshot) VALUES($1,$2,0,0,'{}')")
        .bind(db_tests::TERM).bind(db_tests::TEACHER).execute(&pool).await.unwrap();
    assert!(
        !open(&pool, &actor, contexts[0].0.clone())
            .await
            .unwrap()
            .writable
    );
    sqlx::query("DELETE FROM attendance_term_archives WHERE academic_term_id=$1")
        .bind(db_tests::TERM)
        .execute(&pool)
        .await
        .unwrap();
    let lesson = contexts
        .iter()
        .find(|c| c.0.kind == AttendanceKind::Lesson)
        .unwrap()
        .0
        .clone();
    // A connection-local view supplies unavailable source graphs without mutating immutable versions.
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TEMP VIEW academic_delivery_versions AS SELECT id,'draft'::text status,snapshot FROM \"{schema}\".academic_delivery_versions")))
        .execute(&mut *connection).await.unwrap();
    drop(connection);
    assert!(matches!(
        open(&pool, &actor, lesson.clone()).await,
        Err(AppError::Conflict(_))
    ));
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE OR REPLACE TEMP VIEW academic_delivery_versions AS SELECT id,'draft'::text status,snapshot FROM \"{schema}\".academic_delivery_versions WHERE false")))
        .execute(&mut *connection).await.unwrap();
    drop(connection);
    assert!(matches!(
        open(&pool, &actor, lesson).await,
        Err(AppError::Conflict(_))
    ));
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("DROP VIEW pg_temp.academic_delivery_versions")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(db_tests::TERM)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !open(&pool, &actor, contexts[0].0.clone())
            .await
            .unwrap()
            .writable
    );
    pool.close().await;
    f.pool.close().await;
}

#[tokio::test]
async fn attendance_read_snapshot_settings_defaults_decode_and_plan() {
    let f = db_tests::fixture("attendance_read_snapshot_settings").await;
    sqlx::query("DELETE FROM attendance_settings WHERE academic_term_id=$1")
        .bind(db_tests::TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    let default = AttendanceConfiguration::default();
    let absent = settings::get(&f.pool, db_tests::TERM).await.unwrap();
    assert_eq!(absent.row_version, 0);
    assert_eq!(
        serde_json::to_value(&absent.configuration).unwrap(),
        serde_json::to_value(&default).unwrap()
    );
    let unknown = settings::get(&f.pool, Uuid::new_v4()).await.unwrap();
    assert_eq!(unknown.row_version, 0);
    assert!(!unknown.archived);
    assert_eq!(unknown.auto_finalize_from, absent.auto_finalize_from);
    let mut custom = default.clone();
    custom.enabled = true;
    custom.late_after = NaiveTime::from_hms_opt(9, 17, 0).unwrap();
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration,row_version,updated_by) VALUES($1,$2,7,$3)")
        .bind(db_tests::TERM).bind(Json(&custom)).bind(db_tests::TEACHER).execute(&f.pool).await.unwrap();
    let stored = settings::get(&f.pool, db_tests::TERM).await.unwrap();
    assert_eq!(stored.row_version, 7);
    assert_eq!(stored.configuration.late_after, custom.late_after);
    sqlx::query("INSERT INTO attendance_term_archives(academic_term_id,archived_by,record_count,counted_record_count,settings_snapshot) VALUES($1,$2,0,0,'{}')")
        .bind(db_tests::TERM).bind(db_tests::TEACHER).execute(&f.pool).await.unwrap();
    assert!(
        settings::get(&f.pool, db_tests::TERM)
            .await
            .unwrap()
            .archived
    );
    // A legacy revision-zero row still decodes, then selects the default configuration.
    sqlx::query(
        "ALTER TABLE attendance_settings DROP CONSTRAINT attendance_settings_row_version_check",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE attendance_settings SET row_version=0 WHERE academic_term_id=$1")
        .bind(db_tests::TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    let zero = settings::get(&f.pool, db_tests::TERM).await.unwrap();
    assert_eq!(
        serde_json::to_value(zero.configuration).unwrap(),
        serde_json::to_value(default).unwrap()
    );
    sqlx::query("UPDATE attendance_settings SET configuration='{}' WHERE academic_term_id=$1")
        .bind(db_tests::TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        settings::get(&f.pool, db_tests::TERM).await,
        Err(AppError::DbError(sqlx::Error::ColumnDecode { .. }))
    ));
    sqlx::query(
        "UPDATE attendance_settings SET configuration=$2,row_version=7 WHERE academic_term_id=$1",
    )
    .bind(db_tests::TERM)
    .bind(Json(&custom))
    .execute(&f.pool)
    .await
    .unwrap();
    let mut connection = f.pool.acquire().await.unwrap();
    settings::get_on_connection(&mut connection, db_tests::TERM)
        .await
        .unwrap();
    // Explain the actual owner's prepared statement, avoiding a copied SQL implementation.
    let statement: String = sqlx::query_scalar("SELECT statement FROM pg_prepared_statements WHERE statement LIKE 'SELECT s.configuration,s.row_version,EXISTS%' LIMIT 1").fetch_one(&mut *connection).await.unwrap();
    let Json(plan): Json<serde_json::Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {statement}"
    )))
    .bind(db_tests::TERM)
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    assert_eq!(plan[0]["Plan"]["Actual Rows"].as_f64(), Some(1.0));
    eprintln!(
        "attendance_read_snapshot settings_plan planning_ms={} execution_ms={} rows=1 shared_hit_blocks={} shared_read_blocks={}",
        plan[0]["Planning Time"].as_f64().unwrap(),
        plan[0]["Execution Time"].as_f64().unwrap(),
        plan[0]["Plan"]["Shared Hit Blocks"].as_u64().unwrap(),
        plan[0]["Plan"]["Shared Read Blocks"].as_u64().unwrap()
    );
    drop(connection);
    sqlx::query("DELETE FROM attendance_module_state")
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        settings::get(&f.pool, db_tests::TERM).await,
        Err(AppError::DbError(sqlx::Error::RowNotFound))
    ));
    f.pool.close().await;
}
