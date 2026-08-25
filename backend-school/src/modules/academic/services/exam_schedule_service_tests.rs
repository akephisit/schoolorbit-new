use uuid::Uuid;

use super::exam_schedule_service;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::exam_schedule::{
    CreateExamRoundRequest, ImportExamItemsRequest,
};
use crate::test_helpers::create_named_test_pool;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn import_resolves_submitted_plan_offering_group_and_homeroom_in_one_term() {
    let pool = migrated_pool("exam_canonical_import").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();

    sqlx::query("DELETE FROM academic_exam_sessions WHERE exam_round_id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM academic_exam_schedule_items WHERE exam_round_id = $1")
        .bind(round_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE course_assessment_plans SET status = 'submitted'")
        .execute(&pool)
        .await
        .unwrap();
    let expected_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM academic_exam_rounds round
           JOIN course_assessment_plans plan
             ON plan.academic_term_id = round.academic_term_id
            AND plan.status = 'submitted'
           JOIN course_assessment_categories category
             ON category.plan_id = plan.id
            AND category.exam_mode = 'in_timetable'
            AND category.code = round.exam_kind
            AND category.exam_duration_minutes IS NOT NULL
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = plan.learning_offering_id
           JOIN learning_group_homerooms coverage
             ON coverage.learning_group_id = learning_group.id
           WHERE round.id = $1"#,
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let imported = exam_schedule_service::import_exam_items(
        &pool,
        round_id,
        ImportExamItemsRequest {
            grade_level_ids: None,
        },
        actor_id,
    )
    .await
    .unwrap();
    assert_eq!(imported.inserted_count, expected_count);
    assert!(
        expected_count > 1,
        "fixture must prove one plan can feed multiple groups"
    );

    let (round_term_id, item_term_id, offering_id, group_id, plan_offering_id, subject_id): (
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
    ) = sqlx::query_as(
        r#"SELECT round.academic_term_id, item.academic_term_id,
                  item.learning_offering_id, item.learning_group_id,
                  plan.learning_offering_id, item.subject_id
           FROM academic_exam_schedule_items item
           JOIN academic_exam_rounds round ON round.id = item.exam_round_id
           JOIN course_assessment_plans plan ON plan.id = item.course_assessment_plan_id
           JOIN subjects subject ON subject.id = item.subject_id
           WHERE item.exam_round_id = $1"#,
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(round_term_id, item_term_id);
    assert_eq!(offering_id, plan_offering_id);

    let group_context: (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_term_id, learning_offering_id FROM learning_groups WHERE id = $1",
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(group_context, (item_term_id, offering_id));
    assert_ne!(subject_id, Uuid::nil());
}

#[tokio::test]
async fn published_student_view_requires_term_and_uses_group_roster() {
    let pool = migrated_pool("exam_published_group_roster").await;
    let round_id = Uuid::parse_str("84000000-0000-0000-0000-000000000001").unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_exam_rounds WHERE id = $1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let published = exam_schedule_service::publish_round(&pool, round_id, actor_id)
        .await
        .unwrap();
    assert_eq!(published.status, "published");

    let schedule =
        exam_schedule_service::list_my_published_exam_schedule(&pool, student_id, term_id)
            .await
            .unwrap();
    assert_eq!(schedule.len(), 1);
    assert_eq!(schedule[0].academic_term_id, term_id);
    assert_eq!(schedule[0].sessions.len(), 1);

    sqlx::query(
        r#"UPDATE learning_group_students
           SET membership_status = 'removed', left_at = joined_at
           WHERE student_id = $1 AND academic_term_id = $2"#,
    )
    .bind(student_id)
    .bind(term_id)
    .execute(&pool)
    .await
    .unwrap();
    let removed =
        exam_schedule_service::list_my_published_exam_schedule(&pool, student_id, term_id)
            .await
            .unwrap();
    assert!(removed.is_empty());
}

#[tokio::test]
async fn round_reads_are_explicitly_term_scoped() {
    let pool = migrated_pool("exam_round_term_scope").await;
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM academic_exam_rounds ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let rounds = exam_schedule_service::list_rounds(&pool, term_id)
        .await
        .unwrap();
    assert!(!rounds.is_empty());
    assert!(rounds.iter().all(|round| round.academic_term_id == term_id));
}

#[test]
fn exam_round_wire_rejects_legacy_semester_identity() {
    let payload = serde_json::json!({
        "academicSemesterId": Uuid::new_v4(),
        "name": "สอบกลางภาค",
        "examKind": "midterm"
    });
    assert!(serde_json::from_value::<CreateExamRoundRequest>(payload).is_err());
}
