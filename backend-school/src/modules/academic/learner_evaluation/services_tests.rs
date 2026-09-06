use super::{models::*, services::*};
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use crate::test_helpers::create_named_test_pool_with_max_connections;
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};
use uuid::Uuid;
const DC: LearnerEvaluationDomain = LearnerEvaluationDomain::DesirableCharacteristic;
const RTW: LearnerEvaluationDomain = LearnerEvaluationDomain::ReadingThinkingWriting;

async fn fixture(name: &str) -> (sqlx::PgPool, ActorContext, EvaluationContext, Uuid, Uuid) {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 61).await.unwrap();
    let (year,term,group,subject,teacher):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT g.academic_year_id,g.academic_term_id,g.id,d.subject_id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN course_assessment_plans p ON p.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1").fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE course_assessment_plans SET assessment_coordinator_id=$1 WHERE learning_offering_id=(SELECT learning_offering_id FROM learning_groups WHERE id=$2)").bind(teacher).bind(group).execute(&pool).await.unwrap();
    let actor = ActorContext {
        user_id: teacher,
        permissions: vec![
            codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL.into(),
        ],
    };
    (
        pool,
        actor,
        EvaluationContext {
            academic_year_id: year,
            academic_term_id: term,
        },
        group,
        subject,
    )
}
fn criterion(name: &str, version: Option<i64>) -> CriterionInput {
    CriterionInput {
        name: name.into(),
        active: true,
        display_order: 0,
        row_version: version,
    }
}
fn confirmation(ws: &EvaluationWorkspace) -> ConfirmationInput {
    ConfirmationInput {
        source_checksum: ws.source_checksum.clone(),
        roster_checksum: ws.roster_checksum.clone(),
        row_version: ws.confirmation.as_ref().map(|c| c.row_version),
    }
}
async fn fill(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
    group: Uuid,
    domain: LearnerEvaluationDomain,
    level: i16,
) -> EvaluationWorkspace {
    let ws = get_workspace(pool, actor, group, domain, ctx)
        .await
        .unwrap();
    let cells = ws
        .students
        .iter()
        .flat_map(|s| {
            ws.criteria
                .iter()
                .filter(|c| c.lifecycle == "active")
                .map(move |c| ResponseInput {
                    subject_term_criterion_id: c.id,
                    student_academic_year_id: s.student_academic_year_id,
                    quality_level: Some(level.try_into().unwrap()),
                    row_version: None,
                })
        })
        .collect();
    save_responses(pool, actor, group, domain, ctx, cells)
        .await
        .unwrap()
}

// Catches accepting invalid levels at the HTTP boundary and coercing null to zero.
#[test]
fn learner_level_wire_validation() {
    for valid in 0..=3 {
        assert_eq!(
            i16::from(serde_json::from_str::<LearnerEvaluationLevel>(&valid.to_string()).unwrap()),
            valid
        );
    }
    for invalid in ["-1", "4", "1.5", "null", "\"2\""] {
        assert!(serde_json::from_str::<LearnerEvaluationLevel>(invalid).is_err());
    }
    assert!(serde_json::from_str::<LearnerEvaluationDomain>("\"other\"").is_err());
}

fn locked_value(subject: Uuid, catalog: Option<Uuid>, level: i16) -> LockedCriterionValue {
    LockedCriterionValue {
        id: Uuid::new_v4(),
        subject_id: subject,
        domain: DC,
        subject_term_criterion_id: Uuid::new_v4(),
        school_criterion_id: catalog,
        name: "Criterion".into(),
        quality_level: level,
        row_version: 1,
    }
}
fn bands() -> Vec<(i16, String)> {
    vec![
        (0, "0.00".into()),
        (1, "1.00".into()),
        (2, "1.50".into()),
        (3, "2.50".into()),
    ]
}

