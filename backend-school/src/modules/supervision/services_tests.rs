use super::models::*;
use super::services;
use crate::error::AppError;
use crate::test_helpers::{create_test_pool, create_test_user, run_test_migrations};
use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

struct SupervisionFixture {
    actor_id: Uuid,
    teacher_id: Uuid,
    evaluator_id: Uuid,
    second_evaluator_id: Uuid,
    template: SupervisionTemplate,
    cycle: SupervisionCycle,
    observed_at: DateTime<Utc>,
}

async fn migrated_pool() -> PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

async fn test_user(pool: &PgPool, role: &str) -> Uuid {
    let unique = Uuid::new_v4();
    create_test_user(
        pool,
        &format!("supervision-{role}-{unique}@example.test"),
        "test-password",
    )
    .await
    .expect("supervision fixture user should insert")
}

fn template_input() -> CreateSupervisionTemplateRequest {
    CreateSupervisionTemplateRequest {
        title: "Characterization rubric".to_string(),
        description: Some("Preserves supervision service behavior".to_string()),
        status: Some(SupervisionTemplateStatus::Active),
        rating_min: 1,
        rating_max: 5,
        sections: vec![
            CreateSupervisionTemplateSectionRequest {
                title: "Teaching".to_string(),
                description: Some("Teaching practice".to_string()),
                sort_order: 10,
                items: vec![
                    CreateSupervisionTemplateItemRequest {
                        label: "Lesson clarity".to_string(),
                        description: None,
                        item_type: SupervisionTemplateItemType::Rating,
                        required: true,
                        sort_order: 20,
                    },
                    CreateSupervisionTemplateItemRequest {
                        label: "Teaching comment".to_string(),
                        description: None,
                        item_type: SupervisionTemplateItemType::Text,
                        required: true,
                        sort_order: 30,
                    },
                ],
            },
            CreateSupervisionTemplateSectionRequest {
                title: "Learners".to_string(),
                description: None,
                sort_order: 40,
                items: vec![CreateSupervisionTemplateItemRequest {
                    label: "Learner engagement".to_string(),
                    description: None,
                    item_type: SupervisionTemplateItemType::Rating,
                    required: false,
                    sort_order: 50,
                }],
            },
        ],
        steps: vec![
            CreateSupervisionTemplateStepRequest {
                step_order: 1,
                step_code: "evaluate".to_string(),
                label: "Evaluate".to_string(),
                actor_kind: SupervisionTemplateStepActorKind::Supervisor,
                actor_permission: None,
                organization_position_code: None,
                action_kind: SupervisionTemplateStepActionKind::Submit,
                required: true,
            },
            CreateSupervisionTemplateStepRequest {
                step_order: 2,
                step_code: "acknowledge".to_string(),
                label: "Acknowledge".to_string(),
                actor_kind: SupervisionTemplateStepActorKind::ObservedTeacher,
                actor_permission: None,
                organization_position_code: None,
                action_kind: SupervisionTemplateStepActionKind::Acknowledge,
                required: true,
            },
        ],
    }
}

async fn insert_fixture(pool: &PgPool) -> SupervisionFixture {
    let actor_id = test_user(pool, "actor").await;
    let teacher_id = test_user(pool, "teacher").await;
    let evaluator_id = test_user(pool, "evaluator").await;
    let second_evaluator_id = test_user(pool, "second-evaluator").await;
    let template = services::create_template(pool, template_input(), actor_id)
        .await
        .expect("supervision template should create");
    let now = Utc::now();
    let observed_at = now + Duration::hours(2);
    let cycle = services::create_cycle(
        pool,
        CreateSupervisionCycleRequest {
            academic_year: now.year(),
            semester: "1".to_string(),
            academic_semester_id: None,
            title: "Characterization cycle".to_string(),
            description: Some("Preserves workflow and transaction behavior".to_string()),
            template_id: template.id,
            booking_opens_at: Some(now - Duration::days(1)),
            booking_closes_at: Some(now + Duration::days(7)),
            starts_at: now - Duration::days(1),
            ends_at: now + Duration::days(30),
            status: Some(SupervisionCycleStatus::Open),
            targets: vec![
                CreateSupervisionCycleTargetRequest {
                    target_type: SupervisionTargetType::School,
                    target_id: None,
                    required_observations: 1,
                    priority: 100,
                },
                CreateSupervisionCycleTargetRequest {
                    target_type: SupervisionTargetType::Staff,
                    target_id: Some(teacher_id),
                    required_observations: 2,
                    priority: 10,
                },
            ],
        },
        actor_id,
    )
    .await
    .expect("supervision cycle should create");

    SupervisionFixture {
        actor_id,
        teacher_id,
        evaluator_id,
        second_evaluator_id,
        template,
        cycle,
        observed_at,
    }
}

