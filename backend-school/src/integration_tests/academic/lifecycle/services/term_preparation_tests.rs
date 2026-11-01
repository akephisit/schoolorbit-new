use school_authorization::ActorContext;
use std::collections::HashSet;

use uuid::Uuid;

use super::term_preparation::{apply, normalize_input, preview, validate_checksum};
use crate::integration_tests::academic::{
    core,
    delivery::services::offerings,
    lifecycle::models::{
        ApplyTermPreparationInput, PreviewTermPreparationInput, TermPreparationDateMapping,
        TermPreparationEntityMapping, TermPreparationMappings,
    },
};
use school_academic_core::ports::{TermPreparationMappingKind, TermPreparationModule};
use school_permissions::registry::codes;

fn preparation_mappings() -> TermPreparationMappings {
    TermPreparationMappings {
        offering_starts_on: Some(
            chrono::Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive(),
        ),
        entities: vec![],
        dates: vec![],
    }
}

#[test]
fn preparation_input_requires_unique_modules_and_bounded_unambiguous_mappings() {
    assert!(normalize_input(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &[TermPreparationModule::Delivery],
        &preparation_mappings(),
    )
    .is_ok());

    assert!(
        normalize_input(Uuid::new_v4(), Uuid::new_v4(), &[], &preparation_mappings(),).is_err()
    );
    assert!(normalize_input(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &[
            TermPreparationModule::Delivery,
            TermPreparationModule::Delivery,
        ],
        &preparation_mappings(),
    )
    .is_err());

    let source = Uuid::new_v4();
    let target_a = Uuid::new_v4();
    let target_b = Uuid::new_v4();
    let mappings = TermPreparationMappings {
        offering_starts_on: preparation_mappings().offering_starts_on,
        entities: vec![
            TermPreparationEntityMapping {
                kind: TermPreparationMappingKind::Teacher,
                source_id: source,
                target_id: target_a,
            },
            TermPreparationEntityMapping {
                kind: TermPreparationMappingKind::Teacher,
                source_id: source,
                target_id: target_b,
            },
        ],
        dates: Vec::new(),
    };
    assert!(normalize_input(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &[TermPreparationModule::Timetable],
        &mappings,
    )
    .is_err());
}

#[test]
fn preparation_normalization_is_deterministic() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let mappings = TermPreparationMappings {
        offering_starts_on: preparation_mappings().offering_starts_on,
        entities: vec![
            TermPreparationEntityMapping {
                kind: TermPreparationMappingKind::Room,
                source_id: second,
                target_id: second,
            },
            TermPreparationEntityMapping {
                kind: TermPreparationMappingKind::Teacher,
                source_id: first,
                target_id: first,
            },
        ],
        dates: Vec::new(),
    };
    let normalized = normalize_input(
        Uuid::from_u128(10),
        Uuid::from_u128(11),
        &[
            TermPreparationModule::Timetable,
            TermPreparationModule::Delivery,
        ],
        &mappings,
    )
    .unwrap();
    assert_eq!(
        normalized.modules,
        vec![
            TermPreparationModule::Delivery,
            TermPreparationModule::Timetable
        ]
    );
    assert_eq!(
        normalized
            .mappings
            .entities
            .iter()
            .map(|mapping| (mapping.kind, mapping.source_id))
            .collect::<HashSet<_>>()
            .len(),
        2
    );
    assert!(validate_checksum(&"a".repeat(64)).is_ok());
    assert!(validate_checksum(&"A".repeat(64)).is_err());
    assert!(validate_checksum("short").is_err());
}

