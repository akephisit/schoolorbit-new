use super::models::*;
use super::services;
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::test_helpers::{create_named_test_pool_with_max_connections, create_test_user};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

struct SupervisionFixture {
    actor_id: Uuid,
    teacher_id: Uuid,
    evaluator_id: Uuid,
    second_evaluator_id: Uuid,
    template: SupervisionTemplate,
    cycle: SupervisionCycle,
    academic_term_id: Uuid,
    observed_at: DateTime<Utc>,
}

async fn migrated_pool(name: &str) -> PgPool {
    migrated_pool_with_connections(name, 1).await
}

async fn migrated_pool_with_connections(name: &str, connections: u32) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, connections).await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy supervision fixture migrations should run");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("academic cutover fixture should seed");
    apply_phase_b_runtime_migrations(&pool)
        .await
        .expect("canonical supervision fixture migrations should run");
    apply_migrations_through(&pool, 67)
        .await
        .expect("current supervision fixture migrations should run");
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
    let (academic_term_id, academic_year_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT term.id, term.academic_year_id FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id WHERE term.status='active' AND year.status='active' ORDER BY term.id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("supervision fixture should have a canonical term");
    let cycle = services::create_cycle(
        pool,
        CreateSupervisionCycleRequest {
            academic_year_id,
            academic_term_id: None,
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
        academic_term_id,
        observed_at,
    }
}

async fn request_observation(
    pool: &PgPool,
    fixture: &SupervisionFixture,
) -> SupervisionObservation {
    services::request_observation(pool, fixture.teacher_id, observation_input(fixture))
        .await
        .expect("observation request should create")
}

fn observation_input(fixture: &SupervisionFixture) -> RequestSupervisionObservationRequest {
    RequestSupervisionObservationRequest {
        cycle_id: fixture.cycle.id,
        academic_term_id: fixture.academic_term_id,
        timetable_block_group_id: None,
        observed_at: None,
        manual_lesson: Some(ManualLessonInput {
            subject_name: "Mathematics".to_string(),
            classroom_label: "Grade 6/1".to_string(),
            room_label: Some("Room 601".to_string()),
            observed_at: fixture.observed_at,
            period_label: "Period 2".to_string(),
            reason: "Characterization fixture".to_string(),
        }),
    }
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
async fn supervision_lifecycle_cycles_reject_closed_contexts_without_losing_targets() {
    let pool = migrated_pool("supervision_lifecycle_cycles").await;
    let fixture = insert_fixture(&pool).await;
    let cycle_input = CreateSupervisionCycleRequest {
        academic_year_id: fixture.cycle.academic_year_id,
        academic_term_id: None,
        title: "Lifecycle cycle".into(),
        description: None,
        template_id: fixture.template.id,
        booking_opens_at: fixture.cycle.booking_opens_at,
        booking_closes_at: fixture.cycle.booking_closes_at,
        starts_at: fixture.cycle.starts_at,
        ends_at: fixture.cycle.ends_at,
        status: Some(SupervisionCycleStatus::Open),
        targets: vec![CreateSupervisionCycleTargetRequest {
            target_type: SupervisionTargetType::School,
            target_id: None,
            required_observations: 1,
            priority: 1,
        }],
    };
    let before = serde_json::to_value(&fixture.cycle).unwrap();
    for status in ["closed", "archived"] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(fixture.cycle.academic_year_id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        let created = services::create_cycle(&pool, cycle_input.clone(), fixture.actor_id).await;
        assert!(matches!(created, Err(AppError::Conflict(_))), "{created:?}");
        let updated = services::update_cycle(
            &pool,
            fixture.cycle.id,
            UpdateSupervisionCycleRequest {
                title: Some("Must not change".into()),
                targets: Some(vec![]),
                ..Default::default()
            },
        )
        .await;
        assert!(matches!(updated, Err(AppError::Conflict(_))), "{updated:?}");
        assert_eq!(
            serde_json::to_value(services::get_cycle(&pool, fixture.cycle.id).await.unwrap())
                .unwrap(),
            before
        );
    }
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(fixture.cycle.academic_year_id)
        .execute(&pool)
        .await
        .unwrap();
    let term_cycle = services::update_cycle(
        &pool,
        fixture.cycle.id,
        UpdateSupervisionCycleRequest {
            academic_term_id: Some(Some(fixture.academic_term_id)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    for status in ["closed", "cancelled"] {
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(fixture.academic_term_id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        let mut input = cycle_input.clone();
        input.academic_term_id = Some(fixture.academic_term_id);
        assert!(matches!(
            services::create_cycle(&pool, input, fixture.actor_id).await,
            Err(AppError::Conflict(_))
        ));
        assert!(
            matches!(
                services::update_cycle(
                    &pool,
                    fixture.cycle.id,
                    UpdateSupervisionCycleRequest {
                        academic_term_id: Some(None),
                        ..Default::default()
                    }
                )
                .await,
                Err(AppError::Conflict(_))
            ),
            "cannot escape a closed term by changing to a year-wide cycle"
        );
        assert_eq!(
            serde_json::to_value(services::get_cycle(&pool, fixture.cycle.id).await.unwrap())
                .unwrap(),
            serde_json::to_value(&term_cycle).unwrap()
        );
    }
    sqlx::query("UPDATE academic_terms SET status='closing' WHERE id=$1")
        .bind(fixture.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    let updated = services::update_cycle(
        &pool,
        fixture.cycle.id,
        UpdateSupervisionCycleRequest {
            title: Some("Finish during closing".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.title, "Finish during closing");
    assert_eq!(updated.targets.len(), fixture.cycle.targets.len());
}

#[tokio::test]
async fn supervision_lifecycle_cycles_cannot_move_existing_observations_out_of_context() {
    let pool = migrated_pool("supervision_lifecycle_cycle_context").await;
    let fixture = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &fixture).await;
    let target_year: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE status='planning' AND id<>$1 ORDER BY id LIMIT 1",
    )
    .bind(fixture.cycle.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let target_term = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_terms (id,academic_year_id,name,start_date,planned_end_date,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT $1,academic_year_id,'Other test term',start_date,planned_end_date,(SELECT MAX(sequence_no)+1 FROM academic_terms WHERE academic_year_id=$3),'LIFECYCLE-OTHER','custom',false,false,'planning',bell_schedule_id FROM academic_terms WHERE id=$2")
        .bind(target_term).bind(fixture.academic_term_id).bind(fixture.cycle.academic_year_id).execute(&pool).await.unwrap();
    let before = serde_json::to_value(&fixture.cycle).unwrap();
    for request in [
        UpdateSupervisionCycleRequest {
            academic_year_id: Some(target_year),
            ..Default::default()
        },
        UpdateSupervisionCycleRequest {
            academic_term_id: Some(Some(target_term)),
            ..Default::default()
        },
    ] {
        let result = services::update_cycle(&pool, fixture.cycle.id, request).await;
        assert!(matches!(result, Err(AppError::Conflict(_))), "{result:?}");
        assert_eq!(
            serde_json::to_value(services::get_cycle(&pool, fixture.cycle.id).await.unwrap())
                .unwrap(),
            before
        );
        assert_eq!(
            services::get_observation(&pool, observation.id)
                .await
                .unwrap()
                .academic_term_id,
            fixture.academic_term_id
        );
    }
}

#[tokio::test]
async fn supervision_lifecycle_requested_mutations_preserve_closed_history() {
    let pool = migrated_pool("supervision_lifecycle_requests").await;
    let fixture = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &fixture).await;
    let before = serde_json::to_value(&observation).unwrap();
    for (year_status, term_status) in [
        ("closed", "active"),
        ("archived", "active"),
        ("active", "closed"),
        ("active", "cancelled"),
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(fixture.cycle.academic_year_id)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(fixture.academic_term_id)
            .bind(term_status)
            .execute(&pool)
            .await
            .unwrap();
        let evaluator = EvaluatorAssignmentInput {
            evaluator_user_id: fixture.evaluator_id,
            role_label: None,
            is_required: Some(true),
        };
        let results = vec![
            services::request_observation(&pool, fixture.teacher_id, observation_input(&fixture))
                .await,
            services::update_requested_observation(
                &pool,
                fixture.teacher_id,
                observation.id,
                UpdateRequestedObservationRequest {
                    timetable_block_group_id: None,
                    observed_at: None,
                    manual_lesson: observation_input(&fixture).manual_lesson,
                },
            )
            .await,
            services::cancel_requested_observation(&pool, fixture.teacher_id, observation.id).await,
            services::update_observation(
                &pool,
                fixture.actor_id,
                observation.id,
                UpdateSupervisionObservationRequest::default(),
            )
            .await,
            services::cancel_observation(
                &pool,
                fixture.actor_id,
                observation.id,
                CancelObservationRequest::default(),
            )
            .await,
            services::approve_observation_request(
                &pool,
                fixture.actor_id,
                observation.id,
                ApproveObservationRequest {
                    evaluators: vec![evaluator.clone()],
                },
            )
            .await,
            services::return_observation_request(
                &pool,
                fixture.actor_id,
                observation.id,
                ReturnObservationRequest::default(),
            )
            .await,
            services::replace_observation_evaluators(
                &pool,
                fixture.actor_id,
                observation.id,
                ReplaceObservationEvaluatorsRequest {
                    evaluators: vec![evaluator],
                },
            )
            .await,
        ];
        for (operation, result) in results.into_iter().enumerate() {
            assert!(
                matches!(result, Err(AppError::Conflict(_))),
                "operation {operation}, {year_status}/{term_status}: {result:?}"
            );
        }
        assert_eq!(
            serde_json::to_value(
                services::get_observation(&pool, observation.id)
                    .await
                    .unwrap()
            )
            .unwrap(),
            before
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM supervision_observations WHERE cycle_id=$1"
            )
            .bind(fixture.cycle.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert!(matches!(
            services::cancel_requested_observation(&pool, fixture.evaluator_id, observation.id)
                .await,
            Err(AppError::Forbidden(_))
        ));
    }
}

#[tokio::test]
async fn supervision_lifecycle_action_failure_rolls_back_observations_and_evaluations() {
    let pool = migrated_pool("supervision_lifecycle_atomic_actions").await;
    let mut fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    fixture.observed_at += Duration::hours(1);
    let planned = request_observation(&pool, &fixture).await;
    let planned = approve_with(&pool, &fixture, planned.id, vec![fixture.evaluator_id]).await;
    let requested_before = serde_json::to_value(&requested).unwrap();
    let planned_before = serde_json::to_value(&planned).unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_supervision_test_action() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'SUPERVISION_TEST_ACTION_FAILURE'; END $$; CREATE TRIGGER fail_supervision_test_action BEFORE INSERT ON supervision_actions FOR EACH ROW EXECUTE FUNCTION fail_supervision_test_action();").execute(&pool).await.unwrap();
    assert!(
        services::request_observation(&pool, fixture.teacher_id, observation_input(&fixture))
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM supervision_observations WHERE cycle_id=$1"
        )
        .bind(fixture.cycle.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2,
        "request and action must be atomic"
    );
    assert!(services::update_observation(
        &pool,
        fixture.actor_id,
        requested.id,
        UpdateSupervisionObservationRequest {
            manual_lesson: observation_input(&fixture).manual_lesson,
            ..Default::default()
        }
    )
    .await
    .is_err());
    assert!(services::approve_observation_request(
        &pool,
        fixture.actor_id,
        requested.id,
        ApproveObservationRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: fixture.second_evaluator_id,
                role_label: None,
                is_required: Some(true)
            }]
        }
    )
    .await
    .is_err());
    assert_eq!(
        serde_json::to_value(
            services::get_observation(&pool, requested.id)
                .await
                .unwrap()
        )
        .unwrap(),
        requested_before
    );
    assert!(services::replace_observation_evaluators(
        &pool,
        fixture.actor_id,
        planned.id,
        ReplaceObservationEvaluatorsRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: fixture.second_evaluator_id,
                role_label: None,
                is_required: Some(true)
            }]
        }
    )
    .await
    .is_err());
    assert!(services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        planned.id,
        evaluation_responses(&fixture.template, 4.0)
    )
    .await
    .is_err());
    assert!(services::cancel_observation(
        &pool,
        fixture.actor_id,
        planned.id,
        CancelObservationRequest::default()
    )
    .await
    .is_err());
    assert_eq!(
        serde_json::to_value(services::get_observation(&pool, planned.id).await.unwrap()).unwrap(),
        planned_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM supervision_evaluator_responses WHERE observation_id=$1"
        )
        .bind(planned.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "failed submission must not retain responses"
    );
}

#[tokio::test]
async fn cycle_and_template_round_trip_preserves_targets_sections_items_and_steps() {
    let pool = migrated_pool("supervision_round_trip").await;
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

    let year_cycles = services::list_cycles(
        &pool,
        SupervisionCycleQuery {
            academic_year_id: cycle.academic_year_id,
            academic_term_id: None,
        },
    )
    .await
    .expect("year view should list supervision cycles");
    assert_eq!(
        year_cycles.iter().filter(|row| row.id == cycle.id).count(),
        1
    );
    assert!(year_cycles
        .iter()
        .all(|row| row.academic_year_id == cycle.academic_year_id));

    let term_cycles = services::list_cycles(
        &pool,
        SupervisionCycleQuery {
            academic_year_id: cycle.academic_year_id,
            academic_term_id: Some(fixture.academic_term_id),
        },
    )
    .await
    .expect("term view should include a whole-year supervision cycle");
    assert_eq!(
        term_cycles.iter().filter(|row| row.id == cycle.id).count(),
        1
    );
    assert!(term_cycles
        .iter()
        .all(|row| row.academic_year_id == cycle.academic_year_id
            && row
                .academic_term_id
                .is_none_or(|term| term == fixture.academic_term_id)));

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
    let pool = migrated_pool("supervision_replacement").await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    assert_eq!(requested.academic_year_id, fixture.cycle.academic_year_id);
    assert_eq!(requested.academic_term_id, fixture.academic_term_id);

    let term_observations = services::list_observations(
        &pool,
        services::SupervisionObservationListAccess::school(),
        SupervisionObservationFilter {
            academic_year_id: fixture.cycle.academic_year_id,
            academic_term_id: Some(fixture.academic_term_id),
            cycle_id: Some(fixture.cycle.id),
            status: Some(SupervisionObservationStatus::Requested),
        },
    )
    .await
    .expect("term view should list the requested observation");
    assert_eq!(term_observations.len(), 1);
    assert_eq!(term_observations[0].id, requested.id);

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
    let pool = migrated_pool("supervision_replacement_rollback").await;
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
async fn supervision_lifecycle_locks_context_before_cycle_and_observation_then_rechecks_status() {
    let pool = migrated_pool_with_connections("supervision_lifecycle_lock_order", 3).await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    for cancel in [false, true] {
        let mut boundary = pool.begin().await.unwrap();
        let boundary_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM academic_years WHERE id=$1 FOR UPDATE")
            .bind(fixture.cycle.academic_year_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        let worker_pool = pool.clone();
        let actor = fixture.actor_id;
        let teacher = fixture.teacher_id;
        let observation_id = requested.id;
        let worker = tokio::spawn(async move {
            if cancel {
                services::cancel_requested_observation(&worker_pool, teacher, observation_id).await
            } else {
                services::update_observation(
                    &worker_pool,
                    actor,
                    observation_id,
                    UpdateSupervisionObservationRequest::default(),
                )
                .await
            }
        });
        let mut waiting = false;
        for _ in 0..200 {
            waiting = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))")
                .bind(boundary_pid).fetch_one(&pool).await.unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let mut probe = pool.begin().await.unwrap();
        let cycle_free =
            sqlx::query("SELECT id FROM supervision_cycles WHERE id=$1 FOR UPDATE NOWAIT")
                .bind(fixture.cycle.id)
                .execute(&mut *probe)
                .await
                .is_ok();
        let observation_free =
            sqlx::query("SELECT id FROM supervision_observations WHERE id=$1 FOR UPDATE NOWAIT")
                .bind(requested.id)
                .execute(&mut *probe)
                .await
                .is_ok();
        probe.rollback().await.unwrap();
        if cancel {
            // Approval commits while the cancellation waits on the earlier academic lock.
            sqlx::query("UPDATE supervision_observations SET status='planned',approved_by=$2,approved_at=now() WHERE id=$1")
                .bind(requested.id).bind(fixture.actor_id).execute(&mut *boundary).await.unwrap();
        }
        boundary.commit().await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
            .await
            .unwrap()
            .unwrap();
        assert!(
            waiting && cycle_free && observation_free,
            "context must precede entity locks, cancel={cancel}"
        );
        if cancel {
            assert!(
                matches!(result, Err(AppError::ValidationError(_))),
                "{result:?}"
            );
            let current = services::get_observation(&pool, requested.id)
                .await
                .unwrap();
            assert_eq!(current.status, SupervisionObservationStatus::Planned);
            assert!(!current
                .actions
                .iter()
                .any(|action| action.action_kind == "request_cancelled"));
        } else {
            result.unwrap();
        }
    }
}

#[tokio::test]
async fn supervision_lifecycle_review_stages_preserve_results_and_resume_in_closing() {
    let pool = migrated_pool("supervision_lifecycle_reviews").await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    approve_with(&pool, &fixture, requested.id, vec![fixture.evaluator_id]).await;

    for stage in 0..4 {
        let before = serde_json::to_value(
            services::get_observation_review(&pool, requested.id)
                .await
                .unwrap(),
        )
        .unwrap();
        for (year_status, term_status) in [
            ("closed", "active"),
            ("archived", "active"),
            ("active", "closed"),
            ("active", "cancelled"),
        ] {
            sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
                .bind(fixture.cycle.academic_year_id)
                .bind(year_status)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
                .bind(fixture.academic_term_id)
                .bind(term_status)
                .execute(&pool)
                .await
                .unwrap();
            let result = match stage {
                0 => {
                    services::submit_my_evaluation(
                        &pool,
                        fixture.evaluator_id,
                        requested.id,
                        evaluation_responses(&fixture.template, 4.0),
                    )
                    .await
                }
                1 => services::certify_observation(&pool, fixture.actor_id, requested.id).await,
                2 => services::approve_observation(&pool, fixture.actor_id, requested.id).await,
                _ => {
                    services::acknowledge_observation(
                        &pool,
                        fixture.teacher_id,
                        requested.id,
                        AcknowledgeObservationRequest { comment: None },
                    )
                    .await
                }
            };
            assert!(
                matches!(result, Err(AppError::Conflict(_))),
                "stage {stage}, {year_status}/{term_status}: {result:?}"
            );
            assert!(matches!(
                services::submit_my_evaluation(
                    &pool,
                    fixture.teacher_id,
                    requested.id,
                    evaluation_responses(&fixture.template, 4.0)
                )
                .await,
                Err(AppError::Forbidden(_))
            ));
            assert_eq!(
                serde_json::to_value(
                    services::get_observation_review(&pool, requested.id)
                        .await
                        .unwrap()
                )
                .unwrap(),
                before
            );
        }
        sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
            .bind(fixture.cycle.academic_year_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status='closing' WHERE id=$1")
            .bind(fixture.academic_term_id)
            .execute(&pool)
            .await
            .unwrap();
        let updated = match stage {
            0 => {
                services::submit_my_evaluation(
                    &pool,
                    fixture.evaluator_id,
                    requested.id,
                    evaluation_responses(&fixture.template, 4.0),
                )
                .await
            }
            1 => services::certify_observation(&pool, fixture.actor_id, requested.id).await,
            2 => services::approve_observation(&pool, fixture.actor_id, requested.id).await,
            _ => {
                services::acknowledge_observation(
                    &pool,
                    fixture.teacher_id,
                    requested.id,
                    AcknowledgeObservationRequest { comment: None },
                )
                .await
            }
        }
        .expect("closing still permits the ordinary supervision workflow");
        let expected = [
            SupervisionObservationStatus::EvaluatorsSubmitted,
            SupervisionObservationStatus::Approved,
            SupervisionObservationStatus::Published,
            SupervisionObservationStatus::Completed,
        ];
        assert_eq!(updated.status, expected[stage]);
    }
    let review = services::get_observation_review(&pool, requested.id)
        .await
        .unwrap();
    assert_eq!(review.average_rating, Some(4.0));
}

#[tokio::test]
async fn completed_evaluations_flow_through_certification_approval_acknowledgement_and_reports() {
    let pool = migrated_pool("supervision_completed_flow").await;
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

#[tokio::test]
async fn list_hydrators_preserve_multi_parent_relations() {
    let pool = migrated_pool("supervision_multi_parent_hydration").await;
    let fixture = insert_fixture(&pool).await;

    let mut second_template_input = template_input();
    second_template_input.title = "Second rubric".to_string();
    second_template_input.sections[0].title = "Second teaching section".to_string();
    second_template_input.sections[0].items[0].label = "Second lesson clarity".to_string();
    second_template_input.steps[0].step_code = "second_evaluate".to_string();
    let second_template = services::create_template(&pool, second_template_input, fixture.actor_id)
        .await
        .expect("second supervision template should create");

    let templates = services::list_templates(&pool)
        .await
        .expect("templates should list");
    assert!(templates
        .windows(2)
        .all(|pair| pair[0].created_at >= pair[1].created_at));
    let first_template = templates
        .iter()
        .find(|template| template.id == fixture.template.id)
        .expect("first template should be listed");
    let listed_second_template = templates
        .iter()
        .find(|template| template.id == second_template.id)
        .expect("second template should be listed");
    assert_eq!(first_template.sections[0].title, "Teaching");
    assert_eq!(first_template.sections[0].items[0].label, "Lesson clarity");
    assert_eq!(first_template.steps[0].step_code, "evaluate");
    assert_eq!(
        listed_second_template.sections[0].title,
        "Second teaching section"
    );
    assert_eq!(
        listed_second_template.sections[0].items[0].label,
        "Second lesson clarity"
    );
    assert_eq!(listed_second_template.steps[0].step_code, "second_evaluate");

    let first = request_observation(&pool, &fixture).await;
    approve_with(
        &pool,
        &fixture,
        first.id,
        vec![fixture.evaluator_id, fixture.second_evaluator_id],
    )
    .await;
    services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        first.id,
        evaluation_responses(&fixture.template, 5.0),
    )
    .await
    .expect("first observation evaluation should submit");
    let first_rating_item_id = fixture.template.sections[0].items[0].id;
    services::submit_my_evaluation(
        &pool,
        fixture.second_evaluator_id,
        first.id,
        SaveEvaluationRequest {
            responses: vec![EvaluationResponseInput {
                template_item_id: first_rating_item_id,
                rating_score: Some(1.0),
                text_response: None,
            }],
        },
    )
    .await
    .expect("second evaluator should submit a partial rating set");

    let second = services::request_observation(
        &pool,
        fixture.teacher_id,
        RequestSupervisionObservationRequest {
            cycle_id: fixture.cycle.id,
            academic_term_id: fixture.academic_term_id,
            timetable_block_group_id: None,
            observed_at: None,
            manual_lesson: Some(ManualLessonInput {
                subject_name: "Science".to_string(),
                classroom_label: "Grade 6/2".to_string(),
                room_label: Some("Room 602".to_string()),
                observed_at: fixture.observed_at + Duration::days(1),
                period_label: "Period 3".to_string(),
                reason: "Second list hydration fixture".to_string(),
            }),
        },
    )
    .await
    .expect("second observation request should create");
    approve_with(
        &pool,
        &fixture,
        second.id,
        vec![fixture.second_evaluator_id],
    )
    .await;
    services::submit_my_evaluation(
        &pool,
        fixture.second_evaluator_id,
        second.id,
        evaluation_responses(&fixture.template, 2.0),
    )
    .await
    .expect("second observation evaluation should submit");

    let inaccessible_teacher_id = test_user(&pool, "inaccessible-teacher").await;
    let inaccessible = services::request_observation(
        &pool,
        inaccessible_teacher_id,
        RequestSupervisionObservationRequest {
            cycle_id: fixture.cycle.id,
            academic_term_id: fixture.academic_term_id,
            timetable_block_group_id: None,
            observed_at: None,
            manual_lesson: Some(ManualLessonInput {
                subject_name: "English".to_string(),
                classroom_label: "Grade 6/3".to_string(),
                room_label: None,
                observed_at: fixture.observed_at + Duration::days(2),
                period_label: "Period 4".to_string(),
                reason: "Access boundary fixture".to_string(),
            }),
        },
    )
    .await
    .expect("inaccessible observation request should create");

    let observations = services::list_observations(
        &pool,
        services::SupervisionObservationListAccess {
            own_user_id: Some(fixture.teacher_id),
            ..Default::default()
        },
        SupervisionObservationFilter {
            academic_year_id: fixture.cycle.academic_year_id,
            academic_term_id: Some(fixture.academic_term_id),
            cycle_id: Some(fixture.cycle.id),
            status: None,
        },
    )
    .await
    .expect("authorized observations should list");

    assert_eq!(observations.len(), 2);
    assert!(observations
        .windows(2)
        .all(|pair| pair[0].created_at >= pair[1].created_at));
    assert!(!observations
        .iter()
        .any(|observation| observation.id == inaccessible.id));

    let listed_first = observations
        .iter()
        .find(|observation| observation.id == first.id)
        .expect("first observation should be listed");
    let listed_second = observations
        .iter()
        .find(|observation| observation.id == second.id)
        .expect("second observation should be listed");
    assert_eq!(listed_first.evaluators.len(), 2);
    assert!(listed_first
        .evaluators
        .iter()
        .any(|evaluator| evaluator.evaluator_user_id == fixture.evaluator_id));
    assert!(listed_first
        .evaluators
        .iter()
        .any(|evaluator| evaluator.evaluator_user_id == fixture.second_evaluator_id));
    assert!(listed_first
        .actions
        .iter()
        .all(|action| action.observation_id == first.id));
    assert_eq!(listed_first.average_rating, Some(3.0));
    assert_eq!(listed_second.evaluators.len(), 1);
    assert_eq!(
        listed_second.evaluators[0].evaluator_user_id,
        fixture.second_evaluator_id
    );
    assert!(listed_second
        .actions
        .iter()
        .all(|action| action.observation_id == second.id));
    assert_eq!(listed_second.average_rating, Some(2.0));
}
