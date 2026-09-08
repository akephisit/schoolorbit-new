use super::{models::*, services::*};
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use crate::test_helpers::create_named_test_pool_with_max_connections;
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};
use uuid::Uuid;

#[tokio::test]
async fn gradebook_defaults_limit_generated_subject_group_reads_to_heads() {
    let (pool, actor, _, _, _) = fixture("gradebook_default_scopes").await;
    let unit: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE code='SUBJ-SC'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let other: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE unit_type='subject_group' AND id<>$1 ORDER BY id LIMIT 1")
        .bind(unit).fetch_one(&pool).await.unwrap();
    let customized_source: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE unit_type='subject_group' AND id<>$1 AND id<>$2 ORDER BY id LIMIT 1")
        .bind(unit).bind(other).fetch_one(&pool).await.unwrap();
    // A school can intentionally grant all three reads together with matching
    // timestamps; they must not be mistaken for the original migration seed.
    sqlx::query("UPDATE organization_permission_grants g SET created_at=now() FROM permissions p WHERE g.permission_id=p.id AND g.organization_unit_id=$1 AND p.code=ANY($2)")
        .bind(customized_source).bind(vec![codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT, codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT, codes::ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT]).execute(&pool).await.unwrap();
    // A later school edit must survive even if the broad position was retained.
    sqlx::query("UPDATE organization_permission_grants g SET created_by=$1 FROM permissions p WHERE g.permission_id=p.id AND g.organization_unit_id=$2 AND p.code=$3")
        .bind(actor.user_id).bind(other).bind(codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    sqlx::query("UPDATE organization_permission_grants g SET created_at=g.created_at+interval '1 day' FROM permissions p WHERE g.permission_id=p.id AND g.organization_unit_id=$1 AND p.code=$2")
        .bind(other).bind(codes::ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    // Existing head grants must not cause unique-key failure or be duplicated.
    sqlx::query("INSERT INTO organization_permission_grants (organization_unit_id,permission_id,position_code) SELECT $1,id,'head' FROM permissions WHERE code=$2 ON CONFLICT DO NOTHING")
        .bind(unit).bind(codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    crate::db::migration::run_tenant_migrations(&pool)
        .await
        .unwrap();
    for code in [
        codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT,
    ] {
        let positions: Vec<Option<String>> = sqlx::query_scalar("SELECT g.position_code FROM organization_permission_grants g JOIN permissions p ON p.id=g.permission_id WHERE g.organization_unit_id=$1 AND p.code=$2 ORDER BY g.position_code NULLS FIRST")
            .bind(unit).bind(code).fetch_all(&pool).await.unwrap();
        assert_eq!(positions, vec![Some("head".into())]);
        let custom_positions: Vec<Option<String>> = sqlx::query_scalar("SELECT g.position_code FROM organization_permission_grants g JOIN permissions p ON p.id=g.permission_id WHERE g.organization_unit_id=$1 AND p.code=$2")
            .bind(other).bind(code).fetch_all(&pool).await.unwrap();
        assert_eq!(custom_positions, vec![None]);
        let custom_source_positions: Vec<Option<String>> = sqlx::query_scalar("SELECT g.position_code FROM organization_permission_grants g JOIN permissions p ON p.id=g.permission_id WHERE g.organization_unit_id=$1 AND p.code=$2")
            .bind(customized_source).bind(code).fetch_all(&pool).await.unwrap();
        assert_eq!(custom_source_positions, vec![None]);
    }
    let assessment_positions: Vec<Option<String>> = sqlx::query_scalar("SELECT g.position_code FROM organization_permission_grants g JOIN permissions p ON p.id=g.permission_id WHERE g.organization_unit_id=$1 AND p.code=$2")
        .bind(unit).bind(codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT).fetch_all(&pool).await.unwrap();
    assert_eq!(assessment_positions, vec![None]);
}

// Catches overflow through either mutation path, including concurrent writers.
#[tokio::test]
async fn gradebook_item_budget_prevents_overallocation() {
    let (pool, actor, context, group, phase) = fixture("gradebook_item_budget").await;
    sqlx::query("DELETE FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(group).bind(phase).execute(&pool).await.unwrap();
    sqlx::query("UPDATE course_assessment_phases SET max_score=20 WHERE id=$1")
        .bind(phase)
        .execute(&pool)
        .await
        .unwrap();
    let input = |maximum: &str, version| ItemInput {
        name: "Work".into(),
        max_score: maximum.into(),
        display_order: 1,
        row_version: version,
    };
    let first = create_item(&pool, &actor, group, phase, &context, input("10", None))
        .await
        .unwrap();
    assert!(matches!(
        create_item(&pool, &actor, group, phase, &context, input("10.01", None)).await,
        Err(AppError::ValidationError(_))
    ));
    let (left, right) = tokio::join!(
        create_item(&pool, &actor, group, phase, &context, input("10", None)),
        create_item(&pool, &actor, group, phase, &context, input("10", None))
    );
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "only one concurrent writer may consume the remaining budget"
    );
    assert!(matches!(
        create_item(&pool, &actor, group, phase, &context, input("0", None)).await,
        Err(AppError::ValidationError(_))
    ));
    assert!(matches!(
        update_item(
            &pool,
            &actor,
            group,
            phase,
            first.id,
            &context,
            input("10.01", Some(first.row_version))
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    let lowered = update_item(
        &pool,
        &actor,
        group,
        phase,
        first.id,
        &context,
        input("8", Some(first.row_version)),
    )
    .await
    .unwrap();
    create_item(&pool, &actor, group, phase, &context, input("2", None))
        .await
        .unwrap();
    // A later structure reduction must still allow cosmetic edits and gradual repair.
    sqlx::query("UPDATE course_assessment_phases SET max_score=5 WHERE id=$1")
        .bind(phase)
        .execute(&pool)
        .await
        .unwrap();
    let renamed = update_item(
        &pool,
        &actor,
        group,
        phase,
        first.id,
        &context,
        input("8", Some(lowered.row_version)),
    )
    .await
    .unwrap();
    assert!(matches!(
        update_item(
            &pool,
            &actor,
            group,
            phase,
            first.id,
            &context,
            input("8.01", Some(renamed.row_version))
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    update_item(
        &pool,
        &actor,
        group,
        phase,
        first.id,
        &context,
        input("7", Some(renamed.row_version)),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn gradebook_review_phase_code_resolution_checks_context() {
    let (pool, actor, context, group, expected_phase) =
        fixture("gradebook_phase_code_resolution").await;
    for code in ["before_midterm", "midterm", "after_midterm", "final"] {
        let phase = resolve_phase_id(&pool, group, code, &context)
            .await
            .unwrap();
        let workspace = get_group_phase_workspace(&pool, &actor, group, phase, &context)
            .await
            .unwrap();
        assert_eq!(workspace.phase_code, code);
        assert_eq!(workspace.learning_group_id, group);
        if code == "before_midterm" {
            assert_eq!(phase, expected_phase);
        }
    }
    for invalid in ["not_a_phase", "BeforeMidterm", ""] {
        assert!(matches!(
            resolve_phase_id(&pool, group, invalid, &context).await,
            Err(AppError::ValidationError(_))
        ));
    }
    assert!(matches!(
        resolve_phase_id(&pool, group, &expected_phase.to_string(), &context).await,
        Err(AppError::ValidationError(_))
    ));
    let wrong_year = GradebookContext {
        academic_year_id: Uuid::new_v4(),
        academic_term_id: context.academic_term_id,
    };
    assert!(matches!(
        resolve_phase_id(&pool, group, "before_midterm", &wrong_year).await,
        Err(AppError::ValidationError(_))
    ));
    assert!(matches!(
        resolve_phase_id(&pool, Uuid::new_v4(), "before_midterm", &context).await,
        Err(AppError::NotFound(_))
    ));
}

// Catches the public handler contract drifting back to UUID phase paths.
#[test]
fn gradebook_review_group_phase_contract_uses_phase_codes() {
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    #[openapi(paths(
        crate::modules::academic::gradebook::handlers::get_group_phase_workspace,
        crate::modules::academic::gradebook::handlers::create_item,
        crate::modules::academic::gradebook::handlers::update_item,
        crate::modules::academic::gradebook::handlers::remove_item,
        crate::modules::academic::gradebook::handlers::save_scores_batch,
        crate::modules::academic::gradebook::handlers::confirm_phase,
    ))]
    struct GradebookPaths;
    let contract = serde_json::to_value(GradebookPaths::openapi()).unwrap();
    for (suffix, method) in [
        ("", "get"),
        ("/items", "post"),
        ("/items/{item_id}", "put"),
        ("/items/{item_id}", "delete"),
        ("/scores", "put"),
        ("/confirm", "post"),
    ] {
        let path =
            format!("/api/academic/gradebook/groups/{{group_id}}/phases/{{phase_code}}{suffix}");
        let operation = contract["paths"]
            .get(&path)
            .and_then(|item| item.get(method))
            .unwrap_or_else(|| panic!("missing phase-code operation: {method} {path}"));
        let parameter = operation["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "phase_code")
            .unwrap();
        assert_eq!(parameter["in"], "path");
        assert_ne!(parameter["schema"]["format"], "uuid");
    }
}

// Catches confirmation ABA: changing and restoring inputs must not reuse an old approval token.
#[tokio::test]
async fn gradebook_review_confirmation_revision_survives_invalidation() {
    let (pool, actor, context, group, phase) = fixture("gradebook_confirmation_aba").await;
    sqlx::query("DELETE FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(group).bind(phase).execute(&pool).await.unwrap();
    let initial = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    let item = create_item(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ItemInput {
            name: "Complete phase".into(),
            max_score: initial.phase_max_score.clone(),
            display_order: 0,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let source = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    let original = confirm_phase(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ConfirmInput {
            source_checksum: source.source_checksum.clone(),
            roster_checksum: source.roster_checksum.clone(),
            row_version: None,
        },
    )
    .await
    .unwrap();
    let resized = update_item(
        &pool,
        &actor,
        group,
        phase,
        item.id,
        &context,
        ItemInput {
            name: item.name.clone(),
            max_score: "0".into(),
            display_order: 0,
            row_version: Some(item.row_version),
        },
    )
    .await
    .unwrap();
    update_item(
        &pool,
        &actor,
        group,
        phase,
        item.id,
        &context,
        ItemInput {
            name: item.name,
            max_score: initial.phase_max_score,
            display_order: 0,
            row_version: Some(resized.row_version),
        },
    )
    .await
    .unwrap();
    let restored = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    assert_eq!(restored.source_checksum, source.source_checksum);
    let stale = restored
        .confirmation
        .expect("invalidation must retain the confirmation revision");
    assert_eq!(stale.id, original.id);
    assert!(stale.row_version > original.row_version);
    assert!(
        !restored.confirmation_is_current,
        "restoring source values must not silently restore confirmation"
    );
    let latest = confirm_phase(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ConfirmInput {
            source_checksum: source.source_checksum.clone(),
            roster_checksum: source.roster_checksum.clone(),
            row_version: Some(stale.row_version),
        },
    )
    .await
    .unwrap();
    assert_eq!(latest.id, original.id);
    assert!(latest.row_version > stale.row_version);
    assert!(matches!(
        confirm_phase(
            &pool,
            &actor,
            group,
            phase,
            &context,
            ConfirmInput {
                source_checksum: source.source_checksum,
                roster_checksum: source.roster_checksum,
                row_version: Some(original.row_version)
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let current = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    assert!(current.confirmation_is_current);
    assert_eq!(
        current.confirmation.unwrap().row_version,
        latest.row_version
    );
}

#[tokio::test]
async fn gradebook_concurrent_scores_have_one_winner() {
    let (pool, actor, context, group, phase) = fixture("gradebook_race").await;
    let item = create_item(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ItemInput {
            name: "Race".into(),
            max_score: "10".into(),
            display_order: 0,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let ws = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    let cell = |value: &str| {
        vec![ScoreCellMutation::Set {
            score_item_id: item.id,
            student_academic_year_id: ws.students[0].student_academic_year_id,
            value: value.into(),
            row_version: None,
        }]
    };
    let (a, b) = tokio::join!(
        save_scores_batch(&pool, &actor, group, phase, &context, cell("1")),
        save_scores_batch(&pool, &actor, group, phase, &context, cell("2"))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(a, Err(AppError::Conflict(_))) || matches!(b, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn gradebook_invalidation_is_limited_to_the_changed_phase() {
    let (pool, actor, context, group, phase) = fixture("gradebook_targeted_invalidation").await;
    let other:Uuid=sqlx::query_scalar("SELECT p.id FROM course_assessment_phases p JOIN course_assessment_phases selected ON selected.plan_id=p.plan_id WHERE selected.id=$1 AND p.phase_code='midterm'").bind(phase).fetch_one(&pool).await.unwrap();
    let mut confirmations = Vec::new();
    let mut items = Vec::new();
    for selected in [phase, other] {
        sqlx::query("DELETE FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(group).bind(selected).execute(&pool).await.unwrap();
        let ws = get_group_phase_workspace(&pool, &actor, group, selected, &context)
            .await
            .unwrap();
        items.push(
            create_item(
                &pool,
                &actor,
                group,
                selected,
                &context,
                ItemInput {
                    name: "Phase".into(),
                    max_score: ws.phase_max_score,
                    display_order: 0,
                    row_version: None,
                },
            )
            .await
            .unwrap(),
        );
        let ws = get_group_phase_workspace(&pool, &actor, group, selected, &context)
            .await
            .unwrap();
        confirmations.push(
            confirm_phase(
                &pool,
                &actor,
                group,
                selected,
                &context,
                ConfirmInput {
                    source_checksum: ws.source_checksum,
                    roster_checksum: ws.roster_checksum,
                    row_version: None,
                },
            )
            .await
            .unwrap(),
        );
    }
    update_item(
        &pool,
        &actor,
        group,
        phase,
        items[0].id,
        &context,
        ItemInput {
            name: "Phase".into(),
            max_score: "0".into(),
            display_order: 0,
            row_version: Some(1),
        },
    )
    .await
    .unwrap();
    assert!(
        !get_group_phase_workspace(&pool, &actor, group, phase, &context)
            .await
            .unwrap()
            .confirmation_is_current
    );
    let untouched = get_group_phase_workspace(&pool, &actor, group, other, &context)
        .await
        .unwrap();
    assert!(untouched.confirmation_is_current);
    assert_eq!(untouched.confirmation.unwrap().id, confirmations[1].id);
    sqlx::query("UPDATE course_assessment_phases SET max_score=max_score+1,row_version=row_version+1 WHERE id=$1").bind(other).execute(&pool).await.unwrap();
    let stale = get_group_phase_workspace(&pool, &actor, group, other, &context)
        .await
        .unwrap();
    assert!(!stale.confirmation_is_current);
    assert_eq!(
        stale.confirmation.unwrap().row_version,
        confirmations[1].row_version
    );
}

#[tokio::test]
async fn gradebook_policy_window_primary_and_context() {
    let (pool, manager, context, group, phase) = fixture("gradebook_policy").await;
    let mut teacher = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED.to_string()],
    };
    assert!(
        get_group_phase_workspace(&pool, &teacher, group, phase, &context)
            .await
            .is_ok()
    );
    sqlx::query("UPDATE academic_gradebook_phase_controls SET score_entry_enabled=false")
        .execute(&pool)
        .await
        .unwrap();
    let input = || ItemInput {
        name: "Work".into(),
        max_score: "1".into(),
        display_order: 0,
        row_version: None,
    };
    assert!(matches!(
        create_item(&pool, &teacher, group, phase, &context, input()).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(
        create_item(&pool, &manager, group, phase, &context, input())
            .await
            .is_ok()
    );
    teacher.user_id = Uuid::new_v4();
    assert!(matches!(
        get_group_phase_workspace(&pool, &teacher, group, phase, &context).await,
        Err(AppError::Forbidden(_))
    ));
    let wrong = GradebookContext {
        academic_year_id: Uuid::new_v4(),
        academic_term_id: context.academic_term_id,
    };
    assert!(
        get_group_phase_workspace(&pool, &manager, group, phase, &wrong)
            .await
            .is_err()
    );
    let reader = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_GRADEBOOK_READ_SCHOOL.to_string()],
    };
    assert!(
        get_group_phase_workspace(&pool, &reader, group, phase, &context)
            .await
            .is_ok()
    );
    assert!(create_item(&pool, &reader, group, phase, &context, input())
        .await
        .is_err());
    sqlx::query("UPDATE learning_groups SET status='draft' WHERE id=$1")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_group_teachers SET role='secondary' WHERE learning_group_id=$1 AND teacher_id=$2").bind(group).bind(manager.user_id).execute(&pool).await.unwrap();
    teacher.user_id = manager.user_id;
    let secondary = get_group_phase_workspace(&pool, &teacher, group, phase, &context)
        .await
        .unwrap();
    assert!(!secondary.can_confirm);
    assert!(matches!(
        confirm_phase(
            &pool,
            &teacher,
            group,
            phase,
            &context,
            ConfirmInput {
                source_checksum: secondary.source_checksum,
                roster_checksum: secondary.roster_checksum,
                row_version: None
            }
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    sqlx::query("UPDATE learning_group_teachers teacher SET starts_on=term.planned_end_date+1 FROM academic_terms term WHERE teacher.learning_group_id=$1 AND term.id=teacher.academic_term_id").bind(group).execute(&pool).await.unwrap();
    assert!(matches!(
        get_group_phase_workspace(&pool, &teacher, group, phase, &context).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(list_subjects(&pool, &teacher, &context)
        .await
        .unwrap()
        .iter()
        .all(|r| r.learning_group_id != group));
}

async fn fixture(name: &str) -> (sqlx::PgPool, ActorContext, GradebookContext, Uuid, Uuid) {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 60).await.unwrap();
    let (year,term,group,phase,teacher): (Uuid,Uuid,Uuid,Uuid,Uuid) = sqlx::query_as("SELECT g.academic_year_id,g.academic_term_id,g.id,p.id,t.teacher_id FROM learning_groups g JOIN course_assessment_plans plan ON plan.learning_offering_id=g.learning_offering_id JOIN course_assessment_phases p ON p.plan_id=plan.id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND p.phase_code='before_midterm' ORDER BY g.id LIMIT 1").fetch_one(&pool).await.unwrap();
    let actor = ActorContext {
        user_id: teacher,
        permissions: vec![
            codes::ACADEMIC_GRADEBOOK_MANAGE_SCHOOL.to_string(),
            codes::ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED.to_string(),
        ],
    };
    (
        pool,
        actor,
        GradebookContext {
            academic_year_id: year,
            academic_term_id: term,
        },
        group,
        phase,
    )
}

// Catches coercing blank to zero, per-cell partial writes, and stale updates.
#[tokio::test]
async fn gradebook_zero_clear_and_atomic_validation() {
    let (pool, actor, context, group, phase) = fixture("gradebook_zero_clear").await;
    let item = create_item(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ItemInput {
            name: "Test".into(),
            max_score: "10.25".into(),
            display_order: 0,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let ws = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    let student = ws.students[0].student_academic_year_id;
    let set = |value: &str, version| ScoreCellMutation::Set {
        score_item_id: item.id,
        student_academic_year_id: student,
        value: value.into(),
        row_version: version,
    };
    let saved = save_scores_batch(&pool, &actor, group, phase, &context, vec![set("0", None)])
        .await
        .unwrap();
    assert_eq!(saved.cells[0].value.as_deref(), Some("0.00"));
    assert!(matches!(
        save_scores_batch(&pool, &actor, group, phase, &context, vec![set("1", None)]).await,
        Err(AppError::Conflict(_))
    ));
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![set("10.26", saved.cells[0].row_version)]
    )
    .await
    .is_err());
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![set("1.001", saved.cells[0].row_version)]
    )
    .await
    .is_err());
    let cleared = save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![ScoreCellMutation::Clear {
            score_item_id: item.id,
            student_academic_year_id: student,
            row_version: saved.cells[0].row_version,
        }],
    )
    .await
    .unwrap();
    assert_eq!(cleared.cells[0].value, None);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_student_scores WHERE score_item_id=$1",
    )
    .bind(item.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let upper = save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![set("10.25", None), set("10.25", None)],
    )
    .await
    .unwrap();
    assert_eq!(upper.cells.len(), 1);
    assert_eq!(upper.cells[0].value.as_deref(), Some("10.25"));
    let invalid = ScoreCellMutation::Set {
        score_item_id: item.id,
        student_academic_year_id: Uuid::new_v4(),
        value: "1".into(),
        row_version: None,
    };
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![set("5", upper.cells[0].row_version), invalid]
    )
    .await
    .is_err());
    let stored: String = sqlx::query_scalar(
        "SELECT score::text FROM learning_group_student_scores WHERE score_item_id=$1",
    )
    .bind(item.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, "10.25");
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![
            set("1", upper.cells[0].row_version),
            set("2", upper.cells[0].row_version)
        ]
    )
    .await
    .is_err());
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![set("1", upper.cells[0].row_version); 501]
    )
    .await
    .is_err());
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![set("-1", upper.cells[0].row_version)]
    )
    .await
    .is_err());
}

// Catches bypassing locked course scope through any ordinary Gradebook write.
#[tokio::test]
async fn gradebook_locked_scope_is_read_only() {
    let (pool, actor, context, group, phase) = fixture("gradebook_locked").await;
    let ws = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    sqlx::query("INSERT INTO academic_course_result_locks (subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) SELECT d.subject_id,d.academic_term_id,d.academic_year_id,p.id,'{}'::jsonb,$2,$3,'{}'::jsonb,$4 FROM course_offering_details d CROSS JOIN academic_grading_policy_versions p WHERE d.learning_offering_id=$1 AND p.lifecycle='active'").bind(ws.learning_offering_id).bind(&ws.roster_checksum).bind(&ws.source_checksum).bind(actor.user_id).execute(&pool).await.unwrap();
    let locked = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    assert!(locked.locked);
    assert!(!locked.can_manage);
    assert!(!locked.can_confirm);
    assert!(matches!(
        create_item(
            &pool,
            &actor,
            group,
            phase,
            &context,
            ItemInput {
                name: "Blocked".into(),
                max_score: "1".into(),
                display_order: 0,
                row_version: None
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        save_scores_batch(
            &pool,
            &actor,
            group,
            phase,
            &context,
            vec![ScoreCellMutation::Set {
                score_item_id: Uuid::new_v4(),
                student_academic_year_id: Uuid::new_v4(),
                value: "0".into(),
                row_version: None
            }]
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        confirm_phase(
            &pool,
            &actor,
            group,
            phase,
            &context,
            ConfirmInput {
                source_checksum: ws.source_checksum,
                roster_checksum: ws.roster_checksum,
                row_version: None
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

// Catches list policies silently dropping independent assigned/unit grants.
#[tokio::test]
async fn gradebook_lists_union_exact_unit_and_assignments() {
    let (pool, manager, context, _, _) = fixture("gradebook_union").await;
    let school = list_subjects(&pool, &manager, &context).await.unwrap();
    assert!(!school.is_empty());
    assert_eq!(school[0].phases.len(), 4);
    let unit: Uuid = sqlx::query_scalar(
        "SELECT owning_organization_unit_id FROM learning_offerings WHERE id=$1",
    )
    .bind(school[0].learning_offering_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO organization_members (id,user_id,organization_unit_id,position_code,started_at) VALUES (gen_random_uuid(),$1,$2,'head','2020-01-01')").bind(manager.user_id).bind(unit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_permission_grants (organization_unit_id,permission_id,created_by,position_code) SELECT $1,id,$2,'head' FROM permissions WHERE code=$3 ON CONFLICT DO NOTHING").bind(unit).bind(manager.user_id).bind(codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    let assigned = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_GRADEBOOK_READ_ASSIGNED.into()],
    };
    let organization = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT.into()],
    };
    let both = ActorContext {
        user_id: manager.user_id,
        permissions: vec![
            codes::ACADEMIC_GRADEBOOK_READ_ASSIGNED.into(),
            codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT.into(),
        ],
    };
    let assigned_rows = list_subjects(&pool, &assigned, &context).await.unwrap();
    let organization_rows = list_subjects(&pool, &organization, &context).await.unwrap();
    assert!(!assigned_rows.is_empty());
    assert!(assigned_rows.iter().all(|row| row.assigned));
    let unassigned_teacher = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ACADEMIC_GRADEBOOK_READ_ASSIGNED.into()],
    };
    assert!(list_subjects(&pool, &unassigned_teacher, &context)
        .await
        .unwrap()
        .is_empty());
    let school_reader = ActorContext {
        user_id: unassigned_teacher.user_id,
        permissions: vec![codes::ACADEMIC_GRADEBOOK_READ_SCHOOL.into()],
    };
    assert_eq!(
        list_subjects(&pool, &school_reader, &context)
            .await
            .unwrap()
            .len(),
        school.len()
    );
    assert!(!organization_rows.is_empty());
    let union = list_subjects(&pool, &both, &context).await.unwrap();
    for row in assigned_rows.iter().chain(organization_rows.iter()) {
        assert!(union
            .iter()
            .any(|r| r.learning_group_id == row.learning_group_id));
    }
    assert!(union
        .windows(2)
        .all(|pair| pair[0].assigned || !pair[1].assigned));
    let denied = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_ASSESSMENT_READ_SCHOOL.into()],
    };
    assert!(matches!(
        list_subjects(&pool, &denied, &context).await,
        Err(AppError::Forbidden(_))
    ));
}

// Catches unordered hashing and accidental reliance on cosmetic item revisions.
#[test]
fn gradebook_checksums_are_ordered_and_calculation_sensitive() {
    let group = Uuid::from_u128(1);
    let phase = Uuid::from_u128(2);
    let items = vec![
        ScoreItem {
            id: Uuid::from_u128(3),
            name: "A".into(),
            max_score: "10.00".into(),
            display_order: 0,
            lifecycle: "active".into(),
            row_version: 1,
        },
        ScoreItem {
            id: Uuid::from_u128(4),
            name: "B".into(),
            max_score: "5.00".into(),
            display_order: 1,
            lifecycle: "active".into(),
            row_version: 1,
        },
    ];
    let students = vec![GradebookStudent {
        membership_id: Uuid::from_u128(5),
        student_academic_year_id: Uuid::from_u128(6),
        display_name: "Student".into(),
        row_version: 1,
    }];
    let scores = vec![ScoreCell {
        score_item_id: items[0].id,
        student_academic_year_id: students[0].student_academic_year_id,
        value: Some("0.00".into()),
        row_version: Some(1),
    }];
    let first = source_checksums(group, phase, 1, "15.00", &items, &students, &scores).unwrap();
    let mut reordered = items.clone();
    reordered.reverse();
    reordered[0].name = "Renamed".into();
    reordered[0].row_version = 5;
    assert_eq!(
        first,
        source_checksums(group, phase, 1, "15.00", &reordered, &students, &scores).unwrap()
    );
    let blank = source_checksums(group, phase, 1, "15.00", &items, &students, &[]).unwrap();
    assert_eq!(first.0, blank.0);
    assert_ne!(first.1, blank.1);
    assert_ne!(
        first.1,
        source_checksums(group, phase, 2, "15.00", &items, &students, &scores)
            .unwrap()
            .1
    );
    reordered[0].max_score = "6.00".into();
    assert_ne!(
        first.1,
        source_checksums(group, phase, 1, "15.00", &reordered, &students, &scores)
            .unwrap()
            .1
    );
}

// Catches hard-deleting retained evidence and accepting impossible maxima.
#[tokio::test]
async fn gradebook_item_lifecycle_and_lower_maximum() {
    let (pool, actor, context, group, phase) = fixture("gradebook_item_lifecycle").await;
    let item = create_item(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ItemInput {
            name: "Work".into(),
            max_score: "10".into(),
            display_order: 0,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let ws = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![ScoreCellMutation::Set {
            score_item_id: item.id,
            student_academic_year_id: ws.students[0].student_academic_year_id,
            value: "10".into(),
            row_version: None,
        }],
    )
    .await
    .unwrap();
    assert!(update_item(
        &pool,
        &actor,
        group,
        phase,
        item.id,
        &context,
        ItemInput {
            name: "Work".into(),
            max_score: "9.99".into(),
            display_order: 0,
            row_version: Some(1)
        }
    )
    .await
    .is_err());
    let removed = remove_item(&pool, &actor, group, phase, item.id, &context, 1)
        .await
        .unwrap();
    assert_eq!(removed.disposition, ScoreItemRemovalDisposition::Cancelled);
    let retained: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_student_scores WHERE score_item_id=$1",
    )
    .bind(item.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, 1);
    assert!(save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![ScoreCellMutation::Set {
            score_item_id: item.id,
            student_academic_year_id: ws.students[0].student_academic_year_id,
            value: "0".into(),
            row_version: Some(1)
        }]
    )
    .await
    .is_err());
    let empty = create_item(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ItemInput {
            name: "Empty".into(),
            max_score: "0".into(),
            display_order: 1,
            row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        remove_item(&pool, &actor, group, phase, empty.id, &context, 1)
            .await
            .unwrap()
            .disposition,
        ScoreItemRemovalDisposition::Deleted
    );
}

// Catches confirmations surviving score/roster changes or becoming stale on cosmetic changes.
#[tokio::test]
async fn gradebook_confirmation_snapshot_and_cosmetic_changes() {
    let (pool, actor, context, group, phase) = fixture("gradebook_confirm").await;
    sqlx::query("DELETE FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(group).bind(phase).execute(&pool).await.unwrap();
    let ws = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    assert!(confirm_phase(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ConfirmInput {
            source_checksum: ws.source_checksum.clone(),
            roster_checksum: ws.roster_checksum.clone(),
            row_version: None
        }
    )
    .await
    .is_err());
    let item = create_item(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ItemInput {
            name: "All".into(),
            max_score: ws.phase_max_score.clone(),
            display_order: 0,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let ws = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    let confirmed = confirm_phase(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ConfirmInput {
            source_checksum: ws.source_checksum.clone(),
            roster_checksum: ws.roster_checksum.clone(),
            row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(confirmed.blank_score_count, ws.students.len() as i64);
    update_item(
        &pool,
        &actor,
        group,
        phase,
        item.id,
        &context,
        ItemInput {
            name: "Renamed".into(),
            max_score: ws.phase_max_score,
            display_order: 5,
            row_version: Some(1),
        },
    )
    .await
    .unwrap();
    let renamed = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    assert_eq!(renamed.source_checksum, ws.source_checksum);
    assert!(renamed.confirmation.is_some());
    let controls = list_controls(&pool, &actor, &context).await.unwrap();
    let control = controls
        .iter()
        .find(|c| c.phase_code == renamed.phase_code)
        .unwrap();
    update_control(
        &pool,
        &actor,
        control.id,
        &context,
        UpdateControlInput {
            score_entry_enabled: !control.score_entry_enabled,
            row_version: control.row_version,
        },
    )
    .await
    .unwrap();
    assert!(
        get_group_phase_workspace(&pool, &actor, group, phase, &context)
            .await
            .unwrap()
            .confirmation_is_current
    );
    sqlx::query(
        "UPDATE learning_group_students SET row_version=row_version+1 WHERE learning_group_id=$1",
    )
    .bind(group)
    .execute(&pool)
    .await
    .unwrap();
    let changed = get_group_phase_workspace(&pool, &actor, group, phase, &context)
        .await
        .unwrap();
    assert!(!changed.confirmation_is_current);
    assert_ne!(changed.roster_checksum, ws.roster_checksum);
    assert!(matches!(
        confirm_phase(
            &pool,
            &actor,
            group,
            phase,
            &context,
            ConfirmInput {
                source_checksum: ws.source_checksum,
                roster_checksum: ws.roster_checksum,
                row_version: Some(confirmed.row_version)
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let reconfirmed = confirm_phase(
        &pool,
        &actor,
        group,
        phase,
        &context,
        ConfirmInput {
            source_checksum: changed.source_checksum,
            roster_checksum: changed.roster_checksum,
            row_version: Some(confirmed.row_version),
        },
    )
    .await
    .unwrap();
    assert_eq!(reconfirmed.row_version, confirmed.row_version + 1);
    save_scores_batch(
        &pool,
        &actor,
        group,
        phase,
        &context,
        vec![ScoreCellMutation::Set {
            score_item_id: item.id,
            student_academic_year_id: ws.students[0].student_academic_year_id,
            value: "0".into(),
            row_version: None,
        }],
    )
    .await
    .unwrap();
    assert!(
        !get_group_phase_workspace(&pool, &actor, group, phase, &context)
            .await
            .unwrap()
            .confirmation_is_current
    );
}
