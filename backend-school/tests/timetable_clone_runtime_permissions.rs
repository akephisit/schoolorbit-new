use school_academic_timetable::{
    collaboration::TimetableMutationContext,
    models::{timetable::CreateTimetableRequest, timetable_block::*},
    services::{timetable_block_service as blocks, timetable_service as tables},
};
use school_errors::AppError;
use school_test_db::cutover_test_support::{
    CutoverFixture, apply_migrations_through, apply_phase_b_runtime_migrations,
    seed_academic_cutover_fixture,
};
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

async fn seed_source_graph(pool: &PgPool) {
    apply_migrations_through(pool, 40).await.unwrap();
    seed_academic_cutover_fixture(pool, CutoverFixture::Passing)
        .await
        .unwrap();
    // Extend the historical fixture before immutable publication guards exist.
    // Normal migrations produce the published graph; no constraints are disabled.
    sqlx::raw_sql(
        r#"UPDATE academic_timetable_entries
           SET batch_id = '93000000-0000-0000-0000-000000000099';
           INSERT INTO academic_timetable_entries (
               id, day_of_week, period_id, entry_type, classroom_id,
               academic_semester_id, activity_slot_id, batch_id, is_active, created_by
           )
           SELECT entry.id, entry.day, source.period_id, entry.kind, source.classroom_id,
                  source.academic_semester_id, entry.slot_id,
                  '93000000-0000-0000-0000-000000000099', entry.active, source.created_by
           FROM academic_timetable_entries source
           CROSS JOIN (VALUES
               ('93000000-0000-0000-0000-000000000001'::uuid, 'TUE', 'HOMEROOM', NULL::uuid, true),
               ('93000000-0000-0000-0000-000000000002'::uuid, 'WED', 'HOMEROOM', NULL::uuid, false),
               ('93000000-0000-0000-0000-000000000003'::uuid, 'THU', 'ACTIVITY',
                '70000000-0000-0000-0000-000000000001'::uuid, true),
               ('93000000-0000-0000-0000-000000000004'::uuid, 'FRI', 'ACTIVITY',
                '70000000-0000-0000-0000-000000000001'::uuid, false)
           ) entry(id, day, kind, slot_id, active)
           WHERE source.id = '83000000-0000-0000-0000-000000000001';
           INSERT INTO timetable_entry_instructors(id, entry_id, instructor_id, role)
           SELECT gen_random_uuid(), id, created_by, 'primary'
           FROM academic_timetable_entries
           WHERE id IN ('93000000-0000-0000-0000-000000000001',
                        '93000000-0000-0000-0000-000000000003',
                        '93000000-0000-0000-0000-000000000004');
           INSERT INTO timetable_entry_instructors(id, entry_id, instructor_id, role)
           VALUES (gen_random_uuid(), '83000000-0000-0000-0000-000000000001',
                   '50000000-0000-0000-0000-000000000003', 'secondary');"#,
    )
    .execute(pool)
    .await
    .unwrap();
    apply_phase_b_runtime_migrations(pool).await.unwrap();
    apply_migrations_through(pool, school_migrations::latest_migration_version())
        .await
        .unwrap();
}

async fn graph(pool: &PgPool, timetable: Uuid) -> Value {
    sqlx::query_scalar(
        r#"WITH blocks AS MATERIALIZED (
               SELECT * FROM academic_timetable_blocks WHERE timetable_id = $1
           ), groups AS MATERIALIZED (
               SELECT child.* FROM academic_timetable_block_groups child
               JOIN blocks ON blocks.id = child.block_id
           )
           SELECT jsonb_build_object(
               'timetable', (SELECT to_jsonb(current) FROM academic_timetables current WHERE id = $1),
               'blocks', COALESCE((SELECT jsonb_agg(to_jsonb(blocks) ORDER BY id) FROM blocks), '[]'),
               'groups', COALESCE((SELECT jsonb_agg(to_jsonb(groups) ORDER BY id) FROM groups), '[]'),
               'instructors', COALESCE((SELECT jsonb_agg(to_jsonb(child) ORDER BY child.id)
                   FROM academic_timetable_block_group_instructors child
                   JOIN groups ON groups.id = child.block_group_id), '[]'),
               'homerooms', COALESCE((SELECT jsonb_agg(to_jsonb(child) ORDER BY child.id)
                   FROM academic_timetable_block_homerooms child JOIN blocks ON blocks.id = child.block_id), '[]'),
               'teachers', COALESCE((SELECT jsonb_agg(to_jsonb(child) ORDER BY child.id)
                   FROM academic_timetable_block_teachers child JOIN blocks ON blocks.id = child.block_id), '[]'),
               'sync', COALESCE((SELECT jsonb_agg(to_jsonb(child) ORDER BY child.id)
                   FROM academic_timetable_block_group_sync child JOIN blocks ON blocks.id = child.block_id), '[]')
           )"#,
    ).bind(timetable).fetch_one(pool).await.unwrap()
}

