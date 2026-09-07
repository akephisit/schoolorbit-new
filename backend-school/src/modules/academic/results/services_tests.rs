use super::{models::*, services::*};
use crate::modules::academic::{
    cutover_test_support::{apply_migrations_through, seed_release_two_predecessor},
    delivery::{
        models::{
            AddDatedRosterMembershipRequest, LearningTeacherRole,
            RemoveDatedRosterMembershipRequest, ReplaceLearningGroupTeachersRequest,
            TeacherAssignmentInput,
        },
        services::{groups as delivery_groups, roster_memberships},
    },
    gradebook::{models as gm, services as gb},
};
use crate::{
    middleware::permission::ActorContext, permissions::registry::codes,
    test_helpers::create_named_test_pool_with_max_connections,
};
use chrono::NaiveDate;
use std::time::Duration;
use uuid::Uuid;

fn bands() -> Vec<GradingPolicyBand> {
    [
        ("0", "0"),
        ("1", "50"),
        ("1.5", "55"),
        ("2", "60"),
        ("2.5", "65"),
        ("3", "70"),
        ("3.5", "75"),
        ("4", "80"),
    ]
    .into_iter()
    .map(|(g, b)| GradingPolicyBand {
        grade: g.into(),
        lower_bound: b.into(),
    })
    .collect()
}
// Catches exclusive thresholds, decimal rounding and accepting incomplete/disordered policies.
#[test]
fn results_exact_inclusive_policy_and_teacher_contract() {
    assert!(validate_policy(&bands(), "100").is_ok());
    for (score, want) in [
        ("0", "0.00"),
        ("49.99", "0.00"),
        ("50", "1.00"),
        ("54.99", "1.00"),
        ("55", "1.50"),
        ("79.99", "3.50"),
        ("80", "4.00"),
        ("100", "4.00"),
    ] {
        assert_eq!(derive_grade(&bands(), score, "100").unwrap(), want);
    }
    assert!(validate_policy(&bands()[..7], "100").is_err());
    assert!(validate_policy(&bands(), "79.99").is_err());
    let mut wrong = bands();
    wrong[2].lower_bound = "50".into();
    assert!(validate_policy(&wrong, "100").is_err());
    wrong = bands();
    wrong[0].lower_bound = "1".into();
    assert!(validate_policy(&wrong, "100").is_err());
    wrong = bands();
    wrong[1].grade = "0.5".into();
    assert!(validate_policy(&wrong, "100").is_err());
    for selection in [
        "derived",
        "manual_zero",
        "incomplete",
        "insufficient_attendance",
    ] {
        assert!(
            serde_json::from_value::<CourseOutcomeSelection>(serde_json::json!(selection)).is_ok()
        );
    }
    for selection in ["1", "4", "numeric", "percentile"] {
        assert!(
            serde_json::from_value::<CourseOutcomeSelection>(serde_json::json!(selection)).is_err()
        );
    }
    assert!(serde_json::from_value::<GradingPolicyInput>(
        serde_json::json!({"name":"x","bands":bands(),"method":"group"})
    )
    .is_err());
    for outcome in ["pass", "fail"] {
        assert!(serde_json::from_value::<ActivityOutcome>(serde_json::json!(outcome)).is_ok());
    }
    assert!(serde_json::from_value::<ActivityOutcome>(serde_json::json!("incomplete")).is_err());
}
async fn fixture(name: &str) -> (sqlx::PgPool, ActorContext, ResultContext, Uuid) {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 61).await.unwrap();
    let (year,term,group,teacher):(Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT g.academic_year_id,g.academic_term_id,g.id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1").fetch_one(&pool).await.unwrap();
    let actor = ActorContext {
        user_id: teacher,
        permissions: vec![
            codes::ACADEMIC_RESULT_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_RESULT_MANAGE_ASSIGNED.into(),
            codes::ACADEMIC_GRADEBOOK_MANAGE_SCHOOL.into(),
        ],
    };
    (
        pool,
        actor,
        ResultContext {
            academic_year_id: year,
            academic_term_id: term,
        },
        group,
    )
}
async fn prepare_phases(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    ctx: &ResultContext,
    group: Uuid,
) {
    let context = gm::GradebookContext {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
    };
    let phases:Vec<Uuid>=sqlx::query_scalar("SELECT phase.id FROM course_assessment_phases phase JOIN course_assessment_plans p ON p.id=phase.plan_id JOIN learning_groups g ON g.learning_offering_id=p.learning_offering_id WHERE g.id=$1 ORDER BY phase.id").bind(group).fetch_all(pool).await.unwrap();
    for phase in phases {
        sqlx::query("DELETE FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(group).bind(phase).execute(pool).await.unwrap();
        let ws = gb::get_group_phase_workspace(pool, actor, group, phase, &context)
            .await
            .unwrap();
        gb::create_item(
            pool,
            actor,
            group,
            phase,
            &context,
            gm::ItemInput {
                name: "All".into(),
                max_score: ws.phase_max_score,
                display_order: 0,
                row_version: None,
            },
        )
        .await
        .unwrap();
        let ws = gb::get_group_phase_workspace(pool, actor, group, phase, &context)
            .await
            .unwrap();
        gb::confirm_phase(
            pool,
            actor,
            group,
            phase,
            &context,
            gm::ConfirmInput {
                source_checksum: ws.source_checksum,
                roster_checksum: ws.roster_checksum,
                row_version: ws.confirmation.map(|c| c.row_version),
            },
        )
        .await
        .unwrap();
    }
}
fn confirm_input(ws: &CoursePreparationWorkspace) -> ResultConfirmationInput {
    ResultConfirmationInput {
        source_checksum: ws.source_checksum.clone(),
        roster_checksum: ws.roster_checksum.clone(),
        row_version: ws.confirmation.as_ref().map(|c| c.row_version),
    }
}

async fn prepare_published_roster_mutation(pool: &sqlx::PgPool, group: Uuid) -> (Uuid, NaiveDate) {
    sqlx::query(
        "UPDATE learning_offerings SET status='published' WHERE id=(SELECT learning_offering_id FROM learning_groups WHERE id=$1)",
    )
    .bind(group)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE learning_groups SET status='published',roster_status='published' WHERE id=$1",
    )
    .bind(group)
    .execute(pool)
    .await
    .unwrap();
    let (academic_year_id, grade_level_id, study_program_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT group_row.academic_year_id,student_year.grade_level_id,student_year.study_program_id
           FROM learning_groups group_row
           JOIN learning_group_students membership
             ON membership.learning_group_id=group_row.id
            AND membership.membership_status='active'
           JOIN student_academic_years student_year
             ON student_year.id=membership.student_academic_year_id
           WHERE group_row.id=$1
           ORDER BY student_year.id LIMIT 1"#,
    )
    .bind(group)
    .fetch_one(pool)
    .await
    .unwrap();
    let student_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id,email,username,password_hash,first_name,last_name,user_type,status
           ) VALUES ($1,$2,$2,'fixture-not-a-login','Roster','ABA','student','active')"#,
    )
    .bind(student_id)
    .bind(format!("results-roster-aba-{student_id}@example.invalid"))
    .execute(pool)
    .await
    .unwrap();
    let student = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO student_academic_years (
               id,student_id,academic_year_id,grade_level_id,study_program_id,status
           ) VALUES ($1,$2,$3,$4,$5,'active')"#,
    )
    .bind(student)
    .bind(student_id)
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_program_id)
    .execute(pool)
    .await
    .unwrap();
    let joined_at: NaiveDate = sqlx::query_scalar(
        r#"SELECT GREATEST(offering.starts_on,academic_year.start_date)
           FROM learning_groups group_row
           JOIN learning_offerings offering ON offering.id=group_row.learning_offering_id
           JOIN academic_years academic_year ON academic_year.id=group_row.academic_year_id
           WHERE group_row.id=$1"#,
    )
    .bind(group)
    .fetch_one(pool)
    .await
    .unwrap();
    (student, joined_at)
}

