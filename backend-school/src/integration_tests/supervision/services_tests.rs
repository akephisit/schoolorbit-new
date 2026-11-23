use crate::integration_tests::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use school_errors::AppError;
use school_supervision::models::*;
use school_supervision::services;
use school_test_db::{create_named_test_pool_with_max_connections, create_test_user};
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

#[tokio::test]
async fn lifecycle_supervision_provider_warns_only_for_pending_term_observations() {
    let pool = migrated_pool("lifecycle_supervision_pending").await;
    let fixture = insert_fixture(&pool).await;
    let year = fixture.cycle.academic_year_id;
    let term = fixture.academic_term_id;
    let mut tx = pool.begin().await.unwrap();
    let original = services::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    assert_eq!(original.len(), 1);
    let original_id = original[0].id;
    tx.rollback().await.unwrap();
    let observation = request_observation(&pool, &fixture).await;
    let mut tx = pool.begin().await.unwrap();
    let pending = services::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending
        .iter()
        .any(|row| row.id == observation.id && !row.blocks_closure));
    assert!(pending.iter().any(|row| row.id == original_id));
    assert!(services::pending_term_work(&mut tx, Uuid::new_v4(), term)
        .await
        .unwrap()
        .is_empty());
    sqlx::query("UPDATE supervision_observations SET status='cancelled' WHERE id=$1")
        .bind(observation.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let remaining = services::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, original_id);
}

async fn migrated_pool(name: &str) -> PgPool {
    migrated_pool_with_connections(name, 1).await
}

async fn migrated_pool_with_connections(name: &str, connections: u32) -> PgPool {
    migrated_pool_through(
        name,
        connections,
        school_migrations::latest_migration_version(),
    )
    .await
}