async fn table_counts(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar(
        r#"SELECT ARRAY[
               (SELECT count(*) FROM academic_timetables),
               (SELECT count(*) FROM academic_timetable_blocks),
               (SELECT count(*) FROM academic_timetable_block_groups),
               (SELECT count(*) FROM academic_timetable_block_group_instructors),
               (SELECT count(*) FROM academic_timetable_block_homerooms),
               (SELECT count(*) FROM academic_timetable_block_teachers),
               (SELECT count(*) FROM academic_timetable_block_group_sync),
               (SELECT count(*) FROM academic_audit_events)]"#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn current_singleton_and_direct_edits_need_only_runtime_dml_and_preserve_other_term() {
    let admin = school_test_db::create_named_test_pool("timetable_current_runtime_admin").await;
    let identity = Uuid::new_v4().simple().to_string();
    let database = format!("current_fixture_{identity}");
    let role = format!("current_runtime_{identity}");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{database}\""
    )))
    .execute(&admin)
    .await
    .unwrap();
    let options = admin.connect_options().as_ref().clone().database(&database);
    let owner = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    seed_source_graph(&owner).await;
    let (source, actor, source_term, year, bell): (Uuid, Uuid, Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT timetable.id, '50000000-0000-0000-0000-000000000002'::uuid, timetable.academic_term_id,timetable.academic_year_id,timetable.bell_schedule_id FROM academic_timetables timetable JOIN academic_terms term ON term.id=timetable.academic_term_id WHERE term.status='active' ORDER BY timetable.id LIMIT 1",
    ).fetch_one(&owner).await.unwrap();
    let target_term: Uuid = sqlx::query_scalar("SELECT id FROM academic_terms WHERE academic_year_id=$1 AND id<>$2 ORDER BY sequence_no LIMIT 1").bind(year).bind(source_term).fetch_one(&owner).await.unwrap();
    sqlx::query("UPDATE academic_terms SET bell_schedule_id=$1,status='planning' WHERE id=$2")
        .bind(bell)
        .bind(target_term)
        .execute(&owner)
        .await
        .unwrap();
    assert!(
        tables::get_for_term(&owner, target_term)
            .await
            .unwrap()
            .is_none(),
        "unused term stays unopened until explicit create"
    );
    let source_before = graph(&owner, source).await;
    for kind in [
        "blocks",
        "groups",
        "instructors",
        "homerooms",
        "teachers",
        "sync",
    ] {
        assert!(
            !source_before[kind].as_array().unwrap().is_empty(),
            "retained current fixture exercises {kind}"
        );
    }
    assert!(
        source_before["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["is_active"] == true),
        "retired inactive placements were physically removed at cutover"
    );

    // The disposable runtime role has ordinary DML only on both physical connections.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        r#"
        CREATE ROLE "{role}" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOBYPASSRLS;
        REVOKE ALL ON DATABASE "{database}" FROM PUBLIC;
        GRANT CONNECT ON DATABASE "{database}" TO "{role}";
        REVOKE CREATE ON SCHEMA public FROM PUBLIC;
        GRANT USAGE ON SCHEMA public TO "{role}";
        GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO "{role}";
        REVOKE INSERT,UPDATE,DELETE ON _sqlx_migrations FROM "{role}";
        GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO "{role}";
    "#
    )))
    .execute(&owner)
    .await
    .unwrap();
    let runtime_role = role.clone();
    let runtime = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let role = runtime_role.clone();
            Box::pin(async move {
                sqlx::query(sqlx::AssertSqlSafe(format!("SET ROLE \"{role}\"")))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .unwrap();
    let mut first = runtime.acquire().await.unwrap();
    let mut second = runtime.acquire().await.unwrap();
    for connection in [&mut first, &mut second] {
        let (current_role,temporary,create_schema,superuser,bypass_rls):(String,bool,bool,bool,bool)=sqlx::query_as("SELECT current_user::text,has_database_privilege(current_user,current_database(),'TEMP'),has_schema_privilege(current_user,'public','CREATE'),rolsuper,rolbypassrls FROM pg_roles WHERE rolname=current_user").fetch_one(&mut **connection).await.unwrap();
        assert_eq!(current_role, role);
        assert!(!temporary && !create_schema && !superuser && !bypass_rls);
    }
    for statement in [
        "CREATE TEMP TABLE fixture_temp_must_be_denied(id integer)",
        "CREATE TABLE public.fixture_ddl_must_be_denied(id integer)",
        "DELETE FROM _sqlx_migrations",
    ] {
        let denied = sqlx::query(statement)
            .execute(&mut *first)
            .await
            .unwrap_err();
        assert_eq!(
            denied.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
    }
    drop(first);
    drop(second);

    // A late failure in singleton INSERT must roll back everything without DDL/TEMP.
    sqlx::raw_sql("CREATE FUNCTION fixture_reject_current_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture_current_insert_failure' USING ERRCODE='23514'; RETURN NEW; END $$; CREATE TRIGGER fixture_reject_current_insert AFTER INSERT ON academic_timetables FOR EACH ROW EXECUTE FUNCTION fixture_reject_current_insert();").execute(&owner).await.unwrap();
    let before = table_counts(&owner).await;
    let request = || CreateTimetableRequest {
        academic_term_id: target_term,
    };
    let failed = tables::create(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        request(),
    )
    .await
    .unwrap_err();
    assert_eq!(failed.database_code().as_deref(), Some("23514"));
    assert_eq!(table_counts(&owner).await, before);
    assert_eq!(graph(&owner, source).await, source_before);
    sqlx::raw_sql("DROP TRIGGER fixture_reject_current_insert ON academic_timetables; DROP FUNCTION fixture_reject_current_insert();").execute(&owner).await.unwrap();
    let current = tables::create(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        request(),
    )
    .await
    .unwrap();
    assert_eq!(current.academic_term_id, target_term);
    assert_eq!(current.academic_year_id, year);
    assert_eq!(current.bell_schedule_id, bell);
    let initial = graph(&owner, current.id).await;
    assert_eq!(initial["blocks"], serde_json::json!([]));
    let repeated = tables::create(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        request(),
    )
    .await
    .unwrap();
    assert_eq!(repeated.id, current.id);
    assert_eq!(graph(&owner, current.id).await, initial);
    let period:Uuid=sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index LIMIT 1").bind(bell).fetch_one(&owner).await.unwrap();
    let created = blocks::create_structural_blocks(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        CreateStructuralTimetableBlocksRequest {
            timetable_id: current.id,
            academic_term_id: target_term,
            structural_kind: TimetableStructuralKind::Other,
            title: "Current runtime placement".into(),
            note: None,
            slots: vec![TimetableStructuralSlotInput {
                day_of_week: "MON".into(),
                bell_schedule_period_id: period,
            }],
            homeroom_ids: vec![],
            teacher_ids: vec![actor],
            all_homerooms: false,
            all_teachers: false,
            room_id: None,
        },
    )
    .await
    .unwrap()
    .remove(0);
    let edit = |revision| UpdateTimetableBlockRequest {
        timetable_id: current.id,
        row_version: revision,
        day_of_week: None,
        bell_schedule_period_id: None,
        title: None,
        clear_title: false,
        note: Some("Direct accepted edit".into()),
        clear_note: false,
        room_id: None,
        clear_room: false,
        instructor_ids: None,
        teacher_ids: None,
    };
    let edited = blocks::update_block(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        created.id,
        edit(created.row_version),
    )
    .await
    .unwrap();
    assert_eq!(edited.note.as_deref(), Some("Direct accepted edit"));
    assert!(edited.row_version > created.row_version);
    assert!(matches!(
        blocks::update_block(
            &runtime,
            &TimetableMutationContext::for_tests(&runtime, actor),
            created.id,
            edit(created.row_version)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        graph(&owner, source).await,
        source_before,
        "direct target edits do not copy or change the other term"
    );
    blocks::deactivate_block(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        created.id,
        current.id,
        edited.row_version,
    )
    .await
    .unwrap();
    assert!(matches!(
        blocks::get_block(&runtime, created.id).await,
        Err(AppError::NotFound(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_timetables WHERE academic_term_id=$1"
        )
        .bind(target_term)
        .fetch_one(&owner)
        .await
        .unwrap(),
        1
    );
    assert_eq!(graph(&owner, source).await, source_before);
    runtime.close().await;
    owner.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{database}\"")))
        .execute(&admin)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE \"{role}\"")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