// Catches accidentally using the response-entry switch as a configuration authorization gate.
#[tokio::test]
async fn learner_coordinator_configuration_remains_editable_while_entry_closed() {
    let (pool, manager, ctx, group, subject) = fixture("learner_coordinator_closed").await;
    let teacher = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into()],
    };
    let config = get_configuration(&pool, &teacher, subject, DC, &ctx)
        .await
        .unwrap();
    assert!(config.can_manage);
    let added = save_criterion(
        &pool,
        &teacher,
        subject,
        DC,
        &ctx,
        None,
        criterion("Subject-only", None),
    )
    .await
    .unwrap();
    assert!(added.school_criterion_id.is_none());
    assert!(
        !get_workspace(&pool, &teacher, group, DC, &ctx)
            .await
            .unwrap()
            .can_manage
    );
    sqlx::query("UPDATE course_assessment_plans SET assessment_coordinator_id=NULL WHERE learning_offering_id=(SELECT learning_offering_id FROM learning_groups WHERE id=$1)").bind(group).execute(&pool).await.unwrap();
    assert!(matches!(
        save_criterion(
            &pool,
            &teacher,
            subject,
            DC,
            &ctx,
            None,
            criterion("Denied", None)
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    let reader = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into()],
    };
    assert!(list_subjects(&pool, &reader, &ctx)
        .await
        .unwrap()
        .iter()
        .any(|s| s.subject_id == subject));
    assert!(matches!(
        save_criterion(
            &pool,
            &reader,
            subject,
            DC,
            &ctx,
            None,
            criterion("Denied", None)
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        lock_subject(&pool, &reader, subject, DC, &ctx).await,
        Err(AppError::Forbidden(_))
    ));
}

// Catches copying non-applicable catalog criteria and deleting catalog identities used by subjects.
#[tokio::test]
async fn learner_catalog_applicability_and_referenced_removal() {
    let (pool, actor, ctx, _, subject) = fixture("learner_catalog_applicability").await;
    let matching:String=sqlx::query_scalar("SELECT DISTINCT level.level_type FROM learning_offering_targets t JOIN course_offering_details d ON d.learning_offering_id=t.learning_offering_id JOIN grade_levels level ON level.id=t.grade_level_id WHERE d.subject_id=$1 AND d.academic_term_id=$2 ORDER BY level.level_type LIMIT 1").bind(subject).bind(ctx.academic_term_id).fetch_one(&pool).await.unwrap();
    let other = if matching == "primary" {
        "secondary"
    } else {
        "primary"
    };
    let included = save_catalog(
        &pool,
        &actor,
        &ctx,
        None,
        CatalogInput {
            domain: DC,
            applicability: matching,
            criterion: criterion("Matching level", None),
        },
    )
    .await
    .unwrap();
    let excluded = save_catalog(
        &pool,
        &actor,
        &ctx,
        None,
        CatalogInput {
            domain: DC,
            applicability: other.into(),
            criterion: criterion("Other level", None),
        },
    )
    .await
    .unwrap();
    let config = get_configuration(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap();
    assert!(config
        .criteria
        .iter()
        .any(|c| c.school_criterion_id == Some(included.id)));
    assert!(!config
        .criteria
        .iter()
        .any(|c| c.school_criterion_id == Some(excluded.id)));
    remove_catalog(&pool, &actor, &ctx, included.id, included.row_version)
        .await
        .unwrap();
    remove_catalog(&pool, &actor, &ctx, excluded.id, excluded.row_version)
        .await
        .unwrap();
    let catalog = list_catalog(&pool, &actor, &ctx).await.unwrap();
    assert_eq!(
        catalog
            .iter()
            .find(|c| c.id == included.id)
            .unwrap()
            .lifecycle,
        "inactive"
    );
    assert!(!catalog.iter().any(|c| c.id == excluded.id));
    assert_eq!(
        get_configuration(&pool, &actor, subject, DC, &ctx)
            .await
            .unwrap()
            .criteria
            .iter()
            .find(|c| c.school_criterion_id == Some(included.id))
            .unwrap()
            .lifecycle,
        "active"
    );
}

// Catches losing an existing configuration or accepting a cross-year header during the forward migration.
#[tokio::test]
async fn learner_061_backfills_existing_configuration_and_enforces_context() {
    let pool = create_named_test_pool_with_max_connections("learner_061_backfill", 2).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 60).await.unwrap();
    let (subject,term,year):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT subject_id,academic_term_id,academic_year_id FROM course_offering_details ORDER BY learning_offering_id LIMIT 1").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO subject_term_evaluation_criteria (subject_id,academic_term_id,academic_year_id,domain,name,row_version) VALUES ($1,$2,$3,'desirable_characteristic','Existing',7)").bind(subject).bind(term).bind(year).execute(&pool).await.unwrap();
    apply_migrations_through(&pool, 61).await.unwrap();
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT row_version FROM subject_term_evaluation_configurations WHERE subject_id=$1 AND academic_term_id=$2 AND domain='desirable_characteristic'").bind(subject).bind(term).fetch_one(&pool).await.unwrap(),7);
    assert!(sqlx::query("INSERT INTO subject_term_evaluation_configurations (subject_id,academic_term_id,academic_year_id,domain) VALUES ($1,$2,$3,'reading_thinking_writing')").bind(subject).bind(term).bind(Uuid::new_v4()).execute(&pool).await.is_err());
}