async fn migrated_pool_through(name: &str, connections: u32, version: i64) -> PgPool {
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
    apply_migrations_through(&pool, version)
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
    let observed_at = Utc::now()
        .date_naive()
        .and_hms_opt(3, 0, 0)
        .unwrap()
        .and_utc();
    let (academic_term_id, academic_year_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT term.id, term.academic_year_id FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id WHERE term.status='active' AND year.status='active' ORDER BY term.id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("supervision fixture should have a canonical term");
    seed_teaching(pool, teacher_id, academic_year_id, academic_term_id).await;
    let cycle = services::create_cycle(
        pool,
        CreateSupervisionCycleRequest {
            observation_start_date: chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
            observation_end_date: chrono::NaiveDate::from_ymd_opt(2200, 12, 31).unwrap(),
            academic_year_id,
            academic_term_id: None,
            title: "Characterization cycle".to_string(),
            description: Some("Preserves workflow and transaction behavior".to_string()),
            template_id: template.id,
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
    services::request_observation(
        pool,
        fixture.teacher_id,
        observation_input(pool, fixture).await,
    )
    .await
    .expect("observation request should create")
}

async fn seed_teaching(pool: &PgPool, teacher: Uuid, year: Uuid, term: Uuid) {
    let (timetable, schedule, group, offering): (Uuid, Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT t.id,t.bell_schedule_id,g.id,g.learning_offering_id FROM academic_timetables t
         JOIN learning_groups g ON g.academic_term_id=t.academic_term_id
         JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.kind='course'
         WHERE t.academic_term_id=$1 ORDER BY g.id LIMIT 1",
    )
    .bind(term)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO learning_group_homerooms(id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source)
        SELECT uuidv7(),$1,$2,$3,h.id,'supervision_fixture' FROM homerooms h WHERE h.academic_year_id=$3
        AND NOT EXISTS(SELECT 1 FROM learning_group_homerooms WHERE learning_group_id=$1) ORDER BY h.id LIMIT 1")
        .bind(group).bind(term).bind(year).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role)
        VALUES(uuidv7(),$1,$2,$3,$4,'secondary') ON CONFLICT(learning_group_id,teacher_id) DO NOTHING")
        .bind(group).bind(term).bind(year).bind(teacher).execute(pool).await.unwrap();
    for hour in 10..=12 {
        let name = format!("Supervision fixture {hour}");
        let period: Uuid = if let Some(id) = sqlx::query_scalar(
            "SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND name=$2",
        )
        .bind(schedule)
        .bind(&name)
        .fetch_optional(pool)
        .await
        .unwrap()
        {
            id
        } else {
            sqlx::query_scalar("INSERT INTO bell_schedule_periods(id,bell_schedule_id,name,start_time,end_time,order_index)
                SELECT uuidv7(),$1,$2,make_time($3,0,0),make_time($3+1,0,0),COALESCE(max(order_index),0)+1
                FROM bell_schedule_periods WHERE bell_schedule_id=$1 RETURNING id")
                .bind(schedule).bind(&name).bind(hour).fetch_one(pool).await.unwrap()
        };
        for day in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"] {
            let block:Uuid=if let Some(id)=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_id=$1 AND title=$2 AND day_of_week=$3")
                .bind(timetable).bind(&name).bind(day).fetch_optional(pool).await.unwrap(){id}else{
                sqlx::query_scalar("INSERT INTO academic_timetable_blocks(timetable_id,academic_term_id,academic_year_id,bell_schedule_id,
                    bell_schedule_period_id,day_of_week,block_kind,scheduling_mode,learning_offering_id,title)
                    VALUES($1,$2,$3,$4,$5,$6,'COURSE','independent',$7,$8) RETURNING id")
                    .bind(timetable).bind(term).bind(year).bind(schedule).bind(period).bind(day).bind(offering).bind(&name)
                    .fetch_one(pool).await.unwrap()};
            let placed:Uuid=if let Some(id)=sqlx::query_scalar("SELECT id FROM academic_timetable_block_groups WHERE block_id=$1 AND learning_group_id=$2")
                .bind(block).bind(group).fetch_optional(pool).await.unwrap(){id}else{
                sqlx::query_scalar("INSERT INTO academic_timetable_block_groups(block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,homeroom_ids)
                    VALUES($1,$2,$3,$4,$5,ARRAY(SELECT homeroom_id FROM learning_group_homerooms WHERE learning_group_id=$2 ORDER BY homeroom_id LIMIT 1)) RETURNING id").bind(block).bind(group).bind(offering).bind(term).bind(year).fetch_one(pool).await.unwrap()};
            sqlx::query("INSERT INTO academic_timetable_block_group_instructors(block_group_id,instructor_id,role,display_order)
                SELECT $1,$2,'secondary',COALESCE(max(display_order),0)+1 FROM academic_timetable_block_group_instructors WHERE block_group_id=$1
                ON CONFLICT(block_group_id,instructor_id) DO NOTHING")
                .bind(placed).bind(teacher).execute(pool).await.unwrap();
        }
    }
}

async fn observation_input(
    pool: &PgPool,
    fixture: &SupervisionFixture,
) -> RequestSupervisionObservationRequest {
    input_at(pool, fixture, fixture.teacher_id, fixture.observed_at).await
}

async fn input_at(
    pool: &PgPool,
    fixture: &SupervisionFixture,
    teacher: Uuid,
    at: DateTime<Utc>,
) -> RequestSupervisionObservationRequest {
    let date = at.with_timezone(&chrono_tz::Asia::Bangkok).date_naive();
    let monday = date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
    let options = services::booking_options(
        pool,
        teacher,
        SupervisionBookingOptionsQuery {
            cycle_id: fixture.cycle.id,
            academic_year_id: fixture.cycle.academic_year_id,
            academic_term_id: fixture.academic_term_id,
            week_start_date: monday,
        },
    )
    .await
    .unwrap();
    let hour = at.with_timezone(&chrono_tz::Asia::Bangkok).hour();
    let item = options
        .items
        .iter()
        .find(|p| {
            p.observed_date == date
                && p.start_time.hour() == hour
                && p.period_name.starts_with("Supervision fixture")
        })
        .expect("fixture has an actual teaching period");
    RequestSupervisionObservationRequest {
        cycle_id: fixture.cycle.id,
        academic_term_id: fixture.academic_term_id,
        observed_date: date,
        timetable_block_group_ids: vec![item.block_group_id],
        expected_source_digest: options.source_digest,
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
async fn supervision_action_notifications_are_atomic_and_follow_confirmed_participants() {
    use crate::adapters::notification_delivery::APPLICATION_NOTIFICATION_ELIGIBILITY as ELIGIBILITY;
    use school_notifications::{
        dispatch::{
            self, NewNotification, NotificationContent, NotificationSource, PendingPresentation,
            Presentation,
        },
        publisher::NotificationType,
    };
    use std::collections::BTreeSet;

    #[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
    struct InboxRow {
        receipt_id: Uuid,
        notification_id: Uuid,
        recipient_id: Uuid,
        action_kind: String,
        title: String,
        message: String,
        link: String,
        presentation: String,
    }

    let pool = migrated_pool_through("supervision_action_notifications", 1, 166).await;
    let f = insert_fixture(&pool).await;
    let direct = NewNotification::new(
        f.actor_id,
        NotificationSource::Direct,
        "existing-direct-notice".into(),
        NotificationContent {
            title: "Existing notice".into(),
            message: "Existing text".into(),
            type_: NotificationType::Info,
            link: None,
        },
        Presentation::AlertOnce,
    );
    let mut tx = pool.begin().await.unwrap();
    dispatch::create_in_transaction(&mut tx, std::slice::from_ref(&direct))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let direct_snapshot = |id| {
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT jsonb_build_array(to_jsonb(r),to_jsonb(n),to_jsonb(p)) FROM notification_receipts r
             JOIN notifications n ON n.id=r.notification_id
             JOIN notification_presentations p ON p.receipt_id=r.id WHERE r.id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
    };
    let original_direct = direct_snapshot(direct.id).await.unwrap();
    for _ in 0..2 {
        school_test_db::run_test_migrations(&pool).await;
        assert_eq!(direct_snapshot(direct.id).await.unwrap(), original_direct);
    }

    let observation = request_observation(&pool, &f).await;
    let id = observation.id;
    let third = test_user(&pool, "third-evaluator").await;
    let roster = |ids: &[Uuid]| -> Vec<_> {
        ids.iter()
            .map(|&evaluator_user_id| EvaluatorAssignmentInput {
                evaluator_user_id,
                role_label: Some("Synthetic role".into()),
                is_required: Some(true),
            })
            .collect()
    };
    let replace = |evaluators| {
        services::replace_observation_evaluators(
            &pool,
            f.actor_id,
            id,
            ReplaceObservationEvaluatorsRequest { evaluators },
        )
    };
    let approve = || {
        services::approve_observation_request(
            &pool,
            f.actor_id,
            id,
            ApproveObservationRequest {
                evaluators: roster(&[f.evaluator_id, f.second_evaluator_id]),
            },
        )
    };
    let inboxes = |observation_id| {
        sqlx::query_as::<_, InboxRow>(
            "SELECT r.id AS receipt_id,n.id AS notification_id,r.recipient_id,a.action_kind,n.title,n.message,n.link,r.presentation
             FROM notification_receipts r JOIN supervision_actions a ON a.id=r.source_id
             JOIN notifications n ON n.id=r.notification_id AND n.user_id=r.recipient_id
             WHERE r.source_kind='supervision_action' AND a.observation_id=$1 ORDER BY r.id",
        )
        .bind(observation_id)
        .fetch_all(&pool)
    };
    let recipients = |rows: &[InboxRow]| {
        rows.iter()
            .map(|row| row.recipient_id)
            .collect::<BTreeSet<_>>()
    };
    let delivery_counts = || {
        sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT (SELECT count(*) FROM notifications),(SELECT count(*) FROM notification_receipts),
             (SELECT count(*) FROM notification_presentations)",
        )
        .fetch_one(&pool)
    };
    let action_source =
        |observation: &SupervisionObservation, kind: &str| NotificationSource::SupervisionAction {
            source_id: observation
                .actions
                .iter()
                .find(|row| row.action_kind == kind)
                .unwrap()
                .id,
        };
    let prefetched = |pending: &[PendingPresentation], source, recipient_id| {
        pending
            .iter()
            .find(|item| item.source == source && item.recipient_id == recipient_id)
            .copied()
            .unwrap()
    };
    let presentation_outcome = |receipt_id| {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT presentation_outcome FROM notification_receipts WHERE id=$1",
        )
        .bind(receipt_id)
        .fetch_one(&pool)
    };

    let provisional = replace(roster(&[f.second_evaluator_id])).await.unwrap();
    assert!(inboxes(id).await.unwrap().is_empty());
    sqlx::raw_sql(
        "CREATE FUNCTION reject_supervision_presentation() RETURNS trigger LANGUAGE plpgsql
         AS $$ BEGIN RAISE EXCEPTION 'SUPERVISION_PRESENTATION_ROLLBACK'; END $$;
         CREATE TRIGGER reject_supervision_presentation BEFORE INSERT ON notification_presentations
         FOR EACH ROW EXECUTE FUNCTION reject_supervision_presentation();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(approve().await.is_err());
    let rolled_back = services::get_observation(&pool, id).await.unwrap();
    assert_eq!(rolled_back.status, SupervisionObservationStatus::Requested);
    assert_eq!(rolled_back.actions.len(), provisional.actions.len());
    assert_eq!(rolled_back.evaluators.len(), 1);
    assert_eq!(
        rolled_back.evaluators[0].evaluator_user_id,
        f.second_evaluator_id
    );
    assert!(inboxes(id).await.unwrap().is_empty());
    assert_eq!(delivery_counts().await.unwrap(), (1, 1, 1));
    sqlx::raw_sql(
        "DROP TRIGGER reject_supervision_presentation ON notification_presentations;
         DROP FUNCTION reject_supervision_presentation();",
    )
    .execute(&pool)
    .await
    .unwrap();

    let approved = approve().await.unwrap();
    let approved_inboxes = inboxes(id).await.unwrap();
    assert_eq!(approved_inboxes.len(), 3);
    assert_eq!(
        recipients(&approved_inboxes),
        BTreeSet::from([f.teacher_id, f.evaluator_id, f.second_evaluator_id])
    );
    let destination = format!(
        "/staff/academic/supervision/{id}?academicYearId={}&academicTermId={}",
        f.cycle.academic_year_id, f.academic_term_id
    );
    let evaluator_destination = format!(
        "/staff/academic/supervision/evaluate?academicYearId={}&academicTermId={}",
        f.cycle.academic_year_id, f.academic_term_id
    );
    assert!(approved_inboxes
        .iter()
        .all(|row| row.action_kind == "planned" && row.presentation == "alert_once"));
    assert!(approved_inboxes
        .iter()
        .any(|row| row.recipient_id == f.teacher_id
            && row.title == "อนุมัติคำขอนิเทศแล้ว"
            && row.link == destination));
    assert!(approved_inboxes
        .iter()
        .filter(|row| row.recipient_id != f.teacher_id)
        .all(|row| row.title == "ได้รับมอบหมายให้ประเมินการนิเทศ" && row.link == evaluator_destination));
    assert!(approve().await.is_err());
    assert_eq!(inboxes(id).await.unwrap(), approved_inboxes);

    let second_input = input_at(&pool, &f, f.teacher_id, f.observed_at + Duration::hours(1)).await;
    let second = services::request_observation(&pool, f.teacher_id, second_input)
        .await
        .unwrap();
    let second = approve_with(&pool, &f, second.id, vec![f.second_evaluator_id]).await;
    let approval_source = action_source(&approved, "planned");
    let pending = dispatch::pending(&pool).await.unwrap();
    let teacher_alert = prefetched(&pending, approval_source, f.teacher_id);
    let retained_assignment = prefetched(&pending, approval_source, f.evaluator_id);
    let removed_assignment = prefetched(&pending, approval_source, f.second_evaluator_id);
    let other_observation_assignment = prefetched(
        &pending,
        action_source(&second, "planned"),
        f.second_evaluator_id,
    );
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(f.teacher_id)
        .execute(&pool)
        .await
        .unwrap();
    let presented = dispatch::consume(&pool, &teacher_alert, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(presented.id, teacher_alert.notification_id);
    assert_eq!(presented.link.as_deref(), Some(destination.as_str()));
    assert!(dispatch::consume(&pool, &teacher_alert, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE users SET status='active' WHERE id=$1")
        .bind(f.teacher_id)
        .execute(&pool)
        .await
        .unwrap();

    let replacement = replace(roster(&[f.evaluator_id, third])).await.unwrap();
    let updated = inboxes(id).await.unwrap();
    assert_eq!(updated.len(), 5);
    assert_eq!(&updated[..3], approved_inboxes.as_slice());
    let diff = &updated[3..];
    assert_eq!(
        recipients(diff),
        BTreeSet::from([f.second_evaluator_id, third])
    );
    assert!(diff
        .iter()
        .all(|row| row.action_kind == "evaluators_updated" && row.presentation == "alert_once"));
    assert!(diff.iter().all(|row| row.link == evaluator_destination));
    assert_eq!(
        presentation_outcome(removed_assignment.id)
            .await
            .unwrap()
            .as_deref(),
        Some("skipped")
    );
    assert!(dispatch::consume(&pool, &removed_assignment, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    let replacement_pending = dispatch::pending(&pool).await.unwrap();
    let replacement_source = action_source(&replacement, "evaluators_updated");
    let added_assignment = prefetched(&replacement_pending, replacement_source, third);
    let removal_alert = prefetched(
        &replacement_pending,
        replacement_source,
        f.second_evaluator_id,
    );
    let presented_removal = dispatch::consume(&pool, &removal_alert, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        presented_removal.link.as_deref(),
        Some(evaluator_destination.as_str())
    );
    let mut same_people = roster(&[third, f.evaluator_id]);
    same_people[0].role_label = None;
    replace(same_people).await.unwrap();
    assert_eq!(inboxes(id).await.unwrap(), updated);
    let pending_after_noop: BTreeSet<_> = dispatch::pending(&pool)
        .await
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect();
    for unaffected in [
        retained_assignment,
        other_observation_assignment,
        added_assignment,
    ] {
        assert!(pending_after_noop.contains(&unaffected.id));
    }

    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(third)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE supervision_observations SET requested_by=$2 WHERE id=$1")
        .bind(id)
        .bind(f.actor_id)
        .execute(&pool)
        .await
        .unwrap();
    let return_request = || {
        services::return_observation_request(
            &pool,
            f.actor_id,
            id,
            ReturnObservationRequest {
                comment: Some("private return reason".into()),
            },
        )
    };
    let returned_observation = return_request().await.unwrap();
    let returned = inboxes(id).await.unwrap();
    assert_eq!(returned.len(), 7);
    assert_eq!(&returned[..5], updated.as_slice());
    assert_eq!(
        recipients(&returned[5..]),
        BTreeSet::from([f.teacher_id, f.evaluator_id])
    );
    assert!(returned[5..]
        .iter()
        .all(|row| row.action_kind == "request_returned"
            && !row.message.contains("private return reason")));
    for superseded in [retained_assignment, added_assignment] {
        assert_eq!(
            presentation_outcome(superseded.id)
                .await
                .unwrap()
                .as_deref(),
            Some("skipped")
        );
        assert!(dispatch::consume(&pool, &superseded, &ELIGIBILITY)
            .await
            .unwrap()
            .is_none());
    }
    let pending_after_return = dispatch::pending(&pool).await.unwrap();
    assert!(pending_after_return
        .iter()
        .any(|item| item.id == other_observation_assignment.id));
    let withdrawal_alert = prefetched(
        &pending_after_return,
        action_source(&returned_observation, "request_returned"),
        f.evaluator_id,
    );
    return_request().await.unwrap();
    replace(roster(&[f.second_evaluator_id])).await.unwrap();
    assert_eq!(inboxes(id).await.unwrap(), returned);

    let replanned = approve().await.unwrap();
    assert_eq!(inboxes(id).await.unwrap().len(), 10);
    assert_eq!(
        presentation_outcome(withdrawal_alert.id)
            .await
            .unwrap()
            .as_deref(),
        Some("skipped")
    );
    assert!(dispatch::consume(&pool, &withdrawal_alert, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    let stale = prefetched(
        &dispatch::pending(&pool).await.unwrap(),
        action_source(&replanned, "planned"),
        f.evaluator_id,
    );
    let cancel = || {
        services::cancel_observation(
            &pool,
            f.actor_id,
            id,
            CancelObservationRequest {
                reason: Some("private cancellation reason".into()),
            },
        )
    };
    let cancelled = cancel().await.unwrap();
    let cancellation = inboxes(id).await.unwrap();
    assert_eq!(cancellation.len(), 13);
    assert_eq!(
        recipients(&cancellation[10..]),
        recipients(&approved_inboxes)
    );
    assert!(cancellation[10..]
        .iter()
        .all(|row| row.action_kind == "cancelled"
            && !row.message.contains("private cancellation reason")));
    assert!(cancellation[10..]
        .iter()
        .any(|row| row.recipient_id == f.teacher_id && row.link == destination));
    assert!(cancellation[10..]
        .iter()
        .filter(|row| row.recipient_id != f.teacher_id)
        .all(|row| row.link == evaluator_destination));
    cancel().await.unwrap();
    assert_eq!(inboxes(id).await.unwrap(), cancellation);
    assert!(dispatch::consume(&pool, &stale, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    assert_eq!(replanned.status, SupervisionObservationStatus::Planned);
    let new_alert = prefetched(
        &dispatch::pending(&pool).await.unwrap(),
        action_source(&cancelled, "cancelled"),
        f.evaluator_id,
    );
    let presented_cancellation = dispatch::consume(&pool, &new_alert, &ELIGIBILITY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        presented_cancellation.link.as_deref(),
        Some(evaluator_destination.as_str())
    );

    assert!(
        dispatch::consume(&pool, &other_observation_assignment, &ELIGIBILITY)
            .await
            .unwrap()
            .is_some()
    );
    let evaluation = evaluation_responses(&f.template, 3.0);
    let before = services::get_observation_review(&pool, second.id)
        .await
        .unwrap();
    let counts = delivery_counts().await.unwrap();
    sqlx::raw_sql(
        "CREATE FUNCTION reject_publication_notice() RETURNS trigger LANGUAGE plpgsql
         AS $$ BEGIN RAISE EXCEPTION 'TEST_SUPERVISION_PUBLICATION_NOTICE'; END $$;
         CREATE TRIGGER reject_publication_notice BEFORE INSERT ON notification_presentations
         FOR EACH ROW EXECUTE FUNCTION reject_publication_notice();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(services::submit_my_evaluation(
        &pool,
        f.second_evaluator_id,
        second.id,
        evaluation.clone(),
    )
    .await
    .is_err());
    assert_eq!(delivery_counts().await.unwrap(), counts);
    assert_eq!(
        serde_json::to_value(
            services::get_observation_review(&pool, second.id)
                .await
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(before).unwrap(),
    );
    sqlx::query("DROP TRIGGER reject_publication_notice ON notification_presentations")
        .execute(&pool)
        .await
        .unwrap();

    let result =
        services::submit_my_evaluation(&pool, f.second_evaluator_id, second.id, evaluation.clone())
            .await
            .unwrap();
    assert_eq!(result.status, SupervisionObservationStatus::Published);
    let replay =
        services::submit_my_evaluation(&pool, f.second_evaluator_id, second.id, evaluation)
            .await
            .unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(result).unwrap()
    );
    let published = inboxes(second.id).await.unwrap();
    assert_eq!(published.len(), 3);
    assert_eq!(published[2].recipient_id, f.teacher_id);
    assert_eq!(published[2].action_kind, "result_published");

    let already_withdrawn = request_observation(&pool, &f).await;
    approve_with(&pool, &f, already_withdrawn.id, vec![f.evaluator_id]).await;
    let already_withdrawn = services::return_observation_request(
        &pool,
        f.actor_id,
        already_withdrawn.id,
        ReturnObservationRequest { comment: None },
    )
    .await
    .unwrap();
    let outstanding_withdrawal = prefetched(
        &dispatch::pending(&pool).await.unwrap(),
        action_source(&already_withdrawn, "request_returned"),
        f.evaluator_id,
    );
    services::cancel_requested_observation(&pool, f.teacher_id, already_withdrawn.id)
        .await
        .unwrap();
    let requested_cancellation = inboxes(already_withdrawn.id).await.unwrap();
    assert_eq!(requested_cancellation.len(), 5);
    assert_eq!(requested_cancellation[4].action_kind, "request_cancelled");
    assert_eq!(requested_cancellation[4].recipient_id, f.teacher_id);
    assert!(
        dispatch::consume(&pool, &outstanding_withdrawal, &ELIGIBILITY)
            .await
            .unwrap()
            .is_some()
    );

    let before_delete = dispatch::pending(&pool).await.unwrap();
    let impact = services::cycle_deletion_impact(&pool, f.cycle.id)
        .await
        .unwrap();
    services::delete_cycle(
        &pool,
        f.cycle.id,
        DeleteSupervisionCycleRequest {
            expected_revision: impact.revision,
        },
    )
    .await
    .unwrap();
    for item in before_delete
        .iter()
        .filter(|item| matches!(item.source, NotificationSource::SupervisionAction { .. }))
    {
        assert!(dispatch::consume(&pool, item, &ELIGIBILITY)
            .await
            .unwrap()
            .is_none());
    }
    assert_eq!(delivery_counts().await.unwrap(), (1, 1, 1));
    assert_eq!(direct_snapshot(direct.id).await.unwrap(), original_direct);
}

#[tokio::test]
async fn supervision_lifecycle_cycles_reject_closed_contexts_without_losing_targets() {
    let pool = migrated_pool("supervision_lifecycle_cycles").await;
    let fixture = insert_fixture(&pool).await;
    let cycle_input = CreateSupervisionCycleRequest {
        observation_start_date: chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
        observation_end_date: chrono::NaiveDate::from_ymd_opt(2200, 12, 31).unwrap(),
        academic_year_id: fixture.cycle.academic_year_id,
        academic_term_id: None,
        title: "Lifecycle cycle".into(),
        description: None,
        template_id: fixture.template.id,
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
    sqlx::query("INSERT INTO academic_terms (school_days, id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT 'MON,TUE,WED,THU,FRI', $1,academic_year_id,'Other test term',(SELECT MAX(sequence_no)+1 FROM academic_terms WHERE academic_year_id=$3),'LIFECYCLE-OTHER','custom',false,false,'planning',bell_schedule_id FROM academic_terms WHERE id=$2")
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
            services::request_observation(
                &pool,
                fixture.teacher_id,
                observation_input(&pool, &fixture).await,
            )
            .await,
            services::update_requested_observation(
                &pool,
                fixture.teacher_id,
                observation.id,
                UpdateRequestedObservationRequest {
                    observed_date: observation_input(&pool, &fixture).await.observed_date,
                    timetable_block_group_ids: observation_input(&pool, &fixture)
                        .await
                        .timetable_block_group_ids,
                    expected_source_digest: observation_input(&pool, &fixture)
                        .await
                        .expected_source_digest,
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
    assert!(services::request_observation(
        &pool,
        fixture.teacher_id,
        observation_input(&pool, &fixture).await
    )
    .await
    .is_err());
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
            template_id: Some(fixture.template.id),
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
    assert!(term_cycles.iter().all(|row| {
        row.academic_year_id == cycle.academic_year_id
            && row
                .academic_term_id
                .is_none_or(|term| term == fixture.academic_term_id)
    }));

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
    let other = insert_fixture(&pool).await;
    let second = request_observation(&pool, &other).await;
    separate_saved_class(&pool, &second).await;
    approve_with(&pool, &fixture, first.id, vec![fixture.evaluator_id]).await;
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
    assert!(matches!(error, AppError::Conflict(message) if message.contains("ช่วงเวลาเดียวกัน")));

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
async fn supervision_lifecycle_publication_and_acknowledgement_resume_in_closing() {
    let pool = migrated_pool("supervision_lifecycle_reviews").await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    approve_with(&pool, &fixture, requested.id, vec![fixture.evaluator_id]).await;

    for stage in 0..2 {
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
async fn completed_evaluations_publish_on_final_required_submission_and_support_acknowledgement_and_reports(
) {
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

    services::update_cycle(
        &pool,
        fixture.cycle.id,
        UpdateSupervisionCycleRequest {
            status: Some(SupervisionCycleStatus::Closed),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let partial = services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 4.0),
    )
    .await
    .expect("first evaluator should submit");
    assert_eq!(partial.status, SupervisionObservationStatus::Planned);
    assert!(!services::can_view_observation_results(
        partial.status,
        false
    ));
    assert!(services::acknowledge_observation(
        &pool,
        fixture.teacher_id,
        requested.id,
        AcknowledgeObservationRequest::default(),
    )
    .await
    .is_err());

    let published = services::submit_my_evaluation(
        &pool,
        fixture.second_evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 2.0),
    )
    .await
    .expect("final required evaluation should publish");
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

    let progress = services::cycle_progress(
        &pool,
        &school_authorization::ActorContext {
            user_id: fixture.actor_id,
            permissions: vec![crate::test_permission_codes::WILDCARD.into()],
        },
        fixture.cycle.id,
    )
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
        input_at(
            &pool,
            &fixture,
            fixture.teacher_id,
            fixture.observed_at + Duration::days(1),
        )
        .await,
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
    seed_teaching(
        &pool,
        inaccessible_teacher_id,
        fixture.cycle.academic_year_id,
        fixture.academic_term_id,
    )
    .await;
    let inaccessible = services::request_observation(
        &pool,
        inaccessible_teacher_id,
        input_at(
            &pool,
            &fixture,
            inaccessible_teacher_id,
            fixture.observed_at + Duration::days(2),
        )
        .await,
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

#[tokio::test]
async fn management_template_summaries_preserve_counts_without_rubric_payloads() {
    let pool = migrated_pool("supervision_template_summaries").await;
    let fixture = insert_fixture(&pool).await;
    let mut input = template_input();
    input.title = "Multiple sections".into();
    let mut second_section = input.sections[0].clone();
    second_section.title = "Second section".into();
    second_section.items.truncate(1);
    input.sections.push(second_section);
    let second = services::create_template(&pool, input, fixture.actor_id)
        .await
        .unwrap();
    let summaries = services::list_template_summaries(&pool).await.unwrap();
    for template in [&fixture.template, &second] {
        let summary = summaries.iter().find(|row| row.id == template.id).unwrap();
        let selected = services::get_template_summary(&pool, template.id)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&selected).unwrap(),
            serde_json::to_value(summary).unwrap()
        );
        assert_eq!(summary.section_count, template.sections.len() as i64);
        assert_eq!(
            summary.item_count,
            template
                .sections
                .iter()
                .map(|section| section.items.len() as i64)
                .sum::<i64>()
        );
        let value = serde_json::to_value(summary).unwrap();
        assert!(value.get("sections").is_none());
        assert!(value.get("steps").is_none());
    }
    assert!(matches!(
        services::get_template_summary(&pool, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn permission_coverage_supervision_embargo_uses_exact_grant_origin_for_each_observed_user() {
    use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
    use crate::integration_tests::permission_coverage::{
        coverage_actor, coverage_actor_for_user, coverage_request, coverage_router,
    };
    use crate::test_permission_codes as codes;
    use axum::http::{Method, StatusCode};
    use serde_json::{json, Value};
    let pool = migrated_pool_with_connections("permission_coverage_supervision", 4).await;
    school_test_db::run_test_migrations(&pool).await;
    let first = insert_fixture(&pool).await;
    let mut second = insert_fixture(&pool).await;
    second.observed_at += Duration::hours(1);
    let units: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    // Exact-source fixture: seeded position grants must not accidentally add a school scope.
    sqlx::query("DELETE FROM organization_permission_grants WHERE organization_unit_id=ANY($1)")
        .bind(&units)
        .execute(&pool)
        .await
        .unwrap();
    let auth = ProtectedFixture::with_prepared_tenant_pool(
        "permission_coverage_supervision",
        pool.clone(),
    )
    .await;
    let _managed_lease = auth
        .runtime
        .pool_manager
        .get_pool("test-pool://session-router-tenant-a", "tenant-a")
        .await
        .unwrap();
    let manager = coverage_actor(
        &auth,
        &[
            codes::SUPERVISION_READ_SCHOOL,
            codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT,
        ],
        61,
    )
    .await;
    sqlx::query("UPDATE user_roles SET organization_unit_id=$2 WHERE user_id=$1")
        .bind(manager.id)
        .bind(units[0])
        .execute(&pool)
        .await
        .unwrap();
    for (user, unit) in [
        (manager.id, units[0]),
        (manager.id, units[1]),
        (first.teacher_id, units[0]),
        (first.teacher_id, units[1]),
        (second.teacher_id, units[1]),
    ] {
        sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(user).bind(unit).execute(&pool).await.unwrap();
    }
    let a = request_observation(&pool, &first).await;
    let b = request_observation(&pool, &second).await;
    for (fixture, observation) in [(&first, &a), (&second, &b)] {
        approve_with(
            &pool,
            fixture,
            observation.id,
            vec![fixture.evaluator_id, fixture.second_evaluator_id],
        )
        .await;
        services::submit_my_evaluation(
            &pool,
            fixture.evaluator_id,
            observation.id,
            evaluation_responses(&fixture.template, 4.0),
        )
        .await
        .unwrap();
    }
    let app = coverage_router(&auth);
    let scoped_reader =
        coverage_actor(&auth, &[codes::SUPERVISION_READ_ORGANIZATION_UNIT], 65).await;
    sqlx::query("UPDATE user_roles SET organization_unit_id=$2 WHERE user_id=$1")
        .bind(scoped_reader.id)
        .bind(units[0])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(scoped_reader.id).bind(units[0]).execute(&pool).await.unwrap();
    let actual_scoped = school_authorization::load_actor_context(
        scoped_reader.id,
        "tenant-a",
        &pool,
        &school_authorization::PermissionCache::new(),
    )
    .await
    .unwrap();
    assert!(
        !actual_scoped.has_any_permission(&[
            codes::SUPERVISION_READ_SCHOOL,
            codes::SUPERVISION_MANAGE_SCHOOL
        ]),
        "the metadata test must not inherit a school-wide scope"
    );
    let list = format!(
        "/api/supervision/observations?academicYearId={}",
        first.cycle.academic_year_id
    );
    let (status, listed) =
        coverage_request(&app, &auth, &manager, Method::GET, &list, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    for (observation, can_manage) in [(&a, true), (&b, false)] {
        let item = listed["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == observation.id.to_string())
            .unwrap();
        assert_eq!(item["canManage"], can_manage);
        assert_eq!(item["canReview"], can_manage);
        assert_eq!(
            item["averageRating"],
            if can_manage { json!(4.0) } else { Value::Null }
        );
        let path = format!("/api/supervision/observations/{}", observation.id);
        let (status, detail) =
            coverage_request(&app, &auth, &manager, Method::GET, &path, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["data"]["canManage"], can_manage);
        assert_eq!(detail["data"]["averageRating"], item["averageRating"]);
        let (status, review) = coverage_request(
            &app,
            &auth,
            &manager,
            Method::GET,
            &format!("{path}/review"),
            Value::Null,
        )
        .await;
        assert_eq!(
            status,
            if can_manage {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{review}"
        );
    }
    let reader = coverage_actor(&auth, &[codes::SUPERVISION_READ_SCHOOL], 63).await;
    let school_manager = coverage_actor(&auth, &[codes::SUPERVISION_MANAGE_SCHOOL], 64).await;
    let observed =
        coverage_actor_for_user(&auth, second.teacher_id, &[codes::SUPERVISION_READ_OWN], 66).await;
    let assigned = coverage_actor_for_user(
        &auth,
        second.second_evaluator_id,
        &[
            codes::SUPERVISION_READ_ASSIGNED,
            codes::SUPERVISION_EVALUATE_ASSIGNED,
        ],
        67,
    )
    .await;
    let second_path = format!("/api/supervision/observations/{}", b.id);
    for actor in [&reader, &observed, &assigned] {
        let (status, detail) =
            coverage_request(&app, &auth, actor, Method::GET, &second_path, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["data"]["canReview"], false);
        assert_eq!(detail["data"]["averageRating"], Value::Null);
        let (status, review) = coverage_request(
            &app,
            &auth,
            actor,
            Method::GET,
            &format!("{second_path}/review"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{review}");
    }
    for (cycle, scoped_rating) in [(first.cycle.id, true), (second.cycle.id, false)] {
        let path = format!("/api/supervision/reports/cycles/{cycle}/progress");
        for (actor, visible) in [
            (&reader, false),
            (&manager, scoped_rating),
            (&school_manager, true),
        ] {
            let (status, report) =
                coverage_request(&app, &auth, actor, Method::GET, &path, Value::Null).await;
            assert_eq!(status, StatusCode::OK, "{report}");
            assert_eq!(report["data"]["totalObservations"], 1);
            assert_eq!(
                report["data"]["averageRating"],
                if visible { json!(4.0) } else { Value::Null }
            );
        }
    }
    // Single-resource mutation and batched response capabilities use the same exact source.
    let actor = school_authorization::ActorContext {
        user_id: manager.id,
        permissions: vec![codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT.into()],
    };
    let users = [first.teacher_id, second.teacher_id];
    let allowed =
        school_supervision::access_policy::observation_management_user_ids(&pool, &actor, &users)
            .await
            .unwrap();
    for user in users {
        assert_eq!(
            allowed.contains(&user),
            school_supervision::access_policy::require_observation_management_access(
                &pool, &actor, user
            )
            .await
            .is_ok()
        );
    }
    let delegated = coverage_actor(&auth, &[], 62).await;
    sqlx::query("INSERT INTO organization_permission_delegations(from_user_id,to_user_id,organization_unit_id,permission_id) SELECT $1,$2,$3,id FROM permissions WHERE code=$4")
        .bind(manager.id).bind(delegated.id).bind(units[0]).bind(codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    let delegated_actor = school_authorization::ActorContext {
        user_id: delegated.id,
        permissions: vec![codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT.into()],
    };
    assert!(
        school_supervision::access_policy::require_observation_management_access(
            &pool,
            &delegated_actor,
            first.teacher_id
        )
        .await
        .is_ok(),
        "valid delegation needs no incidental membership"
    );
    let delegated_allowed = school_supervision::access_policy::observation_management_user_ids(
        &pool,
        &delegated_actor,
        &users,
    )
    .await
    .unwrap();
    assert_eq!(
        delegated_allowed,
        std::collections::HashSet::from([first.teacher_id])
    );
    let tree_role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES($1,'Tree union fixture','staff') RETURNING id").bind(format!("TREE_{}",Uuid::new_v4().simple())).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(tree_role).bind(codes::SUPERVISION_MANAGE_ORGANIZATION_TREE).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id,organization_unit_id) VALUES($1,$2,$3)")
        .bind(manager.id)
        .bind(tree_role)
        .bind(units[1])
        .execute(&pool)
        .await
        .unwrap();
    let union_actor = school_authorization::ActorContext {
        user_id: manager.id,
        permissions: vec![
            codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT.into(),
            codes::SUPERVISION_MANAGE_ORGANIZATION_TREE.into(),
        ],
    };
    assert_eq!(
        school_supervision::access_policy::observation_management_user_ids(
            &pool,
            &union_actor,
            &users
        )
        .await
        .unwrap(),
        std::collections::HashSet::from(users),
        "tree and independent exact-unit grants form a union"
    );
    for user in users {
        assert!(
            school_supervision::access_policy::require_observation_management_access(
                &pool,
                &union_actor,
                user
            )
            .await
            .is_ok()
        );
    }
    // Fresh unused registry rows exercise each current-affiliation predicate without changing history.
    let mut affiliations = Vec::new();
    for name in [
        "Temporal current",
        "Temporal future",
        "Inactive unit",
        "Inactive registry",
    ] {
        let group = school_academic_core::services::catalog::create_subject_group(
            &pool,
            school_academic_core::models::CreateSubjectGroupRequest {
                name_th: name.into(),
                name_en: name.into(),
                display_order: None,
            },
        )
        .await
        .unwrap();
        affiliations.push(group);
    }
    for (index, group) in affiliations.iter().enumerate() {
        sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at) VALUES($1,$2,'member',CURRENT_DATE+$3)").bind(first.teacher_id).bind(group.organization_unit_id).bind(if index==1 {1} else {-1}).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE organization_units SET is_active=false WHERE id=$1")
        .bind(affiliations[2].organization_unit_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE subject_groups SET is_active=false WHERE id=$1")
        .bind(affiliations[3].id)
        .execute(&pool)
        .await
        .unwrap();
    // Keep the independent projection flag active so the registry-status guard is tested itself.
    sqlx::query("UPDATE organization_units SET is_active=true WHERE id=$1")
        .bind(affiliations[3].organization_unit_id)
        .execute(&pool)
        .await
        .unwrap();
    let scoped_list = format!(
        "/api/supervision/observations?academicYearId={}&cycleId={}",
        first.cycle.academic_year_id, first.cycle.id
    );
    let scoped_status = format!(
        "/api/supervision/reports/cycles/{}/teacher-status",
        first.cycle.id
    );
    for (starts, ends, visible) in [
        (-1, None, true),
        (1, None, false),
        (-2, Some(0), false),
        (0, None, true),
    ] {
        sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE+$3,ended_at=CASE WHEN $4::int IS NULL THEN NULL ELSE CURRENT_DATE+$4 END WHERE user_id=$1 AND organization_unit_id=$2").bind(first.teacher_id).bind(units[0]).bind(starts).bind(ends).execute(&pool).await.unwrap();
        let (status, observations) = coverage_request(
            &app,
            &auth,
            &scoped_reader,
            Method::GET,
            &scoped_list,
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{observations}");
        assert_eq!(
            observations["data"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == a.id.to_string()),
            visible,
            "observation discovery must use current target membership"
        );
        let (status, teachers) = coverage_request(
            &app,
            &auth,
            &scoped_reader,
            Method::GET,
            &scoped_status,
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{teachers}");
        let first_row = teachers["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["teacherId"] == first.teacher_id.to_string());
        assert_eq!(
            first_row.is_some(),
            visible,
            "teacher discovery must agree with observation/detail access"
        );
        assert!(
            !teachers["data"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["teacherId"] == second.teacher_id.to_string()),
            "the current B target is outside the exact A grant"
        );
        if let Some(row) = first_row {
            let names = row["organizationUnitNames"].as_array().unwrap();
            assert!(names.iter().any(|name| name == &affiliations[0].name_th));
            for group in &affiliations[1..] {
                assert!(
                    !names.iter().any(|name| name == &group.name_th),
                    "future and inactive affiliation names are not current affiliations"
                );
            }
        }
        let (status, detail) = coverage_request(
            &app,
            &auth,
            &scoped_reader,
            Method::GET,
            &format!("/api/supervision/observations/{}", a.id),
            Value::Null,
        )
        .await;
        assert_eq!(
            status,
            if visible {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{detail}"
        );
    }
    sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE+1 WHERE user_id=$1")
        .bind(first.teacher_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        school_supervision::access_policy::observation_management_user_ids(&pool, &actor, &users)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        school_supervision::access_policy::require_observation_management_access(
            &pool,
            &actor,
            first.teacher_id
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE-1,ended_at=CURRENT_DATE WHERE user_id=$1").bind(first.teacher_id).execute(&pool).await.unwrap();
    assert!(
        school_supervision::access_policy::observation_management_user_ids(&pool, &actor, &users)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("UPDATE organization_members SET ended_at=NULL WHERE user_id=$1")
        .bind(first.teacher_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE organization_units SET is_active=false WHERE id=$1")
        .bind(units[0])
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        school_supervision::access_policy::observation_management_user_ids(&pool, &actor, &users)
            .await
            .unwrap()
            .is_empty()
    );
    let responses: Vec<_> = evaluation_responses(&second.template, 4.0)
        .responses
        .into_iter()
        .map(|response| {
            json!({
                "templateItemId": response.template_item_id,
                "ratingScore": response.rating_score,
                "textResponse": response.text_response,
            })
        })
        .collect();
    let (status, final_submission) = coverage_request(
        &app,
        &auth,
        &assigned,
        Method::POST,
        &format!("{second_path}/evaluations/me/submit"),
        json!({ "responses": responses }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{final_submission}");
    assert_eq!(final_submission["data"]["status"], "published");
    let (status, detail) =
        coverage_request(&app, &auth, &reader, Method::GET, &second_path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["data"]["canManage"], false);
    assert_eq!(detail["data"]["canReview"], true);
    assert_eq!(detail["data"]["averageRating"], 4.0);
    let (status, review) = coverage_request(
        &app,
        &auth,
        &observed,
        Method::GET,
        &format!("{second_path}/review"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{review}");
    assert_eq!(review["data"]["averageRating"], 4.0);
    let progress = format!(
        "/api/supervision/reports/cycles/{}/progress",
        second.cycle.id
    );
    let (status, report) =
        coverage_request(&app, &auth, &reader, Method::GET, &progress, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["data"]["publishedCount"], 1);
    assert_eq!(report["data"]["averageRating"], 4.0);
    let (status, acknowledged) = coverage_request(
        &app,
        &auth,
        &observed,
        Method::POST,
        &format!("{second_path}/acknowledge"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{acknowledged}");
    assert_eq!(acknowledged["data"]["status"], "completed");
}

#[tokio::test]
async fn physical_current_placement_removal_detaches_saved_supervision_and_preserves_evaluations() {
    let pool = migrated_pool_with_connections("supervision_current_placement_detach", 3).await;
    let fixture = insert_fixture(&pool).await;
    let (placement,block_id,teacher,group,_day):(Uuid,Uuid,Uuid,Uuid,String)=sqlx::query_as("SELECT placed.id,block.id,instructor.instructor_id,placed.learning_group_id,block.day_of_week FROM academic_timetable_blocks block JOIN academic_timetable_block_groups placed ON placed.block_id=block.id AND placed.is_active JOIN academic_timetable_block_group_instructors instructor ON instructor.block_group_id=placed.id JOIN learning_offerings offering ON offering.id=placed.learning_offering_id AND offering.kind='course' JOIN learning_group_teachers current_teacher ON current_teacher.learning_group_id=placed.learning_group_id AND current_teacher.teacher_id=instructor.instructor_id WHERE block.academic_term_id=$1 AND block.is_active ORDER BY block.id,instructor.instructor_id LIMIT 1").bind(fixture.academic_term_id).fetch_one(&pool).await.unwrap();
    let week = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
    let options = services::booking_options(
        &pool,
        teacher,
        SupervisionBookingOptionsQuery {
            cycle_id: fixture.cycle.id,
            academic_year_id: fixture.cycle.academic_year_id,
            academic_term_id: fixture.academic_term_id,
            week_start_date: week,
        },
    )
    .await
    .unwrap();
    let selected = options
        .items
        .iter()
        .find(|p| p.block_group_id == placement)
        .unwrap();
    let observation = services::request_observation(
        &pool,
        teacher,
        RequestSupervisionObservationRequest {
            cycle_id: fixture.cycle.id,
            academic_term_id: fixture.academic_term_id,
            observed_date: selected.observed_date,
            timetable_block_group_ids: vec![placement],
            expected_source_digest: options.source_digest,
        },
    )
    .await
    .unwrap();
    approve_with(&pool, &fixture, observation.id, vec![fixture.evaluator_id]).await;
    services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        observation.id,
        evaluation_responses(&fixture.template, 4.0),
    )
    .await
    .unwrap();
    let before = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    let block =
        school_academic_timetable::services::timetable_block_service::get_block(&pool, block_id)
            .await
            .unwrap();
    let mut context = school_academic_timetable::collaboration::TimetableMutationContext::for_tests(
        &pool,
        fixture.actor_id,
    );
    context.lesson_source_detachment = &crate::adapters::lesson_sources::LESSON_SOURCE_DETACHMENT;
    let removed = school_academic_timetable::services::timetable_block_service::deactivate_block(
        &pool,
        &context,
        block.id,
        block.timetable_id,
        block.row_version,
    )
    .await
    .unwrap();
    assert!(!removed.is_active);
    assert!(matches!(
        school_academic_timetable::services::timetable_block_service::get_block(&pool, block.id)
            .await,
        Err(AppError::NotFound(_))
    ));
    let after = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    assert_eq!(after.periods[0].timetable_block_group_id, None);
    assert_eq!(after.periods[0].learning_group_id, Some(group));
    assert!(after.periods[0].manual_lesson.is_none());
    assert_eq!(
        serde_json::to_value(&after.periods[0].lesson_snapshot).unwrap(),
        serde_json::to_value(&before.periods[0].lesson_snapshot).unwrap()
    );
    assert_eq!(after.status, before.status);
    assert_eq!(
        serde_json::to_value(&after.evaluators).unwrap(),
        serde_json::to_value(&before.evaluators).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&after.actions).unwrap(),
        serde_json::to_value(&before.actions).unwrap()
    );
}

#[test]
fn cycle_requests_reject_all_retired_schedule_fields() {
    let create = serde_json::json!({
        "academicYearId": Uuid::new_v4(), "title": "Manual cycle", "templateId": Uuid::new_v4(),
        "status": "open", "targets": [], "observationStartDate":"2026-01-01", "observationEndDate":"2026-12-31"
    });
    assert!(serde_json::from_value::<CreateSupervisionCycleRequest>(create.clone()).is_ok());
    for field in ["bookingOpensAt", "bookingClosesAt", "startsAt", "endsAt"] {
        let mut legacy = create.clone();
        legacy[field] = serde_json::json!("2025-01-01T00:00:00Z");
        assert!(
            serde_json::from_value::<CreateSupervisionCycleRequest>(legacy).is_err(),
            "{field}"
        );
        let mut update = serde_json::json!({});
        update[field] = serde_json::json!("2025-01-01T00:00:00Z");
        assert!(
            serde_json::from_value::<UpdateSupervisionCycleRequest>(update).is_err(),
            "{field}"
        );
    }
}

async fn corrected_request(
    pool: &PgPool,
    fixture: &SupervisionFixture,
) -> UpdateRequestedObservationRequest {
    let request = observation_input(pool, fixture).await;
    UpdateRequestedObservationRequest {
        observed_date: request.observed_date,
        timetable_block_group_ids: request.timetable_block_group_ids,
        expected_source_digest: request.expected_source_digest,
    }
}

#[tokio::test]
async fn manual_cycle_status_alone_controls_new_bookings_and_retains_actual_observation_dates() {
    let pool = migrated_pool("supervision_manual_booking").await;
    let mut fixture = insert_fixture(&pool).await;
    for status in [
        SupervisionCycleStatus::Draft,
        SupervisionCycleStatus::Closed,
        SupervisionCycleStatus::Archived,
    ] {
        services::update_cycle(
            &pool,
            fixture.cycle.id,
            UpdateSupervisionCycleRequest {
                status: Some(status),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            services::request_observation(
                &pool,
                fixture.teacher_id,
                observation_input(&pool, &fixture).await
            )
            .await,
            Err(AppError::ValidationError(_))
        ));
    }
    services::update_cycle(
        &pool,
        fixture.cycle.id,
        UpdateSupervisionCycleRequest {
            status: Some(SupervisionCycleStatus::Open),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Actual lesson dates remain facts; neither old nor future dates select the booking context.
    for observed_at in ["2000-01-03T03:00:00Z", "2100-01-04T03:00:00Z"] {
        fixture.observed_at = observed_at.parse().unwrap();
        let requested = request_observation(&pool, &fixture).await;
        assert_eq!(requested.observed_at, fixture.observed_at);
        assert_eq!(requested.academic_term_id, fixture.academic_term_id);
    }
}

#[tokio::test]
async fn own_returned_save_resubmits_atomically_and_preserves_return_evidence() {
    let pool = migrated_pool("supervision_returned_resubmit").await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    let edited = services::update_requested_observation(
        &pool,
        fixture.teacher_id,
        requested.id,
        corrected_request(&pool, &fixture).await,
    )
    .await
    .unwrap();
    assert_eq!(edited.status, SupervisionObservationStatus::Requested);
    assert_eq!(
        serde_json::to_value(&edited.actions).unwrap(),
        serde_json::to_value(&requested.actions).unwrap(),
        "ordinary pending edits do not add history events"
    );
    services::return_observation_request(
        &pool,
        fixture.actor_id,
        requested.id,
        ReturnObservationRequest {
            comment: Some("Please correct the lesson".into()),
        },
    )
    .await
    .unwrap();
    let returned = services::get_observation(&pool, requested.id)
        .await
        .unwrap();
    assert_eq!(returned.status, SupervisionObservationStatus::Returned);
    let before = serde_json::to_value(&returned).unwrap();
    assert!(matches!(
        services::update_requested_observation(
            &pool,
            fixture.second_evaluator_id,
            requested.id,
            corrected_request(&pool, &fixture).await
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    for status in [
        SupervisionCycleStatus::Draft,
        SupervisionCycleStatus::Closed,
        SupervisionCycleStatus::Archived,
    ] {
        services::update_cycle(
            &pool,
            fixture.cycle.id,
            UpdateSupervisionCycleRequest {
                status: Some(status),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            services::update_requested_observation(
                &pool,
                fixture.teacher_id,
                requested.id,
                corrected_request(&pool, &fixture).await
            )
            .await,
            Err(AppError::ValidationError(_))
        ));
        assert_eq!(
            serde_json::to_value(
                services::get_observation(&pool, requested.id)
                    .await
                    .unwrap()
            )
            .unwrap(),
            before
        );
    }
    services::update_cycle(
        &pool,
        fixture.cycle.id,
        UpdateSupervisionCycleRequest {
            status: Some(SupervisionCycleStatus::Open),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(fixture.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        services::update_requested_observation(
            &pool,
            fixture.teacher_id,
            requested.id,
            corrected_request(&pool, &fixture).await
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        serde_json::to_value(
            services::get_observation(&pool, requested.id)
                .await
                .unwrap()
        )
        .unwrap(),
        before
    );
    sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
        .bind(fixture.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    let resubmitted = services::update_requested_observation(
        &pool,
        fixture.teacher_id,
        requested.id,
        corrected_request(&pool, &fixture).await,
    )
    .await
    .unwrap();
    assert_eq!(resubmitted.status, SupervisionObservationStatus::Requested);
    assert!(resubmitted.periods[0].manual_lesson.is_none());
    assert!(resubmitted.periods[0].timetable_block_group_id.is_some());
    assert!(resubmitted
        .actions
        .iter()
        .any(|action| action.action_kind == "request_returned"
            && action.comment.as_deref() == Some("Please correct the lesson")));
    assert!(resubmitted
        .actions
        .iter()
        .any(|action| action.from_status.as_deref() == Some("returned")
            && action.to_status.as_deref() == Some("requested")));
    approve_with(&pool, &fixture, requested.id, vec![fixture.evaluator_id]).await;
    assert!(matches!(
        services::update_requested_observation(
            &pool,
            fixture.teacher_id,
            requested.id,
            corrected_request(&pool, &fixture).await
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    let planned = services::get_observation(&pool, requested.id)
        .await
        .unwrap();
    services::return_observation_request(
        &pool,
        fixture.actor_id,
        requested.id,
        ReturnObservationRequest {
            comment: Some("Correct planned lesson".into()),
        },
    )
    .await
    .unwrap();
    assert!(
        matches!(
            services::submit_my_evaluation(
                &pool,
                fixture.evaluator_id,
                requested.id,
                evaluation_responses(&fixture.template, 4.0)
            )
            .await,
            Err(AppError::ValidationError(_))
        ),
        "a returned request cannot acquire a new submitted evaluation"
    );
    let resubmitted_planned =
        services::resubmit_observation_request(&pool, fixture.teacher_id, requested.id)
            .await
            .unwrap();
    assert_eq!(
        resubmitted_planned.status,
        SupervisionObservationStatus::Requested
    );
    assert_eq!(
        serde_json::to_value(&resubmitted_planned.evaluators).unwrap(),
        serde_json::to_value(&planned.evaluators).unwrap(),
        "saving correction retains existing evaluator identities until approval"
    );
    let another = services::request_observation(
        &pool,
        fixture.teacher_id,
        input_at(
            &pool,
            &fixture,
            fixture.teacher_id,
            fixture.observed_at + Duration::hours(1),
        )
        .await,
    )
    .await
    .unwrap();
    services::return_observation_request(
        &pool,
        fixture.actor_id,
        another.id,
        ReturnObservationRequest { comment: None },
    )
    .await
    .unwrap();
    let cancelled = services::cancel_requested_observation(&pool, fixture.teacher_id, another.id)
        .await
        .unwrap();
    assert_eq!(cancelled.status, SupervisionObservationStatus::Cancelled);
}

#[tokio::test]
async fn submitted_evidence_blocks_return_resubmit_and_reapproval_without_mutation() {
    let pool = migrated_pool("supervision_return_submitted").await;
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
    .unwrap();
    let review_before = serde_json::to_value(
        services::get_observation_review(&pool, requested.id)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        services::return_observation_request(
            &pool,
            fixture.actor_id,
            requested.id,
            ReturnObservationRequest {
                comment: Some("Unsafe return".into())
            }
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    assert_eq!(
        serde_json::to_value(
            services::get_observation_review(&pool, requested.id)
                .await
                .unwrap()
        )
        .unwrap(),
        review_before
    );
    // A legacy returned row can already contain submissions; every new path must retain it exactly.
    sqlx::query("UPDATE supervision_observations SET status='returned' WHERE id=$1")
        .bind(requested.id)
        .execute(&pool)
        .await
        .unwrap();
    let legacy_before = serde_json::to_value(
        services::get_observation_review(&pool, requested.id)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        services::update_requested_observation(
            &pool,
            fixture.teacher_id,
            requested.id,
            corrected_request(&pool, &fixture).await
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        services::resubmit_observation_request(&pool, fixture.teacher_id, requested.id).await,
        Err(AppError::ValidationError(_))
    ));
    assert!(matches!(
        services::approve_observation_request(
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
        .await,
        Err(AppError::ValidationError(_))
    ));
    assert_eq!(
        serde_json::to_value(
            services::get_observation_review(&pool, requested.id)
                .await
                .unwrap()
        )
        .unwrap(),
        legacy_before
    );
}

#[tokio::test]
async fn return_and_evaluation_submission_serialize_without_stranding_or_erasing_evidence() {
    let pool = migrated_pool_with_connections("supervision_return_submit_race", 3).await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    approve_with(
        &pool,
        &fixture,
        requested.id,
        vec![fixture.evaluator_id, fixture.second_evaluator_id],
    )
    .await;
    let (returned, submitted) = tokio::join!(
        services::return_observation_request(
            &pool,
            fixture.actor_id,
            requested.id,
            ReturnObservationRequest {
                comment: Some("Correct lesson".into())
            }
        ),
        services::submit_my_evaluation(
            &pool,
            fixture.evaluator_id,
            requested.id,
            evaluation_responses(&fixture.template, 4.0)
        ),
    );
    let final_state = services::get_observation(&pool, requested.id)
        .await
        .unwrap();
    match (returned, submitted) {
        (Ok(_), Err(AppError::ValidationError(_))) => {
            assert_eq!(final_state.status, SupervisionObservationStatus::Returned);
            assert!(final_state
                .evaluators
                .iter()
                .all(|evaluator| evaluator.status != SupervisionEvaluatorStatus::Submitted));
            services::resubmit_observation_request(&pool, fixture.teacher_id, requested.id)
                .await
                .unwrap();
        }
        (Err(AppError::ValidationError(_)), Ok(_)) => {
            assert_eq!(final_state.status, SupervisionObservationStatus::Planned);
            assert!(final_state
                .evaluators
                .iter()
                .any(
                    |evaluator| evaluator.evaluator_user_id == fixture.evaluator_id
                        && evaluator.status == SupervisionEvaluatorStatus::Submitted
                ));
            assert!(!final_state
                .actions
                .iter()
                .any(|action| action.action_kind == "request_returned"));
        }
        other => panic!("exactly one serialized transition must win: {other:?}"),
    }
}

async fn supervision_graph_snapshot(pool: &PgPool) -> serde_json::Value {
    sqlx::query_scalar(r#"SELECT jsonb_build_object(
        'cycles', (SELECT jsonb_agg(to_jsonb(cycle)-'booking_opens_at'-'booking_closes_at'-'starts_at'-'ends_at' ORDER BY id) FROM supervision_cycles cycle),
        'targets', (SELECT jsonb_agg(to_jsonb(target) ORDER BY id) FROM supervision_cycle_targets target),
        'observations', (SELECT jsonb_agg(to_jsonb(observation) ORDER BY id) FROM supervision_observations observation),
        'evaluators', (SELECT jsonb_agg(to_jsonb(evaluator) ORDER BY id) FROM supervision_evaluators evaluator),
        'responses', (SELECT jsonb_agg(to_jsonb(response) ORDER BY id) FROM supervision_evaluator_responses response),
        'actions', (SELECT jsonb_agg(to_jsonb(action) ORDER BY id) FROM supervision_actions action)
    )"#).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn supervision_manual_cycle_migration_preserves_populated_graph_and_refuses_dependencies_atomically(
) {
    let pool = migrated_pool_through("supervision_manual_migration159", 1, 158).await;
    // The historical fixture already has a planned observation and a nonempty cycle.
    // Add every manual status plus submitted evaluator evidence before the destructive boundary.
    sqlx::raw_sql(r#"
        INSERT INTO supervision_cycles(academic_year_id,academic_term_id,title,description,template_id,booking_opens_at,booking_closes_at,starts_at,ends_at,status,created_by)
        SELECT academic_year_id,academic_term_id,'Retained '||value,description,template_id,'2001-01-01 00:00:00+00'::timestamptz,'2099-12-31 00:00:00+00'::timestamptz,starts_at,ends_at,value,created_by
        FROM supervision_cycles CROSS JOIN unnest(ARRAY['draft','closed','archived']) AS value;
        INSERT INTO supervision_cycle_targets(cycle_id,target_type,required_observations,priority)
        SELECT id,'school',2,10 FROM supervision_cycles;
        INSERT INTO supervision_template_sections(id,template_id,title,sort_order)
        VALUES('a0000000-0000-0000-0000-000000000159','87000000-0000-0000-0000-000000000001','Retained section',1);
        INSERT INTO supervision_template_items(id,section_id,label,item_type,required,sort_order)
        VALUES('a1000000-0000-0000-0000-000000000159','a0000000-0000-0000-0000-000000000159','Retained evidence','rating',true,1);
        INSERT INTO supervision_evaluators(id,observation_id,evaluator_user_id,status,submitted_at)
        VALUES('a2000000-0000-0000-0000-000000000159','89000000-0000-0000-0000-000000000001','50000000-0000-0000-0000-000000000001','submitted',now());
        INSERT INTO supervision_evaluator_responses(observation_id,evaluator_id,template_item_id,rating_score,text_response)
        VALUES('89000000-0000-0000-0000-000000000001','a2000000-0000-0000-0000-000000000159','a1000000-0000-0000-0000-000000000159',4,'Retain exactly');
        INSERT INTO supervision_actions(observation_id,actor_user_id,action_kind,from_status,to_status,comment)
        VALUES('89000000-0000-0000-0000-000000000001','50000000-0000-0000-0000-000000000001','request_returned','requested','returned','Retained return reason');
        CREATE VIEW supervision159_unexpected_dependency AS SELECT starts_at FROM supervision_cycles;
    "#).execute(&pool).await.unwrap();
    let before = supervision_graph_snapshot(&pool).await;
    let error = apply_migrations_through(&pool, 159)
        .await
        .expect_err("RESTRICT must refuse an unknown dependent view");
    assert!(error.to_string().contains("159"), "{error}");
    let retired_columns: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='supervision_cycles' AND column_name=ANY($1)")
        .bind(vec!["booking_opens_at", "booking_closes_at", "starts_at", "ends_at"])
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        retired_columns, 4,
        "all dropped columns roll back on dependency refusal"
    );
    let checks: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_constraint WHERE conrelid='supervision_cycles'::regclass AND conname IN ('supervision_cycles_window_check','supervision_cycles_booking_window_check')").fetch_one(&pool).await.unwrap();
    assert_eq!(checks, 2, "named check removals also roll back");
    let recorded: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version=159)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!recorded);
    assert_eq!(supervision_graph_snapshot(&pool).await, before);
    sqlx::query("DROP VIEW supervision159_unexpected_dependency")
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 159).await.unwrap();
    apply_migrations_through(&pool, 159)
        .await
        .expect("native migration replay is idempotent");
    assert_eq!(supervision_graph_snapshot(&pool).await, before);
    let cycle: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(cycle) FROM supervision_cycles cycle ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for field in [
        "booking_opens_at",
        "booking_closes_at",
        "starts_at",
        "ends_at",
    ] {
        assert!(
            cycle.get(field).is_none(),
            "retired column {field} must be physically absent"
        );
    }
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT status FROM supervision_cycles ORDER BY status")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(statuses, vec!["archived", "closed", "draft", "open"]);
}

#[tokio::test]
async fn returned_resubmission_rolls_back_lesson_and_status_if_audit_write_fails() {
    let pool = migrated_pool("supervision_resubmit_atomic").await;
    let fixture = insert_fixture(&pool).await;
    let requested = request_observation(&pool, &fixture).await;
    services::return_observation_request(
        &pool,
        fixture.actor_id,
        requested.id,
        ReturnObservationRequest {
            comment: Some("Correction required".into()),
        },
    )
    .await
    .unwrap();
    let before = serde_json::to_value(
        services::get_observation(&pool, requested.id)
            .await
            .unwrap(),
    )
    .unwrap();
    sqlx::raw_sql(r#"
        CREATE FUNCTION supervision159_refuse_resubmit_action() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.from_status='returned' AND NEW.to_status='requested' THEN
                RAISE EXCEPTION 'owned resubmission atomicity fixture';
            END IF;
            RETURN NEW;
        END $$;
        CREATE TRIGGER supervision159_refuse_resubmit_action BEFORE INSERT ON supervision_actions
        FOR EACH ROW EXECUTE FUNCTION supervision159_refuse_resubmit_action();
    "#).execute(&pool).await.unwrap();
    assert!(services::update_requested_observation(
        &pool,
        fixture.teacher_id,
        requested.id,
        corrected_request(&pool, &fixture).await
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
        before
    );
}

#[tokio::test]
async fn optional_evaluators_may_contribute_until_automatic_publication() {
    use crate::adapters::notification_delivery::APPLICATION_NOTIFICATION_ELIGIBILITY;
    use school_notifications::dispatch;

    let pool = migrated_pool("supervision_optional_before_publication").await;
    let fixture = insert_fixture(&pool).await;
    let late_optional = test_user(&pool, "late-optional").await;
    let requested = request_observation(&pool, &fixture).await;
    let planned = services::approve_observation_request(
        &pool,
        fixture.actor_id,
        requested.id,
        ApproveObservationRequest {
            evaluators: [
                (fixture.evaluator_id, true),
                (fixture.second_evaluator_id, false),
                (late_optional, false),
            ]
            .into_iter()
            .map(|(evaluator_user_id, required)| EvaluatorAssignmentInput {
                evaluator_user_id,
                role_label: None,
                is_required: Some(required),
            })
            .collect(),
        },
    )
    .await
    .unwrap();
    let booking_action = planned
        .actions
        .iter()
        .find(|action| action.action_kind == "request_approved")
        .unwrap();
    let stale_assignment = dispatch::pending(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| {
            entry.source
                == (dispatch::NotificationSource::SupervisionAction {
                    source_id: booking_action.id,
                })
                && entry.recipient_id == late_optional
        })
        .unwrap();
    let late_assignment = planned
        .evaluators
        .iter()
        .find(|evaluator| evaluator.evaluator_user_id == late_optional)
        .unwrap();
    sqlx::query("UPDATE supervision_evaluators SET status='draft' WHERE id=$1")
        .bind(late_assignment.id)
        .execute(&pool)
        .await
        .unwrap();
    let text_item = fixture
        .template
        .sections
        .iter()
        .flat_map(|section| &section.items)
        .find(|item| item.item_type == SupervisionTemplateItemType::Text)
        .unwrap();
    sqlx::query(
        "INSERT INTO supervision_evaluator_responses
            (observation_id,evaluator_id,template_item_id,text_response)
         VALUES ($1,$2,$3,'Unsubmitted optional evidence')",
    )
    .bind(requested.id)
    .bind(late_assignment.id)
    .bind(text_item.id)
    .execute(&pool)
    .await
    .unwrap();
    let optional_complete = services::submit_my_evaluation(
        &pool,
        fixture.second_evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 2.0),
    )
    .await
    .expect("an assigned optional evaluator may contribute before publication");
    assert_eq!(
        optional_complete.status,
        SupervisionObservationStatus::Planned
    );
    assert!(optional_complete
        .evaluators
        .iter()
        .any(
            |evaluator| evaluator.evaluator_user_id == fixture.second_evaluator_id
                && !evaluator.is_required
                && evaluator.status == SupervisionEvaluatorStatus::Submitted
        ));
    let required_complete = services::submit_my_evaluation(
        &pool,
        fixture.evaluator_id,
        requested.id,
        evaluation_responses(&fixture.template, 4.0),
    )
    .await
    .unwrap();
    assert_eq!(
        required_complete.status,
        SupervisionObservationStatus::Published
    );
    assert!(dispatch::consume(
        &pool,
        &stale_assignment,
        &APPLICATION_NOTIFICATION_ELIGIBILITY
    )
    .await
    .unwrap()
    .is_none());
    let retained_assignment: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM notification_receipts r
         JOIN notifications n ON n.id=r.notification_id
         WHERE r.source_kind='supervision_action' AND r.source_id=$1 AND r.recipient_id=$2)",
    )
    .bind(booking_action.id)
    .bind(late_optional)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(retained_assignment);
    let review = services::get_observation_review(&pool, requested.id)
        .await
        .unwrap();
    assert_eq!(review.average_rating, Some(3.0));
    assert!(review
        .evaluator_results
        .iter()
        .find(|evaluator| evaluator.evaluator_user_id == late_optional)
        .unwrap()
        .responses
        .is_empty());
    let before = serde_json::to_value(
        services::get_observation_review(&pool, requested.id)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        services::submit_my_evaluation(
            &pool,
            late_optional,
            requested.id,
            evaluation_responses(&fixture.template, 1.0),
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    assert_eq!(
        serde_json::to_value(
            services::get_observation_review(&pool, requested.id)
                .await
                .unwrap()
        )
        .unwrap(),
        before,
        "publication freezes both existing and missing optional evidence"
    );
}

#[tokio::test]
async fn supervision_concurrent_required_submissions_publish_once_and_preserve_replay_evidence() {
    let pool = migrated_pool_with_connections("supervision_publication_concurrent", 4).await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    let planned = approve_with(
        &pool,
        &f,
        observation.id,
        vec![f.evaluator_id, f.second_evaluator_id],
    )
    .await;
    let first = evaluation_responses(&f.template, 3.0);
    let second = evaluation_responses(&f.template, 4.0);
    let mut missing_text = first.clone();
    missing_text
        .responses
        .iter_mut()
        .find(|response| response.text_response.is_some())
        .unwrap()
        .text_response = Some("   ".into());
    let mut missing_score = first.clone();
    missing_score
        .responses
        .iter_mut()
        .find(|response| response.rating_score.is_some())
        .unwrap()
        .rating_score = None;
    for incomplete in [
        SaveEvaluationRequest::default(),
        missing_text,
        missing_score,
    ] {
        assert!(matches!(
            services::submit_my_evaluation(&pool, f.evaluator_id, observation.id, incomplete,)
                .await,
            Err(AppError::ValidationError(_))
        ));
    }
    let after_rejection = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(after_rejection).unwrap(),
        serde_json::to_value(planned).unwrap()
    );

    let (initial, repeated, other) = tokio::join!(
        services::submit_my_evaluation(&pool, f.evaluator_id, observation.id, first.clone()),
        services::submit_my_evaluation(&pool, f.evaluator_id, observation.id, first),
        services::submit_my_evaluation(&pool, f.second_evaluator_id, observation.id, second),
    );
    let results = [initial.unwrap(), repeated.unwrap(), other.unwrap()];
    assert!(results
        .iter()
        .any(|result| result.status == SupervisionObservationStatus::Published));
    let released = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    assert_eq!(released.status, SupervisionObservationStatus::Published);
    assert_eq!(
        released
            .actions
            .iter()
            .filter(|action| action.action_kind == "evaluator_submitted")
            .count(),
        2
    );
    let publication_actions: Vec<_> = released
        .actions
        .iter()
        .filter(|action| action.action_kind == "result_published")
        .collect();
    assert_eq!(publication_actions.len(), 1);
    assert_eq!(
        publication_actions[0].from_status.as_deref(),
        Some("planned")
    );
    assert_eq!(
        publication_actions[0].to_status.as_deref(),
        Some("published")
    );
    let recipients: Vec<Uuid> = sqlx::query_scalar("SELECT recipient_id FROM notification_receipts WHERE source_kind='supervision_action' AND source_id=$1")
        .bind(publication_actions[0].id).fetch_all(&pool).await.unwrap();
    assert_eq!(recipients, vec![f.teacher_id]);
    assert!(matches!(
        services::submit_my_evaluation(
            &pool,
            f.evaluator_id,
            observation.id,
            evaluation_responses(&f.template, 2.0),
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    let replay = services::submit_my_evaluation(
        &pool,
        f.evaluator_id,
        observation.id,
        SaveEvaluationRequest::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(released).unwrap()
    );
}

#[tokio::test]
async fn supervision_confirmed_roster_replacement_releases_only_with_submitted_required_evidence() {
    let pool = migrated_pool("supervision_roster_publication").await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    let optional = test_user(&pool, "submitted-optional").await;
    let new_optional = test_user(&pool, "new-optional").await;
    let assignment = |evaluator_user_id, required| EvaluatorAssignmentInput {
        evaluator_user_id,
        role_label: None,
        is_required: Some(required),
    };
    services::approve_observation_request(
        &pool,
        f.actor_id,
        observation.id,
        ApproveObservationRequest {
            evaluators: vec![
                assignment(f.evaluator_id, true),
                assignment(f.second_evaluator_id, true),
                assignment(optional, false),
            ],
        },
    )
    .await
    .unwrap();
    let prior = services::submit_my_evaluation(
        &pool,
        optional,
        observation.id,
        evaluation_responses(&f.template, 2.0),
    )
    .await
    .unwrap();
    let replace = |evaluators| {
        services::replace_observation_evaluators(
            &pool,
            f.actor_id,
            observation.id,
            ReplaceObservationEvaluatorsRequest { evaluators },
        )
    };
    assert!(matches!(
        replace(vec![assignment(optional, true)]).await,
        Err(AppError::ValidationError(_))
    ));
    let same = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(same).unwrap(),
        serde_json::to_value(prior).unwrap()
    );

    let partial = services::submit_my_evaluation(
        &pool,
        f.evaluator_id,
        observation.id,
        evaluation_responses(&f.template, 4.0),
    )
    .await
    .unwrap();
    assert_eq!(partial.status, SupervisionObservationStatus::Planned);
    let released = replace(vec![assignment(new_optional, false)])
        .await
        .unwrap();
    assert_eq!(released.status, SupervisionObservationStatus::Published);
    assert_eq!(released.average_rating, Some(3.0));
    assert!(released.evaluators.iter().any(|evaluator| {
        evaluator.evaluator_user_id == optional
            && !evaluator.is_required
            && evaluator.status == SupervisionEvaluatorStatus::Submitted
    }));
    let notices: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT a.action_kind,r.recipient_id FROM notification_receipts r
         JOIN supervision_actions a ON a.id=r.source_id
         WHERE r.source_kind='supervision_action' AND a.observation_id=$1
         AND a.action_kind IN ('evaluators_updated','result_published') ORDER BY a.action_kind",
    )
    .bind(observation.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        notices,
        vec![
            ("evaluators_updated".into(), f.second_evaluator_id),
            ("result_published".into(), f.teacher_id)
        ]
    );
}

#[tokio::test]
async fn supervision_publication_migration_reconciles_pending_rows_and_preserves_read_grants() {
    let pool = migrated_pool_through("supervision_publication_migration", 1, 168).await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    approve_with(&pool, &f, observation.id, vec![f.evaluator_id]).await;

    let fixtures = [
        (
            "evaluators_submitted",
            vec![(true, "submitted"), (false, "draft")],
            "published",
        ),
        (
            "under_review",
            vec![(true, "submitted"), (true, "assigned")],
            "planned",
        ),
        ("approved", vec![(false, "submitted")], "planned"),
        ("approved", vec![], "planned"),
        ("published", vec![(false, "submitted")], "published"),
        ("completed", vec![], "completed"),
    ];
    let item_id = f.template.sections[0].items[0].id;
    let mut migrated = Vec::new();
    for (status, evidence, expected) in fixtures {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO supervision_observations
            SELECT (jsonb_populate_record(NULL::supervision_observations,
                to_jsonb(o)||jsonb_build_object('id',$2::uuid,'status',$3::text))).*
            FROM supervision_observations o WHERE id=$1",
        )
        .bind(observation.id)
        .bind(id)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO supervision_observation_periods
            SELECT (jsonb_populate_record(NULL::supervision_observation_periods,
                to_jsonb(p)||jsonb_build_object('id',uuidv7(),'observation_id',$2::uuid))).*
            FROM supervision_observation_periods p WHERE observation_id=$1",
        )
        .bind(observation.id)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        for (user_id, (required, evaluator_status)) in [f.evaluator_id, f.second_evaluator_id]
            .into_iter()
            .zip(evidence)
        {
            let evaluator_id: Uuid = sqlx::query_scalar(
                "INSERT INTO supervision_evaluators
                    (observation_id,evaluator_user_id,status,is_required,submitted_at)
                 VALUES ($1,$2,$3,$4,CASE WHEN $3='submitted' THEN now() END) RETURNING id",
            )
            .bind(id)
            .bind(user_id)
            .bind(evaluator_status)
            .bind(required)
            .fetch_one(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO supervision_evaluator_responses
                    (observation_id,evaluator_id,template_item_id,rating_score) VALUES ($1,$2,$3,4)",
            ).bind(id).bind(evaluator_id).bind(item_id).execute(&pool).await.unwrap();
        }
        sqlx::query(
            "INSERT INTO supervision_actions (observation_id,actor_user_id,action_kind,from_status,to_status,comment)
             VALUES ($1,$2,'subject_group_certified','evaluators_submitted','approved','Historical evidence')",
        ).bind(id).bind(f.actor_id).execute(&pool).await.unwrap();
        migrated.push((id, status, expected));
    }

    let snapshot = |ids: Vec<Uuid>| {
        sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT jsonb_build_array(
            (SELECT jsonb_agg(to_jsonb(o)-'status' ORDER BY id) FROM supervision_observations o WHERE id=ANY($1)),
            (SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM supervision_observation_periods p WHERE observation_id=ANY($1)),
            (SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM supervision_evaluators e WHERE observation_id=ANY($1)),
            (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM supervision_evaluator_responses r WHERE observation_id=ANY($1)),
            (SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM supervision_actions a WHERE observation_id=ANY($1)
              AND action_kind<>'result_status_migrated'),
            (SELECT count(*) FROM notification_receipts))",
    ).bind(ids).fetch_one(&pool)
    };
    let ids: Vec<_> = migrated.iter().map(|row| row.0).collect();
    let before = snapshot(ids.clone()).await.unwrap();

    let approval_id: Uuid =
        sqlx::query_scalar("SELECT id FROM permissions WHERE code='supervision.approve.school'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let role_id: Uuid = sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES ('SUPERVISION169_TEST','Publication fixture','staff') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) VALUES ($1,$2)")
        .bind(role_id)
        .bind(approval_id)
        .execute(&pool)
        .await
        .unwrap();
    let delegation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_permission_delegations (from_user_id,to_user_id,permission_id,reason,started_at,expires_at)
         VALUES ($1,$2,$3,'Coverage',now()-interval '1 day',now()+interval '1 day') RETURNING id",
    ).bind(f.actor_id).bind(f.evaluator_id).bind(approval_id).fetch_one(&pool).await.unwrap();
    let delegation_snapshot = || {
        sqlx::query_as::<_, (Uuid, serde_json::Value)>(
        "SELECT permission_id,to_jsonb(d)-'permission_id' FROM organization_permission_delegations d WHERE id=$1",
    ).bind(delegation_id).fetch_one(&pool)
    };
    let (original_permission, delegation_before) = delegation_snapshot().await.unwrap();
    assert_eq!(original_permission, approval_id);
    let position_grants = |permission| {
        sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT organization_unit_id,position_code FROM organization_permission_grants
         WHERE permission_id=$1 ORDER BY organization_unit_id,position_code",
        )
        .bind(permission)
        .fetch_all(&pool)
    };
    let grants_before = position_grants(approval_id).await.unwrap();
    assert!(!grants_before.is_empty());
    let existing_read_id: Uuid = sqlx::query_scalar(
        "INSERT INTO permissions(code,name,module,action,scope)
         VALUES ('supervision.read.school','Existing readers','supervision','read','school')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (unit_id, position) = &grants_before[0];
    sqlx::query(
        "INSERT INTO organization_permission_grants
            (organization_unit_id,permission_id,created_at,created_by,position_code)
         VALUES ($1,$2,'2026-01-01T00:00:00Z',$3,$4)",
    )
    .bind(unit_id)
    .bind(existing_read_id)
    .bind(f.actor_id)
    .bind(position)
    .execute(&pool)
    .await
    .unwrap();
    let existing_grant = || {
        sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT to_jsonb(g) FROM organization_permission_grants g
         WHERE organization_unit_id=$1 AND permission_id=$2 AND position_code IS NOT DISTINCT FROM $3",
    ).bind(unit_id).bind(existing_read_id).bind(position).fetch_one(&pool)
    };
    let existing_grant_before = existing_grant().await.unwrap();
    let revoked_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_permission_delegations
            (from_user_id,to_user_id,permission_id,revoked_at)
         VALUES ($1,$2,$3,now()) RETURNING id",
    )
    .bind(f.actor_id)
    .bind(f.evaluator_id)
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        "CREATE FUNCTION drift_supervision_169() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN NEW.requested_at=NEW.requested_at+interval '1 day'; RETURN NEW; END $$;
         CREATE TRIGGER drift_supervision_169 BEFORE UPDATE ON supervision_observations
         FOR EACH ROW EXECUTE FUNCTION drift_supervision_169();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let error = apply_migrations_through(&pool, 169).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("SUPERVISION169_STATUS_RECONCILIATION_FAILED"));
    assert_eq!(snapshot(ids.clone()).await.unwrap(), before);
    assert_eq!(delegation_snapshot().await.unwrap().0, approval_id);
    sqlx::query("DROP TRIGGER drift_supervision_169 ON supervision_observations")
        .execute(&pool)
        .await
        .unwrap();

    for _ in 0..2 {
        apply_migrations_through(&pool, 169).await.unwrap();
        assert_eq!(snapshot(ids.clone()).await.unwrap(), before);
        for &(id, prior, status) in &migrated {
            let current = services::get_observation(&pool, id).await.unwrap();
            assert_eq!(current.status.as_str(), status);
            assert!(current.actions.iter().any(|action| {
                action.action_kind == "subject_group_certified"
                    && action.from_status.as_deref() == Some("evaluators_submitted")
                    && action.to_status.as_deref() == Some("approved")
                    && action.comment.as_deref() == Some("Historical evidence")
            }));
            let changes: Vec<_> = current
                .actions
                .iter()
                .filter(|action| action.action_kind == "result_status_migrated")
                .collect();
            if prior == status {
                assert!(changes.is_empty());
            } else {
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].from_status.as_deref(), Some(prior));
                assert_eq!(changes[0].to_status.as_deref(), Some(status));
                assert_eq!(changes[0].actor_user_id, None);
            }
        }
    }
    assert!(
        sqlx::query("UPDATE supervision_observations SET status='approved' WHERE id=$1")
            .bind(ids[0])
            .execute(&pool)
            .await
            .is_err()
    );

    let read_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM permissions WHERE code='supervision.read.school' AND is_active",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let retired: bool = sqlx::query_scalar("SELECT NOT is_active FROM permissions WHERE id=$1")
        .bind(approval_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(retired);
    assert_eq!(read_id, existing_read_id);
    let role_has_read: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM role_permissions WHERE role_id=$1 AND permission_id=$2)",
    )
    .bind(role_id)
    .bind(read_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(role_has_read);
    let retired_role_grants: i64 =
        sqlx::query_scalar("SELECT count(*) FROM role_permissions WHERE permission_id=$1")
            .bind(approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retired_role_grants, 0);
    assert!(position_grants(approval_id).await.unwrap().is_empty());
    assert_eq!(position_grants(read_id).await.unwrap(), grants_before);
    assert_eq!(existing_grant().await.unwrap(), existing_grant_before);
    assert_eq!(
        delegation_snapshot().await.unwrap(),
        (read_id, delegation_before)
    );
    let revoked_permission: Uuid = sqlx::query_scalar(
        "SELECT permission_id FROM organization_permission_delegations WHERE id=$1",
    )
    .bind(revoked_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked_permission, approval_id);
}

async fn options_for(
    pool: &PgPool,
    f: &SupervisionFixture,
    teacher: Uuid,
    date: chrono::NaiveDate,
) -> SupervisionBookingOptions {
    services::booking_options(
        pool,
        teacher,
        SupervisionBookingOptionsQuery {
            cycle_id: f.cycle.id,
            academic_year_id: f.cycle.academic_year_id,
            academic_term_id: f.academic_term_id,
            week_start_date: date
                - Duration::days(i64::from(date.weekday().num_days_from_monday())),
        },
    )
    .await
    .unwrap()
}

async fn multi_input(
    pool: &PgPool,
    f: &SupervisionFixture,
    teacher: Uuid,
    date: chrono::NaiveDate,
    count: usize,
) -> RequestSupervisionObservationRequest {
    let options = options_for(pool, f, teacher, date).await;
    let mut items = options
        .items
        .iter()
        .filter(|p| p.observed_date == date && p.period_name.starts_with("Supervision fixture"))
        .collect::<Vec<_>>();
    items.sort_by_key(|p| p.start_time);
    RequestSupervisionObservationRequest {
        cycle_id: f.cycle.id,
        academic_term_id: f.academic_term_id,
        observed_date: date,
        timetable_block_group_ids: items
            .iter()
            .take(count)
            .rev()
            .map(|p| p.block_group_id)
            .collect(),
        expected_source_digest: options.source_digest,
    }
}

#[tokio::test]
async fn supervision_books_unlimited_consecutive_periods_and_counts_one_observation() {
    let pool = migrated_pool("supervision_multi_period_round_trip").await;
    let f = insert_fixture(&pool).await;
    for count in 1..=3 {
        let date = f.observed_at.date_naive() + Duration::weeks(count as i64);
        let input = multi_input(&pool, &f, f.teacher_id, date, count).await;
        let observation = services::request_observation(&pool, f.teacher_id, input)
            .await
            .unwrap();
        assert_eq!(observation.periods.len(), count);
        assert_eq!(observation.observed_at, observation.periods[0].observed_at);
        assert_eq!(
            observation.periods[0]
                .observed_at
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .hour(),
            10
        );
        for (index, period) in observation.periods.iter().enumerate() {
            assert_eq!(period.position, index as i32 + 1);
            assert_eq!(period.homeroom_id, observation.periods[0].homeroom_id);
            assert!(period.manual_lesson.is_none());
            if index > 0 {
                assert_eq!(
                    observation.periods[index - 1].ends_at,
                    Some(period.observed_at)
                );
            }
        }
        assert!(matches!(
            services::request_observation(
                &pool,
                f.teacher_id,
                multi_input(&pool, &f, f.teacher_id, date, count).await
            )
            .await,
            Err(AppError::Conflict(_))
        ));
    }
    let progress = services::cycle_progress(
        &pool,
        &school_authorization::ActorContext {
            user_id: f.actor_id,
            permissions: vec![crate::test_permission_codes::SUPERVISION_MANAGE_SCHOOL.into()],
        },
        f.cycle.id,
    )
    .await
    .unwrap();
    assert_eq!(progress.total_observations, 3);
    let report = services::cycle_teacher_status(
        &pool,
        services::SupervisionObservationListAccess::school(),
        f.cycle.id,
    )
    .await
    .unwrap();
    let teacher = report
        .iter()
        .find(|r| r.teacher_id == f.teacher_id)
        .unwrap();
    let title = teacher.lesson_title.as_deref().unwrap();
    assert!(
        title.contains("Supervision fixture 10") && title.contains("Supervision fixture 12"),
        "{title}"
    );
}

#[tokio::test]
async fn supervision_selection_rejects_forgery_staleness_and_preserves_atomic_edits() {
    let pool = migrated_pool("supervision_multi_period_denials").await;
    let f = insert_fixture(&pool).await;
    let date = f.observed_at.date_naive();
    let valid = multi_input(&pool, &f, f.teacher_id, date, 3).await;
    for forged in [
        vec![valid.timetable_block_group_ids[0]; 2],
        vec![
            valid.timetable_block_group_ids[0],
            valid.timetable_block_group_ids[2],
        ],
        vec![Uuid::new_v4()],
    ] {
        let mut input = valid.clone();
        input.timetable_block_group_ids = forged;
        assert!(services::request_observation(&pool, f.teacher_id, input)
            .await
            .is_err());
    }
    let mut wrong_date = valid.clone();
    wrong_date.observed_date = date.succ_opt().unwrap();
    assert!(
        services::request_observation(&pool, f.teacher_id, wrong_date)
            .await
            .is_err()
    );
    assert!(
        services::request_observation(&pool, f.evaluator_id, valid.clone())
            .await
            .is_err()
    );
    let mut wrong_term = valid.clone();
    wrong_term.academic_term_id = Uuid::new_v4();
    assert!(
        services::request_observation(&pool, f.teacher_id, wrong_term)
            .await
            .is_err()
    );
    let mut one = valid.clone();
    one.timetable_block_group_ids = vec![valid.timetable_block_group_ids[2]];
    let observation = services::request_observation(&pool, f.teacher_id, one)
        .await
        .unwrap();
    let before = serde_json::to_value(&observation).unwrap();
    assert!(services::update_observation(
        &pool,
        f.actor_id,
        observation.id,
        UpdateSupervisionObservationRequest {
            observed_date: Some(date),
            ..Default::default()
        }
    )
    .await
    .is_err());
    let mut gap = valid.clone();
    gap.timetable_block_group_ids.remove(1);
    assert!(services::update_requested_observation(
        &pool,
        f.teacher_id,
        observation.id,
        UpdateRequestedObservationRequest {
            observed_date: date,
            timetable_block_group_ids: gap.timetable_block_group_ids,
            expected_source_digest: gap.expected_source_digest,
        }
    )
    .await
    .is_err());
    assert_eq!(
        serde_json::to_value(
            services::get_observation(&pool, observation.id)
                .await
                .unwrap()
        )
        .unwrap(),
        before
    );
    sqlx::query("UPDATE bell_schedule_periods SET name=name||' changed' WHERE id=(SELECT bell_schedule_period_id FROM academic_timetable_blocks WHERE id=(SELECT block_id FROM academic_timetable_block_groups WHERE id=$1))")
        .bind(valid.timetable_block_group_ids[0]).execute(&pool).await.unwrap();
    assert!(matches!(
        services::update_requested_observation(
            &pool,
            f.teacher_id,
            observation.id,
            UpdateRequestedObservationRequest {
                observed_date: date,
                timetable_block_group_ids: valid.timetable_block_group_ids,
                expected_source_digest: valid.expected_source_digest,
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        serde_json::to_value(
            services::get_observation(&pool, observation.id)
                .await
                .unwrap()
        )
        .unwrap(),
        before
    );
}

#[tokio::test]
async fn supervision_same_lesson_across_cycles_is_allowed_but_later_period_evaluator_clashes_are_global(
) {
    let pool = migrated_pool_with_connections("supervision_later_period_global_conflict", 4).await;
    let a = insert_fixture(&pool).await;
    let b = insert_fixture(&pool).await;
    let date = a.observed_at.date_naive();
    let first = services::request_observation(
        &pool,
        a.teacher_id,
        multi_input(&pool, &a, a.teacher_id, date, 3).await,
    )
    .await
    .unwrap();
    let mut second_input = multi_input(&pool, &b, a.teacher_id, date, 3).await;
    second_input.timetable_block_group_ids = vec![second_input.timetable_block_group_ids[0]];
    let second = services::request_observation(&pool, a.teacher_id, second_input)
        .await
        .unwrap();
    approve_with(&pool, &a, first.id, vec![a.evaluator_id]).await;
    let availability = services::evaluator_availability(
        &pool,
        &school_authorization::ActorContext {
            user_id: b.actor_id,
            permissions: vec![crate::test_permission_codes::WILDCARD.into()],
        },
        second.id,
    )
    .await
    .unwrap();
    assert!(
        !availability
            .items
            .iter()
            .find(|e| e.id == a.evaluator_id)
            .unwrap()
            .available
    );
    assert!(services::approve_observation_request(
        &pool,
        b.actor_id,
        second.id,
        ApproveObservationRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: a.evaluator_id,
                role_label: None,
                is_required: Some(true)
            }],
        }
    )
    .await
    .is_err());
    assert_eq!(
        services::get_observation(&pool, second.id)
            .await
            .unwrap()
            .status,
        SupervisionObservationStatus::Requested
    );
    // Concurrent approvals on unrelated cycles still serialize the same evaluator/date.
    let c = insert_fixture(&pool).await;
    let d = insert_fixture(&pool).await;
    let next = date + Duration::weeks(1);
    let c_obs = services::request_observation(
        &pool,
        c.teacher_id,
        multi_input(&pool, &c, c.teacher_id, next, 2).await,
    )
    .await
    .unwrap();
    let d_obs = services::request_observation(
        &pool,
        d.teacher_id,
        multi_input(&pool, &d, d.teacher_id, next, 3).await,
    )
    .await
    .unwrap();
    separate_saved_class(&pool, &d_obs).await;
    let assignment = ApproveObservationRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: a.evaluator_id,
            role_label: None,
            is_required: Some(true),
        }],
    };
    let (c_result, d_result) = tokio::join!(
        services::approve_observation_request(&pool, c.actor_id, c_obs.id, assignment.clone()),
        services::approve_observation_request(&pool, d.actor_id, d_obs.id, assignment)
    );
    assert_eq!(
        usize::from(c_result.is_ok()) + usize::from(d_result.is_ok()),
        1
    );
    let next = date + Duration::weeks(2);
    let c2 = services::request_observation(
        &pool,
        c.teacher_id,
        multi_input(&pool, &c, c.teacher_id, next, 1).await,
    )
    .await
    .unwrap();
    let d2 = services::request_observation(
        &pool,
        d.teacher_id,
        multi_input(&pool, &d, d.teacher_id, next, 1).await,
    )
    .await
    .unwrap();
    let assignment = |user| ApproveObservationRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: user,
            role_label: None,
            is_required: Some(true),
        }],
    };
    let (left, right) = tokio::join!(
        services::approve_observation_request(&pool, c.actor_id, c2.id, assignment(c.evaluator_id)),
        services::approve_observation_request(&pool, d.actor_id, d2.id, assignment(d.evaluator_id))
    );
    assert_eq!(
        usize::from(left.is_ok()) + usize::from(right.is_ok()),
        1,
        "same exact class serializes even when all participants differ"
    );
}

#[tokio::test]
async fn supervision_period_migrations_preserve_raw_evidence_and_refuse_every_cleanup_drift() {
    let pool = migrated_pool_through("supervision_period_migration", 1, 163).await;
    // NULL group with a live placement is valid old evidence; its offering must
    // still be discoverable for Delivery detachment after cutover.
    sqlx::query("UPDATE supervision_observations SET learning_group_id=NULL,observed_at='2001-01-03 04:05:06.123456+00',
        manual_room_label='Retained room',lesson_snapshot=lesson_snapshot||$1")
        .bind(serde_json::json!({"unknownEvidence":{"nested":[1,{"thai":"เก็บครบ"}]}})).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supervision_observations SELECT (jsonb_populate_record(NULL::supervision_observations,to_jsonb(o)||
        jsonb_build_object('id',uuidv7(),'timetable_block_group_id',NULL,'learning_group_id',NULL,'homeroom_id',NULL,
            'observed_at','1999-12-31 18:30:59.987654+00','manual_subject_name','Historical subject',
            'manual_classroom_label','ม.3/2','manual_period_label','Historical period','manual_reason','Original reason',
            'lesson_snapshot',jsonb_build_object('source','manual','anotherUnknown',jsonb_build_array('retained',7))))).* FROM supervision_observations o LIMIT 1")
        .execute(&pool).await.unwrap();
    let originals: Vec<(Uuid, serde_json::Value)> =
        sqlx::query_as("SELECT id,to_jsonb(o) FROM supervision_observations o ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    apply_migrations_through(&pool, 164).await.unwrap();
    let canonical:Vec<(Uuid,serde_json::Value)>=sqlx::query_as("SELECT observation_id,to_jsonb(p) FROM supervision_observation_periods p ORDER BY observation_id").fetch_all(&pool).await.unwrap();
    assert_eq!(originals.len(), canonical.len());
    for ((id, old), (owner, new)) in originals.iter().zip(&canonical) {
        assert_eq!(id, owner);
        assert_eq!(new["id"], id.to_string());
        assert_eq!(new["ends_at"], serde_json::Value::Null);
        for key in [
            "observed_at",
            "manual_subject_name",
            "manual_classroom_label",
            "manual_room_label",
            "manual_period_label",
            "manual_reason",
            "lesson_snapshot",
            "created_at",
            "updated_at",
        ] {
            assert_eq!(old[key], new[key], "{key}");
        }
    }
    let (backed_id, backed) = canonical
        .iter()
        .find(|(_, p)| !p["timetable_block_group_id"].is_null())
        .unwrap();
    assert!(!backed["learning_offering_id"].is_null());
    // Rollback-only changes test all new row fields and both missing/stale receipts.
    for sql in [
        "UPDATE supervision_observation_periods SET id=uuidv7()",
        "UPDATE supervision_observation_periods SET position=position+10",
        "UPDATE supervision_observation_periods SET ends_at=observed_at+interval '1 hour'",
        "UPDATE supervision_observation_periods SET learning_offering_id=NULL WHERE learning_offering_id IS NOT NULL",
        "UPDATE supervision_observation_periods SET lesson_snapshot=lesson_snapshot||'{\"drift\":true}'::jsonb",
        "DELETE FROM supervision_period_cutover_receipt",
        "UPDATE supervision_period_cutover_receipt SET parents_hash=repeat('0',64)",
        "CREATE VIEW supervision165_external_view AS SELECT observed_at FROM supervision_observations",
    ] {
        let mut tx=pool.begin().await.unwrap();sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
        let result=sqlx::raw_sql(include_str!("../../../migrations/165_supervision_period_canonical_cleanup.sql")).execute(&mut *tx).await;
        assert!(result.is_err(),"cleanup must reject: {sql}");tx.rollback().await.unwrap();
        let parent_columns:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='supervision_observations' AND column_name='observed_at'").fetch_one(&pool).await.unwrap();
        assert_eq!(parent_columns,1);
    }
    apply_migrations_through(&pool, 165).await.unwrap();
    apply_migrations_through(&pool, 165).await.unwrap();
    let after:Vec<(Uuid,serde_json::Value)>=sqlx::query_as("SELECT observation_id,to_jsonb(p) FROM supervision_observation_periods p ORDER BY observation_id").fetch_all(&pool).await.unwrap();
    assert_eq!(after, canonical);
    for (id, _) in originals {
        let observation = services::get_observation(&pool, id).await.unwrap();
        assert_eq!(observation.periods.len(), 1);
        assert!(observation.periods[0].ends_at.is_none());
        services::update_observation(
            &pool,
            observation.observed_user_id,
            id,
            UpdateSupervisionObservationRequest::default(),
        )
        .await
        .unwrap();
    }
    let (year,term,offering):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT academic_year_id,academic_term_id,learning_offering_id FROM supervision_observation_periods WHERE learning_offering_id IS NOT NULL LIMIT 1").fetch_one(&pool).await.unwrap();
    let scope = school_academic_delivery::models::current::DeliveryDeletionScope {
        academic_year_id: year,
        academic_term_id: term,
        learning_offering_id: offering,
        learning_group_ids: vec![],
        delete_offering: true,
    };
    let mut tx = pool.begin().await.unwrap();
    let impact = services::current_source::deletion_impact_on_connection(&mut tx, &scope)
        .await
        .unwrap();
    assert_eq!(impact.observations, 1);
    assert_eq!(
        services::current_source::detach_deleted_source_in_transaction(&mut tx, &scope)
            .await
            .unwrap(),
        1
    );
    let raw: serde_json::Value = sqlx::query_scalar(
        "SELECT lesson_snapshot FROM supervision_observation_periods WHERE observation_id=$1",
    )
    .bind(backed_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(raw, backed["lesson_snapshot"]);
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn supervision_period_migration_refuses_unhydratable_evidence_before_switching() {
    let pool = migrated_pool_through("supervision_period_unmappable", 1, 163).await;
    for raw in [
        serde_json::json!(["not an object"]),
        serde_json::json!({"subjectName":7}),
        serde_json::json!({"observedAt":"2026-11-17"}),
        serde_json::json!({"timetableBlockGroupId":"not a UUID"}),
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE supervision_observations SET lesson_snapshot=$1")
            .bind(raw)
            .execute(&mut *tx)
            .await
            .unwrap();
        let result = sqlx::raw_sql(include_str!(
            "../../../migrations/164_supervision_observation_periods.sql"
        ))
        .execute(&mut *tx)
        .await;
        assert!(result.is_err());
        tx.rollback().await.unwrap();
        assert!(!sqlx::query_scalar::<_, bool>(
            "SELECT to_regclass('supervision_observation_periods') IS NOT NULL"
        )
        .fetch_one(&pool)
        .await
        .unwrap());
    }
    let mut ambiguous = pool.begin().await.unwrap();
    let offering = Uuid::now_v7();
    let group = Uuid::now_v7();
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(o)||jsonb_build_object(
        'id',$1::uuid,'code_snapshot','SUPERVISION_AMBIGUOUS_SOURCE'))).* FROM learning_offerings o
        JOIN academic_timetable_block_groups p ON p.learning_offering_id=o.id
        JOIN supervision_observations saved ON saved.timetable_block_group_id=p.id LIMIT 1")
        .bind(offering).execute(&mut *ambiguous).await.unwrap();
    sqlx::query("INSERT INTO learning_groups SELECT (jsonb_populate_record(NULL::learning_groups,to_jsonb(g)||jsonb_build_object(
        'id',$1::uuid,'learning_offering_id',$2::uuid,'code','SUPERVISION_AMBIGUOUS_GROUP'))).* FROM learning_groups g
        JOIN academic_timetable_block_groups p ON p.learning_group_id=g.id
        JOIN supervision_observations saved ON saved.timetable_block_group_id=p.id LIMIT 1")
        .bind(group).bind(offering).execute(&mut *ambiguous).await.unwrap();
    sqlx::query("UPDATE supervision_observations SET learning_group_id=$1 WHERE timetable_block_group_id IS NOT NULL")
        .bind(group).execute(&mut *ambiguous).await.unwrap();
    let error = sqlx::raw_sql(include_str!(
        "../../../migrations/164_supervision_observation_periods.sql"
    ))
    .execute(&mut *ambiguous)
    .await
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("SUPERVISION_PERIOD164_SOURCE_AMBIGUOUS"));
    ambiguous.rollback().await.unwrap();
    apply_migrations_through(&pool, 165).await.unwrap();
}

#[tokio::test]
async fn supervision_cycle_delete_hashes_and_removes_the_complete_owned_graph_only() {
    let pool = migrated_pool("supervision_cycle_delete_graph").await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    approve_with(&pool, &f, observation.id, vec![f.evaluator_id]).await;
    services::submit_my_evaluation(
        &pool,
        f.evaluator_id,
        observation.id,
        evaluation_responses(&f.template, 4.0),
    )
    .await
    .unwrap();
    for status in [
        "requested",
        "planned",
        "in_progress",
        "returned",
        "published",
        "acknowledged",
        "completed",
        "cancelled",
    ] {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO supervision_observations SELECT (jsonb_populate_record(NULL::supervision_observations,to_jsonb(o)||jsonb_build_object('id',$2::uuid,'status',$3::text))).* FROM supervision_observations o WHERE id=$1")
            .bind(observation.id).bind(id).bind(status).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO supervision_observation_periods SELECT (jsonb_populate_record(NULL::supervision_observation_periods,to_jsonb(p)||jsonb_build_object('id',uuidv7(),'observation_id',$2::uuid))).* FROM supervision_observation_periods p WHERE observation_id=$1")
            .bind(observation.id).bind(id).execute(&pool).await.unwrap();
    }
    let shared: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_build_array(
        (SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM supervision_templates t),
        (SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM supervision_template_sections s),
        (SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM supervision_template_items i),
        (SELECT jsonb_agg(to_jsonb(u) ORDER BY id) FROM users u),
        (SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_timetable_blocks t),
        (SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g))",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let owned_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM supervision_observations WHERE cycle_id=$1 ORDER BY id")
            .bind(f.cycle.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let preview = services::cycle_deletion_impact(&pool, f.cycle.id)
        .await
        .unwrap();
    assert_eq!(preview.counts.observations, 9);
    assert_eq!(preview.counts.periods, 9);
    assert_eq!(preview.counts.evaluators, 1);
    assert!(preview.counts.responses > 0);
    assert!(preview.counts.actions > 0);
    // A child-only response change must stale a preview even if cycle.updated_at is unchanged.
    sqlx::query("UPDATE supervision_evaluator_responses SET text_response='Changed saved evidence' WHERE observation_id=$1")
        .bind(observation.id).execute(&pool).await.unwrap();
    assert!(matches!(
        services::delete_cycle(
            &pool,
            f.cycle.id,
            DeleteSupervisionCycleRequest {
                expected_revision: preview.revision
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let preview = services::cycle_deletion_impact(&pool, f.cycle.id)
        .await
        .unwrap();
    sqlx::query("INSERT INTO supervision_actions(observation_id,action_kind,comment) VALUES($1,'retained_note','Stale preview action')")
        .bind(observation.id).execute(&pool).await.unwrap();
    assert!(matches!(
        services::delete_cycle(
            &pool,
            f.cycle.id,
            DeleteSupervisionCycleRequest {
                expected_revision: preview.revision
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let preview = services::cycle_deletion_impact(&pool, f.cycle.id)
        .await
        .unwrap();
    let deleted = services::delete_cycle(
        &pool,
        f.cycle.id,
        DeleteSupervisionCycleRequest {
            expected_revision: preview.revision.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(deleted.counts, preview.counts);
    assert_eq!(deleted.revision, preview.revision);
    assert!(matches!(
        services::cycle_deletion_impact(&pool, f.cycle.id).await,
        Err(AppError::NotFound(_))
    ));
    let remaining: i64 = sqlx::query_scalar(
        "SELECT
        (SELECT count(*) FROM supervision_cycle_targets WHERE cycle_id=$1)+
        (SELECT count(*) FROM supervision_observations WHERE cycle_id=$1)+
        (SELECT count(*) FROM supervision_observation_periods WHERE observation_id=ANY($2))+
        (SELECT count(*) FROM supervision_evaluators WHERE observation_id=ANY($2))+
        (SELECT count(*) FROM supervision_evaluator_responses WHERE observation_id=ANY($2))+
        (SELECT count(*) FROM supervision_actions WHERE observation_id=ANY($2))",
    )
    .bind(f.cycle.id)
    .bind(&owned_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    let shared_after: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_build_array(
        (SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM supervision_templates t),
        (SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM supervision_template_sections s),
        (SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM supervision_template_items i),
        (SELECT jsonb_agg(to_jsonb(u) ORDER BY id) FROM users u),
        (SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_timetable_blocks t),
        (SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g))",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(shared_after, shared);
}

#[tokio::test]
async fn supervision_cycle_delete_refuses_malformed_cross_cycle_edges_and_external_same_named_cascades(
) {
    let pool = migrated_pool("supervision_cycle_delete_graph_refusals").await;
    let a = insert_fixture(&pool).await;
    let b = insert_fixture(&pool).await;
    let oa = request_observation(&pool, &a).await;
    let ob = request_observation(&pool, &b).await;
    let planned = approve_with(&pool, &a, oa.id, vec![a.evaluator_id]).await;
    let preview = services::cycle_deletion_impact(&pool, a.cycle.id)
        .await
        .unwrap();
    sqlx::query("INSERT INTO supervision_evaluator_responses(observation_id,evaluator_id,template_item_id,rating_score)
        VALUES($1,$2,$3,4)").bind(ob.id).bind(planned.evaluators[0].id).bind(a.template.sections[0].items[0].id).execute(&pool).await.unwrap();
    assert!(matches!(
        services::cycle_deletion_impact(&pool, a.cycle.id).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        services::delete_cycle(
            &pool,
            a.cycle.id,
            DeleteSupervisionCycleRequest {
                expected_revision: preview.revision
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(services::get_observation(&pool, ob.id).await.is_ok());
    sqlx::query("DELETE FROM supervision_evaluator_responses WHERE observation_id=$1")
        .bind(ob.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE supervision_external_owned_fact(id uuid PRIMARY KEY,cycle_id uuid,
        CONSTRAINT supervision_cycle_targets_cycle_id_fkey FOREIGN KEY(cycle_id) REFERENCES supervision_cycles(id) ON DELETE CASCADE)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supervision_external_owned_fact VALUES(uuidv7(),$1)")
        .bind(a.cycle.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        services::cycle_deletion_impact(&pool, a.cycle.id).await,
        Err(AppError::Conflict(_))
    ));
    assert!(services::get_cycle(&pool, a.cycle.id).await.is_ok());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM supervision_external_owned_fact")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn supervision_annual_cycle_delete_guards_every_observation_term_and_serializes_child_writes()
{
    let pool = migrated_pool_with_connections("supervision_cycle_delete_lifecycle", 4).await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    let extra_term = Uuid::now_v7();
    let extra_observation = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_terms SELECT (jsonb_populate_record(NULL::academic_terms,to_jsonb(t)||jsonb_build_object(
        'id',$2::uuid,'code','EXTRA_SUPERVISION','sequence_no',(SELECT max(sequence_no)+1 FROM academic_terms WHERE academic_year_id=t.academic_year_id),
        'name','Another annual-cycle child term','status','planning'))).* FROM academic_terms t WHERE id=$1")
        .bind(f.academic_term_id).bind(extra_term).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supervision_observations SELECT (jsonb_populate_record(NULL::supervision_observations,to_jsonb(o)||jsonb_build_object('id',$2::uuid,'academic_term_id',$3::uuid))).* FROM supervision_observations o WHERE id=$1")
        .bind(observation.id).bind(extra_observation).bind(extra_term).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supervision_observation_periods SELECT (jsonb_populate_record(NULL::supervision_observation_periods,to_jsonb(p)||jsonb_build_object(
        'id',uuidv7(),'observation_id',$2::uuid,'academic_term_id',$3::uuid,'timetable_block_group_id',NULL,'learning_offering_id',NULL,'learning_group_id',NULL,'homeroom_id',NULL))).*
        FROM supervision_observation_periods p WHERE observation_id=$1")
        .bind(observation.id).bind(extra_observation).bind(extra_term).execute(&pool).await.unwrap();
    sqlx::query("UPDATE supervision_cycles SET academic_term_id=$2 WHERE id=$1")
        .bind(f.cycle.id)
        .bind(f.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        matches!(
            services::cycle_deletion_impact(&pool, f.cycle.id).await,
            Err(AppError::Conflict(_))
        ),
        "a term-bound cycle cannot own an observation from another term"
    );
    sqlx::query("UPDATE supervision_cycles SET academic_term_id=NULL WHERE id=$1")
        .bind(f.cycle.id)
        .execute(&pool)
        .await
        .unwrap();
    let preview = services::cycle_deletion_impact(&pool, f.cycle.id)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=$1")
        .bind(extra_term)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        services::delete_cycle(
            &pool,
            f.cycle.id,
            DeleteSupervisionCycleRequest {
                expected_revision: preview.revision.clone()
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(extra_term)
        .execute(&pool)
        .await
        .unwrap();
    let mut writer = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM supervision_observations WHERE id=$1 FOR UPDATE")
        .bind(observation.id)
        .execute(&mut *writer)
        .await
        .unwrap();
    let delete_pool = pool.clone();
    let cycle = f.cycle.id;
    let mut deleting = tokio::spawn(async move {
        services::delete_cycle(
            &delete_pool,
            cycle,
            DeleteSupervisionCycleRequest {
                expected_revision: preview.revision,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut deleting)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO supervision_actions(observation_id,action_kind,comment) VALUES($1,'accepted_concurrent_note','Must stale the preview')")
        .bind(observation.id).execute(&mut *writer).await.unwrap();
    writer.commit().await.unwrap();
    assert!(matches!(
        deleting.await.unwrap(),
        Err(AppError::Conflict(_))
    ));
    assert!(services::get_cycle(&pool, f.cycle.id).await.is_ok());
}

#[tokio::test]
async fn supervision_source_detach_updates_each_matching_period_and_pending_revision() {
    let pool = migrated_pool("supervision_partial_period_source_detach").await;
    let f = insert_fixture(&pool).await;
    let observation = services::request_observation(
        &pool,
        f.teacher_id,
        multi_input(&pool, &f, f.teacher_id, f.observed_at.date_naive(), 3).await,
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let before = services::pending_term_work(&mut tx, f.cycle.academic_year_id, f.academic_term_id)
        .await
        .unwrap();
    let first = observation.periods[0].timetable_block_group_id.unwrap();
    let last = observation.periods[2].timetable_block_group_id.unwrap();
    let count =
        services::current_source::detach_timetable_sources_in_transaction(&mut tx, &[last, first])
            .await
            .unwrap();
    assert_eq!(count, 1, "two periods still belong to one observation");
    let after = services::pending_term_work(&mut tx, f.cycle.academic_year_id, f.academic_term_id)
        .await
        .unwrap();
    assert_ne!(
        before
            .iter()
            .find(|r| r.id == observation.id)
            .unwrap()
            .revision,
        after
            .iter()
            .find(|r| r.id == observation.id)
            .unwrap()
            .revision
    );
    tx.commit().await.unwrap();
    let saved = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    assert!(saved.periods[0].timetable_block_group_id.is_none());
    assert!(saved.periods[2].timetable_block_group_id.is_none());
    assert_eq!(
        saved.periods[1].timetable_block_group_id,
        observation.periods[1].timetable_block_group_id
    );
    for (old, new) in observation.periods.iter().zip(&saved.periods) {
        assert_eq!(old.lesson_snapshot, new.lesson_snapshot);
        assert_eq!(old.observed_at, new.observed_at);
        assert_eq!(old.ends_at, new.ends_at);
        assert_eq!(old.learning_group_id, new.learning_group_id);
    }
}

#[tokio::test]
async fn supervision_legacy_unknown_ends_conflict_as_points_without_invented_durations() {
    let pool = migrated_pool("supervision_legacy_point_conflict").await;
    let f = insert_fixture(&pool).await;
    let requested = services::request_observation(
        &pool,
        f.teacher_id,
        multi_input(&pool, &f, f.teacher_id, f.observed_at.date_naive(), 3).await,
    )
    .await
    .unwrap();
    let legacy: Uuid = sqlx::query_scalar(
        "SELECT id FROM supervision_observations WHERE cycle_id<>$1 ORDER BY id LIMIT 1",
    )
    .bind(f.cycle.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let point = requested.periods[1].observed_at + Duration::minutes(30);
    sqlx::query("UPDATE supervision_observation_periods SET observed_at=$2,ends_at=NULL WHERE observation_id=$1")
        .bind(legacy).bind(point).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO supervision_evaluators(observation_id,evaluator_user_id) VALUES($1,$2)",
    )
    .bind(legacy)
    .bind(f.evaluator_id)
    .execute(&pool)
    .await
    .unwrap();
    let assignment = ApproveObservationRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: f.evaluator_id,
            role_label: None,
            is_required: Some(true),
        }],
    };
    assert!(services::approve_observation_request(
        &pool,
        f.actor_id,
        requested.id,
        assignment.clone()
    )
    .await
    .is_err());
    sqlx::query(
        "UPDATE supervision_observation_periods SET observed_at=$2 WHERE observation_id=$1",
    )
    .bind(legacy)
    .bind(requested.periods[2].ends_at.unwrap())
    .execute(&pool)
    .await
    .unwrap();
    services::approve_observation_request(&pool, f.actor_id, requested.id, assignment)
        .await
        .unwrap();
    assert!(services::get_observation(&pool, legacy)
        .await
        .unwrap()
        .periods[0]
        .ends_at
        .is_none());
}

#[tokio::test]
async fn supervision_http_booking_and_deletion_enforce_permissions_resource_scope_and_tenant_boundary(
) {
    use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
    use crate::integration_tests::permission_coverage::{
        coverage_actor, coverage_request, coverage_router,
    };
    use crate::test_permission_codes as codes;
    use axum::http::{Method, StatusCode};
    use serde_json::{json, Value};
    let pool = migrated_pool_with_connections("supervision_period_http_scope", 4).await;
    school_test_db::run_test_migrations(&pool).await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    let auth =
        ProtectedFixture::with_prepared_tenant_pool("supervision_period_http_scope", pool.clone())
            .await;
    let _lease = auth
        .runtime
        .pool_manager
        .get_pool("test-pool://session-router-tenant-a", "tenant-a")
        .await
        .unwrap();
    let reader = coverage_actor(&auth, &[codes::SUPERVISION_READ_SCHOOL], 201).await;
    let evaluator = coverage_actor(&auth, &[codes::SUPERVISION_EVALUATE_ASSIGNED], 202).await;
    let requester = coverage_actor(&auth, &[codes::SUPERVISION_REQUEST_OWN], 203).await;
    let manager = coverage_actor(&auth, &[codes::SUPERVISION_MANAGE_SCHOOL], 204).await;
    let app = coverage_router(&auth);
    let week = f.observed_at.date_naive()
        - Duration::days(i64::from(f.observed_at.weekday().num_days_from_monday()));
    let options = format!(
        "/api/supervision/booking-options?cycleId={}&academicYearId={}&academicTermId={}&weekStartDate={week}",
        f.cycle.id, f.cycle.academic_year_id, f.academic_term_id
    );
    for actor in [&reader, &evaluator] {
        let (status, _) =
            coverage_request(&app, &auth, actor, Method::GET, &options, Value::Null).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = coverage_request(
            &app,
            &auth,
            actor,
            Method::GET,
            &format!("/api/supervision/cycles/{}/deletion-impact", f.cycle.id),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = coverage_request(
            &app,
            &auth,
            actor,
            Method::DELETE,
            &format!("/api/supervision/cycles/{}", f.cycle.id),
            json!({"expectedRevision":"0".repeat(64)}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, own) =
        coverage_request(&app, &auth, &requester, Method::GET, &options, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{own}");
    assert!(
        own["data"]["items"].as_array().unwrap().is_empty(),
        "requester cannot enumerate another teacher's schedule"
    );
    use tower::ServiceExt;
    let (session, token) = auth.insert_session(&pool, requester.id, 205, false).await;
    let forged = axum::http::Request::builder()
        .method(Method::GET)
        .uri(format!("{options}&teacherId={}", f.teacher_id))
        .header(
            axum::http::header::ORIGIN,
            "https://tenant-a.schoolorbit.test",
        )
        .header(
            axum::http::header::COOKIE,
            format!(
                "{}={}",
                school_auth::config::SESSION_COOKIE_NAME,
                token.encode().expose_for_cookie()
            ),
        )
        .header("x-csrf-token", auth.csrf(session))
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(forged).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    let observation_options = format!(
        "/api/supervision/observations/{}/timetable-options?weekStartDate={week}",
        observation.id
    );
    let (status, _) = coverage_request(
        &app,
        &auth,
        &evaluator,
        Method::GET,
        &observation_options,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, items) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::GET,
        &observation_options,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{items}");
    assert!(!items["data"]["items"].as_array().unwrap().is_empty());
    let path = format!("/api/supervision/cycles/{}/deletion-impact", f.cycle.id);
    let (status, preview) =
        coverage_request(&app, &auth, &manager, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let other = migrated_pool("supervision_other_tenant").await;
    assert!(matches!(
        services::cycle_deletion_impact(&other, f.cycle.id).await,
        Err(AppError::NotFound(_))
    ));
    let (status, result) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::DELETE,
        &format!("/api/supervision/cycles/{}", f.cycle.id),
        json!({"expectedRevision":preview["data"]["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["data"]["counts"], preview["data"]["counts"]);
    assert!(services::get_template(&pool, f.template.id).await.is_ok());
}

#[tokio::test]
async fn supervision_global_evaluator_locks_serialize_approvals_across_academic_years() {
    let pool = migrated_pool_with_connections("supervision_evaluator_cross_year", 4).await;
    let f = insert_fixture(&pool).await;
    let original = request_observation(&pool, &f).await;
    let year = Uuid::now_v7();
    let term = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_years SELECT (jsonb_populate_record(NULL::academic_years,to_jsonb(y)||jsonb_build_object(
        'id',$2::uuid,'year',(SELECT max(year)+1 FROM academic_years),'name','Independent supervision year','status','planning'))).* FROM academic_years y WHERE id=$1")
        .bind(f.cycle.academic_year_id).bind(year).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_terms SELECT (jsonb_populate_record(NULL::academic_terms,to_jsonb(t)||jsonb_build_object(
        'id',$2::uuid,'academic_year_id',$3::uuid,'status','planning'))).* FROM academic_terms t WHERE id=$1")
        .bind(f.academic_term_id).bind(term).bind(year).execute(&pool).await.unwrap();
    let cycle = services::create_cycle(
        &pool,
        CreateSupervisionCycleRequest {
            observation_start_date: chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
            observation_end_date: chrono::NaiveDate::from_ymd_opt(2200, 12, 31).unwrap(),
            academic_year_id: year,
            academic_term_id: Some(term),
            title: "Independent year supervision".into(),
            description: None,
            template_id: f.template.id,
            status: Some(SupervisionCycleStatus::Open),
            targets: vec![CreateSupervisionCycleTargetRequest {
                target_type: SupervisionTargetType::School,
                target_id: None,
                required_observations: 1,
                priority: 100,
            }],
        },
        f.actor_id,
    )
    .await
    .unwrap();
    let second = Uuid::now_v7();
    let other_teacher = test_user(&pool, "cross-year-teacher").await;
    // Saved detached evidence in a different explicit context remains evaluable.
    sqlx::query("INSERT INTO supervision_observations SELECT (jsonb_populate_record(NULL::supervision_observations,to_jsonb(o)||jsonb_build_object(
        'id',$2::uuid,'cycle_id',$3::uuid,'academic_year_id',$4::uuid,'academic_term_id',$5::uuid,'observed_user_id',$6::uuid,'requested_by',$6::uuid))).*
        FROM supervision_observations o WHERE id=$1")
        .bind(original.id).bind(second).bind(cycle.id).bind(year).bind(term).bind(other_teacher).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supervision_observation_periods SELECT (jsonb_populate_record(NULL::supervision_observation_periods,to_jsonb(p)||jsonb_build_object(
        'id',uuidv7(),'observation_id',$2::uuid,'academic_year_id',$3::uuid,'academic_term_id',$4::uuid,
        'timetable_block_group_id',NULL,'learning_offering_id',NULL,'learning_group_id',NULL,'homeroom_id',NULL))).*
        FROM supervision_observation_periods p WHERE observation_id=$1")
        .bind(original.id).bind(second).bind(year).bind(term).execute(&pool).await.unwrap();
    let assignment = ApproveObservationRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: f.evaluator_id,
            role_label: None,
            is_required: Some(true),
        }],
    };
    let mut barrier = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "supervision-evaluator:{}:{}",
            f.evaluator_id,
            original
                .observed_at
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive()
        ))
        .execute(&mut *barrier)
        .await
        .unwrap();
    let a_pool = pool.clone();
    let b_pool = pool.clone();
    let actor = f.actor_id;
    let first_id = original.id;
    let second_assignment = assignment.clone();
    let mut a = tokio::spawn(async move {
        services::approve_observation_request(&a_pool, actor, first_id, assignment).await
    });
    let mut b = tokio::spawn(async move {
        services::approve_observation_request(&b_pool, actor, second, second_assignment).await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut a)
            .await
            .is_err(),
        "first context must acquire the global evaluator/date lock"
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut b)
            .await
            .is_err(),
        "second year must acquire the same global evaluator/date lock"
    );
    barrier.commit().await.unwrap();
    let (a, b) = (a.await.unwrap(), b.await.unwrap());
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM supervision_evaluators WHERE evaluator_user_id=$1 AND observation_id=ANY($2)")
        .bind(f.evaluator_id).bind(vec![original.id,second]).fetch_one(&pool).await.unwrap(),1);
}

#[tokio::test]
async fn supervision_booking_requires_exact_active_instructor_current_assignment_and_one_homeroom()
{
    let pool = migrated_pool("supervision_exact_source_eligibility").await;
    let f = insert_fixture(&pool).await;
    let valid = observation_input(&pool, &f).await;
    let placement = valid.timetable_block_group_ids[0];
    let instructor:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(i) FROM academic_timetable_block_group_instructors i WHERE block_group_id=$1 AND instructor_id=$2")
        .bind(placement).bind(f.teacher_id).fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM academic_timetable_block_group_instructors WHERE block_group_id=$1 AND instructor_id=$2").bind(placement).bind(f.teacher_id).execute(&pool).await.unwrap();
    let mut forged = valid.clone();
    forged.expected_source_digest = options_for(&pool, &f, f.teacher_id, valid.observed_date)
        .await
        .source_digest;
    assert!(
        services::request_observation(&pool, f.teacher_id, forged)
            .await
            .is_err(),
        "Delivery membership cannot replace exact timetable instructor membership"
    );
    sqlx::query("INSERT INTO academic_timetable_block_group_instructors SELECT (jsonb_populate_record(NULL::academic_timetable_block_group_instructors,$1)).*")
        .bind(instructor).execute(&pool).await.unwrap();
    let assignment:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_teachers t JOIN academic_timetable_block_groups p ON p.learning_group_id=t.learning_group_id WHERE p.id=$1 AND t.teacher_id=$2")
        .bind(placement).bind(f.teacher_id).fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM learning_group_teachers WHERE teacher_id=$1")
        .bind(f.teacher_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut forged = valid.clone();
    forged.expected_source_digest = options_for(&pool, &f, f.teacher_id, valid.observed_date)
        .await
        .source_digest;
    assert!(
        services::request_observation(&pool, f.teacher_id, forged)
            .await
            .is_err(),
        "Old timetable instructor membership cannot replace current Delivery authorization"
    );
    sqlx::query("INSERT INTO learning_group_teachers SELECT (jsonb_populate_record(NULL::learning_group_teachers,$1)).*").bind(assignment).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_timetable_block_groups SET is_active=false WHERE id=$1")
        .bind(placement)
        .execute(&pool)
        .await
        .unwrap();
    let mut forged = valid.clone();
    forged.expected_source_digest = options_for(&pool, &f, f.teacher_id, valid.observed_date)
        .await
        .source_digest;
    assert!(services::request_observation(&pool, f.teacher_id, forged)
        .await
        .is_err());
    sqlx::query(
        "UPDATE academic_timetable_block_groups SET is_active=true,homeroom_ids='{}' WHERE id=$1",
    )
    .bind(placement)
    .execute(&pool)
    .await
    .unwrap();
    let mut forged = valid.clone();
    forged.expected_source_digest = options_for(&pool, &f, f.teacher_id, valid.observed_date)
        .await
        .source_digest;
    assert!(
        services::request_observation(&pool, f.teacher_id, forged)
            .await
            .is_err(),
        "A missing exact class must never be inferred from labels or grade"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM supervision_observations WHERE cycle_id=$1"
        )
        .bind(f.cycle.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn supervision_cycle_delete_accepts_every_cycle_status_in_writable_contexts() {
    let pool = migrated_pool("supervision_cycle_delete_statuses").await;
    let f = insert_fixture(&pool).await;
    for status in [
        SupervisionCycleStatus::Draft,
        SupervisionCycleStatus::Open,
        SupervisionCycleStatus::Closed,
        SupervisionCycleStatus::Archived,
    ] {
        let cycle = services::create_cycle(
            &pool,
            CreateSupervisionCycleRequest {
                observation_start_date: chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
                observation_end_date: chrono::NaiveDate::from_ymd_opt(2200, 12, 31).unwrap(),
                academic_year_id: f.cycle.academic_year_id,
                academic_term_id: Some(f.academic_term_id),
                title: format!("Delete {} cycle", status.as_str()),
                description: None,
                template_id: f.template.id,
                status: Some(status),
                targets: vec![CreateSupervisionCycleTargetRequest {
                    target_type: SupervisionTargetType::School,
                    target_id: None,
                    required_observations: 1,
                    priority: 100,
                }],
            },
            f.actor_id,
        )
        .await
        .unwrap();
        let preview = services::cycle_deletion_impact(&pool, cycle.id)
            .await
            .unwrap();
        assert_eq!(preview.counts.targets, 1);
        let deleted = services::delete_cycle(
            &pool,
            cycle.id,
            DeleteSupervisionCycleRequest {
                expected_revision: preview.revision,
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted.counts.targets, 1);
        assert!(services::get_template(&pool, f.template.id).await.is_ok());
    }
}

#[tokio::test]
async fn supervision_replacement_validates_omitted_retained_submitted_evaluators() {
    let pool = migrated_pool("supervision_retained_evaluator_conflict").await;
    let a = insert_fixture(&pool).await;
    let b = insert_fixture(&pool).await;
    let first = request_observation(&pool, &a).await;
    let second = request_observation(&pool, &b).await;
    approve_with(
        &pool,
        &a,
        first.id,
        vec![a.evaluator_id, a.second_evaluator_id],
    )
    .await;
    services::submit_my_evaluation(
        &pool,
        a.evaluator_id,
        first.id,
        evaluation_responses(&a.template, 4.0),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(a.evaluator_id)
        .execute(&pool)
        .await
        .unwrap();
    let before_replacement = supervision_graph_snapshot(&pool).await;
    let active_replacement = ReplaceObservationEvaluatorsRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: a.second_evaluator_id,
            role_label: None,
            is_required: Some(true),
        }],
    };
    let updated = services::replace_observation_evaluators(
        &pool,
        a.actor_id,
        first.id,
        active_replacement.clone(),
    )
    .await
    .expect("submitted evidence stays assigned when an evaluator later becomes inactive");
    let retained = updated
        .evaluators
        .iter()
        .find(|row| row.evaluator_user_id == a.evaluator_id)
        .unwrap();
    assert_eq!(retained.status, SupervisionEvaluatorStatus::Submitted);
    let after_replacement = supervision_graph_snapshot(&pool).await;
    assert_eq!(
        after_replacement["responses"],
        before_replacement["responses"]
    );
    assert_eq!(
        after_replacement["evaluators"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == retained.id.to_string()),
        before_replacement["evaluators"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == retained.id.to_string())
    );
    // Legacy saved assignments may already conflict; omission cannot bypass a
    // conflict check while retaining submitted evidence in the replacement.
    sqlx::query(
        "INSERT INTO supervision_evaluators(observation_id,evaluator_user_id) VALUES($1,$2)",
    )
    .bind(second.id)
    .bind(a.evaluator_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE supervision_observations SET status='planned' WHERE id=$1")
        .bind(second.id)
        .execute(&pool)
        .await
        .unwrap();
    let before =
        serde_json::to_value(services::get_observation(&pool, first.id).await.unwrap()).unwrap();
    let result =
        services::replace_observation_evaluators(&pool, a.actor_id, first.id, active_replacement)
            .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(
        serde_json::to_value(services::get_observation(&pool, first.id).await.unwrap()).unwrap(),
        before
    );
}

#[tokio::test]
async fn supervision_empty_legacy_snapshots_retain_known_allocation_and_time_when_sources_detach() {
    let pool = migrated_pool("supervision_empty_legacy_detachment").await;
    let f = insert_fixture(&pool).await;
    let first = request_observation(&pool, &f).await;
    let second = services::request_observation(
        &pool,
        f.teacher_id,
        input_at(&pool, &f, f.teacher_id, f.observed_at + Duration::weeks(1)).await,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE supervision_observation_periods SET lesson_snapshot='{}' WHERE observation_id=ANY($1)")
        .bind(vec![first.id,second.id]).execute(&pool).await.unwrap();
    let allocation = first.periods[0].timetable_block_group_id.unwrap();
    let (year,term,offering,group):(Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT academic_year_id,academic_term_id,learning_offering_id,learning_group_id FROM supervision_observation_periods WHERE observation_id=$1")
        .bind(first.id).fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        services::current_source::detach_timetable_sources_in_transaction(&mut tx, &[allocation])
            .await
            .unwrap(),
        2
    );
    for observation in [&first, &second] {
        let raw: serde_json::Value = sqlx::query_scalar(
            "SELECT lesson_snapshot FROM supervision_observation_periods WHERE observation_id=$1",
        )
        .bind(observation.id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(raw["source"], "removed_timetable");
        assert_eq!(raw["timetableBlockGroupId"], allocation.to_string());
        assert_eq!(
            serde_json::from_value::<DateTime<Utc>>(raw["observedAt"].clone()).unwrap(),
            observation.periods[0].observed_at
        );
    }
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let scope = school_academic_delivery::models::current::DeliveryDeletionScope {
        delete_offering: false,
        learning_offering_id: offering,
        academic_year_id: year,
        academic_term_id: term,
        learning_group_ids: vec![group],
    };
    services::current_source::detach_deleted_source_in_transaction(&mut tx, &scope)
        .await
        .unwrap();
    for observation in [&first, &second] {
        let (raw,source,live_group):(serde_json::Value,Option<Uuid>,Option<Uuid>)=sqlx::query_as("SELECT lesson_snapshot,timetable_block_group_id,learning_group_id FROM supervision_observation_periods WHERE observation_id=$1")
            .bind(observation.id).fetch_one(&mut *tx).await.unwrap();
        assert_eq!(raw["timetableBlockGroupId"], allocation.to_string());
        assert_eq!(source, None);
        assert_eq!(live_group, None);
        assert_eq!(
            serde_json::from_value::<DateTime<Utc>>(raw["observedAt"].clone()).unwrap(),
            observation.periods[0].observed_at
        );
    }
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE supervision_observation_periods SET timetable_block_group_id=NULL WHERE observation_id=$1")
        .bind(second.id).execute(&pool).await.unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY observation_id,position,id) FROM supervision_observation_periods p WHERE observation_id=ANY($1)")
        .bind(vec![first.id,second.id]).fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(services::current_source::detach_deleted_source_in_transaction(&mut tx,&scope).await,Err(AppError::Conflict(_))),
        "an empty group-only record cannot lose its last current pointer without saved lesson evidence");
    let unchanged:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY observation_id,position,id) FROM supervision_observation_periods p WHERE observation_id=ANY($1)")
        .bind(vec![first.id,second.id]).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(
        unchanged, before,
        "one unmappable period must refuse all matching updates atomically"
    );
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE supervision_observation_periods SET manual_subject_name='Historical manual subject',manual_classroom_label='ม.3/2',
        manual_period_label='Historical period',manual_reason='Original manual reason' WHERE observation_id=$1")
        .bind(second.id).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    services::current_source::detach_deleted_source_in_transaction(&mut tx, &scope)
        .await
        .unwrap();
    let manual:(String,String,String,String,Option<Uuid>)=sqlx::query_as("SELECT manual_subject_name,manual_classroom_label,manual_period_label,manual_reason,learning_group_id
        FROM supervision_observation_periods WHERE observation_id=$1")
        .bind(second.id).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(
        manual,
        (
            "Historical manual subject".into(),
            "ม.3/2".into(),
            "Historical period".into(),
            "Original manual reason".into(),
            None
        )
    );
    tx.rollback().await.unwrap();
}

fn management_actor(user_id: Uuid) -> school_authorization::ActorContext {
    school_authorization::ActorContext {
        user_id,
        permissions: vec![crate::test_permission_codes::SUPERVISION_MANAGE_SCHOOL.into()],
    }
}

async fn set_cycle_dates(
    pool: &PgPool,
    f: &SupervisionFixture,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> Result<SupervisionCycle, AppError> {
    services::update_cycle(
        pool,
        f.cycle.id,
        UpdateSupervisionCycleRequest {
            observation_start_date: Some(start),
            observation_end_date: Some(end),
            ..Default::default()
        },
    )
    .await
}

#[tokio::test]
async fn supervision_booking_targets_require_current_active_organization_affiliation() {
    let pool = migrated_pool("supervision_current_booking_target").await;
    let f = insert_fixture(&pool).await;
    let group = school_academic_core::services::catalog::create_subject_group(
        &pool,
        school_academic_core::models::CreateSubjectGroupRequest {
            name_th: "Booking target group".into(),
            name_en: "Booking target group".into(),
            display_order: None,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES ($1,$2,'member')")
        .bind(f.teacher_id).bind(group.organization_unit_id).execute(&pool).await.unwrap();
    let date = f.observed_at.date_naive();
    let query = SupervisionBookingOptionsQuery {
        cycle_id: f.cycle.id,
        academic_year_id: f.cycle.academic_year_id,
        academic_term_id: f.academic_term_id,
        week_start_date: date - Duration::days(i64::from(date.weekday().num_days_from_monday())),
    };
    for (target_type, target_id) in [
        (
            SupervisionTargetType::OrganizationUnit,
            group.organization_unit_id,
        ),
        (SupervisionTargetType::SubjectGroup, group.id),
    ] {
        services::update_cycle(
            &pool,
            f.cycle.id,
            UpdateSupervisionCycleRequest {
                targets: Some(vec![CreateSupervisionCycleTargetRequest {
                    target_type,
                    target_id: Some(target_id),
                    required_observations: 1,
                    priority: 10,
                }]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let selection = multi_input(&pool, &f, f.teacher_id, date, 1).await;
        assert_eq!(selection.timetable_block_group_ids.len(), 1);
        for (start, end, active) in [(1, None, true), (-1, Some(0), true), (0, None, false)] {
            sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE+$3,ended_at=CURRENT_DATE+$4 WHERE user_id=$1 AND organization_unit_id=$2")
                .bind(f.teacher_id).bind(group.organization_unit_id).bind(start).bind(end).execute(&pool).await.unwrap();
            sqlx::query("UPDATE organization_units SET is_active=$2 WHERE id=$1")
                .bind(group.organization_unit_id)
                .bind(active)
                .execute(&pool)
                .await
                .unwrap();
            assert!(matches!(
                services::booking_options(&pool, f.teacher_id, query.clone()).await,
                Err(AppError::Forbidden(_))
            ));
            assert!(matches!(
                services::request_observation(&pool, f.teacher_id, selection.clone()).await,
                Err(AppError::Forbidden(_))
            ));
        }
        sqlx::query("UPDATE organization_units SET is_active=true WHERE id=$1")
            .bind(group.organization_unit_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn supervision_date_migration_preserves_populated_165_evidence_and_replays() {
    let pool = migrated_pool_through("supervision_date_migration166", 1, 165).await;
    sqlx::raw_sql(r#"
        INSERT INTO supervision_cycle_targets(cycle_id,target_type,required_observations,priority)
        SELECT id,'school',2,10 FROM supervision_cycles;
        INSERT INTO supervision_template_sections(id,template_id,title,sort_order)
        SELECT 'a0000000-0000-0000-0000-000000000166',template_id,'Retained section',1
        FROM supervision_observations LIMIT 1;
        INSERT INTO supervision_template_items(id,section_id,label,item_type,required,sort_order)
        VALUES('a1000000-0000-0000-0000-000000000166','a0000000-0000-0000-0000-000000000166','Retained item','rating',true,1);
        INSERT INTO supervision_evaluators(id,observation_id,evaluator_user_id,status,submitted_at)
        SELECT 'a2000000-0000-0000-0000-000000000166',id,observed_user_id,'submitted',now()
        FROM supervision_observations LIMIT 1;
        INSERT INTO supervision_evaluator_responses(observation_id,evaluator_id,template_item_id,rating_score,text_response)
        SELECT observation_id,id,'a1000000-0000-0000-0000-000000000166',4,'Retained evaluation' FROM supervision_evaluators;
        INSERT INTO supervision_actions(observation_id,actor_user_id,action_kind,from_status,to_status,comment)
        SELECT id,observed_user_id,'planned','requested','planned','Retained approval' FROM supervision_observations;
    "#).execute(&pool).await.unwrap();
    let mut expected = supervision_graph_snapshot(&pool).await;
    let cycles = expected["cycles"].as_array_mut().unwrap();
    assert!(!cycles.is_empty());
    for cycle in cycles {
        cycle["observation_start_date"] = serde_json::Value::Null;
        cycle["observation_end_date"] = serde_json::Value::Null;
    }
    let period_snapshot =
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM supervision_observation_periods p";
    let periods: serde_json::Value = sqlx::query_scalar(period_snapshot)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!periods.as_array().unwrap().is_empty());
    for _ in 0..2 {
        school_test_db::run_test_migrations(&pool).await;
        assert_eq!(supervision_graph_snapshot(&pool).await, expected);
        assert_eq!(
            sqlx::query_scalar::<_, serde_json::Value>(period_snapshot)
                .fetch_one(&pool)
                .await
                .unwrap(),
            periods
        );
    }
    for invalid in [
        "UPDATE supervision_cycles SET observation_start_date='2026-01-01'",
        "UPDATE supervision_cycles SET observation_end_date='2026-01-01'",
        "UPDATE supervision_cycles SET observation_start_date='2026-01-02',observation_end_date='2026-01-01'",
    ] {
        let err = sqlx::query(invalid).execute(&pool).await.unwrap_err();
        assert_eq!(err.as_database_error().and_then(|e| e.constraint()), Some("supervision_cycles_observation_dates_check"));
    }
    assert_eq!(supervision_graph_snapshot(&pool).await, expected);
}

#[test]
fn supervision_date_payloads_require_pairs_and_never_accept_null_clearing() {
    for payload in [
        serde_json::json!({"observationStartDate":null}),
        serde_json::json!({"observationEndDate":null}),
    ] {
        assert!(serde_json::from_value::<UpdateSupervisionCycleRequest>(payload).is_err());
    }
    assert!(serde_json::from_value::<UpdateSupervisionCycleRequest>(serde_json::json!({})).is_ok());
    assert!(
        serde_json::from_value::<ResubmitSupervisionObservationRequest>(serde_json::json!({}))
            .is_ok()
    );
    for field in [
        "observedDate",
        "timetableBlockGroupIds",
        "expectedSourceDigest",
        "observedAt",
    ] {
        let mut payload = serde_json::json!({});
        payload[field] = serde_json::json!("forged");
        assert!(serde_json::from_value::<ResubmitSupervisionObservationRequest>(payload).is_err());
    }
}

#[tokio::test]
async fn supervision_date_windows_are_inclusive_and_protect_every_ever_approved_period() {
    let pool = migrated_pool("supervision_date_window").await;
    let original_unset: i64 = sqlx::query_scalar("SELECT count(*) FROM supervision_cycles WHERE observation_start_date IS NULL AND observation_end_date IS NULL").fetch_one(&pool).await.unwrap();
    assert!(
        original_unset > 0,
        "166 must leave preexisting cycles unset"
    );
    let f = insert_fixture(&pool).await;
    let date = f.observed_at.date_naive();
    assert!(set_cycle_dates(&pool, &f, date + Duration::days(1), date)
        .await
        .is_err());
    assert!(services::update_cycle(
        &pool,
        f.cycle.id,
        UpdateSupervisionCycleRequest {
            observation_start_date: Some(date),
            ..Default::default()
        }
    )
    .await
    .is_err());
    set_cycle_dates(&pool, &f, date, date).await.unwrap();
    let options = options_for(&pool, &f, f.teacher_id, date).await;
    assert!(!options.items.is_empty());
    assert!(options.items.iter().all(|p| p.observed_date == date));
    let stale = observation_input(&pool, &f).await;
    set_cycle_dates(&pool, &f, date, date + Duration::days(1))
        .await
        .unwrap();
    assert!(matches!(
        services::request_observation(&pool, f.teacher_id, stale).await,
        Err(AppError::Conflict(_))
    ));
    set_cycle_dates(&pool, &f, date, date).await.unwrap();
    let outside = options_for(&pool, &f, f.teacher_id, date + Duration::weeks(1)).await;
    assert!(outside.items.is_empty());
    assert_eq!(
        outside.week_end_date - outside.week_start_date,
        Duration::days(6)
    );
    let observation = services::request_observation(
        &pool,
        f.teacher_id,
        multi_input(&pool, &f, f.teacher_id, date, 3).await,
    )
    .await
    .unwrap();
    let mut outside_input = observation_input(&pool, &f).await;
    outside_input.observed_date = date + Duration::weeks(1);
    assert!(
        services::request_observation(&pool, f.teacher_id, outside_input)
            .await
            .is_err()
    );
    approve_with(&pool, &f, observation.id, vec![f.evaluator_id]).await;
    services::return_observation_request(
        &pool,
        f.actor_id,
        observation.id,
        ReturnObservationRequest {
            comment: Some("Retain approved dates".into()),
        },
    )
    .await
    .unwrap();
    set_cycle_dates(&pool, &f, date, date + Duration::days(1))
        .await
        .unwrap();
    // A legacy later period can occupy another civil date; protecting only the
    // first summary timestamp would incorrectly allow this shrink.
    sqlx::query("UPDATE supervision_observation_periods SET observed_at=observed_at+interval '1 day',ends_at=ends_at+interval '1 day' WHERE observation_id=$1 AND position=3")
        .bind(observation.id).execute(&pool).await.unwrap();
    assert!(set_cycle_dates(&pool, &f, date, date).await.is_err());
    let before =
        serde_json::to_value(services::get_cycle(&pool, f.cycle.id).await.unwrap()).unwrap();
    assert!(set_cycle_dates(
        &pool,
        &f,
        date + Duration::days(1),
        date + Duration::days(2)
    )
    .await
    .is_err());
    assert_eq!(
        serde_json::to_value(services::get_cycle(&pool, f.cycle.id).await.unwrap()).unwrap(),
        before
    );
    // Legacy postapproval evidence still protects dates without inventing approved_at.
    sqlx::query("UPDATE supervision_observations SET approved_at=NULL WHERE id=$1")
        .bind(observation.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        services::get_observation(&pool, observation.id)
            .await
            .unwrap()
            .periods_locked
    );
    assert!(set_cycle_dates(
        &pool,
        &f,
        date - Duration::days(2),
        date - Duration::days(1)
    )
    .await
    .is_err());
    services::cancel_observation(
        &pool,
        f.actor_id,
        observation.id,
        CancelObservationRequest {
            reason: Some("Cancelled visits release date protection".into()),
        },
    )
    .await
    .unwrap();
    set_cycle_dates(
        &pool,
        &f,
        date + Duration::days(1),
        date + Duration::days(2),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn supervision_frozen_returned_resubmits_saved_times_without_stale_evaluator_gate() {
    let pool = migrated_pool("supervision_frozen_resubmit").await;
    let a = insert_fixture(&pool).await;
    let b = insert_fixture(&pool).await;
    let date = a.observed_at.date_naive();
    let first = request_observation(&pool, &a).await;
    let second = request_observation(&pool, &b).await;
    // Independent saved class evidence, with no shared teacher/evaluator yet.
    let other_room: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND id<>$2 ORDER BY id LIMIT 1",
    )
    .bind(b.cycle.academic_year_id)
    .bind(first.periods[0].homeroom_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE supervision_observation_periods SET homeroom_id=$2 WHERE observation_id=$1",
    )
    .bind(second.id)
    .bind(other_room)
    .execute(&pool)
    .await
    .unwrap();
    let approved = services::approve_observation_request(
        &pool,
        a.actor_id,
        first.id,
        ApproveObservationRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: a.evaluator_id,
                role_label: None,
                is_required: Some(true),
            }],
        },
    )
    .await
    .unwrap();
    assert!(approved.periods_locked);
    let selection = corrected_request(&pool, &a).await;
    assert!(services::update_requested_observation(
        &pool,
        a.teacher_id,
        first.id,
        selection.clone()
    )
    .await
    .is_err());
    assert!(services::update_observation(
        &pool,
        a.actor_id,
        first.id,
        UpdateSupervisionObservationRequest {
            observed_date: Some(selection.observed_date),
            timetable_block_group_ids: Some(selection.timetable_block_group_ids),
            expected_source_digest: Some(selection.expected_source_digest),
            template_id: None
        }
    )
    .await
    .is_err());
    services::return_observation_request(
        &pool,
        a.actor_id,
        first.id,
        ReturnObservationRequest {
            comment: Some("Choose a different evaluator".into()),
        },
    )
    .await
    .unwrap();
    approve_with(&pool, &b, second.id, vec![a.evaluator_id]).await;
    let saved = services::resubmit_observation_request(&pool, a.teacher_id, first.id)
        .await
        .unwrap();
    assert_eq!(saved.status, SupervisionObservationStatus::Requested);
    assert_eq!(
        serde_json::to_value(&saved.periods).unwrap(),
        serde_json::to_value(&approved.periods).unwrap()
    );
    assert_eq!(saved.evaluators[0].id, approved.evaluators[0].id);
    let availability =
        services::evaluator_availability(&pool, &management_actor(a.actor_id), first.id)
            .await
            .unwrap();
    assert!(availability.within_cycle_dates);
    assert!(!availability.overlap.blocked);
    assert!(
        !availability
            .items
            .iter()
            .find(|p| p.id == a.evaluator_id)
            .unwrap()
            .available
    );
    assert!(
        availability
            .items
            .iter()
            .find(|p| p.id == a.second_evaluator_id)
            .unwrap()
            .available
    );
    assert!(services::approve_observation_request(
        &pool,
        a.actor_id,
        first.id,
        ApproveObservationRequest {
            evaluators: vec![EvaluatorAssignmentInput {
                evaluator_user_id: a.evaluator_id,
                role_label: None,
                is_required: Some(true)
            }]
        }
    )
    .await
    .is_err());
    approve_with(&pool, &a, first.id, vec![a.second_evaluator_id]).await;
    assert_eq!(
        saved.periods[0]
            .observed_at
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive(),
        date
    );
}

#[tokio::test]
async fn supervision_aggregate_counts_are_private_and_hard_blocks_are_participant_or_exact_class() {
    let pool = migrated_pool("supervision_booking_aggregate").await;
    let a = insert_fixture(&pool).await;
    let b = insert_fixture(&pool).await;
    let date = a.observed_at.date_naive();
    let first = request_observation(&pool, &a).await;
    let second = request_observation(&pool, &b).await;
    let pending = options_for(&pool, &b, b.teacher_id, date)
        .await
        .items
        .into_iter()
        .find(|p| p.start_time.hour() == 10 && p.observed_date == date)
        .unwrap();
    assert_eq!(pending.booking_availability.pending_count, 2);
    assert!(
        pending.booking_availability.blocked,
        "own same-cycle pending blocks"
    );
    let correction = services::observation_timetable_options(
        &pool,
        second.id,
        date - Duration::days(i64::from(date.weekday().num_days_from_monday())),
    )
    .await
    .unwrap();
    let item = correction
        .items
        .iter()
        .find(|p| p.observed_date == date && p.start_time.hour() == 10)
        .unwrap();
    assert_eq!(item.booking_availability.pending_count, 1);
    assert!(
        !item.booking_availability.blocked,
        "other pending requests do not reserve a class"
    );
    approve_with(&pool, &a, first.id, vec![a.evaluator_id]).await;
    let private_actor = school_authorization::ActorContext {
        user_id: b.actor_id,
        permissions: vec![],
    };
    let response = services::evaluator_availability(&pool, &private_actor, second.id)
        .await
        .unwrap();
    assert_eq!(response.overlap.confirmed_count, 1);
    assert!(response.overlap.blocked);
    assert!(response.overlapping_bookings.is_empty());
    let hidden = response
        .items
        .iter()
        .find(|e| e.id == a.evaluator_id)
        .unwrap();
    assert!(!hidden.available && hidden.conflict.is_none());
    assert!(hidden
        .conflict_reason
        .as_deref()
        .unwrap()
        .contains("งานนิเทศ"));
    let scoped = services::evaluator_availability(&pool, &management_actor(b.actor_id), second.id)
        .await
        .unwrap();
    assert_eq!(scoped.overlapping_bookings.len(), 1);
    assert!(scoped
        .items
        .iter()
        .find(|e| e.id == a.evaluator_id)
        .unwrap()
        .conflict
        .is_some());
    let replacement = ApproveObservationRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: b.evaluator_id,
            role_label: None,
            is_required: Some(true),
        }],
    };
    assert!(
        services::approve_observation_request(&pool, b.actor_id, second.id, replacement.clone())
            .await
            .is_err(),
        "same exact class blocks even with a different evaluator"
    );
    let other_room: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND id<>$2 ORDER BY id LIMIT 1",
    )
    .bind(b.cycle.academic_year_id)
    .bind(first.periods[0].homeroom_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE supervision_observation_periods SET homeroom_id=$2 WHERE observation_id=$1",
    )
    .bind(second.id)
    .bind(other_room)
    .execute(&pool)
    .await
    .unwrap();
    services::approve_observation_request(&pool, b.actor_id, second.id, replacement)
        .await
        .unwrap();
    // Different exact classes and people can be confirmed at the same minute.
    assert_eq!(
        services::get_observation(&pool, second.id)
            .await
            .unwrap()
            .status,
        SupervisionObservationStatus::Planned
    );
}

#[tokio::test]
async fn supervision_cycle_date_shrink_and_approval_serialize_to_a_valid_outcome() {
    let pool = migrated_pool_with_connections("supervision_date_approval_race", 4).await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    let date = f.observed_at.date_naive();
    let assignment = ApproveObservationRequest {
        evaluators: vec![EvaluatorAssignmentInput {
            evaluator_user_id: f.evaluator_id,
            role_label: None,
            is_required: Some(true),
        }],
    };
    let (dates, approval) = tokio::join!(
        set_cycle_dates(
            &pool,
            &f,
            date + Duration::days(1),
            date + Duration::days(2)
        ),
        services::approve_observation_request(&pool, f.actor_id, observation.id, assignment)
    );
    assert_eq!(
        usize::from(dates.is_ok()) + usize::from(approval.is_ok()),
        1
    );
    let saved = services::get_observation(&pool, observation.id)
        .await
        .unwrap();
    let cycle = services::get_cycle(&pool, f.cycle.id).await.unwrap();
    if saved.periods_locked {
        assert!(
            cycle.observation_start_date.unwrap() <= date
                && date <= cycle.observation_end_date.unwrap()
        );
    } else {
        assert_eq!(cycle.observation_start_date, Some(date + Duration::days(1)));
    }
}

/// Distinct captured class evidence lets evaluator tests isolate people conflicts
/// without accidentally relying on two confirmed visits in the same exact class.
async fn separate_saved_class(pool: &PgPool, observation: &SupervisionObservation) {
    let other: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND id<>$2 ORDER BY id LIMIT 1",
    )
    .bind(observation.academic_year_id)
    .bind(observation.periods[0].homeroom_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE supervision_observation_periods SET homeroom_id=$2 WHERE observation_id=$1",
    )
    .bind(observation.id)
    .bind(other)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn supervision_http_evaluator_teaching_does_not_block_selection_or_assignment() {
    use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
    use crate::integration_tests::permission_coverage::{
        coverage_actor, coverage_request, coverage_router,
    };
    use crate::test_permission_codes as codes;
    use axum::http::{Method, StatusCode};
    use school_academic_timetable::services::dated_teaching::{
        occupancy_on_connection, AvailabilityWindow,
    };
    use serde_json::{json, Value};

    let pool = migrated_pool_with_connections("supervision_evaluator_teaching", 4).await;
    school_test_db::run_test_migrations(&pool).await;
    let f = insert_fixture(&pool).await;
    seed_teaching(
        &pool,
        f.second_evaluator_id,
        f.cycle.academic_year_id,
        f.academic_term_id,
    )
    .await;
    let observation = request_observation(&pool, &f).await;
    let period = &observation.periods[0];
    let start = period.observed_at.with_timezone(&chrono_tz::Asia::Bangkok);
    let end = period
        .ends_at
        .unwrap()
        .with_timezone(&chrono_tz::Asia::Bangkok);
    let window = AvailabilityWindow {
        date: start.date_naive(),
        start_time: start.time(),
        end_time: end.time(),
    };
    let occupancy = occupancy_on_connection(
        &mut pool.acquire().await.unwrap(),
        f.academic_term_id,
        &[f.second_evaluator_id],
        std::slice::from_ref(&window),
        observation.id,
    )
    .await
    .unwrap();
    assert!(occupancy.iter().any(|row| {
        row.teacher_id == f.second_evaluator_id
            && row.date == window.date
            && row.start_time < window.end_time
            && window.start_time < row.end_time
    }));

    let auth =
        ProtectedFixture::with_prepared_tenant_pool("supervision_evaluator_teaching", pool.clone())
            .await;
    let _lease = auth
        .runtime
        .pool_manager
        .get_pool("test-pool://session-router-tenant-a", "tenant-a")
        .await
        .unwrap();
    let manager = coverage_actor(&auth, &[codes::SUPERVISION_MANAGE_SCHOOL], 214).await;
    let app = coverage_router(&auth);
    let path = format!("/api/supervision/observations/{}", observation.id);
    let (status, availability) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::GET,
        &format!("{path}/evaluator-availability"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{availability}");
    let person = availability["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == f.second_evaluator_id.to_string())
        .unwrap();
    assert_eq!(person["available"], true);
    assert!(person["conflict"].is_null() && person["conflictReason"].is_null());

    let assignment =
        json!({"evaluators":[{"evaluatorUserId":f.second_evaluator_id,"isRequired":true}]});
    let (status, replaced) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::PUT,
        &format!("{path}/evaluators"),
        assignment.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["data"]["status"], "requested");
    assert_eq!(
        replaced["data"]["evaluators"][0]["evaluatorUserId"],
        person["id"]
    );

    let (status, approved) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::POST,
        &format!("{path}/approve-request"),
        assignment,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["data"]["status"], "planned");
    assert_eq!(
        approved["data"]["evaluators"][0]["evaluatorUserId"],
        person["id"]
    );

    // Consecutive confirmed supervision periods can share the same participants and class.
    let adjacent = services::request_observation(
        &pool,
        f.teacher_id,
        input_at(&pool, &f, f.teacher_id, period.ends_at.unwrap()).await,
    )
    .await
    .unwrap();
    assert_eq!(Some(adjacent.periods[0].observed_at), period.ends_at);
    assert_eq!(adjacent.periods[0].homeroom_id, period.homeroom_id);
    approve_with(&pool, &f, adjacent.id, vec![f.second_evaluator_id]).await;
}

#[tokio::test]
async fn supervision_observed_teacher_cannot_book_while_already_a_confirmed_evaluator() {
    let pool = migrated_pool("supervision_observed_as_participant").await;
    let f = insert_fixture(&pool).await;
    let observation = request_observation(&pool, &f).await;
    separate_saved_class(&pool, &observation).await;
    approve_with(&pool, &f, observation.id, vec![f.evaluator_id]).await;
    // A later timetable change makes the evaluator eligible to request their own
    // lesson in a different class; the confirmed supervision promise still blocks it.
    seed_teaching(
        &pool,
        f.evaluator_id,
        f.cycle.academic_year_id,
        f.academic_term_id,
    )
    .await;
    let options = options_for(&pool, &f, f.evaluator_id, f.observed_at.date_naive()).await;
    let item = options
        .items
        .iter()
        .find(|p| p.observed_date == f.observed_at.date_naive() && p.start_time.hour() == 10)
        .unwrap();
    assert_ne!(
        Some(item.homeroom_id),
        services::get_observation(&pool, observation.id)
            .await
            .unwrap()
            .periods[0]
            .homeroom_id
    );
    assert!(item.booking_availability.blocked);
    assert_eq!(item.booking_availability.confirmed_count, 1);
    let input = input_at(&pool, &f, f.evaluator_id, f.observed_at).await;
    let error = services::request_observation(&pool, f.evaluator_id, input)
        .await
        .unwrap_err();
    assert!(
        matches!(error,AppError::Conflict(message) if !message.contains(&observation.id.to_string()))
    );
}

#[tokio::test]
async fn supervision_http_dates_and_saved_times_resubmit_enforce_strict_actor_contracts() {
    use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
    use crate::integration_tests::permission_coverage::{
        coverage_actor, coverage_request, coverage_router,
    };
    use crate::test_permission_codes as codes;
    use axum::http::{Method, StatusCode};
    use serde_json::{json, Value};
    let pool = migrated_pool_with_connections("supervision_dates_http", 4).await;
    school_test_db::run_test_migrations(&pool).await;
    let f = insert_fixture(&pool).await;
    let auth =
        ProtectedFixture::with_prepared_tenant_pool("supervision_dates_http", pool.clone()).await;
    let _lease = auth
        .runtime
        .pool_manager
        .get_pool("test-pool://session-router-tenant-a", "tenant-a")
        .await
        .unwrap();
    let own = coverage_actor(&auth, &[codes::SUPERVISION_REQUEST_OWN], 211).await;
    let other = coverage_actor(&auth, &[codes::SUPERVISION_REQUEST_OWN], 212).await;
    let manager = coverage_actor(&auth, &[codes::SUPERVISION_MANAGE_SCHOOL], 213).await;
    seed_teaching(&pool, own.id, f.cycle.academic_year_id, f.academic_term_id).await;
    let app = coverage_router(&auth);
    let date = f.observed_at.date_naive();
    let week = date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
    let cycle_path = format!("/api/supervision/cycles/{}", f.cycle.id);
    let dates = json!({"observationStartDate":date,"observationEndDate":date});
    let (status, _) =
        coverage_request(&app, &auth, &own, Method::PATCH, &cycle_path, dates.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for (payload, expected) in [
        (
            json!({"observationStartDate":date}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({"observationStartDate":null,"observationEndDate":null}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({"observationStartDate":date+Duration::days(1),"observationEndDate":date}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let (status, response) = coverage_request(
            &app,
            &auth,
            &manager,
            Method::PATCH,
            &cycle_path,
            payload.clone(),
        )
        .await;
        assert_eq!(status, expected, "payload={payload}; response={response}");
        assert_eq!(response["success"], false);
        assert!(response["error"].as_str().is_some());
    }
    let (status, configured) =
        coverage_request(&app, &auth, &manager, Method::PATCH, &cycle_path, dates).await;
    assert_eq!(status, StatusCode::OK, "{configured}");
    assert_eq!(configured["data"]["observationStartDate"], date.to_string());
    let mut cycle_create = json!({"academicYearId":f.cycle.academic_year_id,"academicTermId":f.academic_term_id,"title":"Dates required","templateId":f.template.id,"status":"open","targets":[{"targetType":"staff","targetId":other.id,"requiredObservations":1,"priority":1}]});
    let (status, _) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::POST,
        "/api/supervision/cycles",
        cycle_create.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    cycle_create["observationStartDate"] = json!(date);
    cycle_create["observationEndDate"] = json!(date);
    let (status, created_cycle) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::POST,
        "/api/supervision/cycles",
        cycle_create,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created_cycle}");
    assert_eq!(
        created_cycle["data"]["observationEndDate"],
        date.to_string()
    );
    let outside_target=format!("/api/supervision/booking-options?cycleId={}&academicYearId={}&academicTermId={}&weekStartDate={week}",created_cycle["data"]["id"].as_str().unwrap(),f.cycle.academic_year_id,f.academic_term_id);
    let (status, _) =
        coverage_request(&app, &auth, &own, Method::GET, &outside_target, Value::Null).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let missing_cycle = format!(
        "/api/supervision/booking-options?academicYearId={}&academicTermId={}&weekStartDate={week}",
        f.cycle.academic_year_id, f.academic_term_id
    );
    let (status, _) =
        coverage_request(&app, &auth, &own, Method::GET, &missing_cycle, Value::Null).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let bad_context=format!("/api/supervision/booking-options?cycleId={}&academicYearId={}&academicTermId={}&weekStartDate={week}",f.cycle.id,Uuid::new_v4(),f.academic_term_id);
    let (status, _) =
        coverage_request(&app, &auth, &own, Method::GET, &bad_context, Value::Null).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let options_path = format!("{missing_cycle}&cycleId={}", f.cycle.id);
    let (status, options) =
        coverage_request(&app, &auth, &own, Method::GET, &options_path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{options}");
    let option = options["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["startTime"] == "10:00:00")
        .unwrap();
    assert_eq!(
        option["bookingAvailability"],
        json!({"pendingCount":0,"confirmedCount":0,"blocked":false})
    );
    assert!(option.get("observedUserId").is_none() && option.get("overlappingBookings").is_none());
    let selection = json!({"observedDate":date,"timetableBlockGroupIds":[option["blockGroupId"]],"expectedSourceDigest":options["data"]["sourceDigest"]});
    let mut create = selection.clone();
    create["cycleId"] = json!(f.cycle.id);
    create["academicTermId"] = json!(f.academic_term_id);
    let (status, requested) = coverage_request(
        &app,
        &auth,
        &own,
        Method::POST,
        "/api/supervision/observations/requests",
        create,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{requested}");
    assert_eq!(requested["data"]["periodsLocked"], false);
    let id = requested["data"]["id"].as_str().unwrap();
    let path = format!("/api/supervision/observations/{id}");
    let availability_path = format!("{path}/evaluator-availability");
    let (status, _) = coverage_request(
        &app,
        &auth,
        &own,
        Method::GET,
        &availability_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, availability) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::GET,
        &availability_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{availability}");
    assert_eq!(availability["data"]["withinCycleDates"], true);
    assert_eq!(availability["data"]["overlap"]["pendingCount"], 0);
    assert_eq!(availability["data"]["overlappingBookings"], json!([]));
    assert!(availability["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == f.second_evaluator_id.to_string() && row["available"] == true));
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(f.second_evaluator_id)
        .execute(&pool)
        .await
        .unwrap();
    let student = test_user(&pool, "nonstaff-evaluator").await;
    sqlx::query("UPDATE users SET user_type='student' WHERE id=$1")
        .bind(student)
        .execute(&pool)
        .await
        .unwrap();
    let approve_path = format!("{path}/approve-request");
    let replace_path = format!("{path}/evaluators");
    let assignment = |id| json!({"evaluators":[{"evaluatorUserId":id,"isRequired":true}]});
    let before_assignment = supervision_graph_snapshot(&pool).await;
    for (method, path) in [(Method::POST, &approve_path), (Method::PUT, &replace_path)] {
        for evaluator in [f.second_evaluator_id, student] {
            let (status, error) = coverage_request(
                &app,
                &auth,
                &manager,
                method.clone(),
                path,
                assignment(evaluator),
            )
            .await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
            assert_eq!(error["success"], false);
            assert_eq!(error["error"], "ผู้ประเมินต้องเป็นบุคลากรที่ยังใช้งานอยู่");
        }
    }
    assert_eq!(supervision_graph_snapshot(&pool).await, before_assignment);
    sqlx::query("UPDATE users SET status='active' WHERE id=$1")
        .bind(f.second_evaluator_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, approved) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::POST,
        &approve_path,
        assignment(f.evaluator_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["data"]["periodsLocked"], true);
    let (status, returned) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::POST,
        &format!("{path}/return-request"),
        json!({"comment":"Keep approved times; replace evaluator if needed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    let (status, _) = coverage_request(
        &app,
        &auth,
        &own,
        Method::PATCH,
        &format!("{path}/request"),
        selection,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status,_)=coverage_request(&app,&auth,&manager,Method::PATCH,&cycle_path,json!({"observationStartDate":date+Duration::days(1),"observationEndDate":date+Duration::days(2)})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let resubmit = format!("{path}/resubmit-request");
    let (status, error) = coverage_request(
        &app,
        &auth,
        &own,
        Method::POST,
        &resubmit,
        json!({"observedDate":date}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["success"], false);
    for actor in [&other, &manager] {
        let (status, _) =
            coverage_request(&app, &auth, actor, Method::POST, &resubmit, json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let before = serde_json::to_value(
        services::get_observation(&pool, id.parse().unwrap())
            .await
            .unwrap(),
    )
    .unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_saved_resubmit_action() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'SUPERVISION_SAVED_RESUBMIT_AUDIT'; END $$; CREATE TRIGGER fail_saved_resubmit_action BEFORE INSERT ON supervision_actions FOR EACH ROW EXECUTE FUNCTION fail_saved_resubmit_action();").execute(&pool).await.unwrap();
    let (status, _) = coverage_request(&app, &auth, &own, Method::POST, &resubmit, json!({})).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        serde_json::to_value(
            services::get_observation(&pool, id.parse().unwrap())
                .await
                .unwrap()
        )
        .unwrap(),
        before
    );
    sqlx::raw_sql("DROP TRIGGER fail_saved_resubmit_action ON supervision_actions; DROP FUNCTION fail_saved_resubmit_action();").execute(&pool).await.unwrap();
    let (status, saved) =
        coverage_request(&app, &auth, &own, Method::POST, &resubmit, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["data"]["status"], "requested");
    assert_eq!(saved["data"]["periods"], approved["data"]["periods"]);
    assert_eq!(saved["data"]["canManage"], false);
    assert_eq!(saved["data"]["canReview"], false);
    assert_eq!(saved["data"]["averageRating"], Value::Null);
    // A legacy approved cycle may still be unset. Roster availability describes
    // people independently, while approval is separately guarded by its bounds.
    sqlx::query("UPDATE supervision_cycles SET observation_start_date=NULL,observation_end_date=NULL WHERE id=$1").bind(f.cycle.id).execute(&pool).await.unwrap();
    let (status, availability) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::GET,
        &availability_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{availability}");
    assert_eq!(availability["data"]["withinCycleDates"], false);
    assert!(availability["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == f.second_evaluator_id.to_string() && e["available"] == true));
    let (status, reassigned) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::PUT,
        &format!("{path}/evaluators"),
        json!({"evaluators":[{"evaluatorUserId":f.second_evaluator_id,"isRequired":true}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reassigned}");
    assert_eq!(reassigned["data"]["periods"], approved["data"]["periods"]);
    let (status, _) = coverage_request(
        &app,
        &auth,
        &manager,
        Method::POST,
        &format!("{path}/approve-request"),
        json!({"evaluators":[{"evaluatorUserId":f.second_evaluator_id,"isRequired":true}]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