async fn delivery_preparation_fixture(name: &str) -> (sqlx::PgPool, ActorContext, Uuid, Uuid) {
    let pool = core::services_tests::prepare_core_fixture(name).await;
    let actor = ActorContext {
        user_id: sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        permissions: vec![codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into()],
    };
    let (source_term_id, source_year_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id,academic_year_id FROM academic_terms WHERE status='active' ORDER BY sequence_no,id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (target_term_id, target_year_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT term.id,term.academic_year_id
           FROM academic_terms term
           JOIN academic_years year ON year.id=term.academic_year_id
           WHERE term.status='planning'
             AND year.year > (SELECT year FROM academic_years WHERE id=$1)
           ORDER BY year.year,term.sequence_no
           LIMIT 1"#,
    )
    .bind(source_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO bell_schedule_periods(
               id,bell_schedule_id,name,start_time,end_time,order_index,applicable_days
           )
           SELECT gen_random_uuid(),target_term.bell_schedule_id,source_period.name,
                  source_period.start_time,source_period.end_time,source_period.order_index,
                  source_period.applicable_days
           FROM academic_terms source_term
           JOIN bell_schedule_periods source_period
             ON source_period.bell_schedule_id=source_term.bell_schedule_id
           CROSS JOIN academic_terms target_term
           WHERE source_term.id=$1 AND target_term.id=$2
             AND NOT EXISTS (
                 SELECT 1 FROM bell_schedule_periods target_period
                 WHERE target_period.bell_schedule_id=target_term.bell_schedule_id
                   AND target_period.order_index=source_period.order_index
             )"#,
    )
    .bind(source_term_id)
    .bind(target_term_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO academic_year_grade_levels(academic_year_id,grade_level_id)
           SELECT $1,grade_level_id
           FROM academic_year_grade_levels
           WHERE academic_year_id=$2
           ON CONFLICT DO NOTHING"#,
    )
    .bind(target_year_id)
    .bind(source_year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO homerooms(
               id,code,name,academic_year_id,grade_level_id,room_number,is_active,
               metadata,study_program_id,capacity,row_version,migration_provenance
           )
           SELECT gen_random_uuid(),'PREP-' || code,name,$1,grade_level_id,room_number,true,
                  '{}'::jsonb,study_program_id,capacity,1,'{}'::jsonb
           FROM homerooms source
           WHERE academic_year_id=$2 AND is_active
             AND NOT EXISTS (
                 SELECT 1 FROM homerooms target
                 WHERE target.academic_year_id=$1
                   AND target.grade_level_id=source.grade_level_id
                   AND target.room_number=source.room_number
             )"#,
    )
    .bind(target_year_id)
    .bind(source_year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed',row_version=row_version+1 WHERE id=$1")
        .bind(source_term_id)
        .execute(&pool)
        .await
        .unwrap();
    (pool, actor, source_term_id, target_term_id)
}

#[tokio::test]
async fn preparation_requires_a_future_term_in_an_open_target_year() {
    let (pool, actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_future_boundary").await;
    let (source_year_id, target_year_id, target_year_number): (Uuid, Uuid, i32) = sqlx::query_as("SELECT source.academic_year_id,target.academic_year_id,year.year FROM academic_terms source CROSS JOIN academic_terms target JOIN academic_years year ON year.id=target.academic_year_id WHERE source.id=$1 AND target.id=$2")
        .bind(source_term_id).bind(target_term_id).fetch_one(&pool).await.unwrap();
    let request = PreviewTermPreparationInput {
        source_term_id,
        target_term_id,
        modules: vec![TermPreparationModule::Delivery],
        mappings: preparation_mappings(),
    };
    let source_year_number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(source_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET year=$2 WHERE id=$1")
        .bind(target_year_id)
        .bind(source_year_number - 100)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        preview(&pool, &actor, request.clone()).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    sqlx::query("UPDATE academic_years SET year=$2 WHERE id=$1")
        .bind(target_year_id)
        .bind(target_year_number)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(target_year_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        preview(&pool, &actor, request).await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn delivery_preparation_preview_is_read_only_and_apply_creates_only_target_drafts() {
    let (pool, actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_delivery").await;
    let source_before: (i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1),
               (SELECT count(*) FROM learning_group_students student
                JOIN learning_groups learning_group ON learning_group.id=student.learning_group_id
                WHERE learning_group.academic_term_id=$1)"#,
    )
    .bind(source_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering_date =
        preparation_mappings().offering_starts_on.unwrap() + chrono::Duration::days(7);
    let request = PreviewTermPreparationInput {
        source_term_id,
        target_term_id,
        modules: vec![TermPreparationModule::Delivery],
        mappings: TermPreparationMappings {
            offering_starts_on: Some(offering_date),
            ..preparation_mappings()
        },
    };
    let workspace = preview(&pool, &actor, request.clone()).await.unwrap();
    assert!(workspace.can_apply, "{:?}", workspace.findings);
    assert!(workspace.modules[0].draft_count > 0);
    let preview_writes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1")
            .bind(target_term_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(preview_writes, 0);

    let apply_request = ApplyTermPreparationInput {
        request_id: Uuid::new_v4(),
        source_checksum: workspace.source_checksum,
        source_term_id,
        target_term_id,
        modules: request.modules.clone(),
        mappings: request.mappings.clone(),
    };
    let outcome = apply(&pool, &actor, apply_request.clone()).await.unwrap();
    let mut changed_date = apply_request;
    changed_date.mappings.offering_starts_on = Some(offering_date + chrono::Duration::days(1));
    assert!(matches!(
        apply(&pool, &actor, changed_date).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let first_intent: Option<chrono::NaiveDate> = sqlx::query_scalar(
        "SELECT offering_starts_on FROM academic_term_preparation_runs WHERE id=$1",
    )
    .bind(outcome.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first_intent, Some(offering_date));
    assert_eq!(outcome.modules.len(), 1);
    assert!(outcome.modules[0].created_count > 0);
    let target_state: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1 AND status='draft'),
               (SELECT count(*) FROM learning_groups WHERE academic_term_id=$1 AND status='draft'),
               (SELECT count(*) FROM learning_group_students student
                JOIN learning_groups learning_group ON learning_group.id=student.learning_group_id
                WHERE learning_group.academic_term_id=$1)"#,
    )
    .bind(target_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(target_state.0 > 0 && target_state.1 > 0);
    assert_eq!(target_state.2, 0);
    let retained_workspace = preview(&pool, &actor, request.clone()).await.unwrap();
    assert!(
        retained_workspace.can_apply,
        "{:?}",
        retained_workspace.findings
    );
    let retained_request = ApplyTermPreparationInput {
        request_id: Uuid::new_v4(),
        source_checksum: retained_workspace.source_checksum,
        source_term_id,
        target_term_id,
        modules: request.modules,
        mappings: request.mappings,
    };
    let retained = apply(&pool, &actor, retained_request.clone())
        .await
        .unwrap();
    assert_eq!(
        retained.modules[0].created_count, 1,
        "retained offerings still create a distinct Delivery revision"
    );
    assert_ne!(retained.run_id, outcome.run_id);
    let retained_intent: Option<chrono::NaiveDate> = sqlx::query_scalar(
        "SELECT offering_starts_on FROM academic_term_preparation_runs WHERE id=$1",
    )
    .bind(retained.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        retained_intent, None,
        "the date binds only a newly created offering effect"
    );
    let facts_sql = r#"SELECT jsonb_build_object(
        'offerings',(SELECT jsonb_agg(to_jsonb(offering) ORDER BY offering.id) FROM learning_offerings offering WHERE academic_term_id=$1),
        'revisions',(SELECT jsonb_agg(to_jsonb(revision) ORDER BY revision.id) FROM academic_term_change_sets revision WHERE academic_term_id=$1),
        'versions',(SELECT jsonb_agg(to_jsonb(version) ORDER BY version.id) FROM academic_delivery_versions version WHERE academic_term_id=$1),
        'audits',(SELECT jsonb_agg(to_jsonb(audit) ORDER BY audit.id) FROM academic_audit_events audit WHERE academic_term_id=$1))"#;
    let before_replay: serde_json::Value = sqlx::query_scalar(facts_sql)
        .bind(target_term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let distinct_dates: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM academic_term_change_sets revision JOIN academic_delivery_versions version ON version.id=revision.target_delivery_version_id WHERE revision.id=ANY($1) AND revision.reference_date=version.reference_date AND revision.reference_date<>$2)",
    )
    .bind(&retained.modules[0].target_ids).bind(offering_date).fetch_one(&pool).await.unwrap();
    assert!(
        distinct_dates,
        "the retained run preserves its independent dated Delivery revision"
    );
    let mut retained_changed_date = retained_request;
    retained_changed_date.mappings.offering_starts_on =
        Some(offering_date + chrono::Duration::days(30));
    let replay = apply(&pool, &actor, retained_changed_date).await.unwrap();
    assert_eq!(
        serde_json::to_value(&replay).unwrap(),
        serde_json::to_value(&retained).unwrap()
    );
    let after_replay: serde_json::Value = sqlx::query_scalar(facts_sql)
        .bind(target_term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_replay, before_replay, "replay preserves exact offering dates, revision/version IDs and dates, and original audits");

    let source_after: (i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1),
               (SELECT count(*) FROM learning_group_students student
                JOIN learning_groups learning_group ON learning_group.id=student.learning_group_id
                WHERE learning_group.academic_term_id=$1)"#,
    )
    .bind(source_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source_after, source_before);
    assert!(
        sqlx::query("DELETE FROM academic_term_preparation_runs WHERE id=$1")
            .bind(outcome.run_id)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn selected_module_preparation_is_atomic_replay_safe_and_omits_operational_records() {
    let (pool, actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_all_modules").await;
    let base_mappings = preparation_mappings();
    let offering_date = base_mappings.offering_starts_on.unwrap();
    let date_mappings = vec![
        TermPreparationDateMapping {
            source_date: chrono::NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
            target_date: offering_date,
        },
        TermPreparationDateMapping {
            source_date: chrono::NaiveDate::from_ymd_opt(2025, 9, 15).unwrap(),
            target_date: offering_date + chrono::Duration::days(137),
        },
        TermPreparationDateMapping {
            source_date: chrono::NaiveDate::from_ymd_opt(2025, 10, 31).unwrap(),
            target_date: offering_date + chrono::Duration::days(183),
        },
    ];
    let source_facts_sql = r#"SELECT jsonb_build_object(
        'offerings',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offerings r WHERE academic_term_id=$1),
        'groups',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_groups r WHERE academic_term_id=$1),
        'plans',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM course_assessment_plans r WHERE academic_term_id=$1),
        'phases',(SELECT jsonb_agg(to_jsonb(r) ORDER BY r.id) FROM course_assessment_phases r JOIN course_assessment_plans p ON p.id=r.plan_id WHERE p.academic_term_id=$1),
        'timetables',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_timetable_versions r WHERE academic_term_id=$1),
        'examRounds',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_exam_rounds r WHERE academic_term_id=$1),
        'examDays',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_exam_days r WHERE academic_term_id=$1),
        'cycles',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM supervision_cycles r WHERE academic_term_id=$1))"#;
    let source_before: serde_json::Value = sqlx::query_scalar(source_facts_sql)
        .bind(source_term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let modules = TermPreparationModule::ALL.to_vec();
    let initial_workspace = preview(
        &pool,
        &actor,
        PreviewTermPreparationInput {
            source_term_id,
            target_term_id,
            modules: modules.clone(),
            mappings: base_mappings.clone(),
        },
    )
    .await
    .unwrap();
    assert!(
        initial_workspace
            .modules
            .iter()
            .all(|evidence| evidence.source_count > 0),
        "fixture must exercise every selected provider: {:?}",
        initial_workspace.modules
    );
    assert!(!initial_workspace.can_apply);
    assert!(initial_workspace
        .findings
        .iter()
        .any(|finding| finding.code == "timetable.delivery_not_published"));
    let opening_preparation = preview(
        &pool,
        &actor,
        PreviewTermPreparationInput {
            source_term_id,
            target_term_id,
            modules: vec![
                TermPreparationModule::Delivery,
                TermPreparationModule::Assessments,
            ],
            mappings: base_mappings.clone(),
        },
    )
    .await
    .unwrap();
    assert!(
        opening_preparation.can_apply,
        "{:?}",
        opening_preparation.findings
    );
    let opening_request = ApplyTermPreparationInput {
        request_id: Uuid::new_v4(),
        source_checksum: opening_preparation.source_checksum,
        source_term_id,
        target_term_id,
        modules: vec![
            TermPreparationModule::Delivery,
            TermPreparationModule::Assessments,
        ],
        mappings: base_mappings.clone(),
    };
    let opening_outcome = apply(&pool, &actor, opening_request.clone()).await.unwrap();
    assert_eq!(opening_outcome.modules.len(), 2);
    assert!(opening_outcome
        .modules
        .iter()
        .all(|module| module.created_count > 0));
    let assessment_facts_sql = r#"SELECT jsonb_build_object(
        'plans',(SELECT jsonb_agg(to_jsonb(plan) ORDER BY plan.id) FROM course_assessment_plans plan WHERE academic_term_id=$1),
        'phases',(SELECT jsonb_agg(to_jsonb(phase) ORDER BY phase.id) FROM course_assessment_phases phase JOIN course_assessment_plans plan ON plan.id=phase.plan_id WHERE plan.academic_term_id=$1))"#;
    let prepared_assessments: serde_json::Value = sqlx::query_scalar(assessment_facts_sql)
        .bind(target_term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let opening_revision_id:Uuid=sqlx::query_scalar("SELECT id FROM academic_term_change_sets WHERE academic_term_id=$1 AND status='draft' ORDER BY created_at DESC LIMIT 1").bind(target_term_id).fetch_one(&pool).await.unwrap();
    use school_academic_delivery::{
        models as delivery_models,
        services::{change_sets, groups},
    };
    let target_groups: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT id,row_version FROM learning_groups WHERE academic_term_id=$1 ORDER BY id",
    )
    .bind(target_term_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    for (group_id, row_version) in target_groups {
        groups::replace_teachers(
            &pool,
            actor.user_id,
            group_id,
            delivery_models::ReplaceLearningGroupTeachersRequest {
                row_version,
                teachers: vec![delivery_models::TeacherAssignmentInput {
                    teacher_id: actor.user_id,
                    role: delivery_models::LearningTeacherRole::Primary,
                }],
            },
        )
        .await
        .unwrap();
    }
    let opening_readiness = publication_preview(&pool, opening_revision_id)
        .await
        .unwrap();
    assert!(
        !opening_readiness
            .findings
            .iter()
            .any(|finding| finding.severity
                == delivery_models::AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        opening_readiness.findings
    );
    change_sets::publish_change_set(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
        &pool,
        actor.user_id,
        opening_revision_id,
        delivery_models::PublishAcademicTermChangeSetRequest {
            effective_from: offering_date,
            row_version: opening_readiness.change_set_row_version,
            target_delivery_version_row_version: opening_readiness
                .target_delivery_version_row_version,
            preview_hash: opening_readiness.preview_hash,
            acknowledged_warning_codes: Vec::new(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let published_assessments: serde_json::Value = sqlx::query_scalar(assessment_facts_sql)
        .bind(target_term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        published_assessments["phases"], prepared_assessments["phases"],
        "publication preserves every prepared phase fact and identity"
    );
    let mut retained_assessments = published_assessments.clone();
    let before_plans = prepared_assessments["plans"].as_array().unwrap();
    let after_plans = retained_assessments["plans"].as_array_mut().unwrap();
    assert_eq!(after_plans.len(), before_plans.len());
    for after in after_plans {
        let before = before_plans
            .iter()
            .find(|plan| plan["id"] == after["id"])
            .unwrap();
        if before["assessment_coordinator_id"].is_null() {
            assert_eq!(
                after["assessment_coordinator_id"],
                serde_json::json!(actor.user_id)
            );
            assert_eq!(
                after["row_version"].as_i64().unwrap(),
                before["row_version"].as_i64().unwrap() + 1
            );
            let before_updated: chrono::DateTime<chrono::Utc> =
                serde_json::from_value(before["updated_at"].clone()).unwrap();
            let after_updated: chrono::DateTime<chrono::Utc> =
                serde_json::from_value(after["updated_at"].clone()).unwrap();
            assert!(after_updated >= before_updated);
            // Main146 initializes only a missing owner; every other prepared fact stays exact.
            for field in ["assessment_coordinator_id", "row_version", "updated_at"] {
                after[field] = before[field].clone();
            }
        }
        assert_eq!(
            after, before,
            "publication preserves all other plan facts, including existing owners"
        );
    }
    assert_eq!(retained_assessments, prepared_assessments);
    let opening_replay = apply(&pool, &actor, opening_request).await.unwrap();
    assert_eq!(
        serde_json::to_value(&opening_replay).unwrap(),
        serde_json::to_value(&opening_outcome).unwrap()
    );
    let actual_offering_dates:Vec<chrono::NaiveDate>=sqlx::query_scalar("SELECT DISTINCT starts_on FROM learning_offerings WHERE academic_term_id=$1 ORDER BY starts_on").bind(target_term_id).fetch_all(&pool).await.unwrap();
    assert_eq!(actual_offering_dates, vec![offering_date]);
    let modules = TermPreparationModule::ALL
        .into_iter()
        .filter(|module| {
            !matches!(
                module,
                TermPreparationModule::Delivery | TermPreparationModule::Assessments
            )
        })
        .collect::<Vec<_>>();
    let mut tx = pool.begin().await.unwrap();
    let delivery_preview =
        offerings::preview_term_preparation(&mut tx, target_term_id, offering_date)
            .await
            .unwrap();
    tx.rollback().await.unwrap();
    let mut entity_mappings = Vec::new();
    for requirement in &initial_workspace.mapping_requirements {
        let target_id = if requirement.kind == TermPreparationMappingKind::LearningGroup {
            let source_offering_id: Uuid =
                sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id=$1")
                    .bind(requirement.source_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            let target_offering_id = initial_workspace
                .mapping_requirements
                .iter()
                .find(|candidate| {
                    candidate.kind == TermPreparationMappingKind::LearningOffering
                        && candidate.source_id == source_offering_id
                })
                .and_then(|candidate| candidate.selected_target_id)
                .unwrap();
            let proposal = delivery_preview
                .proposals
                .iter()
                .find(|proposal| {
                    proposal.existing_offering_id.unwrap_or_else(|| {
                        offerings::prepared_offering_id(
                            target_term_id,
                            proposal.resource_kind,
                            proposal.catalog_version_id,
                        )
                    }) == target_offering_id
                })
                .unwrap();
            offerings::prepared_group_id(target_offering_id, &proposal.default_groups[0].group_key)
        } else {
            requirement
                .selected_target_id
                .or_else(|| requirement.target_options.first().map(|option| option.id))
                .unwrap()
        };
        entity_mappings.push(TermPreparationEntityMapping {
            kind: requirement.kind,
            source_id: requirement.source_id,
            target_id,
        });
    }
    let mappings = TermPreparationMappings {
        offering_starts_on: Some(offering_date),
        entities: entity_mappings,
        dates: date_mappings.clone(),
    };
    let workspace = preview(
        &pool,
        &actor,
        PreviewTermPreparationInput {
            source_term_id,
            target_term_id,
            modules: modules.clone(),
            mappings: mappings.clone(),
        },
    )
    .await
    .unwrap();
    assert!(workspace.can_apply, "{:?}", workspace.findings);
    assert!(workspace
        .mapping_requirements
        .iter()
        .all(|requirement| requirement.selected_target_id.is_some()));
    assert!(workspace
        .date_requirements
        .iter()
        .all(|requirement| requirement.selected_target_date.is_some()));
    let request = ApplyTermPreparationInput {
        request_id: Uuid::new_v4(),
        source_checksum: workspace.source_checksum.clone(),
        source_term_id,
        target_term_id,
        modules,
        mappings,
    };
    let outcome = apply(&pool, &actor, request.clone()).await.unwrap();
    let replay = apply(&pool, &actor, request.clone()).await.unwrap();
    assert_eq!(replay.run_id, outcome.run_id);
    assert_eq!(outcome.modules.len(), TermPreparationModule::ALL.len() - 2);
    assert_ne!(outcome.run_id, opening_outcome.run_id);
    assert!(outcome
        .modules
        .iter()
        .all(|module| module.created_count > 0));
    let reviewed_dates = date_mappings
        .iter()
        .map(|mapping| (mapping.source_date, mapping.target_date))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        workspace
            .date_requirements
            .iter()
            .map(|requirement| (
                requirement.source_date,
                requirement.selected_target_date.unwrap()
            ))
            .collect::<std::collections::BTreeMap<_, _>>(),
        reviewed_dates
    );
    let exam_dates:Vec<chrono::NaiveDate> = sqlx::query_scalar("SELECT DISTINCT exam_date FROM academic_exam_days WHERE academic_term_id=$1 ORDER BY exam_date").bind(target_term_id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        exam_dates,
        vec![offering_date + chrono::Duration::days(137)]
    );
    let cycle_dates:Vec<(chrono::NaiveDate,chrono::NaiveDate)> = sqlx::query_as("SELECT (starts_at AT TIME ZONE 'UTC')::date,(ends_at AT TIME ZONE 'UTC')::date FROM supervision_cycles WHERE academic_term_id=$1 ORDER BY starts_at,id").bind(target_term_id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        cycle_dates,
        vec![(offering_date, offering_date + chrono::Duration::days(183))]
    );
    let source_after: serde_json::Value = sqlx::query_scalar(source_facts_sql)
        .bind(source_term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        source_after, source_before,
        "preparation retains exact source graph and actual dated facts"
    );

    let different_actor = ActorContext {
        user_id: sqlx::query_scalar("SELECT id FROM users WHERE id<>$1 ORDER BY id LIMIT 1")
            .bind(actor.user_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        permissions: actor.permissions.clone(),
    };
    let different_actor_error = apply(&pool, &different_actor, request).await.unwrap_err();
    assert!(matches!(
        different_actor_error,
        school_errors::AppError::Conflict(ref message) if message.contains("requestId")
    ));

    let target_drafts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM course_assessment_plans WHERE academic_term_id=$1),
               (SELECT count(*) FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='draft'),
               (SELECT count(*) FROM academic_exam_rounds WHERE academic_term_id=$1 AND status='draft'),
               (SELECT count(*) FROM supervision_cycles WHERE academic_term_id=$1 AND status='draft'),
               (SELECT count(*) FROM academic_term_preparation_runs WHERE target_academic_term_id=$1)"#,
    )
    .bind(target_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(target_drafts.4, 2);
    assert!(target_drafts.0 > 0);
    assert!(target_drafts.1 > 0);
    assert!(target_drafts.2 > 0);
    assert!(target_drafts.3 > 0);
    let forbidden_copies: (i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM learning_group_students student
                JOIN learning_groups learning_group ON learning_group.id=student.learning_group_id
                WHERE learning_group.academic_term_id=$1),
               (SELECT count(*) FROM learning_group_student_scores score
                WHERE score.academic_term_id=$1),
               (SELECT count(*) FROM academic_exam_day_invigilators invigilator
                JOIN academic_exam_days day ON day.id=invigilator.exam_day_id
                WHERE day.academic_term_id=$1),
               (SELECT count(*) FROM supervision_observations WHERE academic_term_id=$1)"#,
    )
    .bind(target_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(forbidden_copies, (0, 0, 0, 0));
}

#[tokio::test]
async fn preparation_rejects_stale_preview_and_rolls_every_draft_back_when_audit_fails() {
    let (pool, actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_atomic").await;
    let preview_input = PreviewTermPreparationInput {
        source_term_id,
        target_term_id,
        modules: vec![TermPreparationModule::Delivery],
        mappings: preparation_mappings(),
    };
    let stale_workspace = preview(&pool, &actor, preview_input.clone()).await.unwrap();
    sqlx::query("UPDATE academic_terms SET row_version=row_version+1 WHERE id=$1")
        .bind(target_term_id)
        .execute(&pool)
        .await
        .unwrap();
    let stale = apply(
        &pool,
        &actor,
        ApplyTermPreparationInput {
            request_id: Uuid::new_v4(),
            source_checksum: stale_workspace.source_checksum,
            source_term_id,
            target_term_id,
            modules: preview_input.modules.clone(),
            mappings: preview_input.mappings.clone(),
        },
    )
    .await;
    assert!(matches!(stale, Err(school_errors::AppError::Conflict(_))));

    let workspace = preview(&pool, &actor, preview_input.clone()).await.unwrap();
    sqlx::raw_sql(
        r#"CREATE FUNCTION fail_term_preparation_audit() RETURNS trigger AS $$
           BEGIN
               IF NEW.event_code='academic_term.prepared' THEN
                   RAISE EXCEPTION 'TERM_PREPARATION_AUDIT_FAILURE';
               END IF;
               RETURN NEW;
           END;
           $$ LANGUAGE plpgsql;
           CREATE TRIGGER fail_term_preparation_audit
           BEFORE INSERT ON academic_audit_events
           FOR EACH ROW EXECUTE FUNCTION fail_term_preparation_audit();"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let failed = apply(
        &pool,
        &actor,
        ApplyTermPreparationInput {
            request_id: Uuid::new_v4(),
            source_checksum: workspace.source_checksum,
            source_term_id,
            target_term_id,
            modules: preview_input.modules,
            mappings: preview_input.mappings,
        },
    )
    .await;
    assert!(failed.is_err());
    let target_writes: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1),
               (SELECT count(*) FROM learning_groups WHERE academic_term_id=$1),
               (SELECT count(*) FROM academic_term_preparation_runs WHERE target_academic_term_id=$1)"#,
    )
    .bind(target_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(target_writes, (0, 0, 0));
}

#[tokio::test]
async fn preparation_checksum_changes_when_selected_module_content_changes() {
    let (pool, actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_content_checksum").await;
    let input = PreviewTermPreparationInput {
        source_term_id,
        target_term_id,
        modules: vec![TermPreparationModule::Assessments],
        mappings: preparation_mappings(),
    };
    let before = preview(&pool, &actor, input.clone()).await.unwrap();
    sqlx::query(
        r#"UPDATE course_assessment_phases
           SET max_score=max_score+0.01
           WHERE id=(
               SELECT phase.id
               FROM course_assessment_phases phase
               JOIN course_assessment_plans plan ON plan.id=phase.plan_id
               WHERE plan.academic_term_id=$1
               ORDER BY phase.id
               LIMIT 1
           )"#,
    )
    .bind(source_term_id)
    .execute(&pool)
    .await
    .unwrap();
    let after = preview(&pool, &actor, input).await.unwrap();
    assert_ne!(before.source_checksum, after.source_checksum);
}

#[tokio::test]
async fn preparation_preview_requires_school_manage_permission() {
    let (pool, mut actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_permission").await;
    actor.permissions.clear();
    let denied = preview(
        &pool,
        &actor,
        PreviewTermPreparationInput {
            source_term_id,
            target_term_id,
            modules: vec![TermPreparationModule::Delivery],
            mappings: preparation_mappings(),
        },
    )
    .await;
    assert!(matches!(denied, Err(school_errors::AppError::Forbidden(_))));
}

async fn publication_preview(
    pool: &sqlx::PgPool,
    id: uuid::Uuid,
) -> Result<school_academic_delivery::models::AcademicTermChangeSetPreview, school_errors::AppError>
{
    let revision =
        school_academic_delivery::services::change_sets::get_change_set(pool, id).await?;
    school_academic_delivery::services::change_sets::preview_change_set_at(
        &crate::adapters::publication_history::PUBLICATION_HISTORY,
        pool,
        id,
        Some(revision.reference_date),
    )
    .await
}

#[tokio::test]
async fn delivery_preparation_preview_preserves_managed_one_connection_read_only_snapshot() {
    use school_database::DbSession;
    use school_tenancy::{PoolManager, PoolManagerConfig, TenantPool};
    use std::time::Duration;

    let (fixture, _, _, target_term_id) =
        delivery_preparation_fixture("term_preparation_managed_read_only").await;
    // Managed pools enforce the current verified schema, unlike historical cutover fixtures.
    school_test_db::run_test_migrations(&fixture).await;
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&fixture)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema},public"));
    fixture.close().await;
    let manager = PoolManager::with_config(PoolManagerConfig::for_tests(1, 1, 1));
    let pool = manager
        .get_pool(url.as_str(), "preparation-read-only")
        .await
        .unwrap();
    assert!(TenantPool::retain_for_pool(&pool).unwrap().is_some());

    let mut tx = TenantPool::begin_observed(&pool, "test.preparation_read_only")
        .await
        .unwrap();
    DbSession::new(&mut tx)
        .query(
            "test.preparation_snapshot",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await
        .unwrap();
    let preview = tokio::time::timeout(
        Duration::from_secs(5),
        offerings::preview_term_preparation(
            &mut tx,
            target_term_id,
            preparation_mappings().offering_starts_on.unwrap(),
        ),
    )
    .await
    .expect("preview must reuse the transaction's only connection")
    .unwrap();
    assert_eq!(preview.academic_term_id, target_term_id);
    assert!(!preview.proposals.is_empty());
    let state: (String, String, i32) = DbSession::new(&mut tx)
        .query_as("test.preparation_snapshot_usable", "SELECT current_setting('transaction_read_only'),current_setting('transaction_isolation'),1::integer")
        .fetch_one().await.unwrap();
    assert_eq!(state, ("on".into(), "repeatable read".into(), 1));
    tx.rollback().await.unwrap();

    let mut connection = tokio::time::timeout(
        Duration::from_secs(2),
        TenantPool::observed_checkout_pool(&pool, "test.preparation_after_rollback"),
    )
    .await
    .expect("rollback must release the only connection")
    .unwrap();
    let available: i32 = DbSession::new(&mut connection)
        .scalar("test.preparation_resource_available", "SELECT 1::integer")
        .fetch_one()
        .await
        .unwrap();
    assert_eq!(available, 1);
    drop(connection);
    pool.close().await;
}

#[tokio::test]
async fn preparation_database_failures_preserve_cause_and_release_managed_connection() {
    use school_database::DbSession;
    use school_tenancy::{PoolManager, PoolManagerConfig, TenantPool};
    use std::time::Duration;

    let (fixture, actor, source_term_id, target_term_id) =
        delivery_preparation_fixture("term_preparation_database_failure").await;
    // Managed pools enforce the current verified schema, unlike historical cutover fixtures.
    school_test_db::run_test_migrations(&fixture).await;
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&fixture)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema},public"));
    fixture.close().await;
    let manager = PoolManager::with_config(PoolManagerConfig::for_tests(1, 1, 1));
    let pool = manager
        .get_pool(url.as_str(), "preparation-database-failure")
        .await
        .unwrap();
    assert!(TenantPool::retain_for_pool(&pool).unwrap().is_some());

    // Verify the managed pool first, then fault only this disposable schema.
    let mut fault = TenantPool::observed_checkout_pool(&pool, "test.preparation_inject_fault")
        .await
        .unwrap();
    DbSession::new(&mut fault)
        .query(
            "test.preparation_missing_view",
            "ALTER VIEW published_study_programs RENAME TO preparation_fault_study_programs",
        )
        .execute()
        .await
        .unwrap();
    drop(fault);

    // Delivery uses the finding catch; Assessments uses the optional Delivery fallback.
    for module in [
        TermPreparationModule::Delivery,
        TermPreparationModule::Assessments,
    ] {
        let error = tokio::time::timeout(
            Duration::from_secs(5),
            preview(
                &pool,
                &actor,
                PreviewTermPreparationInput {
                    source_term_id,
                    target_term_id,
                    modules: vec![module],
                    mappings: preparation_mappings(),
                },
            ),
        )
        .await
        .expect("database failure must release its transaction")
        .unwrap_err();
        let school_errors::AppError::DbError(sqlx::Error::Database(error)) = error else {
            panic!("preview must preserve the original typed database error");
        };
        assert_eq!(error.code().as_deref(), Some("42P01"));

        let mut connection = tokio::time::timeout(
            Duration::from_secs(2),
            TenantPool::observed_checkout_pool(&pool, "test.preparation_after_error"),
        )
        .await
        .expect("failed preview must release the only connection")
        .unwrap();
        let available: i32 = DbSession::new(&mut connection)
            .scalar(
                "test.preparation_failure_resource_available",
                "SELECT 1::integer",
            )
            .fetch_one()
            .await
            .unwrap();
        assert_eq!(available, 1);
        drop(connection);
    }
    pool.close().await;
}

#[tokio::test]
async fn legacy_offering_receipts_replay_unbound_dates_after_publication() {
    use school_test_db::academic_context_receipt_fixture::{
        legacy_offering_receipt_facts, retime_legacy_created_offerings,
        seed_legacy_curriculum_receipt, seed_legacy_preparation_receipt, ACTOR,
    };
    for preparation in [false, true] {
        for created in [1, 2] {
            let pool = core::services_tests::prepare_core_fixture_through(
                &format!("legacy_date_replay_{preparation}_{created}"),
                145,
            )
            .await;
            let actor = ActorContext {
                user_id: ACTOR,
                permissions: vec![codes::WILDCARD.into()],
            };
            if preparation {
                let (nonce, _, _, checksum) = seed_legacy_preparation_receipt(&pool, created).await;
                let (source,target,source_checksum,mappings,original_outcome):(Uuid,Uuid,String,serde_json::Value,serde_json::Value)=sqlx::query_as("SELECT source_academic_term_id,target_academic_term_id,source_checksum,mappings,outcome FROM academic_term_preparation_runs WHERE request_id=$1").bind(nonce).fetch_one(&pool).await.unwrap();
                let dates = retime_legacy_created_offerings(&pool, nonce, true).await;
                assert_eq!(dates.len(), created);
                school_test_db::run_test_migrations(&pool).await;
                let before = legacy_offering_receipt_facts(&pool, target).await;
                let original_mappings: TermPreparationMappings =
                    serde_json::from_value(mappings).unwrap();
                for year in [2030, 2040] {
                    let mut mappings = original_mappings.clone();
                    mappings.offering_starts_on = chrono::NaiveDate::from_ymd_opt(year, 2, 1);
                    let replay = apply(
                        &pool,
                        &actor,
                        ApplyTermPreparationInput {
                            request_id: nonce,
                            source_checksum: source_checksum.clone(),
                            source_term_id: source,
                            target_term_id: target,
                            modules: vec![TermPreparationModule::Delivery],
                            mappings,
                        },
                    )
                    .await
                    .unwrap();
                    assert_eq!(
                        serde_json::to_value(&replay.modules).unwrap(),
                        original_outcome
                    );
                    assert_eq!(legacy_offering_receipt_facts(&pool, target).await, before);
                }
                let stored:(Option<chrono::NaiveDate>,String)=sqlx::query_as("SELECT offering_starts_on,request_checksum FROM academic_term_preparation_runs WHERE request_id=$1").bind(nonce).fetch_one(&pool).await.unwrap();
                assert_eq!(stored, (None, checksum));
            } else {
                let receipt = seed_legacy_curriculum_receipt(&pool, created).await;
                let original_ids:Vec<Uuid>=sqlx::query_scalar("SELECT offering_ids FROM learning_delivery_apply_runs WHERE idempotency_key=$1").bind(receipt.request_id).fetch_one(&pool).await.unwrap();
                let dates = retime_legacy_created_offerings(&pool, receipt.request_id, false).await;
                assert_eq!(dates.len(), created);
                school_test_db::run_test_migrations(&pool).await;
                let before = legacy_offering_receipt_facts(&pool, receipt.target_term_id).await;
                for year in [2030, 2040] {
                    let mut body = receipt.request_body.clone();
                    body.as_object_mut().unwrap().insert(
                        "startsOn".into(),
                        serde_json::json!(chrono::NaiveDate::from_ymd_opt(year, 2, 1).unwrap()),
                    );
                    let request: school_academic_delivery::models::ApplyCurriculumOfferingsRequest =
                        serde_json::from_value(body).unwrap();
                    let replay = offerings::apply_from_curriculum(&pool, ACTOR, request)
                        .await
                        .unwrap();
                    assert_eq!(replay.offering_ids, original_ids);
                    assert_eq!(replay.created_offering_count, created);
                    assert_eq!(
                        legacy_offering_receipt_facts(&pool, receipt.target_term_id).await,
                        before
                    );
                }
                let stored:(Option<chrono::NaiveDate>,String)=sqlx::query_as("SELECT starts_on,request_hash::text FROM learning_delivery_apply_runs WHERE idempotency_key=$1").bind(receipt.request_id).fetch_one(&pool).await.unwrap();
                assert_eq!(stored, (None, receipt.request_hash));
            }
        }
    }
}