async fn add_then_remove_membership_without_result_read(
    pool: &sqlx::PgPool,
    actor: Uuid,
    group: Uuid,
) {
    let (student_academic_year_id, joined_at) =
        prepare_published_roster_mutation(pool, group).await;
    let group_before = delivery_groups::get(pool, group).await.unwrap();
    let added = roster_memberships::add_membership(
        pool,
        actor,
        group,
        AddDatedRosterMembershipRequest {
            group_row_version: group_before.row_version,
            student_academic_year_id,
            joined_at,
        },
    )
    .await
    .unwrap();
    let group_after_add = delivery_groups::get(pool, group).await.unwrap();
    roster_memberships::remove_membership(
        pool,
        actor,
        group,
        added.id,
        RemoveDatedRosterMembershipRequest {
            group_row_version: group_after_add.row_version,
            membership_row_version: added.row_version,
            left_at: joined_at,
        },
    )
    .await
    .unwrap();
}

async fn wait_for_result_confirmation_read_lock(pool: &sqlx::PgPool) {
    for _ in 0..100 {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%learning_group_result_confirmations%')",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("workspace GET did not reach the confirmation read while the table was locked");
}

// Catches accepting missing/stale phase approval, changing blank storage, and reusing confirmation revisions.
#[tokio::test]
async fn results_course_confirmation_blank_and_monotonic_phase_sources() {
    let (pool, actor, ctx, group) = fixture("results_course").await;
    let ws = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(!ws.blockers.is_empty());
    assert!(
        !confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&ws))
            .await
            .unwrap()
            .confirmation_is_current
    );
    prepare_phases(&pool, &actor, &ctx, group).await;
    let ws = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(ws.blockers.is_empty());
    assert_eq!(ws.students[0].calculated_score, "0.00");
    assert_eq!(ws.students[0].numeric_grade.as_deref(), Some("0.00"));
    let confirmed = confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&ws))
        .await
        .unwrap();
    assert!(confirmed.confirmation_is_current);
    let original = confirmed.confirmation.as_ref().unwrap().row_version;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_student_scores WHERE learning_group_id=$1",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("UPDATE learning_group_phase_confirmations SET row_version=row_version+1,source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true') WHERE learning_group_id=$1").bind(group).execute(&pool).await.unwrap();
    let stale = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(!stale.confirmation_is_current);
    assert_eq!(stale.confirmation.as_ref().unwrap().row_version, original);
    assert!(stale
        .blockers
        .iter()
        .any(|b| b.code == ResultBlockerCode::StalePhaseConfirmation));
    assert!(
        !confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&stale))
            .await
            .unwrap()
            .confirmation_is_current
    );
    sqlx::query("UPDATE learning_group_phase_confirmations SET source_snapshot=jsonb_set(source_snapshot,'{invalidated}','false') WHERE learning_group_id=$1").bind(group).execute(&pool).await.unwrap();
    let restored = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(!restored.confirmation_is_current);
    assert!(
        confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&confirmed))
            .await
            .is_err()
    );
}

