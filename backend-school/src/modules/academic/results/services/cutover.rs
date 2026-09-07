use crate::error::AppError;
use crate::modules::academic::reconciliation::ReconciliationCheck;
use sqlx::PgPool;

pub const GRADEBOOK_RESULTS_MIGRATION_VERSION: i64 = 60;

const REQUIRED_TABLES: [&str; 23] = [
    "academic_gradebook_phase_controls",
    "learning_group_student_scores",
    "learning_group_phase_confirmations",
    "academic_grading_policy_versions",
    "academic_grading_policy_bands",
    "learning_group_result_overrides",
    "learning_group_result_confirmations",
    "academic_learner_evaluation_criteria",
    "subject_term_evaluation_criteria",
    "academic_learner_evaluation_controls",
    "learning_group_student_evaluations",
    "learning_group_evaluation_confirmations",
    "subject_term_evaluation_locks",
    "subject_term_student_evaluations",
    "academic_learner_evaluation_policy_versions",
    "academic_learner_evaluation_policy_bands",
    "academic_course_result_locks",
    "academic_course_results",
    "academic_activity_evaluations",
    "academic_activity_result_confirmations",
    "academic_activity_result_locks",
    "academic_activity_results",
    "academic_result_corrections",
];

const REQUIRED_PERMISSION_CODES: [&str; 19] = [
    "academic_gradebook.read.assigned",
    "academic_gradebook.read.organization_unit",
    "academic_gradebook.read.school",
    "academic_gradebook.manage.assigned",
    "academic_gradebook.manage.school",
    "academic_result.read.assigned",
    "academic_result.read.organization_unit",
    "academic_result.read.school",
    "academic_result.manage.assigned",
    "academic_result.manage.school",
    "academic_result.lock.school",
    "academic_result.correct.school",
    "academic_learner_evaluation.read.assigned",
    "academic_learner_evaluation.read.organization_unit",
    "academic_learner_evaluation.read.school",
    "academic_learner_evaluation.manage.assigned",
    "academic_learner_evaluation.manage.school",
    "academic_learner_evaluation.lock.school",
    "academic_learner_evaluation.correct.school",
];

#[derive(Debug)]
pub struct GradebookResultsCutoverAudit {
    pub completed: bool,
    pub checks: Vec<ReconciliationCheck>,
}

fn count_check(code: &str, expected: i64, actual: i64) -> ReconciliationCheck {
    ReconciliationCheck {
        code: code.to_string(),
        passed: expected == actual,
        source_count: expected,
        target_count: actual,
    }
}

async fn existing_relation_count(pool: &PgPool, relation_names: &[&str]) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "SELECT count(*)
         FROM unnest($1::text[]) AS relation(name)
         WHERE to_regclass(format('%I.%I', current_schema(), relation.name)) IS NOT NULL",
    )
    .bind(relation_names)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn read_gradebook_results_cutover_audit(
    pool: &PgPool,
) -> Result<GradebookResultsCutoverAudit, AppError> {
    let required_table_count = existing_relation_count(pool, &REQUIRED_TABLES).await?;
    let mut checks = vec![count_check(
        "GRADEBOOK_RESULTS_REQUIRED_TABLES_PRESENT",
        REQUIRED_TABLES.len() as i64,
        required_table_count,
    )];

    if required_table_count == REQUIRED_TABLES.len() as i64 {
        let (
            expected_gradebook_controls,
            gradebook_controls,
            expected_evaluation_controls,
            evaluation_controls,
            seed_policy_versions,
            seed_policy_bands,
            permission_definitions,
        ): (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
                (SELECT count(*) * 4 FROM academic_terms),
                (SELECT count(*) FROM academic_gradebook_phase_controls),
                (SELECT count(*) * 2 FROM academic_terms),
                (SELECT count(*) FROM academic_learner_evaluation_controls),
                (SELECT count(*) FROM academic_grading_policy_versions
                 WHERE id = '06000000-0000-0000-0000-000000000001' AND version_no = 1)
                  +
                (SELECT count(*) FROM academic_learner_evaluation_policy_versions
                 WHERE id = '06000000-0000-0000-0000-000000000002' AND version_no = 1),
                (SELECT count(*) FROM academic_grading_policy_bands
                 WHERE policy_version_id = '06000000-0000-0000-0000-000000000001')
                  +
                (SELECT count(*) FROM academic_learner_evaluation_policy_bands
                 WHERE policy_version_id = '06000000-0000-0000-0000-000000000002'),
                (SELECT count(*) FROM permissions
                 WHERE code = ANY($1::text[]) AND is_active)",
        )
        .bind(&REQUIRED_PERMISSION_CODES)
        .fetch_one(pool)
        .await?;

        checks.extend([
            count_check(
                "GRADEBOOK_RESULTS_PHASE_CONTROLS_COMPLETE",
                expected_gradebook_controls,
                gradebook_controls,
            ),
            count_check(
                "GRADEBOOK_RESULTS_EVALUATION_CONTROLS_COMPLETE",
                expected_evaluation_controls,
                evaluation_controls,
            ),
            count_check(
                "GRADEBOOK_RESULTS_SEED_POLICY_VERSIONS_PRESENT",
                2,
                seed_policy_versions,
            ),
            count_check(
                "GRADEBOOK_RESULTS_SEED_POLICY_BANDS_PRESENT",
                12,
                seed_policy_bands,
            ),
            count_check(
                "GRADEBOOK_RESULTS_PERMISSION_DEFINITIONS_PRESENT",
                19,
                permission_definitions,
            ),
        ]);
    }

    Ok(GradebookResultsCutoverAudit {
        completed: checks.iter().all(|check| check.passed),
        checks,
    })
}