async fn request_observation(
    pool: &PgPool,
    fixture: &SupervisionFixture,
) -> SupervisionObservation {
    services::request_observation(
        pool,
        fixture.teacher_id,
        RequestSupervisionObservationRequest {
            cycle_id: fixture.cycle.id,
            timetable_entry_id: None,
            observed_at: None,
            manual_lesson: Some(ManualLessonInput {
                subject_name: "Mathematics".to_string(),
                classroom_label: "Grade 6/1".to_string(),
                room_label: Some("Room 601".to_string()),
                observed_at: fixture.observed_at,
                period_label: "Period 2".to_string(),
                reason: "Characterization fixture".to_string(),
            }),
        },
    )
    .await
    .expect("observation request should create")
}

async fn approve_with(
    pool: &PgPool,
    fixture: &SupervisionFixture,
    observation_id: Uuid,
    evaluators: Vec<Uuid>,
) -> SupervisionObservation {
    services::approve_observation_request(
        pool,
        fixture.actor_id,
        observation_id,
        ApproveObservationRequest {
            evaluators: evaluators
                .into_iter()
                .map(|evaluator_user_id| EvaluatorAssignmentInput {
                    evaluator_user_id,
                    role_label: Some("Evaluator".to_string()),
                    is_required: Some(true),
                })
                .collect(),
        },
    )
    .await
    .expect("observation request should approve")
}

fn evaluation_responses(template: &SupervisionTemplate, score: f64) -> SaveEvaluationRequest {
    SaveEvaluationRequest {
        responses: template
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .map(|item| EvaluationResponseInput {
                template_item_id: item.id,
                rating_score: (item.item_type == SupervisionTemplateItemType::Rating)
                    .then_some(score),
                text_response: (item.item_type == SupervisionTemplateItemType::Text)
                    .then(|| "Observed evidence".to_string()),
            })
            .collect(),
    }
}

#[tokio::test]
async fn cycle_and_template_round_trip_preserves_targets_sections_items_and_steps() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    let cycle = services::get_cycle(&pool, fixture.cycle.id)
        .await
        .expect("cycle should reload");
    assert_eq!(cycle.id, fixture.cycle.id);
    assert_eq!(cycle.status, SupervisionCycleStatus::Open);
    assert_eq!(cycle.targets.len(), 2);
    assert_eq!(cycle.targets[0].target_type, SupervisionTargetType::Staff);
    assert_eq!(cycle.targets[0].priority, 10);
    assert_eq!(cycle.targets[1].target_type, SupervisionTargetType::School);
    assert_eq!(cycle.targets[1].required_observations, 1);

    let template = services::get_template(&pool, fixture.template.id)
        .await
        .expect("template should reload");
    assert_eq!(template.id, fixture.template.id);
    assert_eq!(template.status, SupervisionTemplateStatus::Active);
    assert_eq!(template.sections.len(), 2);
    assert_eq!(template.sections[0].items.len(), 2);
    assert_eq!(template.sections[1].items.len(), 1);
    assert_eq!(
        template.sections[0].items[0].item_type,
        SupervisionTemplateItemType::Rating
    );
    assert!(template.sections[0].items[0].required);
    assert_eq!(
        template.sections[0].items[1].item_type,
        SupervisionTemplateItemType::Text
    );
    assert!(!template.sections[1].items[0].required);
    assert_eq!(template.steps.len(), 2);
    assert_eq!(template.steps[0].step_order, 1);
    assert_eq!(
        template.steps[1].action_kind,
        SupervisionTemplateStepActionKind::Acknowledge
    );
}

#[tokio::test]
async fn request_approval_and_evaluator_replacement_preserve_status_and_submitted_evaluators() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    let planned = approve_with(
        &pool,
        &fixture,
        requested.id,
        vec![fixture.evaluator_id, fixture.second_evaluator_id],
    )
    .await;
    assert_eq!(planned.status, SupervisionObservationStatus::Planned);

    services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 4.0),
    )
    .await
    .expect("first evaluator should submit");

    let replaced = services::replace_observation_evaluators(
        &pool,
        fixture.actor_id,
        requested.id,
        ReplaceObservationEvaluatorsRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: fixture.second_evaluator_id,
                role_label: Some("Replacement".to_string()),
                is_required: Some(true),
            }],
        },
    )
    .await
    .expect("non-submitted evaluator should replace");

    assert_eq!(replaced.status, SupervisionObservationStatus::Planned);
    assert_eq!(replaced.evaluators.len(), 2);
    assert_eq!(
        replaced
            .evaluators
            .iter()
            .filter(|evaluator| evaluator.evaluator_user_id == fixture.evaluator_id)
            .count(),
        1
    );
    assert_eq!(
        replaced
            .evaluators
            .iter()
            .find(|evaluator| evaluator.evaluator_user_id == fixture.evaluator_id)
            .map(|evaluator| evaluator.status),
        Some(SupervisionEvaluatorStatus::Submitted)
    );
    assert_eq!(
        replaced
            .evaluators
            .iter()
            .filter(|evaluator| evaluator.evaluator_user_id == fixture.second_evaluator_id)
            .count(),
        1
    );
    assert!(replaced
        .actions
        .iter()
        .any(|action| action.action_kind == "evaluators_updated"));
}

