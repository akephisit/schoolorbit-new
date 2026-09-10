use crate::{error::AppError, modules::academic::results::models::*};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ActivityRow {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    result_id: Option<Uuid>,
    effective_version: Option<i64>,
    outcome: Option<String>,
}

#[derive(sqlx::FromRow)]
struct StudentActivityRow {
    student_academic_year_id: Uuid,
    #[sqlx(flatten)]
    activity: ActivityRow,
}

fn activity_input(row: ActivityRow) -> Result<ActivityAggregateInput, AppError> {
    Ok(ActivityAggregateInput {
        learning_group_id: row.learning_group_id,
        learning_offering_id: row.learning_offering_id,
        result_id: row.result_id,
        effective_version: row.effective_version,
        outcome: row
            .outcome
            .as_deref()
            .map(ActivityOutcome::try_from)
            .transpose()
            .map_err(|message| AppError::InternalServerError(message.into()))?,
    })
}

pub(super) async fn load_activity_inputs_batch(
    tx: &mut Transaction<'_, Postgres>,
    context: &ResultContext,
    students: &[Uuid],
) -> Result<BTreeMap<Uuid, Vec<ActivityAggregateInput>>, AppError> {
    let rows: Vec<StudentActivityRow> = sqlx::query_as(r#"
        WITH expected AS (
            SELECT m.student_academic_year_id,g.id AS learning_group_id,g.learning_offering_id
            FROM learning_group_students m
            JOIN learning_groups g ON g.id=m.learning_group_id
            JOIN activity_offering_details d ON d.learning_offering_id=g.learning_offering_id
            WHERE m.student_academic_year_id=ANY($1) AND m.academic_term_id=$2
              AND m.academic_year_id=$3 AND m.membership_status='active'
            UNION
            SELECT student_academic_year_id,learning_group_id,learning_offering_id FROM academic_activity_results
            WHERE student_academic_year_id=ANY($1) AND academic_term_id=$2 AND academic_year_id=$3
        )
        SELECT expected.student_academic_year_id,expected.learning_group_id,expected.learning_offering_id,result.id AS result_id,
               COALESCE(correction.expected_effective_version+1,result.row_version) AS effective_version,
               COALESCE(correction.new_activity_outcome,result.outcome) AS outcome
        FROM expected
        LEFT JOIN academic_activity_results result ON result.learning_group_id=expected.learning_group_id
          AND result.learning_offering_id=expected.learning_offering_id
          AND result.student_academic_year_id=expected.student_academic_year_id AND result.academic_term_id=$2 AND result.academic_year_id=$3
        LEFT JOIN LATERAL (
            SELECT item.new_activity_outcome,item.expected_effective_version
            FROM academic_result_corrections item WHERE item.activity_result_id=result.id
            ORDER BY item.expected_effective_version DESC,item.id DESC LIMIT 1
        ) correction ON true
        ORDER BY expected.student_academic_year_id,expected.learning_group_id
        LIMIT 100001
    "#).bind(students).bind(context.academic_term_id).bind(context.academic_year_id)
        .fetch_all(&mut **tx).await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError(
            "Activity result batch exceeds 100000 source rows; select fewer students".into(),
        ));
    }
    let mut inputs: BTreeMap<Uuid, Vec<ActivityAggregateInput>> = BTreeMap::new();
    for row in rows {
        let student = inputs.entry(row.student_academic_year_id).or_default();
        if student.len() >= 2000 {
            return Err(AppError::ValidationError(
                "Term result preview exceeds 2000 activity groups".into(),
            ));
        }
        student.push(activity_input(row.activity)?);
    }
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_activity_outcome_is_not_coerced_to_missing() {
        let row = ActivityRow {
            learning_group_id: Uuid::new_v4(),
            learning_offering_id: Uuid::new_v4(),
            result_id: Some(Uuid::new_v4()),
            effective_version: Some(1),
            outcome: Some("invalid".into()),
        };
        assert!(matches!(
            activity_input(row),
            Err(AppError::InternalServerError(_))
        ));
    }
}