// Course selections are an approval-stage choice, never a way to bypass the
// four current Gradebook phase confirmations that supply the derived outcome.
#[tokio::test]
async fn results_course_selection_requires_all_current_phase_confirmations() {
    let (pool, actor, ctx, group) = fixture("results_selection_requires_phases").await;
    let workspace = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let student = workspace.students[0].student_academic_year_id;

    assert!(save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::ExplicitZero,
            row_version: None,
        },
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM learning_group_result_overrides WHERE learning_group_id=$1",
        )
        .bind(group)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
// Catches policy activation leaving unlocked approvals valid, mutating activated versions, and losing explicit zero source.
#[tokio::test]
async fn results_policy_activation_and_selection_versions() {
    let (pool, actor, ctx, group) = fixture("results_policy").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    let ws = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let student = ws.students[0].student_academic_year_id;
    let ws = save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::ExplicitZero,
            row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        ws.students[0].selection,
        CourseOutcomeSelection::ExplicitZero
    );
    let v = ws.students[0].selection_row_version;
    let non_primary_manager = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ACADEMIC_RESULT_MANAGE_SCHOOL.into()],
    };
    let read_only = get_course_workspace(&pool, &non_primary_manager, &ctx, group)
        .await
        .unwrap();
    assert!(!read_only.can_manage);
    assert!(save_selection(
        &pool,
        &non_primary_manager,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::Incomplete,
            row_version: v,
        },
    )
    .await
    .is_err());
    assert!(save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::Incomplete,
            row_version: None
        }
    )
    .await
    .is_err());
    let ws = confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&ws))
        .await
        .unwrap();
    assert!(ws.confirmation_is_current);
    let policy = create_policy(
        &pool,
        &actor,
        &ctx,
        GradingPolicyInput {
            name: "New criterion".into(),
            bands: bands(),
        },
    )
    .await
    .unwrap();
    activate_policy(&pool, &actor, &ctx, policy.id, policy.row_version)
        .await
        .unwrap();
    assert!(
        activate_policy(&pool, &actor, &ctx, ws.policy.id, ws.policy.row_version)
            .await
            .is_err()
    );
    let changed = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(!changed.confirmation_is_current);
    assert_eq!(changed.policy.id, policy.id);
    let cleared = save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::Derived,
            row_version: v,
        },
    )
    .await
    .unwrap();
    assert_eq!(cleared.students[0].selection_row_version, None);
    let next = save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::Incomplete,
            row_version: None,
        },
    )
    .await
    .unwrap();
    assert!(next.students[0].selection_row_version > v);
    assert_eq!(next.students[0].numeric_grade, None);
    let reader = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_RESULT_READ_SCHOOL.into()],
    };
    assert!(list_policies(&pool, &reader, &ctx).await.is_ok());
    assert!(create_policy(
        &pool,
        &reader,
        &ctx,
        GradingPolicyInput {
            name: "Denied".into(),
            bands: bands()
        }
    )
    .await
    .is_err());
    let wrong = ResultContext {
        academic_year_id: Uuid::new_v4(),
        ..ctx
    };
    assert!(get_course_workspace(&pool, &actor, &wrong, group)
        .await
        .is_err());
}