#[tokio::test]
async fn failed_evaluator_replacement_rolls_back_assignments_and_action_rows() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    let first = request_observation(&pool, &fixture).await;
    approve_with(&pool, &fixture, first.id, vec![fixture.evaluator_id]).await;
    let second = request_observation(&pool, &fixture).await;
    approve_with(
        &pool,
        &fixture,
        second.id,
        vec![fixture.second_evaluator_id],
    )
    .await;

    let evaluator_ids_before = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM supervision_evaluators WHERE observation_id = $1 ORDER BY id",
    )
    .bind(first.id)
    .fetch_all(&pool)
    .await
    .expect("evaluator ids should load");
    let action_ids_before = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM supervision_actions WHERE observation_id = $1 ORDER BY id",
    )
    .bind(first.id)
    .fetch_all(&pool)
    .await
    .expect("action ids should load");

    let error = services::replace_observation_evaluators(
        &pool,
        fixture.actor_id,
        first.id,
        ReplaceObservationEvaluatorsRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: fixture.second_evaluator_id,
                role_label: None,
                is_required: Some(true),
            }],
        },
    )
    .await
    .expect_err("busy evaluator should be rejected");
    assert!(
        matches!(error, AppError::ValidationError(message) if message.contains("ช่วงเวลาเดียวกัน"))
    );

    let evaluator_ids_after = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM supervision_evaluators WHERE observation_id = $1 ORDER BY id",
    )
    .bind(first.id)
    .fetch_all(&pool)
    .await
    .expect("evaluator ids should reload");
    let action_ids_after = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM supervision_actions WHERE observation_id = $1 ORDER BY id",
    )
    .bind(first.id)
    .fetch_all(&pool)
    .await
    .expect("action ids should reload");

    assert_eq!(evaluator_ids_after, evaluator_ids_before);
    assert_eq!(action_ids_after, action_ids_before);
}

#[tokio::test]
async fn completed_evaluations_flow_through_certification_approval_acknowledgement_and_reports() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    approve_with(
        &pool,
        &fixture,
        requested.id,
        vec![fixture.evaluator_id, fixture.second_evaluator_id],
    )
    .await;

    services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 4.0),
    )
    .await
    .expect("first evaluator should submit");
    let submitted = services::submit_my_evaluation(
        &pool,
        fixture.second_evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 2.0),
    )
    .await
    .expect("second evaluator should submit");
    assert_eq!(
        submitted.status,
        SupervisionObservationStatus::EvaluatorsSubmitted
    );
    assert!(!services::can_view_observation_results(
        submitted.status,
        false
    ));

    let certified = services::certify_observation(&pool, fixture.actor_id, requested.id)
        .await
        .expect("completed evaluations should certify");
    assert_eq!(certified.status, SupervisionObservationStatus::Approved);
    let published = services::approve_observation(&pool, fixture.actor_id, requested.id)
        .await
        .expect("certified observation should publish");
    assert_eq!(published.status, SupervisionObservationStatus::Published);
    assert!(services::can_view_observation_results(
        published.status,
        false
    ));
    let completed = services::acknowledge_observation(
        &pool,
        fixture.teacher_id,
        requested.id,
        AcknowledgeObservationRequest {
            comment: Some("Acknowledged".to_string()),
        },
    )
    .await
    .expect("observed teacher should acknowledge");
    assert_eq!(completed.status, SupervisionObservationStatus::Completed);

    let review = services::get_observation_review(&pool, requested.id)
        .await
        .expect("review should load");
    assert_eq!(review.evaluator_results.len(), 2);
    assert_eq!(review.average_rating, Some(3.0));

    let progress = services::cycle_progress(&pool, fixture.cycle.id)
        .await
        .expect("cycle progress should load");
    assert_eq!(progress.total_observations, 1);
    assert_eq!(progress.completed_count, 1);
    assert_eq!(progress.average_rating, Some(3.0));

    let teacher_status = services::cycle_teacher_status(
        &pool,
        services::SupervisionObservationListAccess::school(),
        fixture.cycle.id,
    )
    .await
    .expect("teacher status should load");
    let teacher_row = teacher_status
        .iter()
        .find(|row| row.teacher_id == fixture.teacher_id)
        .expect("fixture teacher should appear");
    assert_eq!(teacher_row.observation_id, Some(requested.id));
    assert_eq!(
        teacher_row.status,
        Some(SupervisionObservationStatus::Completed)
    );
    assert_eq!(teacher_row.average_rating, Some(3.0));
}
