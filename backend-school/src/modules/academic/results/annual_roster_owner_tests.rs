use crate::modules::academic::{
    cutover_test_support::apply_migrations_through, results::services_tests::fixture,
};
use school_academic_results::services::*;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use uuid::Uuid;

#[tokio::test]
async fn annual_roster_retains_missing_results_and_requires_both_school_domains() {
    let (pool, actor, context, _) = fixture("annual_roster").await;
    apply_migrations_through(&pool, 69).await.unwrap();
    let office = ActorContext {
        user_id: actor.user_id,
        permissions: vec![
            codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
        ],
    };
    let rows = list_annual_students(&pool, &office, context.academic_year_id)
        .await
        .unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| !row.student_name.is_empty()
        && !row.grade_level_name.is_empty()
        && !row.study_program_name.is_empty()
        && row.closure.revision_id.is_none()
        && !row.closure.is_current));
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
            list_annual_students(&pool, &denied, context.academic_year_id).await,
            Err(AppError::Forbidden(_))
        ));
    }
    assert!(list_annual_students(&pool, &office, Uuid::new_v4())
        .await
        .is_err());
}