// Catches policy activation rewriting a locked subject's preparation snapshot or
// invalidating its retained primary-teacher confirmation.
#[tokio::test]
async fn results_locked_course_keeps_policy_and_confirmation_snapshot() {
    let (pool, actor, ctx, group) = fixture("results_locked_policy").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    let workspace = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let confirmed = confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&workspace))
        .await
        .unwrap();
    let original_policy = confirmed.policy.id;
    let original_confirmation_version = confirmed.confirmation.as_ref().unwrap().row_version;
    let policy_snapshot = serde_json::to_value(&confirmed.policy).unwrap();
    sqlx::query(
        "INSERT INTO academic_course_result_locks (subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES ($1,$2,$3,$4,$5,$6,$7,'{}'::jsonb,$8)",
    )
    .bind(confirmed.subject_id)
    .bind(ctx.academic_term_id)
    .bind(ctx.academic_year_id)
    .bind(original_policy)
    .bind(policy_snapshot)
    .bind(&confirmed.roster_checksum)
    .bind(&confirmed.source_checksum)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();
    let replacement = create_policy(
        &pool,
        &actor,
        &ctx,
        GradingPolicyInput {
            name: "Later criterion".into(),
            bands: bands(),
        },
    )
    .await
    .unwrap();
    activate_policy(&pool, &actor, &ctx, replacement.id, replacement.row_version)
        .await
        .unwrap();
    let locked = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(locked.locked);
    assert_eq!(locked.policy.id, original_policy);
    assert_eq!(locked.policy.lifecycle, confirmed.policy.lifecycle);
    assert_eq!(locked.policy.row_version, confirmed.policy.row_version);
    assert_eq!(locked.policy.bands, confirmed.policy.bands);
    assert!(locked.confirmation_is_current);
    assert_eq!(
        locked.confirmation.as_ref().unwrap().row_version,
        original_confirmation_version
    );
}
// Catches blank-as-pass, partial activity batches, assignment/primary bypass and roster approval resurrection.
#[tokio::test]
async fn results_activity_completeness_assignment_and_roster_staleness() {
    let (pool, mut actor, ctx, _) = fixture("results_activity").await;
    let (group,teacher):(Uuid,Uuid)=sqlx::query_as("SELECT g.id,t.teacher_id FROM learning_groups g JOIN activity_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1").fetch_one(&pool).await.unwrap();
    actor.user_id = teacher;
    let ws = get_activity_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let input = |w: &ActivityPreparationWorkspace| ResultConfirmationInput {
        source_checksum: w.source_checksum.clone(),
        roster_checksum: w.roster_checksum.clone(),
        row_version: w.confirmation.as_ref().map(|c| c.row_version),
    };
    let cells = ws
        .students
        .iter()
        .map(|s| ActivityCellInput {
            student_academic_year_id: s.student_academic_year_id,
            outcome: None,
            row_version: s.row_version,
        })
        .collect();
    let blank = save_activity_outcomes(&pool, &actor, &ctx, group, ActivityBatchInput { cells })
        .await
        .unwrap();
    assert!(blank
        .blockers
        .iter()
        .any(|b| b.code == ResultBlockerCode::MissingActivityOutcome));
    assert!(
        !confirm_activity(&pool, &actor, &ctx, group, input(&blank))
            .await
            .unwrap()
            .confirmation_is_current
    );
    let cells = blank
        .students
        .iter()
        .map(|s| ActivityCellInput {
            student_academic_year_id: s.student_academic_year_id,
            outcome: Some(ActivityOutcome::Fail),
            row_version: None,
        })
        .collect();
    let filled = save_activity_outcomes(&pool, &actor, &ctx, group, ActivityBatchInput { cells })
        .await
        .unwrap();
    let confirmed = confirm_activity(&pool, &actor, &ctx, group, input(&filled))
        .await
        .unwrap();
    assert!(confirmed.confirmation_is_current);
    let sibling: Uuid = sqlx::query_scalar(
        "INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT uuid_generate_v4(),learning_offering_id,academic_term_id,academic_year_id,'RESULT-SKIP','Not ready','draft','draft' FROM learning_groups WHERE id=$1 RETURNING id",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    let queue = readiness(&pool, &actor, &ctx).await.unwrap();
    assert!(
        queue
            .activities
            .iter()
            .find(|g| g.learning_group_id == group)
            .unwrap()
            .ready
    );
    let skipped = queue
        .activities
        .iter()
        .find(|candidate| candidate.learning_group_id == sibling)
        .unwrap();
    assert!(!skipped.ready);
    assert!(skipped
        .blockers
        .iter()
        .any(|item| item.code == ResultBlockerCode::MissingPrimaryTeacher));
    assert!(skipped
        .blockers
        .iter()
        .any(|item| item.code == ResultBlockerCode::MissingGroupConfirmation));
    sqlx::query("INSERT INTO learning_group_teachers (id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on) SELECT uuid_generate_v4(),$1,academic_term_id,academic_year_id,teacher_id,role,starts_on FROM learning_group_teachers WHERE learning_group_id=$2 AND teacher_id=$3")
        .bind(sibling)
        .bind(group)
        .bind(actor.user_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO learning_group_students (id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,row_version) SELECT uuid_generate_v4(),$1,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,row_version FROM learning_group_students WHERE learning_group_id=$2 AND membership_status='active'")
        .bind(sibling)
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    let sibling_workspace = get_activity_workspace(&pool, &actor, &ctx, sibling)
        .await
        .unwrap();
    let sibling_cells = sibling_workspace
        .students
        .iter()
        .map(|student| ActivityCellInput {
            student_academic_year_id: student.student_academic_year_id,
            outcome: Some(ActivityOutcome::Pass),
            row_version: None,
        })
        .collect();
    let sibling_workspace = save_activity_outcomes(
        &pool,
        &actor,
        &ctx,
        sibling,
        ActivityBatchInput {
            cells: sibling_cells,
        },
    )
    .await
    .unwrap();
    let sibling_workspace =
        confirm_activity(&pool, &actor, &ctx, sibling, input(&sibling_workspace))
            .await
            .unwrap();
    assert!(sibling_workspace.confirmation_is_current);
    let sibling_group = delivery_groups::get(&pool, sibling).await.unwrap();
    let secondary = delivery_groups::replace_teachers(
        &pool,
        actor.user_id,
        sibling,
        ReplaceLearningGroupTeachersRequest {
            row_version: sibling_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: actor.user_id,
                role: LearningTeacherRole::Secondary,
            }],
        },
    )
    .await
    .unwrap();
    let no_primary = get_activity_workspace(&pool, &actor, &ctx, sibling)
        .await
        .unwrap();
    assert!(!no_primary.confirmation_is_current);
    assert!(no_primary
        .blockers
        .iter()
        .any(|item| item.code == ResultBlockerCode::MissingPrimaryTeacher));
    delivery_groups::replace_teachers(
        &pool,
        actor.user_id,
        sibling,
        ReplaceLearningGroupTeachersRequest {
            row_version: secondary.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: actor.user_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let restored_primary = get_activity_workspace(&pool, &actor, &ctx, sibling)
        .await
        .unwrap();
    assert!(!restored_primary.confirmation_is_current);
    let restored_confirmation =
        confirm_activity(&pool, &actor, &ctx, sibling, input(&restored_primary))
            .await
            .unwrap();
    assert!(restored_confirmation.confirmation_is_current);
    add_then_remove_membership_without_result_read(&pool, actor.user_id, group).await;
    let stale = get_activity_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(!stale.confirmation_is_current);
    assert!(
        stale.confirmation.as_ref().unwrap().row_version
            > confirmed.confirmation.as_ref().unwrap().row_version
    );
    let other = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![codes::ACADEMIC_RESULT_MANAGE_SCHOOL.into()],
    };
    assert!(confirm_activity(&pool, &other, &ctx, group, input(&stale))
        .await
        .is_err());
    let invalid = ActivityBatchInput {
        cells: vec![
            ActivityCellInput {
                student_academic_year_id: stale.students[0].student_academic_year_id,
                outcome: Some(ActivityOutcome::Pass),
                row_version: stale.students[0].row_version,
            },
            ActivityCellInput {
                student_academic_year_id: Uuid::new_v4(),
                outcome: Some(ActivityOutcome::Pass),
                row_version: None,
            },
        ],
    };
    assert!(save_activity_outcomes(&pool, &actor, &ctx, group, invalid)
        .await
        .is_err());
    assert_eq!(
        get_activity_workspace(&pool, &actor, &ctx, group)
            .await
            .unwrap()
            .students[0]
            .outcome,
        Some(ActivityOutcome::Fail)
    );
}

// School-wide read/management exposes the workspace, but activity data entry
// remains a current assigned-teacher responsibility.
#[tokio::test]
async fn results_activity_entry_requires_current_assignment() {
    let (pool, actor, ctx, _) = fixture("results_activity_assignment").await;
    let source_group: Uuid = sqlx::query_scalar(
        "SELECT g.id FROM learning_groups g JOIN activity_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let group: Uuid = sqlx::query_scalar(
        "INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT uuid_generate_v4(),learning_offering_id,academic_term_id,academic_year_id,'RESULT-NO-ASSIGNMENT','No assignment','draft','draft' FROM learning_groups WHERE id=$1 RETURNING id",
    )
    .bind(source_group)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO learning_group_students (id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,row_version) SELECT uuid_generate_v4(),$1,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,row_version FROM learning_group_students WHERE learning_group_id=$2 AND membership_status='active'",
    )
    .bind(group)
    .bind(source_group)
    .execute(&pool)
    .await
    .unwrap();

    let workspace = get_activity_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(!workspace.can_manage);
    assert!(save_activity_outcomes(
        &pool,
        &actor,
        &ctx,
        group,
        ActivityBatchInput {
            cells: workspace
                .students
                .iter()
                .map(|student| ActivityCellInput {
                    student_academic_year_id: student.student_academic_year_id,
                    outcome: Some(ActivityOutcome::Pass),
                    row_version: None,
                })
                .collect(),
        },
    )
    .await
    .is_err());
}

