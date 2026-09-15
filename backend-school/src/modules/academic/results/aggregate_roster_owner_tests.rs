use crate::modules::academic::{
    cutover_test_support::apply_migrations_through, results::services_tests::fixture,
};
use school_academic_results::{models::*, services::*};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use uuid::Uuid;

#[tokio::test]
async fn aggregate_roster_keeps_missing_students_and_requires_both_domains() {
    let (pool, actor, context, _) = fixture("aggregate_roster").await;
    apply_migrations_through(&pool, 68).await.unwrap();
    let office = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
        ],
    };
    let rows = list_aggregate_students(&pool, &office, &context)
        .await
        .unwrap();
    assert!(
        !rows.is_empty(),
        "students without locked aggregate revisions must remain visible"
    );
    assert!(rows.iter().all(|row| !row.student_name.is_empty()
        && !row.grade_level_name.is_empty()
        && !row.study_program_name.is_empty()
        && row.closure.revision_id.is_none()
        && !row.closure.is_current));
    let mut tx = pool.begin().await.unwrap();
    let expected = term_closure_coverage(&mut tx, &context).await.unwrap();
    let mut ids: Vec<_> = rows
        .iter()
        .map(|row| row.student_academic_year_id)
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        expected
            .students
            .iter()
            .map(|row| row.student_academic_year_id)
            .collect::<Vec<_>>()
    );
    assert!(rows
        .iter()
        .all(|row| row.student_academic_year_id == row.closure.student_academic_year_id));
    for permissions in [
        vec![codes::ACADEMIC_RESULT_READ_SCHOOL.into()],
        vec![
            codes::ACADEMIC_RESULT_READ_ASSIGNED.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED.into(),
        ],
    ] {
        let denied = ActorContext {
            user_id: actor.user_id,
            permissions,
        };
        assert!(matches!(
            list_aggregate_students(&pool, &denied, &context).await,
            Err(AppError::Forbidden(_))
        ));
    }
    let foreign = ResultContext {
        academic_year_id: Uuid::new_v4(),
        academic_term_id: context.academic_term_id,
    };
    assert!(list_aggregate_students(&pool, &office, &foreign)
        .await
        .is_err());
}
