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
    error::AppError, middleware::permission::ActorContext, permissions::registry::codes,
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
    let result_id = Uuid::new_v4();
    assert!(
        serde_json::from_value::<ResultCorrectionInput>(serde_json::json!({
            "kind": "course",
            "courseResultId": result_id,
            "outcome": "incomplete",
            "numericGrade": null,
            "expectedEffectiveVersion": 1
        }))
        .is_ok()
    );
    assert!(
        serde_json::from_value::<ResultCorrectionInput>(serde_json::json!({
            "kind": "course",
            "courseResultId": result_id,
            "outcome": "incomplete",
            "numericGrade": null,
            "expectedEffectiveVersion": 1,
            "remark": "must not enter the correction contract"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ResultCorrectionInput>(serde_json::json!({
            "kind": "learner_evaluation",
            "subjectStudentEvaluationId": result_id,
            "qualityLevel": 4,
            "expectedEffectiveVersion": 1
        }))
        .is_err()
    );
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

// Result preparation uses the same bounded readiness workspace as academic affairs, but an
// assigned teacher must only receive groups that the resource policy authorizes.
#[tokio::test]
async fn results_readiness_is_scoped_for_assigned_teachers() {
    let (pool, mut actor, ctx, _) = fixture("results_readiness_assigned_scope").await;
    actor.permissions = vec![codes::ACADEMIC_RESULT_READ_ASSIGNED.into()];

    let queue = readiness(&pool, &actor, &ctx).await.unwrap();

    assert!(!queue.courses.is_empty());
    assert!(queue
        .courses
        .iter()
        .flat_map(|subject| subject.groups.iter())
        .all(|group| group.assigned));
    assert!(queue.activities.iter().all(|group| group.assigned));
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

fn academic_affairs_actor(actor: &ActorContext) -> ActorContext {
    ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
            codes::ACADEMIC_RESULT_LOCK_SCHOOL.into(),
            codes::ACADEMIC_RESULT_CORRECT_SCHOOL.into(),
        ],
    }
}

async fn course_subject_rooms(
    pool: &sqlx::PgPool,
    ctx: &ResultContext,
    subject: Uuid,
) -> Vec<(Uuid, Uuid)> {
    sqlx::query_as(
        r#"SELECT learning_group.id, teacher.teacher_id
           FROM learning_groups learning_group
           JOIN course_offering_details detail
             ON detail.learning_offering_id = learning_group.learning_offering_id
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
            AND teacher.role = 'primary'
           WHERE detail.subject_id = $1
             AND learning_group.academic_term_id = $2
             AND learning_group.academic_year_id = $3
             AND learning_group.status <> 'closed'
           ORDER BY learning_group.id, teacher.id"#,
    )
    .bind(subject)
    .bind(ctx.academic_term_id)
    .bind(ctx.academic_year_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn prepare_course_subject(
    pool: &sqlx::PgPool,
    base_actor: &ActorContext,
    ctx: &ResultContext,
    subject: Uuid,
) -> Vec<CoursePreparationWorkspace> {
    let mut workspaces = Vec::new();
    for (group, teacher) in course_subject_rooms(pool, ctx, subject).await {
        let teacher_actor = ActorContext {
            user_id: teacher,
            permissions: base_actor.permissions.clone(),
        };
        prepare_phases(pool, &teacher_actor, ctx, group).await;
        let workspace = get_course_workspace(pool, &teacher_actor, ctx, group)
            .await
            .unwrap();
        let confirmed =
            confirm_group_results(pool, &teacher_actor, ctx, group, confirm_input(&workspace))
                .await
                .unwrap();
        assert!(confirmed.confirmation_is_current);
        workspaces.push(confirmed);
    }
    workspaces
}

async fn prepare_activity_group(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    ctx: &ResultContext,
    group: Uuid,
) -> ActivityPreparationWorkspace {
    let workspace = get_activity_workspace(pool, actor, ctx, group)
        .await
        .unwrap();
    let cells = workspace
        .students
        .iter()
        .map(|student| ActivityCellInput {
            student_academic_year_id: student.student_academic_year_id,
            outcome: Some(ActivityOutcome::Pass),
            row_version: student.row_version,
        })
        .collect();
    let workspace = save_activity_outcomes(pool, actor, ctx, group, ActivityBatchInput { cells })
        .await
        .unwrap();
    let confirmed = confirm_activity(
        pool,
        actor,
        ctx,
        group,
        ResultConfirmationInput {
            source_checksum: workspace.source_checksum.clone(),
            roster_checksum: workspace.roster_checksum.clone(),
            row_version: workspace
                .confirmation
                .as_ref()
                .map(|confirmation| confirmation.row_version),
        },
    )
    .await
    .unwrap();
    assert!(confirmed.confirmation_is_current);
    confirmed
}

// Catches an activity lock widening from its one group to another group in the same offering.
#[tokio::test]
async fn results_activity_groups_lock_independently() {
    let (pool, mut teacher, ctx, _) = fixture("results_activity_lock_independent").await;
    let (group, primary): (Uuid, Uuid) = sqlx::query_as(
        "SELECT learning_group.id,teacher.teacher_id FROM learning_groups learning_group JOIN activity_offering_details detail ON detail.learning_offering_id=learning_group.learning_offering_id JOIN learning_group_teachers teacher ON teacher.learning_group_id=learning_group.id AND teacher.role='primary' WHERE EXISTS(SELECT 1 FROM learning_group_students member WHERE member.learning_group_id=learning_group.id AND member.membership_status='active') ORDER BY learning_group.id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    teacher.user_id = primary;
    prepare_activity_group(&pool, &teacher, &ctx, group).await;
    let sibling: Uuid = sqlx::query_scalar(
        "INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT uuid_generate_v4(),learning_offering_id,academic_term_id,academic_year_id,'ACTIVITY-UNLOCKED','Unlocked sibling','draft','draft' FROM learning_groups WHERE id=$1 RETURNING id",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();

    let outcome = lock_activity_group(&pool, &academic_affairs_actor(&teacher), &ctx, group)
        .await
        .unwrap();
    let lock = outcome.lock.expect("the complete activity group must lock");
    assert!(outcome.blockers.is_empty());
    assert!(lock.result_count > 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_activity_result_locks WHERE learning_group_id=$1",
        )
        .bind(group)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_activity_result_locks WHERE learning_group_id=$1",
        )
        .bind(sibling)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

// Catches activity corrections mutating the locked initial result or sharing course-grade rules.
#[tokio::test]
async fn results_activity_corrections_are_append_only_and_group_scoped() {
    let (pool, mut teacher, ctx, _) = fixture("results_activity_correction").await;
    let (group, primary): (Uuid, Uuid) = sqlx::query_as(
        "SELECT learning_group.id,teacher.teacher_id FROM learning_groups learning_group JOIN activity_offering_details detail ON detail.learning_offering_id=learning_group.learning_offering_id JOIN learning_group_teachers teacher ON teacher.learning_group_id=learning_group.id AND teacher.role='primary' WHERE EXISTS(SELECT 1 FROM learning_group_students member WHERE member.learning_group_id=learning_group.id AND member.membership_status='active') ORDER BY learning_group.id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    teacher.user_id = primary;
    prepare_activity_group(&pool, &teacher, &ctx, group).await;
    lock_activity_group(&pool, &academic_affairs_actor(&teacher), &ctx, group)
        .await
        .unwrap();
    let result_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_activity_results WHERE learning_group_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    let initial: (String, i64) =
        sqlx::query_as("SELECT outcome,row_version FROM academic_activity_results WHERE id=$1")
            .bind(result_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let corrected = correct_result(
        &pool,
        &academic_affairs_actor(&teacher),
        &ctx,
        ResultCorrectionInput::Activity {
            activity_result_id: result_id,
            outcome: ActivityOutcome::Fail,
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(corrected.effective_version, 2);
    assert_eq!(
        corrected.initial,
        EffectiveResultValue::Activity {
            outcome: ActivityOutcome::Pass
        }
    );
    assert_eq!(
        corrected.effective,
        EffectiveResultValue::Activity {
            outcome: ActivityOutcome::Fail
        }
    );
    assert_eq!(corrected.corrections.len(), 1);
    assert!(matches!(
        correct_result(
            &pool,
            &academic_affairs_actor(&teacher),
            &ctx,
            ResultCorrectionInput::Activity {
                activity_result_id: result_id,
                outcome: ActivityOutcome::Pass,
                expected_effective_version: 1,
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        sqlx::query_as::<_, (String, i64)>(
            "SELECT outcome,row_version FROM academic_activity_results WHERE id=$1",
        )
        .bind(result_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        initial
    );
}

// Catches the bulk activity action silently locking an incomplete group instead of returning its
// actionable readiness blockers.
#[tokio::test]
async fn results_bulk_activity_lock_skips_groups_that_are_not_ready() {
    let (pool, mut teacher, ctx, _) = fixture("results_activity_lock_bulk").await;
    let (ready_group, primary): (Uuid, Uuid) = sqlx::query_as(
        "SELECT learning_group.id,teacher.teacher_id FROM learning_groups learning_group JOIN activity_offering_details detail ON detail.learning_offering_id=learning_group.learning_offering_id JOIN learning_group_teachers teacher ON teacher.learning_group_id=learning_group.id AND teacher.role='primary' WHERE EXISTS(SELECT 1 FROM learning_group_students member WHERE member.learning_group_id=learning_group.id AND member.membership_status='active') ORDER BY learning_group.id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    teacher.user_id = primary;
    prepare_activity_group(&pool, &teacher, &ctx, ready_group).await;
    let incomplete_group: Uuid = sqlx::query_scalar(
        "INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT uuid_generate_v4(),learning_offering_id,academic_term_id,academic_year_id,'ACTIVITY-INCOMPLETE','Incomplete activity','draft','draft' FROM learning_groups WHERE id=$1 RETURNING id",
    )
    .bind(ready_group)
    .fetch_one(&pool)
    .await
    .unwrap();

    let outcome = lock_all_ready_activities(&pool, &academic_affairs_actor(&teacher), &ctx)
        .await
        .unwrap();
    assert!(outcome
        .locked
        .iter()
        .any(|lock| lock.learning_group_id == ready_group));
    let skipped = outcome
        .skipped
        .iter()
        .find(|group| group.learning_group_id == incomplete_group)
        .expect("the incomplete group must remain visible in the bulk result");
    assert!(skipped
        .blockers
        .iter()
        .any(|blocker| blocker.code == ResultBlockerCode::MissingPrimaryTeacher));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_activity_result_locks WHERE learning_group_id=$1",
        )
        .bind(incomplete_group)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

// Catches a subject lock committing one ready room while another room is missing a current
// primary-teacher result confirmation.
#[tokio::test]
async fn results_course_lock_is_all_room_atomic() {
    let (pool, actor, ctx, group) = fixture("results_course_lock_atomic").await;
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_id FROM course_offering_details WHERE learning_offering_id=(SELECT learning_offering_id FROM learning_groups WHERE id=$1)",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut rooms = course_subject_rooms(&pool, &ctx, subject).await;
    if rooms.len() == 1 {
        let other: Uuid = sqlx::query_scalar(
            "INSERT INTO learning_groups (id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT uuid_generate_v4(),learning_offering_id,academic_term_id,academic_year_id,'COURSE-LOCK-SECOND','Second room','draft','draft' FROM learning_groups WHERE id=$1 RETURNING id",
        )
        .bind(group)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO learning_group_teachers (id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on) SELECT uuid_generate_v4(),$1,academic_term_id,academic_year_id,teacher_id,role,starts_on FROM learning_group_teachers WHERE learning_group_id=$2 AND role='primary' ORDER BY id LIMIT 1",
        )
        .bind(other)
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
        rooms = course_subject_rooms(&pool, &ctx, subject).await;
    }
    assert!(rooms.len() > 1);

    let (ready_group, ready_teacher) = rooms[0];
    let ready_actor = ActorContext {
        user_id: ready_teacher,
        permissions: actor.permissions.clone(),
    };
    prepare_phases(&pool, &ready_actor, &ctx, ready_group).await;
    let workspace = get_course_workspace(&pool, &ready_actor, &ctx, ready_group)
        .await
        .unwrap();
    confirm_group_results(
        &pool,
        &ready_actor,
        &ctx,
        ready_group,
        confirm_input(&workspace),
    )
    .await
    .unwrap();

    let outcome = lock_course_subject(&pool, &academic_affairs_actor(&actor), &ctx, subject)
        .await
        .unwrap();
    assert!(outcome.lock.is_none());
    assert!(outcome.groups.iter().any(|candidate| {
        candidate.learning_group_id != ready_group
            && candidate.blockers.iter().any(|blocker| {
                matches!(
                    blocker.code,
                    ResultBlockerCode::MissingPhaseConfirmation
                        | ResultBlockerCode::MissingGroupConfirmation
                )
            })
    }));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_course_result_locks WHERE subject_id=$1 AND academic_term_id=$2",
        )
        .bind(subject)
        .bind(ctx.academic_term_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_course_results WHERE subject_id=$1 AND academic_term_id=$2",
        )
        .bind(subject)
        .bind(ctx.academic_term_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

// Catches a successful subject lock omitting a room/student or failing to preserve the exact
// policy and preparation sources used to create immutable initial results.
#[tokio::test]
async fn results_course_lock_snapshots_all_initial_results() {
    let (pool, actor, ctx, group) = fixture("results_course_lock_snapshot").await;
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_id FROM course_offering_details WHERE learning_offering_id=(SELECT learning_offering_id FROM learning_groups WHERE id=$1)",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    let prepared = prepare_course_subject(&pool, &actor, &ctx, subject).await;
    assert!(!prepared.is_empty());

    let outcome = lock_course_subject(&pool, &academic_affairs_actor(&actor), &ctx, subject)
        .await
        .unwrap();
    assert!(outcome.groups.iter().all(|room| room.ready));
    let lock = outcome
        .lock
        .expect("every prepared room must lock together");
    let expected_students: i64 = sqlx::query_scalar(
        r#"SELECT count(DISTINCT membership.student_academic_year_id)::bigint
           FROM learning_group_students membership
           JOIN learning_groups learning_group ON learning_group.id=membership.learning_group_id
           JOIN course_offering_details detail
             ON detail.learning_offering_id=learning_group.learning_offering_id
           WHERE detail.subject_id=$1
             AND membership.academic_term_id=$2
             AND membership.academic_year_id=$3
             AND membership.membership_status='active'
             AND learning_group.status<>'closed'"#,
    )
    .bind(subject)
    .bind(ctx.academic_term_id)
    .bind(ctx.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(i64::from(lock.result_count), expected_students);
    let stored_results: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_course_results WHERE course_result_lock_id=$1",
    )
    .bind(lock.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored_results, expected_students);

    let (policy_snapshot, source_snapshot): (serde_json::Value, serde_json::Value) =
        sqlx::query_as(
            "SELECT policy_snapshot,source_snapshot FROM academic_course_result_locks WHERE id=$1",
        )
        .bind(lock.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        policy_snapshot
            .get("id")
            .and_then(serde_json::Value::as_str),
        Some(lock.policy_version_id.to_string().as_str())
    );
    assert_eq!(
        source_snapshot
            .get("groups")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(prepared.len())
    );
}

// Catches correction code overwriting immutable initial course results or Gradebook scores,
// accepting a stale effective version, or widening correction access to a teacher.
#[tokio::test]
async fn results_course_corrections_append_and_preserve_initial_sources() {
    let (pool, actor, ctx, group) = fixture("results_course_correction").await;
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_id FROM course_offering_details WHERE learning_offering_id=(SELECT learning_offering_id FROM learning_groups WHERE id=$1)",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    prepare_course_subject(&pool, &actor, &ctx, subject).await;
    lock_course_subject(&pool, &academic_affairs_actor(&actor), &ctx, subject)
        .await
        .unwrap();
    let course_result_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_course_results WHERE subject_id=$1 AND academic_term_id=$2 ORDER BY id LIMIT 1",
    )
    .bind(subject)
    .bind(ctx.academic_term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let initial: (String, Option<String>, String, String, i64) = sqlx::query_as(
        "SELECT outcome,numeric_grade::text,calculated_score::text,calculated_grade::text,row_version FROM academic_course_results WHERE id=$1",
    )
    .bind(course_result_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let score_sources: (i64, Option<String>) = sqlx::query_as(
        "SELECT count(*)::bigint,sum(score)::text FROM learning_group_student_scores WHERE learning_group_id=(SELECT learning_group_id FROM academic_course_results WHERE id=$1)",
    )
    .bind(course_result_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let denied = correct_result(
        &pool,
        &actor,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id,
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("0.50".into()),
            expected_effective_version: 1,
        },
    )
    .await;
    assert!(matches!(denied, Err(AppError::Forbidden(_))));

    let correction_actor = academic_affairs_actor(&actor);
    let corrected = correct_result(
        &pool,
        &correction_actor,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id,
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("0.50".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(corrected.effective_version, 2);
    assert_eq!(corrected.corrections.len(), 1);
    assert_eq!(
        corrected.effective,
        EffectiveResultValue::Course {
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("0.50".into())
        }
    );
    assert!(matches!(
        corrected.initial,
        EffectiveResultValue::Course { .. }
    ));

    let stale = correct_result(
        &pool,
        &correction_actor,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id,
            outcome: CourseOfficialOutcome::Incomplete,
            numeric_grade: None,
            expected_effective_version: 1,
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_result_corrections WHERE course_result_id=$1",
        )
        .bind(course_result_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>, String, String, i64)>(
            "SELECT outcome,numeric_grade::text,calculated_score::text,calculated_grade::text,row_version FROM academic_course_results WHERE id=$1",
        )
        .bind(course_result_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        initial
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT count(*)::bigint,sum(score)::text FROM learning_group_student_scores WHERE learning_group_id=(SELECT learning_group_id FROM academic_course_results WHERE id=$1)",
        )
        .bind(course_result_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        score_sources
    );

    let search = search_effective_results(
        &pool,
        &correction_actor,
        &EffectiveResultSearch {
            academic_year_id: ctx.academic_year_id,
            academic_term_id: ctx.academic_term_id,
            kind: Some(EffectiveResultKind::Course),
            search: None,
            limit: Some(20),
        },
    )
    .await
    .unwrap();
    let item = search
        .iter()
        .find(|item| item.result.result_id == course_result_id)
        .expect("corrected course result must be searchable");
    assert_eq!(item.result.effective_version, 2);
    assert_eq!(item.result.corrections.len(), 1);
    assert_eq!(item.result.effective, corrected.effective);
    let wire = serde_json::to_string(item).unwrap();
    assert!(!wire.to_ascii_lowercase().contains("national"));

    let incomplete = correct_result(
        &pool,
        &correction_actor,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id,
            outcome: CourseOfficialOutcome::Incomplete,
            numeric_grade: None,
            expected_effective_version: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(incomplete.effective_version, 3);
    let refreshed = search_effective_results(
        &pool,
        &correction_actor,
        &EffectiveResultSearch {
            academic_year_id: ctx.academic_year_id,
            academic_term_id: ctx.academic_term_id,
            kind: Some(EffectiveResultKind::Course),
            search: None,
            limit: Some(20),
        },
    )
    .await
    .unwrap();
    let refreshed = refreshed
        .iter()
        .find(|item| item.result.result_id == course_result_id)
        .unwrap();
    assert_eq!(refreshed.result.effective_version, 3);
    assert_eq!(refreshed.result.corrections.len(), 2);
    assert_eq!(
        refreshed.result.effective,
        EffectiveResultValue::Course {
            outcome: CourseOfficialOutcome::Incomplete,
            numeric_grade: None
        }
    );
    assert!(matches!(
        search_effective_results(
            &pool,
            &correction_actor,
            &EffectiveResultSearch {
                academic_year_id: ctx.academic_year_id,
                academic_term_id: ctx.academic_term_id,
                kind: None,
                search: None,
                limit: Some(101),
            },
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
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

// Catches policy activation bypassing the offering/group lock order used by Assessment,
// Gradebook, result preparation, and the initial result lock.
#[tokio::test]
async fn results_policy_activation_serializes_with_course_source_writers() {
    let (pool, actor, ctx, group) = fixture("results_policy_activation_serializes").await;
    let policy = create_policy(
        &pool,
        &actor,
        &ctx,
        GradingPolicyInput {
            name: "Serialized criterion".into(),
            bands: bands(),
        },
    )
    .await
    .unwrap();
    let offering: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id=$1")
            .bind(group)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut source_writer = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM learning_offerings WHERE id=$1 FOR UPDATE")
        .bind(offering)
        .execute(&mut *source_writer)
        .await
        .unwrap();

    let activation_pool = pool.clone();
    let activation_actor = actor.clone();
    let activation_context = ctx;
    let activation = tokio::spawn(async move {
        activate_policy(
            &activation_pool,
            &activation_actor,
            &activation_context,
            policy.id,
            policy.row_version,
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !activation.is_finished(),
        "policy activation must wait for an in-flight course source writer"
    );
    source_writer.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), activation)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
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
        super::handlers::lock_course_subject,
        super::handlers::lock_activity_group,
        super::handlers::lock_all_ready_activities,
        super::handlers::search_effective_results,
        super::handlers::correct_result,
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
