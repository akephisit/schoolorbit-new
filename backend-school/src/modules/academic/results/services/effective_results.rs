use super::{decimal_wire, validate_context};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    policies::{
        academic_result_access_policy, learner_evaluation_access_policy,
        resource_access_policy::AcademicResourceListFilter,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::modules::academic::results::models::*;

#[derive(sqlx::FromRow)]
struct EffectiveSearchRow {
    kind: String,
    result_id: Uuid,
    student_academic_year_id: Uuid,
    student_code: Option<String>,
    display_name: String,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    offering_code: String,
    offering_name: String,
    group_name: String,
    subject_id: Option<Uuid>,
    domain: Option<String>,
    subject_term_criterion_id: Option<Uuid>,
    criterion_name: Option<String>,
    initial_course_outcome: Option<String>,
    initial_numeric_grade: Option<BigDecimal>,
    initial_activity_outcome: Option<String>,
    initial_quality_level: Option<i16>,
    effective_course_outcome: Option<String>,
    effective_numeric_grade: Option<BigDecimal>,
    effective_activity_outcome: Option<String>,
    effective_quality_level: Option<i16>,
    effective_version: i64,
}

#[derive(sqlx::FromRow)]
struct CorrectionRow {
    id: Uuid,
    course_result_id: Option<Uuid>,
    activity_result_id: Option<Uuid>,
    subject_student_evaluation_id: Option<Uuid>,
    old_course_outcome: Option<String>,
    new_course_outcome: Option<String>,
    old_numeric_grade: Option<BigDecimal>,
    new_numeric_grade: Option<BigDecimal>,
    old_activity_outcome: Option<String>,
    new_activity_outcome: Option<String>,
    old_quality_level: Option<i16>,
    new_quality_level: Option<i16>,
    expected_effective_version: i64,
    corrected_by: Uuid,
    corrected_at: DateTime<Utc>,
}

fn optional_access(
    result: Result<AcademicResourceListFilter, AppError>,
) -> Result<Option<AcademicResourceListFilter>, AppError> {
    match result {
        Ok(filter) => Ok(Some(filter)),
        Err(AppError::Forbidden(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn kind(value: &str) -> Result<EffectiveResultKind, AppError> {
    match value {
        "course" => Ok(EffectiveResultKind::Course),
        "activity" => Ok(EffectiveResultKind::Activity),
        "learner_evaluation" => Ok(EffectiveResultKind::LearnerEvaluation),
        _ => Err(AppError::InternalServerError(
            "Stored effective result family is invalid".into(),
        )),
    }
}

fn course_value(
    outcome: Option<&str>,
    numeric_grade: Option<&BigDecimal>,
) -> Result<EffectiveResultValue, AppError> {
    let outcome = outcome.ok_or_else(|| {
        AppError::InternalServerError("Stored course result outcome is missing".into())
    })?;
    let outcome = CourseOfficialOutcome::try_from(outcome)
        .map_err(|message| AppError::InternalServerError(message.into()))?;
    Ok(EffectiveResultValue::Course {
        outcome,
        numeric_grade: numeric_grade.map(decimal_wire),
    })
}

fn activity_value(outcome: Option<&str>) -> Result<EffectiveResultValue, AppError> {
    let outcome = outcome.ok_or_else(|| {
        AppError::InternalServerError("Stored activity result outcome is missing".into())
    })?;
    let outcome = ActivityOutcome::try_from(outcome)
        .map_err(|message| AppError::InternalServerError(message.into()))?;
    Ok(EffectiveResultValue::Activity { outcome })
}

fn learner_value(level: Option<i16>) -> Result<EffectiveResultValue, AppError> {
    let quality_level = level.ok_or_else(|| {
        AppError::InternalServerError("Stored learner evaluation level is missing".into())
    })?;
    if !(0..=3).contains(&quality_level) {
        return Err(AppError::InternalServerError(
            "Stored learner evaluation level is invalid".into(),
        ));
    }
    Ok(EffectiveResultValue::LearnerEvaluation { quality_level })
}

fn correction_record(
    row: CorrectionRow,
) -> Result<(String, Uuid, ResultCorrectionRecord), AppError> {
    let (family, result_id, previous, corrected) = if let Some(id) = row.course_result_id {
        (
            "course",
            id,
            course_value(
                row.old_course_outcome.as_deref(),
                row.old_numeric_grade.as_ref(),
            )?,
            course_value(
                row.new_course_outcome.as_deref(),
                row.new_numeric_grade.as_ref(),
            )?,
        )
    } else if let Some(id) = row.activity_result_id {
        (
            "activity",
            id,
            activity_value(row.old_activity_outcome.as_deref())?,
            activity_value(row.new_activity_outcome.as_deref())?,
        )
    } else if let Some(id) = row.subject_student_evaluation_id {
        (
            "learner_evaluation",
            id,
            learner_value(row.old_quality_level)?,
            learner_value(row.new_quality_level)?,
        )
    } else {
        return Err(AppError::InternalServerError(
            "Stored result correction has no target".into(),
        ));
    };
    Ok((
        family.into(),
        result_id,
        ResultCorrectionRecord {
            id: row.id,
            expected_effective_version: row.expected_effective_version,
            previous,
            corrected,
            corrected_by: row.corrected_by,
            corrected_at: row.corrected_at,
        },
    ))
}

fn initial_value(row: &EffectiveSearchRow) -> Result<EffectiveResultValue, AppError> {
    match row.kind.as_str() {
        "course" => course_value(
            row.initial_course_outcome.as_deref(),
            row.initial_numeric_grade.as_ref(),
        ),
        "activity" => activity_value(row.initial_activity_outcome.as_deref()),
        "learner_evaluation" => learner_value(row.initial_quality_level),
        _ => Err(AppError::InternalServerError(
            "Stored effective result family is invalid".into(),
        )),
    }
}

fn effective_value(row: &EffectiveSearchRow) -> Result<EffectiveResultValue, AppError> {
    match row.kind.as_str() {
        "course" => course_value(
            row.effective_course_outcome.as_deref(),
            row.effective_numeric_grade.as_ref(),
        ),
        "activity" => activity_value(row.effective_activity_outcome.as_deref()),
        "learner_evaluation" => learner_value(row.effective_quality_level),
        _ => Err(AppError::InternalServerError(
            "Stored effective result family is invalid".into(),
        )),
    }
}

pub async fn search_effective_results(
    pool: &PgPool,
    actor: &ActorContext,
    query: &EffectiveResultSearch,
) -> Result<Vec<EffectiveResultSearchItem>, AppError> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(AppError::ValidationError(
            "Effective result search limit must be between 1 and 100".into(),
        ));
    }
    let result_access =
        optional_access(academic_result_access_policy::list_access(pool, actor).await)?;
    let learner_access =
        optional_access(learner_evaluation_access_policy::list_access(pool, actor).await)?;
    if result_access.is_none() && learner_access.is_none() {
        return Err(AppError::Forbidden(
            "Official result read permission is required".into(),
        ));
    }
    let result_access = result_access.unwrap_or_default();
    let learner_access = learner_access.unwrap_or_default();
    let result_units = result_access.allowed_organization_unit_ids();
    let learner_units = learner_access.allowed_organization_unit_ids();
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > 100) {
        return Err(AppError::ValidationError(
            "Effective result search text exceeds 100 characters".into(),
        ));
    }
    let requested_kind = query.kind.map(|value| match value {
        EffectiveResultKind::Course => "course",
        EffectiveResultKind::Activity => "activity",
        EffectiveResultKind::LearnerEvaluation => "learner_evaluation",
    });
    let context = ResultContext {
        academic_year_id: query.academic_year_id,
        academic_term_id: query.academic_term_id,
    };
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_context(&mut tx, &context).await?;
    let rows: Vec<EffectiveSearchRow> = sqlx::query_as(
        r#"WITH visible AS (
            SELECT 'course'::text AS kind,result.id AS result_id,result.student_academic_year_id,
                   student_info.student_id AS student_code,
                   concat_ws(' ',nullif(btrim(student.title),''),student.first_name,student.last_name) AS display_name,
                   result.learning_group_id,result.learning_offering_id,offering.code_snapshot AS offering_code,
                   offering.name_snapshot AS offering_name,learning_group.name AS group_name,result.subject_id,
                   NULL::text AS domain,NULL::uuid AS subject_term_criterion_id,NULL::text AS criterion_name,
                   result.outcome AS initial_course_outcome,result.numeric_grade AS initial_numeric_grade,
                   NULL::text AS initial_activity_outcome,NULL::smallint AS initial_quality_level,
                   COALESCE(correction.new_course_outcome,result.outcome) AS effective_course_outcome,
                   CASE WHEN correction.expected_effective_version IS NULL
                        THEN result.numeric_grade ELSE correction.new_numeric_grade END AS effective_numeric_grade,
                   NULL::text AS effective_activity_outcome,NULL::smallint AS effective_quality_level,
                   COALESCE(correction.expected_effective_version+1,result.row_version) AS effective_version
            FROM academic_course_results result
            JOIN learning_groups learning_group ON learning_group.id=result.learning_group_id
            JOIN learning_offerings offering ON offering.id=result.learning_offering_id
            JOIN academic_terms term ON term.id=result.academic_term_id
            JOIN student_academic_years student_year ON student_year.id=result.student_academic_year_id
            JOIN users student ON student.id=student_year.student_id
            LEFT JOIN student_info ON student_info.user_id=student.id
            LEFT JOIN LATERAL (
                SELECT item.new_course_outcome,item.new_numeric_grade,item.expected_effective_version
                FROM academic_result_corrections item WHERE item.course_result_id=result.id
                ORDER BY item.expected_effective_version DESC,item.id DESC LIMIT 1
            ) correction ON true
            WHERE result.academic_term_id=$1 AND result.academic_year_id=$2
              AND ($4 OR offering.owning_organization_unit_id=ANY($5)
                   OR ($6 AND EXISTS(SELECT 1 FROM learning_group_teachers teacher
                       JOIN users teacher_account ON teacher_account.id=teacher.teacher_id AND teacher_account.status='active'
                       WHERE teacher.learning_group_id=learning_group.id AND teacher.teacher_id=$3
                         AND teacher.starts_on<=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)
                         AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)))))
            UNION ALL
            SELECT 'activity'::text,result.id,result.student_academic_year_id,student_info.student_id,
                   concat_ws(' ',nullif(btrim(student.title),''),student.first_name,student.last_name),
                   result.learning_group_id,result.learning_offering_id,offering.code_snapshot,offering.name_snapshot,
                   learning_group.name,NULL::uuid,NULL::text,NULL::uuid,NULL::text,
                   NULL::text,NULL::numeric,result.outcome,NULL::smallint,
                   NULL::text,NULL::numeric,COALESCE(correction.new_activity_outcome,result.outcome),NULL::smallint,
                   COALESCE(correction.expected_effective_version+1,result.row_version)
            FROM academic_activity_results result
            JOIN learning_groups learning_group ON learning_group.id=result.learning_group_id
            JOIN learning_offerings offering ON offering.id=result.learning_offering_id
            JOIN academic_terms term ON term.id=result.academic_term_id
            JOIN student_academic_years student_year ON student_year.id=result.student_academic_year_id
            JOIN users student ON student.id=student_year.student_id
            LEFT JOIN student_info ON student_info.user_id=student.id
            LEFT JOIN LATERAL (
                SELECT item.new_activity_outcome,item.expected_effective_version
                FROM academic_result_corrections item WHERE item.activity_result_id=result.id
                ORDER BY item.expected_effective_version DESC,item.id DESC LIMIT 1
            ) correction ON true
            WHERE result.academic_term_id=$1 AND result.academic_year_id=$2
              AND ($4 OR offering.owning_organization_unit_id=ANY($5)
                   OR ($6 AND EXISTS(SELECT 1 FROM learning_group_teachers teacher
                       JOIN users teacher_account ON teacher_account.id=teacher.teacher_id AND teacher_account.status='active'
                       WHERE teacher.learning_group_id=learning_group.id AND teacher.teacher_id=$3
                         AND teacher.starts_on<=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)
                         AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)))))
            UNION ALL
            SELECT 'learner_evaluation'::text,result.id,result.student_academic_year_id,student_info.student_id,
                   concat_ws(' ',nullif(btrim(student.title),''),student.first_name,student.last_name),
                   result.learning_group_id,result.learning_offering_id,offering.code_snapshot,offering.name_snapshot,
                   learning_group.name,result.subject_id,result.domain,result.subject_term_criterion_id,criterion.name,
                   NULL::text,NULL::numeric,NULL::text,result.quality_level,
                   NULL::text,NULL::numeric,NULL::text,COALESCE(correction.new_quality_level,result.quality_level),
                   COALESCE(correction.expected_effective_version+1,result.row_version)
            FROM subject_term_student_evaluations result
            JOIN subject_term_evaluation_locks result_lock ON result_lock.id=result.evaluation_lock_id
            CROSS JOIN LATERAL jsonb_to_recordset(result_lock.source_snapshot->'criteria') AS criterion(id uuid,name text)
            JOIN learning_groups learning_group ON learning_group.id=result.learning_group_id
            JOIN learning_offerings offering ON offering.id=result.learning_offering_id
            JOIN academic_terms term ON term.id=result.academic_term_id
            JOIN student_academic_years student_year ON student_year.id=result.student_academic_year_id
            JOIN users student ON student.id=student_year.student_id
            LEFT JOIN student_info ON student_info.user_id=student.id
            LEFT JOIN LATERAL (
                SELECT item.new_quality_level,item.expected_effective_version
                FROM academic_result_corrections item WHERE item.subject_student_evaluation_id=result.id
                ORDER BY item.expected_effective_version DESC,item.id DESC LIMIT 1
            ) correction ON true
            WHERE criterion.id=result.subject_term_criterion_id
              AND result.academic_term_id=$1 AND result.academic_year_id=$2
              AND ($7 OR offering.owning_organization_unit_id=ANY($8)
                   OR ($9 AND (EXISTS(SELECT 1 FROM learning_group_teachers teacher
                       JOIN users teacher_account ON teacher_account.id=teacher.teacher_id AND teacher_account.status='active'
                       WHERE teacher.learning_group_id=learning_group.id AND teacher.teacher_id=$3
                         AND teacher.starts_on<=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)
                         AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,term.start_date),term.planned_end_date)))
                       OR EXISTS(SELECT 1 FROM course_assessment_plans plan
                           JOIN course_offering_details detail ON detail.learning_offering_id=plan.learning_offering_id
                           WHERE detail.subject_id=result.subject_id AND plan.academic_term_id=result.academic_term_id
                             AND plan.academic_year_id=result.academic_year_id AND plan.assessment_coordinator_id=$3))))
        )
        SELECT * FROM visible
        WHERE ($10::text IS NULL OR kind=$10)
          AND ($11::text IS NULL OR student_code ILIKE '%'||$11||'%'
               OR display_name ILIKE '%'||$11||'%' OR offering_code ILIKE '%'||$11||'%'
               OR offering_name ILIKE '%'||$11||'%' OR group_name ILIKE '%'||$11||'%'
               OR COALESCE(criterion_name,'') ILIKE '%'||$11||'%')
        ORDER BY display_name,student_academic_year_id,kind,offering_code,group_name,
                 subject_term_criterion_id,result_id
        LIMIT $12"#,
    )
    .bind(query.academic_term_id)
    .bind(query.academic_year_id)
    .bind(actor.user_id)
    .bind(result_access.includes_school_owned)
    .bind(&result_units)
    .bind(result_access.assigned_actor_id.is_some())
    .bind(learner_access.includes_school_owned)
    .bind(&learner_units)
    .bind(learner_access.assigned_actor_id.is_some())
    .bind(requested_kind)
    .bind(search)
    .bind(i64::from(limit))
    .fetch_all(&mut *tx)
    .await?;

    let course_ids = rows
        .iter()
        .filter(|row| row.kind == "course")
        .map(|row| row.result_id)
        .collect::<Vec<_>>();
    let activity_ids = rows
        .iter()
        .filter(|row| row.kind == "activity")
        .map(|row| row.result_id)
        .collect::<Vec<_>>();
    let learner_ids = rows
        .iter()
        .filter(|row| row.kind == "learner_evaluation")
        .map(|row| row.result_id)
        .collect::<Vec<_>>();
    let corrections: Vec<CorrectionRow> = sqlx::query_as(
        r#"SELECT id,course_result_id,activity_result_id,subject_student_evaluation_id,
                  old_course_outcome,new_course_outcome,old_numeric_grade,new_numeric_grade,
                  old_activity_outcome,new_activity_outcome,old_quality_level,new_quality_level,
                  expected_effective_version,corrected_by,corrected_at
           FROM academic_result_corrections
           WHERE course_result_id=ANY($1) OR activity_result_id=ANY($2)
              OR subject_student_evaluation_id=ANY($3)
           ORDER BY COALESCE(course_result_id,activity_result_id,subject_student_evaluation_id),
                    expected_effective_version,id"#,
    )
    .bind(&course_ids)
    .bind(&activity_ids)
    .bind(&learner_ids)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut histories: HashMap<(String, Uuid), Vec<ResultCorrectionRecord>> = HashMap::new();
    for correction in corrections {
        let (family, result_id, record) = correction_record(correction)?;
        histories
            .entry((family, result_id))
            .or_default()
            .push(record);
    }
    rows.into_iter()
        .map(|row| {
            let row_kind = kind(&row.kind)?;
            let initial = initial_value(&row)?;
            let effective = effective_value(&row)?;
            let corrections = histories
                .remove(&(row.kind.clone(), row.result_id))
                .unwrap_or_default();
            Ok(EffectiveResultSearchItem {
                kind: row_kind,
                student_code: row.student_code,
                display_name: row.display_name,
                learning_group_id: row.learning_group_id,
                learning_offering_id: row.learning_offering_id,
                offering_code: row.offering_code,
                offering_name: row.offering_name,
                group_name: row.group_name,
                subject_id: row.subject_id,
                domain: row.domain,
                subject_term_criterion_id: row.subject_term_criterion_id,
                criterion_name: row.criterion_name,
                result: EffectiveResult {
                    result_id: row.result_id,
                    student_academic_year_id: row.student_academic_year_id,
                    initial,
                    effective,
                    effective_version: row.effective_version,
                    corrections,
                },
            })
        })
        .collect()
}