// Readiness must not call a phase current merely because its stored row was
// never rewritten: membership revisions are part of the phase source.
#[tokio::test]
async fn results_readiness_detects_roster_stale_phase_confirmations() {
    let (pool, actor, ctx, group) = fixture("results_readiness_roster_stale").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    sqlx::query(
        "UPDATE learning_group_students SET row_version=row_version+1 WHERE learning_group_id=$1 AND membership_status='active'",
    )
    .bind(group)
    .execute(&pool)
    .await
    .unwrap();

    let readiness = readiness(&pool, &actor, &ctx).await.unwrap();
    let group = readiness
        .courses
        .iter()
        .flat_map(|subject| &subject.groups)
        .find(|candidate| candidate.learning_group_id == group)
        .unwrap();
    assert!(group
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::StalePhaseConfirmation));
}

// The readiness queue independently verifies the Gradebook source snapshot, so
// a source change cannot be called current merely because its invalidation write
// was missed by an out-of-band repair or import.
#[tokio::test]
async fn results_readiness_detects_score_source_stale_phase_confirmations() {
    let (pool, actor, ctx, group) = fixture("results_readiness_score_stale").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    sqlx::query(
        "UPDATE learning_group_score_items SET max_score=max_score+0.01 WHERE learning_group_id=$1 AND lifecycle='active'",
    )
    .bind(group)
    .execute(&pool)
    .await
    .unwrap();

    let readiness = readiness(&pool, &actor, &ctx).await.unwrap();
    let group = readiness
        .courses
        .iter()
        .flat_map(|subject| &subject.groups)
        .find(|candidate| candidate.learning_group_id == group)
        .unwrap();
    assert!(group
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::StalePhaseConfirmation));
}

