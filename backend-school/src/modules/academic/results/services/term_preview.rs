use super::{
    activity_aggregation::aggregate_activity_outcomes, activity_preview::load_activity_inputs_batch,
};
use super::{aggregate_course_credits, decimal, decimal_wire, hash, validate_context};
use crate::{
    error::AppError, middleware::permission::ActorContext, modules::academic::results::models::*,
    policies::academic_result_access_policy,
};
use bigdecimal::BigDecimal;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::BTreeMap;
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

#[derive(sqlx::FromRow)]
struct StudentCourseRow {
    student_academic_year_id: Uuid,
    #[sqlx(flatten)]
    course: CourseRow,
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
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let preview = preview_student_term_in_transaction(&mut tx, student_year_id, query).await?;
    tx.commit().await?;
    Ok(preview)
}

/// Internal provider: the caller owns authorization and transaction isolation.
pub(crate) async fn preview_student_term_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    student_year_id: Uuid,
    query: &TermResultPreviewQuery,
) -> Result<TermResultPreview, AppError> {
    preview_student_terms_in_transaction(tx, &[student_year_id], query)
        .await?
        .remove(&student_year_id)
        .ok_or_else(|| {
            AppError::InternalServerError("Term batch omitted a validated student".into())
        })
}

/// Set-based provider; the caller owns authorization and snapshot isolation.
pub(crate) async fn preview_student_terms_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    students: &[Uuid],
    query: &TermResultPreviewQuery,
) -> Result<BTreeMap<Uuid, TermResultPreview>, AppError> {
    aggregate_course_credits(&[], &query.passing_grade)?;
    let passing_grade = decimal_wire(&decimal(&query.passing_grade)?);
    let context = ResultContext {
        academic_year_id: query.academic_year_id,
        academic_term_id: query.academic_term_id,
    };
    validate_context(tx, &context).await?;
    crate::modules::academic::core::services::student_year_batch::validate_student_year_batch(
        tx,
        context.academic_year_id,
        students,
    )
    .await?;
    if students.is_empty() {
        return Ok(BTreeMap::new());
    }
    // Retained locked results remain expected even after roster removal or group closure.
    // UNION deduplicates multiple groups of the same offering; conflicting offerings for
    // the same subject are rejected by aggregation rather than arbitrarily selected.
    let rows: Vec<StudentCourseRow> = sqlx::query_as(r#"
        WITH expected AS (
            SELECT membership.student_academic_year_id,detail.subject_id,detail.learning_offering_id
            FROM learning_group_students membership
            JOIN learning_groups learning_group ON learning_group.id=membership.learning_group_id
            JOIN course_offering_details detail ON detail.learning_offering_id=learning_group.learning_offering_id
            WHERE membership.student_academic_year_id=ANY($1) AND membership.academic_term_id=$2
              AND membership.academic_year_id=$3 AND membership.membership_status='active'
            UNION
            SELECT student_academic_year_id,subject_id,learning_offering_id FROM academic_course_results
            WHERE student_academic_year_id=ANY($1) AND academic_term_id=$2 AND academic_year_id=$3
        )
        SELECT expected.student_academic_year_id,expected.subject_id,expected.learning_offering_id,detail.credit AS credits,
               result.id AS result_id,
               COALESCE(correction.expected_effective_version+1,result.row_version) AS effective_version,
               COALESCE(correction.new_course_outcome,result.outcome) AS outcome,
               CASE WHEN correction.expected_effective_version IS NULL THEN result.numeric_grade
                    ELSE correction.new_numeric_grade END AS numeric_grade
        FROM expected
        JOIN course_offering_details detail ON detail.learning_offering_id=expected.learning_offering_id
        LEFT JOIN academic_course_results result ON result.subject_id=expected.subject_id
          AND result.learning_offering_id=expected.learning_offering_id
          AND result.student_academic_year_id=expected.student_academic_year_id AND result.academic_term_id=$2 AND result.academic_year_id=$3
        LEFT JOIN LATERAL (
            SELECT item.new_course_outcome,item.new_numeric_grade,item.expected_effective_version
            FROM academic_result_corrections item WHERE item.course_result_id=result.id
            ORDER BY item.expected_effective_version DESC,item.id DESC LIMIT 1
        ) correction ON true
        ORDER BY expected.student_academic_year_id,expected.subject_id,expected.learning_offering_id
        LIMIT 100001
    "#).bind(students).bind(context.academic_term_id).bind(context.academic_year_id)
        .fetch_all(&mut **tx).await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError(
            "Course result batch exceeds 100000 source rows; select fewer students".into(),
        ));
    }
    let mut by_student: BTreeMap<Uuid, Vec<CourseAggregateInput>> = BTreeMap::new();
    for row in rows {
        let courses = by_student.entry(row.student_academic_year_id).or_default();
        if courses.len() >= 2000 {
            return Err(AppError::ValidationError(
                "Term result preview exceeds 2000 course offerings".into(),
            ));
        }
        courses.push(course_input(row.course)?);
    }
    let mut activity_inputs = load_activity_inputs_batch(tx, &context, students).await?;
    let mut previews = BTreeMap::new();
    for student in students {
        let courses = by_student.remove(student).unwrap_or_default();
        let activities = activity_inputs.remove(student).unwrap_or_default();
        previews.insert(
            *student,
            assemble_preview(&context, *student, &passing_grade, courses, activities)?,
        );
    }
    Ok(previews)
}

fn assemble_preview(
    context: &ResultContext,
    student_year_id: Uuid,
    passing_grade: &str,
    courses: Vec<CourseAggregateInput>,
    activities: Vec<ActivityAggregateInput>,
) -> Result<TermResultPreview, AppError> {
    let totals = aggregate_course_credits(&courses, passing_grade)?;
    let activity_totals = aggregate_activity_outcomes(&activities)?;
    let source_checksum = hash(&(
        context.academic_year_id,
        context.academic_term_id,
        student_year_id,
        &passing_grade,
        &courses,
        &activities,
    ))?;
    Ok(TermResultPreview {
        academic_year_id: context.academic_year_id,
        academic_term_id: context.academic_term_id,
        student_academic_year_id: student_year_id,
        passing_grade: passing_grade.into(),
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
    fn term_preview_preserves_the_pre_batch_source_checksum_contract() {
        let context = ResultContext {
            academic_year_id: Uuid::from_u128(1),
            academic_term_id: Uuid::from_u128(2),
        };
        let preview = assemble_preview(
            &context,
            Uuid::from_u128(3),
            "1.00",
            vec![CourseAggregateInput {
                subject_id: Uuid::from_u128(4),
                learning_offering_id: Uuid::from_u128(5),
                result_id: Some(Uuid::from_u128(6)),
                effective_version: Some(2),
                credits: "1.50".into(),
                outcome: Some(CourseOfficialOutcome::Numeric),
                numeric_grade: Some("3.00".into()),
            }],
            vec![],
        )
        .unwrap();
        assert_eq!(
            preview.source_checksum,
            "dc1aa6760eb2f054d74134ae22b1d74ad7e89b8d4b006a55f555f1e77822393f"
        );
    }

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
