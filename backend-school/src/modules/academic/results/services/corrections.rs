use super::{conflict, decimal, decimal_wire, validate_context};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::results::models::*,
    policies::{academic_result_access_policy, learner_evaluation_access_policy},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct CourseResultRow {
    id: Uuid,
    student_academic_year_id: Uuid,
    outcome: String,
    numeric_grade: Option<BigDecimal>,
    row_version: i64,
}

#[derive(sqlx::FromRow)]
struct CourseCorrectionRow {
    id: Uuid,
    old_course_outcome: String,
    new_course_outcome: String,
    old_numeric_grade: Option<BigDecimal>,
    new_numeric_grade: Option<BigDecimal>,
    expected_effective_version: i64,
    corrected_by: Uuid,
    corrected_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ActivityResultRow {
    id: Uuid,
    student_academic_year_id: Uuid,
    outcome: String,
    row_version: i64,
}

#[derive(sqlx::FromRow)]
struct ActivityCorrectionRow {
    id: Uuid,
    old_activity_outcome: String,
    new_activity_outcome: String,
    expected_effective_version: i64,
    corrected_by: Uuid,
    corrected_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct LearnerEvaluationResultRow {
    id: Uuid,
    student_academic_year_id: Uuid,
    quality_level: i16,
    row_version: i64,
}

#[derive(sqlx::FromRow)]
struct LearnerEvaluationCorrectionRow {
    id: Uuid,
    old_quality_level: i16,
    new_quality_level: i16,
    expected_effective_version: i64,
    corrected_by: Uuid,
    corrected_at: DateTime<Utc>,
}

fn course_value(
    outcome: &str,
    numeric_grade: Option<&BigDecimal>,
) -> Result<EffectiveResultValue, AppError> {
    let outcome = CourseOfficialOutcome::try_from(outcome)
        .map_err(|message| AppError::InternalServerError(message.into()))?;
    Ok(EffectiveResultValue::Course {
        outcome,
        numeric_grade: numeric_grade.map(decimal_wire),
    })
}

fn validate_course_value(
    outcome: CourseOfficialOutcome,
    numeric_grade: Option<String>,
) -> Result<(String, Option<BigDecimal>), AppError> {
    match (outcome, numeric_grade) {
        (CourseOfficialOutcome::Numeric, Some(value)) => {
            let grade = decimal(&value)?;
            let allowed = ["0", "0.5", "1", "1.5", "2", "2.5", "3", "3.5", "4"]
                .into_iter()
                .map(decimal)
                .collect::<Result<Vec<_>, _>>()?;
            if !allowed.contains(&grade) {
                return Err(AppError::ValidationError(
                    "Official numeric grade must be 0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, or 4".into(),
                ));
            }
            Ok((outcome.as_str().into(), Some(grade)))
        }
        (CourseOfficialOutcome::Numeric, None) => Err(AppError::ValidationError(
            "Official numeric outcome requires a numeric grade".into(),
        )),
        (
            CourseOfficialOutcome::Incomplete | CourseOfficialOutcome::InsufficientAttendance,
            None,
        ) => Ok((outcome.as_str().into(), None)),
        (_, Some(_)) => Err(AppError::ValidationError(
            "Non-numeric official outcome cannot include a numeric grade".into(),
        )),
    }
}

fn course_correction_record(row: CourseCorrectionRow) -> Result<ResultCorrectionRecord, AppError> {
    Ok(ResultCorrectionRecord {
        id: row.id,
        expected_effective_version: row.expected_effective_version,
        previous: course_value(&row.old_course_outcome, row.old_numeric_grade.as_ref())?,
        corrected: course_value(&row.new_course_outcome, row.new_numeric_grade.as_ref())?,
        corrected_by: row.corrected_by,
        corrected_at: row.corrected_at,
    })
}

fn activity_value(outcome: &str) -> Result<EffectiveResultValue, AppError> {
    let outcome = ActivityOutcome::try_from(outcome)
        .map_err(|message| AppError::InternalServerError(message.into()))?;
    Ok(EffectiveResultValue::Activity { outcome })
}

fn activity_correction_record(
    row: ActivityCorrectionRow,
) -> Result<ResultCorrectionRecord, AppError> {
    Ok(ResultCorrectionRecord {
        id: row.id,
        expected_effective_version: row.expected_effective_version,
        previous: activity_value(&row.old_activity_outcome)?,
        corrected: activity_value(&row.new_activity_outcome)?,
        corrected_by: row.corrected_by,
        corrected_at: row.corrected_at,
    })
}

fn learner_evaluation_value(quality_level: i16) -> Result<EffectiveResultValue, AppError> {
    if !(0..=3).contains(&quality_level) {
        return Err(AppError::InternalServerError(
            "Stored learner evaluation level is invalid".into(),
        ));
    }
    Ok(EffectiveResultValue::LearnerEvaluation { quality_level })
}

fn learner_evaluation_correction_record(
    row: LearnerEvaluationCorrectionRow,
) -> Result<ResultCorrectionRecord, AppError> {
    Ok(ResultCorrectionRecord {
        id: row.id,
        expected_effective_version: row.expected_effective_version,
        previous: learner_evaluation_value(row.old_quality_level)?,
        corrected: learner_evaluation_value(row.new_quality_level)?,
        corrected_by: row.corrected_by,
        corrected_at: row.corrected_at,
    })
}

async fn correct_course(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    context: &ResultContext,
    course_result_id: Uuid,
    outcome: CourseOfficialOutcome,
    numeric_grade: Option<String>,
    expected_effective_version: i64,
) -> Result<EffectiveResult, AppError> {
    if expected_effective_version <= 0 {
        return Err(AppError::ValidationError(
            "expectedEffectiveVersion must be greater than zero".into(),
        ));
    }
    let initial: CourseResultRow = sqlx::query_as(
        r#"SELECT id,student_academic_year_id,outcome,numeric_grade,row_version
           FROM academic_course_results
           WHERE id=$1 AND academic_term_id=$2 AND academic_year_id=$3
           FOR UPDATE"#,
    )
    .bind(course_result_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Official course result not found".into()))?;
    let mut history: Vec<CourseCorrectionRow> = sqlx::query_as(
        r#"SELECT id,old_course_outcome,new_course_outcome,old_numeric_grade,new_numeric_grade,
                  expected_effective_version,corrected_by,corrected_at
           FROM academic_result_corrections
           WHERE course_result_id=$1
           ORDER BY expected_effective_version,id"#,
    )
    .bind(initial.id)
    .fetch_all(&mut **tx)
    .await?;
    let current_version = history.last().map_or(initial.row_version, |row| {
        row.expected_effective_version + 1
    });
    if expected_effective_version != current_version {
        return Err(conflict());
    }
    let (old_outcome, old_grade) = history.last().map_or_else(
        || (initial.outcome.clone(), initial.numeric_grade.clone()),
        |row| {
            (
                row.new_course_outcome.clone(),
                row.new_numeric_grade.clone(),
            )
        },
    );
    let (new_outcome, new_grade) = validate_course_value(outcome, numeric_grade)?;
    if old_outcome == new_outcome && old_grade == new_grade {
        return Err(AppError::ValidationError(
            "Corrected result must differ from the effective result".into(),
        ));
    }
    let inserted: CourseCorrectionRow = sqlx::query_as(
        r#"INSERT INTO academic_result_corrections (
               course_result_id,old_course_outcome,new_course_outcome,
               old_numeric_grade,new_numeric_grade,expected_effective_version,corrected_by
           ) VALUES ($1,$2,$3,$4,$5,$6,$7)
           RETURNING id,old_course_outcome,new_course_outcome,old_numeric_grade,new_numeric_grade,
                     expected_effective_version,corrected_by,corrected_at"#,
    )
    .bind(initial.id)
    .bind(&old_outcome)
    .bind(&new_outcome)
    .bind(&old_grade)
    .bind(&new_grade)
    .bind(current_version)
    .bind(actor.user_id)
    .fetch_one(&mut **tx)
    .await?;
    history.push(inserted);

    let initial_value = course_value(&initial.outcome, initial.numeric_grade.as_ref())?;
    let effective = course_value(&new_outcome, new_grade.as_ref())?;
    Ok(EffectiveResult {
        result_id: initial.id,
        student_academic_year_id: initial.student_academic_year_id,
        initial: initial_value,
        effective,
        effective_version: current_version + 1,
        corrections: history
            .into_iter()
            .map(course_correction_record)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

async fn correct_activity(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    context: &ResultContext,
    activity_result_id: Uuid,
    outcome: ActivityOutcome,
    expected_effective_version: i64,
) -> Result<EffectiveResult, AppError> {
    if expected_effective_version <= 0 {
        return Err(AppError::ValidationError(
            "expectedEffectiveVersion must be greater than zero".into(),
        ));
    }
    let initial: ActivityResultRow = sqlx::query_as(
        r#"SELECT id,student_academic_year_id,outcome,row_version
           FROM academic_activity_results
           WHERE id=$1 AND academic_term_id=$2 AND academic_year_id=$3
           FOR UPDATE"#,
    )
    .bind(activity_result_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Official activity result not found".into()))?;
    let mut history: Vec<ActivityCorrectionRow> = sqlx::query_as(
        r#"SELECT id,old_activity_outcome,new_activity_outcome,
                  expected_effective_version,corrected_by,corrected_at
           FROM academic_result_corrections
           WHERE activity_result_id=$1
           ORDER BY expected_effective_version,id"#,
    )
    .bind(initial.id)
    .fetch_all(&mut **tx)
    .await?;
    let current_version = history.last().map_or(initial.row_version, |row| {
        row.expected_effective_version + 1
    });
    if expected_effective_version != current_version {
        return Err(conflict());
    }
    let old_outcome = history.last().map_or_else(
        || initial.outcome.clone(),
        |row| row.new_activity_outcome.clone(),
    );
    let new_outcome = outcome.as_str();
    if old_outcome == new_outcome {
        return Err(AppError::ValidationError(
            "Corrected result must differ from the effective result".into(),
        ));
    }
    let inserted: ActivityCorrectionRow = sqlx::query_as(
        r#"INSERT INTO academic_result_corrections (
               activity_result_id,old_activity_outcome,new_activity_outcome,
               expected_effective_version,corrected_by
           ) VALUES ($1,$2,$3,$4,$5)
           RETURNING id,old_activity_outcome,new_activity_outcome,
                     expected_effective_version,corrected_by,corrected_at"#,
    )
    .bind(initial.id)
    .bind(&old_outcome)
    .bind(new_outcome)
    .bind(current_version)
    .bind(actor.user_id)
    .fetch_one(&mut **tx)
    .await?;
    history.push(inserted);

    Ok(EffectiveResult {
        result_id: initial.id,
        student_academic_year_id: initial.student_academic_year_id,
        initial: activity_value(&initial.outcome)?,
        effective: activity_value(new_outcome)?,
        effective_version: current_version + 1,
        corrections: history
            .into_iter()
            .map(activity_correction_record)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

async fn correct_learner_evaluation(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    context: &ResultContext,
    subject_student_evaluation_id: Uuid,
    quality_level: i16,
    expected_effective_version: i64,
) -> Result<EffectiveResult, AppError> {
    if expected_effective_version <= 0 {
        return Err(AppError::ValidationError(
            "expectedEffectiveVersion must be greater than zero".into(),
        ));
    }
    if !(0..=3).contains(&quality_level) {
        return Err(AppError::ValidationError(
            "Quality level must be 0 through 3".into(),
        ));
    }
    let initial: LearnerEvaluationResultRow = sqlx::query_as(
        r#"SELECT id,student_academic_year_id,quality_level,row_version
           FROM subject_term_student_evaluations
           WHERE id=$1 AND academic_term_id=$2 AND academic_year_id=$3
           FOR UPDATE"#,
    )
    .bind(subject_student_evaluation_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Official learner evaluation not found".into()))?;
    let mut history: Vec<LearnerEvaluationCorrectionRow> = sqlx::query_as(
        r#"SELECT id,old_quality_level,new_quality_level,
                  expected_effective_version,corrected_by,corrected_at
           FROM academic_result_corrections
           WHERE subject_student_evaluation_id=$1
           ORDER BY expected_effective_version,id"#,
    )
    .bind(initial.id)
    .fetch_all(&mut **tx)
    .await?;
    let current_version = history.last().map_or(initial.row_version, |row| {
        row.expected_effective_version + 1
    });
    if expected_effective_version != current_version {
        return Err(conflict());
    }
    let old_quality_level = history
        .last()
        .map_or(initial.quality_level, |row| row.new_quality_level);
    if old_quality_level == quality_level {
        return Err(AppError::ValidationError(
            "Corrected result must differ from the effective result".into(),
        ));
    }
    let inserted: LearnerEvaluationCorrectionRow = sqlx::query_as(
        r#"INSERT INTO academic_result_corrections (
               subject_student_evaluation_id,old_quality_level,new_quality_level,
               expected_effective_version,corrected_by
           ) VALUES ($1,$2,$3,$4,$5)
           RETURNING id,old_quality_level,new_quality_level,
                     expected_effective_version,corrected_by,corrected_at"#,
    )
    .bind(initial.id)
    .bind(old_quality_level)
    .bind(quality_level)
    .bind(current_version)
    .bind(actor.user_id)
    .fetch_one(&mut **tx)
    .await?;
    history.push(inserted);

    Ok(EffectiveResult {
        result_id: initial.id,
        student_academic_year_id: initial.student_academic_year_id,
        initial: learner_evaluation_value(initial.quality_level)?,
        effective: learner_evaluation_value(quality_level)?,
        effective_version: current_version + 1,
        corrections: history
            .into_iter()
            .map(learner_evaluation_correction_record)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub async fn correct_result(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    input: ResultCorrectionInput,
) -> Result<EffectiveResult, AppError> {
    let can_correct = match &input {
        ResultCorrectionInput::Course { .. } | ResultCorrectionInput::Activity { .. } => {
            academic_result_access_policy::can_correct(actor)
        }
        ResultCorrectionInput::LearnerEvaluation { .. } => {
            learner_evaluation_access_policy::can_correct(actor)
        }
    };
    if !can_correct {
        return Err(AppError::Forbidden(
            "Official result correction permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let result = match input {
        ResultCorrectionInput::Course {
            course_result_id,
            outcome,
            numeric_grade,
            expected_effective_version,
        } => {
            correct_course(
                &mut tx,
                actor,
                context,
                course_result_id,
                outcome,
                numeric_grade,
                expected_effective_version,
            )
            .await?
        }
        ResultCorrectionInput::Activity {
            activity_result_id,
            outcome,
            expected_effective_version,
        } => {
            correct_activity(
                &mut tx,
                actor,
                context,
                activity_result_id,
                outcome,
                expected_effective_version,
            )
            .await?
        }
        ResultCorrectionInput::LearnerEvaluation {
            subject_student_evaluation_id,
            quality_level,
            expected_effective_version,
        } => {
            correct_learner_evaluation(
                &mut tx,
                actor,
                context,
                subject_student_evaluation_id,
                i16::from(quality_level),
                expected_effective_version,
            )
            .await?
        }
    };
    tx.commit().await?;
    Ok(result)
}