// A retained confirmation identity is not approval: readiness compares its
// explicit outcome snapshot to live selections in addition to the monotonic
// invalidation flag.
#[tokio::test]
async fn results_readiness_detects_stale_course_selection_snapshot() {
    let (pool, actor, ctx, group) = fixture("results_readiness_selection_stale").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    let workspace = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let confirmed = confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&workspace))
        .await
        .unwrap();
    assert!(confirmed.confirmation_is_current);
    let student = confirmed.students[0].student_academic_year_id;
    save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::ExplicitZero,
            row_version: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE learning_group_result_confirmations SET source_snapshot=jsonb_set(source_snapshot,'{invalidated}','false') WHERE learning_group_id=$1",
    )
    .bind(group)
    .execute(&pool)
    .await
    .unwrap();

    let readiness = readiness(&pool, &actor, &ctx).await.unwrap();
    let group = readiness
        .courses
        .iter()
        .flat_map(|subject| &subject.groups)
        .find(|candidate| candidate.learning_group_id == group)
        .unwrap();
    assert!(group
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::StaleGroupConfirmation));
}

// A transient participant must permanently stale the activity approval even
// when the active roster and outcome checksum return to their original shape.
#[tokio::test]
async fn results_activity_membership_aba_invalidates_at_the_delivery_boundary() {
    let (pool, mut actor, ctx, _) = fixture("results_activity_membership_aba").await;
    let (group, teacher): (Uuid, Uuid) = sqlx::query_as(
        "SELECT g.id,t.teacher_id FROM learning_groups g JOIN activity_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    actor.user_id = teacher;
    let workspace = get_activity_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let prepared = save_activity_outcomes(
        &pool,
        &actor,
        &ctx,
        group,
        ActivityBatchInput {
            cells: workspace
                .students
                .iter()
                .map(|student| ActivityCellInput {
                    student_academic_year_id: student.student_academic_year_id,
                    outcome: Some(ActivityOutcome::Pass),
                    row_version: student.row_version,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    let confirmed = confirm_activity(
        &pool,
        &actor,
        &ctx,
        group,
        ResultConfirmationInput {
            source_checksum: prepared.source_checksum,
            roster_checksum: prepared.roster_checksum,
            row_version: prepared
                .confirmation
                .as_ref()
                .map(|value| value.row_version),
        },
    )
    .await
    .unwrap();
    let original_version = confirmed.confirmation.as_ref().unwrap().row_version;

    add_then_remove_membership_without_result_read(&pool, actor.user_id, group).await;

    let row: (i64, bool) = sqlx::query_as(
        "SELECT row_version,COALESCE((source_snapshot->>'invalidated')::boolean,false) FROM academic_activity_result_confirmations WHERE learning_group_id=$1",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.1, "the delivery mutation must persist invalidation");
    assert!(row.0 > original_version);
    assert!(
        !get_activity_workspace(&pool, &actor, &ctx, group)
            .await
            .unwrap()
            .confirmation_is_current
    );
}

// The same delivery roster source feeds Gradebook phase, course-result, and
// learner-evaluation confirmations. All retain their identity but must become
// explicitly stale before an ABA roster can restore its old checksum.
#[tokio::test]
async fn results_course_membership_aba_invalidates_phase_result_and_learner_confirmations() {
    let (pool, actor, ctx, group) = fixture("results_course_membership_aba").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    let workspace = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let confirmed = confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&workspace))
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_evaluation_confirmations
           (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,domain,roster_checksum,source_checksum,source_snapshot,confirmed_by)
           SELECT g.id,g.learning_offering_id,g.academic_term_id,g.academic_year_id,d.subject_id,
                  'desirable_characteristic',repeat('0',64),repeat('1',64),'{"invalidated":false}'::jsonb,$2
           FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id
           WHERE g.id=$1"#,
    )
    .bind(group)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();
    let result_version = confirmed.confirmation.as_ref().unwrap().row_version;

    add_then_remove_membership_without_result_read(&pool, actor.user_id, group).await;

    let invalidated_phase_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_phase_confirmations WHERE learning_group_id=$1 AND COALESCE((source_snapshot->>'invalidated')::boolean,false)",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(invalidated_phase_count, 4);
    let result_row: (i64, bool) = sqlx::query_as(
        "SELECT row_version,COALESCE((source_snapshot->>'invalidated')::boolean,false) FROM learning_group_result_confirmations WHERE learning_group_id=$1",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(result_row.1);
    assert!(result_row.0 > result_version);
    let learner_invalidated: bool = sqlx::query_scalar(
        "SELECT COALESCE((source_snapshot->>'invalidated')::boolean,false) FROM learning_group_evaluation_confirmations WHERE learning_group_id=$1 AND domain='desirable_characteristic'",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(learner_invalidated);
    assert!(
        !get_course_workspace(&pool, &actor, &ctx, group)
            .await
            .unwrap()
            .confirmation_is_current
    );
}

#[tokio::test]
async fn results_readiness_returns_course_blockers_after_primary_removal() {
    let (pool, actor, ctx, group) = fixture("results_readiness_course_primary_removed").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    let workspace = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&workspace))
        .await
        .unwrap();
    sqlx::query("UPDATE learning_groups SET status='draft' WHERE id=$1")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE learning_group_teachers SET role='secondary' WHERE learning_group_id=$1 AND teacher_id=$2",
    )
    .bind(group)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();

    let readiness = readiness(&pool, &actor, &ctx).await.unwrap();
    let row = readiness
        .courses
        .iter()
        .flat_map(|subject| &subject.groups)
        .find(|candidate| candidate.learning_group_id == group)
        .unwrap();
    assert!(row
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::MissingPrimaryTeacher));
    assert!(row
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::StaleGroupConfirmation));
}

