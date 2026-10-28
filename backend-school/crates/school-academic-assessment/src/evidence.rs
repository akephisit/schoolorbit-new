//! Assessment-owned established facts. This leaf never calls the Results port.
use crate::ports::CURRENT_OBLIGATION_GROUP_SCAN_LIMIT;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use std::collections::HashSet;
use uuid::Uuid;

pub async fn groups_with_established_history_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    year: Uuid,
    groups: &[Uuid],
) -> Result<HashSet<Uuid>, AppError> {
    let groups = groups
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if groups.len() > CURRENT_OBLIGATION_GROUP_SCAN_LIMIT {
        return Err(AppError::ValidationError(
            "Assessment history group scan exceeds its hard limit".into(),
        ));
    }
    if groups.is_empty() {
        return Ok(HashSet::new());
    }
    let valid: i64 = DbSession::new(&mut *connection).scalar(
        "academic_assessment.evidence.groups_with_established_history_on_connection",
        "SELECT count(*) FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.academic_term_id=g.academic_term_id AND o.academic_year_id=g.academic_year_id WHERE g.id=ANY($1) AND g.academic_term_id=$2 AND g.academic_year_id=$3",
    ).bind(&groups).bind(term).bind(year).fetch_one().await?;
    if valid != groups.len() as i64 {
        return Err(AppError::Conflict(
            "Assessment history groups do not match the selected academic context".into(),
        ));
    }
    let rows: Vec<Uuid> = DbSession::new(connection).scalar(
        "academic_assessment.evidence.groups_with_established_history_on_connection",
        r#"SELECT g.id FROM learning_groups g WHERE g.id=ANY($1) AND g.academic_term_id=$2 AND g.academic_year_id=$3 AND (
            EXISTS(SELECT 1 FROM learning_group_score_items f WHERE f.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM learning_group_student_scores f WHERE f.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM learning_group_phase_confirmations f WHERE f.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM learning_group_student_evaluations f WHERE f.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM learning_group_evaluation_confirmations f WHERE f.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM subject_term_student_evaluations f WHERE f.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM course_offering_details d JOIN subject_term_evaluation_locks f
                ON f.subject_id=d.subject_id AND f.academic_term_id=$2 AND f.academic_year_id=$3
                WHERE d.learning_offering_id=g.learning_offering_id)
        ) ORDER BY g.id"#,
    ).bind(&groups).bind(term).bind(year).fetch_all().await?;
    // Subject/domain locks protect their complete context, including zero-result
    // locks and historical snapshots whose JSON shape cannot prove absence.
    Ok(rows.into_iter().collect())
}