// Catches accepting a former primary's retained confirmation or using manager status as a primary role.
#[tokio::test]
async fn learner_current_primary_is_revalidated_for_confirmation_and_lock() {
    let (pool, actor, ctx, group, subject) = fixture("learner_current_primary").await;
    let ws = fill(&pool, &actor, &ctx, group, DC, 1).await;
    let confirmed = confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws))
        .await
        .unwrap()
        .confirmation
        .unwrap();
    sqlx::query("UPDATE learning_groups SET status='draft' WHERE id=$1")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_group_teachers SET role='secondary' WHERE learning_group_id=$1 AND teacher_id=$2").bind(group).bind(actor.user_id).execute(&pool).await.unwrap();
    let ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    assert!(ws.can_manage);
    assert!(!ws.can_confirm);
    assert!(!ws.confirmation_is_current);
    assert!(ws.confirmation.as_ref().unwrap().row_version > confirmed.row_version);
    assert!(matches!(
        confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws)).await,
        Err(AppError::Forbidden(_))
    ));
    let outcome = lock_subject(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap();
    assert!(outcome.lock.is_none());
    assert!(outcome
        .blockers
        .iter()
        .any(|b| b.learning_group_id == group && b.reason == "stale_group_confirmation"));
    let teacher = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into()],
    };
    sqlx::query("UPDATE learning_group_teachers SET starts_on=(SELECT start_date-10 FROM academic_terms WHERE id=$3),ends_on=(SELECT start_date-1 FROM academic_terms WHERE id=$3) WHERE learning_group_id=$1 AND teacher_id=$2").bind(group).bind(actor.user_id).bind(ctx.academic_term_id).execute(&pool).await.unwrap();
    assert!(matches!(
        get_workspace(&pool, &teacher, group, DC, &ctx).await,
        Err(AppError::Forbidden(_))
    ));
}

// Catches cell-weighted averaging, label-based catalog merging, rounded threshold comparison,
// and caching a derived summary instead of recalculating supplied effective values.
#[test]
fn learner_summary_equal_subject_weighting_thresholds_and_effective_value_recalculation() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let missing = Uuid::new_v4();
    let catalog = Uuid::new_v4();
    let mut rows = vec![
        locked_value(a, Some(catalog), 1),
        locked_value(a, None, 1),
        locked_value(a, None, 0),
        locked_value(b, Some(catalog), 3),
    ];
    let result = summarize_domains(&rows, &[a, b, missing], &[(a, DC), (b, DC)], &bands()).unwrap();
    assert_eq!(result.len(), 2);
    let dc = &result[0];
    let average = dc.average.as_ref().unwrap();
    assert_eq!((&*average.numerator, &*average.denominator), ("11", "6"));
    assert_eq!(dc.quality_level, Some(2));
    assert!(!dc.complete);
    assert_eq!(dc.missing_subjects.len(), 1);
    assert_eq!(dc.missing_subjects[0].subject_id, missing);
    assert_eq!(dc.catalog_criteria.len(), 1);
    assert_eq!(dc.catalog_criteria[0].average.decimal, "2");
    rows[3].quality_level = 0;
    let recalculated = summarize_domains(&rows, &[a, b], &[(a, DC), (b, DC)], &bands()).unwrap();
    assert_eq!(recalculated[0].average.as_ref().unwrap().numerator, "1");
    assert_eq!(recalculated[0].average.as_ref().unwrap().denominator, "3");
    assert_eq!(recalculated[0].quality_level, Some(0));
    assert!(recalculated[0].complete);
    for (values, want) in [
        (vec![0], 0),
        (vec![0, 1, 1], 0),
        (vec![1], 1),
        (vec![1, 1, 2], 1),
        (vec![1, 2], 2),
        (vec![2, 2, 3], 2),
        (vec![2, 3], 3),
        (vec![3], 3),
    ] {
        let rows = values
            .into_iter()
            .map(|v| locked_value(a, None, v))
            .collect::<Vec<_>>();
        assert_eq!(
            summarize_domains(&rows, &[a], &[(a, DC)], &bands()).unwrap()[0].quality_level,
            Some(want)
        );
    }
}

