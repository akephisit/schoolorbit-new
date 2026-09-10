use super::{
    activity_aggregation::aggregate_activity_outcomes, activity_preview::load_activity_inputs,
};
use super::{aggregate_course_credits, decimal, decimal_wire, hash, validate_context};
use crate::{
    error::AppError, middleware::permission::ActorContext, modules::academic::results::models::*,
    policies::academic_result_access_policy,
};
use bigdecimal::BigDecimal;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct CourseRow {
    subject_id: Uuid,
    learning_offering_id: Uuid,
    credits: BigDecimal,
    result_id: Option<Uuid>,
    effective_version: Option<i64>,
    outcome: Option<String>,
    numeric_grade: Option<BigDecimal>,
}

fn course_input(row: CourseRow) -> Result<CourseAggregateInput, AppError> {
    Ok(CourseAggregateInput {
        subject_id: row.subject_id,
        learning_offering_id: row.learning_offering_id,
        result_id: row.result_id,
        effective_version: row.effective_version,
        credits: decimal_wire(&row.credits),
        outcome: row
            .outcome
            .as_deref()
            .map(CourseOfficialOutcome::try_from)
            .transpose()
            .map_err(|message| AppError::InternalServerError(message.into()))?,
        numeric_grade: row.numeric_grade.as_ref().map(decimal_wire),
    })
}

pub async fn preview_student_term(
    pool: &PgPool,
    actor: &ActorContext,
    student_year_id: Uuid,
    query: &TermResultPreviewQuery,
) -> Result<TermResultPreview, AppError> {
    if !academic_result_access_policy::can_read_student_aggregate(actor) {
        return Err(AppError::Forbidden(
            "School-level result permission is required for whole-student totals".into(),
        ));
    }
    aggregate_course_credits(&[], &query.passing_grade)?;
    let passing_grade = decimal_wire(&decimal(&query.passing_grade)?);
    let context = ResultContext {
        academic_year_id: query.academic_year_id,
        academic_term_id: query.academic_term_id,
    };
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_context(&mut tx, &context).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM student_academic_years WHERE id=$1 AND academic_year_id=$2)",
    )
    .bind(student_year_id)
    .bind(context.academic_year_id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound(
            "Student academic year not found in selected context".into(),
        ));
    }
    // Retained locked results remain expected even after roster removal or group closure.
    // UNION deduplicates multiple groups of the same offering; conflicting offerings for
    // the same subject are rejected by aggregation rather than arbitrarily selected.
    let rows: Vec<CourseRow> = sqlx::query_as(r#"
        WITH expected AS (
            SELECT detail.subject_id,detail.learning_offering_id
            FROM learning_group_students membership
            JOIN learning_groups learning_group ON learning_group.id=membership.learning_group_id
            JOIN course_offering_details detail ON detail.learning_offering_id=learning_group.learning_offering_id
            WHERE membership.student_academic_year_id=$1 AND membership.academic_term_id=$2
              AND membership.academic_year_id=$3 AND membership.membership_status='active'
            UNION
            SELECT subject_id,learning_offering_id FROM academic_course_results
            WHERE student_academic_year_id=$1 AND academic_term_id=$2 AND academic_year_id=$3
        )
        SELECT expected.subject_id,expected.learning_offering_id,detail.credit AS credits,
               result.id AS result_id,
               COALESCE(correction.expected_effective_version+1,result.row_version) AS effective_version,
               COALESCE(correction.new_course_outcome,result.outcome) AS outcome,
               CASE WHEN correction.expected_effective_version IS NULL THEN result.numeric_grade
                    ELSE correction.new_numeric_grade END AS numeric_grade
        FROM expected
        JOIN course_offering_details detail ON detail.learning_offering_id=expected.learning_offering_id
        LEFT JOIN academic_course_results result ON result.subject_id=expected.subject_id
          AND result.learning_offering_id=expected.learning_offering_id
          AND result.student_academic_year_id=$1 AND result.academic_term_id=$2 AND result.academic_year_id=$3
        LEFT JOIN LATERAL (
            SELECT item.new_course_outcome,item.new_numeric_grade,item.expected_effective_version
            FROM academic_result_corrections item WHERE item.course_result_id=result.id
            ORDER BY item.expected_effective_version DESC,item.id DESC LIMIT 1
        ) correction ON true
        ORDER BY expected.subject_id,expected.learning_offering_id
        LIMIT 2001
    "#).bind(student_year_id).bind(context.academic_term_id).bind(context.academic_year_id)
        .fetch_all(&mut *tx).await?;
    if rows.len() > 2000 {
        return Err(AppError::ValidationError(
            "Term result preview exceeds 2000 course offerings".into(),
        ));
    }
    let courses = rows
        .into_iter()
        .map(course_input)
        .collect::<Result<Vec<_>, _>>()?;
    let totals = aggregate_course_credits(&courses, &passing_grade)?;
    let activities = load_activity_inputs(&mut tx, &context, student_year_id).await?;
    let activity_totals = aggregate_activity_outcomes(&activities)?;
    let source_checksum = hash(&(
        context.academic_year_id,
        context.academic_term_id,
        student_year_id,
        &passing_grade,
        &courses,
        &activities,
    ))?;
    tx.commit().await?;
    Ok(TermResultPreview {
        academic_year_id: context.academic_year_id,
        academic_term_id: context.academic_term_id,
        student_academic_year_id: student_year_id,
        passing_grade,
        courses,
        totals,
        activities,
        activity_totals,
        source_checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_stored_outcome_fails_instead_of_becoming_missing() {
        let row = CourseRow {
            subject_id: Uuid::new_v4(),
            learning_offering_id: Uuid::new_v4(),
            credits: BigDecimal::from(1),
            result_id: Some(Uuid::new_v4()),
            effective_version: Some(1),
            outcome: Some("invalid".into()),
            numeric_grade: None,
        };
        assert!(matches!(
            course_input(row),
            Err(AppError::InternalServerError(_))
        ));
    }
}
