use sqlx::{types::Json, PgPool};
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{
    AcademicContextOptions, AcademicTermOption, AcademicTermStatus, AcademicYearOption,
    AcademicYearStatus,
};

pub async fn list_public_options(pool: &PgPool) -> Result<AcademicContextOptions, AppError> {
    let mut options = list_options(pool).await?;
    options
        .years
        .retain(|year| year.status != AcademicYearStatus::Planning);
    let public_year_ids = options
        .years
        .iter()
        .map(|year| year.id)
        .collect::<HashSet<_>>();
    options.terms.retain(|term| {
        public_year_ids.contains(&term.academic_year_id)
            && !matches!(
                term.status,
                AcademicTermStatus::Planning | AcademicTermStatus::Cancelled
            )
    });
    Ok(options)
}

pub async fn list_options(pool: &PgPool) -> Result<AcademicContextOptions, AppError> {
    let (years, terms, active_academic_year_id, active_academic_term_id): (
        Json<Vec<AcademicYearOption>>,
        Json<Vec<AcademicTermOption>>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        r#"
        SELECT
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', year.id,
                        'year', year.year,
                        'name', year.name,
                        'startDate', year.start_date,
                        'endDate', year.end_date,
                        'status', year.status
                    ) ORDER BY year.year DESC, year.start_date DESC, year.id
                )
                FROM academic_years year
            ), '[]'::jsonb),
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', term.id,
                        'academicYearId', term.academic_year_id,
                        'sequence', term.sequence_no,
                        'code', term.code,
                        'name', term.name,
                        'termType', term.term_type,
                        'startDate', term.start_date,
                        'plannedEndDate', term.planned_end_date,
                        'closedOn', term.closed_on,
                        'includedInYearResult', term.included_in_year_result,
                        'blocksYearClosure', term.blocks_year_closure,
                        'status', term.status
                    ) ORDER BY year.year DESC, term.sequence_no, term.start_date, term.id
                )
                FROM academic_terms term
                JOIN academic_years year ON year.id = term.academic_year_id
            ), '[]'::jsonb),
            (SELECT year.id FROM academic_years year WHERE year.status = 'active'),
            (SELECT term.id FROM academic_terms term WHERE term.status = 'active')
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(reason = "academic_context_options_query_failed", database_error = %error);
        AppError::InternalServerError("ไม่สามารถโหลดตัวเลือกปีและภาคเรียนได้".to_string())
    })?;

    Ok(AcademicContextOptions {
        years: years.0,
        terms: terms.0,
        active_academic_year_id,
        active_academic_term_id,
    })
}

pub async fn list_options_for_student(
    pool: &PgPool,
    student_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    let (years, terms, active_academic_year_id, active_academic_term_id): (
        Json<Vec<AcademicYearOption>>,
        Json<Vec<AcademicTermOption>>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        r#"
        SELECT
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', year.id,
                        'year', year.year,
                        'name', year.name,
                        'startDate', year.start_date,
                        'endDate', year.end_date,
                        'status', year.status
                    ) ORDER BY year.year DESC, year.start_date DESC, year.id
                )
                FROM academic_years year
                WHERE EXISTS (
                    SELECT 1
                    FROM student_academic_years student_year
                    WHERE student_year.academic_year_id = year.id
                      AND student_year.student_id = $1
                )
            ), '[]'::jsonb),
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', term.id,
                        'academicYearId', term.academic_year_id,
                        'sequence', term.sequence_no,
                        'code', term.code,
                        'name', term.name,
                        'termType', term.term_type,
                        'startDate', term.start_date,
                        'plannedEndDate', term.planned_end_date,
                        'closedOn', term.closed_on,
                        'includedInYearResult', term.included_in_year_result,
                        'blocksYearClosure', term.blocks_year_closure,
                        'status', term.status
                    ) ORDER BY year.year DESC, term.sequence_no, term.start_date, term.id
                )
                FROM academic_terms term
                JOIN academic_years year ON year.id = term.academic_year_id
                WHERE EXISTS (
                    SELECT 1
                    FROM student_academic_years student_year
                    WHERE student_year.academic_year_id = term.academic_year_id
                      AND student_year.student_id = $1
                )
            ), '[]'::jsonb),
            (
                SELECT year.id
                FROM academic_years year
                WHERE year.status = 'active'
                  AND EXISTS (
                      SELECT 1
                      FROM student_academic_years student_year
                      WHERE student_year.academic_year_id = year.id
                        AND student_year.student_id = $1
                  )
            ),
            (
                SELECT term.id
                FROM academic_terms term
                WHERE term.status = 'active'
                  AND EXISTS (
                      SELECT 1
                      FROM student_academic_years student_year
                      WHERE student_year.academic_year_id = term.academic_year_id
                        AND student_year.student_id = $1
                  )
            )
        "#,
    )
    .bind(student_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(reason = "student_academic_context_options_query_failed", database_error = %error);
        AppError::InternalServerError("ไม่สามารถโหลดประวัติปีและภาคเรียนได้".to_string())
    })?;

    Ok(AcademicContextOptions {
        years: years.0,
        terms: terms.0,
        active_academic_year_id,
        active_academic_term_id,
    })
}

pub async fn list_options_for_parent(
    pool: &PgPool,
    parent_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    let (years, terms, active_academic_year_id, active_academic_term_id): (
        Json<Vec<AcademicYearOption>>,
        Json<Vec<AcademicTermOption>>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        r#"
        WITH linked_years AS (
            SELECT DISTINCT
                year.id, year.year, year.name, year.start_date, year.end_date, year.status
            FROM student_parents parent_link
            JOIN users student
              ON student.id = parent_link.student_user_id
             AND student.user_type = 'student'
             AND student.status = 'active'
            JOIN student_academic_years student_year
              ON student_year.student_id = student.id
            JOIN academic_years year ON year.id = student_year.academic_year_id
            WHERE parent_link.parent_user_id = $1
        ),
        linked_terms AS (
            SELECT term.*
            FROM academic_terms term
            JOIN linked_years year ON year.id = term.academic_year_id
        )
        SELECT
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', year.id,
                        'year', year.year,
                        'name', year.name,
                        'startDate', year.start_date,
                        'endDate', year.end_date,
                        'status', year.status
                    ) ORDER BY year.year DESC, year.start_date DESC, year.id
                )
                FROM linked_years year
            ), '[]'::jsonb),
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', term.id,
                        'academicYearId', term.academic_year_id,
                        'sequence', term.sequence_no,
                        'code', term.code,
                        'name', term.name,
                        'termType', term.term_type,
                        'startDate', term.start_date,
                        'plannedEndDate', term.planned_end_date,
                        'closedOn', term.closed_on,
                        'includedInYearResult', term.included_in_year_result,
                        'blocksYearClosure', term.blocks_year_closure,
                        'status', term.status
                    ) ORDER BY year.year DESC, term.sequence_no, term.start_date, term.id
                )
                FROM linked_terms term
                JOIN linked_years year ON year.id = term.academic_year_id
            ), '[]'::jsonb),
            (SELECT id FROM linked_years WHERE status = 'active'),
            (SELECT id FROM linked_terms WHERE status = 'active')
        "#,
    )
    .bind(parent_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(reason = "parent_academic_context_options_query_failed", database_error = %error);
        AppError::InternalServerError(
            "ไม่สามารถโหลดประวัติปีและภาคเรียนของบุตรหลานได้".to_string(),
        )
    })?;

    Ok(AcademicContextOptions {
        years: years.0,
        terms: terms.0,
        active_academic_year_id,
        active_academic_term_id,
    })
}