#[tokio::test]
async fn results_readiness_returns_activity_blockers_after_primary_removal() {
    let (pool, mut actor, ctx, _) = fixture("results_readiness_activity_primary_removed").await;
    let (group, teacher): (Uuid, Uuid) = sqlx::query_as(
        "SELECT g.id,t.teacher_id FROM learning_groups g JOIN activity_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id WHERE t.role='primary' AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=g.id AND m.membership_status='active') ORDER BY g.id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    actor.user_id = teacher;
    let workspace = get_activity_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let prepared = save_activity_outcomes(
        &pool,
        &actor,
        &ctx,
        group,
        ActivityBatchInput {
            cells: workspace
                .students
                .iter()
                .map(|student| ActivityCellInput {
                    student_academic_year_id: student.student_academic_year_id,
                    outcome: Some(ActivityOutcome::Pass),
                    row_version: student.row_version,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    confirm_activity(
        &pool,
        &actor,
        &ctx,
        group,
        ResultConfirmationInput {
            source_checksum: prepared.source_checksum,
            roster_checksum: prepared.roster_checksum,
            row_version: prepared
                .confirmation
                .as_ref()
                .map(|value| value.row_version),
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE learning_groups SET status='draft' WHERE id=$1")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE learning_group_teachers SET role='secondary' WHERE learning_group_id=$1 AND teacher_id=$2",
    )
    .bind(group)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();

    let readiness = readiness(&pool, &actor, &ctx).await.unwrap();
    let row = readiness
        .activities
        .iter()
        .find(|candidate| candidate.learning_group_id == group)
        .unwrap();
    assert!(row
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::MissingPrimaryTeacher));
    assert!(row
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::StaleGroupConfirmation));
}

// The GET starts from an older source snapshot while the retained confirmation
// is protected. Restoring the newer source must not let that stale reader write
// an invalidation after the lock is released.
#[tokio::test]
async fn results_stale_workspace_get_cannot_invalidate_newer_confirmation() {
    let (pool, actor, ctx, group) = fixture("results_stale_workspace_get_race").await;
    prepare_phases(&pool, &actor, &ctx, group).await;
    let workspace = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    let student = workspace.students[0].student_academic_year_id;
    let selected = save_selection(
        &pool,
        &actor,
        &ctx,
        group,
        SelectionInput {
            student_academic_year_id: student,
            selection: CourseOutcomeSelection::ExplicitZero,
            row_version: None,
        },
    )
    .await
    .unwrap();
    let confirmed = confirm_group_results(&pool, &actor, &ctx, group, confirm_input(&selected))
        .await
        .unwrap();
    let confirmation_version = confirmed.confirmation.as_ref().unwrap().row_version;
    let selection_version = confirmed.students[0].selection_row_version.unwrap();
    sqlx::query(
        "DELETE FROM learning_group_result_overrides WHERE learning_group_id=$1 AND student_academic_year_id=$2",
    )
    .bind(group)
    .bind(student)
    .execute(&pool)
    .await
    .unwrap();

    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE learning_group_result_confirmations IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let get_pool = pool.clone();
    let get_actor = actor.clone();
    let get_ctx = ctx;
    let stale_get = tokio::spawn(async move {
        get_course_workspace(&get_pool, &get_actor, &get_ctx, group)
            .await
            .unwrap()
    });
    wait_for_result_confirmation_read_lock(&pool).await;
    sqlx::query(
        "INSERT INTO learning_group_result_overrides (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,student_academic_year_id,outcome,row_version,updated_by) SELECT g.id,g.learning_offering_id,g.academic_term_id,g.academic_year_id,d.subject_id,$2,'manual_zero',$3,$4 FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1",
    )
    .bind(group)
    .bind(student)
    .bind(selection_version)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();
    lock.commit().await.unwrap();
    let stale = stale_get.await.unwrap();
    assert!(!stale.confirmation_is_current);
    let refreshed = get_course_workspace(&pool, &actor, &ctx, group)
        .await
        .unwrap();
    assert!(refreshed.confirmation_is_current);
    assert_eq!(
        refreshed.confirmation.as_ref().unwrap().row_version,
        confirmation_version
    );
}

// Exercises the owning handler declarations. Aggregate OpenAPI registration remains Task 9.
#[test]
fn results_endpoint_contract_requires_exact_context_and_typed_envelopes() {
    use utoipa::OpenApi;

    #[derive(OpenApi)]
    #[openapi(paths(
        super::handlers::list_policies,
        super::handlers::create_policy,
        super::handlers::activate_policy,
        super::handlers::get_course_workspace,
        super::handlers::save_selection,
        super::handlers::confirm_group_results,
        super::handlers::get_activity_workspace,
        super::handlers::save_activity_outcomes,
        super::handlers::confirm_activity,
        super::handlers::readiness,
    ))]
    struct ResultPaths;

    let contract = serde_json::to_value(ResultPaths::openapi()).unwrap();
    for item in contract["paths"].as_object().unwrap().values() {
        for operation in item.as_object().unwrap().values() {
            let parameters = operation["parameters"].as_array().unwrap();
            for name in ["academicYearId", "academicTermId"] {
                let parameter = parameters
                    .iter()
                    .find(|value| value["name"] == name)
                    .unwrap();
                assert_eq!(parameter["in"], "query");
                assert_eq!(parameter["required"], true);
            }
            assert!(
                operation["responses"]["200"]["content"]["application/json"]["schema"]
                    .to_string()
                    .contains("ApiResponse")
            );
        }
    }
}
