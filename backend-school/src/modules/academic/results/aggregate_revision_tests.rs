use super::{models::*, services::*, services_tests::*};
use crate::modules::academic::learner_evaluation::{
    models::EvaluationContext, services as learner,
};
use uuid::Uuid;

#[tokio::test]
async fn aggregate_batch_providers_preserve_student_boundaries_and_existing_checksums() {
    let (pool, mut actor, ctx, _) = fixture("aggregate_batch_parity").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 66)
        .await
        .unwrap();
    actor.permissions.push(
        crate::permissions::registry::codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into(),
    );
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Batch policy".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let original: Uuid = sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE academic_year_id=$1 AND academic_term_id=$2 AND membership_status='active' ORDER BY student_academic_year_id LIMIT 1")
        .bind(ctx.academic_year_id).bind(ctx.academic_term_id).fetch_one(&pool).await.unwrap();
    let extra_user = Uuid::new_v4();
    let extra_student = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,username,password_hash,first_name,last_name,user_type,status) VALUES ($1,'batch-fixture-learner','fixture-not-a-login','Batch','Fixture','student','active')")
        .bind(extra_user).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years (id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM student_academic_years WHERE id=$3")
        .bind(extra_student).bind(extra_user).bind(original).execute(&pool).await.unwrap();
    let students = vec![original, extra_student];
    let query = TermResultPreviewQuery {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
        passing_grade: "1".into(),
    };
    let lc = EvaluationContext {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
    };
    let mut expected = vec![];
    for student in &students {
        expected.push((
            *student,
            preview_student_term(&pool, &actor, *student, &query)
                .await
                .unwrap(),
            learner::summarize_student_term(&pool, &lc, *student)
                .await
                .unwrap(),
        ));
    }
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    let mut reversed = students.clone();
    reversed.reverse();
    let results = preview_student_terms_in_transaction(&mut tx, &reversed, &query)
        .await
        .unwrap();
    let evaluations = learner::summarize_student_terms_in_transaction(&mut tx, &lc, &reversed)
        .await
        .unwrap();
    assert_eq!(results.len(), students.len());
    assert_eq!(evaluations.len(), students.len());
    assert!(!results[&original].courses.is_empty());
    assert!(results[&extra_student].courses.is_empty());
    assert!(!results[&extra_student].totals.coverage_complete);
    let aggregates = aggregate_students_in_transaction(
        &mut tx,
        &reversed,
        &AggregatePreviewQuery {
            academic_year_id: ctx.academic_year_id,
            academic_term_id: ctx.academic_term_id,
            policy_id: policy.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(aggregates.len(), students.len());
    assert!(!aggregates[&original]
        .blockers
        .contains(&AggregateBlocker::NoCourseCoverage));
    assert!(aggregates[&extra_student]
        .blockers
        .contains(&AggregateBlocker::NoCourseCoverage));
    for (student, result, summary) in expected {
        assert_eq!(
            serde_json::to_value(&results[&student]).unwrap(),
            serde_json::to_value(&result).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&evaluations[&student]).unwrap(),
            serde_json::to_value(&summary).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&aggregates[&student].results).unwrap(),
            serde_json::to_value(&result).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&aggregates[&student].learner_evaluations).unwrap(),
            serde_json::to_value(&summary).unwrap()
        );
    }
    assert!(preview_student_terms_in_transaction(&mut tx, &[], &query)
        .await
        .unwrap()
        .is_empty());
    assert!(
        learner::summarize_student_terms_in_transaction(&mut tx, &lc, &[])
            .await
            .unwrap()
            .is_empty()
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn aggregate_batch_providers_reject_foreign_duplicate_and_oversized_requests() {
    let (pool, _, ctx, _) = fixture("aggregate_batch_context").await;
    let student: Uuid = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(ctx.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let foreign: Uuid = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id<>$1 ORDER BY id LIMIT 1",
    )
    .bind(ctx.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let query = TermResultPreviewQuery {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
        passing_grade: "1".into(),
    };
    let lc = EvaluationContext {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
    };
    let oversized: Vec<Uuid> = (0..501).map(|_| Uuid::new_v4()).collect();
    for students in [
        vec![student, foreign],
        vec![student, Uuid::new_v4()],
        vec![student, student],
        oversized,
    ] {
        let mut tx = pool.begin().await.unwrap();
        assert!(
            preview_student_terms_in_transaction(&mut tx, &students, &query)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert!(
            learner::summarize_student_terms_in_transaction(&mut tx, &lc, &students)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
    }
}

async fn ready_aggregate_fixture(
    name: &str,
) -> (
    sqlx::PgPool,
    crate::middleware::permission::ActorContext,
    ResultContext,
    Uuid,
) {
    use crate::modules::academic::learner_evaluation::models as lm;
    use crate::{middleware::permission::ActorContext, permissions::registry::codes};
    let (pool, mut actor, ctx, group) = fixture(name).await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 66)
        .await
        .unwrap();
    actor.permissions.extend([
        codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into(),
        codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL.into(),
        codes::ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL.into(),
        codes::ACADEMIC_RESULT_LOCK_SCHOOL.into(),
        codes::ACADEMIC_RESULT_CORRECT_SCHOOL.into(),
    ]);
    let student: Uuid = sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' ORDER BY student_academic_year_id LIMIT 1").bind(group).fetch_one(&pool).await.unwrap();
    let preview = preview_student_term(
        &pool,
        &actor,
        student,
        &TermResultPreviewQuery {
            academic_year_id: ctx.academic_year_id,
            academic_term_id: ctx.academic_term_id,
            passing_grade: "1".into(),
        },
    )
    .await
    .unwrap();
    let lc = lm::EvaluationContext {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
    };
    for course in &preview.courses {
        prepare_course_subject(&pool, &actor, &ctx, course.subject_id).await;
        assert!(lock_course_subject(&pool, &actor, &ctx, course.subject_id)
            .await
            .unwrap()
            .lock
            .is_some());
        let rooms: Vec<(Uuid, Uuid)> = sqlx::query_as("SELECT g.id,t.teacher_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_teachers t ON t.learning_group_id=g.id AND t.role='primary' WHERE d.subject_id=$1 AND g.academic_term_id=$2 AND g.status<>'closed' ORDER BY g.id").bind(course.subject_id).bind(ctx.academic_term_id).fetch_all(&pool).await.unwrap();
        for domain in [
            lm::LearnerEvaluationDomain::DesirableCharacteristic,
            lm::LearnerEvaluationDomain::ReadingThinkingWriting,
        ] {
            for (room, teacher) in &rooms {
                let teacher = ActorContext {
                    user_id: *teacher,
                    permissions: actor.permissions.clone(),
                };
                let mut ws = learner::get_workspace(&pool, &teacher, *room, domain, &lc)
                    .await
                    .unwrap();
                let cells: Vec<_> = ws
                    .students
                    .iter()
                    .flat_map(|student| {
                        ws.criteria
                            .iter()
                            .filter(|criterion| criterion.lifecycle == "active")
                            .map(move |criterion| lm::ResponseInput {
                                subject_term_criterion_id: criterion.id,
                                student_academic_year_id: student.student_academic_year_id,
                                quality_level: Some(3.try_into().unwrap()),
                                row_version: None,
                            })
                    })
                    .collect();
                if !cells.is_empty() {
                    ws = learner::save_responses(&pool, &teacher, *room, domain, &lc, cells)
                        .await
                        .unwrap();
                }
                learner::confirm_group(
                    &pool,
                    &teacher,
                    *room,
                    domain,
                    &lc,
                    lm::ConfirmationInput {
                        source_checksum: ws.source_checksum,
                        roster_checksum: ws.roster_checksum,
                        row_version: ws.confirmation.map(|row| row.row_version),
                    },
                )
                .await
                .unwrap();
            }
            let lock = learner::lock_subject(&pool, &actor, course.subject_id, domain, &lc)
                .await
                .unwrap();
            assert!(lock.lock.is_some(), "{:?}", lock.blockers);
        }
    }
    for activity in &preview.activities {
        let teacher: Uuid = sqlx::query_scalar("SELECT teacher_id FROM learning_group_teachers WHERE learning_group_id=$1 AND role='primary' ORDER BY id LIMIT 1").bind(activity.learning_group_id).fetch_one(&pool).await.unwrap();
        let teacher = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        prepare_activity_group(&pool, &teacher, &ctx, activity.learning_group_id).await;
        assert!(
            lock_activity_group(&pool, &actor, &ctx, activity.learning_group_id)
                .await
                .unwrap()
                .lock
                .is_some()
        );
    }
    (pool, actor, ctx, student)
}

#[tokio::test]
async fn aggregate_revisions_retain_history_retry_safely_and_detect_corrections() {
    use crate::error::AppError;
    let (pool, actor, ctx, student) = ready_aggregate_fixture("aggregate_revision_history").await;
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Strict school policy".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let mut query = AggregatePreviewQuery {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
        policy_id: policy.id,
    };
    let preview = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    assert!(preview.can_lock, "{:?}", preview.blockers);
    assert!(preview.hold_findings.is_empty());
    let input = AggregateLockInput {
        policy_id: policy.id,
        source_checksum: preview.source_checksum.clone(),
        expected_revision: None,
        request_id: Uuid::new_v4(),
        hold_reason: None,
    };
    let first = lock_term_aggregate(&pool, &actor, &ctx, student, input.clone())
        .await
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.official_gpa.as_deref(), Some("0.00"));
    assert!(first.is_current);
    let retry = lock_term_aggregate(&pool, &actor, &ctx, student, input.clone())
        .await
        .unwrap();
    assert_eq!(retry.id, first.id);
    let mut wrong = input.clone();
    wrong.expected_revision = Some(1);
    assert!(matches!(
        lock_term_aggregate(&pool, &actor, &ctx, student, wrong).await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        sqlx::query("DELETE FROM academic_term_aggregate_revisions WHERE id=$1")
            .bind(first.id)
            .execute(&pool)
            .await
            .is_err()
    );
    let course = &preview.results.courses[0];
    correct_result(
        &pool,
        &actor,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id: course.result_id.unwrap(),
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let history = list_term_aggregate_revisions(&pool, &actor, &ctx, student)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert!(!history[0].is_current);
    assert_eq!(
        history[0].snapshot.results.courses[0]
            .numeric_grade
            .as_deref(),
        Some("0.00")
    );
    let mut stale = input.clone();
    stale.request_id = Uuid::new_v4();
    stale.expected_revision = Some(1);
    assert!(matches!(
        lock_term_aggregate(&pool, &actor, &ctx, student, stale).await,
        Err(AppError::Conflict(_))
    ));
    let current = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    let second = lock_term_aggregate(
        &pool,
        &actor,
        &ctx,
        student,
        AggregateLockInput {
            source_checksum: current.source_checksum,
            expected_revision: Some(1),
            request_id: Uuid::new_v4(),
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(second.revision, 2);
    assert_eq!(second.official_gpa.as_deref(), Some("4.00"));
    assert!(
        !lock_term_aggregate(&pool, &actor, &ctx, student, input)
            .await
            .unwrap()
            .is_current
    );
    correct_result(
        &pool,
        &actor,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id: course.result_id.unwrap(),
            outcome: CourseOfficialOutcome::Incomplete,
            numeric_grade: None,
            expected_effective_version: 2,
        },
    )
    .await
    .unwrap();
    let strict = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    assert!(!strict.can_lock);
    assert!(strict
        .blockers
        .contains(&AggregateBlocker::ReviewedHoldNotAllowed));
    let hold_policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Reviewed exceptions".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: true,
        },
    )
    .await
    .unwrap();
    query.policy_id = hold_policy.id;
    let held = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    assert!(held.can_lock);
    assert_eq!(
        held.hold_findings,
        vec![AggregateHoldFinding::ExceptionalCourseOutcomes]
    );
    let mut held_input = AggregateLockInput {
        policy_id: hold_policy.id,
        source_checksum: held.source_checksum,
        expected_revision: Some(2),
        request_id: Uuid::new_v4(),
        hold_reason: None,
    };
    assert!(matches!(
        lock_term_aggregate(&pool, &actor, &ctx, student, held_input.clone()).await,
        Err(AppError::ValidationError(_))
    ));
    held_input.hold_reason = Some("รอแก้ผลการเรียนตามที่โรงเรียนพิจารณา".into());
    let held_lock = lock_term_aggregate(&pool, &actor, &ctx, student, held_input)
        .await
        .unwrap();
    assert_eq!(held_lock.revision, 3);
    assert!(held_lock.official_gpa.is_none());
    let history = list_term_aggregate_revisions(&pool, &actor, &ctx, student)
        .await
        .unwrap();
    assert_eq!(history.len(), 3);
    assert!(history[0].is_current);
    assert!(history[1..].iter().all(|row| !row.is_current));
    let criterion = &history[0].snapshot.learner_evaluations.domains[0].subjects[0].criteria[0];
    correct_result(
        &pool,
        &actor,
        &ctx,
        ResultCorrectionInput::LearnerEvaluation {
            subject_student_evaluation_id: criterion.id,
            quality_level: 2.try_into().unwrap(),
            expected_effective_version: criterion.row_version,
        },
    )
    .await
    .unwrap();
    let stale = list_term_aggregate_revisions(&pool, &actor, &ctx, student)
        .await
        .unwrap();
    assert!(
        !stale[0].is_current,
        "learner corrections must invalidate the combined snapshot too"
    );
    assert_eq!(
        stale[0].snapshot.learner_evaluations.domains[0].subjects[0].criteria[0].quality_level,
        3
    );
}

#[tokio::test]
async fn aggregate_concurrent_locks_cannot_both_accept_the_same_revision() {
    use crate::error::AppError;
    let (pool, actor, ctx, student) = ready_aggregate_fixture("aggregate_concurrent_locks").await;
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Concurrent policy".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let query = AggregatePreviewQuery {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
        policy_id: policy.id,
    };
    let preview = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    let first = AggregateLockInput {
        policy_id: policy.id,
        source_checksum: preview.source_checksum,
        expected_revision: None,
        request_id: Uuid::new_v4(),
        hold_reason: None,
    };
    let second = AggregateLockInput {
        request_id: Uuid::new_v4(),
        ..first.clone()
    };
    let (left, right) = tokio::join!(
        lock_term_aggregate(&pool, &actor, &ctx, student, first),
        lock_term_aggregate(&pool, &actor, &ctx, student, second)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let error = if left.is_err() {
        left.unwrap_err()
    } else {
        right.unwrap_err()
    };
    assert!(matches!(error, AppError::Conflict(_)), "{error:?}");
    let history = list_term_aggregate_revisions(&pool, &actor, &ctx, student)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].revision, 1);
    assert!(history[0].is_current);
    let wrong = ResultContext {
        academic_year_id: Uuid::new_v4(),
        ..ctx
    };
    assert!(
        list_term_aggregate_revisions(&pool, &actor, &wrong, student)
            .await
            .is_err()
    );
}

