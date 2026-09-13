use std::collections::HashSet;

use uuid::Uuid;

use super::term_preparation::{apply, normalize_input, preview, validate_checksum};
use crate::{
    middleware::permission::ActorContext,
    modules::academic::{
        core,
        cutover_test_support::apply_migrations_through,
        delivery::services::offerings,
        lifecycle::models::{
            ApplyTermPreparationInput, PreviewTermPreparationInput, TermPreparationEntityMapping,
            TermPreparationMappingKind, TermPreparationMappings, TermPreparationModule,
        },
    },
    permissions::registry::codes,
};

#[test]
fn preparation_input_requires_unique_modules_and_bounded_unambiguous_mappings() {
    assert!(normalize_input(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &[TermPreparationModule::Delivery],
        &TermPreparationMappings::default(),
    )
    .is_ok());

    assert!(normalize_input(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &[],
        &TermPreparationMappings::default(),
    )
    .is_err());
    assert!(normalize_input(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &[
            TermPreparationModule::Delivery,
            TermPreparationModule::Delivery,
        ],
        &TermPreparationMappings::default(),
    )
    .is_err());

    let source = Uuid::new_v4();
    let target_a = Uuid::new_v4();
    let target_b = Uuid::new_v4();
    let mappings = TermPreparationMappings {
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
    apply_migrations_through(&pool, 79).await.unwrap();
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
        "SELECT id,academic_year_id FROM academic_terms WHERE status='active' ORDER BY start_date LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (target_term_id, target_year_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT term.id,term.academic_year_id
           FROM academic_terms term
           JOIN academic_years year ON year.id=term.academic_year_id
           WHERE term.status='planning'
             AND year.start_date > (SELECT start_date FROM academic_years WHERE id=$1)
           ORDER BY year.start_date,term.sequence_no
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
    sqlx::query(
        "UPDATE academic_terms SET status='closed',closed_on=COALESCE(planned_end_date,start_date),row_version=row_version+1 WHERE id=$1",
    )
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
    let (
        source_start_date,
        target_start_date,
        target_end_date,
        target_year_id,
        target_year_start_date,
    ): (
        chrono::NaiveDate,
        chrono::NaiveDate,
        chrono::NaiveDate,
        Uuid,
        chrono::NaiveDate,
    ) = sqlx::query_as(
        r#"SELECT source.start_date,target.start_date,target.planned_end_date,
                  target.academic_year_id,target_year.start_date
           FROM academic_terms source
           CROSS JOIN academic_terms target
           JOIN academic_years target_year ON target_year.id=target.academic_year_id
           WHERE source.id=$1 AND target.id=$2"#,
    )
    .bind(source_term_id)
    .bind(target_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let request = PreviewTermPreparationInput {
        source_term_id,
        target_term_id,
        modules: vec![TermPreparationModule::Delivery],
        mappings: TermPreparationMappings::default(),
    };

    sqlx::query("UPDATE academic_years SET start_date=$2 WHERE id=$1")
        .bind(target_year_id)
        .bind(source_start_date)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET start_date=$2,planned_end_date=$2 WHERE id=$1")
        .bind(target_term_id)
        .bind(source_start_date)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        preview(&pool, &actor, request.clone()).await,
        Err(crate::error::AppError::Conflict(_))
    ));

    sqlx::query("UPDATE academic_terms SET start_date=$2,planned_end_date=$3 WHERE id=$1")
        .bind(target_term_id)
        .bind(target_start_date)
        .bind(target_end_date)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET start_date=$2 WHERE id=$1")
        .bind(target_year_id)
        .bind(target_year_start_date)
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
        Err(crate::error::AppError::Conflict(_))
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
    let request = PreviewTermPreparationInput {
        source_term_id,
        target_term_id,
        modules: vec![TermPreparationModule::Delivery],
        mappings: TermPreparationMappings::default(),
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

    let outcome = apply(
        &pool,
        &actor,
        ApplyTermPreparationInput {
            request_id: Uuid::new_v4(),
            source_checksum: workspace.source_checksum,
            source_term_id,
            target_term_id,
            modules: request.modules,
            mappings: request.mappings,
        },
    )
    .await
    .unwrap();
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
    let modules = TermPreparationModule::ALL.to_vec();
    let initial_workspace = preview(
        &pool,
        &actor,
        PreviewTermPreparationInput {
            source_term_id,
            target_term_id,
            modules: modules.clone(),
            mappings: TermPreparationMappings::default(),
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
    let mut tx = pool.begin().await.unwrap();
    let delivery_preview = offerings::preview_term_preparation(&mut tx, target_term_id)
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
        entities: entity_mappings,
        dates: Vec::new(),
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
        source_checksum: workspace.source_checksum,
        source_term_id,
        target_term_id,
        modules,
        mappings,
    };
    let outcome = apply(&pool, &actor, request.clone()).await.unwrap();
    let replay = apply(&pool, &actor, request.clone()).await.unwrap();
    assert_eq!(replay.run_id, outcome.run_id);
    assert_eq!(outcome.modules.len(), TermPreparationModule::ALL.len());

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
        crate::error::AppError::Conflict(ref message) if message.contains("requestId")
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
    assert_eq!(target_drafts.4, 1);
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
        mappings: TermPreparationMappings::default(),
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
    assert!(matches!(stale, Err(crate::error::AppError::Conflict(_))));

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
        mappings: TermPreparationMappings::default(),
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
            mappings: TermPreparationMappings::default(),
        },
    )
    .await;
    assert!(matches!(denied, Err(crate::error::AppError::Forbidden(_))));
}
