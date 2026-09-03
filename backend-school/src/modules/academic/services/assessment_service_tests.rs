use uuid::Uuid;

use super::assessment_service;
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, CutoverFixture,
};
use crate::modules::academic::models::assessment::{
    AssessmentPhaseCode, AssessmentPlanListQuery, SaveAssessmentPhaseRequest,
    SaveAssessmentPlanRequest, UpdateAssessmentPhaseControlRequest,
};
use crate::permissions::registry::codes;
use crate::policies::assessment_access_policy::{
    require_assessment_plan_list_access, AssessmentAction,
};
use crate::policies::resource_access_policy::AcademicResourceListFilter;
use crate::test_helpers::create_named_test_pool;
use sqlx::postgres::PgPoolOptions;

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 59).await.unwrap();
    pool
}

fn list_query(academic_term_id: Uuid) -> AssessmentPlanListQuery {
    AssessmentPlanListQuery {
        academic_term_id,
        subject_id: None,
        instructor_id: None,
        ready: None,
        exam_arrangement: None,
    }
}

fn actor(permissions: &[&str]) -> ActorContext {
    ActorContext {
        user_id: Uuid::new_v4(),
        permissions: permissions
            .iter()
            .map(|permission| permission.to_string())
            .collect(),
    }
}

fn lazy_pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://schoolorbit:schoolorbit@127.0.0.1:1/schoolorbit")
        .unwrap()
}

#[tokio::test]
async fn assessment_assigned_read_permission_resolves_an_assigned_plan_filter() {
    let actor = actor(&[codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED]);

    let access = require_assessment_plan_list_access(&lazy_pool(), &actor, AssessmentAction::Read)
        .await
        .unwrap();

    assert_eq!(access.assigned_actor_id, Some(actor.user_id));
    assert!(!access.includes_school_owned);
}

#[tokio::test]
async fn learning_offering_permission_does_not_grant_assessment_plan_access() {
    let actor = actor(&[codes::LEARNING_OFFERING_READ_ASSIGNED]);

    let result =
        require_assessment_plan_list_access(&lazy_pool(), &actor, AssessmentAction::Read).await;

    assert!(matches!(result, Err(AppError::Forbidden(_))));
}

fn save_payload(
    detail: &crate::modules::academic::models::assessment::AssessmentPlanDetail,
) -> SaveAssessmentPlanRequest {
    SaveAssessmentPlanRequest {
        row_version: detail.row_version,
        assessment_coordinator_id: detail
            .assessment_coordinator_id
            .or(detail.suggested_coordinator_id)
            .or_else(|| {
                detail
                    .coordinator_candidates
                    .first()
                    .map(|candidate| candidate.teacher_id)
            }),
        phases: detail
            .phases
            .iter()
            .map(|phase| SaveAssessmentPhaseRequest {
                id: phase.id,
                phase_code: phase.phase_code,
                max_score: phase.max_score.clone(),
                exam_arrangement: phase.exam_arrangement,
                exam_duration_minutes: phase.exam_duration_minutes,
            })
            .collect(),
    }
}

#[tokio::test]
async fn one_offering_plan_is_shared_by_every_learning_group() {
    let pool = migrated_pool("assessment_phase_shared_groups").await;
    let (offering_id, term_id, year_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT learning_offering_id, academic_term_id, academic_year_id
           FROM course_assessment_plans
           ORDER BY id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let original_group_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_groups WHERE learning_offering_id = $1 AND status <> 'closed'",
    )
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
        &list_query(term_id),
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
    assert_eq!(summary.phases.len(), 4);

    let detail = assessment_service::get_plan_detail(&pool, offering_id)
        .await
        .unwrap();
    assert!(detail.suggested_coordinator_id.is_none());
    assert!(detail.coordinator_candidates.len() >= 2);
    assert_eq!(detail.id, summary.plan_id);
    assert_eq!(
        detail.learning_group_ids.len() as i64,
        original_group_count + 1
    );
}

#[tokio::test]
async fn auto_save_derives_readiness_and_rejects_stale_versions() {
    let pool = migrated_pool("assessment_auto_save_readiness").await;
    let offering_id: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let detail = assessment_service::get_plan_detail(&pool, offering_id)
        .await
        .unwrap();
    let original_version = detail.row_version.unwrap();
    let payload = save_payload(&detail);
    assert!(payload.assessment_coordinator_id.is_some());

    let saved = assessment_service::save_plan(&pool, offering_id, actor_id, true, payload)
        .await
        .unwrap();
    assert!(saved.readiness.ready);
    assert!(saved.row_version.unwrap() > original_version);

    let mut stale_payload = save_payload(&saved);
    stale_payload.row_version = Some(original_version);
    let stale =
        assessment_service::save_plan(&pool, offering_id, actor_id, true, stale_payload).await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));

    let mut reallocation = save_payload(&saved);
    reallocation.phases[0].max_score = "70.00".to_string();
    let incomplete =
        assessment_service::save_plan(&pool, offering_id, actor_id, true, reallocation)
            .await
            .unwrap();
    assert!(!incomplete.readiness.ready);
}

