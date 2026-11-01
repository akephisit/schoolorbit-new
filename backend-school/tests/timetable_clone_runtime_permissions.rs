#[path = "../src/adapters/publication_history.rs"]
mod publication_history;
use std::collections::HashSet;

use school_academic_timetable::{
    collaboration::TimetableMutationContext,
    models::timetable_version::{CloneTimetableVersionRequest, CreateTimetableVersionRequest},
    services::timetable_version_service as tables,
};
use school_errors::AppError;
use school_test_db::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
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

async fn graph(pool: &PgPool, version_id: Uuid) -> Value {
    sqlx::query_scalar(
        r#"WITH blocks AS MATERIALIZED (
               SELECT * FROM academic_timetable_blocks WHERE timetable_version_id = $1
           ), groups AS MATERIALIZED (
               SELECT child.* FROM academic_timetable_block_groups child
               JOIN blocks ON blocks.id = child.block_id
           )
           SELECT jsonb_build_object(
               'version', (SELECT to_jsonb(version) FROM academic_timetable_versions version WHERE id = $1),
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
    )
    .bind(version_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn mapped_id(namespace: Uuid, prefix: &str, id: &Value) -> Value {
    json!(Uuid::new_v5(
        &namespace,
        format!("{prefix}:{}", id.as_str().unwrap()).as_bytes()
    ))
}

fn expected_clone(mut source: Value, namespace: Uuid, actor: Uuid) -> Value {
    let source_id = source["version"]["id"].clone();
    let blocks: HashSet<Value> = source["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["is_active"] == true)
        .map(|row| row["id"].clone())
        .collect();
    let groups: HashSet<Value> = source["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| blocks.contains(&row["block_id"]) && row["is_active"] == true)
        .map(|row| row["id"].clone())
        .collect();
    source.as_object_mut().unwrap().remove("version");
    for (kind, rows) in source.as_object_mut().unwrap() {
        let rows = rows.as_array_mut().unwrap();
        rows.retain(|row| match kind.as_str() {
            "blocks" => blocks.contains(&row["id"]),
            "groups" => groups.contains(&row["id"]),
            "instructors" => groups.contains(&row["block_group_id"]),
            "sync" => blocks.contains(&row["block_id"]),
            _ => blocks.contains(&row["block_id"]) && row["is_active"] == true,
        });
        for row in rows {
            let row = row.as_object_mut().unwrap();
            let old_id = row["id"].clone();
            let (prefix, provenance_key) = match kind.as_str() {
                "blocks" => ("block", "clonedFromBlockId"),
                "groups" => ("block-group", "clonedFromBlockGroupId"),
                "homerooms" => ("homeroom-target", "clonedFromTargetId"),
                "teachers" => ("teacher-target", "clonedFromTargetId"),
                "sync" => ("sync", ""),
                "instructors" => ("", ""),
                _ => unreachable!(),
            };
            row.insert("id".into(), mapped_id(namespace, prefix, &old_id));
            if let Some(parent) = row.get_mut("block_id") {
                *parent = mapped_id(namespace, "block", parent);
            }
            if let Some(parent) = row.get_mut("block_group_id") {
                *parent = mapped_id(namespace, "block-group", parent);
            }
            if let Some(link) = row.get_mut("linked_block_group_id") {
                *link = if groups.contains(link) {
                    mapped_id(namespace, "block-group", link)
                } else {
                    Value::Null
                };
            }
            if kind == "blocks" {
                row.insert("timetable_version_id".into(), json!(namespace));
                if !row["series_id"].is_null() {
                    row.insert(
                        "series_id".into(),
                        mapped_id(namespace, "series", &row["series_id"]),
                    );
                }
            }
            if let Some(provenance) = row.get_mut("migration_provenance") {
                let provenance = provenance.as_object_mut().unwrap();
                provenance.insert(provenance_key.into(), old_id);
                provenance.insert("sourceVersionId".into(), source_id.clone());
            }
            row.insert("row_version".into(), json!(1));
            for field in ["created_by", "updated_by"] {
                if let Some(value) = row.get_mut(field) {
                    *value = json!(actor);
                }
            }
        }
    }
    comparable_graph(source)
}

fn comparable_graph(mut graph: Value) -> Value {
    graph.as_object_mut().unwrap().remove("version");
    for (kind, rows) in graph.as_object_mut().unwrap() {
        let rows = rows.as_array_mut().unwrap();
        for row in &mut *rows {
            let row = row.as_object_mut().unwrap();
            row.remove("created_at");
            row.remove("updated_at");
            if kind == "instructors" {
                row.remove("id"); // Instructor row IDs are intentionally newly random.
            }
        }
        rows.sort_by_cached_key(Value::to_string);
    }
    graph
}

async fn table_counts(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar(
        r#"SELECT ARRAY[
               (SELECT count(*) FROM academic_timetable_versions),
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
async fn clone_without_temp_preserves_graph_and_other_term() {
    let admin = school_test_db::create_named_test_pool("timetable_clone_runtime_admin").await;
    let identity = Uuid::new_v4().simple().to_string();
    let database = format!("clone_fixture_{identity}");
    let role = format!("clone_runtime_{identity}");
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
    let (source_id, actor, term_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT id,published_by,academic_term_id FROM academic_timetable_versions WHERE status='published' AND academic_term_id='11000000-0000-0000-0000-000000000251' ORDER BY id LIMIT 1",
    ).fetch_one(&owner).await.unwrap();
    let source = tables::get_version(&owner, source_id, chrono::Utc::now().date_naive())
        .await
        .unwrap();
    let other_term: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 AND sequence_no=2",
    )
    .bind(source.academic_year_id)
    .fetch_one(&owner)
    .await
    .unwrap();
    assert_ne!(term_id, other_term);
    sqlx::query("UPDATE academic_terms SET bell_schedule_id=$1 WHERE id=$2")
        .bind(source.bell_schedule_id)
        .bind(other_term)
        .execute(&owner)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO academic_delivery_versions (
               academic_term_id,academic_year_id,effective_from,snapshot,status,
               created_by,published_by,published_at,publication_idempotency_key,publication_request_hash
           ) VALUES ($1,$2,'2025-11-01','{"offerings":[]}','published',
                     $3,$3,now(),gen_random_uuid(),repeat('a',64))"#,
    )
    .bind(other_term)
    .bind(source.academic_year_id)
    .bind(actor)
    .execute(&owner)
    .await
    .unwrap();
    let other_draft = tables::create_initial_draft(
        &owner,
        &TimetableMutationContext::for_tests(&owner, actor),
        CreateTimetableVersionRequest {
            academic_term_id: other_term,
        },
    )
    .await
    .unwrap();
    // Give the other term content, so accidental child copying/updating is observable.
    sqlx::query(
        r#"INSERT INTO academic_timetable_blocks (
               timetable_version_id,academic_term_id,academic_year_id,bell_schedule_id,
               bell_schedule_period_id,day_of_week,block_kind,structural_kind,title,created_by
           ) SELECT $1,$2,academic_year_id,bell_schedule_id,bell_schedule_period_id,
                    day_of_week,'STRUCTURAL','OTHER','Other term draft',$3
             FROM academic_timetable_blocks WHERE timetable_version_id=$4 ORDER BY id LIMIT 1"#,
    )
    .bind(other_draft.id)
    .bind(other_term)
    .bind(actor)
    .bind(source_id)
    .execute(&owner)
    .await
    .unwrap();
    let source_before = graph(&owner, source_id).await;
    let other_before = graph(&owner, other_draft.id).await;
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
            "fixture must exercise {kind}"
        );
    }
    assert!(source_before["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["is_active"] == false));

    // Only this owned disposable database loses PUBLIC TEMP. The pool's hook applies
    // SET ROLE to every physical connection, including service-created transactions.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        r#"CREATE ROLE "{role}" NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
           REVOKE ALL ON DATABASE "{database}" FROM PUBLIC;
           GRANT CONNECT ON DATABASE "{database}" TO "{role}";
           REVOKE CREATE ON SCHEMA public FROM PUBLIC;
           GRANT USAGE ON SCHEMA public TO "{role}";
           GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO "{role}";
           REVOKE INSERT,UPDATE,DELETE ON _sqlx_migrations FROM "{role}";
           GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO "{role}";"#,
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
        let (current_role, temporary, create_schema, superuser): (String, bool, bool, bool) =
            sqlx::query_as("SELECT current_user::text,has_database_privilege(current_user,current_database(),'TEMP'),has_schema_privilege(current_user,'public','CREATE'),(SELECT rolsuper FROM pg_roles WHERE rolname=current_user)")
                .fetch_one(&mut **connection).await.unwrap();
        assert_eq!(current_role, role);
        assert!(!temporary && !create_schema && !superuser);
    }
    let denied = sqlx::query("CREATE TEMP TABLE fixture_temp_must_be_denied(id integer)")
        .execute(&mut *first)
        .await
        .unwrap_err();
    assert_eq!(
        denied.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    drop(first);
    drop(second);

    let request = || CloneTimetableVersionRequest {
        source_row_version: source.row_version,
        resume_draft_id: None,
        draft_row_version: None,
    };
    // Force a failure at the final audit, after every child INSERT, to prove rollback.
    sqlx::raw_sql(
        r#"CREATE FUNCTION fixture_reject_clone_audit() RETURNS trigger LANGUAGE plpgsql AS $$
           BEGIN
               IF NEW.event_code='academic_timetable_version.draft_created' THEN
                   RAISE EXCEPTION 'fixture_clone_audit_failure' USING ERRCODE='23514';
               END IF;
               RETURN NEW;
           END $$;
           CREATE TRIGGER fixture_reject_clone_audit BEFORE INSERT ON academic_audit_events
           FOR EACH ROW EXECUTE FUNCTION fixture_reject_clone_audit();"#,
    )
    .execute(&owner)
    .await
    .unwrap();
    let counts_before = table_counts(&owner).await;
    let failure = tables::clone_draft(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        source_id,
        request(),
    )
    .await
    .unwrap_err();
    assert_eq!(failure.database_code().as_deref(), Some("23514"));
    assert_eq!(table_counts(&owner).await, counts_before);
    assert_eq!(graph(&owner, source_id).await, source_before);
    assert_eq!(graph(&owner, other_draft.id).await, other_before);
    sqlx::raw_sql("DROP TRIGGER fixture_reject_clone_audit ON academic_audit_events; DROP FUNCTION fixture_reject_clone_audit();")
        .execute(&owner).await.unwrap();

    let draft = tables::clone_draft(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        source_id,
        request(),
    )
    .await
    .unwrap();
    assert_eq!(draft.source_version_id, Some(source_id));
    assert_eq!(draft.academic_term_id, term_id);
    assert_eq!(draft.delivery_version_id, source.delivery_version_id);
    assert_eq!(draft.effective_from, None);
    assert_eq!(
        comparable_graph(graph(&owner, draft.id).await),
        expected_clone(source_before.clone(), draft.id, actor),
    );
    let draft_before = graph(&owner, draft.id).await;
    let resumed = tables::clone_draft(
        &runtime,
        &TimetableMutationContext::for_tests(&runtime, actor),
        source_id,
        request(),
    )
    .await
    .unwrap();
    assert_eq!(resumed.id, draft.id);
    assert_eq!(graph(&owner, draft.id).await, draft_before);
    let mut stale = request();
    stale.source_row_version += 1;
    assert!(matches!(
        tables::clone_draft(
            &runtime,
            &TimetableMutationContext::for_tests(&runtime, actor),
            source_id,
            stale,
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_timetable_version.draft_created'")
        .bind(draft.id).fetch_one(&owner).await.unwrap();
    assert_eq!(audits, 1);
    assert_eq!(graph(&owner, source_id).await, source_before);
    assert_eq!(graph(&owner, other_draft.id).await, other_before);
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
