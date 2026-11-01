use school_academic_delivery::services::versions::{
    assigned_group_ids_on_connection, group_teacher_assignments_on_connection,
};
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use sqlx::PgPool;
use uuid::Uuid;

const FIRST: Uuid = Uuid::from_u128(0x50000000000000000000000000000002);
const SECOND: Uuid = Uuid::from_u128(0x50000000000000000000000000000003);

struct Fixture {
    pool: PgPool,
    year: Uuid,
    term: Uuid,
    offering: Uuid,
    groups: Vec<Uuid>,
    live_only: Uuid,
    future: Uuid,
}

async fn fixture(name: &str) -> Fixture {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 151).await.unwrap();
    let source: Uuid = sqlx::query_scalar("SELECT learning_offering_id FROM course_offering_details ORDER BY learning_offering_id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let term = Uuid::now_v7();
    let offering = Uuid::now_v7();
    let live_only = Uuid::now_v7();
    let future = Uuid::now_v7();
    let groups = vec![Uuid::now_v7(), Uuid::now_v7()];
    let mut tx = pool.begin().await.unwrap();
    let year: Uuid = sqlx::query_scalar("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT $1,offering.academic_year_id,'Assignment IDs',1000,'ASSIGN-IDS','custom',false,false,'planning',term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms term ON term.id=offering.academic_term_id WHERE offering.id=$2 RETURNING academic_year_id")
        .bind(term).bind(source).fetch_one(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,starts_on,owning_organization_unit_id) SELECT $1,$2,academic_year_id,'course','ASSIGN-IDS','Assignment IDs','draft',current_date-40,owning_organization_unit_id FROM learning_offerings WHERE id=$3")
        .bind(offering).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3")
        .bind(offering).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) SELECT id,id::text,'fixture-not-a-login','Assignment','Fixture','staff','active' FROM unnest($1::uuid[]) id")
        .bind(vec![live_only,future]).execute(&mut *tx).await.unwrap();
    for (index, group) in groups.iter().enumerate() {
        sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) VALUES($1,$2,$3,$4,$5,$5,'draft','draft')")
            .bind(group).bind(offering).bind(term).bind(year).bind(format!("ASSIGN-{index}")).execute(&mut *tx).await.unwrap();
        for teacher in [FIRST, SECOND] {
            sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,ends_on) VALUES(uuidv7(),$1,$2,$3,$4,'primary',current_date-40,current_date-30)")
                .bind(group).bind(term).bind(year).bind(teacher).execute(&mut *tx).await.unwrap();
        }
        sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on) VALUES(uuidv7(),$1,$2,$3,$4,'secondary',current_date-5),(uuidv7(),$1,$2,$3,$5,'primary',current_date+20)")
            .bind(group).bind(term).bind(year).bind(live_only).bind(future).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("UPDATE learning_groups SET status='published' WHERE id=ANY($1)")
        .bind(&groups)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_offerings SET status='published',published_at=now() WHERE id=$1")
        .bind(offering)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Fixture {
        pool,
        year,
        term,
        offering,
        groups,
        live_only,
        future,
    }
}

async fn graph(f: &Fixture, connection: &mut sqlx::PgConnection, offset: i32) -> serde_json::Value {
    sqlx::query_scalar("SELECT academic_capture_delivery_snapshot(academic_term_id,current_date+$2,jsonb_build_array(jsonb_build_object('id',id,'weekly_period_target',1))) FROM learning_offerings WHERE id=$1")
        .bind(f.offering).bind(offset).fetch_one(connection).await.unwrap()
}

async fn version(
    f: &Fixture,
    connection: &mut sqlx::PgConnection,
    offset: i32,
    published: bool,
    graph: &serde_json::Value,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_delivery_versions(publication_no,id,academic_term_id,academic_year_id,effective_from,reference_date,status,snapshot,published_by,published_at,publication_idempotency_key,publication_request_hash) VALUES(CASE WHEN $4 THEN (SELECT COALESCE(max(publication_no),0)+1 FROM academic_delivery_versions WHERE academic_term_id=$2) ELSE NULL END,$1,$2,$3,CASE WHEN $4 THEN current_date+$5 ELSE NULL END,CASE WHEN $4 THEN NULL ELSE current_date+$5 END,CASE WHEN $4 THEN 'published' ELSE 'draft' END,$6,CASE WHEN $4 THEN $7::uuid ELSE NULL END,CASE WHEN $4 THEN now() ELSE NULL END,CASE WHEN $4 THEN uuidv7() ELSE NULL END,CASE WHEN $4 THEN repeat('a',64) ELSE NULL END)")
        .bind(id).bind(f.term).bind(f.year).bind(published).bind(offset).bind(graph).bind(FIRST).execute(connection).await.unwrap();
    id
}