#[test]
fn aggregate_policy_requires_explicit_valid_thresholds() {
    let policy = AggregatePolicyInput {
        name: "Reviewed term policy".into(),
        passing_grade: "1".into(),
        minimum_learner_level: 1,
        allow_reviewed_holds: false,
    };
    assert!(validate_aggregate_policy(&policy).is_ok());
    for threshold in ["0", "0.25", "4.5", "-1", "NaN"] {
        let mut bad = policy.clone();
        bad.passing_grade = threshold.into();
        assert!(validate_aggregate_policy(&bad).is_err(), "{threshold}");
    }
    let mut bad = policy.clone();
    bad.minimum_learner_level = 4;
    assert!(validate_aggregate_policy(&bad).is_err());
    bad = policy;
    bad.name = " ".into();
    assert!(validate_aggregate_policy(&bad).is_err());
}

#[tokio::test]
async fn aggregate_preview_missing_results_cannot_be_locked_even_with_a_hold() {
    use crate::{error::AppError, permissions::registry::codes};
    let (pool, mut actor, ctx, group) = fixture("aggregate_missing_results").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 66)
        .await
        .unwrap();
    actor.permissions.extend([
        codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into(),
        codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL.into(),
        codes::ACADEMIC_RESULT_LOCK_SCHOOL.into(),
    ]);
    let student: Uuid = sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' ORDER BY student_academic_year_id LIMIT 1").bind(group).fetch_one(&pool).await.unwrap();
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Allow reviewed outcomes".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: true,
        },
    )
    .await
    .unwrap();
    let query = AggregatePreviewQuery {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
        policy_id: policy.id,
    };
    let preview = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    assert!(!preview.can_lock);
    assert!(preview
        .blockers
        .contains(&AggregateBlocker::MissingCourseResults));
    assert!(preview
        .blockers
        .contains(&AggregateBlocker::MissingLearnerEvaluations));
    let input = AggregateLockInput {
        policy_id: policy.id,
        source_checksum: preview.source_checksum,
        expected_revision: None,
        request_id: Uuid::new_v4(),
        hold_reason: Some("Reviewed by school".into()),
    };
    assert!(matches!(
        lock_term_aggregate(&pool, &actor, &ctx, student, input).await,
        Err(AppError::Conflict(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_term_aggregate_revisions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let history = list_term_aggregate_revisions(&pool, &actor, &ctx, student)
        .await
        .unwrap();
    assert!(history.is_empty());
    actor.permissions = vec![codes::ACADEMIC_RESULT_READ_SCHOOL.into()];
    assert!(matches!(
        preview_aggregate(&pool, &actor, student, &query).await,
        Err(AppError::Forbidden(_))
    ));
    let update =
        sqlx::query("UPDATE academic_aggregate_policy_versions SET name='Changed' WHERE id=$1")
            .bind(policy.id)
            .execute(&pool)
            .await;
    assert!(
        update.is_err(),
        "approved policy contents must be immutable"
    );
}

#[tokio::test]
async fn aggregate_readers_share_one_snapshot_across_a_committed_correction() {
    let (pool, actor, ctx, group) = fixture("aggregate_shared_snapshot").await;
    let (student, subject): (Uuid, Uuid) = sqlx::query_as(
        "SELECT m.student_academic_year_id,d.subject_id FROM learning_group_students m JOIN learning_groups g ON g.id=m.learning_group_id JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1 AND m.membership_status='active' ORDER BY m.student_academic_year_id LIMIT 1"
    ).bind(group).fetch_one(&pool).await.unwrap();
    prepare_course_subject(&pool, &actor, &ctx, subject).await;
    let affairs = academic_affairs_actor(&actor);
    lock_course_subject(&pool, &affairs, &ctx, subject)
        .await
        .unwrap();
    let query = TermResultPreviewQuery {
        academic_year_id: ctx.academic_year_id,
        academic_term_id: ctx.academic_term_id,
        passing_grade: "1".into(),
    };
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    let before = preview_student_term_in_transaction(&mut tx, student, &query)
        .await
        .unwrap();
    let result = before
        .courses
        .iter()
        .find(|row| row.subject_id == subject)
        .unwrap();
    correct_result(
        &pool,
        &affairs,
        &ctx,
        ResultCorrectionInput::Course {
            course_result_id: result.result_id.unwrap(),
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let retained = preview_student_term_in_transaction(&mut tx, student, &query)
        .await
        .unwrap();
    assert_eq!(retained.totals.provisional_gpa.as_deref(), Some("0.00"));
    assert_eq!(retained.source_checksum, before.source_checksum);
    let evaluations = learner::summarize_student_term_in_transaction(
        &mut tx,
        &EvaluationContext {
            academic_year_id: ctx.academic_year_id,
            academic_term_id: ctx.academic_term_id,
        },
        student,
    )
    .await
    .unwrap();
    assert_eq!(evaluations.domains.len(), 2);
    assert!(evaluations.domains.iter().all(|domain| !domain.complete));
    tx.commit().await.unwrap();
    let current = preview_student_term(&pool, &affairs, student, &query)
        .await
        .unwrap();
    assert_eq!(current.totals.provisional_gpa.as_deref(), Some("4.00"));
    assert_ne!(current.source_checksum, before.source_checksum);
}