#[tokio::test]
async fn assigned_coordinator_cannot_change_a_locked_plan_phase() {
    let pool = migrated_pool("assessment_locked_plan_phase").await;
    let offering_id: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let detail = assessment_service::get_plan_detail(&pool, offering_id)
        .await
        .unwrap();
    let mut payload = save_payload(&detail);
    let coordinator_id = payload.assessment_coordinator_id.unwrap();
    sqlx::query(
        "UPDATE course_assessment_plans SET assessment_coordinator_id = $1 WHERE learning_offering_id = $2",
    )
    .bind(coordinator_id)
    .bind(offering_id)
    .execute(&pool)
    .await
    .unwrap();
    let before_midterm = payload
        .phases
        .iter_mut()
        .find(|phase| phase.phase_code == AssessmentPhaseCode::BeforeMidterm)
        .unwrap();
    before_midterm.max_score = "71.00".to_string();

    let result =
        assessment_service::save_plan(&pool, offering_id, coordinator_id, false, payload).await;

    assert!(matches!(result, Err(AppError::Forbidden(message)) if message.contains("ก่อนกลางภาค")));
}

#[tokio::test]
async fn assigned_coordinator_changes_only_an_enabled_plan_phase() {
    let pool = migrated_pool("assessment_enabled_plan_phase").await;
    let offering_id: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let detail = assessment_service::get_plan_detail(&pool, offering_id)
        .await
        .unwrap();
    let mut payload = save_payload(&detail);
    let coordinator_id = payload.assessment_coordinator_id.unwrap();
    sqlx::query(
        "UPDATE course_assessment_plans SET assessment_coordinator_id = $1 WHERE learning_offering_id = $2",
    )
    .bind(coordinator_id)
    .bind(offering_id)
    .execute(&pool)
    .await
    .unwrap();
    let controls = assessment_service::list_phase_controls(&pool, detail.academic_term_id)
        .await
        .unwrap();
    let before_midterm_control = controls
        .iter()
        .find(|control| control.phase_code == AssessmentPhaseCode::BeforeMidterm)
        .unwrap();
    assessment_service::update_phase_control(
        &pool,
        before_midterm_control.id,
        coordinator_id,
        UpdateAssessmentPhaseControlRequest {
            row_version: before_midterm_control.row_version,
            plan_editing_enabled: true,
            score_entry_enabled: false,
        },
    )
    .await
    .unwrap();
    let midterm = detail
        .phases
        .iter()
        .find(|phase| phase.phase_code == AssessmentPhaseCode::Midterm)
        .unwrap();
    let midterm_id = midterm.id.unwrap();
    let midterm_version = midterm.row_version.unwrap();
    payload
        .phases
        .iter_mut()
        .find(|phase| phase.phase_code == AssessmentPhaseCode::BeforeMidterm)
        .unwrap()
        .max_score = "71.00".to_string();

    let saved = assessment_service::save_plan(&pool, offering_id, coordinator_id, false, payload)
        .await
        .unwrap();

    assert_eq!(
        saved
            .phases
            .iter()
            .find(|phase| phase.phase_code == AssessmentPhaseCode::BeforeMidterm)
            .unwrap()
            .max_score,
        "71"
    );
    let locked_midterm_version: i64 =
        sqlx::query_scalar("SELECT row_version FROM course_assessment_phases WHERE id = $1")
            .bind(midterm_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(locked_midterm_version, midterm_version);
}

#[tokio::test]
async fn plan_and_score_controls_update_independently_with_optimistic_versioning() {
    let pool = migrated_pool("assessment_phase_controls").await;
    let term_id: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM course_assessment_plans LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let controls = assessment_service::list_phase_controls(&pool, term_id)
        .await
        .unwrap();
    assert_eq!(controls.len(), 4);
    assert!(controls
        .iter()
        .all(|control| !control.plan_editing_enabled && !control.score_entry_enabled));

    let control = &controls[1];
    let updated = assessment_service::update_phase_control(
        &pool,
        control.id,
        actor_id,
        UpdateAssessmentPhaseControlRequest {
            row_version: control.row_version,
            plan_editing_enabled: true,
            score_entry_enabled: false,
        },
    )
    .await
    .unwrap();
    assert!(updated.plan_editing_enabled);
    assert!(!updated.score_entry_enabled);

    let stale = assessment_service::update_phase_control(
        &pool,
        control.id,
        actor_id,
        UpdateAssessmentPhaseControlRequest {
            row_version: control.row_version,
            plan_editing_enabled: true,
            score_entry_enabled: true,
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));
}

#[test]
fn save_wire_contract_rejects_legacy_category_and_workflow_fields() {
    let legacy_payload = serde_json::json!({
        "rowVersion": 1,
        "status": "submitted",
        "categories": []
    });
    assert!(serde_json::from_value::<SaveAssessmentPlanRequest>(legacy_payload).is_err());

    let group_payload = serde_json::json!({
        "rowVersion": 1,
        "learningGroupId": Uuid::new_v4(),
        "assessmentCoordinatorId": null,
        "phases": []
    });
    assert!(serde_json::from_value::<SaveAssessmentPlanRequest>(group_payload).is_err());
}
