//! Current aggregates replace publication/clone/source-alignment workflow tests.
use super::timetable_block_service_tests::migrated_pool;
use school_academic_timetable::{
    collaboration::TimetableMutationContext,
    models::{timetable::CreateTimetableRequest, timetable_block::TimetableBlockWorkspaceQuery},
    policy::TimetableAccessFilter,
    services::{timetable_block_service, timetable_service},
};
use uuid::Uuid;

#[tokio::test]
async fn reading_an_empty_term_is_read_only_and_concurrent_creation_keeps_one_identity() {
    let pool = migrated_pool("timetable_current_singleton").await;
    let (year,bell): (Uuid,Uuid) = sqlx::query_as("SELECT academic_year_id,bell_schedule_id FROM academic_terms WHERE status='active' ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    let term = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,sequence_no,code,name,term_type,included_in_year_result,blocks_year_closure,bell_schedule_id,school_days,status) SELECT $1,$2,COALESCE(max(sequence_no),0)+1,'CURRENT-TEST','Current singleton test','regular',false,false,$3,'MON,TUE,WED,THU,FRI','planning' FROM academic_terms WHERE academic_year_id=$2").bind(term).bind(year).bind(bell).execute(&pool).await.unwrap();
    for _ in 0..2 {
        assert!(timetable_service::get_for_term(&pool, term)
            .await
            .unwrap()
            .is_none());
    }
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let mutation = TimetableMutationContext::for_tests(&pool, actor);
    let first = timetable_service::create(
        &pool,
        &mutation,
        CreateTimetableRequest {
            academic_term_id: term,
        },
    );
    let second = timetable_service::create(
        &pool,
        &mutation,
        CreateTimetableRequest {
            academic_term_id: term,
        },
    );
    let (first, second) = tokio::join!(first, second);
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.row_version, second.row_version);
    assert_eq!(first.bell_schedule_id, bell);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_timetables WHERE academic_term_id=$1")
            .bind(term)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        timetable_service::get_for_term(&pool, term)
            .await
            .unwrap()
            .unwrap()
            .id,
        first.id
    );
}

#[tokio::test]
async fn current_workspace_reads_weekly_demand_without_source_alignment_or_new_aggregate() {
    let pool = migrated_pool("timetable_current_live_demand").await;
    let (id,term,year): (Uuid,Uuid,Uuid)=sqlx::query_as("SELECT timetable.id,timetable.academic_term_id,timetable.academic_year_id FROM academic_timetables timetable JOIN academic_terms term ON term.id=timetable.academic_term_id WHERE term.status='active' ORDER BY timetable.id LIMIT 1").fetch_one(&pool).await.unwrap();
    let query = || TimetableBlockWorkspaceQuery {
        academic_year_id: year,
        academic_term_id: term,
        timetable_id: id,
    };
    let access = TimetableAccessFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let before = timetable_block_service::get_workspace(
        &pool,
        query(),
        &access,
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    let demand = before.ordinary_demands.first().unwrap();
    let (revision, weekly): (i64, i32) = sqlx::query_as(
        "SELECT row_version,weekly_period_target FROM learning_offerings WHERE id=$1",
    )
    .bind(demand.learning_offering_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    school_academic_delivery::services::offerings::update_weekly_period_target(
        &pool,
        demand.learning_offering_id,
        school_academic_delivery::models::current::UpdateWeeklyPeriodTargetRequest {
            row_version: revision,
            weekly_period_target: weekly + 1,
        },
    )
    .await
    .unwrap();
    let after = timetable_block_service::get_workspace(
        &pool,
        query(),
        &access,
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    assert_eq!(after.timetable.id, before.timetable.id);
    assert!(after.delivery_row_version > before.delivery_row_version);
    assert_eq!(
        after
            .ordinary_demands
            .iter()
            .find(|row| row.learning_group_id == demand.learning_group_id)
            .unwrap()
            .required_periods,
        weekly + 1
    );
    assert_eq!(
        serde_json::to_value(&before.blocks).unwrap(),
        serde_json::to_value(&after.blocks).unwrap()
    );
    let retired: Option<String> = sqlx::query_scalar(
        "SELECT to_regclass(current_schema()||'.academic_timetable_versions')::text",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(retired.is_none());
}

#[tokio::test]
async fn removing_current_teacher_revokes_assigned_board_access_without_rewriting_placements() {
    let pool = migrated_pool("timetable_current_teacher_scope").await;
    let (id,term,year,group,teacher,offering):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT block.timetable_id,block.academic_term_id,block.academic_year_id,placed.learning_group_id,instructor.instructor_id,placed.learning_offering_id FROM academic_timetable_blocks block JOIN academic_timetable_block_groups placed ON placed.block_id=block.id AND placed.is_active JOIN academic_timetable_block_group_instructors instructor ON instructor.block_group_id=placed.id JOIN learning_group_teachers current_teacher ON current_teacher.learning_group_id=placed.learning_group_id AND current_teacher.teacher_id=instructor.instructor_id WHERE block.is_active ORDER BY block.id LIMIT 1").fetch_one(&pool).await.unwrap();
    let query = || TimetableBlockWorkspaceQuery {
        academic_year_id: year,
        academic_term_id: term,
        timetable_id: id,
    };
    let access = TimetableAccessFilter {
        assigned_actor_id: Some(teacher),
        ..Default::default()
    };
    let before = timetable_block_service::get_workspace(
        &pool,
        query(),
        &access,
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    assert!(before
        .blocks
        .iter()
        .any(|block| block.learning_offering_id == Some(offering)));
    let captured:serde_json::Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(instructor) ORDER BY instructor.id),'[]') FROM academic_timetable_block_group_instructors instructor JOIN academic_timetable_block_groups placed ON placed.id=instructor.block_group_id WHERE placed.learning_group_id=$1").bind(group).fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM learning_group_teachers WHERE teacher_id=$1 AND learning_group_id IN(SELECT id FROM learning_groups WHERE learning_offering_id=$2)").bind(teacher).bind(offering).execute(&pool).await.unwrap();
    let after = timetable_block_service::get_workspace(
        &pool,
        query(),
        &access,
        &crate::adapters::staff_directory::STAFF_DIRECTORY,
    )
    .await
    .unwrap();
    assert!(!after
        .blocks
        .iter()
        .any(|block| block.learning_offering_id == Some(offering)));
    let retained:serde_json::Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(instructor) ORDER BY instructor.id),'[]') FROM academic_timetable_block_group_instructors instructor JOIN academic_timetable_block_groups placed ON placed.id=instructor.block_group_id WHERE placed.learning_group_id=$1").bind(group).fetch_one(&pool).await.unwrap();
    assert_eq!(retained, captured);
}