// Catches recopying catalog revisions, including after every unused column is removed.
#[tokio::test]
async fn learner_config_copies_once_and_empty_initialization_persists() {
    let (pool, actor, ctx, _, subject) = fixture("learner_copy_once").await;
    let first = get_configuration(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap();
    assert_eq!(first.criteria.len(), 8);
    let catalog_id = first.criteria[0].school_criterion_id.unwrap();
    save_catalog(
        &pool,
        &actor,
        &ctx,
        Some(catalog_id),
        CatalogInput {
            domain: DC,
            applicability: "all".into(),
            criterion: criterion("Changed school label", Some(1)),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        get_configuration(&pool, &actor, subject, DC, &ctx)
            .await
            .unwrap()
            .criteria[0]
            .name,
        first.criteria[0].name
    );
    for c in first.criteria {
        assert!(
            remove_criterion(&pool, &actor, subject, DC, &ctx, c.id, c.row_version)
                .await
                .unwrap()
                .deleted
        );
    }
    assert!(get_configuration(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap()
        .criteria
        .is_empty());
    sqlx::query("UPDATE academic_learner_evaluation_criteria SET lifecycle='inactive' WHERE domain='reading_thinking_writing'").execute(&pool).await.unwrap();
    assert!(get_configuration(&pool, &actor, subject, RTW, &ctx)
        .await
        .unwrap()
        .criteria
        .is_empty());
    save_catalog(
        &pool,
        &actor,
        &ctx,
        None,
        CatalogInput {
            domain: RTW,
            applicability: "all".into(),
            criterion: criterion("Added later", None),
        },
    )
    .await
    .unwrap();
    assert!(get_configuration(&pool, &actor, subject, RTW, &ctx)
        .await
        .unwrap()
        .criteria
        .is_empty());
}

// Catches leaking window or authorization across domains, stale controls, and batch partial writes.
#[tokio::test]
async fn learner_windows_authorization_and_batch_atomicity() {
    let (pool, actor, ctx, group, subject) = fixture("learner_windows").await;
    let teacher = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into()],
    };
    let ws = get_workspace(&pool, &teacher, group, DC, &ctx)
        .await
        .unwrap();
    assert!(!ws.can_manage);
    let input = ResponseInput {
        subject_term_criterion_id: ws.criteria[0].id,
        student_academic_year_id: ws.students[0].student_academic_year_id,
        quality_level: Some(0.try_into().unwrap()),
        row_version: None,
    };
    assert!(matches!(
        save_responses(&pool, &teacher, group, DC, &ctx, vec![input.clone()]).await,
        Err(AppError::Forbidden(_))
    ));
    let controls = list_controls(&pool, &teacher, &ctx).await.unwrap();
    let control = controls.iter().find(|c| c.domain == DC).unwrap();
    update_control(
        &pool,
        &actor,
        DC,
        &ctx,
        ControlInput {
            entry_enabled: true,
            row_version: control.row_version,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        update_control(
            &pool,
            &actor,
            DC,
            &ctx,
            ControlInput {
                entry_enabled: false,
                row_version: control.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        !get_workspace(&pool, &teacher, group, RTW, &ctx)
            .await
            .unwrap()
            .can_manage
    );
    let invalid = ResponseInput {
        student_academic_year_id: Uuid::new_v4(),
        ..input.clone()
    };
    assert!(save_responses(
        &pool,
        &teacher,
        group,
        DC,
        &ctx,
        vec![input.clone(), invalid]
    )
    .await
    .is_err());
    assert!(get_workspace(&pool, &teacher, group, DC, &ctx)
        .await
        .unwrap()
        .responses
        .is_empty());
    let saved = save_responses(&pool, &teacher, group, DC, &ctx, vec![input.clone()])
        .await
        .unwrap();
    assert_eq!(saved.responses[0].quality_level, 0);
    assert!(matches!(
        save_responses(&pool, &teacher, group, DC, &ctx, vec![input.clone()]).await,
        Err(AppError::Conflict(_))
    ));
    let cleared = save_responses(
        &pool,
        &teacher,
        group,
        DC,
        &ctx,
        vec![ResponseInput {
            quality_level: None,
            row_version: Some(saved.responses[0].row_version),
            ..input
        }],
    )
    .await
    .unwrap();
    assert!(cleared.responses.is_empty());
    let reinserted = save_responses(
        &pool,
        &teacher,
        group,
        DC,
        &ctx,
        vec![ResponseInput {
            subject_term_criterion_id: ws.criteria[0].id,
            student_academic_year_id: ws.students[0].student_academic_year_id,
            quality_level: Some(2.try_into().unwrap()),
            row_version: None,
        }],
    )
    .await
    .unwrap();
    assert!(reinserted.responses[0].row_version > saved.responses[0].row_version);
    let outsider = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: teacher.permissions.clone(),
    };
    assert!(matches!(
        get_configuration(&pool, &outsider, subject, DC, &ctx).await,
        Err(AppError::Forbidden(_))
    ));
    let wrong = EvaluationContext {
        academic_year_id: Uuid::new_v4(),
        ..ctx.clone()
    };
    assert!(matches!(
        get_workspace(&pool, &actor, group, DC, &wrong).await,
        Err(AppError::ValidationError(_))
    ));
}

// Catches blank-as-zero confirmation, cosmetic invalidation, deleting response history, and version reset.
#[tokio::test]
async fn learner_confirmation_completeness_lifecycle_and_monotonic_versions() {
    let (pool, actor, ctx, group, subject) = fixture("learner_confirmation").await;
    let ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    let outcome = confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws))
        .await
        .unwrap();
    assert!(outcome.confirmation.is_none());
    assert_eq!(outcome.missing.len(), ws.students.len() * 8);
    let ws = fill(&pool, &actor, &ctx, group, DC, 0).await;
    assert!(
        confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws))
            .await
            .unwrap()
            .confirmation
            .is_some()
    );
    let c = &ws.criteria[0];
    let renamed = save_criterion(
        &pool,
        &actor,
        subject,
        DC,
        &ctx,
        Some(c.id),
        criterion("Cosmetic", Some(c.row_version)),
    )
    .await
    .unwrap();
    let renamed_ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    assert!(renamed_ws.confirmation_is_current);
    let old_version = renamed_ws.confirmation.unwrap().row_version;
    let removed = remove_criterion(&pool, &actor, subject, DC, &ctx, c.id, renamed.row_version)
        .await
        .unwrap();
    assert!(!removed.deleted);
    assert_eq!(removed.criterion.unwrap().lifecycle, "inactive");
    let ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    assert!(!ws.confirmation_is_current);
    assert!(ws.confirmation.as_ref().unwrap().invalidated);
    assert!(ws.confirmation.as_ref().unwrap().row_version > old_version);
    let confirm = confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws))
        .await
        .unwrap()
        .confirmation
        .unwrap();
    assert!(confirm.row_version > ws.confirmation.unwrap().row_version);
    let ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    sqlx::query(
        "UPDATE learning_group_students SET row_version=row_version+1 WHERE learning_group_id=$1",
    )
    .bind(group)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        !get_workspace(&pool, &actor, group, DC, &ctx)
            .await
            .unwrap()
            .confirmation_is_current
    );
    assert!(matches!(
        confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws)).await,
        Err(AppError::Conflict(_))
    ));
}

