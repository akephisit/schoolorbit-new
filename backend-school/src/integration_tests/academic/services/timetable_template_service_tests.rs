use super::timetable_block_service_tests::migrated_pool;
use school_academic_timetable::{
    collaboration::TimetableMutationContext,
    models::timetable::{ApplyTemplateRequest, ClearTimetableRequest, FromCurrentRequest},
    services::{timetable_service, timetable_template_service},
};
use uuid::Uuid;

#[tokio::test]
async fn template_capture_clear_and_apply_mutate_the_single_current_board() {
    let pool = migrated_pool("timetable_template_current").await;
    let (id,term):(Uuid,Uuid)=sqlx::query_as("SELECT timetable.id,timetable.academic_term_id FROM academic_timetables timetable JOIN academic_terms term ON term.id=timetable.academic_term_id WHERE term.status='active' ORDER BY timetable.id LIMIT 1").fetch_one(&pool).await.unwrap();
    let actor = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let mutation = TimetableMutationContext::for_tests(&pool, actor);
    let before = timetable_service::get_by_id(&pool, id).await.unwrap();
    let template = timetable_template_service::from_current(
        &pool,
        actor,
        FromCurrentRequest {
            timetable_id: id,
            academic_term_id: term,
            name: "Current board template".into(),
            description: None,
            entry_types: None,
        },
    )
    .await
    .unwrap();
    assert!(!template.entries.is_empty());
    let cleared = timetable_template_service::clear_timetable(
        &pool,
        &mutation,
        ClearTimetableRequest {
            timetable_id: id,
            academic_term_id: term,
            entry_types: None,
        },
    )
    .await
    .unwrap();
    assert!(!cleared.is_empty());
    let applied = timetable_template_service::apply_template(
        &pool,
        &mutation,
        template.template.id,
        ApplyTemplateRequest {
            timetable_id: id,
            academic_term_id: term,
        },
    )
    .await
    .unwrap();
    assert_eq!(applied.applied, template.entries.len());
    let after = timetable_service::get_by_id(&pool, id).await.unwrap();
    assert_eq!(after.id, before.id);
    assert!(after.row_version > before.row_version);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM academic_timetables WHERE academic_term_id=$1")
            .bind(term)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    assert!(timetable_template_service::apply_template(
        &pool,
        &mutation,
        template.template.id,
        ApplyTemplateRequest {
            timetable_id: id,
            academic_term_id: Uuid::new_v4()
        }
    )
    .await
    .is_err());
}