#[tokio::test]
async fn teacher_assignments_share_latest_source_availability_and_active_primary_sets() {
    let f = fixture("delivery_teacher_assignment_ids").await;
    let mut connection = f.pool.acquire().await.unwrap();
    let missing =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &f.groups)
            .await
            .unwrap();
    assert_eq!(missing.delivery_version_id, None);
    assert!(missing.groups.is_empty());
    let no_fallback =
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.live_only)
            .await
            .unwrap();
    assert_eq!(no_fallback.delivery_version_id, None);
    assert!(no_fallback.learning_group_ids.is_empty());

    let original_graph = graph(&f, &mut connection, -35).await;
    let published = version(&f, &mut connection, -35, true, &original_graph).await;
    let assignments =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &f.groups)
            .await
            .unwrap();
    assert_eq!(assignments.delivery_version_id, Some(published));
    assert_eq!(assignments.groups.len(), 2);
    for group in &assignments.groups {
        assert_eq!(group.teacher_ids, vec![FIRST, SECOND]);
        assert_eq!(group.primary_teacher_ids, vec![FIRST, SECOND]);
    }
    let actor = assigned_group_ids_on_connection(&mut connection, f.year, f.term, FIRST)
        .await
        .unwrap();
    assert_eq!(actor.delivery_version_id, Some(published));
    assert_eq!(actor.learning_group_ids, f.groups);
    let absent_actor =
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.live_only)
            .await
            .unwrap();
    assert_eq!(absent_actor.delivery_version_id, Some(published));
    assert!(absent_actor.learning_group_ids.is_empty());
    let empty_request =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &[])
            .await
            .unwrap();
    assert_eq!(empty_request.delivery_version_id, Some(published));
    assert!(empty_request.groups.is_empty());
    let wrong_year =
        group_teacher_assignments_on_connection(&mut connection, Uuid::new_v4(), f.term, &f.groups)
            .await
            .unwrap();
    assert_eq!(wrong_year.delivery_version_id, None);
    assert!(wrong_year.groups.is_empty());
    let foreign_group: Uuid = sqlx::query_scalar(
        "SELECT id FROM learning_groups WHERE academic_term_id<>$1 ORDER BY id LIMIT 1",
    )
    .bind(f.term)
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    let wrong_group =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &[foreign_group])
            .await
            .unwrap();
    assert_eq!(wrong_group.delivery_version_id, Some(published));
    assert!(wrong_group.groups.is_empty());
    let bound_groups: i64 = sqlx::query_scalar("SELECT count(*) FROM jsonb_to_recordset($1::jsonb) AS membership(learning_group_id uuid,teacher_ids uuid[],primary_teacher_ids uuid[]) WHERE $2=ANY(teacher_ids) AND $3=ANY(primary_teacher_ids)")
        .bind(sqlx::types::Json(&assignments.groups)).bind(FIRST).bind(SECOND).fetch_one(&mut *connection).await.unwrap();
    assert_eq!(bound_groups, 2);

    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(SECOND)
        .execute(&mut *connection)
        .await
        .unwrap();
    let inactive =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &f.groups)
            .await
            .unwrap();
    assert!(inactive
        .groups
        .iter()
        .all(|group| group.teacher_ids == vec![FIRST] && group.primary_teacher_ids == vec![FIRST]));
    assert!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, SECOND)
            .await
            .unwrap()
            .learning_group_ids
            .is_empty()
    );
    sqlx::query("UPDATE users SET status='active' WHERE id=$1")
        .bind(SECOND)
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_groups SET status='closed' WHERE id=$1")
        .bind(f.groups[0])
        .execute(&mut *connection)
        .await
        .unwrap();
    assert_eq!(
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &f.groups)
            .await
            .unwrap()
            .groups
            .len(),
        1
    );
    sqlx::query("UPDATE learning_groups SET status='published' WHERE id=$1")
        .bind(f.groups[0])
        .execute(&mut *connection)
        .await
        .unwrap();

    let future_graph = graph(&f, &mut connection, 25).await;
    version(&f, &mut connection, 30, false, &future_graph).await;
    assert_eq!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, FIRST)
            .await
            .unwrap()
            .delivery_version_id,
        Some(published)
    );
    let future_published = version(&f, &mut connection, 25, true, &future_graph).await;
    let future =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &f.groups)
            .await
            .unwrap();
    assert_eq!(future.delivery_version_id, Some(future_published));
    assert!(future
        .groups
        .iter()
        .all(|group| group.primary_teacher_ids == vec![f.future]
            && group.teacher_ids.contains(&f.live_only)));
    assert!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, FIRST)
            .await
            .unwrap()
            .learning_group_ids
            .is_empty()
    );
    assert_eq!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.future)
            .await
            .unwrap()
            .learning_group_ids,
        f.groups
    );

    let empty = version(
        &f,
        &mut connection,
        40,
        true,
        &serde_json::json!({"offerings":[]}),
    )
    .await;
    let omitted =
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &f.groups)
            .await
            .unwrap();
    assert_eq!(omitted.delivery_version_id, Some(empty));
    assert!(omitted.groups.is_empty());
    let omitted_actor = assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.future)
        .await
        .unwrap();
    assert_eq!(omitted_actor.delivery_version_id, Some(empty));
    assert!(omitted_actor.learning_group_ids.is_empty());
}