// Catches locking only a ready room, mutable official rows, and coupling the two domain locks.
#[tokio::test]
async fn learner_all_room_lock_is_atomic_and_domains_are_independent() {
    let (pool, actor, ctx, group, subject) = fixture("learner_lock").await;
    let other:Uuid=sqlx::query_scalar("INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT uuid_generate_v4(),learning_offering_id,academic_term_id,academic_year_id,'SECOND','Second room','draft','draft' FROM learning_groups WHERE id=$1 RETURNING id").bind(group).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers (id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on) SELECT uuid_generate_v4(),$1,academic_term_id,academic_year_id,teacher_id,role,starts_on FROM learning_group_teachers WHERE learning_group_id=$2").bind(other).bind(group).execute(&pool).await.unwrap();
    let ws = fill(&pool, &actor, &ctx, group, DC, 3).await;
    confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&ws))
        .await
        .unwrap();
    let denied = lock_subject(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap();
    assert!(denied.lock.is_none());
    assert!(denied.blockers.iter().any(|b| b.learning_group_id == other));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subject_term_evaluation_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let ws = get_workspace(&pool, &actor, other, DC, &ctx).await.unwrap();
    confirm_group(&pool, &actor, other, DC, &ctx, confirmation(&ws))
        .await
        .unwrap();
    // The populated cutover fixture can contain further rooms for the same subject.
    let remaining:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT g.id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id AND t.role='primary' WHERE d.subject_id=$1 AND g.academic_term_id=$2 AND g.status<>'closed' AND g.id<>$3 AND g.id<>$4 ORDER BY g.id").bind(subject).bind(ctx.academic_term_id).bind(group).bind(other).fetch_all(&pool).await.unwrap();
    for (room, teacher) in remaining {
        let room_actor = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        let mut ws = get_workspace(&pool, &room_actor, room, DC, &ctx)
            .await
            .unwrap();
        if !ws.students.is_empty() {
            ws = fill(&pool, &room_actor, &ctx, room, DC, 3).await;
        }
        confirm_group(&pool, &room_actor, room, DC, &ctx, confirmation(&ws))
            .await
            .unwrap();
    }
    let outcome = lock_subject(&pool, &actor, subject, DC, &ctx)
        .await
        .unwrap();
    assert!(
        outcome.blockers.is_empty(),
        "Unexpected blockers: {:?}",
        outcome.blockers
    );
    let locked = outcome.lock.unwrap();
    let locked_ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    let cell = &locked_ws.responses[0];
    assert!(matches!(
        save_responses(
            &pool,
            &actor,
            group,
            DC,
            &ctx,
            vec![ResponseInput {
                subject_term_criterion_id: cell.subject_term_criterion_id,
                student_academic_year_id: cell.student_academic_year_id,
                quality_level: Some(0.try_into().unwrap()),
                row_version: Some(cell.row_version)
            }]
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        confirm_group(&pool, &actor, group, DC, &ctx, confirmation(&locked_ws)).await,
        Err(AppError::Conflict(_))
    ));
    let other_domain = fill(&pool, &actor, &ctx, group, RTW, 1).await;
    assert!(other_domain.responses.iter().all(|r| r.quality_level == 1));
    assert!(
        get_workspace(&pool, &actor, group, DC, &ctx)
            .await
            .unwrap()
            .locked
    );
    assert!(
        get_workspace(&pool, &actor, group, RTW, &ctx)
            .await
            .unwrap()
            .can_manage
    );
    assert!(matches!(
        save_criterion(
            &pool,
            &actor,
            subject,
            DC,
            &ctx,
            None,
            criterion("Forbidden", None)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        sqlx::query("UPDATE subject_term_evaluation_locks SET row_version=2 WHERE id=$1")
            .bind(locked.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query(
        "UPDATE subject_term_student_evaluations SET quality_level=0 WHERE evaluation_lock_id=$1"
    )
    .bind(locked.id)
    .execute(&pool)
    .await
    .is_err());
    let student = get_workspace(&pool, &actor, group, DC, &ctx)
        .await
        .unwrap()
        .students[0]
        .student_academic_year_id;
    let summary = summarize_student_term(&pool, &ctx, student).await.unwrap();
    let dc = summary.domains.iter().find(|d| d.domain == DC).unwrap();
    assert_eq!(dc.quality_level, Some(3));
    assert_eq!(dc.subjects[0].criteria.len(), 8);
    let rtw = summary.domains.iter().find(|d| d.domain == RTW).unwrap();
    assert!(!rtw.complete);
    assert!(rtw
        .missing_subjects
        .iter()
        .any(|s| s.subject_id == subject && s.reason == "subject_domain_not_locked"));
}

// Catches a missing shared scope lock allowing both a response update and a stale official lock.
#[tokio::test]
async fn learner_response_and_initial_lock_serialize() {
    let (pool, actor, ctx, group, subject) = fixture("learner_lock_race").await;
    let rooms:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT g.id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id AND t.role='primary' WHERE d.subject_id=$1 AND g.academic_term_id=$2 AND g.status<>'closed' ORDER BY g.id").bind(subject).bind(ctx.academic_term_id).fetch_all(&pool).await.unwrap();
    for (room, teacher) in rooms {
        let room_actor = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        let mut ws = get_workspace(&pool, &room_actor, room, DC, &ctx)
            .await
            .unwrap();
        if !ws.students.is_empty() {
            ws = fill(&pool, &room_actor, &ctx, room, DC, 3).await;
        }
        confirm_group(&pool, &room_actor, room, DC, &ctx, confirmation(&ws))
            .await
            .unwrap();
    }
    let ws = get_workspace(&pool, &actor, group, DC, &ctx).await.unwrap();
    let cell = &ws.responses[0];
    let input = ResponseInput {
        subject_term_criterion_id: cell.subject_term_criterion_id,
        student_academic_year_id: cell.student_academic_year_id,
        quality_level: Some(0.try_into().unwrap()),
        row_version: Some(cell.row_version),
    };
    let (lock, write) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        tokio::join!(
            lock_subject(&pool, &actor, subject, DC, &ctx),
            save_responses(&pool, &actor, group, DC, &ctx, vec![input])
        )
    })
    .await
    .unwrap();
    let locked = lock.unwrap().lock.is_some();
    assert_ne!(
        locked,
        write.is_ok(),
        "Only one competing mutation may succeed"
    );
    let official:Vec<i16>=sqlx::query_scalar("SELECT quality_level FROM subject_term_student_evaluations WHERE subject_id=$1 AND academic_term_id=$2 AND domain='desirable_characteristic'").bind(subject).bind(ctx.academic_term_id).fetch_all(&pool).await.unwrap();
    if locked {
        assert!(matches!(write, Err(AppError::Conflict(_))));
        assert!(!official.is_empty());
        assert!(official.iter().all(|v| *v == 3));
    } else {
        assert!(official.is_empty());
    }
}

// Exercises the actual emitted OpenAPI operations, including required year/term context.
#[test]
fn learner_endpoint_contract_requires_full_context_and_typed_domains() {
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    #[openapi(paths(
        super::handlers::list_subjects,
        super::handlers::list_catalog,
        super::handlers::create_catalog,
        super::handlers::update_catalog,
        super::handlers::remove_catalog,
        super::handlers::list_controls,
        super::handlers::update_control,
        super::handlers::get_configuration,
        super::handlers::create_criterion,
        super::handlers::update_criterion,
        super::handlers::remove_criterion,
        super::handlers::get_workspace,
        super::handlers::save_responses,
        super::handlers::confirm_group,
        super::handlers::lock_subject,
        super::handlers::student_summary,
    ))]
    struct LearnerPaths;
    let contract = serde_json::to_value(LearnerPaths::openapi()).unwrap();
    let paths = contract["paths"].as_object().unwrap();
    for item in paths.values() {
        for operation in item.as_object().unwrap().values() {
            let parameters = operation["parameters"].as_array().unwrap();
            for name in ["academicYearId", "academicTermId"] {
                let parameter = parameters.iter().find(|p| p["name"] == name).unwrap();
                assert_eq!(parameter["in"], "query");
                assert_eq!(parameter["required"], true);
            }
            if let Some(domain) = parameters.iter().find(|p| p["name"] == "domain") {
                assert!(domain["schema"]
                    .to_string()
                    .contains("LearnerEvaluationDomain"));
            }
            assert!(
                operation["responses"]["200"]["content"]["application/json"]["schema"]
                    .to_string()
                    .contains("ApiResponse")
            );
        }
    }
}
