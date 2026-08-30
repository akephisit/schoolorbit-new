use uuid::Uuid;

use super::assessment_service;
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::assessment::{
    AssessmentPlanListQuery, SaveAssessmentCategoryRequest, SaveAssessmentPlanRequest,
};
use crate::policies::resource_access_policy::AcademicResourceListFilter;
use crate::test_helpers::create_named_test_pool;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 53).await.unwrap();
    pool
}

#[tokio::test]
async fn one_offering_plan_is_shared_by_every_learning_group() {
    let pool = migrated_pool("assessment_offering_shared_groups").await;
    let (offering_id, term_id, year_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT learning_offering_id, academic_term_id, academic_year_id
           FROM course_assessment_plans
           ORDER BY id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let original_group_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_groups WHERE learning_offering_id = $1")
            .bind(offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let second_group_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           ) VALUES ($1, $2, $3, $4, 'SECOND-GROUP', 'กลุ่มเรียนที่สอง', 'draft', 'draft')"#,
    )
    .bind(second_group_id)
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .execute(&pool)
    .await
    .unwrap();

    let plans = assessment_service::list_assessment_plans(
        &pool,
        &AssessmentPlanListQuery {
            academic_term_id: term_id,
            subject_id: None,
            instructor_id: None,
            status: None,
        },
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let summary = plans
        .iter()
        .find(|plan| plan.offering_id == offering_id)
        .unwrap();
    assert_eq!(summary.learning_group_count, original_group_count + 1);
    assert!(summary.learning_group_ids.contains(&second_group_id));

    let detail = assessment_service::get_plan_detail(&pool, offering_id)
        .await
        .unwrap();
    assert_eq!(detail.id, summary.plan_id);
    assert_eq!(
        detail.learning_group_ids.len() as i64,
        original_group_count + 1
    );
}

#[tokio::test]
async fn save_and_submit_use_decimal_policy_and_optimistic_version() {
    let pool = migrated_pool("assessment_decimal_policy_submit").await;
    let (offering_id, row_version, category_id): (Uuid, i64, Uuid) = sqlx::query_as(
        r#"SELECT plan.learning_offering_id, plan.row_version, category.id
           FROM course_assessment_plans plan
           JOIN course_assessment_categories category ON category.plan_id = plan.id
           ORDER BY plan.id, category.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let saved = assessment_service::save_plan(
        &pool,
        offering_id,
        actor_id,
        SaveAssessmentPlanRequest {
            row_version: Some(row_version),
            categories: vec![SaveAssessmentCategoryRequest {
                id: Some(category_id),
                code: Some("midterm".to_string()),
                name: "รวมทั้งภาคเรียน".to_string(),
                max_score: "100.00".to_string(),
                exam_mode: "none".to_string(),
                exam_duration_minutes: None,
                display_order: 10,
                items: Vec::new(),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.status, "saved");
    assert_eq!(saved.expected_total_score, "100");
    assert_eq!(saved.categories[0].max_score, "100");

    let stale = assessment_service::save_plan(
        &pool,
        offering_id,
        actor_id,
        SaveAssessmentPlanRequest {
            row_version: Some(row_version),
            categories: Vec::new(),
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));

    let submitted = assessment_service::submit_plan(&pool, offering_id, actor_id)
        .await
        .unwrap();
    assert_eq!(submitted.status, "submitted");
    assert!(submitted.row_version > saved.row_version);
}

#[tokio::test]
async fn stopped_offerings_keep_existing_assessment_work_readable_and_writable() {
    let pool = migrated_pool("assessment_stopped_offering_history").await;
    let (offering_id, term_id, year_id, row_version, category_id, starts_on): (
        Uuid,
        Uuid,
        Uuid,
        i64,
        Uuid,
        chrono::NaiveDate,
    ) = sqlx::query_as(
        r#"SELECT plan.learning_offering_id, plan.academic_term_id,
                  plan.academic_year_id, plan.row_version, category.id,
                  offering.starts_on
           FROM course_assessment_plans plan
           JOIN course_assessment_categories category ON category.plan_id = plan.id
           JOIN learning_offerings offering ON offering.id = plan.learning_offering_id
           ORDER BY plan.id, category.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let change_set_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_term_change_sets (
               id, academic_term_id, academic_year_id, effective_from, reason,
               idempotency_key, creation_request_hash, created_by
           ) VALUES ($1, $2, $3, $4, 'ทดสอบหยุดรายการโดยเก็บงานคะแนน',
                     $5, repeat('1', 64), $6)"#,
    )
    .bind(change_set_id)
    .bind(term_id)
    .bind(year_id)
    .bind(starts_on)
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"UPDATE learning_offerings
           SET ends_on = starts_on, stop_reason = 'ทดสอบหยุดรายการ',
               stopped_at = now(), stopped_by = $2, stop_change_set_id = $3
           WHERE id = $1"#,
    )
    .bind(offering_id)
    .bind(actor_id)
    .bind(change_set_id)
    .execute(&pool)
    .await
    .unwrap();

    let plans = assessment_service::list_assessment_plans(
        &pool,
        &AssessmentPlanListQuery {
            academic_term_id: term_id,
            subject_id: None,
            instructor_id: None,
            status: None,
        },
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(plans.iter().any(|plan| plan.offering_id == offering_id));

    let saved = assessment_service::save_plan(
        &pool,
        offering_id,
        actor_id,
        SaveAssessmentPlanRequest {
            row_version: Some(row_version),
            categories: vec![SaveAssessmentCategoryRequest {
                id: Some(category_id),
                code: Some("midterm".to_string()),
                name: "งานคะแนนหลังหยุดเปิดสอน".to_string(),
                max_score: "100.00".to_string(),
                exam_mode: "none".to_string(),
                exam_duration_minutes: None,
                display_order: 10,
                items: Vec::new(),
            }],
        },
    )
    .await
    .expect("stopping future teaching must not revoke existing assessment ownership");
    assert_eq!(saved.offering_id, offering_id);
}

#[test]
fn save_wire_contract_rejects_group_or_legacy_course_identity() {
    let offering_id = Uuid::new_v4();
    let group_payload = serde_json::json!({
        "rowVersion": 1,
        "learningGroupId": offering_id,
        "categories": []
    });
    assert!(serde_json::from_value::<SaveAssessmentPlanRequest>(group_payload).is_err());

    let legacy_payload = serde_json::json!({
        "rowVersion": 1,
        "classroomCourseId": offering_id,
        "categories": []
    });
    assert!(serde_json::from_value::<SaveAssessmentPlanRequest>(legacy_payload).is_err());
}
